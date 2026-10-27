//! Offline audio processing: pitch shifting that keeps the duration, pitch
//! correction, time stretching that keeps the pitch, and timing correction.
//! All of it uses textbook time-domain methods.

use std::f32::consts::TAU;

use crate::analysis::detect_pitch;
use crate::engine::ClipData;

fn channel(clip: &ClipData, index: usize) -> Vec<f32> {
    let channels = usize::from(clip.channels.max(1));
    clip.samples.iter().skip(index).step_by(channels).copied().collect()
}

fn mono(clip: &ClipData) -> Vec<f32> {
    let channels = usize::from(clip.channels.max(1));
    clip.samples.chunks_exact(channels).map(|f| f.iter().sum::<f32>() / channels as f32).collect()
}

fn interleave(channels: Vec<Vec<f32>>, sample_rate: u32) -> ClipData {
    let count = channels.len().max(1);
    let frames = channels.iter().map(Vec::len).min().unwrap_or(0);
    let mut samples = Vec::with_capacity(frames * count);
    for i in 0..frames {
        for c in &channels {
            samples.push(c[i]);
        }
    }
    ClipData { sample_rate, channels: count as u16, samples }
}

#[inline]
fn read(input: &[f32], pos: f32) -> f32 {
    if pos < 0.0 {
        return 0.0;
    }
    let i = pos as usize;
    if i + 1 >= input.len() {
        return 0.0;
    }
    let frac = pos - i as f32;
    input[i] + (input[i + 1] - input[i]) * frac
}

/// Shifts the pitch of `input` by a ratio that may change over time, keeping
/// the duration. Two read heads sweep through a short window and cross-fade.
pub fn pitch_shift(input: &[f32], sample_rate: f32, ratio: impl Fn(usize) -> f32) -> Vec<f32> {
    let window = (0.045 * sample_rate).max(64.0);
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(input.len());
    for n in 0..input.len() {
        let r = ratio(n).clamp(0.25, 4.0);
        phase = (phase + (1.0 - r) / window).rem_euclid(1.0);
        let mut sample = 0.0;
        for tap in [0.0f32, 0.5] {
            let p = (phase + tap).fract();
            // Silent where the head jumps back, full in the middle of its sweep.
            let gain = 0.5 - 0.5 * (TAU * p).cos();
            sample += gain * read(input, n as f32 + window * (0.5 - p));
        }
        out.push(sample);
    }
    out
}

/// A copy of `clip` transposed by `semitones` and pulled towards the nearest
/// notes by `tune` (0 to 1), with the same duration.
pub fn process_pitch(clip: &ClipData, semitones: i32, tune: f32) -> ClipData {
    let sr = clip.sample_rate.max(1) as f32;
    let base = 2f32.powf(semitones as f32 / 12.0);
    // One correction ratio per hop, measured on a mono, decimated copy.
    const HOP: usize = 512;
    let mut ratios: Vec<f32> = Vec::new();
    if tune > 0.0 {
        let decim = ((sr / 16_000.0).round() as usize).max(1);
        let low: Vec<f32> = mono(clip).chunks(decim).map(|c| c.iter().sum::<f32>() / c.len() as f32).collect();
        let low_sr = sr / decim as f32;
        let window = (low_sr * 0.045) as usize;
        let hop = (HOP / decim).max(1);
        let mut smooth = 1.0f32;
        let mut at = 0;
        while at < low.len() {
            let end = (at + window).min(low.len());
            let target = detect_pitch(&low[at..end], low_sr).map_or(1.0, |hz| {
                let note = (12.0 * (hz / 440.0).log2()).round();
                440.0 * 2f32.powf(note / 12.0) / hz
            });
            // Glide between corrections instead of jumping.
            smooth += 0.4 * (target - smooth);
            ratios.push(smooth.powf(tune.clamp(0.0, 1.0)));
            at += hop;
        }
    }
    let ratio = |n: usize| base * ratios.get(n / HOP).copied().unwrap_or(1.0);
    let channels = (0..usize::from(clip.channels.max(1))).map(|c| pitch_shift(&channel(clip, c), sr, ratio)).collect();
    interleave(channels, clip.sample_rate)
}

/// Positions in the input from which each output frame is taken, for a
/// stretch by `factor` (2 = twice as long). Each frame is nudged to where it
/// lines up best with what came before, which avoids phase cancellation.
fn stretch_plan(input: &[f32], factor: f64, frame: usize, seek: usize) -> Vec<usize> {
    let hop = frame / 2;
    let out_len = (input.len() as f64 * factor) as usize;
    let mut plan = Vec::with_capacity(out_len / hop + 1);
    let mut prev: Option<usize> = None;
    let mut k = 0;
    while k * hop < out_len {
        let ideal = (k * hop) as f64 / factor;
        let mut best = ideal as usize;
        if let Some(prev) = prev {
            // What would naturally follow the previous frame.
            let natural = prev + hop;
            if natural + hop < input.len() {
                let lo = (ideal as usize).saturating_sub(seek);
                let hi = ((ideal as usize) + seek).min(input.len().saturating_sub(hop + 1));
                let mut best_score = f32::MIN;
                let mut cand = lo;
                while cand <= hi {
                    let mut score = 0.0;
                    let mut i = 0;
                    while i < hop {
                        score += input[cand + i] * input[natural + i];
                        i += 4;
                    }
                    if score > best_score {
                        best_score = score;
                        best = cand;
                    }
                    cand += 8;
                }
            }
        }
        best = best.min(input.len().saturating_sub(1));
        plan.push(best);
        prev = Some(best);
        k += 1;
    }
    plan
}

