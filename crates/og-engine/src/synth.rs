//! Sound generators: a subtractive synthesiser, a plucked-string model and a
//! synthesised drum kit. All of them render mono and are driven by note events.

use std::f32::consts::TAU;

use std::collections::HashMap;
use std::sync::Arc;

use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};

use crate::model::{DrumKit, DrumParams, PluckParams, SamplerParams, Sound, SynthParams, Wave};
use crate::sfz::{SfzInstrument, SfzSynth};

/// A loaded bank of sampled instruments.
#[derive(Clone)]
pub enum Bank {
    /// A SoundFont 2 file: many presets.
    Sf2(Arc<SoundFont>),
    /// An SFZ file: one instrument.
    Sfz(Arc<SfzInstrument>),
}

/// Loaded sound banks, by id.
pub type Fonts = HashMap<u64, Bank>;

/// Reads a SoundFont (SF2) file.
pub fn load_font(bytes: &[u8]) -> Result<Arc<SoundFont>, String> {
    SoundFont::new(&mut &bytes[..]).map(Arc::new).map_err(|e| format!("unreadable sound bank ({e:?})"))
}

/// File extensions `load_bank` understands.
pub const BANK_EXTENSIONS: [&str; 3] = ["sf2", "sf3", "sfz"];

/// Reads a sound bank, choosing the format from the file extension.
pub fn load_bank(path: &std::path::Path) -> Result<Bank, String> {
    let extension = crate::codec::extension_of(path);
    if extension == "sfz" {
        SfzInstrument::load(path).map(|i| Bank::Sfz(Arc::new(i)))
    } else {
        let mut bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        // SF3 is SF2 with compressed samples: decompress, then read as usual.
        if crate::sf3::is_sf3(&bytes) {
            bytes = crate::sf3::to_sf2(&bytes)?;
        }
        load_font(&bytes).map(Bank::Sf2)
    }
}

/// (name, bank, preset number) of everything a bank can play.
pub fn bank_presets(bank: &Bank) -> Vec<(String, u16, u8)> {
    match bank {
        Bank::Sf2(font) => font_presets(font),
        Bank::Sfz(instrument) => vec![(instrument.name.clone(), 0, 0)],
    }
}

/// (name, bank, preset number) of every preset of a SoundFont, sorted.
pub fn font_presets(font: &SoundFont) -> Vec<(String, u16, u8)> {
    let mut presets: Vec<(String, u16, u8)> = font
        .get_presets()
        .iter()
        .map(|p| (p.get_name().trim().to_owned(), p.get_bank_number().clamp(0, 16_383) as u16, p.get_patch_number().clamp(0, 127) as u8))
        .collect();
    presets.sort_by_key(|a| (a.1, a.2));
    presets
}

pub const MAX_VOICES: usize = 24;

fn pitch_to_hz(pitch: f32) -> f32 {
    440.0 * 2f32.powf((pitch - 69.0) / 12.0)
}

/// Small deterministic noise source.
#[derive(Clone, Copy)]
pub struct Rng(pub u32);

impl Rng {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / 16_777_216.0
    }

    /// Uniform in [-1, 1).
    pub fn bipolar(&mut self) -> f32 {
        self.unit() * 2.0 - 1.0
    }
}

// ----- subtractive synth ---------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
}

#[derive(Clone, Copy)]
struct Voice {
    pitch: u8,
    stage: Stage,
    env: f32,
    vel: f32,
    inc: f32,
    inc2: f32,
    p1: f32,
    p2: f32,
    psub: f32,
    lp1: f32,
    lp2: f32,
    age: u64,
    /// Where a bend, a slide or a vibrato has taken the pitch: a ratio.
    shift: f32,
}

const SILENT: Voice = Voice {
    pitch: 0,
    stage: Stage::Off,
    env: 0.0,
    vel: 0.0,
    inc: 0.0,
    inc2: 0.0,
    p1: 0.0,
    p2: 0.0,
    psub: 0.0,
    lp1: 0.0,
    lp2: 0.0,
    age: 0,
    shift: 1.0,
};

pub struct Synth {
    params: SynthParams,
    sample_rate: f32,
    voices: [Voice; MAX_VOICES],
    clock: u64,
    lfo: f32,
}

fn poly_blep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt;
        2.0 * x - x * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

fn osc(wave: Wave, p: f32, dt: f32) -> f32 {
    match wave {
        Wave::Sine => (p * TAU).sin(),
        Wave::Saw => 2.0 * p - 1.0 - poly_blep(p, dt),
        Wave::Square => {
            let base = if p < 0.5 { 1.0 } else { -1.0 };
            base + poly_blep(p, dt) - poly_blep((p + 0.5).fract(), dt)
        }
        Wave::Triangle => 4.0 * (p - 0.5).abs() - 1.0,
    }
}

