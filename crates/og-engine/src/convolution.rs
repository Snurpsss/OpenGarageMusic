//! Convolution with an impulse response: the sound of a loudspeaker cabinet
//! or of a room, recorded once as its answer to a click, given to a stream
//! of sound. Done in the frequency domain, the impulse cut into pieces of
//! one block (Gardner, "Efficient convolution without input-output delay",
//! 1995, the uniform case): each piece is turned into a spectrum once;
//! every block of input is turned into a spectrum, multiplied by each
//! piece's in turn, and the sum turned back into sound.

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use crate::engine::ClipData;
use crate::resample::Resampler;

/// Samples in a piece of the impulse and in a block of input; the sound
/// comes out that many samples late.
pub const BLOCK: usize = 256;
/// Longest impulse used: a room's tail beyond that costs more than it gives.
pub const MOST_SECONDS: f64 = 5.0;
/// File extensions an impulse response may have.
pub const EXTENSIONS: [&str; 2] = ["wav", "flac"];

/// An impulse response as read from its file: one or two channels, at its
/// own rate.
pub struct Impulse {
    pub name: String,
    pub sample_rate: u32,
    pub channels: Vec<Vec<f32>>,
}

impl Impulse {
    /// The impulse in a decoded file: its first two channels, cut at
    /// `MOST_SECONDS`.
    pub fn from_clip(name: &str, clip: &ClipData) -> Result<Impulse, String> {
        let count = usize::from(clip.channels.max(1));
        let most = (MOST_SECONDS * f64::from(clip.sample_rate.max(1))) as usize;
        let channels: Vec<Vec<f32>> =
            (0..count.min(2)).map(|c| clip.samples.iter().skip(c).step_by(count).take(most).copied().collect()).collect();
        if channels.iter().all(|c| c.iter().all(|s| s.abs() < 1e-9)) {
            return Err("the impulse is silent".to_owned());
        }
        Ok(Impulse { name: name.to_owned(), sample_rate: clip.sample_rate, channels })
    }

    pub fn seconds(&self) -> f64 {
        self.channels.first().map_or(0.0, |c| c.len() as f64 / f64::from(self.sample_rate.max(1)))
    }
}

/// One channel of an impulse, running on one stream of sound.
pub struct Convolver {
    fft: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    scratch: Vec<Complex<f32>>,
    /// The spectra of the pieces of the impulse.
    pieces: Vec<Vec<Complex<f32>>>,
    /// The spectra of the last blocks of input, a ring whose newest is `newest`.
    history: Vec<Vec<Complex<f32>>>,
    newest: usize,
    /// The last block of input: the next transform starts with it.
    previous: Vec<f32>,
    /// The input of the block being gathered.
    pending: Vec<f32>,
    sum: Vec<Complex<f32>>,
    /// What came out and was not taken yet.
    out: Vec<f32>,
    gain: f32,
}

impl Convolver {
    /// Readies an impulse given at `impulse_rate` for a stream at `sample_rate`,
    /// its loudness brought to one.
    pub fn new(impulse: &[f32], impulse_rate: u32, sample_rate: f32) -> Self {
        let samples: Vec<f32> = if (f64::from(impulse_rate) - f64::from(sample_rate)).abs() < 1.0 {
            impulse.to_vec()
        } else {
            Resampler::convert(impulse, f64::from(impulse_rate), f64::from(sample_rate))
        };
        let energy: f32 = samples.iter().map(|s| s * s).sum();
        let gain = if energy > 1e-12 { 1.0 / energy.sqrt() } else { 0.0 };
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(2 * BLOCK);
        let inverse = planner.plan_fft_inverse(2 * BLOCK);
        let mut scratch = vec![Complex::default(); fft.get_inplace_scratch_len().max(inverse.get_inplace_scratch_len())];
        let pieces: Vec<Vec<Complex<f32>>> = samples
            .chunks(BLOCK)
            .map(|piece| {
                let mut spectrum = vec![Complex::default(); 2 * BLOCK];
                for (slot, s) in spectrum.iter_mut().zip(piece) {
                    slot.re = *s;
                }
                fft.process_with_scratch(&mut spectrum, &mut scratch);
                spectrum
            })
            .collect();
        let count = pieces.len().max(1);
        Self {
            fft,
            inverse,
            scratch,
            pieces,
            history: vec![vec![Complex::default(); 2 * BLOCK]; count],
            newest: 0,
            previous: vec![0.0; BLOCK],
            pending: Vec::with_capacity(BLOCK),
            sum: vec![Complex::default(); 2 * BLOCK],
            out: {
                let mut out = Vec::with_capacity(3 * BLOCK + 8192);
                out.resize(BLOCK, 0.0);
                out
            },
            gain,
        }
    }

