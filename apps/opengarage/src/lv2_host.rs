//! Host for LV2 plug-ins, written against the public LV2 specification.
//!
//! An LV2 plug-in is a folder (`something.lv2`) holding a program library and
//! text files in the Turtle format that describe it. This host reads those
//! descriptions itself, runs effects and instruments, and exposes their control
//! ports as parameters. A plug-in that describes a window for Windows
//! (`ui:WindowsUI`) can also show it, inside a window of the host.

use crate::i18n::tr;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use og_engine::ExternalUnit;

use crate::clap_host::window::HostWindow;
use crate::clap_host::{HostShared, ParamInfo, PluginInfo};
use crate::plugin::{MAX_FRAMES, MAX_STATE};

/// Bytes of each event buffer (atom port).
const ATOM_BYTES: usize = 16 * 1024;

const RDF_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
const SEE_ALSO: &str = "http://www.w3.org/2000/01/rdf-schema#seeAlso";
const DOAP_NAME: &str = "http://usefulinc.com/ns/doap#name";
const LV2: &str = "http://lv2plug.in/ns/lv2core#";
const ATOM: &str = "http://lv2plug.in/ns/ext/atom#";
const MIDI_EVENT: &str = "http://lv2plug.in/ns/ext/midi#MidiEvent";
const URID_MAP: &str = "http://lv2plug.in/ns/ext/urid#map";
const URID_UNMAP: &str = "http://lv2plug.in/ns/ext/urid#unmap";
const OPTIONS: &str = "http://lv2plug.in/ns/ext/options#options";
const BOUNDED_BLOCK: &str = "http://lv2plug.in/ns/ext/buf-size#boundedBlockLength";
const UI: &str = "http://lv2plug.in/ns/extensions/ui#";
const INSTANCE_ACCESS: &str = "http://lv2plug.in/ns/ext/instance-access";
const DATA_ACCESS: &str = "http://lv2plug.in/ns/ext/data-access";
/// Features a plug-in may require: those above, and promises this host keeps anyway.
const SUPPORTED: [&str; 6] = [
    URID_MAP,
    URID_UNMAP,
    OPTIONS,
    BOUNDED_BLOCK,
    "http://lv2plug.in/ns/lv2core#isLive",
    "http://lv2plug.in/ns/lv2core#inPlaceBroken",
];

// ----- reading Turtle ----------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Object {
    /// A resource: a full address, a file name relative to the folder, or a blank node.
    Node(String),
    Literal(String),
}

/// A reader for the subset of Turtle that plug-in descriptions use.
struct Turtle<'a> {
    text: &'a [u8],
    pos: usize,
    prefixes: HashMap<String, String>,
    triples: Vec<(String, String, Object)>,
    blanks: usize,
    /// Distinguishes the blank nodes of different files.
    file: usize,
}

impl Turtle<'_> {
    fn skip(&mut self) {
        while let Some(&c) = self.text.get(self.pos) {
            if c == b'#' {
                while self.text.get(self.pos).is_some_and(|c| *c != b'\n') {
                    self.pos += 1;
                }
            } else if c.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip();
        self.text.get(self.pos).copied()
    }

    fn eat(&mut self, c: u8) -> bool {
        let found = self.peek() == Some(c);
        if found {
            self.pos += 1;
        }
        found
    }

    fn blank(&mut self) -> String {
        self.blanks += 1;
        format!("_:f{}b{}", self.file, self.blanks)
    }

    /// A word made of name characters, such as `lv2:port` or `@prefix`.
    fn word(&mut self) -> String {
        self.skip();
        let start = self.pos;
        while self.text.get(self.pos).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, b':' | b'_' | b'-' | b'.' | b'@' | b'+' | b'%') || *c >= 0x80) {
            self.pos += 1;
        }
        // A trailing dot ends the statement rather than the word.
        while self.pos > start + 1 && self.text[self.pos - 1] == b'.' {
            self.pos -= 1;
        }
        String::from_utf8_lossy(&self.text[start..self.pos]).into_owned()
    }

    fn iri_ref(&mut self) -> Option<String> {
        if !self.eat(b'<') {
            return None;
        }
        let start = self.pos;
        while self.text.get(self.pos).is_some_and(|c| *c != b'>') {
            self.pos += 1;
        }
        let iri = String::from_utf8_lossy(&self.text[start..self.pos]).into_owned();
        self.pos += 1;
        Some(iri)
    }

    fn expand(&self, name: &str) -> Option<String> {
        let (prefix, local) = name.split_once(':')?;
        Some(format!("{}{}", self.prefixes.get(prefix)?, local))
    }

    fn string(&mut self) -> Option<String> {
        let quote = self.peek().filter(|c| matches!(c, b'"' | b'\''))?;
        let long = self.text[self.pos..].starts_with(&[quote, quote, quote]);
        self.pos += if long { 3 } else { 1 };
        let mut out = Vec::new();
        loop {
            let c = *self.text.get(self.pos)?;
            if c == b'\\' {
                let escaped = *self.text.get(self.pos + 1)?;
                out.push(match escaped {
                    b'n' => b'\n',
                    b't' => b'\t',
                    b'r' => b'\r',
                    other => other,
                });
                self.pos += 2;
            } else if c == quote && (!long || self.text[self.pos..].starts_with(&[quote, quote, quote])) {
                self.pos += if long { 3 } else { 1 };
                break;
            } else {
                out.push(c);
                self.pos += 1;
            }
        }
        // A language tag or a datatype may follow; neither matters here.
        if self.text.get(self.pos) == Some(&b'@') {
            self.word();
        } else if self.text[self.pos..].starts_with(b"^^") {
            self.pos += 2;
            if self.iri_ref().is_none() {
                self.word();
            }
        }
        Some(String::from_utf8_lossy(&out).into_owned())
    }

    fn object(&mut self, depth: u32) -> Option<Object> {
        match self.peek()? {
            b'<' => self.iri_ref().map(Object::Node),
            b'"' | b'\'' => self.string().map(Object::Literal),
            b'[' => {
                self.pos += 1;
                let node = self.blank();
                if !self.eat(b']') {
                    self.properties(&node, depth + 1)?;
                    if !self.eat(b']') {
                        return None;
                    }
                }
                Some(Object::Node(node))
            }
            b'(' => {
                // A list: its items are read and dropped.
                self.pos += 1;
                while !self.eat(b')') {
                    self.object(depth + 1)?;
                }
                Some(Object::Node(self.blank()))
            }
            _ => {
                let word = self.word();
                if word.is_empty() {
                    None
                } else if let Some(label) = word.strip_prefix("_:") {
                    Some(Object::Node(format!("_:f{}{label}", self.file)))
                } else if let Some(iri) = self.expand(&word) {
                    Some(Object::Node(iri))
                } else {
                    // Numbers and booleans.
                    Some(Object::Literal(word))
                }
            }
        }
    }

    /// `predicate object, object; predicate object …` for one subject.
    fn properties(&mut self, subject: &str, depth: u32) -> Option<()> {
        if depth > 24 {
            return None;
        }
        loop {
            let predicate = match self.peek()? {
                b'<' => self.iri_ref()?,
                _ => {
                    let word = self.word();
                    if word == "a" {
                        RDF_TYPE.to_owned()
                    } else {
                        self.expand(&word)?
                    }
                }
            };
            loop {
                let object = self.object(depth)?;
                if self.triples.len() >= 200_000 {
                    return None;
                }
                self.triples.push((subject.to_owned(), predicate.clone(), object));
                if !self.eat(b',') {
                    break;
                }
            }
            if !self.eat(b';') {
                return Some(());
            }
            // A semicolon may be followed directly by the end of the block.
            if matches!(self.peek(), Some(b'.' | b']') | None) {
                return Some(());
            }
        }
    }

    fn statements(&mut self) -> Option<()> {
        while let Some(c) = self.peek() {
            // "@prefix", "@base", and their SPARQL spellings without the "@".
            let keyword = |word: &[u8]| {
                let rest = &self.text[self.pos..];
                rest.len() > word.len() && rest[..word.len()].eq_ignore_ascii_case(word) && rest[word.len()].is_ascii_whitespace()
            };
            if c == b'@' || keyword(b"prefix") || keyword(b"base") {
                let keyword = self.word().to_lowercase();
                if keyword.ends_with("prefix") {
                    let name = self.word();
                    let target = self.iri_ref()?;
                    self.prefixes.insert(name.trim_end_matches(':').to_owned(), target);
                } else {
                    self.iri_ref()?;
                }
                self.eat(b'.');
                continue;
            }
            let subject = match self.object(0)? {
                Object::Node(node) => node,
                Object::Literal(_) => return None,
            };
            // A blank node may stand alone as a statement.
            if !self.eat(b'.') {
                self.properties(&subject, 0)?;
                if !self.eat(b'.') {
                    return None;
                }
            }
        }
        Some(())
    }
}

