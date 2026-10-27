//! Effects: equaliser, compressor, amplifier, pedals, echo and reverb.
//! Filter formulas are the widely published "audio EQ cookbook" biquads.

use std::f32::consts::{PI, TAU};

use crate::capture::{CaptureRt, Captures};
use crate::convolution::Convolver;
use crate::model::{Amp, AmpModel, Eq, MasterFx, Pedal, PedalKind, ReverbKind, TrackFx};

fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

/// Most samples a block has, for the buffers kept from one block to the next.
const MOST_BLOCK: usize = 8192;

#[inline]
fn ln_cosh(x: f32) -> f32 {
    let a = x.abs();
    a + (1.0 + (-2.0 * a).exp()).ln() - std::f32::consts::LN_2
}

/// Soft clipping as a valve stage does it: more on one side than the other.
#[inline]
fn valve(y: f32) -> f32 {
    if y >= 0.0 { y.tanh() } else { (y * 0.8).tanh() * 1.1 }
}

/// The antiderivative of `valve`, continuous at zero.
#[inline]
fn valve_sum(y: f32) -> f32 {
    if y >= 0.0 { ln_cosh(y) } else { ln_cosh(0.8 * y) * (1.1 / 0.8) }
}

/// Runs a waveshaper by the mean of its antiderivative over the step from
/// the last sample (Parker, Zavalishin and Le Bivic, "Reducing the aliasing
/// of nonlinear waveshaping using continuous-time convolution", 2016): the
/// same curve, far less aliasing, no oversampling.
#[derive(Clone, Copy, Default)]
struct Adaa {
    x1: f32,
    sum1: f32,
}

impl Adaa {
    #[inline]
    fn run(&mut self, x: f32, shape: fn(f32) -> f32, sum: fn(f32) -> f32) -> f32 {
        let step = x - self.x1;
        let s = sum(x);
        let y = if step.abs() < 1e-4 { shape(0.5 * (x + self.x1)) } else { (s - self.sum1) / step };
        self.x1 = x;
        self.sum1 = s;
        if y.is_finite() { y } else { 0.0 }
    }
}

#[derive(Clone, Copy, Default)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    fn set(&mut self, b0: f32, b1: f32, b2: f32, a0: f32, a1: f32, a2: f32) {
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1 / a0;
        self.a2 = a2 / a0;
    }

    /// Gain of the filter at a frequency, in decibels.
    pub fn gain_db(&self, sr: f32, hz: f32) -> f32 {
        let w = TAU * (hz / sr).clamp(0.0, 0.5);
        let (sin1, cos1) = w.sin_cos();
        let (sin2, cos2) = (2.0 * w).sin_cos();
        // The filter's formula with z set on the unit circle.
        let top = (self.b0 + self.b1 * cos1 + self.b2 * cos2).hypot(self.b1 * sin1 + self.b2 * sin2);
        let bottom = (1.0 + self.a1 * cos1 + self.a2 * cos2).hypot(self.a1 * sin1 + self.a2 * sin2);
        20.0 * (top / bottom.max(1e-9)).max(1e-9).log10()
    }

    pub fn low_pass(&mut self, sr: f32, hz: f32, q: f32) {
        let w = TAU * (hz / sr).clamp(0.0005, 0.49);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        self.set((1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha);
    }

    pub fn high_pass(&mut self, sr: f32, hz: f32, q: f32) {
        let w = TAU * (hz / sr).clamp(0.0005, 0.49);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        self.set((1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0, 1.0 + alpha, -2.0 * cos, 1.0 - alpha);
    }

    pub fn peaking(&mut self, sr: f32, hz: f32, q: f32, db: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = TAU * (hz / sr).clamp(0.0005, 0.49);
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        self.set(1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a);
    }

    pub fn low_shelf(&mut self, sr: f32, hz: f32, db: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = TAU * (hz / sr).clamp(0.0005, 0.49);
        let (sin, cos) = w.sin_cos();
        let beta = 2.0 * a.sqrt() * sin / (2.0 * 0.707);
        self.set(
            a * ((a + 1.0) - (a - 1.0) * cos + beta),
            2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
            a * ((a + 1.0) - (a - 1.0) * cos - beta),
            (a + 1.0) + (a - 1.0) * cos + beta,
            -2.0 * ((a - 1.0) + (a + 1.0) * cos),
            (a + 1.0) + (a - 1.0) * cos - beta,
        );
    }

    pub fn high_shelf(&mut self, sr: f32, hz: f32, db: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = TAU * (hz / sr).clamp(0.0005, 0.49);
        let (sin, cos) = w.sin_cos();
        let beta = 2.0 * a.sqrt() * sin / (2.0 * 0.707);
        self.set(
            a * ((a + 1.0) + (a - 1.0) * cos + beta),
            -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
            a * ((a + 1.0) + (a - 1.0) * cos - beta),
            (a + 1.0) - (a - 1.0) * cos + beta,
            2.0 * ((a - 1.0) - (a + 1.0) * cos),
            (a + 1.0) - (a - 1.0) * cos - beta,
        );
    }

    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        // Keeps denormals and any runaway state out of the filter.
        // Written this way so that NaN, which compares false, is caught too.
        #[allow(clippy::neg_cmp_op_on_partial_ord)]
        let runaway = !(self.z1.abs() < 1e6);
        if runaway {
            self.z1 = 0.0;
            self.z2 = 0.0;
        }
        y
    }
}

