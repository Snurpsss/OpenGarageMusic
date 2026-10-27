//! Sequencer and mixer. `Engine::process` is what the audio callback calls.

use std::collections::{HashMap, VecDeque};
use std::f32::consts::{FRAC_PI_4, TAU};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::capture::Captures;
use crate::fx::{MasterRt, TrackFxRt};
use crate::model::{curve_linear, Arp, ArpMode, Content, Note, Project, Slide, Sound, Track, Vibrato, TEMPO_MAX, TEMPO_MIN};
use crate::synth::{Fonts, SoundRt};

/// Events are scheduled at this granularity (about 1.3 ms at 48 kHz).
pub const BLOCK: usize = 64;

/// Decoded audio of a clip: interleaved samples, one or two channels.
#[derive(Debug, PartialEq)]
pub struct ClipData {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

impl ClipData {
    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels.max(1))
    }

    pub fn seconds(&self) -> f64 {
        self.frames() as f64 / f64::from(self.sample_rate.max(1))
    }
}

/// Live input samples (mono) on their way to a monitored track.
pub struct MonitorFeed {
    pub queue: Mutex<VecDeque<f32>>,
    pub sample_rate: AtomicU32,
}

impl MonitorFeed {
    pub fn new() -> Self {
        Self { queue: Mutex::new(VecDeque::new()), sample_rate: AtomicU32::new(48_000) }
    }

    /// Adds input samples, dropping the oldest when nobody reads them fast enough.
    pub fn push(&self, samples: impl Iterator<Item = f32>) {
        if let Ok(mut queue) = self.queue.lock() {
            queue.extend(samples);
            let limit = self.sample_rate.load(Ordering::Relaxed) as usize / 8;
            let excess = queue.len().saturating_sub(limit.max(256));
            queue.drain(..excess);
        }
    }
}

impl Default for MonitorFeed {
    fn default() -> Self {
        Self::new()
    }
}

/// Sound on its way to an audio output other than the main one: stereo
/// frames (left, right, left, right…) at the engine's sample rate.
pub struct OutFeed {
    pub queue: Mutex<VecDeque<f32>>,
    pub sample_rate: AtomicU32,
}

impl OutFeed {
    pub fn new(sample_rate: u32) -> Self {
        Self { queue: Mutex::new(VecDeque::new()), sample_rate: AtomicU32::new(sample_rate) }
    }

    /// Adds frames, dropping the oldest when the output does not take them
    /// fast enough (two sound cards never run at exactly the same speed).
    fn push(&self, left: &[f32], right: &[f32]) {
        if let Ok(mut queue) = self.queue.try_lock() {
            queue.extend(left.iter().zip(right).flat_map(|(l, r)| [*l, *r]));
            let limit = (self.sample_rate.load(Ordering::Relaxed) as usize / 4).max(2048) * 2;
            if queue.len() > limit {
                // Back to half of the room, on a frame boundary.
                let excess = (queue.len() - limit / 2) & !1;
                queue.drain(..excess);
            }
        }
    }
}

/// An output other than the main one, and the block being mixed for it.
struct Route {
    name: String,
    feed: Arc<OutFeed>,
    left: Vec<f32>,
    right: Vec<f32>,
}

/// A sound generator or effect that lives outside the engine (a plug-in).
pub trait ExternalUnit: Send {
    /// Effects replace `buf` (mono) with their output; instruments add to it.
    fn process(&mut self, buf: &mut [f32]);
    fn note_on(&mut self, pitch: u8, velocity: u8);
    fn note_off(&mut self, pitch: u8);
    fn set_param(&mut self, id: u32, value: f64);

    fn all_notes_off(&mut self) {
        for pitch in 0..128 {
            self.note_off(pitch);
        }
    }
}

/// Running plug-ins, by slot id.
pub type Units = HashMap<u64, Box<dyn ExternalUnit>>;

/// The audio of every clip of the project, by clip id.
pub type Clips = HashMap<u64, Arc<ClipData>>;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Ev {
    beat: f64,
    on: bool,
    pitch: u8,
    vel: u8,
}

struct TrackRt {
    /// The pitch movements of the notes of the track, by start.
    glides: Vec<Glide>,
    sound: SoundRt,
    fx: TrackFxRt,
    /// The effects as the automation curves set them for the block being
    /// computed; kept from one block to the next so as not to allocate.
    driven: crate::model::TrackFx,
    /// Every note of the track with loops, drummer and arpeggiator applied.
    events: Vec<Ev>,
    peak: f32,
    /// Keys held on a live keyboard while the arpeggiator is on: pitch, velocity.
    held: Vec<(u8, u8)>,
    arp_note: Option<u8>,
    arp_step: usize,
    /// Slot of the external instrument that plays this track's notes, if any.
    ext: Option<u64>,
}

/// Sends a note to the track's external instrument, or to its built-in sound.
fn send_note(rt: &mut TrackRt, units: &mut Units, on: bool, pitch: u8, velocity: u8) {
    match rt.ext.and_then(|id| units.get_mut(&id)) {
        Some(unit) if on => unit.note_on(pitch, velocity),
        Some(unit) => unit.note_off(pitch),
        None if on => rt.sound.note_on(pitch, velocity),
        None => rt.sound.note_off(pitch),
    }
}

/// A loop being previewed outside the project.
struct Audition {
    sound: SoundRt,
    events: Vec<Ev>,
    length: f64,
    pos: f64,
}

/// What was played while recording.
#[derive(Debug, PartialEq)]
pub struct Recorded {
    pub track: usize,
    /// Position of the playhead when recording started and stopped.
    pub start: f64,
    pub end: f64,
    /// Notes with absolute start beats.
    pub notes: Vec<Note>,
}

pub struct Engine {
    sample_rate: f32,
    project: Project,
    clips: Clips,
    fonts: Fonts,
    /// The captures and impulses the effects may refer to.
    captures: Captures,
    monitor: Option<Arc<MonitorFeed>>,
    /// Outputs other than the main one that tracks may play on.
    routes: Vec<Route>,
    monitor_frac: f64,
    monitor_prev: f32,
    monitor_cur: f32,
    /// A clock that never stops, for the live arpeggiator.
    free_beat: f64,
    units: Units,
    tracks: Vec<TrackRt>,
    master: MasterRt,
    playing: bool,
    /// Playhead in beats.
    position: f64,
    end_beat: f64,
    /// Beats of count-in left before the playhead starts moving.
    preroll: f64,
    preroll_total: f64,
    rec_track: Option<usize>,
    rec_start: f64,
    rec_open: Vec<(u8, f64, u8)>,
    rec_notes: Vec<Note>,
    audition: Option<Audition>,
    click_phase: f32,
    click_inc: f32,
    click_left: u32,
    click_len: u32,
    peak: f32,
    buf: [f32; BLOCK],
    mix_l: [f32; BLOCK],
    mix_r: [f32; BLOCK],
    echo_in: [f32; BLOCK],
    reverb_in: [f32; BLOCK],
}

type Abs = (f64, f64, u8, u8);

/// Which of `len` notes the arpeggiator plays at its `step`-th step.
fn arp_index(mode: ArpMode, step: usize, len: usize, k: f64) -> usize {
    match mode {
        ArpMode::Up => step % len,
        ArpMode::Down => len - 1 - step % len,
        ArpMode::UpDown if len > 1 => {
            let period = 2 * len - 2;
            let i = step % period;
            if i < len {
                i
            } else {
                period - i
            }
        }
        ArpMode::UpDown => 0,
        ArpMode::Random => ((k as i64).unsigned_abs().wrapping_mul(2_654_435_761) >> 7) as usize % len,
    }
}

/// A movement of the pitch of a note while it sounds: a bend, a slide into
/// or out of it, a vibrato.
#[derive(Clone, Debug, PartialEq)]
struct Glide {
    from: f64,
    to: f64,
    pitch: u8,
    /// Semitones away from the pitch, at beats; straight lines between them.
    points: Vec<(f64, f32)>,
    /// Depth of a vibrato in semitones, 0 for none.
    vibrato: f32,
}

impl Glide {
    /// The semitones away from the pitch at a beat, `seconds` into the note.
    fn offset(&self, beat: f64, seconds: f64) -> f32 {
        let mut value = self.points.first().map_or(0.0, |p| p.1);
        for pair in self.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if beat >= b.0 {
                value = b.1;
                continue;
            }
            if beat >= a.0 {
                let t = if b.0 > a.0 { ((beat - a.0) / (b.0 - a.0)) as f32 } else { 1.0 };
                value = a.1 + (b.1 - a.1) * t;
            }
            break;
        }
        if self.vibrato > 0.0 {
            value += self.vibrato * (seconds * 5.5 * f64::from(TAU)).sin() as f32;
        }
        value
    }
}

/// The pitch movements the articulation of a note asks for, between `on`
/// and `off`: a bend as written, else a slide to the next note on the string
/// (over the second half of the note), into the note from two frets away
/// (quickly), or out of it (over its last part).
fn glide_points(note: &Note, on: f64, off: f64, next: Option<&Note>) -> Vec<(f64, f32)> {
    let art = &note.articulation;
    if !art.bend.is_empty() {
        return art.bend.iter().map(|(at, semitones)| (on + (off - on) * f64::from(at.clamp(0.0, 1.0)), *semitones)).collect();
    }
    let length = off - on;
    match art.slide {
        Some(Slide::Legato | Slide::Shift) => match next {
            Some(next) => vec![(on, 0.0), (on + length * 0.5, 0.0), (off, f32::from(next.pitch) - f32::from(note.pitch))],
            None => Vec::new(),
        },
        Some(Slide::FromBelow) => vec![(on, -2.0), (on + length.min(0.5) * 0.5, 0.0)],
        Some(Slide::FromAbove) => vec![(on, 2.0), (on + length.min(0.5) * 0.5, 0.0)],
        Some(Slide::OutDown) => vec![(on + length * 0.6, 0.0), (off, -3.0)],
        Some(Slide::OutUp) => vec![(on + length * 0.6, 0.0), (off, 3.0)],
        None => Vec::new(),
    }
}

