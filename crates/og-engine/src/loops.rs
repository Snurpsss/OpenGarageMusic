//! The built-in loop library. Loops are stored as notes relative to a key, so
//! they follow the project's tempo and key. Every pattern is original.

use crate::drummer;
use crate::model::{DrumStyle, DrummerParams, HatMode, Note, Perc};

pub struct LoopDef {
    pub name: &'static str,
    pub instrument: &'static str,
    pub genre: &'static str,
    pub mood: &'static str,
    pub bars: u32,
    /// Name of the built-in patch that plays the loop.
    pub patch: &'static str,
    kind: Kind,
}

enum Kind {
    Drums(DrumStyle, f32, HatMode, Perc, bool),
    /// (start, length, scale degree) over the whole loop, and the octave of degree 0.
    Line(&'static [(f32, f32, i8)], i8),
    /// One chord root (scale degree) per bar, played with a rhythm of (start, length).
    Chords(&'static [i8], &'static [(f32, f32)], i8),
    /// One chord root per bar, broken into notes of the given length in beats.
    Arp(&'static [i8], f32, i8),
}

pub const INSTRUMENTS: [&str; 5] = ["Drums", "Bass", "Keyboards", "Synth", "Guitar"];
pub const GENRES: [&str; 6] = ["Rock", "Pop", "Hip-hop", "Electro", "Funk", "Jazz & Latin"];
pub const MOODS: [&str; 4] = ["Calm", "Intense", "Happy", "Dark"];

const BASS_ROOTS: &[(f32, f32, i8)] = &[
    (0.0, 0.5, 0), (0.5, 0.5, 0), (1.0, 0.5, 0), (1.5, 0.5, 0), (2.0, 0.5, 0), (2.5, 0.5, 0), (3.0, 0.5, 0), (3.5, 0.5, 0),
    (4.0, 0.5, 5), (4.5, 0.5, 5), (5.0, 0.5, 5), (5.5, 0.5, 5), (6.0, 0.5, 5), (6.5, 0.5, 5), (7.0, 0.5, 5), (7.5, 0.5, 4),
    (8.0, 0.5, 3), (8.5, 0.5, 3), (9.0, 0.5, 3), (9.5, 0.5, 3), (10.0, 0.5, 3), (10.5, 0.5, 3), (11.0, 0.5, 3), (11.5, 0.5, 3),
    (12.0, 0.5, 4), (12.5, 0.5, 4), (13.0, 0.5, 4), (13.5, 0.5, 4), (14.0, 0.5, 4), (14.5, 0.5, 4), (15.0, 0.5, 6), (15.5, 0.5, 6),
];
const BASS_POP: &[(f32, f32, i8)] = &[
    (0.0, 1.5, 0), (1.5, 0.5, 0), (2.0, 1.0, 4), (3.0, 1.0, 2),
    (4.0, 1.5, 5), (5.5, 0.5, 5), (6.0, 1.0, 7), (7.0, 1.0, 4),
    (8.0, 1.5, 3), (9.5, 0.5, 3), (10.0, 1.0, 5), (11.0, 1.0, 3),
    (12.0, 1.5, 4), (13.5, 0.5, 4), (14.0, 1.0, 6), (15.0, 1.0, 4),
];
const BASS_FUNK: &[(f32, f32, i8)] = &[
    (0.0, 0.25, 0), (0.75, 0.25, 0), (1.5, 0.25, 7), (2.0, 0.25, 0), (2.75, 0.25, 4), (3.25, 0.25, 6), (3.5, 0.5, 7),
    (4.0, 0.25, 0), (4.75, 0.25, 0), (5.5, 0.25, 7), (6.0, 0.25, 0), (6.5, 0.25, 2), (7.0, 0.25, 3), (7.5, 0.5, 4),
];
const BASS_HIPHOP: &[(f32, f32, i8)] = &[
    (0.0, 1.75, 0), (2.5, 0.5, 0), (3.5, 0.5, -1),
    (4.0, 1.75, -2), (6.5, 0.5, -2), (7.5, 0.5, 2),
];
const BASS_ELECTRO: &[(f32, f32, i8)] = &[
    (0.5, 0.25, 0), (1.5, 0.25, 0), (2.5, 0.25, 0), (3.5, 0.25, 7),
    (4.5, 0.25, 0), (5.5, 0.25, 0), (6.5, 0.25, 0), (7.5, 0.25, 4),
    (8.5, 0.25, -2), (9.5, 0.25, -2), (10.5, 0.25, -2), (11.5, 0.25, 5),
    (12.5, 0.25, -1), (13.5, 0.25, -1), (14.5, 0.25, -1), (15.5, 0.25, 6),
];
const BASS_WALK: &[(f32, f32, i8)] = &[
    (0.0, 1.0, 0), (1.0, 1.0, 2), (2.0, 1.0, 4), (3.0, 1.0, 5),
    (4.0, 1.0, 7), (5.0, 1.0, 5), (6.0, 1.0, 4), (7.0, 1.0, 2),
    (8.0, 1.0, 3), (9.0, 1.0, 5), (10.0, 1.0, 7), (11.0, 1.0, 8),
    (12.0, 1.0, 4), (13.0, 1.0, 6), (14.0, 1.0, 8), (15.0, 1.0, 6),
];
const BASS_LATIN: &[(f32, f32, i8)] = &[
    (0.0, 1.0, 0), (1.5, 1.0, 4), (3.0, 1.0, 7),
    (4.0, 1.0, 3), (5.5, 1.0, 7), (7.0, 1.0, 10),
];
const MELODY_CALM: &[(f32, f32, i8)] = &[
    (0.0, 2.0, 4), (2.0, 1.0, 2), (3.0, 1.0, 1), (4.0, 3.0, 0), (7.0, 1.0, 1),
    (8.0, 2.0, 2), (10.0, 1.0, 4), (11.0, 1.0, 5), (12.0, 4.0, 4),
];
const MELODY_BRIGHT: &[(f32, f32, i8)] = &[
    (0.0, 0.5, 0), (0.5, 0.5, 2), (1.0, 1.0, 4), (2.0, 0.5, 5), (2.5, 0.5, 4), (3.0, 1.0, 2),
    (4.0, 0.5, 1), (4.5, 0.5, 2), (5.0, 1.0, 4), (6.0, 2.0, 7),
    (8.0, 0.5, 7), (8.5, 0.5, 5), (9.0, 1.0, 4), (10.0, 0.5, 2), (10.5, 0.5, 4), (11.0, 1.0, 5),
    (12.0, 1.0, 4), (13.0, 1.0, 2), (14.0, 2.0, 0),
];
const RIFF_ROCK: &[(f32, f32, i8)] = &[
    (0.0, 0.5, 0), (0.5, 0.5, 0), (1.0, 0.5, 2), (1.5, 0.5, 0), (2.0, 0.5, 4), (2.5, 0.5, 0), (3.0, 0.5, 2), (3.5, 0.5, 3),
    (4.0, 0.5, 0), (4.5, 0.5, 0), (5.0, 0.5, 2), (5.5, 0.5, 0), (6.0, 0.5, 6), (6.5, 0.5, 4), (7.0, 1.0, 2),
];

const PROG_POP: &[i8] = &[0, 4, 5, 3];
const PROG_SAD: &[i8] = &[5, 3, 0, 4];
const PROG_JAZZ: &[i8] = &[1, 4, 0, 5];
const PROG_TWO: &[i8] = &[0, 3];

const RHYTHM_WHOLE: &[(f32, f32)] = &[(0.0, 3.8)];
const RHYTHM_HALVES: &[(f32, f32)] = &[(0.0, 1.8), (2.0, 1.8)];
const RHYTHM_OFFBEAT: &[(f32, f32)] = &[(0.5, 0.4), (1.5, 0.4), (2.5, 0.4), (3.5, 0.4)];
const RHYTHM_SYNCOPATED: &[(f32, f32)] = &[(0.0, 0.7), (0.75, 0.7), (1.5, 0.9), (2.5, 0.4), (3.0, 0.7), (3.5, 0.4)];
const RHYTHM_STABS: &[(f32, f32)] = &[(0.0, 0.2), (0.75, 0.2), (1.5, 0.2), (2.5, 0.2), (3.25, 0.2)];

pub fn library() -> Vec<LoopDef> {
    use DrumStyle::{Electro, Funk, HipHop, Jazz, Latin, Pop, Rock};
    let d = |name, genre, mood, style, complexity, hats, perc, kit: bool, patch| LoopDef {
        name,
        instrument: "Drums",
        genre,
        mood,
        bars: 4,
        patch,
        kind: Kind::Drums(style, complexity, hats, perc, kit),
    };
    let l = |name, instrument, genre, mood, bars, patch, kind| LoopDef { name, instrument, genre, mood, bars, patch, kind };
    vec![
        d("Straight rock", "Rock", "Intense", Rock, 0.2, HatMode::Hats, Perc::Off, true, "Acoustic kit"),
        d("Edgy rock", "Rock", "Intense", Rock, 0.9, HatMode::Hats, Perc::Off, true, "Acoustic kit"),
        d("Tom rock", "Rock", "Dark", Rock, 0.5, HatMode::Toms, Perc::Off, true, "Acoustic kit"),
        d("Light pop", "Pop", "Happy", Pop, 0.3, HatMode::Hats, Perc::Tambourine, true, "Acoustic kit"),
        d("Dance pop", "Pop", "Happy", Pop, 0.8, HatMode::Hats, Perc::Clap, true, "Electro kit"),
        d("Heavy hip-hop", "Hip-hop", "Dark", HipHop, 0.3, HatMode::Hats, Perc::Off, true, "Hip-hop kit"),
        d("Bouncy hip-hop", "Hip-hop", "Happy", HipHop, 0.9, HatMode::Hats, Perc::Clap, true, "Hip-hop kit"),
        d("Four-on-the-floor electro", "Electro", "Intense", Electro, 0.3, HatMode::Hats, Perc::Clap, true, "Electro kit"),
        d("Busy electro", "Electro", "Intense", Electro, 1.0, HatMode::Hats, Perc::Shaker, true, "Electro kit"),
        d("Tight funk", "Funk", "Happy", Funk, 0.7, HatMode::Hats, Perc::Off, true, "Acoustic kit"),
        d("Jazz on the ride", "Jazz & Latin", "Calm", Jazz, 0.6, HatMode::Hats, Perc::Off, true, "Acoustic kit"),
        d("Latin clave", "Jazz & Latin", "Happy", Latin, 0.6, HatMode::Ride, Perc::Shaker, true, "Acoustic kit"),
        d("Shaker only", "Pop", "Calm", Pop, 0.6, HatMode::Off, Perc::Shaker, false, "Acoustic kit"),
        d("Tambourine only", "Rock", "Happy", Rock, 0.3, HatMode::Off, Perc::Tambourine, false, "Acoustic kit"),
        l("Root-note bass", "Bass", "Rock", "Intense", 4, "Electric bass", Kind::Line(BASS_ROOTS, 2)),
        l("Melodic pop bass", "Bass", "Pop", "Happy", 4, "Electric bass", Kind::Line(BASS_POP, 2)),
        l("Funk slap bass", "Bass", "Funk", "Happy", 2, "Slap bass", Kind::Line(BASS_FUNK, 2)),
        l("Deep hip-hop bass", "Bass", "Hip-hop", "Dark", 2, "Sub bass", Kind::Line(BASS_HIPHOP, 2)),
        l("Off-beat electro bass", "Bass", "Electro", "Intense", 4, "Synth bass", Kind::Line(BASS_ELECTRO, 2)),
        l("Walking jazz bass", "Bass", "Jazz & Latin", "Calm", 4, "Electric bass", Kind::Line(BASS_WALK, 2)),
        l("Latin bass", "Bass", "Jazz & Latin", "Happy", 2, "Electric bass", Kind::Line(BASS_LATIN, 2)),
        l("Held chords", "Keyboards", "Pop", "Calm", 4, "Electric piano", Kind::Chords(PROG_POP, RHYTHM_WHOLE, 4)),
        l("Half-note chords", "Keyboards", "Pop", "Happy", 4, "Electric piano", Kind::Chords(PROG_POP, RHYTHM_HALVES, 4)),
        l("Sad ballad", "Keyboards", "Pop", "Dark", 4, "Felt piano", Kind::Chords(PROG_SAD, RHYTHM_HALVES, 4)),
        l("Reggae off-beat", "Keyboards", "Funk", "Happy", 4, "Drawbar organ", Kind::Chords(PROG_POP, RHYTHM_OFFBEAT, 4)),
        l("Syncopated keyboard", "Keyboards", "Funk", "Happy", 2, "Funky keyboard", Kind::Chords(PROG_TWO, RHYTHM_SYNCOPATED, 4)),
        l("Jazz chords", "Keyboards", "Jazz & Latin", "Calm", 4, "Felt piano", Kind::Chords(PROG_JAZZ, RHYTHM_SYNCOPATED, 4)),
        l("Solemn organ", "Keyboards", "Rock", "Dark", 4, "Church organ", Kind::Chords(PROG_SAD, RHYTHM_WHOLE, 3)),
        l("Floating pad", "Synth", "Electro", "Calm", 4, "Warm pad", Kind::Chords(PROG_SAD, RHYTHM_WHOLE, 4)),
        l("Glass pad", "Synth", "Electro", "Calm", 4, "Glass pad", Kind::Chords(PROG_POP, RHYTHM_WHOLE, 5)),
        l("Fast arpeggio", "Synth", "Electro", "Intense", 4, "Synthetic pluck", Kind::Arp(PROG_POP, 0.25, 4)),
        l("Dark arpeggio", "Synth", "Electro", "Dark", 4, "Acid sequence", Kind::Arp(PROG_SAD, 0.25, 3)),
        l("Calm arpeggio", "Synth", "Pop", "Calm", 4, "Bells", Kind::Arp(PROG_POP, 0.5, 5)),
        l("Synth stabs", "Synth", "Hip-hop", "Intense", 2, "Square lead", Kind::Chords(PROG_TWO, RHYTHM_STABS, 4)),
        l("Gentle melody", "Synth", "Pop", "Calm", 4, "Flute", Kind::Line(MELODY_CALM, 5)),
        l("Cheerful melody", "Synth", "Pop", "Happy", 4, "Classic lead", Kind::Line(MELODY_BRIGHT, 5)),
        l("Arpeggiated guitar", "Guitar", "Pop", "Calm", 4, "Nylon guitar", Kind::Arp(PROG_POP, 0.5, 3)),
        l("Folk guitar", "Guitar", "Rock", "Happy", 4, "Steel guitar", Kind::Chords(PROG_POP, RHYTHM_SYNCOPATED, 3)),
        l("Muted riff", "Guitar", "Rock", "Intense", 2, "Muted guitar", Kind::Line(RIFF_ROCK, 3)),
        l("Sad guitar", "Guitar", "Pop", "Dark", 4, "Nylon guitar", Kind::Arp(PROG_SAD, 0.5, 3)),
        l("Cascading harp", "Guitar", "Jazz & Latin", "Calm", 4, "Harp", Kind::Arp(PROG_JAZZ, 0.25, 4)),
        l("Latin marimba", "Keyboards", "Jazz & Latin", "Happy", 2, "Marimba", Kind::Chords(PROG_TWO, RHYTHM_STABS, 4)),
    ]
}

/// Semitones above the tonic of a scale degree (which may be negative or above 6).
fn degree_semitones(degree: i32, minor: bool) -> i32 {
    const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
    const MINOR: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];
    let scale = if minor { MINOR } else { MAJOR };
    scale[degree.rem_euclid(7) as usize] + 12 * degree.div_euclid(7)
}

impl LoopDef {
    pub fn is_drums(&self) -> bool {
        matches!(self.kind, Kind::Drums(..))
    }