/// The curve of an equaliser at a frequency, in decibels, without running it.
pub fn eq_response_db(eq: &Eq, sr: f32, hz: f32) -> f32 {
    let mut rt = EqRt::default();
    rt.configure(eq, sr);
    rt.response_db(sr, hz)
}

/// Three-band equaliser, one instance per channel.
#[derive(Default)]
pub struct EqRt {
    bands: [Biquad; 3],
    applied: Option<Eq>,
}

impl EqRt {
    pub fn configure(&mut self, eq: &Eq, sr: f32) {
        if self.applied == Some(*eq) {
            return;
        }
        self.bands[0].low_shelf(sr, eq.low_freq.clamp(30.0, 600.0), eq.low.clamp(-24.0, 24.0));
        self.bands[1].peaking(sr, eq.mid_freq.clamp(100.0, 8000.0), 0.9, eq.mid.clamp(-24.0, 24.0));
        self.bands[2].high_shelf(sr, eq.high_freq.clamp(1000.0, 18_000.0), eq.high.clamp(-24.0, 24.0));
        self.applied = Some(*eq);
    }

    /// How much the equaliser, as last configured, raises or lowers a
    /// frequency, in decibels: the curve an equaliser's display draws.
    pub fn response_db(&self, sr: f32, hz: f32) -> f32 {
        self.bands.iter().map(|band| band.gain_db(sr, hz)).sum()
    }

    #[inline]
    pub fn run(&mut self, x: f32) -> f32 {
        let x = self.bands[0].run(x);
        let x = self.bands[1].run(x);
        self.bands[2].run(x)
    }
}

/// Feed-forward compressor driven by a single "amount" knob.
#[derive(Default)]
pub struct Compressor {
    env: f32,
}

impl Compressor {
    /// Returns the gain to apply for an input of level `level` (absolute value).
    #[inline]
    pub fn gain(&mut self, level: f32, amount: f32, sr: f32) -> f32 {
        let attack = (-1.0 / (0.005 * sr)).exp();
        let release = (-1.0 / (0.12 * sr)).exp();
        let coef = if level > self.env { attack } else { release };
        self.env = level + coef * (self.env - level);
        let threshold = db_to_gain(-6.0 - 24.0 * amount);
        let ratio = 1.0 + 5.0 * amount;
        let makeup = db_to_gain(9.0 * amount);
        if self.env <= threshold {
            makeup
        } else {
            let over = self.env / threshold;
            over.powf(1.0 / ratio - 1.0) * makeup
        }
    }
}

struct DelayLine {
    buf: Vec<f32>,
    pos: usize,
}

impl DelayLine {
    fn new(len: usize) -> Self {
        Self { buf: vec![0.0; len.max(2)], pos: 0 }
    }

    /// Reads the sample written `delay` samples ago (fractional).
    #[inline]
    fn read(&self, delay: f32) -> f32 {
        let len = self.buf.len();
        let delay = delay.clamp(1.0, (len - 2) as f32);
        let at = self.pos as f32 + len as f32 - delay;
        let i = at as usize;
        let frac = at - i as f32;
        let a = self.buf[i % len];
        let b = self.buf[(i + 1) % len];
        a + (b - a) * frac
    }

    #[inline]
    fn write(&mut self, x: f32) {
        self.buf[self.pos] = x;
        self.pos = (self.pos + 1) % self.buf.len();
    }
}

