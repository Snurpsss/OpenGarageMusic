//! Connects the engine to the sound cards and to MIDI keyboards.
//!
//! One output, the main one, sets the pace: its callback asks the engine for
//! sound. Tracks sent to another output are mixed apart by the engine and
//! handed to that output through a queue. An output or an input whose sample
//! rate is not the engine's is converted on the way.

use crate::i18n::tr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use og_engine::{Engine, MonitorFeed, OutFeed, Project};

const RING: usize = 8192;

/// What the input callback shares with the UI.
pub struct InputState {
    pub sample_rate: u32,
    /// The most recent samples (mono), for the tuner and the level meter.
    ring: Vec<f32>,
    ring_pos: usize,
    pub level: f32,
}

impl InputState {
    fn push(&mut self, sample: f32) {
        self.ring[self.ring_pos] = sample;
        self.ring_pos = (self.ring_pos + 1) % RING;
        self.level = self.level.max(sample.abs());
    }
}

/// Samples in one chunk of a take.
const CHUNK: usize = 1024;
/// Chunks lent to the input callback: some ten seconds, the longest the
/// window may stand still before a take loses sound.
const CHUNKS: usize = 1024;
/// Two hours at 48 kHz; beyond that a take simply stops growing.
const LONGEST_TAKE: usize = 48_000 * 7200;

/// The input callback's end of a take. The sound goes to the application in
/// chunks the application lends, so that the callback neither allocates nor
/// waits: it fills a chunk and hands it over, chunk by chunk.
struct Taker {
    on: Arc<AtomicBool>,
    empties: Receiver<Vec<f32>>,
    fulls: SyncSender<Vec<f32>>,
    /// Samples that found no chunk to go in.
    lost: Arc<AtomicUsize>,
}

impl Taker {
    /// Keeps the samples of one callback while a take is on.
    fn take(&self, samples: impl Iterator<Item = f32>) {
        if !self.on.load(Ordering::Relaxed) {
            return;
        }
        let mut samples = samples.peekable();
        while samples.peek().is_some() {
            let Ok(mut chunk) = self.empties.try_recv() else {
                self.lost.fetch_add(samples.count(), Ordering::Relaxed);
                return;
            };
            chunk.clear();
            chunk.extend(samples.by_ref().take(chunk.capacity()));
            if self.fulls.try_send(chunk).is_err() {
                return;
            }
        }
    }
}

/// The application's end of a take: it lends the chunks and gathers the
/// sound they bring back.
struct Capture {
    on: Arc<AtomicBool>,
    empties: SyncSender<Vec<f32>>,
    fulls: Receiver<Vec<f32>>,
    lost: Arc<AtomicUsize>,
    samples: Vec<f32>,
}

impl Capture {
    /// Both ends, no chunk lent yet.
    fn pair() -> (Self, Taker) {
        let (lend, empties) = sync_channel(CHUNKS);
        let (fulls, gather) = sync_channel(CHUNKS);
        let (on, lost) = (Arc::new(AtomicBool::new(false)), Arc::new(AtomicUsize::new(0)));
        let capture = Self { on: Arc::clone(&on), empties: lend, fulls: gather, lost: Arc::clone(&lost), samples: Vec::new() };
        (capture, Taker { on, empties, fulls, lost })
    }

    /// Gathers the chunks that came back and lends them again.
    fn gather(&mut self) {
        while let Ok(mut chunk) = self.fulls.try_recv() {
            let room = LONGEST_TAKE.saturating_sub(self.samples.len());
            self.samples.extend_from_slice(&chunk[..chunk.len().min(room)]);
            chunk.clear();
            let _ = self.empties.try_send(chunk);
        }
    }
}

/// An output other than the main one, playing what the engine mixes for it.
struct Extra {
    name: String,
    feed: Arc<OutFeed>,
    _stream: cpal::Stream,
}