impl Synth {
    pub fn new(params: SynthParams, sample_rate: f32) -> Self {
        Self { params, sample_rate, voices: [SILENT; MAX_VOICES], clock: 0, lfo: 0.0 }
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8) {
        self.clock += 1;
        // Reuse the voice already playing this pitch, else a free one, else the oldest.
        let slot = self
            .voices
            .iter()
            .position(|v| v.stage != Stage::Off && v.pitch == pitch)
            .or_else(|| self.voices.iter().position(|v| v.stage == Stage::Off))
            .unwrap_or_else(|| {
                let mut oldest = 0;
                for (i, v) in self.voices.iter().enumerate() {
                    if v.age < self.voices[oldest].age {
                        oldest = i;
                    }
                }
                oldest
            });
        let inc = pitch_to_hz(f32::from(pitch)) / self.sample_rate;
        let voice = &mut self.voices[slot];
        let retrigger = voice.stage != Stage::Off;
        *voice = Voice {
            pitch,
            stage: Stage::Attack,
            env: if retrigger { voice.env } else { 0.0 },
            vel: f32::from(velocity.min(127)) / 127.0,
            inc,
            inc2: inc * 2f32.powf(self.params.detune_cents / 1200.0),
            p1: if retrigger { voice.p1 } else { 0.0 },
            p2: if retrigger { voice.p2 } else { 0.25 },
            psub: if retrigger { voice.psub } else { 0.0 },
            lp1: voice.lp1,
            lp2: voice.lp2,
            age: self.clock,
            shift: 1.0,
        };
    }

    /// Moves the notes sounding at `pitch` by `semitones`.
    pub fn bend(&mut self, pitch: u8, semitones: f32) {
        for v in &mut self.voices {
            if v.pitch == pitch && v.stage != Stage::Off {
                v.shift = 2f32.powf(semitones / 12.0);
            }
        }
    }

    pub fn unbend(&mut self) {
        for v in &mut self.voices {
            v.shift = 1.0;
        }
    }

    pub fn note_off(&mut self, pitch: u8) {
        for v in &mut self.voices {
            if v.pitch == pitch && v.stage != Stage::Off {
                v.stage = Stage::Release;
            }
        }
    }

    pub fn release_all(&mut self) {
        for v in &mut self.voices {
            if v.stage != Stage::Off {
                v.stage = Stage::Release;
            }
        }
    }

    pub fn is_silent(&self) -> bool {
        self.voices.iter().all(|v| v.stage == Stage::Off)
    }

    pub fn render(&mut self, out: &mut [f32]) {
        let p = self.params;
        let sr = self.sample_rate;
        let attack_step = 1.0 / (p.attack.max(1e-4) * sr);
        let decay_step = (1.0 - p.sustain) / (p.decay.max(1e-4) * sr);
        let release_coef = (-5.0 / (p.release.max(1e-3) * sr)).exp();
        let second = p.detune_cents != 0.0;
        let norm = 1.0 / (1.0 + if second { 0.5 } else { 0.0 } + p.sub);
        let max_cut = sr * 0.45;
        // One vibrato value per block is enough at this block size.
        let bend = if p.vibrato > 0.0 {
            self.lfo = (self.lfo + 5.5 * out.len() as f32 / sr).fract();
            2f32.powf(p.vibrato * (self.lfo * TAU).sin() / 12.0)
        } else {
            1.0
        };

        for v in &mut self.voices {
            if v.stage == Stage::Off {
                continue;
            }
            let (inc, inc2) = (v.inc * bend * v.shift, v.inc2 * bend * v.shift);
            for sample in out.iter_mut() {
                match v.stage {
                    Stage::Attack => {
                        v.env += attack_step;
                        if v.env >= 1.0 {
                            v.env = 1.0;
                            v.stage = Stage::Decay;
                        }
                    }
                    Stage::Decay => {
                        v.env -= decay_step;
                        if v.env <= p.sustain {
                            v.env = p.sustain;
                            v.stage = Stage::Sustain;
                        }
                    }
                    Stage::Sustain => {}
                    Stage::Release => {
                        v.env *= release_coef;
                        if v.env < 1e-4 {
                            v.stage = Stage::Off;
                            v.env = 0.0;
                            break;
                        }
                    }
                    Stage::Off => break,
                }

                let mut s = osc(p.wave, v.p1, inc);
                v.p1 = (v.p1 + inc).fract();
                if second {
                    s += 0.5 * osc(p.wave, v.p2, inc2);
                    v.p2 = (v.p2 + inc2).fract();
                }
                if p.sub > 0.0 {
                    s += p.sub * (v.psub * TAU).sin();
                    v.psub = (v.psub + inc * 0.5).fract();
                }

                let cutoff = (p.cutoff + p.cutoff_env * v.env).clamp(20.0, max_cut);
                let a = 1.0 - (-TAU * cutoff / sr).exp();
                v.lp1 += a * (s * norm - v.lp1);
                v.lp2 += a * (v.lp1 - v.lp2);

                *sample += v.lp2 * v.env * v.vel * p.gain;
            }
        }
    }
}