/// Replaces held chords with one note per step.
fn arpeggiate(notes: &[Abs], arp: &Arp) -> Vec<Abs> {
    let rate = arp.rate.clamp(0.0625, 4.0);
    let first = notes.iter().map(|n| n.0).fold(f64::INFINITY, f64::min);
    let last = notes.iter().map(|n| n.1).fold(0.0, f64::max);
    let mut out = Vec::new();
    if !first.is_finite() {
        return out;
    }
    let octaves = usize::from(arp.octaves.clamp(1, 4));
    let mut step = 0usize;
    let mut held: Vec<(u8, u8)> = Vec::new();
    let mut k = (first / rate).floor();
    while k * rate < last && out.len() < 200_000 {
        let t = k * rate;
        held.clear();
        held.extend(notes.iter().filter(|n| n.0 <= t + 1e-9 && t < n.1 - 1e-9).map(|n| (n.2, n.3)));
        held.sort_unstable();
        held.dedup_by_key(|h| h.0);
        if held.is_empty() {
            step = 0;
        } else {
            let len = held.len() * octaves;
            let index = arp_index(arp.mode, step, len, k);
            let (pitch, vel) = held[index % held.len()];
            let pitch = (usize::from(pitch) + 12 * (index / held.len())).min(127) as u8;
            out.push((t, t + rate * 0.9, pitch, vel));
            step += 1;
        }
        k += 1.0;
    }
    out
}

/// The notes of a track as events, with the loops, the drummer and the
/// arpeggiator applied, and the pitch movements their articulations ask for.
fn bake(track: &Track, project: &Project) -> (Vec<Ev>, Vec<Glide>) {
    if track.kind.is_audio() {
        return (Vec::new(), Vec::new());
    }
    let drums = track.sound.is_drums();
    let mut notes: Vec<Abs> = Vec::new();
    let mut glides: Vec<Glide> = Vec::new();
    for region in &track.regions {
        let cycle = region.cycle_len().min(region.length.max(0.0625));
        let generated;
        let source: &[Note] = match &region.content {
            Content::Midi(notes) => notes,
            Content::Drummer(params) => {
                generated = project.drummer_notes(params, region.start, cycle);
                &generated
            }
            Content::Audio(_) => continue,
        };
        let end = region.end();
        let repeats = ((region.length / cycle).ceil() as usize).clamp(1, 4096);
        let next = crate::model::next_on_string(source);
        // A note hammered on or pulled off is not struck: it sounds softer.
        let mut hammered = vec![false; source.len()];
        for (i, following) in next.iter().enumerate() {
            if let (true, Some(j)) = (source[i].articulation.hammer, following) {
                hammered[*j] = true;
            }
        }
        for k in 0..repeats {
            let base = region.start + k as f64 * cycle;
            for (i, note) in source.iter().enumerate() {
                let art = &note.articulation;
                if note.start < 0.0 || note.start >= cycle || art.dead {
                    continue;
                }
                // Written straight, played as the feel of the region says.
                let (from, to) = (crate::model::swung(&region.swing, note.start), crate::model::swung(&region.swing, note.start + note.length.max(0.01)));
                let length = (to - from).max(0.01);
                let on = base + from;
                if on >= end {
                    continue;
                }
                let mut off = (on + length).min(end);
                let mut velocity = f32::from(note.velocity);
                if art.staccato {
                    off = off.min(on + length * 0.5).max(on + 0.01);
                }
                if art.palm_mute {
                    off = off.min(on + length * 0.6).max(on + 0.01);
                    velocity *= 0.9;
                }
                if art.let_ring {
                    let until = next[i].map_or(on + 4.0, |j| base + crate::model::swung(&region.swing, source[j].start)).min(on + 4.0).min(end);
                    off = off.max(until);
                }
                if hammered[i] {
                    velocity *= 0.8;
                }
                let velocity = velocity.round().clamp(1.0, 127.0) as u8;
                // Tremolo picking strikes the note again and again; a trill
                // alternates it with the other note, at the speed written.
                let repeated = art.trill.map(|(other, step)| (Some(other), step)).or(art.tremolo.map(|step| (None, step)));
                if let Some((other, step)) = repeated.filter(|(_, step)| step.is_finite() && *step >= 0.03) {
                    let step = f64::from(step);
                    let mut at = on;
                    let mut k = 0usize;
                    while at < off - 1e-6 && k < 256 {
                        let pitch = if k % 2 == 1 { other.unwrap_or(note.pitch) } else { note.pitch };
                        // The strokes after the first are a little softer, as fingers play them.
                        let strength = if k == 0 { velocity } else { (f32::from(velocity) * 0.85).round().max(1.0) as u8 };
                        notes.push((at, (at + step).min(off), pitch, strength));
                        at += step;
                        k += 1;
                    }
                    continue;
                }
                notes.push((on, off, note.pitch, velocity));
                if drums {
                    continue;
                }
                let points = glide_points(note, on, off, next[i].map(|j| &source[j]));
                let vibrato = match art.vibrato {
                    Some(Vibrato::Slight) => 0.2,
                    Some(Vibrato::Wide) => 0.5,
                    None => 0.0,
                };
                if !points.is_empty() || vibrato > 0.0 {
                    let pitch = (i32::from(note.pitch) + project.transpose_at(on)).clamp(0, 127) as u8;
                    glides.push(Glide { from: on, to: off, pitch, points, vibrato });
                }
            }
        }
    }
    if track.arp.on && !drums {
        notes = arpeggiate(&notes, &track.arp);
        glides.clear();
    }
    glides.sort_by(|a, b| a.from.total_cmp(&b.from));
    let mut events = Vec::with_capacity(notes.len() * 2);
    for (on, off, pitch, vel) in notes {
        let pitch = if drums { pitch } else { (i32::from(pitch) + project.transpose_at(on)).clamp(0, 127) as u8 };
        events.push(Ev { beat: on, on: true, pitch, vel });
        events.push(Ev { beat: off, on: false, pitch, vel: 0 });
    }
    // Note-offs first, so a note that ends where the next one starts is retriggered.
    events.sort_by(|a, b| a.beat.total_cmp(&b.beat).then(a.on.cmp(&b.on)));
    (events, glides)
}

/// The notes a track plays, as they are heard: its loops, its drummer, its
/// arpeggiator, its feel and the transposition applied; each as (start,
/// end, pitch, velocity), in beats.
pub fn played_notes(track: &Track, project: &Project) -> Vec<(f64, f64, u8, u8)> {
    let (events, _) = bake(track, project);
    let mut sounding: Vec<(u8, f64, u8)> = Vec::new();
    let mut notes = Vec::new();
    for event in events {
        if event.on {
            sounding.push((event.pitch, event.beat, event.vel));
        } else if let Some(at) = sounding.iter().position(|(pitch, _, _)| *pitch == event.pitch) {
            let (pitch, start, velocity) = sounding.remove(at);
            notes.push((start, event.beat, pitch, velocity));
        }
    }
    notes.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
    notes
}

/// Adds the audio regions of `track` that overlap the block starting at `pos`.
fn render_audio(track: &Track, project: &Project, clips: &Clips, pos: f64, bpf: f64, sr: f32, out: &mut [f32]) {
    let block_end = pos + out.len() as f64 * bpf;
    for region in &track.regions {
        let Content::Audio(audio) = &region.content else { continue };
        if region.end() <= pos || region.start >= block_end {
            continue;
        }
        // The processed copy when there is one, the raw clip while it is being made.
        let Some(clip) = clips.get(&audio.key()).or_else(|| clips.get(&audio.clip)) else { continue };
        let frames = clip.frames();
        if frames < 2 {
            continue;
        }
        let channels = usize::from(clip.channels.max(1));
        let samples = &clip.samples;
        let read = |frame: usize| {
            if channels == 1 {
                samples[frame]
            } else {
                0.5 * (samples[frame * channels] + samples[frame * channels + 1])
            }
        };
        // Audio runs in real time: seconds since the region started, whatever the tempo.
        let region_sec = project.beat_to_sec(region.start);
        let t0 = project.beat_to_sec(pos) - region_sec;
        let cycle_sec = region.loop_len.map(|len| project.beat_to_sec(region.start + len) - region_sec);
        let dt = 1.0 / f64::from(sr);
        for (i, sample) in out.iter_mut().enumerate() {
            let beat = pos + i as f64 * bpf;
            if beat < region.start || beat >= region.end() {
                continue;
            }
            let mut t = (t0 + i as f64 * dt).max(0.0);
            if let Some(cycle) = cycle_sec.filter(|c| *c > 1e-6) {
                t %= cycle;
            }
            let mut src = (audio.offset + t) * f64::from(clip.sample_rate);
            if audio.reverse {
                src = (frames - 1) as f64 - src;
            }
            if src < 0.0 || src >= (frames - 1) as f64 {
                continue;
            }
            let i0 = src as usize;
            let frac = (src - i0 as f64) as f32;
            let a = read(i0);
            *sample += (a + (read(i0 + 1) - a) * frac) * audio.gain;
        }
    }
}

