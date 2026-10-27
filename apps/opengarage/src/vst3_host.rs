//! Host for VST3 plug-ins, written against the public VST 3 interfaces as the
//! `vst3` crate binds them.
//!
//! A VST3 plug-in is a program library named `something.vst3`, alone or inside
//! a folder of the same name (`something.vst3/Contents/x86_64-win/`). The host
//! runs effects and instruments, exposes their parameters (from 0 to 1, as the
//! format defines them), shows the plug-in's own window inside one of its own,
//! and saves the plug-in's state.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use og_engine::ExternalUnit;
use vst3::Steinberg::Vst::{
    self as v, IAttributeList, IAttributeListTrait, IAudioProcessor, IAudioProcessorTrait, IComponent, IComponentHandler, IComponentHandlerTrait,
    IComponentTrait, IConnectionPoint, IConnectionPointTrait, IEditController, IEditControllerTrait, IEventList, IEventListTrait, IHostApplication,
    IHostApplicationTrait, IMessage, IMessageTrait, IParamValueQueue, IParamValueQueueTrait, IParameterChanges, IParameterChangesTrait,
};
use vst3::Steinberg::{
    self as s, kInvalidArgument, kNotImplemented, kResultFalse, kResultOk, kResultTrue, tresult, FUnknown, IBStream, IBStreamTrait, IPlugFrame,
    IPlugFrameTrait, IPlugView, IPlugViewTrait, IPluginBaseTrait, IPluginFactory, IPluginFactory2, IPluginFactory2Trait, IPluginFactoryTrait, TUID,
};
use vst3::{Class, ComPtr, ComWrapper, Interface};

use crate::clap_host::window::HostWindow;
use crate::clap_host::{HostShared, ParamInfo, PluginInfo};
use crate::i18n::tr;
use crate::plugin::{MAX_FRAMES, MAX_STATE};

/// Parameter changes and notes passed to a plug-in in one block.
const MAX_CHANGES: usize = 64;

/// Whether a path is a VST3 plug-in: a file or a folder named `something.vst3`.
pub fn is_vst3(path: &Path) -> bool {
    path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vst3"))
}

/// The program library of a VST3 plug-in given as a file or as a folder.
fn binary(path: &Path) -> PathBuf {
    if !path.is_dir() {
        return path.to_path_buf();
    }
    let inside = path.join("Contents").join("x86_64-win");
    let named = inside.join(path.file_name().unwrap_or_default());
    if named.is_file() {
        return named;
    }
    std::fs::read_dir(&inside).ok().and_then(|entries| entries.flatten().map(|e| e.path()).find(|p| p.is_file() && is_vst3(p))).unwrap_or(named)
}