// ----- plucked string ----------------------------------------------------------

const PLUCK_VOICES: usize = 16;
const PLUCK_MAX: usize = 4096;

struct PluckVoice {
    buf: Vec<f32>,
    len: usize,
    idx: usize,
    pitch: u8,
    active: bool,
    releasing: bool,
    /// All-pass state used to tune the string between two whole delay lengths.
    ap_coef: f32,
    ap_x: f32,
    ap_y: f32,
    level: f32,
    feedback: f32,
    age: u64,
}

/// Plucked string after Karplus and Strong: a noise burst circulating in a
/// delay line through a gentle low-pass.
pub struct PluckSynth {
    params: PluckParams,
    sample_rate: f32,
    voices: Vec<PluckVoice>,
    rng: Rng,
    clock: u64,
}

/// The loop length and all-pass coefficient that tune a plucked string to `hz`:
/// the loop is `len` samples, minus half a sample because the averaging filter
/// looks one sample ahead, plus the all-pass delay.
fn pluck_tuning(sample_rate: f32, hz: f32) -> (usize, f32) {
    let period = (sample_rate / hz).clamp(4.0, (PLUCK_MAX - 2) as f32);
    let mut len = (period + 0.5).floor();
    let mut frac = period + 0.5 - len;
    if frac < 0.1 {
        len -= 1.0;
        frac += 1.0;
    }
    ((len as usize).clamp(2, PLUCK_MAX - 1), (1.0 - frac) / (1.0 + frac))
}

impl PluckSynth {
    pub fn new(params: PluckParams, sample_rate: f32) -> Self {
        let voices = (0..PLUCK_VOICES)
            .map(|_| PluckVoice {
                buf: vec![0.0; PLUCK_MAX],
                len: 2,
                idx: 0,
                pitch: 0,
                active: false,
                releasing: false,
                ap_coef: 0.0,
                ap_x: 0.0,
                ap_y: 0.0,
                level: 0.0,
                feedback: 0.99,
                age: 0,
            })
            .collect();
        Self { params, sample_rate, voices, rng: Rng(0x1234_5678), clock: 0 }
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8) {
        self.clock += 1;
        let slot = self
            .voices
            .iter()
            .position(|v| v.active && v.pitch == pitch)
            .or_else(|| self.voices.iter().position(|v| !v.active))
            .unwrap_or_else(|| {
                let mut oldest = 0;
                for (i, v) in self.voices.iter().enumerate() {
                    if v.age < self.voices[oldest].age {
                        oldest = i;
                    }
                }
                oldest
            });
        let hz = pitch_to_hz(f32::from(pitch));
        let (len, ap_coef) = pluck_tuning(self.sample_rate, hz);
        let vel = f32::from(velocity.min(127)) / 127.0;
        let bright = self.params.brightness.clamp(0.0, 1.0);
        // Higher notes die faster on a real string; long sustain pushes feedback to 1.
        let seconds = 0.3 + 5.0 * self.params.sustain.clamp(0.0, 1.0);
        let feedback = 10f32.powf(-3.0 / (seconds * hz.max(20.0))).min(0.9999);

        let v = &mut self.voices[slot];
        v.len = len;
        v.idx = 0;
        v.pitch = pitch;
        v.active = true;
        v.releasing = false;
        v.ap_coef = ap_coef;
        v.ap_x = 0.0;
        v.ap_y = 0.0;
        v.level = 1.0;
        v.feedback = feedback;
        v.age = self.clock;
        // The pluck: noise, smoothed more for a duller tone.
        let mut prev = 0.0;
        for sample in &mut v.buf[..v.len] {
            let noise = self.rng.bipolar();
            prev += (0.15 + 0.85 * bright) * (noise - prev);
            *sample = prev * vel;
        }
    }

    pub fn note_off(&mut self, pitch: u8) {
        for v in &mut self.voices {
            if v.active && v.pitch == pitch {
                v.releasing = true;
            }
        }
    }

    /// Moves the strings sounding at `pitch` by `semitones`: the loop is
    /// shortened or lengthened while it rings.
    pub fn bend(&mut self, pitch: u8, semitones: f32) {
        for v in &mut self.voices {
            if v.active && v.pitch == pitch {
                let hz = pitch_to_hz(f32::from(pitch) + semitones);
                (v.len, v.ap_coef) = pluck_tuning(self.sample_rate, hz);
                if v.idx >= v.len {
                    v.idx = 0;
                }
            }
        }
    }

