//! Host for CLAP plug-ins (the open plug-in standard, MIT-licensed): loads a
//! `.clap` file, lists its plug-ins, runs one as an effect or an instrument,
//! shows its own window and saves its state. Written against the public CLAP
//! headers.
//!
//! A running plug-in has two faces: `ClapInstance` goes to the audio engine and
//! does the processing; `ClapHandle` stays with the user interface and does
//! what CLAP reserves for the main thread (window, state).

use crate::i18n::tr;
use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_header, clap_event_note, clap_event_param_value, clap_input_events,
    clap_output_events, CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
    CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::ext::audio_ports::{clap_audio_port_info, clap_plugin_audio_ports, CLAP_EXT_AUDIO_PORTS};
use clap_sys::ext::gui::{clap_host_gui, clap_plugin_gui, clap_window, clap_window_handle, CLAP_EXT_GUI, CLAP_WINDOW_API_WIN32};
use clap_sys::ext::params::{
    clap_host_params, clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_HIDDEN,
    CLAP_PARAM_IS_READONLY,
};
use clap_sys::ext::state::{clap_host_state, clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::id::clap_id;
use clap_sys::plugin::clap_plugin;
use clap_sys::process::{clap_process, CLAP_PROCESS_ERROR};
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::version::CLAP_VERSION;
use og_engine::ExternalUnit;

use crate::plugin::{MAX_FRAMES, MAX_STATE};

const MAX_CHANNELS: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub instrument: bool,
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ParamInfo {
    pub id: u32,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub default: f64,
}

/// A loaded plug-in file. It stays loaded for as long as any instance lives.
pub struct ClapLibrary {
    entry: *const clap_plugin_entry,
    factory: *const clap_plugin_factory,
    pub plugins: Vec<PluginInfo>,
    // Dropped last: the pointers above point into it.
    _library: libloading::Library,
}

// SAFETY: the entry and factory are immutable tables of functions that CLAP
// requires to be callable from any thread.
unsafe impl Send for ClapLibrary {}
unsafe impl Sync for ClapLibrary {}

unsafe fn text(pointer: *const c_char) -> String {
    if pointer.is_null() {
        String::new()
    } else {
        CStr::from_ptr(pointer).to_string_lossy().into_owned()
    }
}

impl ClapLibrary {
    /// Loads a `.clap` file. This runs code from the file.
    pub fn load(path: &Path) -> Result<Arc<Self>, String> {
        let c_path = CString::new(path.to_string_lossy().as_bytes()).map_err(|_| tr("invalid path").to_owned())?;
        // SAFETY: loading and calling a plug-in is inherently trusting it; every
        // pointer it returns is checked before use.
        unsafe {
            let library = libloading::Library::new(path).map_err(|e| tf!("unreadable file ({e})" [e]))?;
            let entry: *const clap_plugin_entry = *library
                .get::<*const clap_plugin_entry>(b"clap_entry\0")
                .map_err(|_| tr("this file is not a CLAP plug-in").to_owned())?;
            let entry_ref = entry.as_ref().ok_or(tr("invalid CLAP plug-in"))?;
            if entry_ref.clap_version.major < 1 {
                return Err(tr("unsupported CLAP version").to_owned());
            }
            let (Some(init), Some(get_factory)) = (entry_ref.init, entry_ref.get_factory) else {
                return Err(tr("invalid CLAP plug-in").to_owned());
            };
            if !init(c_path.as_ptr()) {
                return Err(tr("the plug-in refused to start").to_owned());
            }
            let factory = get_factory(CLAP_PLUGIN_FACTORY_ID.as_ptr()).cast::<clap_plugin_factory>();
            let mut plugins = Vec::new();
            if let Some(f) = factory.as_ref() {
                if let (Some(count), Some(describe)) = (f.get_plugin_count, f.get_plugin_descriptor) {
                    for index in 0..count(factory).min(4096) {
                        let Some(desc) = describe(factory, index).as_ref() else { continue };
                        let mut instrument = false;
                        let mut feature = desc.features;
                        while !feature.is_null() && !(*feature).is_null() {
                            instrument |= CStr::from_ptr(*feature).to_bytes() == b"instrument";
                            feature = feature.add(1);
                        }
                        plugins.push(PluginInfo { id: text(desc.id), name: text(desc.name), instrument });
                    }
                }
            }
            if plugins.is_empty() {
                if let Some(deinit) = entry_ref.deinit {
                    deinit();
                }
                return Err(tr("this file holds no plug-in").to_owned());
            }
            Ok(Arc::new(Self { entry, factory, plugins, _library: library }))
        }
    }
}

impl Drop for ClapLibrary {
    fn drop(&mut self) {
        // SAFETY: every instance holds an `Arc` to the library, so none is left.
        unsafe {
            if let Some(deinit) = self.entry.as_ref().and_then(|e| e.deinit) {
                deinit();
            }
        }
    }
}

// ----- what the plug-in may ask of the host --------------------------------------

/// Requests a plug-in makes from any thread, picked up by the interface thread.
#[derive(Default)]
pub struct HostShared {
    /// The plug-in wants `on_main_thread` to be called.
    pub callback: AtomicBool,
    /// The window was closed, by its close box or by the plug-in.
    pub closed: AtomicBool,
    /// Size the plug-in asks its window to take.
    pub wanted_size: Mutex<Option<(u32, u32)>>,
    /// Size the user gave the window.
    pub user_size: Mutex<Option<(u32, u32)>>,
    /// Parameters the plug-in changed itself (its own window): id and value,
    /// the latest value of each, for the host's sliders to follow.
    pub edits: Mutex<Vec<(u32, f64)>>,
    /// The plug-in has parameter changes to hand over while no sound passes.
    pub flush_wanted: AtomicBool,
}

impl HostShared {
    /// Notes a change the plug-in made to one of its parameters.
    fn note_edit(&self, id: u32, value: f64) {
        // Never waits: this may run on the audio thread.
        if let Ok(mut edits) = self.edits.try_lock() {
            if value.is_finite() && edits.len() < 4096 {
                edits.retain(|known| known.0 != id);
                edits.push((id, value));
            }
        }
    }
}

unsafe fn shared<'a>(host: *const clap_host) -> Option<&'a HostShared> {
    host.as_ref()?.host_data.cast::<HostShared>().as_ref()
}

unsafe extern "C" fn host_extension(_: *const clap_host, id: *const c_char) -> *const c_void {
    if id.is_null() {
        return null();
    }
    let id = CStr::from_ptr(id);
    if id == CLAP_EXT_GUI {
        (&raw const HOST_GUI).cast()
    } else if id == CLAP_EXT_STATE {
        (&raw const HOST_STATE).cast()
    } else if id == CLAP_EXT_PARAMS {
        (&raw const HOST_PARAMS).cast()
    } else {
        null()
    }
}

unsafe extern "C" fn host_ignore(_: *const clap_host) {}

unsafe extern "C" fn host_request_callback(host: *const clap_host) {
    if let Some(shared) = shared(host) {
        shared.callback.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn gui_request_resize(host: *const clap_host, width: u32, height: u32) -> bool {
    let Some(shared) = shared(host) else { return false };
    if let Ok(mut wanted) = shared.wanted_size.lock() {
        *wanted = Some((width.clamp(1, 16_384), height.clamp(1, 16_384)));
    }
    true
}

unsafe extern "C" fn gui_refuse(_: *const clap_host) -> bool {
    false
}

unsafe extern "C" fn gui_closed(host: *const clap_host, _was_destroyed: bool) {
    if let Some(shared) = shared(host) {
        shared.closed.store(true, Ordering::Release);
    }
}

unsafe extern "C" fn params_rescan(_: *const clap_host, _: u32) {}
unsafe extern "C" fn params_clear(_: *const clap_host, _: clap_id, _: u32) {}

unsafe extern "C" fn params_request_flush(host: *const clap_host) {
    if let Some(shared) = shared(host) {
        shared.flush_wanted.store(true, Ordering::Release);
    }
}

static HOST_GUI: clap_host_gui = clap_host_gui {
    resize_hints_changed: Some(host_ignore),
    request_resize: Some(gui_request_resize),
    request_show: Some(gui_refuse),
    request_hide: Some(gui_refuse),
    closed: Some(gui_closed),
};
static HOST_STATE: clap_host_state = clap_host_state { mark_dirty: Some(host_ignore) };
static HOST_PARAMS: clap_host_params =
    clap_host_params { rescan: Some(params_rescan), clear: Some(params_clear), request_flush: Some(params_request_flush) };

// ----- the plug-in itself -------------------------------------------------------------

/// What both faces of a running plug-in share. The plug-in is shut down when
/// the last of them goes.
struct Core {
    plugin: *const clap_plugin,
    gui: *const clap_plugin_gui,
    state: *const clap_plugin_state,
    params: *const clap_plugin_params,
    shared: Arc<HostShared>,
    processing: AtomicBool,
    gui_created: AtomicBool,
    /// Kept alive: the plug-in holds a pointer to it.
    _host: Box<clap_host>,
    /// Keeps the plug-in's code loaded for as long as the plug-in lives.
    _library: Arc<ClapLibrary>,
}

// SAFETY: CLAP divides a plug-in's functions between the audio thread and the
// main thread and requires plug-ins to cope with both running at once. The
// instance only calls the audio functions, the handle only the main-thread ones.
unsafe impl Send for Core {}
unsafe impl Sync for Core {}

impl Drop for Core {
    fn drop(&mut self) {
        // SAFETY: nothing else refers to the plug-in any more; this is the
        // reverse of `ClapInstance::new`.
        unsafe {
            let p = &*self.plugin;
            if self.gui_created.load(Ordering::Acquire) {
                if let Some(destroy) = self.gui.as_ref().and_then(|g| g.destroy) {
                    destroy(self.plugin);
                }
            }
            if self.processing.load(Ordering::Acquire) {
                if let Some(stop) = p.stop_processing {
                    stop(self.plugin);
                }
            }
            if let Some(deactivate) = p.deactivate {
                deactivate(self.plugin);
            }
            if let Some(destroy) = p.destroy {
                destroy(self.plugin);
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Event {
    Note(clap_event_note),
    Param(clap_event_param_value),
}

impl Event {
    fn header(&self) -> *const clap_event_header {
        match self {
            Event::Note(e) => &e.header,
            Event::Param(e) => &e.header,
        }
    }
}

unsafe extern "C" fn events_size(list: *const clap_input_events) -> u32 {
    let events: &Vec<Event> = &*(*list).ctx.cast();
    events.len() as u32
}

unsafe extern "C" fn events_get(list: *const clap_input_events, index: u32) -> *const clap_event_header {
    let events: &Vec<Event> = &*(*list).ctx.cast();
    events.get(index as usize).map_or(null(), Event::header)
}

/// What the plug-in sends back: its own parameter changes are kept, the rest
/// is let go. `ctx` is the `HostShared` of the plug-in.
unsafe extern "C" fn events_push(list: *const clap_output_events, header: *const clap_event_header) -> bool {
    let (Some(list), Some(header)) = (list.as_ref(), header.as_ref()) else { return false };
    if header.space_id == CLAP_CORE_EVENT_SPACE_ID && header.type_ == CLAP_EVENT_PARAM_VALUE && header.size as usize >= size_of::<clap_event_param_value>() {
        if let Some(shared) = list.ctx.cast::<HostShared>().as_ref() {
            let event = &*header_ptr_cast::<clap_event_param_value>(header);
            shared.note_edit(event.param_id, event.value);
        }
    }
    true
}

/// The whole event a header begins.
fn header_ptr_cast<T>(header: &clap_event_header) -> *const T {
    std::ptr::from_ref(header).cast()
}

/// The audio face of a running plug-in.
pub struct ClapInstance {
    core: Arc<Core>,
    instrument: bool,
    inputs: usize,
    outputs: usize,
    in_bufs: Vec<Vec<f32>>,
    out_bufs: Vec<Vec<f32>>,
    events: Vec<Event>,
    failed: bool,
    steady: i64,
}

impl ClapInstance {
    /// Starts a plug-in of `library`. Returns its audio face, its main-thread
    /// face, and its editable parameters.
    pub fn new(library: &Arc<ClapLibrary>, plugin_id: &str, sample_rate: f64) -> Result<(Self, ClapHandle, Vec<ParamInfo>), String> {
        let info = library.plugins.iter().find(|p| p.id == plugin_id).ok_or(tr("the plug-in is not in this file"))?;
        let id = CString::new(plugin_id).map_err(|_| tr("invalid plug-in identifier").to_owned())?;
        let shared = Arc::new(HostShared::default());
        let host = Box::new(clap_host {
            clap_version: CLAP_VERSION,
            host_data: Arc::as_ptr(&shared).cast_mut().cast(),
            name: c"OpenGarageMusic".as_ptr(),
            vendor: c"OpenGarageMusic".as_ptr(),
            url: c"".as_ptr(),
            version: c"0.3".as_ptr(),
            get_extension: Some(host_extension),
            request_restart: Some(host_ignore),
            request_process: Some(host_ignore),
            request_callback: Some(host_request_callback),
        });
        // SAFETY: calls follow the order the CLAP standard prescribes (create,
        // init, query extensions, activate); null and failure are checked.
        unsafe {
            let create = library.factory.as_ref().and_then(|f| f.create_plugin).ok_or(tr("invalid CLAP plug-in"))?;
            let plugin = create(library.factory, &*host, id.as_ptr());
            let p = plugin.as_ref().ok_or(tr("the plug-in could not be created"))?;
            let destroy = |why: &str| {
                if let Some(destroy) = p.destroy {
                    destroy(plugin);
                }
                Err(why.to_owned())
            };
            if !p.init.is_some_and(|init| init(plugin)) {
                return destroy(tr("the plug-in refused to initialise"));
            }
            let extension = |id: &CStr| p.get_extension.map_or(null(), |get| get(plugin, id.as_ptr()));

            // Channel counts of the main ports; stereo when the plug-in does not say.
            let (mut inputs, mut outputs) = (if info.instrument { 0 } else { 2 }, 2);
            if let Some(ports) = extension(CLAP_EXT_AUDIO_PORTS).cast::<clap_plugin_audio_ports>().as_ref() {
                if let (Some(count), Some(get)) = (ports.count, ports.get) {
                    let channels = |is_input: bool| {
                        let mut port: clap_audio_port_info = std::mem::zeroed();
                        if count(plugin, is_input) > 0 && get(plugin, 0, is_input, &mut port) {
                            (port.channel_count as usize).min(MAX_CHANNELS)
                        } else {
                            0
                        }
                    };
                    inputs = channels(true);
                    outputs = channels(false);
                }
            }
            if outputs == 0 {
                return destroy(tr("this plug-in has no audio output"));
            }

            let mut params = Vec::new();
            if let Some(ext) = extension(CLAP_EXT_PARAMS).cast::<clap_plugin_params>().as_ref() {
                if let (Some(count), Some(get_info)) = (ext.count, ext.get_info) {
                    for index in 0..count(plugin).min(512) {
                        let mut raw: clap_param_info = std::mem::zeroed();
                        if !get_info(plugin, index, &mut raw) || raw.flags & (CLAP_PARAM_IS_HIDDEN | CLAP_PARAM_IS_READONLY) != 0 {
                            continue;
                        }
                        // Written this way so that a range with NaN bounds is skipped too.
                        #[allow(clippy::neg_cmp_op_on_partial_ord)]
                        let empty = !(raw.max_value > raw.min_value);
                        if empty {
                            continue;
                        }
                        raw.name[raw.name.len() - 1] = 0;
                        params.push(ParamInfo {
                            id: raw.id,
                            name: text(raw.name.as_ptr()),
                            min: raw.min_value,
                            max: raw.max_value,
                            default: raw.default_value.clamp(raw.min_value, raw.max_value),
                        });
                    }
                }
            }
            let gui = extension(CLAP_EXT_GUI).cast::<clap_plugin_gui>();
            let state = extension(CLAP_EXT_STATE).cast::<clap_plugin_state>();

            if !p.activate.is_some_and(|activate| activate(plugin, sample_rate, 1, MAX_FRAMES as u32)) {
                return destroy(tr("the plug-in refused to activate"));
            }
            let core = Arc::new(Core {
                plugin,
                gui,
                state,
                params: extension(CLAP_EXT_PARAMS).cast(),
                shared,
                processing: AtomicBool::new(false),
                gui_created: AtomicBool::new(false),
                _host: host,
                _library: Arc::clone(library),
            });
            let instance = Self {
                core: Arc::clone(&core),
                instrument: info.instrument,
                inputs,
                outputs,
                in_bufs: vec![vec![0.0; MAX_FRAMES]; inputs],
                out_bufs: vec![vec![0.0; MAX_FRAMES]; outputs],
                events: Vec::with_capacity(256),
                failed: false,
                steady: 0,
            };
            Ok((instance, ClapHandle { window: None, core }, params))
        }
    }

    fn push(&mut self, event: Event) {
        // Bounded so that the audio thread never has to allocate.
        if self.events.len() < self.events.capacity() {
            self.events.push(event);
        }
    }

    fn note(&mut self, kind: u16, pitch: u8, velocity: u8) {
        self.push(Event::Note(clap_event_note {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_note>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: kind,
                flags: 0,
            },
            note_id: -1,
            port_index: 0,
            channel: 0,
            key: i16::from(pitch.min(127)),
            velocity: f64::from(velocity.min(127)) / 127.0,
        }));
    }
}

impl ExternalUnit for ClapInstance {
    fn process(&mut self, buf: &mut [f32]) {
        // A plug-in that failed to start lets the sound through untouched.
        if self.failed {
            return;
        }
        let plugin = self.core.plugin;
        // SAFETY: the buffers outlive the call and are as long as `frames_count`
        // says; the plug-in was activated for blocks of up to MAX_FRAMES.
        unsafe {
            let p = &*plugin;
            if !self.core.processing.load(Ordering::Acquire) {
                if !p.start_processing.is_none_or(|start| start(plugin)) {
                    self.failed = true;
                    return;
                }
                self.core.processing.store(true, Ordering::Release);
            }
            let Some(process) = p.process else { return };
            for chunk in buf.chunks_mut(MAX_FRAMES) {
                let frames = chunk.len();
                for input in &mut self.in_bufs {
                    input[..frames].copy_from_slice(chunk);
                }
                let mut in_ptrs: [*mut f32; MAX_CHANNELS] = [null_mut(); MAX_CHANNELS];
                let mut out_ptrs: [*mut f32; MAX_CHANNELS] = [null_mut(); MAX_CHANNELS];
                for (pointer, buffer) in in_ptrs.iter_mut().zip(&mut self.in_bufs) {
                    *pointer = buffer.as_mut_ptr();
                }
                for (pointer, buffer) in out_ptrs.iter_mut().zip(&mut self.out_bufs) {
                    buffer[..frames].fill(0.0);
                    *pointer = buffer.as_mut_ptr();
                }
                let audio_in = clap_audio_buffer { data32: in_ptrs.as_mut_ptr(), data64: null_mut(), channel_count: self.inputs as u32, latency: 0, constant_mask: 0 };
                let mut audio_out = clap_audio_buffer { data32: out_ptrs.as_mut_ptr(), data64: null_mut(), channel_count: self.outputs as u32, latency: 0, constant_mask: 0 };
                let in_events = clap_input_events { ctx: (&raw mut self.events).cast(), size: Some(events_size), get: Some(events_get) };
                let out_events = clap_output_events { ctx: Arc::as_ptr(&self.core.shared).cast_mut().cast(), try_push: Some(events_push) };
                let block = clap_process {
                    steady_time: self.steady,
                    frames_count: frames as u32,
                    transport: null(),
                    audio_inputs: if self.inputs > 0 { &audio_in } else { null() },
                    audio_outputs: &mut audio_out,
                    audio_inputs_count: u32::from(self.inputs > 0),
                    audio_outputs_count: 1,
                    in_events: &in_events,
                    out_events: &out_events,
                };
                let status = process(plugin, &block);
                self.events.clear();
                self.steady += frames as i64;
                if status == CLAP_PROCESS_ERROR {
                    continue;
                }
                // Mix the plug-in's first two channels down to the track's mono path.
                let stereo = self.outputs >= 2;
                for (i, sample) in chunk.iter_mut().enumerate() {
                    let mut out = self.out_bufs[0][i];
                    if stereo {
                        out = 0.5 * (out + self.out_bufs[1][i]);
                    }
                    if !out.is_finite() {
                        out = 0.0;
                    }
                    *sample = if self.instrument { *sample + out } else { out };
                }
            }
        }
    }

    fn note_on(&mut self, pitch: u8, velocity: u8) {
        self.note(CLAP_EVENT_NOTE_ON, pitch, velocity);
    }

    fn note_off(&mut self, pitch: u8) {
        self.note(CLAP_EVENT_NOTE_OFF, pitch, 0);
    }

    fn set_param(&mut self, id: u32, value: f64) {
        self.push(Event::Param(clap_event_param_value {
            header: clap_event_header {
                size: std::mem::size_of::<clap_event_param_value>() as u32,
                time: 0,
                space_id: CLAP_CORE_EVENT_SPACE_ID,
                type_: CLAP_EVENT_PARAM_VALUE,
                flags: 0,
            },
            param_id: id,
            cookie: null_mut(),
            note_id: -1,
            port_index: -1,
            channel: -1,
            key: -1,
            value,
        }));
    }

    fn all_notes_off(&mut self) {
        // Queue space is limited: drop pending notes rather than add 128 more.
        self.events.retain(|e| matches!(e, Event::Param(_)));
        for pitch in 0..128 {
            self.note(CLAP_EVENT_NOTE_OFF, pitch, 0);
        }
    }
}

// ----- the main-thread face: window and state -------------------------------------

unsafe extern "C" fn stream_write(stream: *const clap_ostream, buffer: *const c_void, size: u64) -> i64 {
    let out: &mut Vec<u8> = &mut *(*stream).ctx.cast();
    let size = size as usize;
    if buffer.is_null() || out.len().saturating_add(size) > MAX_STATE {
        return -1;
    }
    out.extend_from_slice(std::slice::from_raw_parts(buffer.cast::<u8>(), size));
    size as i64
}

unsafe extern "C" fn stream_read(stream: *const clap_istream, buffer: *mut c_void, size: u64) -> i64 {
    let (data, position): &mut (&[u8], usize) = &mut *(*stream).ctx.cast();
    let count = (size as usize).min(data.len() - *position);
    if buffer.is_null() {
        return -1;
    }
    std::ptr::copy_nonoverlapping(data[*position..].as_ptr(), buffer.cast::<u8>(), count);
    *position += count;
    count as i64
}

/// The main-thread face of a running plug-in. Use it from the interface thread only.
pub struct ClapHandle {
    // Before `core`, so that the window goes before the plug-in can.
    window: Option<window::HostWindow>,
    core: Arc<Core>,
}

impl ClapHandle {
    /// Whether the plug-in has a window of its own that can be shown here.
    pub fn has_window(&self) -> bool {
        // SAFETY: a main-thread query on a live plug-in.
        unsafe {
            cfg!(windows)
                && self.core.gui.as_ref().is_some_and(|gui| {
                    gui.create.is_some()
                        && gui.set_parent.is_some()
                        && gui.is_api_supported.is_some_and(|supported| supported(self.core.plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false))
                })
        }
    }

    pub fn window_open(&self) -> bool {
        self.window.is_some()
    }

    /// Shows the plug-in's window, in a window of ours that it draws into.
    pub fn open_window(&mut self, title: &str) -> Result<(), String> {
        if self.window.is_some() {
            return Ok(());
        }
        if !self.has_window() {
            return Err(tr("this plug-in has no window that can be used here").to_owned());
        }
        let plugin = self.core.plugin;
        // SAFETY: the calls CLAP prescribes for an embedded window, in order,
        // each checked; `has_window` established that the functions exist.
        unsafe {
            let gui = &*self.core.gui;
            let (Some(create), Some(set_parent)) = (gui.create, gui.set_parent) else { return Ok(()) };
            if !create(plugin, CLAP_WINDOW_API_WIN32.as_ptr(), false) {
                return Err(tr("the plug-in could not create its window").to_owned());
            }
            self.core.gui_created.store(true, Ordering::Release);
            let (mut width, mut height) = (480u32, 320u32);
            if let Some(get_size) = gui.get_size {
                get_size(plugin, &mut width, &mut height);
            }
            let resizable = gui.can_resize.is_some_and(|can| can(plugin));
            let opened = window::HostWindow::new(title, width.clamp(64, 8192), height.clamp(64, 8192), resizable, &self.core.shared).and_then(|host| {
                let target = clap_window { api: CLAP_WINDOW_API_WIN32.as_ptr(), specific: clap_window_handle { win32: host.handle() } };
                if set_parent(plugin, &target) {
                    Ok(host)
                } else {
                    Err(tr("the plug-in refused the window offered").to_owned())
                }
            });
            match opened {
                Ok(host) => {
                    if let Some(show) = gui.show {
                        show(plugin);
                    }
                    host.show();
                    self.window = Some(host);
                    Ok(())
                }
                Err(e) => {
                    self.destroy_gui();
                    Err(e)
                }
            }
        }
    }

    fn destroy_gui(&mut self) {
        if self.core.gui_created.swap(false, Ordering::AcqRel) {
            // SAFETY: the plug-in's window exists and is ours to close.
            unsafe {
                if let Some(gui) = self.core.gui.as_ref() {
                    if let Some(hide) = gui.hide {
                        hide(self.core.plugin);
                    }
                    if let Some(destroy) = gui.destroy {
                        destroy(self.core.plugin);
                    }
                }
            }
        }
    }

    pub fn close_window(&mut self) {
        // The plug-in lets go of our window before the window is destroyed.
        self.destroy_gui();
        self.window = None;
    }

    /// Does what the plug-in asked of the main thread since the last call.
    /// Returns true when its window was just closed.
    pub fn idle(&mut self) -> bool {
        let shared = Arc::clone(&self.core.shared);
        let plugin = self.core.plugin;
        // SAFETY: main-thread calls on a live plug-in.
        unsafe {
            if shared.callback.swap(false, Ordering::AcqRel) {
                if let Some(on_main_thread) = (*plugin).on_main_thread {
                    on_main_thread(plugin);
                }
            }
            // Changes made in the plug-in's window while no sound passes
            // through it come by a flush, asked for by the plug-in.
            if shared.flush_wanted.swap(false, Ordering::AcqRel) && !self.core.processing.load(Ordering::Acquire) {
                if let Some(flush) = self.core.params.as_ref().and_then(|p| p.flush) {
                    let mut none: Vec<Event> = Vec::new();
                    let in_events = clap_input_events { ctx: (&raw mut none).cast(), size: Some(events_size), get: Some(events_get) };
                    let out_events = clap_output_events { ctx: Arc::as_ptr(&shared).cast_mut().cast(), try_push: Some(events_push) };
                    flush(plugin, &in_events, &out_events);
                }
            }
            if let Some((width, height)) = shared.wanted_size.lock().ok().and_then(|mut w| w.take()) {
                if let Some(host) = &self.window {
                    host.resize(width, height);
                }
            }
            if let Some((mut width, mut height)) = shared.user_size.lock().ok().and_then(|mut s| s.take()) {
                if let (true, Some(gui)) = (self.window.is_some(), self.core.gui.as_ref()) {
                    if let Some(adjust) = gui.adjust_size {
                        adjust(plugin, &mut width, &mut height);
                    }
                    if let Some(set_size) = gui.set_size {
                        set_size(plugin, width, height);
                    }
                }
            }
        }
        if shared.closed.swap(false, Ordering::AcqRel) && self.window.is_some() {
            self.close_window();
            return true;
        }
        false
    }

    /// Current value of a parameter, as the plug-in reports it.
    pub fn param_value(&self, id: u32) -> Option<f64> {
        // SAFETY: a main-thread query on a live plug-in.
        unsafe {
            let get_value = self.core.params.as_ref()?.get_value?;
            let mut value = 0.0;
            get_value(self.core.plugin, id, &mut value).then_some(value).filter(|v| v.is_finite())
        }
    }

    /// Everything the plug-in wants remembered, as it writes it.
    pub fn save_state(&self) -> Option<Vec<u8>> {
        // SAFETY: the stream only lives for the call and writes into `out`.
        unsafe {
            let save = self.core.state.as_ref()?.save?;
            let mut out: Vec<u8> = Vec::new();
            let stream = clap_ostream { ctx: (&raw mut out).cast(), write: Some(stream_write) };
            save(self.core.plugin, &stream).then_some(out)
        }
    }

    pub fn load_state(&self, data: &[u8]) -> bool {
        // SAFETY: the stream only lives for the call and reads from `data`.
        unsafe {
            let Some(load) = self.core.state.as_ref().and_then(|s| s.load) else { return false };
            let mut cursor: (&[u8], usize) = (data, 0);
            let stream = clap_istream { ctx: (&raw mut cursor).cast(), read: Some(stream_read) };
            load(self.core.plugin, &stream)
        }
    }

    /// The native window that holds the plug-in's, for tests.
    #[cfg(test)]
    pub fn window_handle(&self) -> Option<*mut c_void> {
        self.window.as_ref().map(window::HostWindow::handle)
    }
}

impl Drop for ClapHandle {
    fn drop(&mut self) {
        self.close_window();
    }
}

impl crate::plugin::PluginHandle for ClapHandle {
    fn has_window(&self) -> bool {
        ClapHandle::has_window(self)
    }
    fn window_open(&self) -> bool {
        ClapHandle::window_open(self)
    }
    fn open_window(&mut self, title: &str) -> Result<(), String> {
        ClapHandle::open_window(self, title)
    }
    fn close_window(&mut self) {
        ClapHandle::close_window(self);
    }
    fn idle(&mut self) -> crate::plugin::Idle {
        let closed = ClapHandle::idle(self);
        let changes = self.core.shared.edits.lock().map(|mut edits| std::mem::take(&mut *edits)).unwrap_or_default();
        crate::plugin::Idle { closed, changes, ..Default::default() }
    }
    fn param_value(&self, id: u32) -> Option<f64> {
        ClapHandle::param_value(self, id)
    }
    fn save_state(&self) -> Option<Vec<u8>> {
        ClapHandle::save_state(self)
    }
    fn load_state(&self, data: &[u8]) -> bool {
        ClapHandle::load_state(self, data)
    }
    #[cfg(test)]
    fn window_handle(&self) -> Option<*mut c_void> {
        ClapHandle::window_handle(self)
    }
}

#[cfg(windows)]
pub mod window {
    //! A plain top-level window for a plug-in to draw into.

    use std::ffi::c_void;
    use std::sync::atomic::Ordering;
    use std::sync::Arc;

    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        AdjustWindowRectEx, CreateWindowExW, DefWindowProcW, DestroyWindow, GetWindow, GetWindowLongPtrW, GetWindowRect, GW_CHILD,
        RegisterClassW, SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, ShowWindow, CW_USEDEFAULT, GWLP_USERDATA,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOZORDER, SW_HIDE, SW_SHOWNORMAL, WINDOW_EX_STYLE,
        WINDOW_STYLE, WM_CLOSE, WM_SIZE, WNDCLASSW, WS_CAPTION, WS_CLIPCHILDREN, WS_MINIMIZEBOX,
        WS_OVERLAPPED, WS_SYSMENU, WS_THICKFRAME,
    };

    use super::HostShared;
    use crate::i18n::tr;

    const CLASS: PCWSTR = w!("OpenGarageMusicPluginWindow");

    unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // The window carries a pointer to the plug-in's shared requests, which
        // outlive the window.
        let shared = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const HostShared).as_ref();
        match (message, shared) {
            (WM_CLOSE, Some(shared)) => {
                // Hidden now, torn down by the interface on its next pass.
                let _ = ShowWindow(hwnd, SW_HIDE);
                shared.closed.store(true, Ordering::Release);
                LRESULT(0)
            }
            (WM_SIZE, Some(shared)) => {
                let (width, height) = ((lparam.0 & 0xFFFF) as u32, ((lparam.0 >> 16) & 0xFFFF) as u32);
                if width > 0 && height > 0 {
                    if let Ok(mut size) = shared.user_size.lock() {
                        *size = Some((width, height));
                    }
                }
                DefWindowProcW(hwnd, message, wparam, lparam)
            }
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }

    pub struct HostWindow {
        hwnd: HWND,
        style: WINDOW_STYLE,
        /// Keeps what the window procedure points to alive.
        _shared: Arc<HostShared>,
    }

    impl HostWindow {
        /// Creates the window (hidden) with a client area of the given size.
        pub fn new(title: &str, width: u32, height: u32, resizable: bool, shared: &Arc<HostShared>) -> Result<Self, String> {
            // SAFETY: ordinary window creation; the class is registered once and
            // registering it again merely fails.
            unsafe {
                let module = GetModuleHandleW(None).map_err(|e| e.message())?;
                let class = WNDCLASSW { lpfnWndProc: Some(procedure), hInstance: module.into(), lpszClassName: CLASS, ..Default::default() };
                RegisterClassW(&class);
                let mut style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_CLIPCHILDREN;
                if resizable {
                    style |= WS_THICKFRAME;
                }
                let mut frame = RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 };
                let _ = AdjustWindowRectEx(&mut frame, style, false, WINDOW_EX_STYLE(0));
                let hwnd = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    CLASS,
                    &HSTRING::from(title),
                    style,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    frame.right - frame.left,
                    frame.bottom - frame.top,
                    None,
                    None,
                    module,
                    None,
                );
                if hwnd.0 == 0 {
                    return Err(tr("the plug-in window could not be created").to_owned());
                }
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, Arc::as_ptr(shared) as isize);
                Ok(Self { hwnd, style, _shared: Arc::clone(shared) })
            }
        }

        pub fn handle(&self) -> *mut c_void {
            self.hwnd.0 as *mut c_void
        }

        pub fn show(&self) {
            // SAFETY: the window is ours and alive.
            unsafe {
                let _ = ShowWindow(self.hwnd, SW_SHOWNORMAL);
                // In front, also when it belongs to a plug-in's own process.
                let _ = SetForegroundWindow(self.hwnd);
            }
        }

        /// Gives the client area the size of the window the plug-in put in it,
        /// for plug-ins that do not say what size they want.
        pub fn fit_to_child(&self) {
            // SAFETY: plain queries on a window of ours and on its child.
            unsafe {
                let child = GetWindow(self.hwnd, GW_CHILD);
                let mut size = RECT::default();
                if child.0 != 0 && GetWindowRect(child, &mut size).is_ok() {
                    let (width, height) = (size.right - size.left, size.bottom - size.top);
                    if width > 16 && height > 16 {
                        self.resize(width.min(8192) as u32, height.min(8192) as u32);
                    }
                }
            }
        }

        /// Gives the client area a new size.
        pub fn resize(&self, width: u32, height: u32) {
            // SAFETY: the window is ours and alive.
            unsafe {
                let mut frame = RECT { left: 0, top: 0, right: width as i32, bottom: height as i32 };
                let _ = AdjustWindowRectEx(&mut frame, self.style, false, WINDOW_EX_STYLE(0));
                let _ = SetWindowPos(self.hwnd, None, 0, 0, frame.right - frame.left, frame.bottom - frame.top, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
            }
        }
    }

    impl Drop for HostWindow {
        fn drop(&mut self) {
            // SAFETY: the window is ours; detaching the pointer first stops the
            // procedure from using it during destruction.
            unsafe {
                SetWindowLongPtrW(self.hwnd, GWLP_USERDATA, 0);
                let _ = DestroyWindow(self.hwnd);
            }
        }
    }
}

#[cfg(not(windows))]
pub mod window {
    //! Plug-in windows are only implemented for Windows so far.

    use std::ffi::c_void;
    use std::sync::Arc;

    use super::HostShared;
    use crate::i18n::tr;

    pub struct HostWindow;

    impl HostWindow {
        pub fn new(_: &str, _: u32, _: u32, _: bool, _: &Arc<HostShared>) -> Result<Self, String> {
            Err(tr("plug-in windows are only available on Windows for now").to_owned())
        }
        pub fn handle(&self) -> *mut c_void {
            std::ptr::null_mut()
        }
        pub fn show(&self) {}
        pub fn resize(&self, _: u32, _: u32) {}
        pub fn fit_to_child(&self) {}
    }
}
