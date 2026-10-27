//! Changes the rate of a stream of sound, a block at a time, with a
//! windowed-sinc filter, the usual way (J. O. Smith, "Digital Audio
//! Resampling"): good enough to run a capture made at 48 kHz on a card at
//! 44.1 kHz and bring its sound back, without hearing the two changes.

use std::f64::consts::PI;

/// Taps on each side of the point read.
const HALF: usize = 16;
const TAPS: usize = 2 * HALF;
/// Fractions of a sample the filter is kept for; between two, a straight line.
const PHASES: usize = 128;

/// The weights of the filter: a row of `TAPS` for each of `PHASES + 1`
/// fractions. `cutoff` is where the filter stops letting through, as a share
/// of the Nyquist frequency of the input.
fn weights(cutoff: f64) -> Vec<f32> {
    let mut out = Vec::with_capacity((PHASES + 1) * TAPS);
    for p in 0..=PHASES {
        let frac = p as f64 / PHASES as f64;
        let mut row = [0f64; TAPS];
        for (k, w) in row.iter_mut().enumerate() {
            // The tap sits `d` samples from the point read.
            let d = (k as f64 + 1.0 - HALF as f64) - frac;
            let x = PI * cutoff * d;
            let sinc = if x.abs() < 1e-9 { 1.0 } else { x.sin() / x };
            let window = d / HALF as f64;
            let blackman = 0.42 + 0.5 * (PI * window).cos() + 0.08 * (2.0 * PI * window).cos();
            *w = sinc * blackman.max(0.0);
        }
        let sum: f64 = row.iter().sum();
        out.extend(row.iter().map(|w| (w / sum) as f32));
    }
    out
}

/// A stream from one rate to another. Its sound comes `HALF` input samples
/// late.
pub struct Resampler {
    /// Input samples for each output sample.
    step: f64,
    table: Vec<f32>,
    /// The input not yet consumed, the first `HALF - 1` samples being history.
    fifo: Vec<f32>,
    /// Where the next output is read, in samples from the start of `fifo`.
    pos: f64,
}

impl Resampler {
    /// From one rate to another; `block` is the most input given at once, so
    /// that the stream allocates nothing afterwards.
    pub fn new(from: f64, to: f64, block: usize) -> Self {
        let step = from.max(1.0) / to.max(1.0);
        // Going to a lower rate, nothing above its Nyquist frequency may stay.
        let cutoff = (1.0 / step).min(1.0) * 0.92;
        let mut fifo = Vec::with_capacity(block + TAPS + 8);
        fifo.resize(HALF - 1, 0.0);
        Self { step, table: weights(cutoff), fifo, pos: (HALF - 1) as f64 }
    }

    /// Takes a block in and appends what comes out of it to `out` (which
    /// should have room for `input.len() * to / from + 2` more samples).
    pub fn run(&mut self, input: &[f32], out: &mut Vec<f32>) {
        self.fifo.extend_from_slice(input);
        // An output is read once its taps ahead are all there.
        while (self.pos.floor() as usize) + HALF < self.fifo.len() {
            let i = self.pos.floor() as usize;
            let frac = self.pos - i as f64;
            let row = frac * PHASES as f64;
            let r0 = (row.floor() as usize).min(PHASES - 1);
            let mix = (row - r0 as f64) as f32;
            let a = &self.table[r0 * TAPS..][..TAPS];
            let b = &self.table[(r0 + 1) * TAPS..][..TAPS];
            let window = &self.fifo[i + 1 - HALF..][..TAPS];
            let y: f32 = window.iter().zip(a).zip(b).map(|((x, a), b)| x * (a + (b - a) * mix)).sum();
            out.push(y);
            self.pos += self.step;
        }
        // The input no output needs any more goes.
        let drop = (self.pos.floor() as usize + 1).saturating_sub(HALF).min(self.fifo.len());
        self.fifo.drain(..drop);
        self.pos -= drop as f64;
    }

    /// Converts a whole sound at once (not for the audio thread).
    pub fn convert(samples: &[f32], from: f64, to: f64) -> Vec<f32> {
        let mut resampler = Self::new(from, to, samples.len() + TAPS);
        let mut out = Vec::with_capacity((samples.len() as f64 * to / from) as usize + TAPS + 8);
        resampler.run(samples, &mut out);
        // The tail that the filter still holds.
        resampler.run(&[0.0; TAPS], &mut out);
        out.truncate((samples.len() as f64 * to / from).round() as usize);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f64, rate: f64, n: usize) -> Vec<f32> {
        (0..n).map(|i| (2.0 * PI * hz * i as f64 / rate).sin() as f32 * 0.5).collect()
    }

    #[test]
    fn a_sine_survives_the_journey_to_another_rate_and_back() {
        let n = 4800;
        let wave = sine(440.0, 48_000.0, n);
        let there = Resampler::convert(&wave, 48_000.0, 44_100.0);
        assert!((there.len() as i64 - (n as f64 * 44_100.0 / 48_000.0) as i64).abs() <= 2, "{}", there.len());
        let back = Resampler::convert(&there, 44_100.0, 48_000.0);
        // The same wave, once the filters have settled, within a hair.
        let worst = (200..n - 200).map(|i| (back[i] - wave[i]).abs()).fold(0.0f32, f32::max);
        assert!(worst < 0.01, "{worst}");
        // Block by block, the stream gives the same samples as all at once.
        let mut stream = Resampler::new(48_000.0, 44_100.0, 97);
        let mut piecewise = Vec::with_capacity(n);
        for block in wave.chunks(97) {
            stream.run(block, &mut piecewise);
        }
        // The stream still holds the tail the filter needs ahead.
        assert!(piecewise.len() + HALF + 2 >= there.len() && piecewise.len() <= there.len(), "{} {}", piecewise.len(), there.len());
        assert!(piecewise.iter().zip(&there).all(|(a, b)| (a - b).abs() < 1e-5));
    }

    #[test]
    fn going_down_in_rate_cuts_what_the_lower_rate_cannot_hold() {
        // 20 kHz cannot exist at 22.05 kHz: it is filtered out, not folded down.
        let high = sine(20_000.0, 48_000.0, 4800);
        let down = Resampler::convert(&high, 48_000.0, 22_050.0);
        let level = (down[200..].iter().map(|s| s * s).sum::<f32>() / (down.len() - 200) as f32).sqrt();
        assert!(level < 0.02, "{level}");
    }
}
