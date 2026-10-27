//! A minimal CLAP plug-in file with two plug-ins, written against the public
//! CLAP headers. It exists so that the host in OpenGarageMusic can be tested
//! against a real plug-in binary:
//!
//! - `org.opengarage.test.gain`: an effect with one parameter (gain, 0 to 2).
//! - `org.opengarage.test.sine`: an instrument playing a sine at the last note.

#![allow(clippy::missing_safety_doc)]

pub mod vst3_plugin;

use std::ffi::{c_char, c_void, CStr};
use std::ptr::null;

use clap_sys::events::{
    clap_event_note, clap_event_param_value, clap_input_events, clap_output_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON, CLAP_EVENT_PARAM_VALUE,
};
use clap_sys::ext::gui::{clap_gui_resize_hints, clap_plugin_gui, clap_window, CLAP_EXT_GUI, CLAP_WINDOW_API_WIN32};
use clap_sys::ext::params::{clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS, CLAP_PARAM_IS_AUTOMATABLE};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::entry::clap_plugin_entry;
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::id::clap_id;
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::process::{clap_process, clap_process_status, CLAP_PROCESS_CONTINUE};
use clap_sys::version::CLAP_VERSION;

pub const GAIN_ID: &CStr = c"org.opengarage.test.gain";
pub const SINE_ID: &CStr = c"org.opengarage.test.sine";
const GAIN_PARAM: clap_id = 1;

struct Features<const N: usize>([*const c_char; N]);
// SAFETY: the pointers are to string literals, which live for ever and are never written.
unsafe impl<const N: usize> Sync for Features<N> {}

static GAIN_FEATURES: Features<2> = Features([c"audio-effect".as_ptr(), null()]);
static SINE_FEATURES: Features<3> = Features([c"instrument".as_ptr(), c"synthesizer".as_ptr(), null()]);

const fn descriptor(id: &'static CStr, name: &'static CStr, features: *const *const c_char) -> clap_plugin_descriptor {
    clap_plugin_descriptor {
        clap_version: CLAP_VERSION,
        id: id.as_ptr(),
        name: name.as_ptr(),
        vendor: c"OpenGarageMusic".as_ptr(),
        url: c"".as_ptr(),
        manual_url: c"".as_ptr(),
        support_url: c"".as_ptr(),
        version: c"0.1.0".as_ptr(),
        description: c"Test plug-in".as_ptr(),
        features,
    }
}

static GAIN_DESC: clap_plugin_descriptor = descriptor(GAIN_ID, c"Gain d'essai", GAIN_FEATURES.0.as_ptr());
static SINE_DESC: clap_plugin_descriptor = descriptor(SINE_ID, c"Sinus d'essai", SINE_FEATURES.0.as_ptr());

#[cfg(windows)]
#[link(name = "user32")]
extern "system" {
    fn CreateWindowExW(ex: u32, class: *const u16, name: *const u16, style: u32, x: i32, y: i32, w: i32, h: i32, parent: isize, menu: isize, instance: isize, param: *const c_void) -> isize;
    fn DestroyWindow(window: isize) -> i32;
}

struct State {
    plugin: clap_plugin,
    host: *const clap_host,
    /// The child window the plug-in draws in, when its window is open.
    child: isize,
    gui: bool,
    main_thread_calls: u32,
    instrument: bool,
    gain: f64,
    sample_rate: f64,
    note: Option<i16>,
    phase: f64,
}

unsafe fn state<'a>(plugin: *const clap_plugin) -> &'a mut State {
    &mut *((*plugin).plugin_data.cast::<State>())
}