    /// Runs a block of any size through the impulse, in place, `BLOCK`
    /// samples late. Allocates nothing for blocks up to 8192 samples.
    pub fn process(&mut self, buf: &mut [f32]) {
        for s in buf.iter() {
            self.pending.push(*s);
            if self.pending.len() == BLOCK {
                self.block();
            }
        }
        let have = self.out.len().min(buf.len());
        buf[..have].copy_from_slice(&self.out[..have]);
        buf[have..].fill(0.0);
        self.out.drain(..have);
    }

    /// One block gathered: its spectrum, the sum over the pieces, the sound.
    fn block(&mut self) {
        let count = self.history.len();
        self.newest = (self.newest + count - 1) % count;
        let spectrum = &mut self.history[self.newest];
        for (slot, s) in spectrum.iter_mut().zip(self.previous.iter().chain(&self.pending)) {
            *slot = Complex { re: *s, im: 0.0 };
        }
        self.fft.process_with_scratch(spectrum, &mut self.scratch);
        self.previous.copy_from_slice(&self.pending);
        self.pending.clear();
        self.sum.fill(Complex::default());
        for (p, piece) in self.pieces.iter().enumerate() {
            let past = &self.history[(self.newest + p) % count];
            for ((sum, a), b) in self.sum.iter_mut().zip(past).zip(piece) {
                *sum += a * b;
            }
        }
        self.inverse.process_with_scratch(&mut self.sum, &mut self.scratch);
        // The second half of the result is the convolution proper; the
        // transform is not scaled on the way back.
        let scale = self.gain / (2 * BLOCK) as f32;
        self.out.extend(self.sum[BLOCK..].iter().map(|c| {
            let s = c.re * scale;
            if s.is_finite() { s } else { 0.0 }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(n: usize, seed: u32) -> Vec<f32> {
        let mut x = seed;
        (0..n)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect()
    }

    fn direct(input: &[f32], impulse: &[f32]) -> Vec<f32> {
        (0..input.len()).map(|n| (0..=n.min(impulse.len() - 1)).map(|k| impulse[k] * input[n - k]).sum()).collect()
    }

    #[test]
    fn the_convolution_matches_the_one_done_by_hand_a_block_late() {
        // An impulse longer than three pieces, run in blocks of odd sizes.
        let impulse: Vec<f32> = noise(3 * BLOCK + 7, 1).iter().map(|s| s * 0.3).collect();
        let energy: f32 = impulse.iter().map(|s| s * s).sum();
        let input = noise(4000, 2);
        let wanted: Vec<f32> = direct(&input, &impulse).iter().map(|s| s / energy.sqrt()).collect();
        let mut convolver = Convolver::new(&impulse, 48_000, 48_000.0);
        let mut heard = Vec::new();
        for chunk in input.chunks(97) {
            let mut buf = chunk.to_vec();
            convolver.process(&mut buf);
            heard.extend(buf);
        }
        assert!(heard[..BLOCK].iter().all(|s| *s == 0.0), "silent while the first block gathers");
        let worst = (0..input.len() - BLOCK).map(|i| (heard[i + BLOCK] - wanted[i]).abs()).fold(0.0f32, f32::max);
        assert!(worst < 1e-4, "{worst}");
    }

    #[test]
    fn an_impulse_is_read_from_a_file_and_brought_to_the_rate_of_the_stream() {
        // A stereo file: two channels, cut at the longest length.
        let mut samples = vec![0.0f32; 2 * 48_000 * 6];
        samples[0] = 1.0;
        samples[1] = 0.5;
        let clip = ClipData { sample_rate: 48_000, channels: 2, samples };
        let impulse = Impulse::from_clip("room", &clip).unwrap();
        assert_eq!(impulse.channels.len(), 2);
        assert!((impulse.seconds() - MOST_SECONDS).abs() < 1e-6);
        assert_eq!((impulse.channels[0][0], impulse.channels[1][0]), (1.0, 0.5));
        assert!(Impulse::from_clip("nothing", &ClipData { sample_rate: 48_000, channels: 1, samples: vec![0.0; 100] }).is_err());
        // A click at 48 kHz on a stream at 44.1 kHz is still a click.
        let mut click = vec![0.0f32; 100];
        click[10] = 1.0;
        let mut convolver = Convolver::new(&click, 48_000, 44_100.0);
        let mut buf = vec![0.0f32; 2 * BLOCK];
        buf[0] = 1.0;
        convolver.process(&mut buf);
        let (at, peak) = buf.iter().enumerate().fold((0, 0.0f32), |best, (i, s)| if s.abs() > best.1 { (i, s.abs()) } else { best });
        assert!(peak > 0.5 && (at as i64 - (BLOCK + 9) as i64).abs() <= 1, "{at} {peak}");
    }
}
