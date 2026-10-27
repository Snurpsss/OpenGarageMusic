//! A recording of the song beside its score: an audio file or a video, and
//! where in it each beat falls (its timing). From the timing come the
//! seconds of the recording at a beat and the beat at a second, and the
//! tempo the song is played at to keep in step with the recording.
//!
//! A project is played in one of two ways: as written, at its own tempo,
//! the recordings silent; or with its original sound, at the pace of the
//! recording that leads, the other tracks silent unless asked for.

use crate::model::{Content, Point, Project, Timing, TrackKind};

/// The fewest seconds between two points of a timing: closer points say
/// nothing of the pace.
const CLOSEST: f64 = 1e-3;

/// The points of a timing that can be used: in order of beat, each later in
/// the recording than the one before.
fn usable(timing: &[Timing]) -> Vec<Timing> {
    let mut points: Vec<Timing> = timing.iter().copied().filter(|t| t.beat.is_finite() && t.sec.is_finite()).collect();
    points.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    let mut kept: Vec<Timing> = Vec::with_capacity(points.len());
    for point in points {
        if kept.last().is_none_or(|last| point.beat > last.beat + 1e-9 && point.sec > last.sec + CLOSEST) {
            kept.push(point);
        }
    }
    kept
}

/// The two points a beat (or a second) falls between, or the nearest two
/// when it lies before the first or after the last.
fn around(points: &[Timing], at: f64, of: impl Fn(&Timing) -> f64) -> Option<(Timing, Timing)> {
    if points.len() < 2 {
        return None;
    }
    let index = points.partition_point(|p| of(p) <= at).clamp(1, points.len() - 1);
    Some((points[index - 1], points[index]))
}

/// The second of the recording a beat falls on; before the first point and
/// after the last, at the pace of the nearest two. None for a timing of
/// fewer than two points.
pub fn sec_at(timing: &[Timing], beat: f64) -> Option<f64> {
    let points = usable(timing);
    let (a, b) = around(&points, beat, |p| p.beat)?;
    Some(a.sec + (beat - a.beat) * (b.sec - a.sec) / (b.beat - a.beat))
}

/// The beat of the score at a second of the recording.
pub fn beat_at(timing: &[Timing], sec: f64) -> Option<f64> {
    let points = usable(timing);
    let (a, b) = around(&points, sec, |p| p.sec)?;
    Some(a.beat + (sec - a.sec) * (b.beat - a.beat) / (b.sec - a.sec))
}

/// The tempo of the recording: at its start, then where it changes, so that
/// the song played at it keeps in step. None for a timing of fewer than two
/// points.
pub fn tempo_of(timing: &[Timing]) -> Option<(f64, Vec<Point>)> {
    let points = usable(timing);
    if points.len() < 2 {
        return None;
    }
    let pace = |a: &Timing, b: &Timing| ((b.beat - a.beat) * 60.0 / (b.sec - a.sec)).clamp(20.0, 400.0);
    let first = pace(&points[0], &points[1]);
    let mut changes: Vec<Point> = Vec::new();
    for pair in points.windows(2).skip(1) {
        let tempo = pace(&pair[0], &pair[1]);
        let last = changes.last().map_or(first, |p| p.value);
        if (tempo - last).abs() > 1e-6 {
            changes.push(Point { beat: pair[0].beat, value: tempo });
        }
    }
    Some((first, changes))
}

/// The recording that leads: the track marked so, or else the first track
/// with a timing.
pub fn lead(project: &Project) -> Option<usize> {
    let recordings = || project.tracks.iter().enumerate().filter(|(_, t)| t.timing.len() >= 2);
    recordings().find(|(_, t)| t.lead).or_else(|| recordings().next()).map(|(index, _)| index)
}

/// Whether a track is a recording of the song rather than a part of it.
pub fn is_recording(project: &Project, track: usize) -> bool {
    project.tracks.get(track).is_some_and(|t| t.kind == TrackKind::Video || !t.timing.is_empty())
}