/// The statements of a set of description files.
#[derive(Default)]
struct Graph {
    triples: Vec<(String, String, Object)>,
    files: usize,
}

impl Graph {
    fn read(&mut self, file: &Path) -> Result<(), String> {
        let text = std::fs::read(file).map_err(|e| format!("{} : {e}", file.display()))?;
        if text.len() > 8 << 20 {
            return Err(tr("the description file is too large").to_owned());
        }
        self.files += 1;
        let mut turtle = Turtle { text: &text, pos: 0, prefixes: HashMap::new(), triples: Vec::new(), blanks: 0, file: self.files };
        let complete = turtle.statements().is_some();
        // What was understood before a syntax problem is still used.
        self.triples.append(&mut turtle.triples);
        if !complete && self.triples.is_empty() {
            return Err(tf!("{}: unreadable description", file.display()));
        }
        Ok(())
    }

    fn objects<'a>(&'a self, subject: &str, predicate: &str) -> Vec<&'a Object> {
        self.triples.iter().filter(|t| t.0 == subject && t.1 == predicate).map(|t| &t.2).collect()
    }

    fn nodes<'a>(&'a self, subject: &str, predicate: &str) -> Vec<&'a str> {
        self.objects(subject, predicate)
            .into_iter()
            .filter_map(|o| match o {
                Object::Node(node) => Some(node.as_str()),
                Object::Literal(_) => None,
            })
            .collect()
    }

    fn literal<'a>(&'a self, subject: &str, predicate: &str) -> Option<&'a str> {
        self.objects(subject, predicate).into_iter().find_map(|o| match o {
            Object::Literal(text) => Some(text.as_str()),
            Object::Node(_) => None,
        })
    }

    fn number(&self, subject: &str, predicate: &str) -> Option<f32> {
        self.literal(subject, predicate)?.trim().parse::<f32>().ok().filter(|v| v.is_finite())
    }

    fn is_a(&self, subject: &str, class: &str) -> bool {
        self.nodes(subject, RDF_TYPE).contains(&class)
    }
}

/// Reads the descriptions of an LV2 folder: its manifest and the files it points to.
fn describe_bundle(bundle: &Path) -> Result<(Graph, Vec<String>), String> {
    let mut graph = Graph::default();
    graph.read(&bundle.join("manifest.ttl")).map_err(|e| tf!("this folder is not an LV2 plug-in ({e})" [e]))?;
    let plugin_class = format!("{LV2}Plugin");
    let plugins: Vec<String> = graph.triples.iter().filter(|t| t.1 == RDF_TYPE && t.2 == Object::Node(plugin_class.clone())).map(|t| t.0.clone()).collect();
    let mut extra: Vec<String> = plugins.iter().flat_map(|p| graph.nodes(p, SEE_ALSO)).map(str::to_owned).collect();
    extra.sort();
    extra.dedup();
    for file in extra.iter().filter(|f| !f.contains("://") && !f.contains("..")).take(64) {
        // A description that cannot be read loses detail, not the plug-in.
        let _ = graph.read(&bundle.join(file));
    }
    // The windows of the plug-ins may be described in files of their own.
    let windows: Vec<String> = plugins.iter().flat_map(|p| graph.nodes(p, &format!("{UI}ui"))).map(str::to_owned).collect();
    let mut more: Vec<String> = windows.iter().flat_map(|w| graph.nodes(w, SEE_ALSO)).map(str::to_owned).filter(|f| !extra.contains(f)).collect();
    more.sort();
    more.dedup();
    for file in more.iter().filter(|f| !f.contains("://") && !f.contains("..")).take(64) {
        let _ = graph.read(&bundle.join(file));
    }
    Ok((graph, plugins))
}

/// The plug-ins of an LV2 folder. Nothing from the folder is run.
pub fn describe(bundle: &Path) -> Result<Vec<PluginInfo>, String> {
    let (graph, plugins) = describe_bundle(bundle)?;
    let instrument_class = format!("{LV2}InstrumentPlugin");
    let infos: Vec<PluginInfo> = plugins
        .iter()
        .map(|uri| PluginInfo {
            id: uri.clone(),
            name: graph.literal(uri, DOAP_NAME).unwrap_or(uri.rsplit(['/', '#']).next().unwrap_or(uri)).to_owned(),
            instrument: graph.is_a(uri, &instrument_class),
        })
        .collect();
    if infos.is_empty() {
        return Err(tr("this folder does not describe any LV2 plug-in").to_owned());
    }
    Ok(infos)
}

// ----- the binary interface ---------------------------------------------------------------

#[repr(C)]
struct Feature {
    uri: *const c_char,
    data: *mut c_void,
}

#[repr(C)]
struct Descriptor {
    uri: *const c_char,
    instantiate: Option<unsafe extern "C" fn(*const Descriptor, f64, *const c_char, *const *const Feature) -> *mut c_void>,
    connect_port: Option<unsafe extern "C" fn(*mut c_void, u32, *mut c_void)>,
    activate: Option<unsafe extern "C" fn(*mut c_void)>,
    run: Option<unsafe extern "C" fn(*mut c_void, u32)>,
    deactivate: Option<unsafe extern "C" fn(*mut c_void)>,
    cleanup: Option<unsafe extern "C" fn(*mut c_void)>,
    extension_data: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}

#[repr(C)]
struct UridMap {
    handle: *mut c_void,
    map: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
}