unsafe extern "C" fn init(_: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn destroy(plugin: *const clap_plugin) {
    gui_destroy(plugin);
    drop(Box::from_raw((*plugin).plugin_data.cast::<State>()));
}

unsafe extern "C" fn activate(plugin: *const clap_plugin, sample_rate: f64, _: u32, _: u32) -> bool {
    state(plugin).sample_rate = sample_rate;
    true
}

unsafe extern "C" fn nothing(_: *const clap_plugin) {}

unsafe extern "C" fn on_main_thread(plugin: *const clap_plugin) {
    state(plugin).main_thread_calls += 1;
}

// ----- window: a plain child window of the host's, on Windows -----------------------

unsafe extern "C" fn gui_supported(plugin: *const clap_plugin, api: *const c_char, floating: bool) -> bool {
    cfg!(windows) && !state(plugin).instrument && !floating && CStr::from_ptr(api) == CLAP_WINDOW_API_WIN32
}

unsafe extern "C" fn gui_preferred(_: *const clap_plugin, api: *mut *const c_char, floating: *mut bool) -> bool {
    *api = CLAP_WINDOW_API_WIN32.as_ptr();
    *floating = false;
    true
}

unsafe extern "C" fn gui_create(plugin: *const clap_plugin, api: *const c_char, floating: bool) -> bool {
    if !gui_supported(plugin, api, floating) || state(plugin).gui {
        return false;
    }
    let s = state(plugin);
    s.gui = true;
    // Like real plug-ins, ask for a call on the main thread.
    if let Some(request) = s.host.as_ref().and_then(|h| h.request_callback) {
        request(s.host);
    }
    true
}

unsafe extern "C" fn gui_destroy(plugin: *const clap_plugin) {
    let s = state(plugin);
    #[cfg(windows)]
    if s.child != 0 {
        DestroyWindow(s.child);
    }
    s.child = 0;
    s.gui = false;
}

unsafe extern "C" fn gui_scale(_: *const clap_plugin, _: f64) -> bool {
    true
}

unsafe extern "C" fn gui_get_size(_: *const clap_plugin, width: *mut u32, height: *mut u32) -> bool {
    *width = 320;
    *height = 200;
    true
}

unsafe extern "C" fn gui_no(_: *const clap_plugin) -> bool {
    false
}

unsafe extern "C" fn gui_hints(_: *const clap_plugin, _: *mut clap_gui_resize_hints) -> bool {
    false
}

unsafe extern "C" fn gui_adjust(_: *const clap_plugin, _: *mut u32, _: *mut u32) -> bool {
    true
}

unsafe extern "C" fn gui_set_size(_: *const clap_plugin, _: u32, _: u32) -> bool {
    true
}

unsafe extern "C" fn gui_set_parent(plugin: *const clap_plugin, window: *const clap_window) -> bool {
    let s = state(plugin);
    if !s.gui || window.is_null() {
        return false;
    }
    #[cfg(windows)]
    {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let text: Vec<u16> = "Gain d'essai\0".encode_utf16().collect();
        // WS_CHILD | WS_VISIBLE
        s.child = CreateWindowExW(0, class.as_ptr(), text.as_ptr(), 0x5000_0000, 0, 0, 320, 200, (*window).specific.win32 as isize, 0, 0, null());
    }
    s.child != 0
}

unsafe extern "C" fn gui_set_transient(_: *const clap_plugin, _: *const clap_window) -> bool {
    false
}

unsafe extern "C" fn gui_title(_: *const clap_plugin, _: *const c_char) {}

unsafe extern "C" fn gui_yes(plugin: *const clap_plugin) -> bool {
    state(plugin).gui
}

static GUI: clap_plugin_gui = clap_plugin_gui {
    is_api_supported: Some(gui_supported),
    get_preferred_api: Some(gui_preferred),
    create: Some(gui_create),
    destroy: Some(gui_destroy),
    set_scale: Some(gui_scale),
    get_size: Some(gui_get_size),
    can_resize: Some(gui_no),
    get_resize_hints: Some(gui_hints),
    adjust_size: Some(gui_adjust),
    set_size: Some(gui_set_size),
    set_parent: Some(gui_set_parent),
    set_transient: Some(gui_set_transient),
    suggest_title: Some(gui_title),
    show: Some(gui_yes),
    hide: Some(gui_yes),
};

// ----- state: the gain, as eight bytes ------------------------------------------------

unsafe extern "C" fn state_save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let bytes = state(plugin).gain.to_le_bytes();
    let Some(write) = stream.as_ref().and_then(|s| s.write) else { return false };
    write(stream, bytes.as_ptr().cast(), 8) == 8
}

unsafe extern "C" fn state_load(plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    let mut bytes = [0u8; 8];
    let Some(read) = stream.as_ref().and_then(|s| s.read) else { return false };
    if read(stream, bytes.as_mut_ptr().cast(), 8) != 8 {
        return false;
    }
    state(plugin).gain = f64::from_le_bytes(bytes).clamp(0.0, 2.0);
    true
}