    pub fn unbend(&mut self) {
        for v in &mut self.voices {
            if v.active {
                (v.len, v.ap_coef) = pluck_tuning(self.sample_rate, pitch_to_hz(f32::from(v.pitch)));
                if v.idx >= v.len {
                    v.idx = 0;
                }
            }
        }
    }

    pub fn release_all(&mut self) {
        for v in &mut self.voices {
            v.releasing = v.active;
        }
    }

    pub fn is_silent(&self) -> bool {
        self.voices.iter().all(|v| !v.active)
    }

    pub fn render(&mut self, out: &mut [f32]) {
        let gain = self.params.gain;
        for v in &mut self.voices {
            if !v.active {
                continue;
            }
            let feedback = if v.releasing { v.feedback * 0.985 } else { v.feedback };
            for sample in out.iter_mut() {
                let cur = v.buf[v.idx];
                let next_idx = if v.idx + 1 >= v.len { 0 } else { v.idx + 1 };
                let avg = 0.5 * (cur + v.buf[next_idx]);
                let ap = v.ap_coef * avg + v.ap_x - v.ap_coef * v.ap_y;
                v.ap_x = avg;
                v.ap_y = ap;
                v.buf[v.idx] = ap * feedback;
                v.idx = next_idx;
                *sample += cur * gain;
                v.level = v.level * 0.9995 + cur.abs() * 0.0005;
            }
            if v.level < 2e-4 {
                v.active = false;
            }
        }
    }
}

// ----- drums -----------------------------------------------------------------

/// Recipe for one drum hit: a sine with a falling pitch plus filtered noise.
#[derive(Clone, Copy)]
struct Hit {
    tone: f32,
    tone_end: f32,
    sweep: f32,
    tone_amp: f32,
    tone_len: f32,
    noise_amp: f32,
    noise_len: f32,
    noise_hp: f32,
    noise_lp: f32,
    /// Hits in the same non-zero group cut each other (open and closed hi-hat).
    choke: u8,
}

// The parameters of a drum hit, each a number of its own: a struct would
// only rename them.
#[allow(clippy::too_many_arguments)]
const fn hit(
    tone: f32,
    tone_end: f32,
    sweep: f32,
    tone_amp: f32,
    tone_len: f32,
    noise_amp: f32,
    noise_len: f32,
    noise_hp: f32,
    noise_lp: f32,
    choke: u8,
) -> Hit {
    Hit { tone, tone_end, sweep, tone_amp, tone_len, noise_amp, noise_len, noise_hp, noise_lp, choke }
}

/// General MIDI drum note numbers.
pub mod gm {
    pub const KICK: u8 = 36;
    pub const RIM: u8 = 37;
    pub const SNARE: u8 = 38;
    pub const CLAP: u8 = 39;
    pub const TOM_LOW: u8 = 41;
    pub const HAT_CLOSED: u8 = 42;
    pub const TOM_MID: u8 = 45;
    pub const HAT_OPEN: u8 = 46;
    pub const TOM_HIGH: u8 = 48;
    pub const CRASH: u8 = 49;
    pub const RIDE: u8 = 51;
    pub const TAMBOURINE: u8 = 54;
    pub const COWBELL: u8 = 56;
    pub const SHAKER: u8 = 70;
}