#[repr(C)]
struct UridUnmap {
    handle: *mut c_void,
    unmap: unsafe extern "C" fn(*mut c_void, u32) -> *const c_char,
}

/// The numbers handed out for addresses (LV2 "URIDs"): position in the list, plus one.
type Urids = Mutex<Vec<CString>>;

unsafe extern "C" fn urid_map(handle: *mut c_void, uri: *const c_char) -> u32 {
    let (Some(urids), false) = (handle.cast::<Urids>().as_ref(), uri.is_null()) else { return 0 };
    let uri = CStr::from_ptr(uri);
    let Ok(mut list) = urids.lock() else { return 0 };
    if let Some(index) = list.iter().position(|known| known.as_c_str() == uri) {
        return index as u32 + 1;
    }
    if list.len() >= 65_536 {
        return 0;
    }
    list.push(uri.to_owned());
    list.len() as u32
}

unsafe extern "C" fn urid_unmap(handle: *mut c_void, urid: u32) -> *const c_char {
    let Some(urids) = handle.cast::<Urids>().as_ref() else { return null() };
    let Ok(list) = urids.lock() else { return null() };
    // The strings live in boxes that the list never drops, so the pointer stays valid.
    list.get(urid.wrapping_sub(1) as usize).map_or(null(), |uri| uri.as_ptr())
}

enum Port {
    AudioIn(Vec<f32>),
    AudioOut(Vec<f32>),
    /// Index into the instance's control values.
    Control(usize),
    /// An event buffer the plug-in reads; `midi` when notes go there.
    EventsIn { buffer: Vec<u64>, midi: bool },
    EventsOut { buffer: Vec<u64> },
}

/// What a running plug-in shares with its window: the value of each control
/// port, by port index, and what a window may ask to reach the plug-in directly.
pub struct Lv2Shared {
    /// One per port, as the bits of an `f32`; only control ports are used.
    controls: Vec<AtomicU32>,
    /// Which ports are control ports, and whether the plug-in reads them.
    control_ports: Vec<(u32, bool)>,
    handle: *mut c_void,
    extension_data: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
    /// The plug-in's functions to save and restore what its ports do not hold.
    state: *const StateInterface,
    urids: Arc<Urids>,
    /// Held while the plug-in computes sound or handles its state, which LV2
    /// forbids doing at the same time; false once the plug-in is gone.
    gate: Mutex<bool>,
}

// SAFETY: the pointers are only handed to the plug-in's own code, as the
// "instance access" and "state" features of LV2 prescribe, and only while
// `gate` says the plug-in is alive; the values are atomics.
unsafe impl Send for Lv2Shared {}
unsafe impl Sync for Lv2Shared {}

const STATE_INTERFACE: &str = "http://lv2plug.in/ns/ext/state#interface";
/// The value is plain data that can be copied and kept: the only kind saved.
const STATE_IS_POD: u32 = 1;

type StoreFn = unsafe extern "C" fn(*mut c_void, u32, *const c_void, usize, u32, u32) -> u32;
type RetrieveFn = unsafe extern "C" fn(*mut c_void, u32, *mut usize, *mut u32, *mut u32) -> *const c_void;

#[repr(C)]
struct StateInterface {
    save: Option<unsafe extern "C" fn(*mut c_void, StoreFn, *mut c_void, u32, *const *const Feature) -> u32>,
    restore: Option<unsafe extern "C" fn(*mut c_void, RetrieveFn, *mut c_void, u32, *const *const Feature) -> u32>,
}

/// One value of a plug-in's state: what it is called, what it is, and its bytes.
/// Names are kept as addresses, since their numbers change from run to run.
struct Property {
    key: String,
    kind: String,
    flags: u32,
    value: Vec<u8>,
}

/// What `store` and `retrieve` work on.
struct StateBag<'a> {
    urids: &'a Urids,
    properties: Vec<Property>,
}

fn urid_text(urids: &Urids, urid: u32) -> Option<String> {
    let list = urids.lock().ok()?;
    Some(list.get(urid.checked_sub(1)? as usize)?.to_string_lossy().into_owned())
}

unsafe extern "C" fn state_store(handle: *mut c_void, key: u32, value: *const c_void, size: usize, kind: u32, flags: u32) -> u32 {
    let Some(bag) = handle.cast::<StateBag>().as_mut() else { return 1 };
    let (Some(key), Some(kind)) = (urid_text(bag.urids, key), urid_text(bag.urids, kind)) else { return 1 };
    if value.is_null() || size > MAX_STATE || bag.properties.len() >= 4096 {
        return 1;
    }
    // A value that points elsewhere cannot be kept: it is left out.
    if flags & STATE_IS_POD != 0 {
        bag.properties.retain(|p| p.key != key);
        bag.properties.push(Property { key, kind, flags, value: std::slice::from_raw_parts(value.cast::<u8>(), size).to_vec() });
    }
    0
}

unsafe extern "C" fn state_retrieve(handle: *mut c_void, key: u32, size: *mut usize, kind: *mut u32, flags: *mut u32) -> *const c_void {
    let Some(bag) = handle.cast::<StateBag>().as_ref() else { return null() };
    let Some(wanted) = urid_text(bag.urids, key) else { return null() };
    let Some(property) = bag.properties.iter().find(|p| p.key == wanted) else { return null() };
    let Ok(kind_text) = CString::new(property.kind.as_str()) else { return null() };
    if !size.is_null() {
        *size = property.value.len();
    }
    if !kind.is_null() {
        *kind = urid_map((&raw const *bag.urids).cast_mut().cast(), kind_text.as_ptr());
    }
    if !flags.is_null() {
        *flags = property.flags;
    }
    property.value.as_ptr().cast()
}

fn put(out: &mut Vec<u8>, bytes: &[u8]) {
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
}

fn take<'a>(data: &mut &'a [u8]) -> Option<&'a [u8]> {
    let length = u32::from_le_bytes(data.get(..4)?.try_into().ok()?) as usize;
    let bytes = data.get(4..4 + length)?;
    *data = &data[4 + length..];
    Some(bytes)
}

impl Lv2Shared {
    /// Runs `with` on the plug-in's state functions while no sound is being
    /// computed, with the features those functions may ask for.
    fn with_state<R>(&self, with: impl FnOnce(&StateInterface, *mut c_void, *const *const Feature) -> R) -> Option<R> {
        let alive = self.gate.lock().ok()?;
        // SAFETY: the pointer came from the plug-in's own `extension_data` and
        // stays valid while the plug-in is alive, which the gate attests.
        let interface = unsafe { self.state.as_ref() }.filter(|_| *alive)?;
        let urids = Arc::as_ptr(&self.urids).cast_mut().cast::<c_void>();
        let (mut map, mut unmap) = (UridMap { handle: urids, map: urid_map }, UridUnmap { handle: urids, unmap: urid_unmap });
        let names = [CString::new(URID_MAP).ok()?, CString::new(URID_UNMAP).ok()?];
        let features = [Feature { uri: names[0].as_ptr(), data: (&raw mut map).cast() }, Feature { uri: names[1].as_ptr(), data: (&raw mut unmap).cast() }];
        let list = [&raw const features[0], &raw const features[1], null()];
        Some(with(interface, self.handle, list.as_ptr()))
    }