static STATE: clap_plugin_state = clap_plugin_state { save: Some(state_save), load: Some(state_load) };

unsafe extern "C" fn start_processing(_: *const clap_plugin) -> bool {
    true
}

unsafe fn read_events(s: &mut State, events: *const clap_input_events) {
    let Some(events) = events.as_ref() else { return };
    let (Some(size), Some(get)) = (events.size, events.get) else { return };
    for i in 0..size(events) {
        let header = get(events, i);
        if header.is_null() || (*header).space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        match (*header).type_ {
            CLAP_EVENT_NOTE_ON => s.note = Some((*header.cast::<clap_event_note>()).key),
            CLAP_EVENT_NOTE_OFF => {
                if s.note == Some((*header.cast::<clap_event_note>()).key) {
                    s.note = None;
                }
            }
            CLAP_EVENT_PARAM_VALUE => {
                let event = &*header.cast::<clap_event_param_value>();
                if event.param_id == GAIN_PARAM {
                    // Stand-in for a faulty plug-in: this value brings the whole
                    // process down. Only a host that isolates plug-ins survives it.
                    if event.value == -1313.0 {
                        std::process::abort();
                    }
                    s.gain = event.value.clamp(0.0, 2.0);
                }
            }
            _ => {}
        }
    }
}

unsafe extern "C" fn process(plugin: *const clap_plugin, process: *const clap_process) -> clap_process_status {
    let s = state(plugin);
    let p = &*process;
    read_events(s, p.in_events);
    let frames = p.frames_count as usize;
    if p.audio_outputs_count == 0 {
        return CLAP_PROCESS_CONTINUE;
    }
    let output = &*p.audio_outputs;
    for channel in 0..output.channel_count as usize {
        let out = std::slice::from_raw_parts_mut(*output.data32.add(channel), frames);
        if s.instrument {
            let mut phase = s.phase;
            let step = s.note.map_or(0.0, |key| 440.0 * 2f64.powf((f64::from(key) - 69.0) / 12.0) / s.sample_rate);
            for sample in out.iter_mut() {
                *sample = if s.note.is_some() { (phase * std::f64::consts::TAU).sin() as f32 * 0.5 } else { 0.0 };
                phase = (phase + step).fract();
            }
            if channel + 1 == output.channel_count as usize {
                s.phase = phase;
            }
        } else if p.audio_inputs_count > 0 && channel < (*p.audio_inputs).channel_count as usize {
            let input = std::slice::from_raw_parts(*(*p.audio_inputs).data32.add(channel), frames);
            for (o, i) in out.iter_mut().zip(input) {
                *o = *i * s.gain as f32;
            }
        } else {
            out.fill(0.0);
        }
    }
    CLAP_PROCESS_CONTINUE
}

unsafe extern "C" fn params_count(plugin: *const clap_plugin) -> u32 {
    u32::from(!state(plugin).instrument)
}

unsafe extern "C" fn params_info(plugin: *const clap_plugin, index: u32, info: *mut clap_param_info) -> bool {
    if state(plugin).instrument || index != 0 {
        return false;
    }
    let info = &mut *info;
    info.id = GAIN_PARAM;
    info.flags = CLAP_PARAM_IS_AUTOMATABLE;
    info.cookie = std::ptr::null_mut();
    info.name.fill(0);
    for (slot, byte) in info.name.iter_mut().zip(b"Gain") {
        *slot = *byte as c_char;
    }
    info.module.fill(0);
    info.min_value = 0.0;
    info.max_value = 2.0;
    info.default_value = 0.5;
    true
}

unsafe extern "C" fn params_value(plugin: *const clap_plugin, id: clap_id, out: *mut f64) -> bool {
    if id != GAIN_PARAM {
        return false;
    }
    *out = state(plugin).gain;
    true
}

unsafe extern "C" fn params_flush(plugin: *const clap_plugin, input: *const clap_input_events, _: *const clap_output_events) {
    read_events(state(plugin), input);
}

static PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_info),
    get_value: Some(params_value),
    value_to_text: None,
    text_to_value: None,
    flush: Some(params_flush),
};