fn hit_for(pitch: u8, kit: DrumKit) -> Option<Hit> {
    let acoustic = kit == DrumKit::Acoustic;
    let boomy = kit == DrumKit::HipHop;
    Some(match pitch {
        35 | 36 => {
            if acoustic {
                hit(150.0, 52.0, 0.03, 1.0, 0.16, 0.25, 0.012, 800.0, 6000.0, 0)
            } else if boomy {
                hit(130.0, 42.0, 0.07, 1.1, 0.55, 0.1, 0.008, 1000.0, 5000.0, 0)
            } else {
                hit(170.0, 48.0, 0.04, 1.1, 0.28, 0.15, 0.006, 1500.0, 8000.0, 0)
            }
        }
        37 => hit(900.0, 700.0, 0.01, 0.5, 0.03, 0.4, 0.02, 1500.0, 7000.0, 0),
        38 | 40 => {
            if acoustic {
                hit(220.0, 180.0, 0.02, 0.5, 0.09, 0.8, 0.14, 900.0, 9000.0, 0)
            } else {
                hit(240.0, 170.0, 0.03, 0.55, 0.07, 0.75, 0.11, 1400.0, 10000.0, 0)
            }
        }
        39 => hit(0.0, 0.0, 0.01, 0.0, 0.01, 0.9, 0.16, 900.0, 3200.0, 0),
        41 | 43 => hit(125.0, 80.0, 0.08, 0.9, 0.3, 0.12, 0.03, 500.0, 4000.0, 0),
        45 | 47 => hit(170.0, 110.0, 0.07, 0.9, 0.26, 0.12, 0.03, 600.0, 4500.0, 0),
        48 | 50 => hit(230.0, 150.0, 0.06, 0.9, 0.22, 0.12, 0.03, 700.0, 5000.0, 0),
        42 | 44 => hit(0.0, 0.0, 0.01, 0.0, 0.01, 0.55, 0.035, 7000.0, 16000.0, 1),
        46 => hit(0.0, 0.0, 0.01, 0.0, 0.01, 0.5, 0.28, 6500.0, 16000.0, 1),
        49 | 57 => hit(0.0, 0.0, 0.01, 0.0, 0.01, 0.6, 0.9, 4500.0, 15000.0, 0),
        51 | 59 => hit(540.0, 540.0, 0.01, 0.12, 0.4, 0.35, 0.5, 6000.0, 14000.0, 0),
        53 => hit(800.0, 800.0, 0.01, 0.2, 0.5, 0.3, 0.3, 5000.0, 14000.0, 0),
        54 => hit(0.0, 0.0, 0.01, 0.0, 0.01, 0.45, 0.12, 8000.0, 16000.0, 0),
        56 => hit(560.0, 560.0, 0.01, 0.6, 0.12, 0.05, 0.02, 2000.0, 8000.0, 0),
        69 | 70 | 82 => hit(0.0, 0.0, 0.01, 0.0, 0.01, 0.35, 0.05, 5000.0, 12000.0, 0),
        60..=64 => hit(260.0 + 60.0 * f32::from(pitch - 60), 200.0, 0.02, 0.8, 0.12, 0.05, 0.01, 800.0, 5000.0, 0),
        75..=77 => hit(1800.0, 1800.0, 0.01, 0.6, 0.04, 0.1, 0.01, 2000.0, 9000.0, 0),
        _ => return None,
    })
}

#[derive(Clone, Copy)]
struct DrumVoice {
    hit: Hit,
    active: bool,
    t: f32,
    phase: f32,
    vel: f32,
    hp: f32,
    lp: f32,
}

pub struct DrumSynth {
    params: DrumParams,
    sample_rate: f32,
    voices: [DrumVoice; 16],
    rng: Rng,
    next: usize,
}

impl DrumSynth {
    pub fn new(params: DrumParams, sample_rate: f32) -> Self {
        let silent = DrumVoice {
            hit: hit(0.0, 0.0, 0.01, 0.0, 0.01, 0.0, 0.01, 100.0, 1000.0, 0),
            active: false,
            t: 0.0,
            phase: 0.0,
            vel: 0.0,
            hp: 0.0,
            lp: 0.0,
        };
        Self { params, sample_rate, voices: [silent; 16], rng: Rng(0x9E37_79B9), next: 0 }
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8) {
        let Some(mut hit) = hit_for(pitch, self.params.kit) else { return };
        let tune = 2f32.powf(self.params.tune / 12.0);
        let decay = self.params.decay.clamp(0.2, 4.0);
        hit.tone *= tune;
        hit.tone_end *= tune;
        hit.tone_len *= decay;
        hit.noise_len *= decay;
        if hit.choke != 0 {
            for v in &mut self.voices {
                if v.active && v.hit.choke == hit.choke {
                    v.active = false;
                }
            }
        }
        let slot = self.voices.iter().position(|v| !v.active).unwrap_or_else(|| {
            self.next = (self.next + 1) % self.voices.len();
            self.next
        });
        self.voices[slot] = DrumVoice {
            hit,
            active: true,
            t: 0.0,
            phase: 0.0,
            vel: (f32::from(velocity.min(127)) / 127.0).powf(1.5),
            hp: 0.0,
            lp: 0.0,
        };
    }

    pub fn is_silent(&self) -> bool {
        self.voices.iter().all(|v| !v.active)
    }

    pub fn render(&mut self, out: &mut [f32]) {
        let sr = self.sample_rate;
        let dt = 1.0 / sr;
        let gain = self.params.gain;
        for v in &mut self.voices {
            if !v.active {
                continue;
            }
            let h = v.hit;
            let hp_a = 1.0 - (-TAU * h.noise_hp / sr).exp();
            let lp_a = 1.0 - (-TAU * h.noise_lp.min(sr * 0.45) / sr).exp();
            let life = 7.0 * h.tone_len.max(h.noise_len);
            for sample in out.iter_mut() {
                let mut s = 0.0;
                if h.tone_amp > 0.0 {
                    let hz = h.tone_end + (h.tone - h.tone_end) * (-v.t / h.sweep).exp();
                    v.phase = (v.phase + hz * dt).fract();
                    s += (v.phase * TAU).sin() * h.tone_amp * (-v.t / h.tone_len).exp();
                }
                if h.noise_amp > 0.0 {
                    let noise = self.rng.bipolar();
                    v.hp += hp_a * (noise - v.hp);
                    v.lp += lp_a * ((noise - v.hp) - v.lp);
                    s += v.lp * h.noise_amp * (-v.t / h.noise_len).exp();
                }
                *sample += s * v.vel * gain;
                v.t += dt;
            }
            if v.t > life {
                v.active = false;
            }
        }
    }
}

