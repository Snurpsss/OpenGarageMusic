//! Finds where the beats of a score fall in a recording of the song.
//!
//! Both are turned into the same picture: for each instant, how much each of
//! the twelve notes of the scale sounds (its chroma). The score's comes from
//! its notes, the recording's from the spectrum of its sound. A dynamic time
//! warping then pairs the instants of the score with those of the recording
//! so that, step by step, the two pictures agree best: the score may start
//! anywhere in the recording, and its pace may change as the recording's
//! does. Drums are left out: they have no notes of the scale.

use crate::engine::played_notes;
use crate::model::{Project, Timing, TrackKind};
use crate::patches;

/// Pictures a second, when the song is short enough for that.
pub const FPS: f64 = 20.0;
/// The most pairs of instants compared, to keep within memory and time.
const MOST_CELLS: f64 = 60e6;
/// Sample rate the recording is brought down to before its spectrum is taken.
const RATE: f64 = 11_025.0;
/// Samples a spectrum is taken over at that rate: about a third of a second,
/// long enough to tell the semitones of the low notes apart.
const WINDOW: usize = 4096;
/// Samples of the short window the strikes are found in (a twentieth of a second).
const SHORT: usize = 512;

/// What was found: where each beat of the score falls in the recording, and
/// how well the two agree, from 0 (no better than chance) to 1 (the same
/// notes all along).
#[derive(Clone, Debug, PartialEq)]
pub struct Alignment {
    pub timing: Vec<Timing>,
    pub agreement: f64,
}

/// Agreement under which the recording and the score are taken not to be
/// the same song (or not the same version of it).
pub const FAIR: f64 = 0.3;

type Chroma = [f32; 12];

/// The pictures of a score or a recording: how much each note of the scale
/// sounds at each instant, and how strongly notes are struck then (0 to 1).
#[derive(Clone, Debug, Default)]
pub struct Pictures {
    pub chroma: Vec<Chroma>,
    pub onsets: Vec<f32>,
}

/// Pictures of the recording for each of the score, at the same pace: the
/// recording is followed by half steps.
pub const FINER: usize = 2;

/// How much the strikes count against the notes when pictures are compared.
const STRIKES: f32 = 0.3;

/// Strikes brought to 0 to 1: the strongest is 1, the weak ones raised a
/// little so that they count.
fn scale_onsets(onsets: &mut [f32]) {
    let strongest = onsets.iter().copied().fold(0.0f32, f32::max).max(1e-9);
    for value in onsets.iter_mut() {
        *value = (*value / strongest).max(0.0).sqrt();
    }
}

/// Each picture scaled to length one, the loud notes kept from drowning the
/// others. A picture far quieter than the loudest of all is silence: the same
/// for every note, so that silence compares as silence.
fn normalise(frames: &mut [Chroma]) {
    let loudest = frames.iter().map(|f| f.iter().sum::<f32>()).fold(0.0f32, f32::max);
    for frame in frames {
        let peak = frame.iter().copied().fold(0.0f32, f32::max);
        if frame.iter().sum::<f32>() < loudest * 0.02 {
            *frame = [1.0; 12];
        } else {
            for value in frame.iter_mut() {
                *value = (1.0 + 10.0 * *value / peak.max(1e-9)).ln();
            }
        }
        let length = frame.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-9);
        for value in frame.iter_mut() {
            *value /= length;
        }
    }
}

/// Whether a track plays drums, which have no notes of the scale.
fn drums(project: &Project, index: usize) -> bool {
    let track = &project.tracks[index];
    track.kind == TrackKind::Drummer || patches::find(&track.patch).is_some_and(|p| p.category == patches::CAT_DRUMS)
}