unsafe extern "C" fn get_extension(_: *const clap_plugin, id: *const c_char) -> *const c_void {
    let id = CStr::from_ptr(id);
    if id == CLAP_EXT_PARAMS {
        (&raw const PARAMS).cast()
    } else if id == CLAP_EXT_GUI {
        (&raw const GUI).cast()
    } else if id == CLAP_EXT_STATE {
        (&raw const STATE).cast()
    } else {
        null()
    }
}

unsafe extern "C" fn plugin_count(_: *const clap_plugin_factory) -> u32 {
    2
}

unsafe extern "C" fn plugin_descriptor(_: *const clap_plugin_factory, index: u32) -> *const clap_plugin_descriptor {
    match index {
        0 => &GAIN_DESC,
        1 => &SINE_DESC,
        _ => null(),
    }
}

unsafe extern "C" fn create_plugin(_: *const clap_plugin_factory, host: *const clap_host, id: *const c_char) -> *const clap_plugin {
    let id = CStr::from_ptr(id);
    let (desc, instrument) = if id == GAIN_ID {
        (&GAIN_DESC, false)
    } else if id == SINE_ID {
        (&SINE_DESC, true)
    } else {
        return null();
    };
    let state = Box::into_raw(Box::new(State {
        plugin: clap_plugin {
            desc,
            plugin_data: std::ptr::null_mut(),
            init: Some(init),
            destroy: Some(destroy),
            activate: Some(activate),
            deactivate: Some(nothing),
            start_processing: Some(start_processing),
            stop_processing: Some(nothing),
            reset: Some(nothing),
            process: Some(process),
            get_extension: Some(get_extension),
            on_main_thread: Some(on_main_thread),
        },
        host,
        child: 0,
        gui: false,
        main_thread_calls: 0,
        instrument,
        gain: 0.5,
        sample_rate: 48_000.0,
        note: None,
        phase: 0.0,
    }));
    (*state).plugin.plugin_data = state.cast();
    &(*state).plugin
}

static FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(plugin_count),
    get_plugin_descriptor: Some(plugin_descriptor),
    create_plugin: Some(create_plugin),
};

unsafe extern "C" fn entry_init(_: *const c_char) -> bool {
    true
}

unsafe extern "C" fn entry_deinit() {}

unsafe extern "C" fn entry_factory(id: *const c_char) -> *const c_void {
    if CStr::from_ptr(id) == CLAP_PLUGIN_FACTORY_ID {
        (&raw const FACTORY).cast()
    } else {
        null()
    }
}

/// The symbol a CLAP host looks for.
#[no_mangle]
#[allow(non_upper_case_globals)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_factory),
};

// ----- the same two plug-ins again, as LV2 -------------------------------------------
//
// An LV2 host finds them through the `lv2_descriptor` function below and the
// description files the tests write next to a copy of this library.

pub const LV2_GAIN_URI: &CStr = c"http://opengaragemusic.example/lv2/gain";
pub const LV2_SINE_URI: &CStr = c"http://opengaragemusic.example/lv2/sine";

#[repr(C)]
pub struct Lv2Feature {
    uri: *const c_char,
    data: *mut c_void,
}

#[repr(C)]
pub struct Lv2Descriptor {
    uri: *const c_char,
    instantiate: Option<unsafe extern "C" fn(*const Lv2Descriptor, f64, *const c_char, *const *const Lv2Feature) -> *mut c_void>,
    connect_port: Option<unsafe extern "C" fn(*mut c_void, u32, *mut c_void)>,
    activate: Option<unsafe extern "C" fn(*mut c_void)>,
    run: Option<unsafe extern "C" fn(*mut c_void, u32)>,
    deactivate: Option<unsafe extern "C" fn(*mut c_void)>,
    cleanup: Option<unsafe extern "C" fn(*mut c_void)>,
    extension_data: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}

// SAFETY: the descriptors only hold pointers to string literals and functions.
unsafe impl Sync for Lv2Descriptor {}

#[repr(C)]
struct Lv2UridMap {
    handle: *mut c_void,
    map: unsafe extern "C" fn(*mut c_void, *const c_char) -> u32,
}

struct Lv2State {
    instrument: bool,
    ports: [*mut c_void; 3],
    /// A factor no port holds: it only lives in the plug-in's saved state.
    trim: f32,
    /// Numbers the host gave for the name and the type of that value.
    trim_key: u32,
    float_type: u32,
    midi_type: u32,
    note: Option<u8>,
    phase: f64,
    rate: f64,
}