    /// What the plug-in wants remembered beyond the values of its ports.
    fn save_state(&self) -> Option<Vec<u8>> {
        let mut bag = StateBag { urids: &self.urids, properties: Vec::new() };
        let status = self.with_state(|interface, handle, features| {
            let save = interface.save?;
            // SAFETY: the plug-in's own function, on itself, with a bag that
            // outlives the call. Flags: plain data that can be carried elsewhere.
            Some(unsafe { save(handle, state_store, (&raw mut bag).cast(), 3, features) })
        })??;
        if status != 0 {
            return None;
        }
        let mut out = (bag.properties.len() as u32).to_le_bytes().to_vec();
        for property in &bag.properties {
            put(&mut out, property.key.as_bytes());
            put(&mut out, property.kind.as_bytes());
            out.extend_from_slice(&property.flags.to_le_bytes());
            put(&mut out, &property.value);
        }
        Some(out)
    }

    fn load_state(&self, mut data: &[u8]) -> bool {
        let read = |data: &mut &[u8]| -> Option<Vec<Property>> {
            let count = u32::from_le_bytes(data.get(..4)?.try_into().ok()?) as usize;
            *data = &data[4..];
            let mut properties = Vec::new();
            for _ in 0..count.min(4096) {
                let key = String::from_utf8(take(data)?.to_vec()).ok()?;
                let kind = String::from_utf8(take(data)?.to_vec()).ok()?;
                let flags = u32::from_le_bytes(data.get(..4)?.try_into().ok()?);
                *data = &data[4..];
                properties.push(Property { key, kind, flags, value: take(data)?.to_vec() });
            }
            (properties.len() == count).then_some(properties)
        };
        let Some(properties) = read(&mut data) else { return false };
        let mut bag = StateBag { urids: &self.urids, properties };
        let status = self.with_state(|interface, handle, features| {
            let restore = interface.restore?;
            // SAFETY: as for `save_state`.
            Some(unsafe { restore(handle, state_retrieve, (&raw mut bag).cast(), 3, features) })
        });
        status == Some(Some(0))
    }
}

impl Lv2Shared {
    fn value(&self, port: u32) -> f32 {
        self.controls.get(port as usize).map_or(0.0, |v| f32::from_bits(v.load(Ordering::Relaxed)))
    }

    fn set(&self, port: u32, value: f32) {
        if let Some(slot) = self.controls.get(port as usize) {
            slot.store(value.to_bits(), Ordering::Relaxed);
        }
    }
}

/// A running LV2 plug-in.
pub struct Lv2Instance {
    shared: Arc<Lv2Shared>,
    descriptor: *const Descriptor,
    handle: *mut c_void,
    ports: Vec<Port>,
    /// One value per control port; plug-ins hold pointers into it, so it never grows.
    controls: Vec<f32>,
    /// Control port index of each parameter, by parameter id.
    param_ports: HashMap<u32, usize>,
    instrument: bool,
    midi: Vec<[u8; 3]>,
    sequence_type: u32,
    chunk_type: u32,
    midi_type: u32,
    // Kept alive for the plug-in: it may hold on to all of these.
    _urids: Arc<Urids>,
    _map: Box<UridMap>,
    _unmap: Box<UridUnmap>,
    // Each feature boxed on its own: the plug-in keeps pointers to them, which
    // must not move when the list grows.
    #[allow(clippy::vec_box)]
    _features: Vec<Box<Feature>>,
    _feature_list: Vec<*const Feature>,
    _strings: Vec<CString>,
    _options: Box<[u32; 8]>,
    _library: libloading::Library,
}

// SAFETY: the instance is only ever used by one thread at a time (it lives in
// the engine, behind its mutex).
unsafe impl Send for Lv2Instance {}