fn text8(chars: &[c_char]) -> String {
    let bytes: Vec<u8> = chars.iter().take_while(|c| **c != 0).map(|c| *c as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

fn text16(chars: &[u16]) -> String {
    let end = chars.iter().position(|c| *c == 0).unwrap_or(chars.len());
    String::from_utf16_lossy(&chars[..end])
}

fn class_id(cid: &TUID) -> String {
    cid.iter().map(|b| format!("{:02x}", *b as u8)).collect()
}

fn iid<I: Interface>() -> s::FIDString {
    I::IID.as_ptr().cast()
}

/// What a loaded VST3 file says it holds: its factory, its plug-ins, and the
/// class of each by its id.
type Described = (ComPtr<IPluginFactory>, Vec<PluginInfo>, Vec<(String, TUID)>);

/// A loaded VST3 file and what it says it holds.
pub struct Vst3Library {
    factory: Option<ComPtr<IPluginFactory>>,
    pub plugins: Vec<PluginInfo>,
    classes: Vec<(String, TUID)>,
    library: libloading::Library,
}

// SAFETY: the factory is only used to create plug-ins, which VST3 allows from
// any thread; the library handle is a plain token.
unsafe impl Send for Vst3Library {}
unsafe impl Sync for Vst3Library {}

impl Vst3Library {
    /// Loads a VST3 file. This runs code from the file.
    pub fn load(path: &Path) -> Result<Arc<Self>, String> {
        let file = binary(path);
        // SAFETY: loading a plug-in is inherently trusting it; what it returns is checked.
        unsafe {
            let library = libloading::Library::new(&file).map_err(|e| tf!("unreadable plug-in program ({e})" [e]))?;
            if let Ok(init) = library.get::<unsafe extern "system" fn() -> bool>(b"InitDll\0") {
                init();
            }
            // A file that holds no plug-in is told to shut down all the same.
            match Self::describe(&library) {
                Ok((factory, plugins, classes)) => Ok(Arc::new(Self { factory: Some(factory), plugins, classes, library })),
                Err(e) => {
                    Self::exit(&library);
                    Err(e)
                }
            }
        }
    }

    /// The factory of a loaded file, and the plug-ins it lists.
    ///
    /// SAFETY: `InitDll` was called on the library.
    unsafe fn describe(library: &libloading::Library) -> Result<Described, String> {
        unsafe {
            let entry = *library
                .get::<unsafe extern "system" fn() -> *mut IPluginFactory>(b"GetPluginFactory\0")
                .map_err(|_| tr("this file is not a VST3 plug-in").to_owned())?;
            let factory = ComPtr::from_raw(entry()).ok_or(tr("this file is not a VST3 plug-in"))?;
            let detailed = factory.cast::<IPluginFactory2>();
            let mut plugins = Vec::new();
            let mut classes = Vec::new();
            for index in 0..factory.countClasses().clamp(0, 4096) {
                let mut info: s::PClassInfo = std::mem::zeroed();
                if factory.getClassInfo(index, &mut info) != kResultOk || text8(&info.category) != "Audio Module Class" {
                    continue;
                }
                let mut more: s::PClassInfo2 = std::mem::zeroed();
                let instrument =
                    detailed.as_ref().is_some_and(|f| f.getClassInfo2(index, &mut more) == kResultOk) && text8(&more.subCategories).contains("Instrument");
                let id = class_id(&info.cid);
                plugins.push(PluginInfo { id: id.clone(), name: text8(&info.name), instrument });
                classes.push((id, info.cid));
            }
            if plugins.is_empty() {
                return Err(tr("this file holds no plug-in").to_owned());
            }
            Ok((factory, plugins, classes))
        }
    }

    /// The counterpart of `InitDll`.
    ///
    /// SAFETY: nothing of the library is in use any more.
    unsafe fn exit(library: &libloading::Library) {
        unsafe {
            if let Ok(exit) = library.get::<unsafe extern "system" fn() -> bool>(b"ExitDll\0") {
                exit();
            }
        }
    }
}

impl Drop for Vst3Library {
    fn drop(&mut self) {
        // The factory goes before the program is told to shut down.
        self.factory = None;
        // SAFETY: a library nobody uses any more.
        unsafe { Self::exit(&self.library) }
    }
}

// ----- what the host gives a plug-in -------------------------------------------------

/// A block of bytes a plug-in reads its state from or writes it to.
struct Stream {
    data: RefCell<Vec<u8>>,
    position: Cell<usize>,
}

impl Class for Stream {
    type Interfaces = (IBStream,);
}

impl Stream {
    fn new(data: Vec<u8>) -> ComWrapper<Self> {
        ComWrapper::new(Self { data: RefCell::new(data), position: Cell::new(0) })
    }
}

impl IBStreamTrait for Stream {
    unsafe fn read(&self, buffer: *mut c_void, bytes: i32, read: *mut i32) -> tresult {
        let data = self.data.borrow();
        let at = self.position.get().min(data.len());
        let count = (bytes.max(0) as usize).min(data.len() - at);
        if buffer.is_null() {
            return kInvalidArgument;
        }
        std::ptr::copy_nonoverlapping(data[at..].as_ptr(), buffer.cast::<u8>(), count);
        self.position.set(at + count);
        if !read.is_null() {
            *read = count as i32;
        }
        kResultOk
    }

    unsafe fn write(&self, buffer: *mut c_void, bytes: i32, written: *mut i32) -> tresult {
        let mut data = self.data.borrow_mut();
        let (at, count) = (self.position.get().min(data.len()), bytes.max(0) as usize);
        if buffer.is_null() || at + count > MAX_STATE {
            return kInvalidArgument;
        }
        if data.len() < at + count {
            data.resize(at + count, 0);
        }
        std::ptr::copy_nonoverlapping(buffer.cast::<u8>(), data[at..].as_mut_ptr(), count);
        self.position.set(at + count);
        if !written.is_null() {
            *written = count as i32;
        }
        kResultOk
    }

    unsafe fn seek(&self, position: i64, mode: i32, result: *mut i64) -> tresult {
        let length = self.data.borrow().len() as i64;
        let base = match mode {
            0 => 0,
            1 => self.position.get() as i64,
            2 => length,
            _ => return kInvalidArgument,
        };
        let target = base.saturating_add(position).clamp(0, length);
        self.position.set(target as usize);
        if !result.is_null() {
            *result = target;
        }
        kResultOk
    }

    unsafe fn tell(&self, position: *mut i64) -> tresult {
        if position.is_null() {
            return kInvalidArgument;
        }
        *position = self.position.get() as i64;
        kResultOk
    }
}

/// What a message carries: values by name.
#[derive(Default)]
struct Attributes {
    ints: RefCell<HashMap<String, i64>>,
    floats: RefCell<HashMap<String, f64>>,
    strings: RefCell<HashMap<String, Vec<u16>>>,
    blobs: RefCell<HashMap<String, Vec<u8>>>,
}

impl Class for Attributes {
    type Interfaces = (IAttributeList,);
}

unsafe fn key(id: *const c_char) -> Option<String> {
    (!id.is_null()).then(|| CStr::from_ptr(id).to_string_lossy().into_owned())
}

impl IAttributeListTrait for Attributes {
    unsafe fn setInt(&self, id: *const c_char, value: i64) -> tresult {
        let Some(id) = key(id) else { return kInvalidArgument };
        self.ints.borrow_mut().insert(id, value);
        kResultOk
    }

    unsafe fn getInt(&self, id: *const c_char, value: *mut i64) -> tresult {
        match key(id).and_then(|id| self.ints.borrow().get(&id).copied()) {
            Some(found) if !value.is_null() => {
                *value = found;
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setFloat(&self, id: *const c_char, value: f64) -> tresult {
        let Some(id) = key(id) else { return kInvalidArgument };
        self.floats.borrow_mut().insert(id, value);
        kResultOk
    }

    unsafe fn getFloat(&self, id: *const c_char, value: *mut f64) -> tresult {
        match key(id).and_then(|id| self.floats.borrow().get(&id).copied()) {
            Some(found) if !value.is_null() => {
                *value = found;
                kResultOk
            }
            _ => kResultFalse,
        }
    }

    unsafe fn setString(&self, id: *const c_char, string: *const u16) -> tresult {
        let (Some(id), false) = (key(id), string.is_null()) else { return kInvalidArgument };
        let mut text = Vec::new();
        while text.len() < 65_536 && *string.add(text.len()) != 0 {
            text.push(*string.add(text.len()));
        }
        self.strings.borrow_mut().insert(id, text);
        kResultOk
    }

    unsafe fn getString(&self, id: *const c_char, string: *mut u16, bytes: u32) -> tresult {
        let strings = self.strings.borrow();
        let (Some(text), false) = (key(id).and_then(|id| strings.get(&id)), string.is_null() || bytes < 2) else { return kResultFalse };
        let room = bytes as usize / 2 - 1;
        let count = text.len().min(room);
        std::ptr::copy_nonoverlapping(text.as_ptr(), string, count);
        *string.add(count) = 0;
        kResultOk
    }

    unsafe fn setBinary(&self, id: *const c_char, data: *const c_void, bytes: u32) -> tresult {
        let (Some(id), false) = (key(id), data.is_null() && bytes > 0) else { return kInvalidArgument };
        if bytes as usize > MAX_STATE {
            return kInvalidArgument;
        }
        let blob = if bytes == 0 { Vec::new() } else { std::slice::from_raw_parts(data.cast::<u8>(), bytes as usize).to_vec() };
        self.blobs.borrow_mut().insert(id, blob);
        kResultOk
    }

    unsafe fn getBinary(&self, id: *const c_char, data: *mut *const c_void, bytes: *mut u32) -> tresult {
        let blobs = self.blobs.borrow();
        let (Some(blob), false) = (key(id).and_then(|id| blobs.get(&id)), data.is_null() || bytes.is_null()) else { return kResultFalse };
        // The bytes stay where they are until the message goes or the value is replaced.
        *data = blob.as_ptr().cast();
        *bytes = blob.len() as u32;
        kResultOk
    }
}

/// A message the two halves of a plug-in send each other.
struct Message {
    id: RefCell<std::ffi::CString>,
    attributes: ComWrapper<Attributes>,
}

impl Class for Message {
    type Interfaces = (IMessage,);
}

impl IMessageTrait for Message {
    unsafe fn getMessageID(&self) -> s::FIDString {
        self.id.borrow().as_ptr()
    }

    unsafe fn setMessageID(&self, id: s::FIDString) {
        if !id.is_null() {
            *self.id.borrow_mut() = CStr::from_ptr(id).to_owned();
        }
    }

    unsafe fn getAttributes(&self) -> *mut IAttributeList {
        self.attributes.as_com_ref::<IAttributeList>().map_or(null_mut(), |list| list.as_ptr())
    }
}

/// The host as plug-ins see it: a name, and a maker of messages.
struct HostApp;

impl Class for HostApp {
    type Interfaces = (IHostApplication,);
}

impl IHostApplicationTrait for HostApp {
    unsafe fn getName(&self, name: *mut v::String128) -> tresult {
        let Some(name) = name.as_mut() else { return kInvalidArgument };
        for (slot, unit) in name.iter_mut().zip("OpenGarageMusic\0".encode_utf16()) {
            *slot = unit;
        }
        kResultOk
    }

    unsafe fn createInstance(&self, cid: *mut TUID, wanted: *mut TUID, obj: *mut *mut c_void) -> tresult {
        if cid.is_null() || wanted.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let made: Option<*mut c_void> = if *cid.cast::<[u8; 16]>() == IMessage::IID && *wanted.cast::<[u8; 16]>() == IMessage::IID {
            let message = Message { id: RefCell::default(), attributes: ComWrapper::new(Attributes::default()) };
            ComWrapper::new(message).to_com_ptr::<IMessage>().map(|p| p.into_raw().cast())
        } else if *cid.cast::<[u8; 16]>() == IAttributeList::IID && *wanted.cast::<[u8; 16]>() == IAttributeList::IID {
            ComWrapper::new(Attributes::default()).to_com_ptr::<IAttributeList>().map(|p| p.into_raw().cast())
        } else {
            None
        };
        match made {
            Some(pointer) => {
                *obj = pointer;
                kResultOk
            }
            None => {
                *obj = null_mut();
                kNotImplemented
            }
        }
    }
}

/// Parameters the user changes in the plug-in's window, noted for the host.
type Edits = Arc<Mutex<Vec<(u32, f64)>>>;

struct Handler {
    edits: Edits,
}

impl Class for Handler {
    type Interfaces = (IComponentHandler,);
}

impl IComponentHandlerTrait for Handler {
    unsafe fn beginEdit(&self, _: u32) -> tresult {
        kResultOk
    }

    unsafe fn performEdit(&self, id: u32, value: f64) -> tresult {
        if let (true, Ok(mut edits)) = (value.is_finite(), self.edits.lock()) {
            if edits.len() < 4096 {
                edits.push((id, value.clamp(0.0, 1.0)));
            }
        }
        kResultOk
    }

    unsafe fn endEdit(&self, _: u32) -> tresult {
        kResultOk
    }

    unsafe fn restartComponent(&self, _: i32) -> tresult {
        kResultOk
    }
}

/// The window a plug-in's view sits in, as that view sees it.
struct Frame {
    shared: Arc<HostShared>,
}

impl Class for Frame {
    type Interfaces = (IPlugFrame,);
}

impl IPlugFrameTrait for Frame {
    unsafe fn resizeView(&self, _: *mut IPlugView, size: *mut s::ViewRect) -> tresult {
        let Some(size) = size.as_ref() else { return kInvalidArgument };
        let (width, height) = (size.right - size.left, size.bottom - size.top);
        if width <= 0 || height <= 0 {
            return kInvalidArgument;
        }
        if let Ok(mut wanted) = self.shared.wanted_size.lock() {
            *wanted = Some((width.min(8192) as u32, height.min(8192) as u32));
        }
        kResultOk
    }
}

/// One parameter's change in a block: a single value. The host writes it for
/// the plug-in to read, or the plug-in writes it for the host.
struct Queue {
    change: Cell<(u32, f64)>,
}

impl Class for Queue {
    type Interfaces = (IParamValueQueue,);
}

impl IParamValueQueueTrait for Queue {
    unsafe fn getParameterId(&self) -> u32 {
        self.change.get().0
    }

    unsafe fn getPointCount(&self) -> i32 {
        1
    }

    unsafe fn getPoint(&self, index: i32, offset: *mut i32, value: *mut f64) -> tresult {
        if index != 0 || offset.is_null() || value.is_null() {
            return kInvalidArgument;
        }
        *offset = 0;
        *value = self.change.get().1;
        kResultOk
    }

    unsafe fn addPoint(&self, _: i32, value: f64, index: *mut i32) -> tresult {
        // Only the last value of a block is kept.
        self.change.set((self.change.get().0, value));
        if !index.is_null() {
            *index = 0;
        }
        kResultOk
    }
}

/// The parameter changes of a block: those the host hands the plug-in, or
/// those the plug-in reports (a meter, a parameter it moved by itself).
struct Changes {
    count: Cell<usize>,
    queues: Vec<ComWrapper<Queue>>,
}

impl Changes {
    fn new() -> ComWrapper<Self> {
        ComWrapper::new(Self { count: Cell::new(0), queues: (0..MAX_CHANGES).map(|_| ComWrapper::new(Queue { change: Cell::new((0, 0.0)) })).collect() })
    }
}

impl Class for Changes {
    type Interfaces = (IParameterChanges,);
}

impl IParameterChangesTrait for Changes {
    unsafe fn getParameterCount(&self) -> i32 {
        self.count.get() as i32
    }

    unsafe fn getParameterData(&self, index: i32) -> *mut IParamValueQueue {
        if index < 0 || index as usize >= self.count.get() {
            return null_mut();
        }
        self.queues[index as usize].as_com_ref::<IParamValueQueue>().map_or(null_mut(), |queue| queue.as_ptr())
    }

    unsafe fn addParameterData(&self, id: *const u32, index: *mut i32) -> *mut IParamValueQueue {
        let count = self.count.get();
        if id.is_null() {
            return null_mut();
        }
        // The same parameter again gets the queue it already has.
        let at = (0..count).find(|i| self.queues[*i].change.get().0 == *id).unwrap_or(count);
        if at >= self.queues.len() {
            return null_mut();
        }
        if at == count {
            self.queues[at].change.set((*id, f64::NAN));
            self.count.set(count + 1);
        }
        if !index.is_null() {
            *index = at as i32;
        }
        self.queues[at].as_com_ref::<IParamValueQueue>().map_or(null_mut(), |queue| queue.as_ptr())
    }
}

/// The notes of a block.
struct Events {
    list: RefCell<Vec<v::Event>>,
}

impl Class for Events {
    type Interfaces = (IEventList,);
}

impl IEventListTrait for Events {
    unsafe fn getEventCount(&self) -> i32 {
        self.list.borrow().len() as i32
    }

    unsafe fn getEvent(&self, index: i32, event: *mut v::Event) -> tresult {
        match (usize::try_from(index).ok().and_then(|i| self.list.borrow().get(i).copied()), event.is_null()) {
            (Some(found), false) => {
                *event = found;
                kResultOk
            }
            _ => kInvalidArgument,
        }
    }

    unsafe fn addEvent(&self, _: *mut v::Event) -> tresult {
        kResultOk
    }
}

// ----- a running plug-in -------------------------------------------------------------------

/// What both faces of a running plug-in share. The plug-in is shut down when
/// the last of them goes.
struct Core {
    component: ComPtr<IComponent>,
    processor: ComPtr<IAudioProcessor>,
    controller: Option<ComPtr<IEditController>>,
    /// The controller is an object of its own, to be shut down as well.
    separate: bool,
    connection: Option<(ComPtr<IConnectionPoint>, ComPtr<IConnectionPoint>)>,
    shared: Arc<HostShared>,
    edits: Edits,
    /// What the plug-in reported while computing sound, for its other half.
    reported: Edits,
    _handler: ComWrapper<Handler>,
    _host: ComWrapper<HostApp>,
    /// Keeps the plug-in's code loaded for as long as the plug-in lives.
    _library: Arc<Vst3Library>,
}

// SAFETY: VST3 divides a plug-in's functions between the audio thread
// (processing) and the main thread (everything else) and requires plug-ins to
// cope with both running at once. The instance only calls the audio functions,
// the handle only the others.
unsafe impl Send for Core {}
unsafe impl Sync for Core {}

impl Drop for Core {
    fn drop(&mut self) {
        // SAFETY: the reverse of `Vst3Instance::new`, on a plug-in nobody uses any more.
        unsafe {
            self.processor.setProcessing(0);
            self.component.setActive(0);
            if let Some((a, b)) = self.connection.take() {
                a.disconnect(b.as_ptr());
                b.disconnect(a.as_ptr());
            }
            if let Some(controller) = &self.controller {
                controller.setComponentHandler(null_mut());
                if self.separate {
                    controller.terminate();
                }
            }
            self.component.terminate();
        }
    }
}

/// The buffers of one bus. Sound is kept in 32 bits; a plug-in that only works
/// in 64 bits gets copies in that size.
struct Bus {
    channels: Vec<Vec<f32>>,
    pointers: Vec<*mut f32>,
    wide: Vec<Vec<f64>>,
    wide_pointers: Vec<*mut f64>,
}

impl Bus {
    fn new(channels: usize) -> Self {
        let mut channels: Vec<Vec<f32>> = (0..channels).map(|_| vec![0.0; MAX_FRAMES]).collect();
        let pointers = channels.iter_mut().map(|c| c.as_mut_ptr()).collect();
        let mut wide: Vec<Vec<f64>> = channels.iter().map(|_| vec![0.0; MAX_FRAMES]).collect();
        let wide_pointers = wide.iter_mut().map(|c| c.as_mut_ptr()).collect();
        Self { channels, pointers, wide, wide_pointers }
    }
}

/// The audio face of a running VST3 plug-in.
pub struct Vst3Instance {
    core: Arc<Core>,
    inputs: Vec<Bus>,
    outputs: Vec<Bus>,
    input_buffers: Vec<v::AudioBusBuffers>,
    output_buffers: Vec<v::AudioBusBuffers>,
    changes: ComWrapper<Changes>,
    /// What the plug-in reports in a block.
    answers: ComWrapper<Changes>,
    events: ComWrapper<Events>,
    /// The plug-in computes in 64 bits.
    wide: bool,
    /// Parameter changes and notes waiting for the next block.
    pending: Vec<(u32, f64)>,
    notes: Vec<v::Event>,
    instrument: bool,
    context: v::ProcessContext,
}

// SAFETY: the instance is only ever used by one thread at a time (it lives in
// the engine, behind its mutex); see `Core` for what it shares.
unsafe impl Send for Vst3Instance {}

fn note(kind: u16, pitch: u8, velocity: f32) -> v::Event {
    // SAFETY: an event is plain data; all-zero is a valid one.
    let mut event: v::Event = unsafe { std::mem::zeroed() };
    event.r#type = kind;
    if kind == v::Event_::EventTypes_::kNoteOnEvent as u16 {
        event.__field0.noteOn = v::NoteOnEvent { channel: 0, pitch: i16::from(pitch), tuning: 0.0, velocity, length: 0, noteId: -1 };
    } else {
        event.__field0.noteOff = v::NoteOffEvent { channel: 0, pitch: i16::from(pitch), velocity, noteId: -1, tuning: 0.0 };
    }
    event
}

impl Vst3Instance {
    /// Starts the plug-in `plugin_id` of a loaded file. Also returns its
    /// main-thread face and its parameters.
    pub fn new(library: &Arc<Vst3Library>, plugin_id: &str, sample_rate: f64) -> Result<(Self, Vst3Handle, Vec<ParamInfo>), String> {
        let cid = library.classes.iter().find(|c| c.0 == plugin_id).map(|c| c.1).ok_or(tr("the plug-in is not in this file"))?;
        let factory = library.factory.as_ref().ok_or(tr("the plug-in is not in this file"))?;
        let host = ComWrapper::new(HostApp);
        let context: *mut FUnknown = host.as_com_ref::<IHostApplication>().map_or(null_mut(), |h| h.as_ptr().cast());
        let edits: Edits = Arc::default();
        let handler = ComWrapper::new(Handler { edits: Arc::clone(&edits) });
        // SAFETY: the calls VST3 prescribes to start a plug-in, in order, each checked.
        unsafe {
            let mut made: *mut c_void = null_mut();
            factory.createInstance(cid.as_ptr(), iid::<IComponent>(), &mut made);
            let component = ComPtr::from_raw(made.cast::<IComponent>()).ok_or(tr("the plug-in could not be created"))?;
            if component.initialize(context) != kResultOk {
                return Err(tr("the plug-in refused to initialise").to_owned());
            }
            let Some(processor) = component.cast::<IAudioProcessor>() else {
                component.terminate();
                return Err(tr("this file is not a VST3 plug-in").to_owned());
            };

            // The half that knows the parameters and the window: the same object,
            // or one of its own that the first names.
            let mut separate = false;
            let mut controller = component.cast::<IEditController>();
            if controller.is_none() {
                let mut controller_class: TUID = [0; 16];
                if component.getControllerClassId(&mut controller_class) == kResultOk {
                    let mut made: *mut c_void = null_mut();
                    factory.createInstance(controller_class.as_ptr(), iid::<IEditController>(), &mut made);
                    controller = ComPtr::from_raw(made.cast::<IEditController>()).filter(|c| c.initialize(context) == kResultOk);
                    separate = controller.is_some();
                }
            }
            let mut connection = None;
            if let (true, Some(controller)) = (separate, &controller) {
                if let (Some(a), Some(b)) = (component.cast::<IConnectionPoint>(), controller.cast::<IConnectionPoint>()) {
                    a.connect(b.as_ptr());
                    b.connect(a.as_ptr());
                    connection = Some((a, b));
                }
            }
            if let Some(controller) = &controller {
                controller.setComponentHandler(handler.as_com_ref::<IComponentHandler>().map_or(null_mut(), |h| h.as_ptr()));
                // The controller starts from what the other half holds.
                let state = Stream::new(Vec::new());
                if let Some(stream) = state.as_com_ref::<IBStream>() {
                    if component.getState(stream.as_ptr()) == kResultOk {
                        state.position.set(0);
                        controller.setComponentState(stream.as_ptr());
                    }
                }
            }
            let core = Arc::new(Core {
                component,
                processor,
                controller,
                separate,
                connection,
                shared: Arc::new(HostShared::default()),
                edits,
                reported: Arc::default(),
                _handler: handler,
                _host: host,
                _library: Arc::clone(library),
            });
            // From here on, dropping `core` shuts the plug-in down.
            let (component, processor) = (&core.component, &core.processor);

            let audio = v::MediaTypes_::kAudio;
            let events = v::MediaTypes_::kEvent;
            let (input, output) = (v::BusDirections_::kInput, v::BusDirections_::kOutput);
            let buses = |direction: i32| -> Vec<Bus> {
                (0..component.getBusCount(audio, direction).clamp(0, 16))
                    .map(|index| {
                        let mut info: v::BusInfo = std::mem::zeroed();
                        component.getBusInfo(audio, direction, index, &mut info);
                        // Sound goes in by the main bus only (a side-chain input stays
                        // silent); every output is listened to and mixed.
                        component.activateBus(audio, direction, index, u8::from(index == 0 || direction == output));
                        Bus::new(info.channelCount.clamp(0, 32) as usize)
                    })
                    .collect()
            };
            let (inputs, outputs) = (buses(input), buses(output));
            if outputs.first().is_none_or(|bus| bus.channels.is_empty()) {
                return Err(tr("this plug-in has no audio output").to_owned());
            }
            let takes_notes = component.getBusCount(events, input) > 0;
            if takes_notes {
                component.activateBus(events, input, 0, 1);
            }
            let instrument = library.plugins.iter().any(|p| p.id == plugin_id && p.instrument) || inputs.first().is_none_or(|bus| bus.channels.is_empty());

            // 32 bits when the plug-in can, else 64.
            let wide = processor.canProcessSampleSize(v::SymbolicSampleSizes_::kSample32) != kResultOk;
            let mut setup = v::ProcessSetup {
                processMode: v::ProcessModes_::kRealtime,
                symbolicSampleSize: if wide { v::SymbolicSampleSizes_::kSample64 } else { v::SymbolicSampleSizes_::kSample32 },
                maxSamplesPerBlock: MAX_FRAMES as i32,
                sampleRate: sample_rate,
            };
            if processor.canProcessSampleSize(setup.symbolicSampleSize) != kResultOk || processor.setupProcessing(&mut setup) != kResultOk {
                return Err(tr("the plug-in refused to start").to_owned());
            }
            if component.setActive(1) != kResultOk {
                return Err(tr("the plug-in refused to activate").to_owned());
            }
            processor.setProcessing(1);

            let mut params = Vec::new();
            if let Some(controller) = &core.controller {
                let skipped = v::ParameterInfo_::ParameterFlags_::kIsReadOnly | v::ParameterInfo_::ParameterFlags_::kIsHidden | v::ParameterInfo_::ParameterFlags_::kIsProgramChange;
                for index in 0..controller.getParameterCount().clamp(0, 4096) {
                    let mut info: v::ParameterInfo = std::mem::zeroed();
                    if controller.getParameterInfo(index, &mut info) == kResultOk && info.flags & skipped == 0 {
                        params.push(ParamInfo { id: info.id, name: text16(&info.title), min: 0.0, max: 1.0, default: info.defaultNormalizedValue.clamp(0.0, 1.0) });
                    }
                }
            }

            let buffers = |buses: &mut Vec<Bus>| -> Vec<v::AudioBusBuffers> {
                buses
                    .iter_mut()
                    .map(|bus| v::AudioBusBuffers {
                        numChannels: bus.channels.len() as i32,
                        silenceFlags: 0,
                        __field0: if wide {
                            v::AudioBusBuffers__type0 { channelBuffers64: bus.wide_pointers.as_mut_ptr() }
                        } else {
                            v::AudioBusBuffers__type0 { channelBuffers32: bus.pointers.as_mut_ptr() }
                        },
                    })
                    .collect()
            };
            let (mut inputs, mut outputs) = (inputs, outputs);
            let (input_buffers, output_buffers) = (buffers(&mut inputs), buffers(&mut outputs));
            let mut context: v::ProcessContext = std::mem::zeroed();
            context.sampleRate = sample_rate;
            context.tempo = 120.0;
            context.timeSigNumerator = 4;
            context.timeSigDenominator = 4;
            context.state = (v::ProcessContext_::StatesAndFlags_::kPlaying | v::ProcessContext_::StatesAndFlags_::kTempoValid | v::ProcessContext_::StatesAndFlags_::kTimeSigValid) as u32;
            let instance = Self {
                core: Arc::clone(&core),
                inputs,
                outputs,
                input_buffers,
                output_buffers,
                changes: Changes::new(),
                answers: Changes::new(),
                wide,
                events: ComWrapper::new(Events { list: RefCell::new(Vec::with_capacity(4 * MAX_CHANGES)) }),
                pending: Vec::with_capacity(4 * MAX_CHANGES),
                notes: Vec::with_capacity(4 * MAX_CHANGES),
                instrument,
                context,
            };
            let handle = Vst3Handle { view: None, window: None, core };
            Ok((instance, handle, params))
        }
    }
}

impl ExternalUnit for Vst3Instance {
    fn process(&mut self, buf: &mut [f32]) {
        for chunk in buf.chunks_mut(MAX_FRAMES) {
            let frames = chunk.len();
            for (index, bus) in self.inputs.iter_mut().enumerate() {
                for channel in &mut bus.channels {
                    if index == 0 {
                        channel[..frames].copy_from_slice(chunk);
                    } else {
                        channel[..frames].fill(0.0);
                    }
                }
            }
            for channel in self.outputs.iter_mut().flat_map(|bus| &mut bus.channels) {
                channel[..frames].fill(0.0);
            }
            // The latest value of each parameter, as far as there is room.
            let count = self.pending.len().min(MAX_CHANGES);
            for (queue, change) in self.changes.queues.iter().zip(self.pending.drain(..count)) {
                queue.change.set(change);
            }
            self.changes.count.set(count);
            self.answers.count.set(0);
            std::mem::swap(&mut *self.events.list.borrow_mut(), &mut self.notes);
            self.notes.clear();
            if self.wide {
                for bus in self.inputs.iter_mut().chain(&mut self.outputs) {
                    for (wide, channel) in bus.wide.iter_mut().zip(&bus.channels) {
                        for (to, from) in wide[..frames].iter_mut().zip(channel) {
                            *to = f64::from(*from);
                        }
                    }
                }
            }

            let mut data = v::ProcessData {
                processMode: v::ProcessModes_::kRealtime,
                symbolicSampleSize: if self.wide { v::SymbolicSampleSizes_::kSample64 } else { v::SymbolicSampleSizes_::kSample32 },
                numSamples: frames as i32,
                numInputs: self.input_buffers.len() as i32,
                numOutputs: self.output_buffers.len() as i32,
                inputs: if self.input_buffers.is_empty() { null_mut() } else { self.input_buffers.as_mut_ptr() },
                outputs: self.output_buffers.as_mut_ptr(),
                inputParameterChanges: self.changes.as_com_ref::<IParameterChanges>().map_or(null_mut(), |c| c.as_ptr()),
                outputParameterChanges: self.answers.as_com_ref::<IParameterChanges>().map_or(null_mut(), |c| c.as_ptr()),
                inputEvents: self.events.as_com_ref::<IEventList>().map_or(null_mut(), |e| e.as_ptr()),
                outputEvents: null_mut(),
                processContext: &raw mut self.context,
            };
            // SAFETY: the plug-in was started by `new`; every buffer holds
            // MAX_FRAMES samples and blocks never exceed that.
            unsafe {
                self.core.processor.process(&mut data);
            }
            self.context.projectTimeSamples += frames as i64;
            self.events.list.borrow_mut().clear();
            if self.wide {
                for bus in &mut self.outputs {
                    for (channel, wide) in bus.channels.iter_mut().zip(&bus.wide) {
                        for (to, from) in channel[..frames].iter_mut().zip(wide) {
                            *to = *from as f32;
                        }
                    }
                }
            }
            // What the plug-in reported goes to its other half, on the main thread.
            let answered = self.answers.count.get();
            if answered > 0 {
                if let Ok(mut reported) = self.core.reported.try_lock() {
                    for queue in &self.answers.queues[..answered] {
                        let (id, value) = queue.change.get();
                        if value.is_finite() && reported.len() < 4096 {
                            reported.retain(|known| known.0 != id);
                            reported.push((id, value));
                        }
                    }
                }
            }

            // Every output bus is mixed: left and right of each, averaged.
            for (i, sample) in chunk.iter_mut().enumerate() {
                let mut out = 0.0;
                for bus in &self.outputs {
                    if let Some(left) = bus.channels.first() {
                        out += bus.channels.get(1).map_or(left[i], |right| 0.5 * (left[i] + right[i]));
                    }
                }
                if !out.is_finite() {
                    out = 0.0;
                }
                *sample = if self.instrument { *sample + out } else { out };
            }
        }
    }

    fn note_on(&mut self, pitch: u8, velocity: u8) {
        if self.notes.len() < self.notes.capacity() {
            self.notes.push(note(v::Event_::EventTypes_::kNoteOnEvent as u16, pitch.min(127), f32::from(velocity.clamp(1, 127)) / 127.0));
        }
    }

    fn note_off(&mut self, pitch: u8) {
        if self.notes.len() < self.notes.capacity() {
            self.notes.push(note(v::Event_::EventTypes_::kNoteOffEvent as u16, pitch.min(127), 0.0));
        }
    }

    fn set_param(&mut self, id: u32, value: f64) {
        if !value.is_finite() {
            return;
        }
        self.pending.retain(|change| change.0 != id);
        if self.pending.len() < self.pending.capacity() {
            self.pending.push((id, value.clamp(0.0, 1.0)));
        }
    }

    fn all_notes_off(&mut self) {
        self.notes.clear();
        for pitch in 0..128 {
            self.note_off(pitch);
        }
    }
}

// ----- the main-thread face: window and state -------------------------------------

/// The main-thread face of a running plug-in. Use it from the interface thread only.
pub struct Vst3Handle {
    // Before `core`, so that the view and its window go before the plug-in can.
    view: Option<(ComPtr<IPlugView>, ComWrapper<Frame>)>,
    window: Option<HostWindow>,
    core: Arc<Core>,
}

impl Vst3Handle {
    fn make_view(&self) -> Option<ComPtr<IPlugView>> {
        // SAFETY: a main-thread call on a live plug-in; the result is checked.
        unsafe {
            let controller = self.core.controller.as_ref()?;
            let view = ComPtr::from_raw(controller.createView(v::ViewType::kEditor))?;
            (view.isPlatformTypeSupported(s::kPlatformTypeHWND) == kResultTrue).then_some(view)
        }
    }

    fn rect(width: u32, height: u32) -> s::ViewRect {
        s::ViewRect { left: 0, top: 0, right: width as i32, bottom: height as i32 }
    }

    /// Loads what `save_state` wrote. Call it before sound is asked of the plug-in.
    pub fn load_state(&self, data: &[u8]) -> bool {
        let Some(length) = data.get(..4).map(|l| u32::from_le_bytes([l[0], l[1], l[2], l[3]]) as usize) else { return false };
        let Some((component_state, controller_state)) = data[4..].split_at_checked(length) else { return false };
        // SAFETY: the streams only live for the calls and read from `data`.
        unsafe {
            let stream = Stream::new(component_state.to_vec());
            let Some(pointer) = stream.as_com_ref::<IBStream>().map(|p| p.as_ptr()) else { return false };
            if self.core.component.setState(pointer) != kResultOk {
                return false;
            }
            if let Some(controller) = &self.core.controller {
                stream.position.set(0);
                controller.setComponentState(pointer);
                let own = Stream::new(controller_state.to_vec());
                if let (false, Some(pointer)) = (controller_state.is_empty(), own.as_com_ref::<IBStream>().map(|p| p.as_ptr())) {
                    controller.setState(pointer);
                }
            }
            true
        }
    }
}

impl crate::plugin::PluginHandle for Vst3Handle {
    fn has_window(&self) -> bool {
        cfg!(windows) && self.core.controller.is_some()
    }

    fn window_open(&self) -> bool {
        self.window.is_some()
    }

    fn open_window(&mut self, title: &str) -> Result<(), String> {
        if self.window.is_some() {
            return Ok(());
        }
        let view = self.make_view().ok_or(tr("this plug-in has no window that can be used here"))?;
        let frame = ComWrapper::new(Frame { shared: Arc::clone(&self.core.shared) });
        // SAFETY: the calls VST3 prescribes for an embedded window, in order.
        unsafe {
            view.setFrame(frame.as_com_ref::<IPlugFrame>().map_or(null_mut(), |f| f.as_ptr()));
            let mut size = Self::rect(480, 320);
            view.getSize(&mut size);
            let (width, height) = ((size.right - size.left).clamp(64, 8192) as u32, (size.bottom - size.top).clamp(64, 8192) as u32);
            let resizable = view.canResize() == kResultTrue;
            let host = match HostWindow::new(title, width, height, resizable, &self.core.shared) {
                Ok(host) => host,
                Err(e) => {
                    view.setFrame(null_mut());
                    return Err(e);
                }
            };
            if view.attached(host.handle(), s::kPlatformTypeHWND) != kResultOk {
                view.setFrame(null_mut());
                return Err(tr("the plug-in refused the window offered").to_owned());
            }
            host.show();
            self.view = Some((view, frame));
            self.window = Some(host);
        }
        Ok(())
    }

    fn close_window(&mut self) {
        // The plug-in lets go of our window before the window is destroyed.
        if let Some((view, _frame)) = self.view.take() {
            // SAFETY: the view is attached to a window of ours that still exists.
            unsafe {
                view.removed();
                view.setFrame(null_mut());
            }
        }
        self.window = None;
    }

    fn idle(&mut self) -> crate::plugin::Idle {
        let mut result = crate::plugin::Idle::default();
        let shared = Arc::clone(&self.core.shared);
        if let (Some((view, _)), Some(host)) = (&self.view, &self.window) {
            // SAFETY: main-thread calls on a view that is attached.
            unsafe {
                if let Some((width, height)) = shared.wanted_size.lock().ok().and_then(|mut w| w.take()) {
                    host.resize(width, height);
                    view.onSize(&mut Self::rect(width, height));
                    // That resize is ours, not the user's.
                    if let Ok(mut user) = shared.user_size.lock() {
                        *user = None;
                    }
                }
                if let Some((width, height)) = shared.user_size.lock().ok().and_then(|mut s| s.take()) {
                    let mut size = Self::rect(width, height);
                    view.checkSizeConstraint(&mut size);
                    view.onSize(&mut size);
                }
            }
        }
        result.changes = self.core.edits.lock().map(|mut edits| std::mem::take(&mut *edits)).unwrap_or_default();
        // What the sound half reported while computing is shown by the other half.
        let reported = self.core.reported.lock().map(|mut reported| std::mem::take(&mut *reported)).unwrap_or_default();
        if let Some(controller) = &self.core.controller {
            for (id, value) in reported {
                // SAFETY: a main-thread call on a live plug-in.
                unsafe {
                    controller.setParamNormalized(id, value.clamp(0.0, 1.0));
                }
            }
        }
        if shared.closed.swap(false, Ordering::AcqRel) && self.window.is_some() {
            self.close_window();
            result.closed = true;
        }
        result
    }

    fn param_value(&self, id: u32) -> Option<f64> {
        // SAFETY: a main-thread query on a live plug-in.
        let value = unsafe { self.core.controller.as_ref()?.getParamNormalized(id) };
        value.is_finite().then_some(value)
    }

    fn param_changed(&mut self, id: u32, value: f64) {
        // The half that draws the window is told what the sliders set.
        if let (true, Some(controller)) = (value.is_finite(), &self.core.controller) {
            // SAFETY: a main-thread call on a live plug-in.
            unsafe {
                controller.setParamNormalized(id, value.clamp(0.0, 1.0));
            }
        }
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        // SAFETY: the streams only live for the calls and write into their own bytes.
        unsafe {
            let component = Stream::new(Vec::new());
            if self.core.component.getState(component.as_com_ref::<IBStream>()?.as_ptr()) != kResultOk {
                return None;
            }
            let controller = Stream::new(Vec::new());
            if let Some(edit) = &self.core.controller {
                edit.getState(controller.as_com_ref::<IBStream>()?.as_ptr());
            }
            let (first, second) = (component.data.borrow(), controller.data.borrow());
            let mut out = (first.len() as u32).to_le_bytes().to_vec();
            out.extend_from_slice(&first);
            out.extend_from_slice(&second);
            Some(out)
        }
    }

    fn load_state(&self, data: &[u8]) -> bool {
        Vst3Handle::load_state(self, data)
    }

    #[cfg(test)]
    fn window_handle(&self) -> Option<*mut c_void> {
        self.window.as_ref().map(HostWindow::handle)
    }
}

impl Drop for Vst3Handle {
    fn drop(&mut self) {
        crate::plugin::PluginHandle::close_window(self);
    }
}