unsafe extern "C" fn lv2_instantiate(descriptor: *const Lv2Descriptor, rate: f64, bundle: *const c_char, features: *const *const Lv2Feature) -> *mut c_void {
    // Both plug-ins declare that they need the "urid map" feature: refuse to
    // start without it, as a real plug-in would.
    let (mut midi_type, mut trim_key, mut float_type) = (0, 0, 0);
    let mut feature = features;
    while !feature.is_null() && !(*feature).is_null() {
        if CStr::from_ptr((**feature).uri) == c"http://lv2plug.in/ns/ext/urid#map" {
            let map = &*(**feature).data.cast::<Lv2UridMap>();
            midi_type = (map.map)(map.handle, c"http://lv2plug.in/ns/ext/midi#MidiEvent".as_ptr());
            trim_key = (map.map)(map.handle, c"http://opengaragemusic.example/lv2/gain#trim".as_ptr());
            float_type = (map.map)(map.handle, c"http://lv2plug.in/ns/ext/atom#Float".as_ptr());
        }
        feature = feature.add(1);
    }
    if midi_type == 0 || bundle.is_null() || CStr::from_ptr(bundle).to_bytes().is_empty() {
        return std::ptr::null_mut();
    }
    let instrument = CStr::from_ptr((*descriptor).uri) == LV2_SINE_URI;
    Box::into_raw(Box::new(Lv2State { instrument, ports: [std::ptr::null_mut(); 3], trim: 1.0, trim_key, float_type, midi_type, note: None, phase: 0.0, rate })).cast()
}

// The gain's state: one value, saved and restored through the host.

type Lv2Store = unsafe extern "C" fn(*mut c_void, u32, *const c_void, usize, u32, u32) -> u32;
type Lv2Retrieve = unsafe extern "C" fn(*mut c_void, u32, *mut usize, *mut u32, *mut u32) -> *const c_void;

#[repr(C)]
struct Lv2StateInterface {
    save: Option<unsafe extern "C" fn(*mut c_void, Lv2Store, *mut c_void, u32, *const *const Lv2Feature) -> u32>,
    restore: Option<unsafe extern "C" fn(*mut c_void, Lv2Retrieve, *mut c_void, u32, *const *const Lv2Feature) -> u32>,
}

unsafe extern "C" fn lv2_save(instance: *mut c_void, store: Lv2Store, handle: *mut c_void, _: u32, _: *const *const Lv2Feature) -> u32 {
    let s = &*instance.cast::<Lv2State>();
    // Flags: plain data, the same on every machine.
    store(handle, s.trim_key, (&raw const s.trim).cast(), 4, s.float_type, 3)
}

unsafe extern "C" fn lv2_restore(instance: *mut c_void, retrieve: Lv2Retrieve, handle: *mut c_void, _: u32, _: *const *const Lv2Feature) -> u32 {
    let s = &mut *instance.cast::<Lv2State>();
    let (mut size, mut kind, mut flags) = (0usize, 0u32, 0u32);
    let value = retrieve(handle, s.trim_key, &mut size, &mut kind, &mut flags);
    if value.is_null() || size != 4 || kind != s.float_type {
        return 1;
    }
    s.trim = value.cast::<f32>().read_unaligned();
    0
}

static LV2_STATE: Lv2StateInterface = Lv2StateInterface { save: Some(lv2_save), restore: Some(lv2_restore) };

unsafe extern "C" fn lv2_extension(uri: *const c_char) -> *const c_void {
    if !uri.is_null() && CStr::from_ptr(uri) == c"http://lv2plug.in/ns/ext/state#interface" {
        return (&raw const LV2_STATE).cast();
    }
    null()
}

unsafe extern "C" fn lv2_connect(handle: *mut c_void, port: u32, data: *mut c_void) {
    if let Some(slot) = (*handle.cast::<Lv2State>()).ports.get_mut(port as usize) {
        *slot = data;
    }
}