impl Lv2Instance {
    /// Loads and starts the plug-in `uri` of the folder `bundle`. This runs
    /// code from the folder. Also returns the plug-in's parameters.
    pub fn new(bundle: &Path, uri: &str, sample_rate: f64) -> Result<(Self, Vec<ParamInfo>), String> {
        let (graph, plugins) = describe_bundle(bundle)?;
        if !plugins.iter().any(|p| p == uri) {
            return Err(tr("the plug-in is not in this folder").to_owned());
        }
        let lv2 = |name: &str| format!("{LV2}{name}");
        if let Some(missing) = graph.nodes(uri, &lv2("requiredFeature")).into_iter().find(|f| !SUPPORTED.contains(f)) {
            return Err(tf!("this plug-in requires a feature the host does not offer: {missing}" [missing]));
        }
        let binary = graph.nodes(uri, &lv2("binary")).first().copied().ok_or(tr("the description does not name the plug-in's program"))?;
        if binary.contains("://") || binary.contains("..") {
            return Err(tr("the plug-in's program must be inside its folder").to_owned());
        }
        let binary: PathBuf = bundle.join(binary);

        // Ports, in the order of their index.
        let mut described: Vec<(usize, &str)> = graph.nodes(uri, &lv2("port")).into_iter().filter_map(|port| Some((graph.number(port, &lv2("index"))? as usize, port))).collect();
        described.sort();
        if described.is_empty() || described.len() > 512 || described.iter().enumerate().any(|(i, (index, _))| i != *index) {
            return Err(tr("the description of the plug-in's ports is inconsistent").to_owned());
        }
        let mut ports = Vec::new();
        let mut controls = Vec::new();
        let mut params = Vec::new();
        let mut param_ports = HashMap::new();
        for (index, port) in &described {
            let input = graph.is_a(port, &lv2("InputPort"));
            let output = graph.is_a(port, &lv2("OutputPort"));
            if input == output {
                return Err(tr("a port of the plug-in is neither an input nor an output").to_owned());
            }
            if graph.is_a(port, &lv2("AudioPort")) {
                ports.push(if input { Port::AudioIn(vec![0.0; MAX_FRAMES]) } else { Port::AudioOut(vec![0.0; MAX_FRAMES]) });
            } else if graph.is_a(port, &lv2("ControlPort")) {
                let (min, max) = (graph.number(port, &lv2("minimum")).unwrap_or(0.0), graph.number(port, &lv2("maximum")).unwrap_or(1.0));
                let default = graph.number(port, &lv2("default")).unwrap_or(min);
                if input && max > min {
                    param_ports.insert(*index as u32, controls.len());
                    params.push(ParamInfo {
                        id: *index as u32,
                        name: graph.literal(port, &lv2("name")).or(graph.literal(port, &lv2("symbol"))).unwrap_or("?").to_owned(),
                        min: f64::from(min),
                        max: f64::from(max),
                        default: f64::from(default.clamp(min, max)),
                    });
                }
                ports.push(Port::Control(controls.len()));
                controls.push(default);
            } else if graph.is_a(port, &format!("{ATOM}AtomPort")) {
                let midi = graph.nodes(port, &format!("{ATOM}supports")).contains(&MIDI_EVENT);
                let buffer = vec![0u64; ATOM_BYTES / 8];
                ports.push(if input { Port::EventsIn { buffer, midi } } else { Port::EventsOut { buffer } });
            } else {
                return Err(tr("the plug-in has a port of a type the host does not know").to_owned());
            }
        }
        if !ports.iter().any(|p| matches!(p, Port::AudioOut(_))) {
            return Err(tr("this plug-in has no audio output").to_owned());
        }
        let has_audio_in = ports.iter().any(|p| matches!(p, Port::AudioIn(_)));
        let instrument = graph.is_a(uri, &lv2("InstrumentPlugin")) || !has_audio_in;

        let c_uri = CString::new(uri).map_err(|_| tr("invalid plug-in address").to_owned())?;
        let mut bundle_path = bundle.to_string_lossy().into_owned();
        if !bundle_path.ends_with(['/', '\\']) {
            bundle_path.push(std::path::MAIN_SEPARATOR);
        }
        let c_bundle = CString::new(bundle_path).map_err(|_| tr("invalid path").to_owned())?;

        // What the host offers the plug-in.
        let urids: Arc<Urids> = Arc::new(Mutex::new(Vec::new()));
        let urids_ptr = Arc::as_ptr(&urids).cast_mut().cast::<c_void>();
        let mut map = Box::new(UridMap { handle: urids_ptr, map: urid_map });
        let mut unmap = Box::new(UridUnmap { handle: urids_ptr, unmap: urid_unmap });
        // An empty list of options: one all-zero entry marks its end.
        let mut options = Box::new([0u32; 8]);
        let strings: Vec<CString> = [URID_MAP, URID_UNMAP, OPTIONS, BOUNDED_BLOCK].iter().filter_map(|s| CString::new(*s).ok()).collect();
        let data: [*mut c_void; 4] = [(&raw mut *map).cast(), (&raw mut *unmap).cast(), options.as_mut_ptr().cast(), null_mut()];
        let features: Vec<Box<Feature>> = strings.iter().zip(data).map(|(uri, data)| Box::new(Feature { uri: uri.as_ptr(), data })).collect();
        let mut feature_list: Vec<*const Feature> = features.iter().map(|f| &raw const **f).collect();
        feature_list.push(null());

        // SAFETY: loading and calling a plug-in is inherently trusting it; every
        // pointer it returns is checked, and everything it is given outlives it.
        unsafe {
            let library = libloading::Library::new(&binary).map_err(|e| tf!("unreadable plug-in program ({e})" [e]))?;
            let entry = *library
                .get::<unsafe extern "C" fn(u32) -> *const Descriptor>(b"lv2_descriptor\0")
                .map_err(|_| tr("this program is not an LV2 plug-in").to_owned())?;
            let mut descriptor = null();
            for index in 0..1024 {
                let candidate = entry(index);
                let Some(d) = candidate.as_ref() else { break };
                if !d.uri.is_null() && CStr::from_ptr(d.uri) == c_uri.as_c_str() {
                    descriptor = candidate;
                    break;
                }
            }
            let d = descriptor.as_ref().ok_or(tr("the program does not contain this plug-in"))?;
            let (Some(instantiate), Some(connect), Some(_)) = (d.instantiate, d.connect_port, d.run) else {
                return Err(tr("invalid LV2 plug-in").to_owned());
            };
            let handle = instantiate(descriptor, sample_rate, c_bundle.as_ptr(), feature_list.as_ptr());
            if handle.is_null() {
                return Err(tr("the plug-in refused to start").to_owned());
            }
            let urid = |uri: &str| CString::new(uri).map_or(0, |c| urid_map(urids_ptr, c.as_ptr()));
            let control_ports: Vec<(u32, bool)> = ports
                .iter()
                .enumerate()
                .filter(|(_, p)| matches!(p, Port::Control(_)))
                .map(|(index, _)| (index as u32, param_ports.contains_key(&(index as u32))))
                .collect();
            let shared = Arc::new(Lv2Shared {
                controls: ports.iter().map(|p| AtomicU32::new(if let Port::Control(slot) = p { controls[*slot].to_bits() } else { 0 })).collect(),
                control_ports,
                handle,
                extension_data: d.extension_data,
                state: d
                    .extension_data
                    .and_then(|data| CString::new(STATE_INTERFACE).ok().map(|name| data(name.as_ptr())))
                    .map_or(null(), |interface| interface.cast::<StateInterface>()),
                urids: Arc::clone(&urids),
                gate: Mutex::new(true),
            });
            let mut instance = Self {
                shared,
                descriptor,
                handle,
                ports,
                controls,
                param_ports,
                instrument,
                midi: Vec::with_capacity(256),
                sequence_type: urid(&format!("{ATOM}Sequence")),
                chunk_type: urid(&format!("{ATOM}Chunk")),
                midi_type: urid(MIDI_EVENT),
                _urids: urids,
                _map: map,
                _unmap: unmap,
                _features: features,
                _feature_list: feature_list,
                _strings: strings,
                _options: options,
                _library: library,
            };
            // Buffers never move after this point, so ports are connected once.
            let controls = instance.controls.as_mut_ptr();
            for (index, port) in instance.ports.iter_mut().enumerate() {
                let data: *mut c_void = match port {
                    Port::AudioIn(buffer) | Port::AudioOut(buffer) => buffer.as_mut_ptr().cast(),
                    Port::Control(slot) => controls.add(*slot).cast(),
                    Port::EventsIn { buffer, .. } | Port::EventsOut { buffer } => buffer.as_mut_ptr().cast(),
                };
                connect(handle, index as u32, data);
            }
            if let Some(activate) = d.activate {
                activate(handle);
            }
            Ok((instance, params))
        }
    }

    /// What the plug-in's window needs to follow it.
    pub fn shared(&self) -> Arc<Lv2Shared> {
        Arc::clone(&self.shared)
    }
}

