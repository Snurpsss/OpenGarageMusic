//! Pitch detection for the tuner, and waveform summaries for drawing.

/// Fundamental frequency of `samples` in Hz, or `None` when no clear pitch is
/// heard. Uses the normalised difference of the signal with a delayed copy of
/// itself, a standard time-domain method.
pub fn detect_pitch(samples: &[f32], sample_rate: f32) -> Option<f32> {
    let min_lag = (sample_rate / 1200.0) as usize;
    let max_lag = (sample_rate / 60.0) as usize;
    if samples.len() < max_lag * 2 || min_lag < 2 {
        return None;
    }
    let window = samples.len() - max_lag;
    let energy: f32 = samples[..window].iter().map(|s| s * s).sum::<f32>() / window as f32;
    if energy < 1e-5 {
        return None;
    }

    // Cumulative-mean normalised difference: near 0 at the period.
    let mut diff = vec![0.0f32; max_lag + 1];
    for (lag, slot) in diff.iter_mut().enumerate().skip(1) {
        let mut sum = 0.0;
        for i in 0..window {
            let d = samples[i] - samples[i + lag];
            sum += d * d;
        }
        *slot = sum;
    }
    let mut running = 0.0;
    let mut norm = vec![1.0f32; max_lag + 1];
    for lag in 1..=max_lag {
        running += diff[lag];
        norm[lag] = if running > 0.0 { diff[lag] * lag as f32 / running } else { 1.0 };
    }

    // First dip below the threshold, followed down to its local minimum.
    let mut lag = min_lag;
    while lag < max_lag {
        if norm[lag] < 0.15 {
            while lag + 1 < max_lag && norm[lag + 1] < norm[lag] {
                lag += 1;
            }
            // Parabolic interpolation between neighbours for sub-sample accuracy.
            let (a, b, c) = (norm[lag - 1], norm[lag], norm[lag + 1]);
            let denom = a - 2.0 * b + c;
            let shift = if denom.abs() > 1e-9 { 0.5 * (a - c) / denom } else { 0.0 };
            return Some(sample_rate / (lag as f32 + shift.clamp(-1.0, 1.0)));
        }
        lag += 1;
    }
    None
}

/// Nearest note to `hz`: MIDI pitch and the offset from it in cents.
pub fn nearest_note(hz: f32) -> (u8, f32) {
    let exact = 69.0 + 12.0 * (hz / 440.0).log2();
    let pitch = exact.round().clamp(0.0, 127.0);
    (pitch as u8, (exact - pitch) * 100.0)
}

/// Minimum and maximum of each of `columns` equal slices of `samples`
/// (interleaved, `channels` per frame), for drawing a waveform.
pub fn peaks(samples: &[f32], channels: usize, columns: usize) -> Vec<(f32, f32)> {
    let channels = channels.max(1);
    let frames = samples.len() / channels;
    if frames == 0 || columns == 0 {
        return Vec::new();
    }
    (0..columns)
        .map(|col| {
            let from = col * frames / columns;
            let to = ((col + 1) * frames / columns).max(from + 1).min(frames);
            let mut lo = f32::MAX;
            let mut hi = f32::MIN;
            for s in &samples[from * channels..to * channels] {
                lo = lo.min(*s);
                hi = hi.max(*s);
            }
            (lo, hi)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn tone(hz: f32, n: usize) -> Vec<f32> {
        // A fundamental with two harmonics, like a plucked string.
        (0..n)
            .map(|i| {
                let t = i as f32 / 48_000.0;
                0.5 * (TAU * hz * t).sin() + 0.25 * (TAU * 2.0 * hz * t).sin() + 0.1 * (TAU * 3.0 * hz * t).sin()
            })
            .collect()
    }

    #[test]
    fn finds_guitar_string_pitches() {
        for hz in [82.41f32, 110.0, 146.83, 196.0, 246.94, 329.63, 440.0] {
            let found = detect_pitch(&tone(hz, 4096), 48_000.0).unwrap();
            assert!((found - hz).abs() < hz * 0.01, "{hz} -> {found}");
        }
    }

    #[test]
    fn silence_and_noise_have_no_pitch() {
        assert_eq!(detect_pitch(&vec![0.0; 4096], 48_000.0), None);
        let mut x = 12345u32;
        let noise: Vec<f32> = (0..4096)
            .map(|_| {
                x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (x >> 8) as f32 / 8_388_608.0 - 1.0
            })
            .collect();
        assert_eq!(detect_pitch(&noise, 48_000.0), None);
        assert_eq!(detect_pitch(&[0.5; 10], 48_000.0), None);
    }

    #[test]
    fn note_names_and_cents() {
        assert_eq!(nearest_note(440.0).0, 69);
        let (pitch, cents) = nearest_note(445.0);
        assert_eq!(pitch, 69);
        assert!((cents - 19.56).abs() < 0.1);
        assert_eq!(peaks(&[0.0, 1.0, -1.0, 0.5], 1, 2), vec![(0.0, 1.0), (-1.0, 0.5)]);
    }
}