unsafe extern "C" fn lv2_run(handle: *mut c_void, frames: u32) {
    let s = &mut *handle.cast::<Lv2State>();
    let frames = frames as usize;
    if !s.instrument {
        // Ports: 0 audio in, 1 audio out, 2 gain.
        let input = std::slice::from_raw_parts(s.ports[0].cast::<f32>(), frames);
        let output = std::slice::from_raw_parts_mut(s.ports[1].cast::<f32>(), frames);
        let gain = *s.ports[2].cast::<f32>() * s.trim;
        for (o, i) in output.iter_mut().zip(input) {
            *o = *i * gain;
        }
        return;
    }
    // Ports: 0 events in, 1 audio out. The events are a sequence of MIDI messages.
    let words = s.ports[0].cast::<u32>();
    let size = *words as usize;
    let mut offset = 16;
    while offset + 16 <= 8 + size {
        let event = s.ports[0].cast::<u8>().add(offset);
        let (length, kind) = (*event.add(8).cast::<u32>() as usize, *event.add(12).cast::<u32>());
        let data = event.add(16);
        if kind == s.midi_type && length == 3 {
            match (*data & 0xF0, *data.add(1), *data.add(2)) {
                (0x90, key, velocity) if velocity > 0 => s.note = Some(key),
                (0x90 | 0x80, key, _) if s.note == Some(key) => s.note = None,
                (0xB0, 123, _) => s.note = None,
                _ => {}
            }
        }
        offset += 16 + length.div_ceil(8) * 8;
    }
    let output = std::slice::from_raw_parts_mut(s.ports[1].cast::<f32>(), frames);
    let step = s.note.map_or(0.0, |key| 440.0 * 2f64.powf((f64::from(key) - 69.0) / 12.0) / s.rate);
    for sample in output.iter_mut() {
        *sample = if s.note.is_some() { (s.phase * std::f64::consts::TAU).sin() as f32 * 0.5 } else { 0.0 };
        s.phase = (s.phase + step).fract();
    }
}

unsafe extern "C" fn lv2_ignore(_: *mut c_void) {}

unsafe extern "C" fn lv2_cleanup(handle: *mut c_void) {
    drop(Box::from_raw(handle.cast::<Lv2State>()));
}

const fn lv2_plugin(uri: &'static CStr) -> Lv2Descriptor {
    Lv2Descriptor {
        uri: uri.as_ptr(),
        instantiate: Some(lv2_instantiate),
        connect_port: Some(lv2_connect),
        activate: Some(lv2_ignore),
        run: Some(lv2_run),
        deactivate: Some(lv2_ignore),
        cleanup: Some(lv2_cleanup),
        extension_data: Some(lv2_extension),
    }
}

static LV2_GAIN: Lv2Descriptor = lv2_plugin(LV2_GAIN_URI);
static LV2_SINE: Lv2Descriptor = lv2_plugin(LV2_SINE_URI);

/// The function an LV2 host looks for.
#[no_mangle]
pub unsafe extern "C" fn lv2_descriptor(index: u32) -> *const Lv2Descriptor {
    match index {
        0 => &LV2_GAIN,
        1 => &LV2_SINE,
        _ => null(),
    }
}

// ----- the window of the LV2 gain ---------------------------------------------------
//
// A window an LV2 host can put inside one of its own. It stands in for a user:
// when it is told that the gain is 0.25, it "turns the knob" to 1.5.

pub const LV2_GAIN_UI_URI: &CStr = c"http://opengaragemusic.example/lv2/gain#ui";

type Lv2UiWrite = unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void);

#[repr(C)]
pub struct Lv2UiDescriptor {
    uri: *const c_char,
    #[allow(clippy::type_complexity)]
    instantiate: Option<
        unsafe extern "C" fn(*const Lv2UiDescriptor, *const c_char, *const c_char, Lv2UiWrite, *mut c_void, *mut *mut c_void, *const *const Lv2Feature) -> *mut c_void,
    >,
    cleanup: Option<unsafe extern "C" fn(*mut c_void)>,
    port_event: Option<unsafe extern "C" fn(*mut c_void, u32, u32, u32, *const c_void)>,
    extension_data: Option<unsafe extern "C" fn(*const c_char) -> *const c_void>,
}

// SAFETY: the descriptor only holds pointers to constants.
unsafe impl Sync for Lv2UiDescriptor {}

#[repr(C)]
struct Lv2UiResize {
    handle: *mut c_void,
    resize: unsafe extern "C" fn(*mut c_void, i32, i32) -> i32,
}