impl ExternalUnit for Lv2Instance {
    fn process(&mut self, buf: &mut [f32]) {
        // Not while the plug-in's state is being saved or restored.
        let shared = Arc::clone(&self.shared);
        let Ok(_gate) = shared.gate.lock() else { return };
        // SAFETY: the plug-in was started by `new`, its ports point at buffers
        // of MAX_FRAMES samples owned by `self`, and blocks never exceed that.
        unsafe {
            let Some(run) = (*self.descriptor).run else { return };
            for chunk in buf.chunks_mut(MAX_FRAMES) {
                let frames = chunk.len();
                for port in &mut self.ports {
                    match port {
                        Port::AudioIn(buffer) => buffer[..frames].copy_from_slice(chunk),
                        Port::AudioOut(buffer) => buffer[..frames].fill(0.0),
                        Port::Control(_) => {}
                        Port::EventsIn { buffer, midi } => {
                            // A sequence: (size, type) then (unit, pad), then events of
                            // (frame, size, type, three bytes padded to eight).
                            let words = buffer.as_mut_ptr().cast::<u32>();
                            let events = if *midi { self.midi.len().min((ATOM_BYTES - 16) / 24) } else { 0 };
                            *words = 8 + 24 * events as u32;
                            *words.add(1) = self.sequence_type;
                            *words.add(2) = 0;
                            *words.add(3) = 0;
                            for (i, message) in self.midi.iter().take(events).enumerate() {
                                let event = words.add(4 + 6 * i);
                                *event.cast::<i64>() = 0;
                                *event.add(2) = 3;
                                *event.add(3) = self.midi_type;
                                *event.add(4) = u32::from_le_bytes([message[0], message[1], message[2], 0]);
                                *event.add(5) = 0;
                            }
                        }
                        Port::EventsOut { buffer } => {
                            // Says how much room there is for what the plug-in writes.
                            let words = buffer.as_mut_ptr().cast::<u32>();
                            *words = (ATOM_BYTES - 8) as u32;
                            *words.add(1) = self.chunk_type;
                        }
                    }
                }
                self.midi.clear();
                // What the plug-in's window changed since the last block.
                for (index, port) in self.ports.iter().enumerate() {
                    if let Port::Control(slot) = port {
                        if self.param_ports.contains_key(&(index as u32)) {
                            self.controls[*slot] = self.shared.value(index as u32);
                        }
                    }
                }
                run(self.handle, frames as u32);
                // What the plug-in reports (meters and the like), for its window.
                for (index, port) in self.ports.iter().enumerate() {
                    if let Port::Control(slot) = port {
                        if !self.param_ports.contains_key(&(index as u32)) {
                            self.shared.set(index as u32, self.controls[*slot]);
                        }
                    }
                }

                let mut outputs = self.ports.iter().filter_map(|p| match p {
                    Port::AudioOut(buffer) => Some(buffer),
                    _ => None,
                });
                let (Some(left), right) = (outputs.next(), outputs.next()) else { return };
                for (i, sample) in chunk.iter_mut().enumerate() {
                    let mut out = right.map_or(left[i], |r| 0.5 * (left[i] + r[i]));
                    if !out.is_finite() {
                        out = 0.0;
                    }
                    *sample = if self.instrument { *sample + out } else { out };
                }
            }
        }
    }

    fn note_on(&mut self, pitch: u8, velocity: u8) {
        if self.midi.len() < self.midi.capacity() {
            self.midi.push([0x90, pitch.min(127), velocity.clamp(1, 127)]);
        }
    }

    fn note_off(&mut self, pitch: u8) {
        if self.midi.len() < self.midi.capacity() {
            self.midi.push([0x80, pitch.min(127), 0]);
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if let Some(control) = self.param_ports.get(&id).and_then(|slot| self.controls.get_mut(*slot)) {
            if value.is_finite() {
                *control = value as f32;
                self.shared.set(id, value as f32);
            }
        }
    }

    fn all_notes_off(&mut self) {
        // "All notes off" on the first channel, in place of 128 note-offs.
        self.midi.clear();
        self.midi.push([0xB0, 123, 0]);
    }
}

// ----- the plug-in's own window ---------------------------------------------------------

#[repr(C)]
struct UiDescriptor {
    uri: *const c_char,
    #[allow(clippy::type_complexity)]
    instantiate: Option<
        unsafe extern "C" fn(*const UiDescriptor, *const c_char, *const c_char, WriteFn, *mut c_void, *mut *mut c_void, *const *const Feature) -> *mut c_void,
    >,
    cleanup: Option<unsafe extern "C" fn(*mut c_void)>,
    port_event: Option<unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void)>,
    extension_data: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}

type WriteFn = unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void);

#[repr(C)]
struct UiResize {
    handle: *mut c_void,
    resize: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
}

#[repr(C)]
struct UiIdle {
    idle: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
}

#[repr(C)]
struct DataAccess {
    data_access: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}

/// What the window's code tells the host, from the interface thread.
#[derive(Default)]
struct Controller {
    /// Control ports the user changed in the window: port and value.
    writes: Mutex<Vec<(u32, f32)>>,
    /// Size the window asks for.
    size: Mutex<Option<(u32, u32)>>,
}

unsafe extern "C" fn ui_write(controller: *mut c_void, port: u32, size: u32, protocol: u32, buffer: *const c_void) {
    // Protocol 0 is "one float for a control port", the only one offered.
    let (Some(controller), false, 0, 4) = (controller.cast::<Controller>().as_ref(), buffer.is_null(), protocol, size) else { return };
    let value = buffer.cast::<f32>().read_unaligned();
    if let (true, Ok(mut writes)) = (value.is_finite(), controller.writes.lock()) {
        if writes.len() < 4096 {
            writes.push((port, value));
        }
    }
}

unsafe extern "C" fn ui_resize(handle: *mut c_void, width: i32, height: i32) -> i32 {
    let Some(controller) = handle.cast::<Controller>().as_ref() else { return 1 };
    if let (true, Ok(mut size)) = (width > 0 && height > 0, controller.size.lock()) {
        *size = Some((width.min(8192) as u32, height.min(8192) as u32));
    }
    0
}

/// A window that is open: the plug-in's window code, running inside a window of ours.
struct OpenUi {
    descriptor: *const UiDescriptor,
    handle: *mut c_void,
    idle: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
    /// What the window was last told of each control port.
    sent: Vec<(u32, f32)>,
    window: HostWindow,
    controller: Box<Controller>,
    // Kept alive for the window's code: it may hold on to all of these.
    _urids: Box<Urids>,
    _map: Box<UridMap>,
    _unmap: Box<UridUnmap>,
    _resize: Box<UiResize>,
    _access: Box<DataAccess>,
    _options: Box<[u32; 8]>,
    // Boxed one by one: the plug-in keeps pointers to them.
    #[allow(clippy::vec_box)]
    _features: Vec<Box<Feature>>,
    _feature_list: Vec<*const Feature>,
    _strings: Vec<CString>,
    _library: libloading::Library,
}

impl Drop for OpenUi {
    fn drop(&mut self) {
        // SAFETY: the window's code is told to go before its window and its
        // program do (fields drop in the order they are declared).
        unsafe {
            if let Some(cleanup) = (*self.descriptor).cleanup {
                cleanup(self.handle);
            }
        }
    }
}

/// The window of an LV2 plug-in. Until it is asked for, the plug-in is set
/// with the host's sliders and nothing of the window's code is loaded.
pub struct Lv2Ui {
    shared: Arc<Lv2Shared>,
    host: Arc<HostShared>,
    plugin_uri: CString,
    ui_uri: CString,
    bundle: CString,
    binary: PathBuf,
    open: Option<OpenUi>,
}