/// The pictures of the score at its own tempo, `fps` a second.
pub fn score_pictures(project: &Project, fps: f64) -> Pictures {
    let seconds = project.beat_to_sec(project.end_beat());
    let count = ((seconds * fps).ceil() as usize).max(1);
    let mut frames = vec![[0.0f32; 12]; count];
    let mut onsets = vec![0.0f32; count];
    for (index, track) in project.tracks.iter().enumerate() {
        if crate::recording::is_recording(project, index) || drums(project, index) || track.kind.is_audio() {
            continue;
        }
        for (start, length, pitch, velocity) in played_notes(track, project) {
            let on = project.beat_to_sec(start);
            let off = project.beat_to_sec(start + length).max(on + 0.05);
            let weight = f32::from(velocity) / 127.0;
            let class = usize::from(pitch % 12);
            let first = (on * fps).floor().max(0.0) as usize;
            let last = ((off * fps).ceil() as usize).min(count);
            if let Some(strike) = onsets.get_mut((on * fps).round().max(0.0) as usize) {
                *strike += weight;
            }
            for (k, frame) in frames.iter_mut().enumerate().take(last).skip(first) {
                // Struck notes fade, as the recording's do.
                let since = (k as f64 / fps - on).max(0.0);
                frame[class] += weight * (-(since as f32) / 1.5).exp();
            }
        }
    }
    // Seen through the same window as the recording's notes are, which sees
    // a chord a little before it is struck and after it ends.
    let half = (WINDOW as f64 / RATE / 2.0 * fps).round() as usize;
    let weights: Vec<f32> = (0..=2 * half).map(|k| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * (k as f32 + 1.0) / (2 * half + 2) as f32).cos()).collect();
    let sharp = frames.clone();
    for (k, frame) in frames.iter_mut().enumerate() {
        *frame = [0.0; 12];
        for (offset, weight) in weights.iter().enumerate() {
            let Some(source) = (k + offset).checked_sub(half).and_then(|at| sharp.get(at)) else { continue };
            for (value, from) in frame.iter_mut().zip(source) {
                *value += weight * from;
            }
        }
    }
    normalise(&mut frames);
    scale_onsets(&mut onsets);
    Pictures { chroma: frames, onsets }
}

/// A fast Fourier transform, in place, of a length that is a power of two.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut size = 2;
    while size <= n {
        let angle = -2.0 * std::f32::consts::PI / size as f32;
        let (step_re, step_im) = (angle.cos(), angle.sin());
        for start in (0..n).step_by(size) {
            let (mut w_re, mut w_im) = (1.0f32, 0.0f32);
            for k in 0..size / 2 {
                let (a, b) = (start + k, start + k + size / 2);
                let (t_re, t_im) = (re[b] * w_re - im[b] * w_im, re[b] * w_im + im[b] * w_re);
                re[b] = re[a] - t_re;
                im[b] = im[a] - t_im;
                re[a] += t_re;
                im[a] += t_im;
                (w_re, w_im) = (w_re * step_re - w_im * step_im, w_re * step_im + w_im * step_re);
            }
        }
        size <<= 1;
    }
}

/// The pictures of a recording, `fps` a second: its sound, brought to one
/// channel and a lower rate, then the spectrum of each instant, each
/// frequency counted for the note it is nearest; and the strikes, where the
/// spectrum of a short window grows from one instant to the next.
pub fn audio_pictures(samples: &[f32], channels: usize, sample_rate: u32, fps: f64) -> Pictures {
    let channels = channels.max(1);
    // One channel, averaged down to about RATE.
    let factor = (f64::from(sample_rate) / RATE).round().max(1.0) as usize;
    let rate = f64::from(sample_rate) / factor as f64;
    let mono: Vec<f32> = samples
        .chunks(channels * factor)
        .map(|chunk| chunk.iter().sum::<f32>() / chunk.len().max(1) as f32)
        .collect();
    let seconds = mono.len() as f64 / rate;
    let count = ((seconds * fps).ceil() as usize).max(1);
    // Each frequency of the spectrum, the note it is nearest, between 55 Hz and 4 kHz.
    let classes: Vec<Option<usize>> = (0..WINDOW / 2)
        .map(|bin| {
            let hz = bin as f64 * rate / WINDOW as f64;
            (55.0..4000.0).contains(&hz).then(|| ((69.0 + 12.0 * (hz / 440.0).log2()).round() as i64).rem_euclid(12) as usize)
        })
        .collect();
    let hann: Vec<f32> = (0..WINDOW).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / WINDOW as f32).cos()).collect();
    let mut frames = vec![[0.0f32; 12]; count];
    let (mut re, mut im) = (vec![0.0f32; WINDOW], vec![0.0f32; WINDOW]);
    let mut onsets = vec![0.0f32; count];
    let short_hann: Vec<f32> = (0..SHORT).map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / SHORT as f32).cos()).collect();
    let (mut short_re, mut short_im) = (vec![0.0f32; SHORT], vec![0.0f32; SHORT]);
    let mut before = vec![0.0f32; SHORT / 2];
    for (k, frame) in frames.iter_mut().enumerate() {
        let centre = (k as f64 / fps * rate) as usize;
        let from = centre.saturating_sub(WINDOW / 2);
        for i in 0..WINDOW {
            re[i] = mono.get(from + i).copied().unwrap_or(0.0) * hann[i];
            im[i] = 0.0;
        }
        fft(&mut re, &mut im);
        for (bin, class) in classes.iter().enumerate() {
            if let Some(class) = class {
                frame[*class] += (re[bin] * re[bin] + im[bin] * im[bin]).sqrt();
            }
        }
        let from = centre.saturating_sub(SHORT / 2);
        for i in 0..SHORT {
            short_re[i] = mono.get(from + i).copied().unwrap_or(0.0) * short_hann[i];
            short_im[i] = 0.0;
        }
        fft(&mut short_re, &mut short_im);
        let mut growth = 0.0f32;
        for (bin, last) in before.iter_mut().enumerate() {
            let now = (1.0 + 100.0 * (short_re[bin] * short_re[bin] + short_im[bin] * short_im[bin]).sqrt()).ln();
            growth += (now - *last).max(0.0);
            *last = now;
        }
        onsets[k] = growth;
    }
    // Against the strikes around: what stands out of them.
    let local: Vec<f32> = (0..count).map(|k| {
        let around = &onsets[k.saturating_sub(10)..(k + 11).min(count)];
        around.iter().sum::<f32>() / around.len() as f32
    }).collect();
    for (value, mean) in onsets.iter_mut().zip(local) {
        *value = (*value - mean).max(0.0);
    }
    normalise(&mut frames);
    scale_onsets(&mut onsets);
    Pictures { chroma: frames, onsets }
}