#[repr(C)]
struct Lv2UiIdle {
    idle: Option<unsafe extern "C" fn(*mut c_void) -> i32>,
}

struct Lv2UiState {
    write: Lv2UiWrite,
    controller: *mut c_void,
    child: isize,
    /// A change the "user" is about to make, sent on the next idle call.
    pending: Option<f32>,
}

unsafe extern "C" fn lv2ui_instantiate(
    _: *const Lv2UiDescriptor,
    plugin: *const c_char,
    _bundle: *const c_char,
    write: Lv2UiWrite,
    controller: *mut c_void,
    widget: *mut *mut c_void,
    features: *const *const Lv2Feature,
) -> *mut c_void {
    if plugin.is_null() || CStr::from_ptr(plugin) != LV2_GAIN_URI || features.is_null() {
        return std::ptr::null_mut();
    }
    let mut parent: isize = 0;
    let mut resize: *const Lv2UiResize = null();
    let mut feature = features;
    while !(*feature).is_null() {
        let uri = CStr::from_ptr((**feature).uri);
        if uri == c"http://lv2plug.in/ns/extensions/ui#parent" {
            parent = (**feature).data as isize;
        } else if uri == c"http://lv2plug.in/ns/extensions/ui#resize" {
            resize = (**feature).data.cast();
        }
        feature = feature.add(1);
    }
    if parent == 0 {
        return std::ptr::null_mut();
    }
    #[cfg(not(windows))]
    let child: isize = 0;
    #[cfg(windows)]
    let child = {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let text: Vec<u16> = "Gain LV2\0".encode_utf16().collect();
        // WS_CHILD | WS_VISIBLE
        CreateWindowExW(0, class.as_ptr(), text.as_ptr(), 0x5000_0000, 0, 0, 300, 120, parent, 0, 0, null())
    };
    if child == 0 {
        return std::ptr::null_mut();
    }
    if let Some(resize) = resize.as_ref() {
        (resize.resize)(resize.handle, 300, 120);
    }
    if !widget.is_null() {
        *widget = child as *mut c_void;
    }
    Box::into_raw(Box::new(Lv2UiState { write, controller, child, pending: None })).cast()
}

unsafe extern "C" fn lv2ui_cleanup(handle: *mut c_void) {
    let state = Box::from_raw(handle.cast::<Lv2UiState>());
    #[cfg(windows)]
    DestroyWindow(state.child);
    drop(state);
}

unsafe extern "C" fn lv2ui_port_event(handle: *mut c_void, port: u32, size: u32, format: u32, buffer: *const c_void) {
    let state = &mut *handle.cast::<Lv2UiState>();
    if port == 2 && size == 4 && format == 0 && !buffer.is_null() && buffer.cast::<f32>().read_unaligned() == 0.25 {
        state.pending = Some(1.5);
    }
}

unsafe extern "C" fn lv2ui_idle(handle: *mut c_void) -> i32 {
    let state = &mut *handle.cast::<Lv2UiState>();
    if let Some(value) = state.pending.take() {
        (state.write)(state.controller, 2, 4, 0, (&raw const value).cast());
    }
    0
}

static LV2_UI_IDLE: Lv2UiIdle = Lv2UiIdle { idle: Some(lv2ui_idle) };

unsafe extern "C" fn lv2ui_extension(uri: *const c_char) -> *const c_void {
    if !uri.is_null() && CStr::from_ptr(uri) == c"http://lv2plug.in/ns/extensions/ui#idleInterface" {
        return (&raw const LV2_UI_IDLE).cast();
    }
    null()
}

static LV2_GAIN_UI: Lv2UiDescriptor = Lv2UiDescriptor {
    uri: LV2_GAIN_UI_URI.as_ptr(),
    instantiate: Some(lv2ui_instantiate),
    cleanup: Some(lv2ui_cleanup),
    port_event: Some(lv2ui_port_event),
    extension_data: Some(lv2ui_extension),
};

/// The function an LV2 host looks for to show a plug-in's window.
#[no_mangle]
pub unsafe extern "C" fn lv2ui_descriptor(index: u32) -> *const Lv2UiDescriptor {
    match index {
        0 => &LV2_GAIN_UI,
        _ => null(),
    }
}