pub struct Audio {
    pub engine: Arc<Mutex<Engine>>,
    /// Kept alive for as long as sound should play.
    _stream: Option<cpal::Stream>,
    /// Name of the main output in use; empty for the system's default one.
    pub output_name: String,
    extras: Vec<Extra>,
    pub sample_rate: u32,
    pub error: Option<String>,
    pub input: Arc<Mutex<InputState>>,
    capture: Capture,
    /// Input samples on their way to a monitored track, when `monitoring` is set.
    pub feed: Arc<MonitorFeed>,
    pub monitoring: Arc<AtomicBool>,
    input_stream: Option<cpal::Stream>,
    /// Name of the input that is open; empty for the system's default one.
    input_name: String,
    /// Channel of the input to take (1 is the first); 0 mixes them all. Set
    /// before `ensure_input`.
    pub input_channel: u16,
    /// Channel the open input was opened with.
    open_channel: u16,
    _midi: Option<midir::MidiInputConnection<()>>,
    pub midi_name: Option<String>,
    /// Track that notes from the MIDI keyboard play.
    pub midi_track: Arc<AtomicUsize>,
}

/// Names of the audio outputs of this computer. The tests see none.
pub fn output_devices() -> Vec<String> {
    if cfg!(test) {
        return Vec::new();
    }
    cpal::default_host().output_devices().map(|devices| devices.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

/// Names of the audio inputs of this computer. The tests see none.
pub fn input_devices() -> Vec<String> {
    if cfg!(test) {
        return Vec::new();
    }
    cpal::default_host().input_devices().map(|devices| devices.filter_map(|d| d.name().ok()).collect()).unwrap_or_default()
}

/// The output of that name; the system's default one for an empty name.
fn find_output(name: &str) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let found = if name.is_empty() {
        host.default_output_device()
    } else {
        host.output_devices().ok().and_then(|mut devices| devices.find(|d| d.name().is_ok_and(|n| n == name)))
    };
    found.ok_or_else(|| tr("no audio output found").to_owned())
}

fn find_input(name: &str) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let found = if name.is_empty() {
        host.default_input_device()
    } else {
        host.input_devices().ok().and_then(|mut devices| devices.find(|d| d.name().is_ok_and(|n| n == name)))
    };
    found.ok_or_else(|| tr("no audio input found").to_owned())
}

/// How many channels an input has (2 for a stereo card); the tests see 2.
pub fn input_channel_count(name: &str) -> u16 {
    if cfg!(test) {
        return 2;
    }
    find_input(name).ok().and_then(|device| device.default_input_config().ok()).map_or(0, |config| config.channels())
}

/// What is kept of one frame of input: one channel, or the mean of them all.
fn pick(frame: impl ExactSizeIterator<Item = f32>, channel: usize) -> f32 {
    let count = frame.len().max(1);
    let mut frame = frame;
    if (1..=count).contains(&channel) {
        frame.nth(channel - 1).unwrap_or(0.0)
    } else {
        frame.sum::<f32>() / count as f32
    }
}

fn output_config(device: &cpal::Device) -> Result<cpal::StreamConfig, String> {
    let supported = device.default_output_config().map_err(|e| e.to_string())?;
    if supported.sample_format() != cpal::SampleFormat::F32 {
        return Err(tf!("unsupported output format ({})", format!("{:?}", supported.sample_format())));
    }
    Ok(supported.into())
}

/// Turns stereo frames at one sample rate into frames at another, a frame at a
/// time, by drawing a line between neighbours.
struct Converter {
    /// Source frames per output frame.
    step: f64,
    position: f64,
    previous: (f32, f32),
    current: (f32, f32),
}

impl Converter {
    fn new(from: u32, to: u32) -> Self {
        Self { step: f64::from(from.max(1)) / f64::from(to.max(1)), position: 1.0, previous: (0.0, 0.0), current: (0.0, 0.0) }
    }

    /// The next output frame; `next` gives source frames as they are needed,
    /// or nothing when there is none to give, which reads as silence.
    fn frame(&mut self, mut next: impl FnMut() -> Option<(f32, f32)>) -> (f32, f32) {
        self.position += self.step;
        while self.position >= 1.0 {
            self.position -= 1.0;
            self.previous = self.current;
            self.current = next().unwrap_or((0.0, 0.0));
        }
        let t = self.position as f32;
        (self.previous.0 + (self.current.0 - self.previous.0) * t, self.previous.1 + (self.current.1 - self.previous.1) * t)
    }
}