/// The project as it is played: as written, the recordings silent; or, with
/// its original sound, at the pace of the recording that leads, which alone
/// sounds with the tracks asked to play along.
pub fn as_played(project: &Project) -> Project {
    let mut played = project.clone();
    let lead = lead(project).filter(|_| project.follow);
    let Some((lead, (tempo, changes))) = lead.and_then(|lead| tempo_of(&project.tracks[lead].timing).map(|t| (lead, t))) else {
        for (index, track) in played.tracks.iter_mut().enumerate() {
            if is_recording(project, index) {
                track.mute = true;
            }
        }
        return played;
    };
    played.tempo = tempo;
    played.tempo_points = changes;
    // The count-in would play before the recording does.
    played.count_in = false;
    let timing = project.tracks[lead].timing.clone();
    for (index, track) in played.tracks.iter_mut().enumerate() {
        if index == lead {
            track.mute = track.kind == TrackKind::Video;
            track.solo = false;
            // Its sound starts where the recording is at the region's start.
            for region in &mut track.regions {
                if let Content::Audio(audio) = &mut region.content {
                    audio.offset = sec_at(&timing, region.start).unwrap_or(0.0).max(0.0);
                }
            }
        } else if is_recording(project, index) {
            track.mute = true;
        } else {
            track.mute = !track.with_recording;
            track.solo = false;
        }
    }
    played
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AudioRef, Region, Track};

    fn timing(points: &[(f64, f64)]) -> Vec<Timing> {
        points.iter().map(|&(beat, sec)| Timing { beat, sec }).collect()
    }

    #[test]
    fn beats_and_seconds_of_a_recording() {
        // Bars of four beats: the first at 0.5 s, then two seconds each, then 2.5.
        let t = timing(&[(0.0, 0.5), (4.0, 2.5), (8.0, 4.5), (12.0, 7.0)]);
        assert_eq!(sec_at(&t, 2.0), Some(1.5));
        assert_eq!(sec_at(&t, 10.0), Some(5.75));
        assert_eq!(sec_at(&t, -4.0), Some(-1.5), "before the first point, at the first pace");
        assert_eq!(sec_at(&t, 16.0), Some(9.5), "after the last, at the last pace");
        assert_eq!(beat_at(&t, 5.75), Some(10.0));
        assert_eq!(beat_at(&t, 0.0), Some(-1.0));
        assert_eq!(sec_at(&t[..1], 2.0), None);
        // Points out of order or too close are set aside.
        let messy = timing(&[(8.0, 4.5), (0.0, 0.5), (4.0, 2.5), (4.0, 2.6), (6.0, 2.5)]);
        assert_eq!(sec_at(&messy, 6.0), Some(3.5));
    }

    #[test]
    fn the_tempo_of_a_recording_keeps_the_song_in_step() {
        let t = timing(&[(0.0, 0.5), (4.0, 2.5), (8.0, 4.5), (12.0, 7.0)]);
        let (first, changes) = tempo_of(&t).unwrap();
        assert_eq!(first, 120.0);
        assert_eq!(changes, [Point { beat: 8.0, value: 96.0 }], "one change, where the pace changes");
        let mut project = Project { tempo: first, tempo_points: changes, ..Project::default() };
        for beat in [4.0, 8.0, 12.0, 20.0] {
            let expected = sec_at(&t, beat).unwrap() - sec_at(&t, 0.0).unwrap();
            assert!((project.beat_to_sec(beat) - expected).abs() < 1e-9, "beat {beat}");
        }
        project.tempo_points.clear();
    }

    #[test]
    fn played_as_written_or_with_the_original_sound() {
        let mut project = Project::default();
        let mut guitar = Track::new("Guitare", TrackKind::Instrument);
        guitar.regions.push(Region::new("Guitare", 0.0, 16.0));
        let mut bass = Track::new("Basse", TrackKind::Instrument);
        bass.with_recording = true;
        let mut song = Track::new("Morceau", TrackKind::Audio);
        song.timing = timing(&[(0.0, 1.5), (4.0, 3.5), (8.0, 5.5)]);
        let mut region = Region::new("Morceau", 0.0, 16.0);
        region.content = Content::Audio(AudioRef::new(7));
        song.regions.push(region);
        let mut video = Track::new("Clip", TrackKind::Video);
        video.timing = timing(&[(0.0, 0.2), (4.0, 2.2)]);
        project.tracks = vec![guitar, bass, song, video];
        project.tempo = 90.0;

        // As written: the recordings are silent, the tempo is the score's.
        let played = as_played(&project);
        let muted: Vec<bool> = played.tracks.iter().map(|t| t.mute).collect();
        assert_eq!(muted, [false, false, true, true]);
        assert_eq!(played.tempo, 90.0);

        // With the original sound: the first recording leads, at its pace.
        project.follow = true;
        let played = as_played(&project);
        let muted: Vec<bool> = played.tracks.iter().map(|t| t.mute).collect();
        assert_eq!(muted, [true, false, false, true], "the song and the bass asked to play along");
        assert_eq!((played.tempo, played.count_in), (120.0, false));
        let Content::Audio(audio) = &played.tracks[2].regions[0].content else { unreachable!() };
        assert_eq!(audio.offset, 1.5, "the recording starts where the first beat falls");

        // The video leads when marked so: it has no sound of the program's.
        project.tracks[3].lead = true;
        assert_eq!(lead(&project), Some(3));
        let played = as_played(&project);
        assert!(played.tracks[2].mute && played.tracks[3].mute);
        assert_eq!(played.tempo, 120.0);
    }
}