/// A state-variable filter the way Zavalishin writes it ("The Art of VA
/// Filter Design", the topology-preserving transform): the band-pass of
/// `x` around `hz`, the narrower the higher `q`. `state` is its two memories.
#[inline]
fn band_pass(state: &mut [f32; 2], x: f32, hz: f32, q: f32, sr: f32) -> f32 {
    let g = (PI * (hz / sr).clamp(0.0005, 0.45)).tan();
    let k = 1.0 / q.max(0.1);
    let a1 = 1.0 / (1.0 + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;
    let v3 = x - state[1];
    let v1 = a1 * state[0] + a2 * v3;
    let v2 = state[1] + a2 * state[0] + a3 * v3;
    state[0] = 2.0 * v1 - state[0];
    state[1] = 2.0 * v2 - state[1];
    if state[0].is_finite() && state[1].is_finite() {
        v1
    } else {
        *state = [0.0; 2];
        0.0
    }
}

struct PedalRt {
    delay: DelayLine,
    lfo: f32,
    lp: f32,
    ap: [f32; 4],
    adaa: Adaa,
    /// The loudness followed, the gate's opening, the filter's memories, a
    /// slow wobble, a high-pass memory and the saturation of the echoes.
    env: f32,
    gate: f32,
    svf: [f32; 2],
    wow: f32,
    hp: f32,
    sat: Adaa,
}

impl PedalRt {
    fn new(sr: f32) -> Self {
        Self {
            delay: DelayLine::new((sr * 0.8) as usize),
            lfo: 0.0,
            lp: 0.0,
            ap: [0.0; 4],
            adaa: Adaa::default(),
            env: 0.0,
            gate: 0.0,
            svf: [0.0; 2],
            wow: 0.0,
            hp: 0.0,
            sat: Adaa::default(),
        }
    }

    #[inline]
    fn run(&mut self, pedal: &Pedal, x: f32, sr: f32) -> f32 {
        let amount = pedal.amount.clamp(0.0, 1.0);
        let rate = pedal.rate.clamp(0.0, 1.0);
        match pedal.kind {
            PedalKind::Overdrive => {
                let drive = 1.0 + amount * 40.0;
                let y = self.adaa.run(x * drive, f32::tanh, ln_cosh) / drive.sqrt().max(1.0) * 2.0;
                // The second knob is the tone.
                let a = 1.0 - (-TAU * (800.0 + rate * 7000.0) / sr).exp();
                self.lp += a * (y - self.lp);
                self.lp
            }
            PedalKind::Chorus => {
                self.lfo = (self.lfo + (0.2 + rate * 4.0) / sr).fract();
                let sweep = 0.012 + 0.006 * amount * (self.lfo * TAU).sin();
                let wet = self.delay.read(sweep * sr);
                self.delay.write(x);
                x + wet * (0.3 + 0.5 * amount)
            }
            PedalKind::Tremolo => {
                self.lfo = (self.lfo + (1.0 + rate * 11.0) / sr).fract();
                x * (1.0 - amount * (0.5 + 0.5 * (self.lfo * TAU).sin()))
            }
            PedalKind::Phaser => {
                self.lfo = (self.lfo + (0.1 + rate * 3.0) / sr).fract();
                let hz = 300.0 + 1700.0 * (0.5 + 0.5 * (self.lfo * TAU).sin());
                let t = (PI * hz / sr).tan();
                let c = (t - 1.0) / (t + 1.0);
                let mut y = x;
                for state in &mut self.ap {
                    let out = c * y + *state;
                    *state = y - c * out;
                    y = out;
                }
                x + y * amount
            }
            PedalKind::Echo => {
                let wet = self.delay.read((0.06 + rate * 0.6) * sr);
                self.delay.write(x + wet * amount * 0.7);
                x + wet * amount * 0.6
            }
            PedalKind::Gate => {
                // The loudness, followed at once up and slowly down; the gate
                // opens above the threshold in a millisecond and closes in the
                // release time.
                let level = x.abs();
                let fall = (-1.0 / (0.05 * sr)).exp();
                self.env = if level > self.env { level } else { level + fall * (self.env - level) };
                let threshold = db_to_gain(-70.0 + amount * 55.0);
                let open = if self.env > threshold { 1.0 } else { 0.0 };
                let coef = if open > self.gate { (-1.0 / (0.001 * sr)).exp() } else { (-1.0 / ((0.02 + rate * 0.4) * sr)).exp() };
                self.gate = open + coef * (self.gate - open);
                x * self.gate
            }
            PedalKind::Wah => {
                // The pedal from heel to toe: 350 Hz to 2.1 kHz.
                let hz = 350.0 * 2f32.powf(amount * 2.6);
                band_pass(&mut self.svf, x, hz, 1.5 + rate * 8.0, sr) * 2.0 + x * 0.25
            }
            PedalKind::AutoWah => {
                // The loudness of the playing moves the pedal.
                let level = x.abs();
                let (up, down) = ((-1.0 / (0.004 * sr)).exp(), (-1.0 / ((0.05 + (1.0 - rate) * 0.4) * sr)).exp());
                let coef = if level > self.env { up } else { down };
                self.env = level + coef * (self.env - level);
                let position = (self.env * (2.0 + amount * 14.0)).clamp(0.0, 1.0);
                let hz = 300.0 * 2f32.powf(position * 2.6);
                band_pass(&mut self.svf, x, hz, 4.0, sr) * 2.0 + x * 0.25
            }
            PedalKind::Flanger => {
                // A very short delay that sweeps, fed back on itself.
                self.lfo = (self.lfo + (0.1 + rate * 2.0) / sr).fract();
                let sweep = 0.0015 + 0.0035 * (0.5 + 0.5 * (self.lfo * TAU).sin());
                let wet = self.delay.read(sweep * sr);
                self.delay.write(x + wet * amount * 0.7);
                x + wet * (0.4 + 0.4 * amount)
            }
            PedalKind::TapeEcho => {
                // The tape: its speed wobbles a little, each pass saturates a
                // touch and loses its highs and its lows.
                self.wow = (self.wow + 0.6 / sr).fract();
                let time = (0.08 + rate * 0.7) * (1.0 + 0.002 * (self.wow * TAU).sin());
                let wet = self.delay.read(time * sr);
                let a = 1.0 - (-TAU * 4000.0 / sr).exp();
                self.lp += a * (wet - self.lp);
                let b = 1.0 - (-TAU * 120.0 / sr).exp();
                self.hp += b * (self.lp - self.hp);
                let back = self.sat.run((self.lp - self.hp) * 1.5, f32::tanh, ln_cosh) / 1.5;
                self.delay.write(x + back * amount * 0.8);
                x + wet * 0.7
            }
            PedalKind::BbdEcho => {
                // The bucket brigade: darker at each pass, a little grit, the
                // faint shimmer of its clock.
                self.wow = (self.wow + 2.0 / sr).fract();
                let time = (0.04 + rate * 0.46) * (1.0 + 0.0008 * (self.wow * TAU).sin());
                let wet = self.delay.read(time * sr);
                let a = 1.0 - (-TAU * 2500.0 / sr).exp();
                self.lp += a * (wet - self.lp);
                let back = self.sat.run(self.lp * 2.0, f32::tanh, ln_cosh) / 2.0;
                self.delay.write(x + back * amount * 0.85);
                x + wet * 0.7
            }
        }
    }
}

#[derive(Default)]
struct AmpRt {
    input_hp: Biquad,
    tone: [Biquad; 4],
    cab: [Biquad; 2],
    applied: Option<Amp>,
    adaa: Adaa,
    /// The capture that takes the place of the drive stage, by the path of
    /// its file, and whether it holds its own loudspeaker.
    capture: Option<CaptureRt>,
    capture_path: String,
    capture_has_cab: bool,
    /// The impulse that takes the place of the loudspeaker filters, by path.
    cabinet: Option<Convolver>,
    cabinet_path: String,
}

impl AmpRt {
    fn configure(&mut self, amp: &Amp, capture: &str, cabinet: &str, captures: &Captures, sr: f32) {
        if capture != self.capture_path {
            let found = captures.gear.get(capture);
            self.capture = found.and_then(|c| c.start(sr, crate::engine::BLOCK).ok());
            self.capture_has_cab = found.is_some_and(|c| c.about.has_cab == Some(true));
            self.capture_path = capture.to_owned();
        }
        if cabinet != self.cabinet_path {
            self.cabinet = captures.impulses.get(cabinet).and_then(|i| i.channels.first().map(|c| Convolver::new(c, i.sample_rate, sr)));
            self.cabinet_path = cabinet.to_owned();
        }
        if self.applied == Some(*amp) {
            return;
        }
        let bass_amp = amp.model == AmpModel::Bass;
        self.input_hp.high_pass(sr, if bass_amp { 35.0 } else { 80.0 }, 0.707);
        self.tone[0].low_shelf(sr, 130.0, (amp.bass - 0.5) * 24.0);
        self.tone[1].peaking(sr, 700.0, 0.8, (amp.mid - 0.5) * 20.0);
        self.tone[2].high_shelf(sr, 3000.0, (amp.treble - 0.5) * 24.0);
        self.tone[3].peaking(sr, 4500.0, 1.0, (amp.presence - 0.5) * 16.0);
        // The loudspeaker: little below 70 Hz, little above a few kHz.
        self.cab[0].high_pass(sr, if bass_amp { 40.0 } else { 75.0 }, 0.707);
        self.cab[1].low_pass(sr, if bass_amp { 3200.0 } else { 5200.0 }, 0.8);
        self.applied = Some(*amp);
    }

    /// Forgets the files loaded, so that the next `configure` reads them again.
    fn reload(&mut self) {
        self.capture = None;
        self.capture_path.clear();
        self.cabinet = None;
        self.cabinet_path.clear();
    }

    fn process(&mut self, amp: &Amp, buf: &mut [f32]) {
        let master = amp.master.clamp(0.0, 1.0) * 1.4;
        if let Some(capture) = &mut self.capture {
            // The capture expects the instrument as it is; the gain knob sets
            // its level, from -20 to +20 dB.
            let level = db_to_gain((amp.gain.clamp(0.0, 1.0) - 0.5) * 40.0);
            for s in buf.iter_mut() {
                *s *= level;
            }
            capture.process(buf);
            for s in buf.iter_mut() {
                let mut y = *s;
                for band in &mut self.tone {
                    y = band.run(y);
                }
                *s = y * master;
            }
        } else {
            let drive = match amp.model {
                AmpModel::Clean => 1.5,
                AmpModel::Crunch => 8.0,
                AmpModel::Lead => 30.0,
                AmpModel::Bass => 2.5,
            } * (0.2 + amp.gain.clamp(0.0, 1.0) * 2.0);
            let level = master / drive.sqrt().max(1.0);
            for s in buf.iter_mut() {
                // Asymmetric soft clipping, like a valve stage.
                let mut y = self.adaa.run(self.input_hp.run(*s) * drive, valve, valve_sum);
                for band in &mut self.tone {
                    y = band.run(y);
                }
                *s = y * level;
            }
        }
        // The loudspeaker: the impulse of a cabinet, or else the built-in
        // filters, unless the capture holds its own.
        if let Some(cabinet) = &mut self.cabinet {
            cabinet.process(buf);
        } else if self.capture.is_none() || !self.capture_has_cab {
            for s in buf.iter_mut() {
                let first = self.cab[0].run(*s);
                *s = self.cab[1].run(first);
            }
        }
    }
}

/// Everything applied to one track before the fader.
pub struct TrackFxRt {
    sr: f32,
    eq: EqRt,
    comp: Compressor,
    amp: AmpRt,
    pedals: Vec<PedalRt>,
}

impl TrackFxRt {
    pub fn new(sr: f32) -> Self {
        Self { sr, eq: EqRt::default(), comp: Compressor::default(), amp: AmpRt::default(), pedals: Vec::new() }
    }

    /// Called from the UI thread when the project changes (it may allocate).
    /// `captures` holds the files the amp may refer to.
    pub fn configure(&mut self, fx: &TrackFx, captures: &Captures) {
        self.eq.configure(&fx.eq, self.sr);
        self.amp.configure(&fx.amp, &fx.capture, &fx.cabinet, captures, self.sr);
        while self.pedals.len() < fx.pedals.len() {
            self.pedals.push(PedalRt::new(self.sr));
        }
        self.pedals.truncate(fx.pedals.len());
    }

    /// Forgets the files loaded, so that the next `configure` reads them again.
    pub fn reload(&mut self) {
        self.amp.reload();
    }

    pub fn process(&mut self, fx: &TrackFx, buf: &mut [f32]) {
        let sr = self.sr;
        for (pedal, rt) in fx.pedals.iter().zip(&mut self.pedals) {
            if pedal.on {
                for s in buf.iter_mut() {
                    *s = rt.run(pedal, *s, sr);
                }
            }
        }
        if fx.amp.on {
            self.amp.process(&fx.amp, buf);
        }
        if fx.eq.on {
            for s in buf.iter_mut() {
                *s = self.eq.run(*s);
            }
        }
        if fx.comp > 0.001 {
            for s in buf.iter_mut() {
                *s *= self.comp.gain(s.abs(), fx.comp.min(1.0), sr);
            }
        }
    }
}

// ----- reverb --------------------------------------------------------------------

struct Comb {
    line: DelayLine,
    len: f32,
    damp: f32,
}

struct ReverbChannel {
    combs: Vec<Comb>,
    allpass: Vec<DelayLine>,
}

impl ReverbChannel {
    fn new(sr: f32, spread: f32) -> Self {
        let scale = sr / 44_100.0;
        let combs = [1117.0, 1193.0, 1277.0, 1361.0, 1433.0, 1511.0]
            .iter()
            .map(|len| {
                let len: f32 = (len + spread) * scale;
                Comb { line: DelayLine::new(len as usize + 4), len, damp: 0.0 }
            })
            .collect();
        let allpass = [557.0, 443.0, 349.0]
            .iter()
            .map(|len| DelayLine::new(((len + spread) * scale) as usize))
            .collect();
        Self { combs, allpass }
    }

    #[inline]
    fn run(&mut self, x: f32, feedback: f32, damp: f32) -> f32 {
        let mut out = 0.0;
        for comb in &mut self.combs {
            let y = comb.line.read(comb.len);
            comb.damp = y * (1.0 - damp) + comb.damp * damp;
            comb.line.write(x + comb.damp * feedback);
            out += y;
        }
        for ap in &mut self.allpass {
            let len = (ap.buf.len() - 2) as f32;
            let delayed = ap.read(len);
            let input = out + delayed * 0.5;
            ap.write(input);
            out = delayed - input * 0.5;
        }
        out * 0.12
    }
}

/// The rate Dattorro's figures are given at.
const PLATE_RATE: f32 = 29_761.0;

/// An allpass on a delay line: `len` samples, coefficient `g`.
#[inline]
fn allpass(line: &mut DelayLine, len: f32, g: f32, x: f32) -> f32 {
    let d = line.read(len);
    let w = x + g * d;
    line.write(w);
    d - g * w
}

/// One side of the plate's tank: a wobbling allpass, a delay, a damping
/// filter, an allpass, a delay.
struct Side {
    ap1: DelayLine,
    delay1: DelayLine,
    damp: f32,
    ap2: DelayLine,
    delay2: DelayLine,
}

/// Dattorro's plate reverb ("Effect Design, Part 1: Reverberator and Other
/// Filters", JAES 1997): the input diffused by four allpasses, then a tank
/// of two sides in a figure of eight, each feeding the other, the output
/// tapped at seven points of the tank a side. The lengths are the paper's,
/// scaled to the rate.
struct Plate {
    bandwidth: f32,
    diffusers: [DelayLine; 4],
    tank: [Side; 2],
    lfo: f32,
    scale: f32,
}

impl Plate {
    fn new(sr: f32) -> Self {
        let scale = sr / PLATE_RATE;
        let line = |len: f32| DelayLine::new((len * scale) as usize + 64);
        let side = |ap1: f32, delay1: f32, ap2: f32, delay2: f32| Side { ap1: line(ap1), delay1: line(delay1), damp: 0.0, ap2: line(ap2), delay2: line(delay2) };
        Self {
            bandwidth: 0.0,
            diffusers: [line(142.0), line(107.0), line(379.0), line(277.0)],
            tank: [side(672.0, 4453.0, 1800.0, 3720.0), side(908.0, 4217.0, 2656.0, 3163.0)],
            lfo: 0.0,
            scale,
        }
    }

    /// One sample in, the left and the right of the plate out. `decay` is
    /// how much comes round again, `damping` how much of the highs is lost
    /// at each pass, both 0 to 1.
    #[inline]
    fn run(&mut self, x: f32, decay: f32, damping: f32, sr: f32) -> (f32, f32) {
        let s = self.scale;
        self.bandwidth += 0.9995 * (x - self.bandwidth);
        let mut v = self.bandwidth;
        for (line, (len, g)) in self.diffusers.iter_mut().zip([(142.0, 0.75), (107.0, 0.75), (379.0, 0.625), (277.0, 0.625)]) {
            v = allpass(line, len * s, g, v);
        }
        self.lfo = (self.lfo + 1.0 / sr).fract();
        let wobble = (self.lfo * TAU).sin() * 8.0 * s;
        // Each side takes the input and the end of the other side.
        let ends = [self.tank[1].delay2.read(3163.0 * s), self.tank[0].delay2.read(3720.0 * s)];
        for (i, side) in self.tank.iter_mut().enumerate() {
            let (ap1, delay1, ap2, sign) = if i == 0 { (672.0, 4453.0, 1800.0, 1.0) } else { (908.0, 4217.0, 2656.0, -1.0) };
            let t = allpass(&mut side.ap1, ap1 * s + wobble * sign, -0.7, v + ends[i] * decay);
            side.delay1.write(t);
            let d1 = side.delay1.read(delay1 * s);
            side.damp += (1.0 - damping) * (d1 - side.damp);
            let t = allpass(&mut side.ap2, ap2 * s, 0.5, side.damp * decay);
            side.delay2.write(t);
        }
        let tap = |line: &DelayLine, at: f32| line.read(at * s);
        let (a, b) = (&self.tank[0], &self.tank[1]);
        let left = tap(&b.delay1, 266.0) + tap(&b.delay1, 2974.0) - tap(&b.ap2, 1913.0) + tap(&b.delay2, 1996.0) - tap(&a.delay1, 1990.0) - tap(&a.ap2, 187.0) - tap(&a.delay2, 1066.0);
        let right = tap(&a.delay1, 353.0) + tap(&a.delay1, 3627.0) - tap(&a.ap2, 1228.0) + tap(&a.delay2, 2673.0) - tap(&b.delay1, 2111.0) - tap(&b.ap2, 335.0) - tap(&b.delay2, 121.0);
        (left * 0.3, right * 0.3)
    }
}

/// The shared echo and reverb, the master equaliser and compressor.
pub struct MasterRt {
    sr: f32,
    echo: [DelayLine; 2],
    reverb: [ReverbChannel; 2],
    plate: Plate,
    /// The impulse of a room that takes the place of the reverb, its left
    /// and right channels, by the path of its file.
    reverb_impulse: Option<(Convolver, Convolver)>,
    reverb_path: String,
    eq: [EqRt; 2],
    comp: Compressor,
    /// The echo and the reverb of the block, kept from one block to the next
    /// so as not to allocate.
    echo_out: [Vec<f32>; 2],
    wet: [Vec<f32>; 2],
}

impl MasterRt {
    pub fn new(sr: f32) -> Self {
        let line = || DelayLine::new((sr * 3.1) as usize);
        let room = || Vec::with_capacity(MOST_BLOCK);
        Self {
            sr,
            echo: [line(), line()],
            reverb: [ReverbChannel::new(sr, 0.0), ReverbChannel::new(sr, 23.0)],
            plate: Plate::new(sr),
            reverb_impulse: None,
            reverb_path: String::new(),
            eq: [EqRt::default(), EqRt::default()],
            comp: Compressor::default(),
            echo_out: [room(), room()],
            wet: [room(), room()],
        }
    }

    /// `reverb_impulse` is the path of the impulse that takes the place of
    /// the reverb, or empty; `captures` holds the files loaded.
    pub fn configure(&mut self, fx: &MasterFx, reverb_impulse: &str, captures: &Captures) {
        for eq in &mut self.eq {
            eq.configure(&fx.eq, self.sr);
        }
        if reverb_impulse != self.reverb_path {
            self.reverb_impulse = captures.impulses.get(reverb_impulse).and_then(|impulse| {
                let left = impulse.channels.first()?;
                let right = impulse.channels.get(1).unwrap_or(left);
                Some((Convolver::new(left, impulse.sample_rate, self.sr), Convolver::new(right, impulse.sample_rate, self.sr)))
            });
            self.reverb_path = reverb_impulse.to_owned();
        }
    }

    /// Forgets the impulse loaded, so that the next `configure` reads it again.
    pub fn reload(&mut self) {
        self.reverb_impulse = None;
        self.reverb_path.clear();
    }

    /// `l`/`r` hold the dry mix and receive the result; `echo_in` and `reverb_in`
    /// hold what the tracks sent to the shared effects.
    pub fn process(
        &mut self,
        fx: &MasterFx,
        tempo: f64,
        l: &mut [f32],
        r: &mut [f32],
        echo_in: &[f32],
        reverb_in: &[f32],
    ) {
        let sr = self.sr;
        let delay = (fx.echo_beats.clamp(0.0625, 4.0) * 60.0 / tempo.max(20.0)) as f32 * sr;
        let feedback = fx.echo_feedback.clamp(0.0, 0.9);
        let room = 0.7 + 0.28 * fx.reverb_size.clamp(0.0, 1.0);
        let damp = 0.05 + 0.5 * fx.reverb_damp.clamp(0.0, 1.0);
        let n = l.len();
        for buffer in self.echo_out.iter_mut().chain(&mut self.wet) {
            buffer.clear();
            buffer.resize(n, 0.0);
        }
        // The echo first: ping-pong, each side feeding the other. A little of
        // it goes into the reverb.
        for i in 0..n {
            let el = self.echo[0].read(delay);
            let er = self.echo[1].read(delay * 1.5);
            self.echo[0].write(echo_in[i] + er * feedback);
            self.echo[1].write(echo_in[i] * 0.6 + el * feedback);
            self.echo_out[0][i] = el;
            self.echo_out[1][i] = er;
            self.wet[0][i] = reverb_in[i] + el * 0.2;
            self.wet[1][i] = reverb_in[i] + er * 0.2;
        }
        // Then the reverb: the impulse of a room, the plate, or the built-in room.
        match (&mut self.reverb_impulse, fx.reverb_kind) {
            (Some((left, right)), _) => {
                left.process(&mut self.wet[0]);
                right.process(&mut self.wet[1]);
            }
            (None, ReverbKind::Plate) => {
                let decay = 0.25 + 0.7 * fx.reverb_size.clamp(0.0, 1.0);
                let damping = 0.05 + 0.6 * fx.reverb_damp.clamp(0.0, 1.0);
                for i in 0..n {
                    let (left, right) = self.plate.run(0.5 * (self.wet[0][i] + self.wet[1][i]), decay, damping, sr);
                    self.wet[0][i] = left;
                    self.wet[1][i] = right;
                }
            }
            (None, ReverbKind::Room) => {
                for i in 0..n {
                    self.wet[0][i] = self.reverb[0].run(self.wet[0][i], room, damp);
                    self.wet[1][i] = self.reverb[1].run(self.wet[1][i], room, damp);
                }
            }
        }
        for i in 0..n {
            let (mut x, mut y) = (l[i] + self.echo_out[0][i] + self.wet[0][i], r[i] + self.echo_out[1][i] + self.wet[1][i]);
            if fx.eq.on {
                x = self.eq[0].run(x);
                y = self.eq[1].run(y);
            }
            if fx.comp > 0.001 {
                let gain = self.comp.gain(x.abs().max(y.abs()), fx.comp.min(1.0), sr);
                x *= gain;
                y *= gain;
            }
            l[i] = x;
            r[i] = y;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| (TAU * hz * i as f32 / 48_000.0).sin() * 0.5).collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|s| s * s).sum::<f32>() / x.len() as f32).sqrt()
    }

    #[test]
    fn equaliser_boosts_and_cuts_the_right_band() {
        let mut eq = EqRt::default();
        eq.configure(&Eq { on: true, low: 12.0, mid: 0.0, mid_freq: 1000.0, high: -12.0, ..Eq::default() }, 48_000.0);
        let low: Vec<f32> = sine(60.0, 48_000).into_iter().map(|s| eq.run(s)).collect();
        let high: Vec<f32> = sine(10_000.0, 48_000).into_iter().map(|s| eq.run(s)).collect();
        assert!(rms(&low[24_000..]) > rms(&sine(60.0, 48_000)) * 2.5);
        assert!(rms(&high[24_000..]) < rms(&sine(10_000.0, 48_000)) * 0.4);
    }

    #[test]
    fn compressor_reduces_loud_signals_more_than_quiet_ones() {
        let mut comp = Compressor::default();
        let mut loud = 0.0;
        for _ in 0..4800 {
            loud = comp.gain(1.0, 1.0, 48_000.0);
        }
        let mut comp = Compressor::default();
        let mut quiet = 0.0;
        for _ in 0..4800 {
            quiet = comp.gain(0.01, 1.0, 48_000.0);
        }
        assert!(loud < quiet * 0.5, "{loud} {quiet}");
    }

    #[test]
    fn pedals_and_amp_stay_finite_and_bounded() {
        let mut fx = TrackFx::default();
        fx.eq.on = true;
        fx.comp = 0.8;
        fx.amp = Amp { on: true, model: AmpModel::Lead, gain: 1.0, ..Amp::default() };
        fx.pedals = PedalKind::ALL.iter().map(|&kind| Pedal { kind, on: true, amount: 1.0, rate: 0.5 }).collect();
        let mut rt = TrackFxRt::new(48_000.0);
        rt.configure(&fx, &Captures::default());
        let mut buf = sine(196.0, 48_000);
        rt.process(&fx, &mut buf);
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() < 8.0));
        assert!(rms(&buf) > 0.01);
    }

    #[test]
    fn reverb_and_echo_ring_after_the_input_stops() {
        for kind in [ReverbKind::Room, ReverbKind::Plate] {
            let mut master = MasterRt::new(48_000.0);
            let fx = MasterFx { reverb_kind: kind, ..MasterFx::default() };
            master.configure(&fx, "", &Captures::default());
            let n = 48_000;
            let mut send = vec![0.0f32; n];
            send[..2400].copy_from_slice(&sine(440.0, 2400));
            let (mut l, mut r) = (vec![0.0f32; n], vec![0.0f32; n]);
            master.process(&fx, 120.0, &mut l, &mut r, &send, &send);
            assert!(rms(&l[4800..24_000]) > 1e-4, "{kind:?}");
            assert!(l.iter().chain(&r).all(|s| s.is_finite() && s.abs() < 4.0), "{kind:?}");
            // Left and right differ: the effect is stereo.
            assert!(l.iter().zip(&r).any(|(a, b)| (a - b).abs() > 1e-4), "{kind:?}");
            // The tail dies away.
            assert!(rms(&l[40_000..]) < rms(&l[4800..9600]), "{kind:?}");
        }
    }

    #[test]
    fn the_gate_and_the_wah_do_what_their_knobs_say() {
        // The gate: a quiet signal is silenced, a loud one goes through.
        let mut rt = PedalRt::new(48_000.0);
        let gate = Pedal { kind: PedalKind::Gate, on: true, amount: 0.5, rate: 0.2 };
        let quiet: Vec<f32> = sine(220.0, 9600).iter().map(|s| rt.run(&gate, s * 0.001, 48_000.0)).collect();
        let loud: Vec<f32> = sine(220.0, 9600).iter().map(|s| rt.run(&gate, *s, 48_000.0)).collect();
        assert!(rms(&quiet[4800..]) < 1e-5 && rms(&loud[4800..]) > 0.3, "{} {}", rms(&quiet[4800..]), rms(&loud[4800..]));
        // The wah: heel down lets the lows through, toe down the highs.
        let response = |position: f32, hz: f32| {
            let mut rt = PedalRt::new(48_000.0);
            let wah = Pedal { kind: PedalKind::Wah, on: true, amount: position, rate: 0.5 };
            let out: Vec<f32> = sine(hz, 9600).iter().map(|s| rt.run(&wah, *s, 48_000.0)).collect();
            rms(&out[4800..])
        };
        assert!(response(0.0, 350.0) > response(0.0, 2000.0) * 2.0);
        assert!(response(1.0, 2000.0) > response(1.0, 350.0) * 2.0);
        // The echoes come back, and die away.
        for kind in [PedalKind::TapeEcho, PedalKind::BbdEcho, PedalKind::Flanger, PedalKind::AutoWah] {
            let mut rt = PedalRt::new(48_000.0);
            let pedal = Pedal { kind, on: true, amount: 0.6, rate: 0.3 };
            let mut input = vec![0.0f32; 96_000];
            input[..2400].copy_from_slice(&sine(440.0, 2400));
            let out: Vec<f32> = input.iter().map(|s| rt.run(&pedal, *s, 48_000.0)).collect();
            assert!(out.iter().all(|s| s.is_finite() && s.abs() < 4.0), "{kind:?}");
            assert!(rms(&out[90_000..]) < 1e-3, "{kind:?} still rings: {}", rms(&out[90_000..]));
        }
    }
}