fn write_frame(frame: &mut [f32], (left, right): (f32, f32)) {
    match frame {
        [only] => *only = 0.5 * (left + right),
        [l, r, rest @ ..] => {
            *l = left;
            *r = right;
            rest.fill(0.0);
        }
        [] => {}
    }
}

/// Starts the stream that sets the pace: its callback asks the engine for sound.
fn main_stream(engine: &Arc<Mutex<Engine>>, engine_rate: u32, device: &cpal::Device) -> Result<cpal::Stream, String> {
    let config = output_config(device)?;
    let channels = usize::from(config.channels.max(1));
    let same_rate = config.sample_rate.0 == engine_rate;
    let shared = Arc::clone(engine);
    let mut converter = Converter::new(engine_rate, config.sample_rate.0);
    // Sound asked of the engine ahead of what the card has taken, when rates differ.
    let mut ahead = vec![0.0f32; 512];
    let mut taken = ahead.len();
    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let Ok(mut engine) = shared.lock() else {
                    data.fill(0.0);
                    return;
                };
                if same_rate {
                    engine.process(data, channels);
                    return;
                }
                for frame in data.chunks_mut(channels) {
                    let value = converter.frame(|| {
                        if taken >= ahead.len() {
                            engine.process(&mut ahead, 2);
                            taken = 0;
                        }
                        taken += 2;
                        Some((ahead[taken - 2], ahead[taken - 1]))
                    });
                    write_frame(frame, value);
                }
            },
            |err| eprintln!("audio: {err}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}

/// Starts playback on the default output. Without a usable output the engine
/// still exists, so editing and WAV export keep working.
pub fn start(project: Project) -> Audio {
    let mut audio = match open(&project) {
        Ok(audio) => audio,
        Err(error) => Audio { error: Some(error), ..silent(project) },
    };
    audio.connect_midi();
    audio
}

/// An engine that is not connected to any device.
pub fn silent(project: Project) -> Audio {
    Audio {
        engine: Arc::new(Mutex::new(Engine::new(project, 48_000.0))),
        _stream: None,
        output_name: String::new(),
        extras: Vec::new(),
        sample_rate: 48_000,
        error: None,
        input: Arc::new(Mutex::new(InputState {
            sample_rate: 48_000,
            ring: vec![0.0; RING],
            ring_pos: 0,
            level: 0.0,
        })),
        capture: Capture::pair().0,
        feed: Arc::new(MonitorFeed::new()),
        monitoring: Arc::new(AtomicBool::new(false)),
        input_stream: None,
        input_name: String::new(),
        input_channel: 0,
        open_channel: 0,
        _midi: None,
        midi_name: None,
        midi_track: Arc::new(AtomicUsize::new(0)),
    }
}

fn open(project: &Project) -> Result<Audio, String> {
    let device = find_output("")?;
    // The engine runs at the rate of the output it starts on.
    let sample_rate = output_config(&device)?.sample_rate.0;
    let engine = Arc::new(Mutex::new(Engine::new(project.clone(), sample_rate as f32)));
    let stream = main_stream(&engine, sample_rate, &device)?;
    Ok(Audio { engine, _stream: Some(stream), sample_rate, ..silent(Project::empty()) })
}

impl Audio {
    /// Moves the main output to another device, by name; an empty name is the
    /// system's default one. The output in use stays when the other cannot start.
    pub fn set_output(&mut self, name: &str) -> Result<(), String> {
        let device = find_output(name)?;
        let stream = main_stream(&self.engine, self.sample_rate, &device)?;
        // The old stream stops when it is replaced.
        self._stream = Some(stream);
        self.output_name = name.to_owned();
        self.error = None;
        // A track cannot be sent apart to the output that plays the main mix.
        let kept: Vec<String> = self.extras.iter().map(|e| e.name.clone()).filter(|n| n != name).collect();
        self.set_extra_outputs(&kept);
        Ok(())
    }

    /// Opens the outputs that tracks are sent to, closes those no track uses
    /// any more, and tells the engine. Returns the names that could not be opened.
    pub fn set_extra_outputs(&mut self, wanted: &[String]) -> Vec<String> {
        let wanted: Vec<&String> = wanted.iter().filter(|name| !name.is_empty() && **name != self.output_name).collect();
        self.extras.retain(|extra| wanted.contains(&&extra.name));
        let mut failed = Vec::new();
        for name in wanted {
            if self.extras.iter().any(|extra| extra.name == *name) {
                continue;
            }
            match self.extra_stream(name) {
                Ok(extra) => self.extras.push(extra),
                Err(_) => failed.push(name.clone()),
            }
        }
        let routes = self.extras.iter().map(|extra| (extra.name.clone(), Arc::clone(&extra.feed))).collect();
        if let Ok(mut engine) = self.engine.lock() {
            engine.set_routes(routes);
        }
        failed
    }

    fn extra_stream(&self, name: &str) -> Result<Extra, String> {
        let device = find_output(name)?;
        let config = output_config(&device)?;
        let channels = usize::from(config.channels.max(1));
        let feed = Arc::new(OutFeed::new(self.sample_rate));
        let queue = Arc::clone(&feed);
        let mut converter = Converter::new(self.sample_rate, config.sample_rate.0);
        // Nothing is played until a little sound is in hand, so that the two
        // cards' slightly different speeds do not make it stutter at once.
        let cushion = self.sample_rate as usize / 25 * 2;
        let mut started = false;
        let stream = device
            .build_output_stream(
                &config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let Ok(mut queue) = queue.queue.lock() else {
                        data.fill(0.0);
                        return;
                    };
                    started = (started && queue.len() >= 2) || queue.len() >= cushion;
                    if !started {
                        data.fill(0.0);
                        return;
                    }
                    for frame in data.chunks_mut(channels) {
                        let value = converter.frame(|| Some((queue.pop_front()?, queue.pop_front()?)));
                        write_frame(frame, value);
                    }
                },
                |err| eprintln!("audio: {err}"),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Extra { name: name.to_owned(), feed, _stream: stream })
    }

    pub fn has_input(&self) -> bool {
        self.input_stream.is_some()
    }

    /// Opens a microphone or line input, by name; an empty name is the system's
    /// default one. An input that is already open is left as it is.
    pub fn ensure_input(&mut self, name: &str) -> Result<(), String> {
        if self.input_stream.is_some() && self.input_name == name && self.open_channel == self.input_channel {
            return Ok(());
        }
        let channel = usize::from(self.input_channel);
        let device = find_input(name)?;
        let supported = device.default_input_config().map_err(|e| e.to_string())?;
        let format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        let channels = usize::from(config.channels.max(1));
        // The input that was open, if any, closes first.
        self.input_stream = None;
        if let Ok(mut state) = self.input.lock() {
            state.sample_rate = config.sample_rate.0;
        }
        self.feed.sample_rate.store(config.sample_rate.0, Ordering::Relaxed);
        let (capture, taker) = Capture::pair();
        self.capture = capture;
        let on_error = |err| eprintln!("audio input: {err}");
        // Every format ends up as mono f32 in the shared state.
        let stream = match format {
            cpal::SampleFormat::F32 => {
                let shared = Arc::clone(&self.input);
                let (feed, monitoring, taker) = (Arc::clone(&self.feed), Arc::clone(&self.monitoring), taker);
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mono = data.chunks(channels).map(|frame| pick(frame.iter().copied(), channel));
                        if monitoring.load(Ordering::Relaxed) {
                            feed.push(mono.clone());
                        }
                        taker.take(mono.clone());
                        // The tuner and the meter miss a callback rather than hold it up.
                        if let Ok(mut state) = shared.try_lock() {
                            for sample in mono {
                                state.push(sample);
                            }
                        }
                    },
                    on_error,
                    None,
                )
            }
            cpal::SampleFormat::I16 => {
                let shared = Arc::clone(&self.input);
                let (feed, monitoring, taker) = (Arc::clone(&self.feed), Arc::clone(&self.monitoring), taker);
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mono = data.chunks(channels).map(|frame| pick(frame.iter().map(|s| f32::from(*s) / 32768.0), channel));
                        if monitoring.load(Ordering::Relaxed) {
                            feed.push(mono.clone());
                        }
                        taker.take(mono.clone());
                        // The tuner and the meter miss a callback rather than hold it up.
                        if let Ok(mut state) = shared.try_lock() {
                            for sample in mono {
                                state.push(sample);
                            }
                        }
                    },
                    on_error,
                    None,
                )
            }
            other => return Err(tf!("unsupported input format ({})", format!("{other:?}"))),
        }
        .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.input_stream = Some(stream);
        self.input_name = name.to_owned();
        self.open_channel = self.input_channel;
        Ok(())
    }

    /// Stands in for the sound card in tests: a finished take.
    #[cfg(test)]
    pub fn inject_capture(&mut self, samples: Vec<f32>) {
        self.capture.samples = samples;
    }

    /// Starts keeping what comes from the input.
    pub fn start_capture(&mut self) {
        // Whatever a previous take left on its way is dropped.
        while self.capture.fulls.try_recv().is_ok() {}
        self.capture.samples = Vec::new();
        self.capture.lost.store(0, Ordering::Relaxed);
        // The chunks are lent once and go round from then on.
        while self.capture.empties.try_send(Vec::with_capacity(CHUNK)).is_ok() {}
        self.capture.on.store(true, Ordering::Relaxed);
    }

    /// Gathers the sound of the take that is on; to be called often, as the
    /// callback has only so many chunks to fill.
    pub fn poll_capture(&mut self) {
        if self.capture.on.load(Ordering::Relaxed) {
            self.capture.gather();
        }
    }

    /// Ends the take and returns its samples (mono), the sample rate and how
    /// many samples were lost for want of a chunk.
    pub fn stop_capture(&mut self) -> (Vec<f32>, u32, usize) {
        self.capture.on.store(false, Ordering::Relaxed);
        self.capture.gather();
        let rate = self.input.lock().map_or(48_000, |state| state.sample_rate);
        (std::mem::take(&mut self.capture.samples), rate, self.capture.lost.swap(0, Ordering::Relaxed))
    }

    /// The latest input samples in order, the sample rate, and the peak level
    /// since the last call.
    pub fn recent_input(&self) -> (Vec<f32>, u32, f32) {
        match self.input.lock() {
            Ok(mut state) => {
                let mut out = Vec::with_capacity(RING);
                out.extend_from_slice(&state.ring[state.ring_pos..]);
                out.extend_from_slice(&state.ring[..state.ring_pos]);
                (out, state.sample_rate, std::mem::take(&mut state.level))
            }
            Err(_) => (Vec::new(), 48_000, 0.0),
        }
    }

    /// Listens to the first MIDI input port, if there is one.
    /// Lets go of the MIDI input, for another use of it (a pedal module).
    pub fn disconnect_midi(&mut self) {
        self._midi = None;
        self.midi_name = None;
    }

    pub fn connect_midi(&mut self) {
        let Ok(mut input) = midir::MidiInput::new("OpenGarageMusic") else { return };
        input.ignore(midir::Ignore::All);
        let ports = input.ports();
        let Some(port) = ports.first() else { return };
        let name = input.port_name(port).unwrap_or_else(|_| tr("MIDI keyboard").to_owned());
        let engine = Arc::clone(&self.engine);
        let track = Arc::clone(&self.midi_track);
        let connection = input.connect(
            port,
            "opengarage-in",
            move |_stamp, message, _| {
                if message.len() < 3 {
                    return;
                }
                let (kind, pitch, velocity) = (message[0] & 0xF0, message[1] & 0x7F, message[2] & 0x7F);
                let Ok(mut engine) = engine.lock() else { return };
                let track = track.load(Ordering::Relaxed);
                if kind == 0x90 && velocity > 0 {
                    engine.live_note_on(track, pitch, velocity);
                } else if kind == 0x80 || kind == 0x90 {
                    engine.live_note_off(track, pitch);
                }
            },
            (),
        );
        if let Ok(connection) = connection {
            self._midi = Some(connection);
            self.midi_name = Some(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_is_converted_between_sample_rates() {
        // The same rate: every frame comes out as it went in, one frame late.
        let mut same = Converter::new(48_000, 48_000);
        let mut source = (0..).map(|i| (i as f32, -(i as f32)));
        let out: Vec<(f32, f32)> = (0..5).map(|_| same.frame(|| source.next())).collect();
        assert_eq!(out, vec![(0.0, 0.0), (1.0, -1.0), (2.0, -2.0), (3.0, -3.0), (4.0, -4.0)]);
        // To a rate twice as high: twice as many frames, half-way values in between.
        let mut up = Converter::new(24_000, 48_000);
        let mut source = (0..).map(|i| (i as f32, 0.0));
        let out: Vec<f32> = (0..8).map(|_| up.frame(|| source.next()).0).collect();
        assert!(out.windows(2).all(|w| (w[1] - w[0] - 0.5).abs() < 1e-6 || w[0] == 0.0), "{out:?}");
        // To a rate half as high: every other frame.
        let mut down = Converter::new(48_000, 24_000);
        let mut source = (0..).map(|i| (i as f32, 0.0));
        let out: Vec<f32> = (0..4).map(|_| down.frame(|| source.next()).0).collect();
        assert!(out.windows(2).all(|w| (w[1] - w[0] - 2.0).abs() < 1e-6), "{out:?}");
        // A source that runs dry gives silence, not a panic.
        let mut dry = Converter::new(48_000, 44_100);
        assert_eq!(dry.frame(|| None), (0.0, 0.0));
        // One channel of an input is taken, or the mean of them all.
        assert_eq!(pick([0.2f32, 0.6].into_iter(), 0), 0.4);
        assert_eq!(pick([0.2f32, 0.6].into_iter(), 1), 0.2);
        assert_eq!(pick([0.2f32, 0.6].into_iter(), 2), 0.6);
        assert_eq!(pick([0.2f32, 0.6].into_iter(), 3), 0.4, "a channel the card does not have");
        // Frames are written for one, two or more channels.
        let mut mono = [9.0f32];
        write_frame(&mut mono, (0.2, 0.4));
        assert!((mono[0] - 0.3).abs() < 1e-6);
        let mut six = [9.0f32; 6];
        write_frame(&mut six, (0.2, 0.4));
        assert_eq!(six, [0.2, 0.4, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_take_goes_from_the_callback_to_the_application_in_lent_chunks() {
        let (mut capture, taker) = Capture::pair();
        // Nothing is kept while no take is on.
        taker.take([0.5f32; 100].into_iter());
        capture.gather();
        assert!(capture.samples.is_empty());
        // A take on: the samples arrive in the order given, whatever the callback sizes.
        capture.on.store(true, Ordering::Relaxed);
        while capture.empties.try_send(Vec::with_capacity(CHUNK)).is_ok() {}
        taker.take((0..3000).map(|i| i as f32));
        taker.take((3000..3100).map(|i| i as f32));
        capture.gather();
        assert_eq!(capture.samples.len(), 3100);
        assert!(capture.samples.iter().enumerate().all(|(i, s)| *s == i as f32));
        // The chunks go round: as many takes as wanted fit in the same few chunks.
        for _ in 0..10 {
            taker.take((0..CHUNK * CHUNKS / 2).map(|_| 1.0));
            capture.gather();
        }
        assert_eq!(capture.samples.len(), 3100 + 10 * CHUNK * CHUNKS / 2);
        assert_eq!(capture.lost.load(Ordering::Relaxed), 0);
        // When the application does not gather in time, the surplus is lost and counted.
        taker.take((0..CHUNK * CHUNKS + 5).map(|_| 1.0));
        assert_eq!(capture.lost.load(Ordering::Relaxed), 5);
        capture.gather();
        assert_eq!(capture.samples.len(), 3100 + 10 * CHUNK * CHUNKS / 2 + CHUNK * CHUNKS);
    }
}