fn stretch_apply(input: &[f32], plan: &[usize], frame: usize, out_len: usize) -> Vec<f32> {
    let hop = frame / 2;
    let mut out = vec![0.0f32; out_len + frame];
    for (k, &from) in plan.iter().enumerate() {
        for i in 0..frame {
            let Some(sample) = input.get(from + i) else { break };
            // Hann windows half a frame apart add up to one.
            let gain = 0.5 - 0.5 * (TAU * i as f32 / frame as f32).cos();
            out[k * hop + i] += sample * gain;
        }
    }
    out.truncate(out_len);
    out
}

/// Changes the length of mono audio by `factor` without changing its pitch.
pub fn time_stretch(input: &[f32], sample_rate: f32, factor: f64) -> Vec<f32> {
    let frame = ((0.03 * sample_rate) as usize).max(64) & !1;
    let plan = stretch_plan(input, factor, frame, frame / 4);
    stretch_apply(input, &plan, frame, (input.len() as f64 * factor) as usize)
}

/// Sample positions where notes or hits start, from jumps in loudness.
pub fn detect_onsets(input: &[f32], sample_rate: f32) -> Vec<usize> {
    let hop = ((sample_rate * 0.005) as usize).max(1);
    let energy: Vec<f32> = input.chunks(hop).map(|c| c.iter().map(|s| s * s).sum::<f32>() / c.len() as f32).collect();
    let peak = energy.iter().copied().fold(0.0, f32::max);
    if peak <= 1e-8 {
        return Vec::new();
    }
    let min_gap = (0.09 * sample_rate) as usize;
    let mut onsets: Vec<usize> = Vec::new();
    let mut average = 0.0f32;
    for (i, &e) in energy.iter().enumerate() {
        let loud = e > peak * 0.02 && e > average * 3.0 + 1e-7;
        let at = i * hop;
        if loud && onsets.last().is_none_or(|last| at - last >= min_gap) {
            onsets.push(at);
        }
        // Slow to rise, so that the attack itself stands out against it.
        average += (if e > average { 0.08 } else { 0.3 }) * (e - average);
    }
    onsets
}