/// Pairs the pictures of the score with those of the recording: for each
/// picture of the score, the picture of the recording it falls on. The
/// score may start and end anywhere in the recording. The recording has
/// `FINER` pictures for each of the score at the same pace; from one picture
/// of the score to the next, it moves on by none to three times that, so
/// that it may be up to three times as fast as the score, or wait on it, the
/// moves near the same pace costing least.
/// What is weighed is the agreement of each picture of the score, however
/// much of the recording the way takes. Also the agreement along the way,
/// against that of pictures taken anyhow.
pub fn warp(score: &Pictures, recording: &Pictures) -> (Vec<usize>, f64) {
    const MOVES: [(usize, f32); 7] = [(0, 0.15), (1, 0.02), (2, 0.0), (3, 0.02), (4, 0.05), (5, 0.1), (6, 0.15)];
    let (n, m) = (score.chroma.len().min(score.onsets.len()), recording.chroma.len().min(recording.onsets.len()));
    if n == 0 || m == 0 {
        return (vec![0; n], 0.0);
    }
    // Unlike notes, then unlike strikes.
    let cost = |i: usize, j: usize| {
        let notes = 1.0 - score.chroma[i].iter().zip(&recording.chroma[j]).map(|(a, b)| a * b).sum::<f32>();
        (1.0 - STRIKES) * notes + STRIKES * (score.onsets[i] - recording.onsets[j]).abs()
    };
    // How far the recording moved to reach each pair.
    let mut moved = vec![0u8; n * m];
    let mut previous: Vec<f32> = (0..m).map(|j| cost(0, j)).collect();
    let mut current = vec![0.0f32; m];
    for i in 1..n {
        for j in 0..m {
            let mut best = (f32::INFINITY, 0u8);
            for (step, penalty) in MOVES {
                if step <= j {
                    let total = previous[j - step] + penalty;
                    if total < best.0 {
                        best = (total, step as u8);
                    }
                }
            }
            current[j] = best.0 + cost(i, j);
            moved[i * m + j] = best.1;
        }
        std::mem::swap(&mut previous, &mut current);
    }
    // The end in the recording where the whole score fits best, then back.
    let mut j = (0..m).min_by(|a, b| previous[*a].total_cmp(&previous[*b])).unwrap_or(0);
    let mut pairs: Vec<usize> = vec![0; n];
    let mut total = 0.0f64;
    for i in (0..n).rev() {
        pairs[i] = j;
        total += f64::from(cost(i, j));
        j = j.saturating_sub(usize::from(moved[i * m + j]));
    }
    // Notes are shared by many chords: even unrelated pictures look somewhat alike.
    let samples = 4000usize;
    let anyhow = (0..samples).map(|k| 1.0 - f64::from(cost((k * 7919) % n, (k * 104_729) % m))).sum::<f64>() / samples as f64;
    let along = 1.0 - total / n as f64;
    (pairs, ((along - anyhow) / (1.0 - anyhow).max(1e-6)).clamp(0.0, 1.0))
}