    /// Length in beats.
    pub fn length(&self, bar_len: f64) -> f64 {
        f64::from(self.bars) * bar_len
    }

    /// The loop's notes in the given key. Patterns are written in bars of four
    /// beats; other time signatures truncate or pad each bar.
    pub fn notes(&self, key: u8, minor: bool, bar_len: f64) -> Vec<Note> {
        let length = self.length(bar_len);
        let pitch = |degree: i32, octave: i8| -> u8 {
            let p = 12 * (i32::from(octave) + 1) + i32::from(key % 12) + degree_semitones(degree, minor);
            p.clamp(0, 127) as u8
        };
        let mut notes = Vec::new();
        match &self.kind {
            Kind::Drums(style, complexity, hats, perc, kit) => {
                let params = DrummerParams {
                    style: *style,
                    complexity: *complexity,
                    hats: *hats,
                    perc: *perc,
                    kick_snare: *kit,
                    fills: 0.3,
                    ..DrummerParams::default()
                };
                return drummer::generate(&params, length, bar_len);
            }
            Kind::Line(line, octave) => {
                // A short line repeats to fill the loop.
                let span = line.iter().map(|n| n.0 + n.1).fold(0.0f32, f32::max);
                let span = f64::from((span / 4.0).ceil().max(1.0)) * 4.0;
                let mut offset = 0.0;
                while offset < f64::from(self.bars) * 4.0 {
                    for &(start, len, degree) in *line {
                        notes.push((offset + f64::from(start), f64::from(len), pitch(i32::from(degree), *octave), 96));
                    }
                    offset += span;
                }
            }
            Kind::Chords(roots, rhythm, octave) => {
                for bar in 0..self.bars as usize {
                    let root = i32::from(roots[bar % roots.len()]);
                    for &(start, len) in *rhythm {
                        for (i, third) in [0, 2, 4].into_iter().enumerate() {
                            let velocity = if i == 0 { 92 } else { 80 };
                            notes.push((bar as f64 * 4.0 + f64::from(start), f64::from(len), pitch(root + third, *octave), velocity));
                        }
                    }
                }
            }
            Kind::Arp(roots, step, octave) => {
                const SHAPE: [i32; 8] = [0, 2, 4, 7, 4, 2, 4, 2];
                for bar in 0..self.bars as usize {
                    let root = i32::from(roots[bar % roots.len()]);
                    let count = (4.0 / step).round() as usize;
                    for i in 0..count {
                        let start = bar as f64 * 4.0 + i as f64 * f64::from(*step);
                        let velocity = if i % 4 == 0 { 100 } else { 80 };
                        notes.push((start, f64::from(*step) * 0.9, pitch(root + SHAPE[i % 8], *octave), velocity));
                    }
                }
            }
        }
        // Fit bars written in four beats to the project's bar length.
        notes
            .into_iter()
            .filter_map(|(start, len, pitch, velocity)| {
                let bar = (start / 4.0).floor();
                let within = start - bar * 4.0;
                let start = bar * bar_len + within;
                (within < bar_len && start < length)
                    .then(|| Note { start, length: len.min(length - start), pitch, velocity, string: None, ..Default::default() })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_loop_has_notes_inside_its_length() {
        let library = library();
        assert!(library.len() >= 40);
        for def in &library {
            for bar_len in [4.0, 3.0] {
                let notes = def.notes(2, true, bar_len);
                let length = def.length(bar_len);
                assert!(!notes.is_empty(), "{}", def.name);
                assert!(
                    notes.iter().all(|n| n.start >= 0.0 && n.start < length && n.length > 0.0),
                    "{}",
                    def.name
                );
            }
            assert!(INSTRUMENTS.contains(&def.instrument), "{}", def.name);
            assert!(GENRES.contains(&def.genre), "{}", def.name);
            assert!(MOODS.contains(&def.mood), "{}", def.name);
            assert!(crate::patches::find(def.patch).is_some(), "{}", def.patch);
        }
    }

    #[test]
    fn loops_follow_the_key() {
        let library = library();
        let bass = library.iter().find(|l| l.name == "Root-note bass").unwrap();
        let in_c = bass.notes(0, false, 4.0);
        let in_d = bass.notes(2, false, 4.0);
        assert_eq!(in_c[0].pitch + 2, in_d[0].pitch);
        // The sixth degree differs between major and minor.
        let major = bass.notes(0, false, 4.0);
        let minor = bass.notes(0, true, 4.0);
        assert!(major.iter().zip(&minor).any(|(a, b)| a.pitch != b.pitch));
        assert_eq!(degree_semitones(-1, false), -1);
        assert_eq!(degree_semitones(7, true), 12);
    }
}