// ----- sampled instruments (SoundFont) -------------------------------------------

/// Plays one preset of a SoundFont.
/// Semitones a full pitch bend moves a SoundFont channel.
const BEND_RANGE: i32 = 24;

pub struct Sampler {
    params: SamplerParams,
    synth: Synthesizer,
    channel: i32,
    left: Vec<f32>,
    right: Vec<f32>,
    held: u32,
    level: f32,
}

impl Sampler {
    fn new(params: SamplerParams, font: &Arc<SoundFont>, sample_rate: f32) -> Option<Self> {
        let mut settings = SynthesizerSettings::new(sample_rate as i32);
        // The mixer has its own reverb and chorus.
        settings.enable_reverb_and_chorus = false;
        let mut synth = Synthesizer::new(font, &settings).ok()?;
        // Channel 10 is the percussion channel, wired to bank 128.
        let channel = if params.bank == 128 { 9 } else { 0 };
        if channel == 0 {
            synth.process_midi_message(0, 0xB0, 0, i32::from(params.bank.min(127)));
        }
        synth.process_midi_message(channel, 0xC0, i32::from(params.preset), 0);
        // Pitch bend range: two octaves, so that slides reach any fret.
        synth.process_midi_message(channel, 0xB0, 101, 0);
        synth.process_midi_message(channel, 0xB0, 100, 0);
        synth.process_midi_message(channel, 0xB0, 6, BEND_RANGE);
        synth.process_midi_message(channel, 0xB0, 38, 0);
        Some(Self { params, synth, channel, left: vec![0.0; 256], right: vec![0.0; 256], held: 0, level: 0.0 })
    }

    /// Moves every note of the bank by `semitones`: a SoundFont player bends
    /// its channel as a whole.
    fn bend(&mut self, semitones: f32) {
        let value = (semitones / BEND_RANGE as f32 * 8192.0 + 8192.0).round().clamp(0.0, 16383.0) as i32;
        self.synth.process_midi_message(self.channel, 0xE0, value & 0x7F, value >> 7);
    }

    fn render(&mut self, out: &mut [f32]) {
        // SoundFont players leave a lot of headroom; this brings a bank in line
        // with the built-in sounds.
        let gain = self.params.gain * 3.0;
        for chunk in out.chunks_mut(256) {
            let n = chunk.len();
            self.synth.render(&mut self.left[..n], &mut self.right[..n]);
            let mut peak = 0.0f32;
            for (i, sample) in chunk.iter_mut().enumerate() {
                let s = 0.5 * (self.left[i] + self.right[i]) * gain;
                peak = peak.max(s.abs());
                *sample += s;
            }
            self.level = peak.max(self.level * 0.9);
        }
    }
}

// ----- one of the generators ------------------------------------------------------

// The built-in generators hold their voices in place: one of them per track,
// rendered a block at a time, so the room they take is no concern and a box
// would only add a step to every block.
#[allow(clippy::large_enum_variant)]
pub enum SoundRt {
    Synth(Synth),
    Pluck(PluckSynth),
    Drums(DrumSynth),
    Sampler(Box<Sampler>),
    Sfz(SamplerParams, Box<SfzSynth>),
    /// A sampled instrument whose file is not loaded: silence.
    Missing(SamplerParams),
}

impl SoundRt {
    pub fn new(sound: &Sound, sample_rate: f32, fonts: &Fonts) -> Self {
        match sound {
            Sound::Synth(p) => SoundRt::Synth(Synth::new(*p, sample_rate)),
            Sound::Pluck(p) => SoundRt::Pluck(PluckSynth::new(*p, sample_rate)),
            Sound::Drums(p) => SoundRt::Drums(DrumSynth::new(*p, sample_rate)),
            Sound::Sampler(p) => match fonts.get(&p.font) {
                Some(Bank::Sf2(font)) => {
                    Sampler::new(*p, font, sample_rate).map_or(SoundRt::Missing(*p), |s| SoundRt::Sampler(Box::new(s)))
                }
                Some(Bank::Sfz(instrument)) => {
                    SoundRt::Sfz(*p, Box::new(SfzSynth::new(Arc::clone(instrument), sample_rate, p.gain)))
                }
                None => SoundRt::Missing(*p),
            },
        }
    }