/// Finds where each beat of the score falls in a recording given as its
/// samples. None when either holds nothing to compare.
pub fn align(project: &Project, samples: &[f32], channels: usize, sample_rate: u32) -> Option<Alignment> {
    let score_seconds = project.beat_to_sec(project.end_beat());
    let recording_seconds = samples.len() as f64 / channels.max(1) as f64 / f64::from(sample_rate.max(1));
    if score_seconds <= 0.0 || recording_seconds <= 0.0 {
        return None;
    }
    // Fewer pictures a second for a long song, within the cells allowed.
    let fps = FPS.min((MOST_CELLS / (score_seconds * recording_seconds * FINER as f64)).sqrt()).max(2.0);
    let score = score_pictures(project, fps);
    let recording = audio_pictures(samples, channels, sample_rate, fps * FINER as f64);
    let (pairs, agreement) = warp(&score, &recording);
    // Each beat: the picture of the score it falls in, and so the instant of
    // the recording; averaged over the beats around it, the warp going by steps.
    let end = project.end_beat().ceil() as usize;
    let raw: Vec<Timing> = (0..=end)
        .map(|beat| {
            let at = project.beat_to_sec(beat as f64) * fps;
            let index = (at.floor() as usize).min(pairs.len() - 1);
            let next = (index + 1).min(pairs.len() - 1);
            let share = at - at.floor();
            let frame = pairs[index] as f64 * (1.0 - share) + pairs[next] as f64 * share;
            Timing { beat: beat as f64, sec: frame / (fps * FINER as f64) }
        })
        .collect();
    // The warp goes by steps, and strays where the score has little to
    // compare (its first notes, a held chord); the pace is steady over a few
    // beats. Each beat is read on the line that follows the nine beats around
    // it (at the ends, the nine nearest), the slope being the median of those
    // of every two of them, which a few strays do not move.
    const AROUND: usize = 9;
    let timing = raw
        .iter()
        .enumerate()
        .map(|(k, point)| {
            let first = k.saturating_sub(AROUND / 2).min(raw.len().saturating_sub(AROUND));
            let around = &raw[first..(first + AROUND).min(raw.len())];
            let median = |mut values: Vec<f64>| {
                values.sort_by(f64::total_cmp);
                values.get(values.len() / 2).copied().unwrap_or(0.0)
            };
            let mut slopes = Vec::new();
            for (a, p) in around.iter().enumerate() {
                for q in &around[a + 1..] {
                    slopes.push((q.sec - p.sec) / (q.beat - p.beat));
                }
            }
            let slope = median(slopes);
            let at = median(around.iter().map(|p| p.sec - slope * p.beat).collect());
            Timing { beat: point.beat, sec: at + slope * point.beat }
        })
        .collect();
    Some(Alignment { timing, agreement })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Content, Note, Point, Region, Track};
    use crate::{render_offline, Clips, Fonts};

    /// A song of chords that change every bar, at a tempo.
    fn song(tempo: f64) -> Project {
        let mut project = Project { tempo, ..Project::default() };
        let mut track = Track::new("Accords", TrackKind::Instrument);
        track.patch = "Electric piano".to_owned();
        track.sound = patches::find("Electric piano").unwrap().sound;
        let roots = [57u8, 53, 60, 55, 50, 52, 57, 48];
        let mut notes = Vec::new();
        for (bar, root) in roots.iter().enumerate() {
            for (k, interval) in [0u8, 4, 7, 12].iter().enumerate() {
                let minor = if bar % 3 == 0 && *interval == 4 { 3 } else { *interval };
                notes.push(Note { start: bar as f64 * 4.0 + k as f64 * 0.5, length: 3.5 - k as f64 * 0.5, pitch: root + minor, velocity: 100, ..Default::default() });
            }
        }
        let mut region = Region::new("Accords", 0.0, 32.0);
        region.content = Content::Midi(notes);
        track.regions.push(region);
        project.tracks.push(track);
        project
    }

    #[test]
    fn a_recording_at_another_pace_and_later_is_found() {
        // The "recording": the same song faster, then slower, after two seconds of silence.
        for pace in [132.0, 80.0] {
            let rate = 22_050;
            let mut recording = vec![0.0f32; rate as usize * 2 * 2];
            recording.extend(render_offline(&song(pace), &Clips::new(), &Fonts::new(), rate));
            // The score, at its own tempo.
            let found = align(&song(100.0), &recording, 2, rate).unwrap();
            assert!(found.agreement > FAIR, "{}", found.agreement);
            // Each bar where it is in the recording: two seconds, then the beats at its pace.
            for bar in 0..8 {
                let beat = bar as f64 * 4.0;
                let expected = 2.0 + beat * 60.0 / pace;
                let point = found.timing.iter().find(|t| t.beat == beat).unwrap();
                assert!((point.sec - expected).abs() < 0.2, "{pace}: bar {bar}: {} for {expected}", point.sec);
            }
        }
    }

    #[test]
    fn a_recording_that_slows_down_is_followed() {
        // Played at 132, then from the fifth bar at 104, as a band that drags.
        let rate = 22_050;
        let mut played = song(132.0);
        played.tempo_points.push(Point { beat: 16.0, value: 104.0 });
        let recording = render_offline(&played, &Clips::new(), &Fonts::new(), rate);
        let found = align(&song(100.0), &recording, 2, rate).unwrap();
        for bar in 0..8 {
            let beat = bar as f64 * 4.0;
            let expected = played.beat_to_sec(beat);
            let point = found.timing.iter().find(|t| t.beat == beat).unwrap();
            assert!((point.sec - expected).abs() < 0.2, "bar {bar}: {} for {expected}", point.sec);
        }
    }

    #[test]
    fn a_recording_of_another_song_agrees_little() {
        let rate = 22_050;
        // Noise, which has no notes.
        let mut seed = 1u32;
        let noise: Vec<f32> = (0..rate as usize * 2 * 20)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 8) as f32 / (1u32 << 24) as f32 - 0.5
            })
            .collect();
        let found = align(&song(100.0), &noise, 2, rate).unwrap();
        assert!(found.agreement < FAIR, "{}", found.agreement);
    }

    #[test]
    fn the_transform_finds_a_tone() {
        let n = 64;
        let mut re: Vec<f32> = (0..n).map(|i| (2.0 * std::f32::consts::PI * 5.0 * i as f32 / n as f32).sin()).collect();
        let mut im = vec![0.0f32; n];
        fft(&mut re, &mut im);
        let peak = (0..n / 2).max_by(|a, b| (re[*a].hypot(im[*a])).total_cmp(&re[*b].hypot(im[*b]))).unwrap();
        assert_eq!(peak, 5);
    }
}


