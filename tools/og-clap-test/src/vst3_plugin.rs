//! The same two test plug-ins once more, as VST3: a gain with a window and a
//! state, and a sine instrument whose controller is an object of its own.
//!
//! A VST3 host finds them through the `GetPluginFactory` function below.

#![allow(non_snake_case)]

use std::cell::{Cell, RefCell};
use std::ffi::{c_char, c_void, CStr};
use std::ptr::null_mut;
use std::sync::atomic::{AtomicU64, Ordering};

use vst3::Steinberg::Vst::*;
use vst3::Steinberg::*;
use vst3::{uid, Class, ComPtr, ComRef, ComWrapper};

/// Id of the gain's parameter: not its index, on purpose.
pub const VST3_GAIN_PARAM: u32 = 7;
/// Id of the level the gain reports while it computes: read-only.
pub const VST3_LEVEL_PARAM: u32 = 8;

const GAIN64_CID: TUID = uid(0x0F6A11C4, 0x4D5A4E04, 0x9A6B0C0D, 0x0E0F1014);

const GAIN_CID: TUID = uid(0x0F6A11C1, 0x4D5A4E01, 0x9A6B0C0D, 0x0E0F1011);
const SINE_CID: TUID = uid(0x0F6A11C2, 0x4D5A4E02, 0x9A6B0C0D, 0x0E0F1012);
const SINE_CONTROLLER_CID: TUID = uid(0x0F6A11C3, 0x4D5A4E03, 0x9A6B0C0D, 0x0E0F1013);

fn copy8(text: &str, to: &mut [c_char]) {
    for (slot, byte) in to.iter_mut().zip(text.bytes().chain(std::iter::once(0))) {
        *slot = byte as c_char;
    }
}

fn copy16(text: &str, to: &mut [u16]) {
    for (slot, unit) in to.iter_mut().zip(text.encode_utf16().chain(std::iter::once(0))) {
        *slot = unit;
    }
}

unsafe fn stereo_bus(bus: *mut BusInfo, name: &str, direction: i32) -> tresult {
    let bus = &mut *bus;
    bus.mediaType = MediaTypes_::kAudio as MediaType;
    bus.direction = direction;
    bus.channelCount = 2;
    copy16(name, &mut bus.name);
    bus.busType = BusTypes_::kMain as BusType;
    bus.flags = BusInfo_::BusFlags_::kDefaultActive as u32;
    kResultOk
}

// ----- the gain: one object for sound, parameters and window --------------------------

struct Gain {
    /// The parameter, from 0 to 1; the gain applied is twice that.
    value: AtomicU64,
    /// The level last reported, as the half that draws would show it.
    level: AtomicU64,
    /// This one only computes in 64 bits.
    wide: bool,
    handler: RefCell<Option<ComPtr<IComponentHandler>>>,
}

impl Class for Gain {
    type Interfaces = (IComponent, IAudioProcessor, IEditController);
}

impl Gain {
    fn new(wide: bool) -> Self {
        Self { value: AtomicU64::new(0.25f64.to_bits()), level: AtomicU64::new(0), wide, handler: RefCell::new(None) }
    }

    fn get(&self) -> f64 {
        f64::from_bits(self.value.load(Ordering::Relaxed))
    }

    unsafe fn read(&self, state: *mut IBStream) -> tresult {
        let Some(stream) = ComRef::from_raw(state) else { return kInvalidArgument };
        let mut bytes = [0u8; 8];
        let mut count = 0;
        stream.read(bytes.as_mut_ptr().cast(), 8, &mut count);
        if count != 8 {
            return kResultFalse;
        }
        self.value.store(f64::from_le_bytes(bytes).clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        kResultOk
    }
}

impl IPluginBaseTrait for Gain {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        *self.handler.borrow_mut() = None;
        kResultOk
    }
}

impl IComponentTrait for Gain {
    unsafe fn getControllerClassId(&self, _: *mut TUID) -> tresult {
        kNotImplemented
    }

    unsafe fn setIoMode(&self, _: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media: MediaType, _: BusDirection) -> i32 {
        i32::from(media == MediaTypes_::kAudio as MediaType)
    }