impl Lv2Ui {
    /// The window the folder describes for a plug-in, if it has one that can be
    /// shown here: on Windows, a window that goes inside one of ours.
    pub fn find(bundle: &Path, uri: &str, shared: Arc<Lv2Shared>) -> Option<Self> {
        if !cfg!(windows) {
            return None;
        }
        let (graph, _) = describe_bundle(bundle).ok()?;
        let offered = [format!("{UI}parent"), format!("{UI}resize"), format!("{UI}idleInterface"), INSTANCE_ACCESS.to_owned(), DATA_ACCESS.to_owned()];
        let usable = |window: &str| {
            graph.is_a(window, &format!("{UI}WindowsUI"))
                && graph.nodes(window, &format!("{LV2}requiredFeature")).iter().all(|f| SUPPORTED.contains(f) || offered.iter().any(|o| o == f))
        };
        let windows = graph.nodes(uri, &format!("{UI}ui"));
        let window = windows.iter().copied().find(|window| usable(window))?;
        let binary = graph.nodes(window, &format!("{UI}binary")).first().or(graph.nodes(window, &format!("{LV2}binary")).first()).copied()?;
        if binary.contains("://") || binary.contains("..") {
            return None;
        }
        let mut bundle_path = bundle.to_string_lossy().into_owned();
        if !bundle_path.ends_with(['/', '\\']) {
            bundle_path.push(std::path::MAIN_SEPARATOR);
        }
        Some(Self {
            shared,
            host: Arc::new(HostShared::default()),
            plugin_uri: CString::new(uri).ok()?,
            ui_uri: CString::new(window).ok()?,
            bundle: CString::new(bundle_path).ok()?,
            binary: bundle.join(binary),
            open: None,
        })
    }

    fn start(&mut self, title: &str) -> Result<OpenUi, String> {
        let window = HostWindow::new(title, 480, 320, false, &self.host)?;
        let controller = Box::new(Controller::default());
        let controller_ptr = (&raw const *controller).cast_mut().cast::<c_void>();
        let urids: Box<Urids> = Box::new(Mutex::new(Vec::new()));
        let urids_ptr = (&raw const *urids).cast_mut().cast::<c_void>();
        let mut map = Box::new(UridMap { handle: urids_ptr, map: urid_map });
        let mut unmap = Box::new(UridUnmap { handle: urids_ptr, unmap: urid_unmap });
        let mut resize = Box::new(UiResize { handle: controller_ptr, resize: ui_resize });
        let mut access = Box::new(DataAccess { data_access: self.shared.extension_data });
        let mut options = Box::new([0u32; 8]);
        let offered: [(String, *mut c_void); 8] = [
            (URID_MAP.to_owned(), (&raw mut *map).cast()),
            (URID_UNMAP.to_owned(), (&raw mut *unmap).cast()),
            (OPTIONS.to_owned(), options.as_mut_ptr().cast()),
            (format!("{UI}parent"), window.handle()),
            (format!("{UI}resize"), (&raw mut *resize).cast()),
            (format!("{UI}idleInterface"), null_mut()),
            (INSTANCE_ACCESS.to_owned(), self.shared.handle),
            (DATA_ACCESS.to_owned(), (&raw mut *access).cast()),
        ];
        let strings: Vec<CString> = offered.iter().filter_map(|(uri, _)| CString::new(uri.as_str()).ok()).collect();
        let features: Vec<Box<Feature>> = strings.iter().zip(&offered).map(|(uri, (_, data))| Box::new(Feature { uri: uri.as_ptr(), data: *data })).collect();
        let mut feature_list: Vec<*const Feature> = features.iter().map(|f| &raw const **f).collect();
        feature_list.push(null());

        // SAFETY: as for the plug-in itself: its window's code is trusted, what
        // it returns is checked, and what it is given outlives it.
        unsafe {
            let library = libloading::Library::new(&self.binary).map_err(|e| tf!("unreadable plug-in program ({e})" [e]))?;
            let entry = *library
                .get::<unsafe extern "C" fn(u32) -> *const UiDescriptor>(b"lv2ui_descriptor\0")
                .map_err(|_| tr("this plug-in has no window that can be used here").to_owned())?;
            let mut descriptor = null();
            for index in 0..1024 {
                let candidate = entry(index);
                let Some(d) = candidate.as_ref() else { break };
                if !d.uri.is_null() && CStr::from_ptr(d.uri) == self.ui_uri.as_c_str() {
                    descriptor = candidate;
                    break;
                }
            }
            let d = descriptor.as_ref().ok_or(tr("this plug-in has no window that can be used here"))?;
            let instantiate = d.instantiate.ok_or(tr("this plug-in has no window that can be used here"))?;
            let mut widget: *mut c_void = null_mut();
            let handle = instantiate(descriptor, self.plugin_uri.as_ptr(), self.bundle.as_ptr(), ui_write, controller_ptr, &mut widget, feature_list.as_ptr());
            if handle.is_null() {
                return Err(tr("the plug-in could not create its window").to_owned());
            }
            let idle = d
                .extension_data
                .and_then(|data| CString::new(format!("{UI}idleInterface")).ok().map(|name| data(name.as_ptr())))
                .and_then(|interface| interface.cast::<UiIdle>().as_ref())
                .and_then(|interface| interface.idle);
            let mut open = OpenUi {
                descriptor,
                handle,
                idle,
                sent: Vec::new(),
                window,
                controller,
                _urids: urids,
                _map: map,
                _unmap: unmap,
                _resize: resize,
                _access: access,
                _options: options,
                _features: features,
                _feature_list: feature_list,
                _strings: strings,
                _library: library,
            };
            match open.controller.size.lock().ok().and_then(|mut s| s.take()) {
                Some((width, height)) => open.window.resize(width, height),
                None => open.window.fit_to_child(),
            }
            // The window starts from the values the plug-in has now.
            for (port, _) in &self.shared.control_ports {
                let value = self.shared.value(*port);
                if let Some(port_event) = d.port_event {
                    port_event(handle, *port, 4, 0, (&raw const value).cast());
                }
                open.sent.push((*port, value));
            }
            open.window.show();
            Ok(open)
        }
    }
}

impl crate::plugin::PluginHandle for Lv2Ui {
    fn has_window(&self) -> bool {
        true
    }

    fn window_open(&self) -> bool {
        self.open.is_some()
    }

    fn open_window(&mut self, title: &str) -> Result<(), String> {
        if self.open.is_none() {
            self.open = Some(self.start(title)?);
        }
        Ok(())
    }

    fn close_window(&mut self) {
        self.open = None;
    }

    fn idle(&mut self) -> crate::plugin::Idle {
        let mut result = crate::plugin::Idle::default();
        let Some(open) = &mut self.open else { return result };
        // SAFETY: interface-thread calls on a window that `start` made.
        let wants_to_close = unsafe { open.idle.is_some_and(|idle| idle(open.handle) != 0) };
        if let Some((width, height)) = open.controller.size.lock().ok().and_then(|mut s| s.take()) {
            open.window.resize(width, height);
        }
        // What the user turned in the window: only the ports the plug-in reads.
        let writes = open.controller.writes.lock().map(|mut w| std::mem::take(&mut *w)).unwrap_or_default();
        for (port, value) in writes {
            if self.shared.control_ports.contains(&(port, true)) {
                self.shared.set(port, value);
                if let Some(sent) = open.sent.iter_mut().find(|s| s.0 == port) {
                    sent.1 = value;
                }
                result.changes.push((port, f64::from(value)));
            }
        }
        // What changed elsewhere: the host's sliders, and what the plug-in reports.
        for sent in &mut open.sent {
            let value = self.shared.value(sent.0);
            if value.to_bits() != sent.1.to_bits() {
                sent.1 = value;
                // SAFETY: as above.
                unsafe {
                    if let Some(port_event) = (*open.descriptor).port_event {
                        port_event(open.handle, sent.0, 4, 0, (&raw const value).cast());
                    }
                }
            }
        }
        if (self.host.closed.swap(false, Ordering::AcqRel) || wants_to_close) && self.open.is_some() {
            self.open = None;
            result.closed = true;
        }
        result
    }