#[cfg(test)]
mod timing {
    /// How long a song of four minutes takes to align. Run on demand, in release:
    /// `dev test --release -p og-engine -- --ignored four_minutes --nocapture`.
    #[test]
    #[ignore = "a measure, not a check"]
    fn four_minutes() {
        use super::*;
        use crate::model::{Content, Note, Region, Track};
        let mut project = Project { tempo: 100.0, ..Project::default() };
        let mut track = Track::new("Accords", TrackKind::Instrument);
        let mut notes = Vec::new();
        for bar in 0..100u32 {
            let root = [57u8, 53, 60, 55][bar as usize % 4];
            for (k, interval) in [0u8, 4, 7].iter().enumerate() {
                notes.push(Note { start: f64::from(bar) * 4.0 + k as f64 * 0.5, length: 3.0, pitch: root + interval, velocity: 100, ..Default::default() });
            }
        }
        let mut region = Region::new("Accords", 0.0, 400.0);
        region.content = Content::Midi(notes);
        track.regions.push(region);
        project.tracks.push(track);
        let rate = 44_100;
        let samples = vec![0.01f32; rate as usize * 2 * 250];
        let start = std::time::Instant::now();
        let found = align(&project, &samples, 2, rate).unwrap();
        eprintln!("four minutes aligned in {:?}, {} points", start.elapsed(), found.timing.len());
    }
}