/// Moves each onset of `clip` to the nearest grid line by stretching or
/// squeezing the audio between onsets. `first_beat` is the beat position of the
/// clip's first sample relative to a grid line, `beat_seconds` the length of a
/// beat and `grid` the grid spacing in beats. The total duration is kept.
pub fn quantize_timing(clip: &ClipData, first_beat: f64, beat_seconds: f64, grid: f64) -> ClipData {
    let sr = clip.sample_rate.max(1) as f32;
    let mix = mono(clip);
    let total = mix.len();
    let onsets = detect_onsets(&mix, sr);
    let to_sample = |beats: f64| (beats * beat_seconds * f64::from(sr)).round();
    // Anchor points (where it is, where it should be), kept in order.
    let mut anchors: Vec<(usize, usize)> = vec![(0, 0)];
    for onset in onsets {
        let beat = first_beat + onset as f64 / f64::from(sr) / beat_seconds;
        let snapped = (beat / grid).round() * grid;
        let target = to_sample(snapped - first_beat);
        let &(last_from, last_to) = anchors.last().unwrap_or(&(0, 0));
        let min_gap = (0.03 * sr) as usize;
        if onset > last_from + min_gap && target > (last_to + min_gap) as f64 && target < total as f64 {
            anchors.push((onset, target as usize));
        }
    }
    anchors.push((total, total));

    let frame = ((0.03 * sr) as usize).max(64) & !1;
    let count = usize::from(clip.channels.max(1));
    let mut channels: Vec<Vec<f32>> = (0..count).map(|_| Vec::with_capacity(total)).collect();
    for pair in anchors.windows(2) {
        let ((from, to), (next_from, next_to)) = (pair[0], pair[1]);
        let (in_len, out_len) = (next_from - from, next_to - to);
        if in_len == 0 || out_len == 0 {
            continue;
        }
        let factor = out_len as f64 / in_len as f64;
        // One plan from the mono mix, applied to every channel so they stay in phase.
        let plan = stretch_plan(&mix[from..next_from], factor, frame, frame / 4);
        for (c, out) in channels.iter_mut().enumerate() {
            let source = channel(clip, c);
            if (factor - 1.0).abs() < 0.01 {
                out.extend_from_slice(&source[from..next_from.min(source.len())]);
                out.resize(to + out_len, 0.0);
            } else {
                out.extend(stretch_apply(&source[from..next_from.min(source.len())], &plan, frame, out_len));
            }
        }
    }
    interleave(channels, clip.sample_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, seconds: f32) -> Vec<f32> {
        (0..(48_000.0 * seconds) as usize).map(|i| (TAU * hz * i as f32 / 48_000.0).sin() * 0.5).collect()
    }

    fn clip(samples: Vec<f32>) -> ClipData {
        ClipData { sample_rate: 48_000, channels: 1, samples }
    }

    #[test]
    fn transposition_changes_pitch_not_length() {
        let input = clip(tone(220.0, 1.0));
        for (semitones, expected) in [(12, 440.0), (-12, 110.0), (7, 329.63)] {
            let out = process_pitch(&input, semitones, 0.0);
            assert_eq!(out.samples.len(), input.samples.len());
            let found = detect_pitch(&out.samples[12_000..16_096], 48_000.0).unwrap();
            assert!((found - expected).abs() < expected * 0.02, "{semitones}: {found}");
        }
    }

    #[test]
    fn pitch_correction_pulls_a_flat_note_into_tune() {
        // 30 cents below A3.
        let flat = 220.0 * 2f32.powf(-0.30 / 12.0);
        let input = clip(tone(flat, 1.5));
        let out = process_pitch(&input, 0, 1.0);
        let found = detect_pitch(&out.samples[40_000..44_096], 48_000.0).unwrap();
        let cents = 1200.0 * (found / 220.0).log2();
        assert!(cents.abs() < 8.0, "{cents} cents off");
        // Half strength corrects about half of it.
        let half = process_pitch(&input, 0, 0.5);
        let found = detect_pitch(&half.samples[40_000..44_096], 48_000.0).unwrap();
        let cents = 1200.0 * (found / 220.0).log2();
        assert!((-22.0..-8.0).contains(&cents), "{cents}");
    }

    #[test]
    fn stretching_changes_length_not_pitch() {
        let input = tone(330.0, 1.0);
        for factor in [0.5, 0.8, 1.5, 2.0] {
            let out = time_stretch(&input, 48_000.0, factor);
            assert_eq!(out.len(), (48_000.0 * factor) as usize);
            let middle = out.len() / 2;
            let found = detect_pitch(&out[middle..middle + 4096], 48_000.0).unwrap();
            assert!((found - 330.0).abs() < 4.0, "{factor}: {found}");
            let rms = (out[middle..middle + 4096].iter().map(|s| s * s).sum::<f32>() / 4096.0).sqrt();
            assert!((0.25..0.45).contains(&rms), "{factor}: level {rms}");
        }
    }

    fn clicks(times: &[f32]) -> Vec<f32> {
        let mut samples = vec![0.0f32; 96_000];
        for &t in times {
            let at = (t * 48_000.0) as usize;
            for i in 0..2400 {
                samples[at + i] += (TAU * 880.0 * i as f32 / 48_000.0).sin() * (1.0 - i as f32 / 2400.0) * 0.8;
            }
        }
        samples
    }

    #[test]
    fn onsets_are_found_and_moved_to_the_grid() {
        // Beats every 0.5 s (120 bpm); the hits are early or late.
        let played = [0.02f32, 0.56, 0.95, 1.53];
        let input = clip(clicks(&played));
        let found = detect_onsets(&input.samples, 48_000.0);
        assert_eq!(found.len(), 4, "{found:?}");
        for (onset, time) in found.iter().zip(played) {
            assert!((*onset as f32 / 48_000.0 - time).abs() < 0.012);
        }

        let fixed = quantize_timing(&input, 0.0, 0.5, 1.0);
        assert_eq!(fixed.samples.len(), input.samples.len());
        let moved = detect_onsets(&fixed.samples, 48_000.0);
        assert_eq!(moved.len(), 4, "{moved:?}");
        for (onset, grid) in moved.iter().zip([0.0f32, 0.5, 1.0, 1.5]) {
            assert!((*onset as f32 / 48_000.0 - grid).abs() < 0.03, "{} vs {grid}", *onset as f32 / 48_000.0);
        }
        // Silence and stereo input are handled.
        assert!(quantize_timing(&clip(vec![0.0; 4800]), 0.0, 0.5, 1.0).samples.iter().all(|s| *s == 0.0));
        let stereo = ClipData { sample_rate: 48_000, channels: 2, samples: clicks(&played).iter().flat_map(|s| [*s, *s * 0.5]).collect() };
        let fixed = quantize_timing(&stereo, 0.0, 0.5, 1.0);
        assert_eq!((fixed.channels, fixed.samples.len()), (2, stereo.samples.len()));
        assert!(process_pitch(&clip(Vec::new()), 3, 1.0).samples.is_empty());
    }
}