    fn param_value(&self, id: u32) -> Option<f64> {
        self.shared.control_ports.contains(&(id, true)).then(|| f64::from(self.shared.value(id)))
    }

    fn param_changed(&mut self, id: u32, value: f64) {
        // The audio side does the same when it gets the value; doing it here
        // too lets the window follow even while no sound is being made.
        if value.is_finite() && self.shared.control_ports.contains(&(id, true)) {
            self.shared.set(id, value as f32);
        }
    }

    #[cfg(test)]
    fn window_handle(&self) -> Option<*mut c_void> {
        self.open.as_ref().map(|open| open.window.handle())
    }
}

/// The main-thread face of a running LV2 plug-in: its state, and its window
/// when it describes one that can be shown here.
pub struct Lv2Handle {
    shared: Arc<Lv2Shared>,
    window: Option<Lv2Ui>,
}

impl Lv2Handle {
    pub fn new(bundle: &Path, uri: &str, shared: Arc<Lv2Shared>) -> Self {
        Self { window: Lv2Ui::find(bundle, uri, Arc::clone(&shared)), shared }
    }

}

impl crate::plugin::PluginHandle for Lv2Handle {
    fn has_window(&self) -> bool {
        self.window.is_some()
    }

    fn window_open(&self) -> bool {
        self.window.as_ref().is_some_and(crate::plugin::PluginHandle::window_open)
    }

    fn open_window(&mut self, title: &str) -> Result<(), String> {
        match &mut self.window {
            Some(window) => window.open_window(title),
            None => Err(tr("this plug-in has no window that can be used here").to_owned()),
        }
    }

    fn close_window(&mut self) {
        if let Some(window) = &mut self.window {
            window.close_window();
        }
    }

    fn idle(&mut self) -> crate::plugin::Idle {
        self.window.as_mut().map(crate::plugin::PluginHandle::idle).unwrap_or_default()
    }

    fn param_value(&self, id: u32) -> Option<f64> {
        self.shared.control_ports.contains(&(id, true)).then(|| f64::from(self.shared.value(id)))
    }

    fn param_changed(&mut self, id: u32, value: f64) {
        // The audio side does the same when it gets the value; doing it here
        // too lets a window follow even while no sound is being made.
        if value.is_finite() && self.shared.control_ports.contains(&(id, true)) {
            self.shared.set(id, value as f32);
        }
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        self.shared.save_state()
    }

    fn load_state(&self, data: &[u8]) -> bool {
        self.shared.load_state(data)
    }

    #[cfg(test)]
    fn window_handle(&self) -> Option<*mut c_void> {
        self.window.as_ref().and_then(crate::plugin::PluginHandle::window_handle)
    }
}

impl Drop for Lv2Instance {
    fn drop(&mut self) {
        // From here on nothing may reach the plug-in through what it shared.
        if let Ok(mut alive) = self.shared.gate.lock() {
            *alive = false;
        }
        // SAFETY: the reverse of `new`, on a plug-in nobody else uses any more.
        unsafe {
            let d = &*self.descriptor;
            if let Some(deactivate) = d.deactivate {
                deactivate(self.handle);
            }
            if let Some(cleanup) = d.cleanup {
                cleanup(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<(String, String, Object)> {
        let mut turtle = Turtle { text: text.as_bytes(), pos: 0, prefixes: HashMap::new(), triples: Vec::new(), blanks: 0, file: 1 };
        assert!(turtle.statements().is_some(), "syntax accepted");
        turtle.triples
    }

    #[test]
    fn turtle_as_plugins_write_it() {
        let triples = parse(
            r#"
            @prefix lv2:  <http://lv2plug.in/ns/lv2core#> .
            @prefix doap: <http://usefulinc.com/ns/doap#> .
            PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
            # A comment.
            <http://example.org/gain>
                a lv2:Plugin , lv2:AmplifierPlugin ;
                doap:name "Gain \"simple\""@en ;
                lv2:binary <gain.dll> ;
                rdfs:seeAlso <gain.ttl> ;
                lv2:port [
                    a lv2:InputPort , lv2:ControlPort ;
                    lv2:index 2 ;
                    lv2:symbol "gain" ;
                    lv2:default 0.5 ; lv2:minimum -1.0e1 ; lv2:maximum 2 ;
                    lv2:scalePoint [ rdfs:label """long
text""" ; <http://www.w3.org/1999/02/22-rdf-syntax-ns#value> 1 ] ;
                ] , [ a lv2:AudioPort ; lv2:index 0 ] ;
                lv2:optionalFeature ( lv2:isLive lv2:hardRTCapable ) .
            _:x a lv2:Thing .
            "#,
        );
        let has = |s: &str, p: &str, o: Object| triples.contains(&(s.to_owned(), p.to_owned(), o));
        let gain = "http://example.org/gain";
        assert!(has(gain, RDF_TYPE, Object::Node(format!("{LV2}Plugin"))));
        assert!(has(gain, RDF_TYPE, Object::Node(format!("{LV2}AmplifierPlugin"))));
        assert!(has(gain, DOAP_NAME, Object::Literal("Gain \"simple\"".to_owned())));
        assert!(has(gain, &format!("{LV2}binary"), Object::Node("gain.dll".to_owned())));
        assert!(has(gain, SEE_ALSO, Object::Node("gain.ttl".to_owned())));
        let ports: Vec<&Object> = triples.iter().filter(|t| t.0 == gain && t.1 == format!("{LV2}port")).map(|t| &t.2).collect();
        assert_eq!(ports.len(), 2);
        let Object::Node(first) = ports[0] else { panic!() };
        assert!(has(first, &format!("{LV2}index"), Object::Literal("2".to_owned())));
        assert!(has(first, &format!("{LV2}minimum"), Object::Literal("-1.0e1".to_owned())));
        assert!(has(first, &format!("{LV2}maximum"), Object::Literal("2".to_owned())));
        assert!(triples.iter().any(|t| t.2 == Object::Literal("long\ntext".to_owned())));
        assert!(triples.iter().any(|t| t.0 == "_:f1x"));
    }

    #[test]
    fn broken_turtle_is_refused_not_fatal() {
        for text in ["<a> <b>", "<a> x:b <c> .", "<a> <b> \"unterminated", "[[[[[", "<a> <b> ( <c> ", "\u{0}\u{1}"] {
            let mut turtle = Turtle { text: text.as_bytes(), pos: 0, prefixes: HashMap::new(), triples: Vec::new(), blanks: 0, file: 1 };
            assert!(turtle.statements().is_none(), "{text:?}");
        }
        let deep = format!("<a> <b> {} .", "[ <c> ".repeat(100));
        let mut turtle = Turtle { text: deep.as_bytes(), pos: 0, prefixes: HashMap::new(), triples: Vec::new(), blanks: 0, file: 1 };
        assert!(turtle.statements().is_none());
        assert!(describe(Path::new("no-such-folder.lv2")).is_err());
    }
}