impl Engine {
    pub fn new(project: Project, sample_rate: f32) -> Self {
        let mut engine = Self {
            sample_rate,
            project: Project::empty(),
            clips: Clips::new(),
            fonts: Fonts::new(),
            captures: Captures::default(),
            monitor: None,
            monitor_frac: 0.0,
            monitor_prev: 0.0,
            monitor_cur: 0.0,
            free_beat: 0.0,
            units: Units::new(),
            tracks: Vec::new(),
            master: MasterRt::new(sample_rate),
            playing: false,
            position: 0.0,
            end_beat: 0.0,
            preroll: 0.0,
            preroll_total: 0.0,
            routes: Vec::new(),
            rec_track: None,
            rec_start: 0.0,
            rec_open: Vec::new(),
            rec_notes: Vec::new(),
            audition: None,
            click_phase: 0.0,
            click_inc: 0.0,
            click_left: 0,
            click_len: (0.03 * sample_rate) as u32,
            peak: 0.0,
            buf: [0.0; BLOCK],
            mix_l: [0.0; BLOCK],
            mix_r: [0.0; BLOCK],
            echo_in: [0.0; BLOCK],
            reverb_in: [0.0; BLOCK],
        };
        engine.set_project(project);
        engine
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    /// Replaces the document being played. Called by the UI after every edit.
    pub fn set_project(&mut self, mut project: Project) {
        project.tempo = project.tempo.clamp(TEMPO_MIN, TEMPO_MAX);
        project.beats_per_bar = project.beats_per_bar.clamp(1, 32);
        if !matches!(project.beat_unit, 2 | 4 | 8 | 16) {
            project.beat_unit = 4;
        }

        let sr = self.sample_rate;
        let same_shape = project.tracks.len() == self.tracks.len();
        if !same_shape {
            self.tracks = project
                .tracks
                .iter()
                .map(|t| TrackRt {
                    sound: SoundRt::new(&t.sound, sr, &self.fonts),
                    fx: TrackFxRt::new(sr),
                    driven: t.fx.clone(),
                    events: Vec::new(),
                    glides: Vec::new(),
                    peak: 0.0,
                    held: Vec::new(),
                    arp_note: None,
                    arp_step: 0,
                    ext: None,
                })
                .collect();
        }
        // Plug-ins that no track refers to any more are shut down here, on the
        // caller's thread rather than in the audio callback.
        self.units.retain(|id, _| {
            project.tracks.iter().any(|t| t.plugins.iter().chain(&t.instrument).any(|slot| slot.id == *id))
        });
        let globals_changed = !same_shape
            || project.transpose_points != self.project.transpose_points
            || project.bar_len() != self.project.bar_len();
        for (i, (rt, track)) in self.tracks.iter_mut().zip(&project.tracks).enumerate() {
            rt.sound.update(&track.sound, sr, &self.fonts);
            let ext = track.instrument.as_ref().filter(|slot| slot.on).map(|slot| slot.id);
            if ext != rt.ext {
                rt.sound.release_all();
                if let Some(unit) = rt.ext.and_then(|id| self.units.get_mut(&id)) {
                    unit.all_notes_off();
                }
                rt.ext = ext;
            }
            rt.fx.configure(&track.fx, &self.captures);
            if rt.driven.capture != track.fx.capture || rt.driven.cabinet != track.fx.cabinet {
                rt.driven.capture.clone_from(&track.fx.capture);
                rt.driven.cabinet.clone_from(&track.fx.cabinet);
            }
            // A drummer that follows another track depends on that track's notes.
            let follows = track.regions.iter().any(|r| matches!(&r.content, Content::Drummer(p) if p.follow > 0));
            let unchanged = !globals_changed
                && !follows
                && self.project.tracks.get(i).is_some_and(|old| {
                    old.regions == track.regions && old.arp == track.arp && old.kind == track.kind && old.sound.is_drums() == track.sound.is_drums()
                });
            if !unchanged {
                let (events, glides) = bake(track, &project);
                // A note removed while it sounds would never get its note-off.
                if events != rt.events && self.playing {
                    rt.sound.release_all();
                }
                rt.events = events;
                rt.glides = glides;
            }
        }
        self.master.configure(&project.master_fx, &project.reverb_impulse, &self.captures);
        self.end_beat = project.end_beat();
        self.project = project;
    }

    pub fn set_clips(&mut self, clips: Clips) {
        self.clips = clips;
    }

    /// Replaces the loaded SoundFonts; tracks that use one are rebuilt.
    pub fn set_fonts(&mut self, fonts: Fonts) {
        self.fonts = fonts;
        for (rt, track) in self.tracks.iter_mut().zip(&self.project.tracks) {
            if matches!(track.sound, Sound::Sampler(_)) {
                rt.sound = SoundRt::new(&track.sound, self.sample_rate, &self.fonts);
            }
        }
    }

    /// Replaces the captures and impulses loaded; the tracks and the master
    /// read theirs again.
    pub fn set_captures(&mut self, captures: Captures) {
        self.captures = captures;
        for (rt, track) in self.tracks.iter_mut().zip(&self.project.tracks) {
            rt.fx.reload();
            rt.fx.configure(&track.fx, &self.captures);
        }
        self.master.reload();
        self.master.configure(&self.project.master_fx, &self.project.reverb_impulse, &self.captures);
    }

    /// Hands a running plug-in to the engine, under the id of its slot.
    pub fn add_unit(&mut self, id: u64, unit: Box<dyn ExternalUnit>) {
        self.units.insert(id, unit);
    }

    pub fn has_unit(&self, id: u64) -> bool {
        self.units.contains_key(&id)
    }

    /// Lets go of a plug-in; the track it was on plays without it.
    pub fn remove_unit(&mut self, id: u64) {
        self.units.remove(&id);
    }

    pub fn unit_param(&mut self, id: u64, param: u32, value: f64) {
        if let Some(unit) = self.units.get_mut(&id) {
            unit.set_param(param, value);
        }
    }

    /// Sets the outputs, other than the main one, that tracks can be sent to,
    /// by name. A track that names an output that is not here plays in the
    /// main mix.
    pub fn set_routes(&mut self, routes: Vec<(String, Arc<OutFeed>)>) {
        self.routes = routes.into_iter().map(|(name, feed)| Route { name, feed, left: vec![0.0; BLOCK], right: vec![0.0; BLOCK] }).collect();
    }

    /// Names of the outputs set by `set_routes`.
    pub fn route_names(&self) -> Vec<String> {
        self.routes.iter().map(|route| route.name.clone()).collect()
    }

    /// Sets where monitored audio tracks take their live input from.
    pub fn set_monitor(&mut self, feed: Option<Arc<MonitorFeed>>) {
        self.monitor = feed;
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_recording(&self) -> bool {
        self.playing && self.rec_track.is_some()
    }

    /// Beats of count-in still to go.
    pub fn preroll(&self) -> f64 {
        self.preroll
    }

    pub fn position(&self) -> f64 {
        self.position
    }

    pub fn play(&mut self) {
        if self.position >= self.end_beat && !self.project.cycle_enabled {
            self.position = 0.0;
        }
        self.playing = true;
    }

    /// Starts playback and captures what is played on `track`. With `count_in`,
    /// one bar of metronome is heard first.
    pub fn record(&mut self, track: usize, count_in: bool) {
        self.rec_track = Some(track);
        self.rec_start = self.position;
        self.rec_open.clear();
        self.rec_notes.clear();
        self.preroll_total = self.project.bar_len();
        self.preroll = if count_in { self.preroll_total } else { 0.0 };
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.preroll = 0.0;
        let position = self.position;
        for (pitch, start, velocity) in std::mem::take(&mut self.rec_open) {
            self.rec_notes.push(Note { start, length: (position - start).max(0.1), pitch, velocity, ..Default::default() });
        }
        self.release_all();
    }

    /// The notes captured since `record`, once playback has stopped.
    pub fn take_recording(&mut self) -> Option<Recorded> {
        if self.playing {
            return None;
        }
        let track = self.rec_track.take()?;
        let mut notes = std::mem::take(&mut self.rec_notes);
        notes.sort_by(|a, b| a.start.total_cmp(&b.start));
        Some(Recorded { track, start: self.rec_start, end: self.position, notes })
    }

    pub fn set_position(&mut self, beat: f64) {
        self.position = beat.max(0.0);
        self.release_all();
    }

    /// Plays a note immediately, outside the sequence (on-screen, computer or MIDI keyboard).
    pub fn live_note_on(&mut self, track: usize, pitch: u8, velocity: u8) {
        let arpeggiated = self.project.tracks.get(track).is_some_and(|t| t.arp.on && !t.sound.is_drums());
        if let Some(rt) = self.tracks.get_mut(track) {
            if arpeggiated {
                // The arpeggiator plays the held keys one by one.
                if !rt.held.iter().any(|h| h.0 == pitch) {
                    rt.held.push((pitch, velocity));
                    rt.held.sort_unstable();
                }
            } else {
                send_note(rt, &mut self.units, true, pitch, velocity);
            }
        }
        if self.playing && self.preroll <= 0.0 && self.rec_track == Some(track) {
            self.rec_open.push((pitch, self.position, velocity));
        }
    }

    pub fn live_note_off(&mut self, track: usize, pitch: u8) {
        if let Some(rt) = self.tracks.get_mut(track) {
            rt.held.retain(|h| h.0 != pitch);
            send_note(rt, &mut self.units, false, pitch, 0);
        }
        if self.rec_track == Some(track) {
            if let Some(i) = self.rec_open.iter().position(|o| o.0 == pitch) {
                let (pitch, start, velocity) = self.rec_open.remove(i);
                // After a cycle jump the playhead is before the note: end it at the cycle end.
                let end = if self.position > start { self.position } else { self.project.cycle_end.max(start + 0.1) };
                self.rec_notes.push(Note { start, length: (end - start).max(0.05), pitch, velocity, string: None, ..Default::default() });
            }
        }
    }

    /// Plays `notes` in a loop with `sound`, on top of whatever else is going
    /// on, until `stop_audition`. Used to preview loops.
    pub fn audition(&mut self, sound: &Sound, notes: &[Note], length: f64) {
        let mut events = Vec::with_capacity(notes.len() * 2);
        for note in notes {
            events.push(Ev { beat: note.start, on: true, pitch: note.pitch, vel: note.velocity });
            events.push(Ev { beat: (note.start + note.length).min(length), on: false, pitch: note.pitch, vel: 0 });
        }
        events.sort_by(|a, b| a.beat.total_cmp(&b.beat).then(a.on.cmp(&b.on)));
        self.audition = Some(Audition { sound: SoundRt::new(sound, self.sample_rate, &self.fonts), events, length: length.max(0.25), pos: 0.0 });
    }

    pub fn stop_audition(&mut self) {
        self.audition = None;
    }

    pub fn is_auditioning(&self) -> bool {
        self.audition.is_some()
    }

    /// Highest output level since the last call, for the level meter.
    pub fn take_peak(&mut self) -> f32 {
        std::mem::take(&mut self.peak)
    }

    /// Per-track levels since the last call.
    pub fn take_track_peaks(&mut self, out: &mut Vec<f32>) {
        out.clear();
        out.extend(self.tracks.iter_mut().map(|t| std::mem::take(&mut t.peak)));
    }

    fn release_all(&mut self) {
        for track in &mut self.tracks {
            track.sound.release_all();
            if let Some(unit) = track.ext.and_then(|id| self.units.get_mut(&id)) {
                unit.all_notes_off();
            }
        }
    }

    fn is_silent(&self) -> bool {
        self.tracks.iter().all(|t| t.sound.is_silent())
    }

    fn click(&mut self, downbeat: bool) {
        self.click_inc = if downbeat { 1760.0 } else { 1320.0 } / self.sample_rate;
        self.click_phase = 0.0;
        self.click_left = self.click_len;
    }

    /// Fills `out` (interleaved, `channels` per frame) with the next chunk of audio.
    pub fn process(&mut self, out: &mut [f32], channels: usize) {
        if channels == 0 {
            return;
        }
        let frames = out.len() / channels;
        let mut done = 0;
        while done < frames {
            let mut n = (frames - done).min(BLOCK);
            let pos = self.position;
            let moving = self.playing && self.preroll <= 0.0;
            if self.playing {
                n = self.advance(n);
            }
            self.render_block(n, pos, moving);
            for i in 0..n {
                let frame = &mut out[(done + i) * channels..(done + i + 1) * channels];
                let (l, r) = (self.mix_l[i], self.mix_r[i]);
                if channels == 1 {
                    frame[0] = 0.5 * (l + r);
                } else {
                    frame[0] = l;
                    frame[1] = r;
                    for extra in &mut frame[2..] {
                        *extra = 0.0;
                    }
                }
            }
            done += n;
        }
    }

    /// Moves the playhead by up to `max` frames, firing the events on the way.
    /// Returns how many frames to render before the next call (shorter when the
    /// playhead reaches the end of the cycle or of the project).
    fn advance(&mut self, max: usize) -> usize {
        let beats_per_frame = self.project.tempo_at(self.position) / 60.0 / f64::from(self.sample_rate);
        let click_step = 4.0 / f64::from(self.project.beat_unit.max(1));

        if self.preroll > 0.0 {
            // Count-in: clicks only, the playhead waits.
            let before = self.preroll_total - self.preroll;
            let after = before + max as f64 * beats_per_frame;
            let k = (before / click_step - 1e-9).ceil();
            if k * click_step < after {
                self.click(k == 0.0);
            }
            self.preroll = (self.preroll_total - after).max(0.0);
            return max;
        }

        let start = self.position;
        let cycling = self.project.cycle_enabled
            && self.project.cycle_end > self.project.cycle_start
            && start < self.project.cycle_end;
        // While recording the project grows with what is played.
        let limit = if cycling {
            self.project.cycle_end
        } else if self.rec_track.is_some() {
            f64::INFINITY
        } else {
            self.end_beat
        };
        if start >= limit {
            self.stop();
            return max;
        }

        let to_limit = ((limit - start) / beats_per_frame).ceil();
        let hit = to_limit <= max as f64;
        let n = if hit { (to_limit as usize).max(1) } else { max };
        let end = if hit { limit } else { start + n as f64 * beats_per_frame };

        let tempo = self.project.tempo_at(start);
        for rt in &mut self.tracks {
            let from = rt.events.partition_point(|e| e.beat < start);
            let mut index = from;
            while let Some(ev) = rt.events.get(index).copied().filter(|e| e.beat < end) {
                send_note(rt, &mut self.units, ev.on, ev.pitch, ev.vel);
                index += 1;
            }
            // The pitch movements: a note whose movement ends in this block
            // goes back to its pitch, the others move on. One value a block.
            let TrackRt { glides, sound, .. } = rt;
            for glide in glides.iter().take_while(|g| g.from < end) {
                if glide.to <= start {
                    continue;
                }
                if glide.to < end {
                    sound.bend(glide.pitch, 0.0);
                } else {
                    let at = start.max(glide.from);
                    sound.bend(glide.pitch, glide.offset(at, (at - glide.from) * 60.0 / tempo));
                }
            }
        }
        if self.project.metronome {
            let k = (start / click_step - 1e-9).ceil();
            let beat = k * click_step;
            if beat < end {
                let bar = self.project.bar_len();
                let in_bar = beat % bar;
                self.click(in_bar < 1e-6 || bar - in_bar < 1e-6);
            }
        }

        if !hit {
            self.position = end;
        } else if cycling {
            self.position = self.project.cycle_start.max(0.0);
            self.release_all();
        } else {
            self.position = limit;
            self.stop();
        }
        n
    }

    fn render_block(&mut self, n: usize, pos: f64, moving: bool) {
        let sr = self.sample_rate;
        let tempo = self.project.tempo_at(pos);
        let bpf = tempo / 60.0 / f64::from(sr);
        self.mix_l[..n].fill(0.0);
        self.mix_r[..n].fill(0.0);
        self.echo_in[..n].fill(0.0);
        self.reverb_in[..n].fill(0.0);
        for route in &mut self.routes {
            route.left[..n].fill(0.0);
            route.right[..n].fill(0.0);
        }
        let any_solo = self.project.tracks.iter().any(|t| t.solo);

        for (track, rt) in self.project.tracks.iter().zip(&mut self.tracks) {
            let buf = &mut self.buf[..n];
            buf.fill(0.0);
            // Live arpeggiator: one held key per step of a clock that never stops.
            if !rt.held.is_empty() || rt.arp_note.is_some() {
                let rate = track.arp.rate.clamp(0.0625, 4.0);
                let k = (self.free_beat / rate - 1e-9).ceil();
                if k * rate < self.free_beat + n as f64 * bpf {
                    if let Some(pitch) = rt.arp_note.take() {
                        send_note(rt, &mut self.units, false, pitch, 0);
                    }
                    if rt.held.is_empty() {
                        rt.arp_step = 0;
                    } else {
                        let len = rt.held.len() * usize::from(track.arp.octaves.clamp(1, 4));
                        let index = arp_index(track.arp.mode, rt.arp_step, len, k);
                        let (pitch, velocity) = rt.held[index % rt.held.len()];
                        let pitch = (usize::from(pitch) + 12 * (index / rt.held.len())).min(127) as u8;
                        send_note(rt, &mut self.units, true, pitch, velocity);
                        rt.arp_note = Some(pitch);
                        rt.arp_step += 1;
                    }
                }
            }
            // Rendered even when muted so that envelopes keep running.
            rt.sound.render(buf);
            if let Some(unit) = rt.ext.and_then(|id| self.units.get_mut(&id)) {
                unit.process(buf);
            }
            if moving && track.kind.is_audio() {
                render_audio(track, &self.project, &self.clips, pos, bpf, sr, buf);
            }
            if let (true, Some(feed)) = (track.monitor && track.kind.is_audio(), &self.monitor) {
                // Live input, resampled to the output rate, through the track's effects.
                let step = f64::from(feed.sample_rate.load(Ordering::Relaxed)) / f64::from(sr);
                if let Ok(mut queue) = feed.queue.try_lock() {
                    for sample in buf.iter_mut() {
                        self.monitor_frac += step;
                        while self.monitor_frac >= 1.0 {
                            self.monitor_frac -= 1.0;
                            self.monitor_prev = self.monitor_cur;
                            self.monitor_cur = queue.pop_front().unwrap_or(0.0);
                        }
                        *sample += self.monitor_prev + (self.monitor_cur - self.monitor_prev) * self.monitor_frac as f32;
                    }
                }
            }
            // Effects driven by automation curves are set for this block.
            let (echo_send, reverb_send) = if track.fx_automated() {
                track.fx_into(pos, &mut rt.driven);
                rt.fx.configure(&rt.driven, &self.captures);
                rt.fx.process(&rt.driven, buf);
                (rt.driven.echo, rt.driven.reverb)
            } else {
                rt.fx.process(&track.fx, buf);
                (track.fx.echo, track.fx.reverb)
            };
            for slot in track.plugins.iter().filter(|slot| slot.on) {
                if let Some(unit) = self.units.get_mut(&slot.id) {
                    unit.process(buf);
                }
            }
            if track.mute || (any_solo && !track.solo) {
                continue;
            }
            let volume = if track.vol_auto.is_empty() {
                track.volume
            } else {
                curve_linear(&track.vol_auto, pos, f64::from(track.volume)) as f32
            }
            .clamp(0.0, 4.0);
            let pan = if track.pan_auto.is_empty() {
                track.pan
            } else {
                curve_linear(&track.pan_auto, pos, f64::from(track.pan)) as f32
            };
            // Constant-power pan.
            let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
            let gain = volume * std::f32::consts::SQRT_2;
            let (gl, gr) = (angle.cos() * gain, angle.sin() * gain);
            let (echo, reverb) = (echo_send.clamp(0.0, 1.0) * volume, reverb_send.clamp(0.0, 1.0) * volume);
            let mut peak = rt.peak;
            // A track sent to another output goes there as it is, past its own
            // effects and fader: the effects of the main output are not its own.
            let route = if track.output.is_empty() { None } else { self.routes.iter_mut().find(|route| route.name == track.output) };
            if let Some(route) = route {
                for ((&s, left), right) in buf[..n].iter().zip(&mut route.left[..n]).zip(&mut route.right[..n]) {
                    *left += s * gl;
                    *right += s * gr;
                    peak = peak.max((s * volume).abs());
                }
                rt.peak = peak;
                continue;
            }
            let sends = self.mix_l[..n].iter_mut().zip(&mut self.mix_r[..n]).zip(&mut self.echo_in[..n]).zip(&mut self.reverb_in[..n]);
            for (&s, (((left, right), echo_in), reverb_in)) in buf[..n].iter().zip(sends) {
                *left += s * gl;
                *right += s * gr;
                *echo_in += s * echo;
                *reverb_in += s * reverb;
                peak = peak.max((s * volume).abs());
            }
            rt.peak = peak;
        }

        for route in &mut self.routes {
            for sample in route.left[..n].iter_mut().chain(&mut route.right[..n]) {
                *sample = if sample.is_finite() { sample.clamp(-1.0, 1.0) } else { 0.0 };
            }
            route.feed.push(&route.left[..n], &route.right[..n]);
        }
        self.free_beat += n as f64 * bpf;
        if let Some(a) = &mut self.audition {
            let end = a.pos + n as f64 * bpf;
            let from = a.events.partition_point(|e| e.beat < a.pos);
            for ev in a.events[from..].iter().take_while(|e| e.beat < end) {
                if ev.on {
                    a.sound.note_on(ev.pitch, ev.vel);
                } else {
                    a.sound.note_off(ev.pitch);
                }
            }
            if end >= a.length {
                a.pos = 0.0;
                a.sound.release_all();
            } else {
                a.pos = end;
            }
            let buf = &mut self.buf[..n];
            buf.fill(0.0);
            a.sound.render(buf);
            for ((&s, left), right) in buf.iter().zip(&mut self.mix_l[..n]).zip(&mut self.mix_r[..n]) {
                *left += s * 0.8;
                *right += s * 0.8;
            }
        }

        self.master.process(
            &self.project.master_fx,
            tempo,
            &mut self.mix_l[..n],
            &mut self.mix_r[..n],
            &self.echo_in[..n],
            &self.reverb_in[..n],
        );

        let master = if self.project.master_auto.is_empty() {
            self.project.master_volume
        } else {
            curve_linear(&self.project.master_auto, pos, f64::from(self.project.master_volume)) as f32
        }
        .clamp(0.0, 4.0);
        let limiter = self.project.master_fx.limiter;
        for i in 0..n {
            let mut click = 0.0;
            if self.click_left > 0 {
                let env = self.click_left as f32 / self.click_len.max(1) as f32;
                click = (self.click_phase * TAU).sin() * env * 0.3;
                self.click_phase = (self.click_phase + self.click_inc).fract();
                self.click_left -= 1;
            }
            let (mut l, mut r) = (self.mix_l[i] * master + click, self.mix_r[i] * master + click);
            if limiter {
                // tanh keeps the output inside [-1, 1] without hard clipping.
                l = l.tanh();
                r = r.tanh();
            } else {
                l = l.clamp(-1.0, 1.0);
                r = r.clamp(-1.0, 1.0);
            }
            if !l.is_finite() || !r.is_finite() {
                l = 0.0;
                r = 0.0;
            }
            self.peak = self.peak.max(l.abs()).max(r.abs());
            self.mix_l[i] = l;
            self.mix_r[i] = r;
        }
    }
}

/// Renders the whole project to interleaved stereo samples, with a tail so the
/// last notes and effects can ring out. The cycle and the metronome are ignored.
pub fn render_offline(project: &Project, clips: &Clips, fonts: &Fonts, sample_rate: u32) -> Vec<f32> {
    render_offline_with(project, clips, fonts, sample_rate, &mut |_| None)
}

/// Like `render_offline`, with `make_unit` supplying a fresh instance of each
/// plug-in the project uses (the ones playing live cannot be shared).
pub fn render_offline_with(
    project: &Project,
    clips: &Clips,
    fonts: &Fonts,
    sample_rate: u32,
    make_unit: &mut dyn FnMut(&crate::model::PluginSlot) -> Option<Box<dyn ExternalUnit>>,
) -> Vec<f32> {
    render_offline_full(project, clips, fonts, &Captures::default(), sample_rate, make_unit)
}

/// Like `render_offline_with`, with the captures and impulses the effects use.
pub fn render_offline_full(
    project: &Project,
    clips: &Clips,
    fonts: &Fonts,
    captures: &Captures,
    sample_rate: u32,
    make_unit: &mut dyn FnMut(&crate::model::PluginSlot) -> Option<Box<dyn ExternalUnit>>,
) -> Vec<f32> {
    let slots: Vec<crate::model::PluginSlot> =
        project.tracks.iter().flat_map(|t| t.plugins.iter().chain(&t.instrument)).cloned().collect();
    let mut project = project.clone();
    project.cycle_enabled = false;
    project.metronome = false;
    let mut engine = Engine::new(project, sample_rate as f32);
    engine.set_clips(clips.clone());
    engine.set_fonts(fonts.clone());
    engine.set_captures(captures.clone());
    for slot in &slots {
        if let Some(unit) = make_unit(slot) {
            engine.add_unit(slot.id, unit);
        }
    }
    engine.set_position(0.0);
    engine.play();

    let mut out = Vec::new();
    let mut block = [0.0f32; 1024];
    // Stops by itself at the end of the project; the cap guards against a runaway.
    let cap = sample_rate as usize * 2 * 60 * 60 * 3;
    while engine.is_playing() && out.len() < cap {
        engine.process(&mut block, 2);
        out.extend_from_slice(&block);
    }
    // Up to four seconds of tail, cut once it has faded.
    let max_tail = sample_rate as usize * 4 * 2;
    let mut tail = 0;
    while tail < max_tail {
        engine.process(&mut block, 2);
        out.extend_from_slice(&block);
        tail += block.len();
        let quiet = block.iter().all(|s| s.abs() < 1e-4);
        if quiet && engine.is_silent() && tail >= sample_rate as usize / 2 {
            break;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AudioRef, DrummerParams, FxCurve, FxParam, Pedal, PedalKind, Point, Region, TrackKind};
    use crate::patches;
    use crate::templates;

    fn one_note_project() -> Project {
        let mut project = Project::empty();
        let mut track = Track::new("t", TrackKind::Instrument);
        let mut region = Region::new("r", 0.0, 4.0);
        region.content = Content::Midi(vec![Note { start: 0.0, length: 4.0, pitch: 60, velocity: 100, string: None, ..Default::default() }]);
        track.regions.push(region);
        project.tracks.push(track);
        project.master_fx.limiter = true;
        project
    }

    fn loudness(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32).sqrt()
    }

    #[test]
    fn demo_renders_audible_finite_audio() {
        let project = templates::demo();
        let samples = render_offline(&project, &Clips::new(), &Fonts::new(), 44_100);
        // 16 beats at 100 bpm is 9.6 s.
        let seconds = samples.len() as f64 / 2.0 / 44_100.0;
        assert!((9.6..14.0).contains(&seconds), "{seconds}");
        assert!(samples.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(samples.iter().any(|s| s.abs() > 0.05));
    }

    #[test]
    fn cycle_keeps_the_playhead_inside_the_loop() {
        let mut project = templates::demo();
        project.cycle_enabled = true;
        project.cycle_start = 4.0;
        project.cycle_end = 8.0;
        let mut engine = Engine::new(project, 48_000.0);
        engine.set_position(4.0);
        engine.play();
        let mut block = [0.0f32; 960];
        for _ in 0..1000 {
            engine.process(&mut block, 2);
            assert!(engine.is_playing());
            assert!((4.0..8.0).contains(&engine.position()), "{}", engine.position());
        }
    }

    #[test]
    fn playback_stops_at_the_end_of_the_project() {
        let mut engine = Engine::new(one_note_project(), 48_000.0);
        engine.play();
        let mut block = [0.0f32; 4800];
        for _ in 0..100 {
            engine.process(&mut block, 2);
        }
        assert!(!engine.is_playing());
        assert_eq!(engine.position(), 4.0);
    }

    #[test]
    fn mute_solo_and_automation_decide_what_is_heard() {
        let mut project = one_note_project();
        project.tracks.push(Track::new("vide", TrackKind::Instrument));
        let silent = |p: &Project| render_offline(p, &Clips::new(), &Fonts::new(), 22_050).iter().all(|s| s.abs() < 1e-6);

        project.tracks[0].mute = true;
        assert!(silent(&project));
        project.tracks[0].mute = false;
        project.tracks[1].solo = true;
        assert!(silent(&project));
        project.tracks[1].solo = false;
        assert!(!silent(&project));

        project.tracks[0].vol_auto = vec![Point { beat: 0.0, value: 0.0 }];
        assert!(silent(&project));
        project.tracks[0].vol_auto.clear();
        project.master_auto = vec![Point { beat: 0.0, value: 0.0 }];
        assert!(silent(&project));
    }

    #[test]
    fn effect_automation_drives_the_effects_over_time() {
        let mut track = Track::new("t", TrackKind::Instrument);
        track.fx.reverb = 0.2;
        track.fx.pedals.push(Pedal { kind: PedalKind::Overdrive, on: true, amount: 0.5, rate: 0.5 });
        assert!(!track.fx_automated());
        track.fx_auto.push(FxCurve { param: FxParam::Reverb, points: vec![Point { beat: 0.0, value: 0.0 }, Point { beat: 4.0, value: 1.0 }] });
        track.fx_auto.push(FxCurve { param: FxParam::EqLow, points: vec![Point { beat: 0.0, value: 12.0 }] });
        track.fx_auto.push(FxCurve { param: FxParam::Pedal(0), points: vec![Point { beat: 0.0, value: 1.0 }] });
        // A pedal that is not in the chain: nothing to drive.
        track.fx_auto.push(FxCurve { param: FxParam::Pedal(3), points: vec![Point { beat: 0.0, value: 1.0 }] });
        assert!(track.fx_automated());
        assert!((track.fx_at(2.0).reverb - 0.5).abs() < 1e-6, "ramps between the points");
        assert_eq!(track.fx_at(9.0).reverb, 1.0, "holds the last point");
        let at_start = track.fx_at(0.0);
        assert!(at_start.eq.on && at_start.eq.low == 12.0, "a driven band turns the equaliser on");
        assert_eq!(at_start.pedals[0].amount, 1.0);
        assert_eq!(at_start.echo, track.fx.echo, "what has no curve keeps its knob");
        assert_eq!((track.fx.reverb, track.fx.eq.on, track.fx.pedals[0].amount), (0.2, false, 0.5), "the knobs themselves are untouched");
        // A curve without points changes nothing; the curves are found by setting.
        track.fx_curve_mut(FxParam::Echo);
        assert_eq!(track.fx_at(1.0).echo, 0.0);
        assert_eq!(track.fx_curve(FxParam::Reverb).len(), 2);
        assert!(track.fx_curve(FxParam::Comp).is_empty());
        track.fx_curve_mut(FxParam::Comp).push(Point { beat: 0.0, value: 1.0 });
        assert_eq!(track.fx_at(0.0).comp, 1.0);
        for param in FxParam::FIXED {
            let (low, high) = param.range();
            assert!(low < high);
            let mut fx = crate::model::TrackFx::default();
            param.set(&mut fx, high + 100.0);
            assert_eq!(param.get(&fx), high, "{param:?} is kept in its range");
        }
        // Heard: an echo send that a curve opens makes the sound go on.
        let mut project = one_note_project();
        let render = |p: &Project| render_offline(p, &Clips::new(), &Fonts::new(), 22_050);
        let dry = render(&project);
        project.tracks[0].fx_auto = vec![FxCurve { param: FxParam::Echo, points: vec![Point { beat: 0.0, value: 1.0 }] }];
        let wet = render(&project);
        assert_ne!(dry, wet, "the curve is heard");
    }

    #[test]
    fn pan_automation_moves_the_sound() {
        let mut project = one_note_project();
        project.tracks[0].pan_auto = vec![Point { beat: 0.0, value: -1.0 }];
        let out = render_offline(&project, &Clips::new(), &Fonts::new(), 22_050);
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        let right: Vec<f32> = out.chunks(2).map(|f| f[1]).collect();
        assert!(loudness(&left) > 10.0 * loudness(&right));
    }

    #[test]
    fn mono_and_surround_outputs_are_filled() {
        let mut engine = Engine::new(templates::demo(), 48_000.0);
        engine.play();
        let mut mono = [0.0f32; 4800];
        engine.process(&mut mono, 1);
        assert!(mono.iter().any(|s| s.abs() > 0.0));
        let mut six = [1.0f32; 4800];
        engine.process(&mut six, 6);
        assert!(six.chunks(6).all(|f| f[2..].iter().all(|s| *s == 0.0)));
    }

    #[test]
    fn articulations_shape_the_events_and_move_the_pitch() {
        use crate::model::{Articulation, Slide, Vibrato};
        let mut project = one_note_project();
        let note = |start: f64, pitch: u8, string: u8, articulation: Articulation| Note { start, length: 1.0, pitch, velocity: 100, string: Some(string), articulation };
        let slide = Articulation { slide: Some(Slide::Shift), ..Articulation::default() };
        let bend = Articulation { bend: vec![(0.0, 0.0), (0.5, 2.0), (1.0, 2.0)], ..Articulation::default() };
        let dead = Articulation { dead: true, ..Articulation::default() };
        let staccato = Articulation { staccato: true, ..Articulation::default() };
        let hammer = Articulation { hammer: true, ..Articulation::default() };
        let vibrato = Articulation { vibrato: Some(Vibrato::Wide), ..Articulation::default() };
        project.tracks[0].regions[0].length = 8.0;
        project.tracks[0].regions[0].content = Content::Midi(vec![
            note(0.0, 60, 0, slide),
            note(1.0, 64, 0, bend),
            note(2.0, 60, 1, dead),
            note(3.0, 60, 0, staccato),
            note(4.0, 60, 0, hammer),
            note(5.0, 62, 0, vibrato),
        ]);
        let (events, glides) = bake(&project.tracks[0], &project);
        let ons: Vec<(f64, u8, u8)> = events.iter().filter(|e| e.on).map(|e| (e.beat, e.pitch, e.vel)).collect();
        assert_eq!(ons, [(0.0, 60, 100), (1.0, 64, 100), (3.0, 60, 100), (4.0, 60, 100), (5.0, 62, 80)], "no dead note; a hammered-on note is softer");
        let off = events.iter().find(|e| !e.on && e.beat > 3.0 && e.beat < 4.0).unwrap();
        assert!((off.beat - 3.5).abs() < 1e-9, "staccato: half the length");
        assert_eq!(glides.len(), 3);
        assert_eq!((glides[0].from, glides[0].to, glides[0].pitch), (0.0, 1.0, 60));
        assert_eq!(glides[0].offset(0.5, 0.0), 0.0);
        assert_eq!(glides[0].offset(1.0, 0.0), 4.0, "a shift slide reaches the next note on the string");
        assert!((glides[0].offset(0.75, 0.0) - 2.0).abs() < 1e-6);
        assert_eq!(glides[1].offset(1.5, 0.0), 2.0, "a full bend");
        assert!((glides[1].offset(1.25, 0.0) - 1.0).abs() < 1e-6);
        assert_eq!(glides[2].vibrato, 0.5);
        assert!(glides[2].offset(5.0, 0.0).abs() < 1e-6);
        assert!((glides[2].offset(5.0, 1.0 / 22.0) - 0.5).abs() < 1e-3, "a quarter of a cycle of 5.5 Hz in");
        // The arpeggiator plays the notes its own way: no movement then.
        project.tracks[0].arp.on = true;
        assert!(bake(&project.tracks[0], &project).1.is_empty());

        // Played, a bend makes a note climb: the sound is not the same as without it.
        project.tracks[0].arp.on = false;
        project.tracks[0].regions[0].content = Content::Midi(vec![note(0.0, 60, 0, Articulation::default())]);
        let plain = render_offline(&project, &Clips::new(), &Fonts::new(), 48000);
        project.tracks[0].regions[0].content = Content::Midi(vec![note(0.0, 60, 0, Articulation { bend: vec![(0.0, 0.0), (1.0, 12.0)], ..Articulation::default() })]);
        let bent = render_offline(&project, &Clips::new(), &Fonts::new(), 48000);
        assert_eq!(plain.len(), bent.len());
        assert!(plain.iter().any(|s| s.abs() > 1e-3), "the note sounds");
        assert!(plain != bent);
    }

    #[test]
    fn looped_regions_repeat_their_notes() {
        let mut project = one_note_project();
        let region = &mut project.tracks[0].regions[0];
        region.content = Content::Midi(vec![Note { start: 0.0, length: 0.5, pitch: 60, velocity: 100, string: None, ..Default::default() }]);
        region.length = 8.0;
        region.loop_len = Some(2.0);
        let (events, _) = bake(&project.tracks[0], &project);
        let ons: Vec<f64> = events.iter().filter(|e| e.on).map(|e| e.beat).collect();
        assert_eq!(ons, vec![0.0, 2.0, 4.0, 6.0]);
    }

    #[test]
    fn swing_tremolo_picking_and_trills_are_played() {
        use crate::model::{swung, Articulation, Swing};
        // Straight eighth notes played in a triplet feel from beat 1, straight again from beat 3.
        let feels = [Swing::triplet(1.0, false), Swing::straight(3.0)];
        assert_eq!(swung(&feels, 0.5), 0.5, "before the feel");
        assert!((swung(&feels, 1.5) - (1.0 + 2.0 / 3.0)).abs() < 1e-9, "the off-beat eighth comes late");
        assert_eq!((swung(&feels, 1.0), swung(&feels, 2.0)), (1.0, 2.0), "the beats stay");
        assert!((swung(&feels, 1.25) - (1.0 + 1.0 / 3.0)).abs() < 1e-9);
        assert_eq!(swung(&feels, 3.5), 3.5, "straight again");
        let sixteenths = [Swing::triplet(0.0, true)];
        assert!((swung(&sixteenths, 0.25) - 1.0 / 3.0).abs() < 1e-9 && (swung(&sixteenths, 0.75) - (0.5 + 1.0 / 3.0)).abs() < 1e-9);

        let mut project = one_note_project();
        let eighth = |start: f64, pitch: u8| Note { start, length: 0.5, pitch, velocity: 100, string: None, ..Default::default() };
        let region = &mut project.tracks[0].regions[0];
        region.content = Content::Midi(vec![eighth(0.0, 60), eighth(0.5, 62)]);
        region.swing = vec![Swing::triplet(0.0, false)];
        let (events, _) = bake(&project.tracks[0], &project);
        let times: Vec<(f64, bool)> = events.iter().map(|e| ((e.beat * 1000.0).round() / 1000.0, e.on)).collect();
        assert_eq!(times, [(0.0, true), (0.667, false), (0.667, true), (1.0, false)], "long then short");

        // Tremolo picking: a quarter note struck as four sixteenths; a trill
        // with the note a tone above, in eighths.
        let region = &mut project.tracks[0].regions[0];
        region.swing.clear();
        region.content = Content::Midi(vec![
            Note { start: 0.0, length: 1.0, pitch: 60, velocity: 100, articulation: Articulation { tremolo: Some(0.25), ..Articulation::default() }, ..Default::default() },
            Note { start: 2.0, length: 1.0, pitch: 64, velocity: 100, articulation: Articulation { trill: Some((66, 0.5)), ..Articulation::default() }, ..Default::default() },
        ]);
        let (events, _) = bake(&project.tracks[0], &project);
        let ons: Vec<(f64, u8, u8)> = events.iter().filter(|e| e.on).map(|e| (e.beat, e.pitch, e.vel)).collect();
        assert_eq!(ons, [(0.0, 60, 100), (0.25, 60, 85), (0.5, 60, 85), (0.75, 60, 85), (2.0, 64, 100), (2.5, 66, 85)]);
    }

    #[test]
    fn transposition_track_and_arpeggiator_change_the_notes() {
        let mut project = one_note_project();
        project.transpose_points = vec![Point { beat: 0.0, value: 3.0 }];
        let (events, _) = bake(&project.tracks[0], &project);
        assert_eq!(events[0].pitch, 63);

        project.transpose_points.clear();
        project.tracks[0].regions[0].content = Content::Midi(vec![
            Note { start: 0.0, length: 2.0, pitch: 60, velocity: 100, string: None, ..Default::default() },
            Note { start: 0.0, length: 2.0, pitch: 64, velocity: 100, string: None, ..Default::default() },
            Note { start: 0.0, length: 2.0, pitch: 67, velocity: 100, string: None, ..Default::default() },
        ]);
        project.tracks[0].arp = Arp { on: true, rate: 0.5, mode: ArpMode::Up, octaves: 1 };
        let pitches = |p: &Project| -> Vec<u8> { bake(&p.tracks[0], p).0.iter().filter(|e| e.on).map(|e| e.pitch).collect() };
        assert_eq!(pitches(&project), vec![60, 64, 67, 60]);
        project.tracks[0].arp.mode = ArpMode::Down;
        assert_eq!(pitches(&project), vec![67, 64, 60, 67]);
        project.tracks[0].arp.mode = ArpMode::UpDown;
        assert_eq!(pitches(&project), vec![60, 64, 67, 64]);
        project.tracks[0].arp.mode = ArpMode::Up;
        project.tracks[0].arp.octaves = 2;
        project.tracks[0].arp.rate = 0.25;
        assert_eq!(pitches(&project), vec![60, 64, 67, 72, 76, 79, 60, 64]);
    }

    #[test]
    fn drummer_regions_make_sound() {
        let mut project = Project::empty();
        let mut track = Track::new("Batteur", TrackKind::Drummer);
        track.sound = patches::find(patches::DEFAULT_DRUMS).unwrap().sound;
        let mut region = Region::new("Batterie", 0.0, 8.0);
        region.content = Content::Drummer(DrummerParams::default());
        track.regions.push(region);
        project.tracks.push(track);
        let out = render_offline(&project, &Clips::new(), &Fonts::new(), 22_050);
        assert!(loudness(&out) > 0.01);
    }

    #[test]
    fn audio_regions_play_their_clip_inside_their_bounds() {
        let mut project = Project::empty();
        project.tempo = 120.0;
        project.master_fx.limiter = false;
        let mut track = Track::new("Audio", TrackKind::Audio);
        // Region from beat 2 to beat 4 = seconds 1 to 2.
        let mut region = Region::new("clip", 2.0, 2.0);
        region.content = Content::Audio(AudioRef::new(7));
        track.regions.push(region);
        project.tracks.push(track);
        project.end = Some(8.0);

        // A clip at another sample rate than the engine: 2 s of a 0.25 ramp-free constant.
        let clip = ClipData { sample_rate: 8000, channels: 1, samples: vec![0.25; 16_000] };
        let mut clips = Clips::new();
        clips.insert(7, Arc::new(clip));
        let out = render_offline(&project, &clips, &Fonts::new(), 16_000);
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        assert!(left[..15_900].iter().all(|s| s.abs() < 1e-6), "silent before the region");
        assert!(left[16_100..31_900].iter().all(|s| *s > 0.1), "clip heard inside the region");
        assert!(left[32_100..48_000].iter().all(|s| s.abs() < 1e-6), "silent after the region");

        // A missing clip is silence, not a crash.
        let out = render_offline(&project, &Clips::new(), &Fonts::new(), 16_000);
        assert!(out.iter().all(|s| s.abs() < 1e-6));
    }

    #[test]
    fn tempo_track_changes_the_duration() {
        let mut project = one_note_project();
        project.tempo = 120.0;
        let mut engine = Engine::new(project.clone(), 48_000.0);
        let frames_until_stop = |engine: &mut Engine| {
            engine.play();
            let mut block = [0.0f32; 96];
            let mut frames = 0;
            while engine.is_playing() {
                engine.process(&mut block, 2);
                frames += 48;
            }
            frames
        };
        let normal = frames_until_stop(&mut engine);
        assert!((normal as f64 / 48_000.0 - 2.0).abs() < 0.01);
        // Second half at half speed: 1 s + 2 s.
        project.tempo_points = vec![Point { beat: 2.0, value: 60.0 }];
        let mut engine = Engine::new(project, 48_000.0);
        let slowed = frames_until_stop(&mut engine);
        assert!((slowed as f64 / 48_000.0 - 3.0).abs() < 0.01, "{slowed}");
    }

    #[test]
    fn recording_captures_played_notes_after_the_count_in() {
        let mut project = one_note_project();
        project.tempo = 120.0;
        let mut engine = Engine::new(project, 48_000.0);
        engine.record(0, true);
        assert!(engine.is_recording());
        let mut block = [0.0f32; 9600]; // 0.1 s = 0.2 beat
        // One bar of count-in is 2 s: the playhead does not move.
        for _ in 0..19 {
            engine.process(&mut block, 2);
        }
        assert_eq!(engine.position(), 0.0);
        assert!(engine.preroll() > 0.0);
        engine.process(&mut block, 2);
        engine.process(&mut block, 2);
        assert!(engine.position() > 0.0);
        assert!(engine.take_recording().is_none(), "nothing to take while recording");

        let on_at = engine.position();
        engine.live_note_on(0, 72, 90);
        for _ in 0..5 {
            engine.process(&mut block, 2);
        }
        engine.live_note_off(0, 72);
        engine.live_note_on(0, 76, 80); // still held when recording stops
        for _ in 0..40 {
            engine.process(&mut block, 2); // runs past the old end of the project
        }
        assert!(engine.is_playing());
        engine.stop();
        let recorded = engine.take_recording().unwrap();
        assert_eq!(recorded.track, 0);
        assert_eq!(recorded.notes.len(), 2);
        assert_eq!(recorded.notes[0].pitch, 72);
        assert!((recorded.notes[0].start - on_at).abs() < 1e-9);
        assert!((recorded.notes[0].length - 1.0).abs() < 0.01);
        assert_eq!(recorded.notes[1].pitch, 76);
        assert!(recorded.end > 4.0);
        assert!(engine.take_recording().is_none());
    }

    #[test]
    fn audition_plays_without_the_transport() {
        let mut engine = Engine::new(Project::empty(), 48_000.0);
        let notes = [Note { start: 0.0, length: 0.5, pitch: 60, velocity: 100, string: None, ..Default::default() }];
        engine.audition(&patches::find(patches::DEFAULT_PATCH).unwrap().sound, &notes, 1.0);
        let mut block = vec![0.0f32; 96_000];
        engine.process(&mut block, 2);
        assert!(!engine.is_playing());
        assert_eq!(engine.position(), 0.0);
        // The one-beat loop repeats: sound in the first and in the second half second.
        assert!(loudness(&block[..9600]) > 0.01);
        assert!(loudness(&block[48_000..57_600]) > 0.01);
        engine.stop_audition();
        let mut tail = vec![0.0f32; 96_000];
        engine.process(&mut tail, 2);
        assert!(loudness(&tail[90_000..]) < 1e-3);
    }

    #[test]
    fn a_track_sent_to_another_output_leaves_the_main_mix() {
        let mut project = Project::empty();
        project.tracks.push(Track::new("t", TrackKind::Instrument));
        let loud = |samples: &[f32]| samples.iter().any(|s| s.abs() > 0.01);
        // In the main mix to begin with.
        let mut engine = Engine::new(project.clone(), 48_000.0);
        engine.live_note_on(0, 60, 100);
        let mut out = vec![0.0f32; 9600];
        engine.process(&mut out, 2);
        assert!(loud(&out));

        // Sent to an output of its own: nothing of it in the main mix, all of it there.
        project.tracks[0].output = "Casque".to_owned();
        let mut engine = Engine::new(project.clone(), 48_000.0);
        let feed = Arc::new(OutFeed::new(48_000));
        engine.set_routes(vec![("Casque".to_owned(), Arc::clone(&feed))]);
        assert_eq!(engine.route_names(), vec!["Casque".to_owned()]);
        engine.live_note_on(0, 60, 100);
        out.fill(1.0);
        engine.process(&mut out, 2);
        assert!(out.iter().all(|s| s.abs() < 1e-6), "the main mix is silent");
        let sent: Vec<f32> = feed.queue.lock().unwrap().iter().copied().collect();
        assert_eq!(sent.len(), 9600, "a stereo frame for each frame played");
        assert!(loud(&sent) && sent.iter().all(|s| s.abs() <= 1.0));
        assert!(engine.take_peak() < 1e-6);

        // Nobody takes the sound: the queue stays bounded.
        for _ in 0..40 {
            engine.process(&mut out, 2);
        }
        let waiting = feed.queue.lock().unwrap().len();
        assert!(waiting <= 2 * 12_000 && waiting.is_multiple_of(2), "{waiting}");

        // An output that is not there (another computer's): back in the main mix.
        project.tracks[0].output = "Ailleurs".to_owned();
        let mut engine = Engine::new(project, 48_000.0);
        engine.set_routes(vec![("Casque".to_owned(), Arc::clone(&feed))]);
        engine.live_note_on(0, 60, 100);
        engine.process(&mut out, 2);
        assert!(loud(&out));
    }

    #[test]
    fn processed_audio_is_played_when_available() {
        let mut project = Project::empty();
        project.master_fx.limiter = false;
        let mut track = Track::new("Audio", TrackKind::Audio);
        let mut region = Region::new("clip", 0.0, 4.0);
        let audio = AudioRef { transpose: 5, ..AudioRef::new(3) };
        region.content = Content::Audio(audio);
        track.regions.push(region);
        project.tracks.push(track);
        let mut clips = Clips::new();
        clips.insert(3, Arc::new(ClipData { sample_rate: 16_000, channels: 1, samples: vec![0.25; 32_000] }));
        // Until the transposed copy exists, the raw clip is heard.
        let raw = render_offline(&project, &clips, &Fonts::new(), 16_000);
        assert!(raw[2000] > 0.1);
        assert_ne!(audio.key(), 3);
        clips.insert(audio.key(), Arc::new(ClipData { sample_rate: 16_000, channels: 1, samples: vec![-0.25; 32_000] }));
        let processed = render_offline(&project, &clips, &Fonts::new(), 16_000);
        assert!(processed[2000] < -0.1);
    }

    #[test]
    fn live_arpeggiator_plays_held_keys_in_turn() {
        let mut project = one_note_project();
        project.tracks[0].regions.clear();
        project.tempo = 120.0;
        project.tracks[0].arp = Arp { on: true, rate: 0.5, mode: ArpMode::Up, octaves: 1 };
        let mut engine = Engine::new(project, 48_000.0);
        let mut block = vec![0.0f32; 9600];
        engine.live_note_on(0, 60, 100);
        engine.live_note_on(0, 67, 100);
        // Nothing sounds before the first step; then one note per quarter second.
        let mut heard = Vec::new();
        for _ in 0..8 {
            engine.process(&mut block, 2);
            heard.push(engine.tracks[0].arp_note);
        }
        assert!(heard.contains(&Some(60)) && heard.contains(&Some(67)), "{heard:?}");
        assert!(loudness(&block) > 0.005);
        engine.live_note_off(0, 60);
        engine.live_note_off(0, 67);
        for _ in 0..40 {
            engine.process(&mut block, 2);
        }
        assert_eq!(engine.tracks[0].arp_note, None);
        assert!(loudness(&block[4800..]) < 1e-3);
    }

    #[test]
    fn monitored_tracks_pass_the_live_input_through() {
        let mut project = Project::empty();
        project.master_fx.limiter = false;
        let mut track = Track::new("Micro", TrackKind::Audio);
        track.monitor = true;
        project.tracks.push(track);
        let mut engine = Engine::new(project, 48_000.0);
        let feed = Arc::new(MonitorFeed::new());
        feed.sample_rate.store(24_000, Ordering::Relaxed);
        engine.set_monitor(Some(Arc::clone(&feed)));
        // 0.1 s of input at half the output rate fills 0.1 s of output.
        feed.push(std::iter::repeat_n(0.5, 2400));
        let mut block = vec![0.0f32; 9600];
        engine.process(&mut block, 2);
        assert!(block[2000..8000].iter().all(|s| (*s - 0.4).abs() < 0.02), "{}", block[4000]);
        // With nothing coming in, silence; and the queue never grows without limit.
        engine.process(&mut block, 2);
        assert!(block[4000..].iter().all(|s| s.abs() < 1e-6));
        feed.push(std::iter::repeat_n(0.1, 1_000_000));
        assert!(feed.queue.lock().unwrap().len() <= 3000);
    }

    /// A stand-in plug-in: as an instrument it adds a constant while a note is
    /// held, as an effect it multiplies by its one parameter.
    struct FakeUnit {
        held: u32,
        gain: f32,
        effect: bool,
    }

    impl ExternalUnit for FakeUnit {
        fn process(&mut self, buf: &mut [f32]) {
            for s in buf.iter_mut() {
                *s = if self.effect { *s * self.gain } else { *s + if self.held > 0 { 0.25 } else { 0.0 } };
            }
        }
        fn note_on(&mut self, _: u8, _: u8) {
            self.held += 1;
        }
        fn note_off(&mut self, _: u8) {
            self.held = self.held.saturating_sub(1);
        }
        fn set_param(&mut self, _: u32, value: f64) {
            self.gain = value as f32;
        }
    }

    #[test]
    fn external_instruments_and_effects_take_part_in_the_mix() {
        use crate::model::PluginSlot;
        let slot = |id: u64| PluginSlot { id, file: String::new(), plugin: String::new(), name: String::new(), on: true, params: Vec::new(), state: None };
        let mut project = one_note_project();
        project.master_fx.limiter = false;
        project.tracks[0].instrument = Some(slot(1));
        project.tracks[0].plugins = vec![slot(2)];
        let mut make = |s: &PluginSlot| -> Option<Box<dyn ExternalUnit>> {
            Some(Box::new(FakeUnit { held: 0, gain: 0.5, effect: s.id == 2 }))
        };
        // The note goes to the external instrument (0.25), then through the effect (x 0.5).
        let out = render_offline_with(&project, &Clips::new(), &Fonts::new(), 16_000, &mut make);
        assert!((out[8000] - 0.25 * 0.5 * 0.8).abs() < 1e-3, "{}", out[8000]);
        // The built-in sound stays silent, and the note ends with the region.
        let last = out.len() - 2;
        assert!(out[last].abs() < 1e-4);
        // Switched off, the effect is skipped and the built-in sound plays again.
        project.tracks[0].plugins[0].on = false;
        project.tracks[0].instrument = None;
        let out = render_offline_with(&project, &Clips::new(), &Fonts::new(), 16_000, &mut make);
        assert!(out[4000..8000].iter().any(|s| s.abs() > 0.02) && out[4000..8000].iter().any(|s| *s < 0.0));

        // The engine drops units whose slot left the project, and applies parameters.
        let mut engine = Engine::new(project.clone(), 16_000.0);
        engine.add_unit(2, Box::new(FakeUnit { held: 0, gain: 1.0, effect: true }));
        engine.add_unit(9, Box::new(FakeUnit { held: 0, gain: 1.0, effect: true }));
        engine.unit_param(2, 0, 0.0);
        engine.set_project(project);
        assert!(engine.has_unit(2) && !engine.has_unit(9));
    }

    #[test]
    fn effects_on_every_track_stay_bounded() {
        let mut project = templates::demo();
        for track in &mut project.tracks {
            track.fx.eq.on = true;
            track.fx.eq.low = 12.0;
            track.fx.comp = 1.0;
            track.fx.echo = 1.0;
            track.fx.reverb = 1.0;
        }
        project.master_fx.comp = 1.0;
        project.master_fx.echo_feedback = 0.9;
        let out = render_offline(&project, &Clips::new(), &Fonts::new(), 22_050);
        assert!(out.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
    }
}