    /// Applies new settings, keeping the notes that are sounding when the kind
    /// of generator is unchanged.
    pub fn update(&mut self, sound: &Sound, sample_rate: f32, fonts: &Fonts) {
        match (self, sound) {
            (SoundRt::Synth(s), Sound::Synth(p)) => s.params = *p,
            (SoundRt::Pluck(s), Sound::Pluck(p)) => s.params = *p,
            (SoundRt::Drums(s), Sound::Drums(p)) => s.params = *p,
            (SoundRt::Sampler(s), Sound::Sampler(p)) if (s.params.font, s.params.bank, s.params.preset) == (p.font, p.bank, p.preset) => {
                s.params = *p;
            }
            (SoundRt::Sfz(old, s), Sound::Sampler(p)) if old.font == p.font => {
                s.gain = p.gain;
                *old = *p;
            }
            // A missing font may have been loaded since.
            (SoundRt::Missing(old), Sound::Sampler(p)) if old == p && !fonts.contains_key(&p.font) => {}
            (this, sound) => *this = SoundRt::new(sound, sample_rate, fonts),
        }
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8) {
        match self {
            SoundRt::Synth(s) => s.note_on(pitch, velocity),
            SoundRt::Pluck(s) => s.note_on(pitch, velocity),
            SoundRt::Drums(s) => s.note_on(pitch, velocity),
            SoundRt::Sampler(s) => {
                s.held += 1;
                s.level = 1.0;
                s.synth.note_on(s.channel, i32::from(pitch), i32::from(velocity.clamp(1, 127)));
            }
            SoundRt::Sfz(_, s) => s.note_on(pitch, velocity),
            SoundRt::Missing(_) => {}
        }
    }

    pub fn note_off(&mut self, pitch: u8) {
        match self {
            SoundRt::Synth(s) => s.note_off(pitch),
            SoundRt::Pluck(s) => s.note_off(pitch),
            SoundRt::Sampler(s) => {
                s.held = s.held.saturating_sub(1);
                s.synth.note_off(s.channel, i32::from(pitch));
            }
            SoundRt::Sfz(_, s) => s.note_off(pitch),
            SoundRt::Drums(_) | SoundRt::Missing(_) => {}
        }
    }

    pub fn release_all(&mut self) {
        self.unbend();
        match self {
            SoundRt::Synth(s) => s.release_all(),
            SoundRt::Pluck(s) => s.release_all(),
            SoundRt::Sampler(s) => {
                s.held = 0;
                s.synth.note_off_all(false);
            }
            SoundRt::Sfz(_, s) => s.release_all(),
            SoundRt::Drums(_) | SoundRt::Missing(_) => {}
        }
    }

    /// Moves the notes sounding at `pitch` by `semitones`, for a bend, a
    /// slide or a vibrato. A SoundFont bank moves all its notes at once.
    pub fn bend(&mut self, pitch: u8, semitones: f32) {
        match self {
            SoundRt::Synth(s) => s.bend(pitch, semitones),
            SoundRt::Pluck(s) => s.bend(pitch, semitones),
            SoundRt::Sampler(s) => s.bend(semitones),
            SoundRt::Sfz(_, s) => s.bend(pitch, semitones),
            SoundRt::Drums(_) | SoundRt::Missing(_) => {}
        }
    }

    /// Puts every note back at its own pitch.
    pub fn unbend(&mut self) {
        match self {
            SoundRt::Synth(s) => s.unbend(),
            SoundRt::Pluck(s) => s.unbend(),
            SoundRt::Sampler(s) => s.bend(0.0),
            SoundRt::Sfz(_, s) => s.unbend(),
            SoundRt::Drums(_) | SoundRt::Missing(_) => {}
        }
    }

    pub fn is_silent(&self) -> bool {
        match self {
            SoundRt::Synth(s) => s.is_silent(),
            SoundRt::Pluck(s) => s.is_silent(),
            SoundRt::Drums(s) => s.is_silent(),
            SoundRt::Sampler(s) => s.held == 0 && s.level < 1e-4,
            SoundRt::Sfz(_, s) => s.is_silent(),
            SoundRt::Missing(_) => true,
        }
    }