    unsafe fn getBusInfo(&self, media: MediaType, direction: BusDirection, index: i32, bus: *mut BusInfo) -> tresult {
        if media != MediaTypes_::kAudio as MediaType || index != 0 || bus.is_null() {
            return kInvalidArgument;
        }
        stereo_bus(bus, "Audio", direction)
    }

    unsafe fn getRoutingInfo(&self, _: *mut RoutingInfo, _: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }

    unsafe fn activateBus(&self, _: MediaType, _: BusDirection, _: i32, _: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setActive(&self, _: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setState(&self, state: *mut IBStream) -> tresult {
        self.read(state)
    }

    unsafe fn getState(&self, state: *mut IBStream) -> tresult {
        let Some(stream) = ComRef::from_raw(state) else { return kInvalidArgument };
        let mut bytes = self.get().to_le_bytes();
        stream.write(bytes.as_mut_ptr().cast(), 8, null_mut())
    }
}

unsafe fn last_points(data: &ProcessData, mut each: impl FnMut(u32, f64)) {
    let Some(changes) = ComRef::from_raw(data.inputParameterChanges) else { return };
    for index in 0..changes.getParameterCount() {
        let Some(queue) = ComRef::from_raw(changes.getParameterData(index)) else { continue };
        let (mut offset, mut value) = (0, 0.0);
        if queue.getPoint(queue.getPointCount() - 1, &mut offset, &mut value) == kResultOk {
            each(queue.getParameterId(), value);
        }
    }
}

impl IAudioProcessorTrait for Gain {
    unsafe fn setBusArrangements(&self, _: *mut SpeakerArrangement, _: i32, _: *mut SpeakerArrangement, _: i32) -> tresult {
        kResultFalse
    }

    unsafe fn getBusArrangement(&self, _: BusDirection, index: i32, arrangement: *mut SpeakerArrangement) -> tresult {
        if index != 0 || arrangement.is_null() {
            return kInvalidArgument;
        }
        *arrangement = SpeakerArr::kStereo;
        kResultOk
    }

    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if (size == SymbolicSampleSizes_::kSample64) == self.wide { kResultOk } else { kResultFalse }
    }

    unsafe fn getLatencySamples(&self) -> u32 {
        0
    }

    unsafe fn setupProcessing(&self, _: *mut ProcessSetup) -> tresult {
        kResultOk
    }

    unsafe fn setProcessing(&self, _: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = &*data;
        last_points(data, |id, value| {
            if id == VST3_GAIN_PARAM {
                self.value.store(value.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
            }
        });
        if data.numInputs < 1 || data.numOutputs < 1 || (data.symbolicSampleSize == SymbolicSampleSizes_::kSample64) != self.wide {
            return kResultOk;
        }
        let gain = 2.0 * self.get();
        let (input, output) = (&*data.inputs, &*data.outputs);
        let frames = data.numSamples as usize;
        let mut peak = 0.0f64;
        for channel in 0..input.numChannels.min(output.numChannels) as usize {
            if self.wide {
                let from = std::slice::from_raw_parts(*input.__field0.channelBuffers64.add(channel), frames);
                let to = std::slice::from_raw_parts_mut(*output.__field0.channelBuffers64.add(channel), frames);
                for (out, sample) in to.iter_mut().zip(from) {
                    *out = gain * sample;
                    peak = peak.max(out.abs());
                }
            } else {
                let from = std::slice::from_raw_parts(*input.__field0.channelBuffers32.add(channel), frames);
                let to = std::slice::from_raw_parts_mut(*output.__field0.channelBuffers32.add(channel), frames);
                for (out, sample) in to.iter_mut().zip(from) {
                    *out = gain as f32 * sample;
                    peak = peak.max(f64::from(out.abs()));
                }
            }
        }
        // The level of the block is reported as a parameter of its own.
        if let Some(answers) = ComRef::from_raw(data.outputParameterChanges) {
            let mut index = 0;
            if let Some(queue) = ComRef::from_raw(answers.addParameterData(&VST3_LEVEL_PARAM, &mut index)) {
                queue.addPoint(0, peak.min(1.0), &mut index);
            }
        }
        kResultOk
    }

    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}

impl IEditControllerTrait for Gain {
    unsafe fn setComponentState(&self, state: *mut IBStream) -> tresult {
        self.read(state)
    }

    unsafe fn setState(&self, _: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getState(&self, _: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getParameterCount(&self) -> i32 {
        2
    }

    unsafe fn getParameterInfo(&self, index: i32, info: *mut ParameterInfo) -> tresult {
        if !(0..2).contains(&index) || info.is_null() {
            return kInvalidArgument;
        }
        let info = &mut *info;
        let (id, name, default, flags) = if index == 0 {
            (VST3_GAIN_PARAM, "Gain", 0.25, ParameterInfo_::ParameterFlags_::kCanAutomate)
        } else {
            (VST3_LEVEL_PARAM, "Level", 0.0, ParameterInfo_::ParameterFlags_::kIsReadOnly)
        };
        info.id = id;
        copy16(name, &mut info.title);
        copy16(name, &mut info.shortTitle);
        copy16("", &mut info.units);
        info.stepCount = 0;
        info.defaultNormalizedValue = default;
        info.unitId = 0;
        info.flags = flags;
        kResultOk
    }

    unsafe fn getParamStringByValue(&self, _: u32, value: f64, string: *mut String128) -> tresult {
        copy16(&format!("{:.2}", 2.0 * value), &mut *string);
        kResultOk
    }

    unsafe fn getParamValueByString(&self, _: u32, _: *mut TChar, _: *mut f64) -> tresult {
        kNotImplemented
    }

    unsafe fn normalizedParamToPlain(&self, _: u32, value: f64) -> f64 {
        2.0 * value
    }

    unsafe fn plainParamToNormalized(&self, _: u32, plain: f64) -> f64 {
        plain / 2.0
    }

    unsafe fn getParamNormalized(&self, id: u32) -> f64 {
        match id {
            VST3_GAIN_PARAM => self.get(),
            VST3_LEVEL_PARAM => f64::from_bits(self.level.load(Ordering::Relaxed)),
            _ => 0.0,
        }
    }

    unsafe fn setParamNormalized(&self, id: u32, value: f64) -> tresult {
        if id == VST3_LEVEL_PARAM {
            self.level.store(value.to_bits(), Ordering::Relaxed);
            return kResultOk;
        }
        if id != VST3_GAIN_PARAM {
            return kInvalidArgument;
        }
        self.value.store(value.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
        // Stand-in for a user: told that the parameter is 0.125, the plug-in
        // "turns its own knob" to 0.75 and reports it as an edit.
        if value == 0.125 {
            let handler = self.handler.borrow().clone();
            if let Some(handler) = handler {
                self.value.store(0.75f64.to_bits(), Ordering::Relaxed);
                handler.beginEdit(id);
                handler.performEdit(id, 0.75);
                handler.endEdit(id);
            }
        }
        kResultOk
    }

    unsafe fn setComponentHandler(&self, handler: *mut IComponentHandler) -> tresult {
        *self.handler.borrow_mut() = ComRef::from_raw(handler).map(|h| h.to_com_ptr());
        kResultOk
    }

    unsafe fn createView(&self, name: *const c_char) -> *mut IPlugView {
        if name.is_null() || CStr::from_ptr(name) != c"editor" {
            return null_mut();
        }
        ComWrapper::new(View { child: Cell::new(0) }).to_com_ptr::<IPlugView>().map_or(null_mut(), ComPtr::into_raw)
    }
}

/// The gain's window: a label inside the window the host offers.
struct View {
    child: Cell<isize>,
}

impl Class for View {
    type Interfaces = (IPlugView,);
}

impl IPlugViewTrait for View {
    unsafe fn isPlatformTypeSupported(&self, kind: FIDString) -> tresult {
        if cfg!(windows) && !kind.is_null() && CStr::from_ptr(kind) == c"HWND" { kResultTrue } else { kResultFalse }
    }

    unsafe fn attached(&self, parent: *mut c_void, _: FIDString) -> tresult {
        if parent.is_null() {
            return kInvalidArgument;
        }
        #[cfg(windows)]
        {
            let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
            let text: Vec<u16> = "Gain VST3\0".encode_utf16().collect();
            // WS_CHILD | WS_VISIBLE
            self.child.set(super::CreateWindowExW(0, class.as_ptr(), text.as_ptr(), 0x5000_0000, 0, 0, 300, 100, parent as isize, 0, 0, std::ptr::null()));
        }
        if self.child.get() == 0 { kResultFalse } else { kResultOk }
    }

    unsafe fn removed(&self) -> tresult {
        #[cfg(windows)]
        if self.child.get() != 0 {
            super::DestroyWindow(self.child.get());
        }
        self.child.set(0);
        kResultOk
    }

    unsafe fn onWheel(&self, _: f32) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyDown(&self, _: char16, _: i16, _: i16) -> tresult {
        kResultFalse
    }

    unsafe fn onKeyUp(&self, _: char16, _: i16, _: i16) -> tresult {
        kResultFalse
    }

    unsafe fn getSize(&self, size: *mut ViewRect) -> tresult {
        if size.is_null() {
            return kInvalidArgument;
        }
        *size = ViewRect { left: 0, top: 0, right: 300, bottom: 100 };
        kResultOk
    }

    unsafe fn onSize(&self, _: *mut ViewRect) -> tresult {
        kResultOk
    }

    unsafe fn onFocus(&self, _: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setFrame(&self, _: *mut IPlugFrame) -> tresult {
        kResultOk
    }

    unsafe fn canResize(&self) -> tresult {
        kResultFalse
    }

    unsafe fn checkSizeConstraint(&self, _: *mut ViewRect) -> tresult {
        kResultOk
    }
}

// ----- the sine: sound in one object, parameters (none) in another ----------------------

struct Sine {
    rate: Cell<f64>,
    note: Cell<Option<i16>>,
    phase: Cell<f64>,
    /// The host listens to the second output: half of the sound goes there.
    aux: Cell<bool>,
}

impl Class for Sine {
    type Interfaces = (IComponent, IAudioProcessor);
}

impl IPluginBaseTrait for Sine {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IComponentTrait for Sine {
    unsafe fn getControllerClassId(&self, class: *mut TUID) -> tresult {
        if class.is_null() {
            return kInvalidArgument;
        }
        *class = SINE_CONTROLLER_CID;
        kResultOk
    }

    unsafe fn setIoMode(&self, _: IoMode) -> tresult {
        kResultOk
    }

    unsafe fn getBusCount(&self, media: MediaType, direction: BusDirection) -> i32 {
        // Notes in; sound out, on a main output and a second one.
        let (events, input) = (media == MediaTypes_::kEvent as MediaType, direction == BusDirections_::kInput as BusDirection);
        match (events, input) {
            (true, true) => 1,
            (false, false) => 2,
            _ => 0,
        }
    }

    unsafe fn getBusInfo(&self, media: MediaType, direction: BusDirection, index: i32, bus: *mut BusInfo) -> tresult {
        if index < 0 || bus.is_null() || index >= self.getBusCount(media, direction) {
            return kInvalidArgument;
        }
        stereo_bus(bus, "Out", direction);
        if media == MediaTypes_::kEvent as MediaType {
            (*bus).mediaType = media;
            (*bus).channelCount = 1;
        }
        if index == 1 {
            (*bus).busType = BusTypes_::kAux as BusType;
            (*bus).flags = 0;
        }
        kResultOk
    }

    unsafe fn getRoutingInfo(&self, _: *mut RoutingInfo, _: *mut RoutingInfo) -> tresult {
        kNotImplemented
    }

    unsafe fn activateBus(&self, media: MediaType, direction: BusDirection, index: i32, state: TBool) -> tresult {
        if media == MediaTypes_::kAudio as MediaType && direction == BusDirections_::kOutput as BusDirection && index == 1 {
            self.aux.set(state != 0);
        }
        kResultOk
    }

    unsafe fn setActive(&self, _: TBool) -> tresult {
        kResultOk
    }

    unsafe fn setState(&self, _: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getState(&self, _: *mut IBStream) -> tresult {
        kResultOk
    }
}

impl IAudioProcessorTrait for Sine {
    unsafe fn setBusArrangements(&self, _: *mut SpeakerArrangement, _: i32, _: *mut SpeakerArrangement, _: i32) -> tresult {
        kResultFalse
    }

    unsafe fn getBusArrangement(&self, _: BusDirection, _: i32, arrangement: *mut SpeakerArrangement) -> tresult {
        if arrangement.is_null() {
            return kInvalidArgument;
        }
        *arrangement = SpeakerArr::kStereo;
        kResultOk
    }

    unsafe fn canProcessSampleSize(&self, size: i32) -> tresult {
        if size == SymbolicSampleSizes_::kSample32 { kResultOk } else { kResultFalse }
    }

    unsafe fn getLatencySamples(&self) -> u32 {
        0
    }

    unsafe fn setupProcessing(&self, setup: *mut ProcessSetup) -> tresult {
        if let Some(setup) = setup.as_ref() {
            self.rate.set(setup.sampleRate);
        }
        kResultOk
    }

    unsafe fn setProcessing(&self, _: TBool) -> tresult {
        kResultOk
    }

    unsafe fn process(&self, data: *mut ProcessData) -> tresult {
        let data = &*data;
        if let Some(events) = ComRef::from_raw(data.inputEvents) {
            for index in 0..events.getEventCount() {
                let mut event: Event = std::mem::zeroed();
                if events.getEvent(index, &mut event) != kResultOk {
                    continue;
                }
                if event.r#type == Event_::EventTypes_::kNoteOnEvent as u16 {
                    self.note.set(Some(event.__field0.noteOn.pitch));
                } else if event.r#type == Event_::EventTypes_::kNoteOffEvent as u16 && self.note.get() == Some(event.__field0.noteOff.pitch) {
                    self.note.set(None);
                }
            }
        }
        if data.numOutputs < 1 {
            return kResultOk;
        }
        let frames = data.numSamples as usize;
        let step = self.note.get().map(|pitch| 440.0 * 2f64.powf((f64::from(pitch) - 69.0) / 12.0) / self.rate.get());
        let start = self.phase.get();
        // With its second output listened to, the instrument splits its sound
        // between the two: a host that mixes only the first hears half of it.
        let split = self.aux.get() && data.numOutputs >= 2;
        let level = if split { 0.15 } else { 0.3 };
        for bus in 0..if split { 2 } else { 1 } {
            let output = &*data.outputs.add(bus);
            for channel in 0..output.numChannels as usize {
                let to = std::slice::from_raw_parts_mut(*output.__field0.channelBuffers32.add(channel), frames);
                let mut phase = start;
                for out in to {
                    *out = step.map_or(0.0, |_| level * (std::f64::consts::TAU * phase).sin() as f32);
                    phase += step.unwrap_or(0.0);
                }
            }
        }
        self.phase.set(step.map_or(0.0, |step| (start + step * frames as f64).fract()));
        kResultOk
    }

    unsafe fn getTailSamples(&self) -> u32 {
        0
    }
}

struct SineController;

impl Class for SineController {
    type Interfaces = (IEditController,);
}

impl IPluginBaseTrait for SineController {
    unsafe fn initialize(&self, _: *mut FUnknown) -> tresult {
        kResultOk
    }

    unsafe fn terminate(&self) -> tresult {
        kResultOk
    }
}

impl IEditControllerTrait for SineController {
    unsafe fn setComponentState(&self, _: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn setState(&self, _: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getState(&self, _: *mut IBStream) -> tresult {
        kResultOk
    }

    unsafe fn getParameterCount(&self) -> i32 {
        0
    }

    unsafe fn getParameterInfo(&self, _: i32, _: *mut ParameterInfo) -> tresult {
        kInvalidArgument
    }

    unsafe fn getParamStringByValue(&self, _: u32, _: f64, _: *mut String128) -> tresult {
        kInvalidArgument
    }

    unsafe fn getParamValueByString(&self, _: u32, _: *mut TChar, _: *mut f64) -> tresult {
        kInvalidArgument
    }

    unsafe fn normalizedParamToPlain(&self, _: u32, value: f64) -> f64 {
        value
    }

    unsafe fn plainParamToNormalized(&self, _: u32, plain: f64) -> f64 {
        plain
    }

    unsafe fn getParamNormalized(&self, _: u32) -> f64 {
        0.0
    }

    unsafe fn setParamNormalized(&self, _: u32, _: f64) -> tresult {
        kInvalidArgument
    }

    unsafe fn setComponentHandler(&self, _: *mut IComponentHandler) -> tresult {
        kResultOk
    }

    unsafe fn createView(&self, _: *const c_char) -> *mut IPlugView {
        null_mut()
    }
}

// ----- what the file says it holds -------------------------------------------------------

struct Factory;

impl Class for Factory {
    type Interfaces = (IPluginFactory2,);
}

/// Class id, category, name and kind of each class of the file.
const CLASSES: [(TUID, &str, &str, &str); 4] = [
    (GAIN_CID, "Audio Module Class", "Gain VST3", "Fx"),
    (SINE_CID, "Audio Module Class", "Sinus VST3", "Instrument|Synth"),
    (SINE_CONTROLLER_CID, "Component Controller Class", "Sinus VST3", ""),
    (GAIN64_CID, "Audio Module Class", "Gain 64 VST3", "Fx"),
];

impl IPluginFactoryTrait for Factory {
    unsafe fn getFactoryInfo(&self, info: *mut PFactoryInfo) -> tresult {
        let Some(info) = info.as_mut() else { return kInvalidArgument };
        copy8("OpenGarageMusic", &mut info.vendor);
        copy8("https://opengaragemusic.example", &mut info.url);
        copy8("", &mut info.email);
        info.flags = PFactoryInfo_::FactoryFlags_::kUnicode;
        kResultOk
    }

    unsafe fn countClasses(&self) -> i32 {
        CLASSES.len() as i32
    }

    unsafe fn getClassInfo(&self, index: i32, info: *mut PClassInfo) -> tresult {
        let (Some(class), Some(info)) = (usize::try_from(index).ok().and_then(|i| CLASSES.get(i)), info.as_mut()) else { return kInvalidArgument };
        info.cid = class.0;
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances;
        copy8(class.1, &mut info.category);
        copy8(class.2, &mut info.name);
        kResultOk
    }

    unsafe fn createInstance(&self, cid: FIDString, iid: FIDString, obj: *mut *mut c_void) -> tresult {
        if cid.is_null() || iid.is_null() || obj.is_null() {
            return kInvalidArgument;
        }
        let made: Option<ComPtr<FUnknown>> = match *cid.cast::<TUID>() {
            GAIN_CID => ComWrapper::new(Gain::new(false)).to_com_ptr::<FUnknown>(),
            GAIN64_CID => ComWrapper::new(Gain::new(true)).to_com_ptr::<FUnknown>(),
            SINE_CID => {
                ComWrapper::new(Sine { rate: Cell::new(48_000.0), note: Cell::new(None), phase: Cell::new(0.0), aux: Cell::new(false) }).to_com_ptr::<FUnknown>()
            }
            SINE_CONTROLLER_CID => ComWrapper::new(SineController).to_com_ptr::<FUnknown>(),
            _ => None,
        };
        let Some(made) = made else { return kInvalidArgument };
        let pointer = made.as_ptr();
        ((*(*pointer).vtbl).queryInterface)(pointer, iid.cast(), obj)
    }
}

impl IPluginFactory2Trait for Factory {
    unsafe fn getClassInfo2(&self, index: i32, info: *mut PClassInfo2) -> tresult {
        let (Some(class), Some(info)) = (usize::try_from(index).ok().and_then(|i| CLASSES.get(i)), info.as_mut()) else { return kInvalidArgument };
        info.cid = class.0;
        info.cardinality = PClassInfo_::ClassCardinality_::kManyInstances;
        copy8(class.1, &mut info.category);
        copy8(class.2, &mut info.name);
        info.classFlags = 0;
        copy8(class.3, &mut info.subCategories);
        copy8("OpenGarageMusic", &mut info.vendor);
        copy8("1.0", &mut info.version);
        copy8("VST 3.8", &mut info.sdkVersion);
        kResultOk
    }
}

#[no_mangle]
extern "system" fn InitDll() -> bool {
    true
}

#[no_mangle]
extern "system" fn ExitDll() -> bool {
    true
}

/// The function a VST3 host looks for.
#[no_mangle]
extern "system" fn GetPluginFactory() -> *mut IPluginFactory {
    ComWrapper::new(Factory).to_com_ptr::<IPluginFactory>().map_or(null_mut(), ComPtr::into_raw)
}