    /// Adds the mono output to `out`.
    pub fn render(&mut self, out: &mut [f32]) {
        match self {
            SoundRt::Synth(s) => s.render(out),
            SoundRt::Pluck(s) => s.render(out),
            SoundRt::Drums(s) => s.render(out),
            SoundRt::Sampler(s) => s.render(out),
            SoundRt::Sfz(_, s) => s.render(out),
            SoundRt::Missing(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(sound: Sound, pitch: u8) -> (Vec<f32>, SoundRt) {
        let mut rt = SoundRt::new(&sound, 48_000.0, &Fonts::new());
        rt.note_on(pitch, 100);
        let mut buf = vec![0.0f32; 9600];
        rt.render(&mut buf);
        (buf, rt)
    }

    #[test]
    fn every_generator_makes_sound_and_falls_silent() {
        for (sound, pitch) in [
            (Sound::Synth(SynthParams::default()), 69),
            (Sound::Pluck(PluckParams::default()), 57),
            (Sound::Drums(DrumParams::default()), gm::SNARE),
        ] {
            let (buf, mut rt) = rendered(sound, pitch);
            assert!(buf.iter().any(|s| s.abs() > 0.01));
            assert!(buf.iter().all(|s| s.is_finite()));
            rt.note_off(pitch);
            let mut tail = vec![0.0f32; 48_000 * 8];
            rt.render(&mut tail);
            assert!(rt.is_silent());
        }
    }

    #[test]
    fn plucked_string_is_in_tune() {
        // The string is tuned with a fractional delay: check low and high notes.
        for pitch in [40u8, 57, 69, 81] {
            let (_, mut rt) = rendered(Sound::Pluck(PluckParams { brightness: 0.2, ..Default::default() }), pitch);
            let mut buf = vec![0.0f32; 8192];
            rt.render(&mut buf);
            let expected = pitch_to_hz(f32::from(pitch));
            let found = crate::analysis::detect_pitch(&buf, 48_000.0).unwrap();
            assert!((found - expected).abs() < expected * 0.006, "{pitch}: {found} vs {expected}");
        }
    }

    #[test]
    fn more_notes_than_voices_does_not_panic() {
        let mut synth = Synth::new(SynthParams::default(), 44_100.0);
        for pitch in 20..(20 + MAX_VOICES as u8 + 10) {
            synth.note_on(pitch, 90);
        }
        let mut drums = DrumSynth::new(DrumParams::default(), 44_100.0);
        for _ in 0..40 {
            drums.note_on(gm::KICK, 100);
            drums.note_on(200, 100);
        }
        let mut buf = vec![0.0f32; 512];
        synth.render(&mut buf);
        drums.render(&mut buf);
        assert!(buf.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn closed_hat_cuts_the_open_hat() {
        let mut drums = DrumSynth::new(DrumParams::default(), 48_000.0);
        drums.note_on(gm::HAT_OPEN, 100);
        drums.note_on(gm::HAT_CLOSED, 100);
        assert_eq!(drums.voices.iter().filter(|v| v.active).count(), 1);
    }
}
#[cfg(test)]
mod bend_tests {
    use super::*;
    use crate::model::{PluckParams, SynthParams, Wave};

    fn crossings(samples: &[f32]) -> usize {
        samples.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
    }

    #[test]
    fn a_bend_moves_a_sounding_note_and_unbend_brings_it_back() {
        // A pure sine: two crossings a cycle, nothing else.
        let params = SynthParams { wave: Wave::Sine, detune_cents: 0.0, sub: 0.0, vibrato: 0.0, ..SynthParams::default() };
        let mut synth = Synth::new(params, 48000.0);
        synth.note_on(69, 100);
        let mut out = vec![0.0; 48000];
        synth.render(&mut out);
        out.fill(0.0);
        synth.render(&mut out);
        let plain = crossings(&out);
        synth.bend(69, 12.0);
        out.fill(0.0);
        synth.render(&mut out);
        let bent = crossings(&out);
        assert!((bent as f32 / plain as f32 - 2.0).abs() < 0.1, "an octave up: twice the crossings ({plain} then {bent})");
        synth.unbend();
        out.fill(0.0);
        synth.render(&mut out);
        assert!((crossings(&out) as f32 / plain as f32 - 1.0).abs() < 0.05);
        synth.bend(70, 12.0);
        out.fill(0.0);
        synth.render(&mut out);
        assert!((crossings(&out) as f32 / plain as f32 - 1.0).abs() < 0.05, "another pitch: untouched");

        // A plucked string: the loop is shortened while it rings.
        let mut pluck = PluckSynth::new(PluckParams { brightness: 0.2, ..PluckParams::default() }, 48000.0);
        pluck.note_on(57, 100);
        let mut out = vec![0.0; 8192];
        pluck.render(&mut out);
        pluck.bend(57, 12.0);
        out.fill(0.0);
        pluck.render(&mut out);
        let found = crate::analysis::detect_pitch(&out, 48_000.0).unwrap();
        let expected = pitch_to_hz(69.0);
        assert!((found - expected).abs() < expected * 0.02, "{found} vs {expected}");
        pluck.unbend();
        out.fill(0.0);
        pluck.render(&mut out);
        let found = crate::analysis::detect_pitch(&out, 48_000.0).unwrap();
        let expected = pitch_to_hz(57.0);
        assert!((found - expected).abs() < expected * 0.02, "{found} vs {expected}");
    }
}
