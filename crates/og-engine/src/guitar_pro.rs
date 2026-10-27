//! Reader for the files of Guitar Pro, the tablature editor: the binary
//! files of versions 3, 4 and 5 (`.gp3`, `.gp4`, `.gp5`), and the XML score
//! of version 6, in a container of its own (`.gpx`, opened by [`crate::gpx`]),
//! and of versions 7 and 8, zipped (`.gp`). The files of versions 1 and 2
//! (`.gtp`) are recognised and refused, naming their version. MusicXML scores
//! are read into the same song by `musicxml.rs`.
//! What is read becomes the parts of
//! [`crate::songsterr`], so that a Guitar Pro song and a Songsterr tablature
//! make the same kind of project.
//!
//! The layouts come from the public descriptions of the formats. Every read
//! is bounded: a truncated or corrupt file gives an error, never a crash.
//! What the program cannot play as notes (page setup, colours, lyrics,
//! chord diagrams beyond their name, the sound settings of Guitar Pro's own
//! synthesiser) is passed over.

use std::collections::HashMap;
use std::path::Path;

use crate::model::{Articulation, ChordMark, Marker, Note, Point, Project, Slide, Swing, Vibrato};
use crate::songsterr::{self, Part, DRUMS};

/// A song read from a Guitar Pro file, ready to become a project.
#[derive(Clone, Debug, PartialEq)]
pub struct Score {
    pub title: String,
    pub artist: String,
    pub parts: Vec<Part>,
}

impl Score {
    /// Which file formats are read, for a file dialog: lowercase extensions,
    /// those of Guitar Pro, then those of MusicXML.
    pub const EXTENSIONS: [&'static str; 8] = ["gp3", "gp4", "gp5", "gpx", "gp", "musicxml", "mxl", "xml"];

    /// The General MIDI program of each part (or `songsterr::DRUMS`), to pick real instruments.
    pub fn programs(&self) -> Vec<u32> {
        self.parts.iter().map(|p| p.instrument_id).collect()
    }

    /// A new project of the parts of the score, one track each.
    pub fn project(&self) -> Project {
        let mut project = songsterr::build_project(&self.artist, &self.title, &self.parts);
        project.lyrics = crate::lyrics::from_parts(&self.parts);
        project
    }
}

/// Reads a Guitar Pro file of any version the program knows, or a MusicXML
/// score, told apart by what they hold.
pub fn parse(bytes: &[u8]) -> Result<Score, String> {
    if bytes.starts_with(b"PK\x03\x04") {
        if musicxml::is_archive(bytes) {
            return musicxml::parse(bytes);
        }
        return gpif::parse_archive(bytes).map(|song| song.score());
    }
    if musicxml::is_score(bytes) {
        return musicxml::parse(bytes);
    }
    // The score of a version 7 file taken out of its archive.
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    if head.contains("<GPIF") {
        return gpif::parse_gpif(&String::from_utf8_lossy(bytes)).map(|song| song.score());
    }
    if crate::gpx::is_container(bytes) {
        let files = crate::gpx::files(bytes)?;
        let score = files.get("score.gpif").ok_or("the Guitar Pro 6 file holds no score (score.gpif)")?;
        return gpif::parse_gpif(&String::from_utf8_lossy(score)).map(|song| song.score());
    }
    binary::parse(bytes).map(|song| song.score())
}

/// Whether a text file is a score the program reads: MusicXML, or the score
/// of a version 7 file out of its archive.
pub fn is_score_text(bytes: &[u8]) -> bool {
    musicxml::is_score(bytes) || String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).contains("<GPIF")
}

/// Whether a file name ends in an extension of the formats read.
pub fn is_guitar_pro(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| Score::EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

// ----- what the formats have in common ----------------------------------------------

/// The longest bend, in semitones, the project keeps.
const BEND_LIMIT: f32 = 24.0;

/// A grace note never sounds longer than this, in beats.
const GRACE_LENGTH: f64 = 0.125;

/// A song as every format describes it: bars, tracks and the beats of each
/// track in each bar, before the repeats are unfolded.
#[derive(Clone, Debug, Default)]
struct Song {
    title: String,
    artist: String,
    /// Quarter notes per minute at the start.
    tempo: f64,
    key: Option<(u8, bool)>,
    bars: Vec<BarInfo>,
    tracks: Vec<Track>,
    /// The words: the track they are sung on, the bar each line starts at,
    /// and its text.
    lyrics: Vec<(usize, usize, String)>,
}

#[derive(Clone, Debug, Default)]
struct BarInfo {
    signature: (u32, u32),
    repeat_start: bool,
    /// How many times in all the section ending here is played.
    repeat: Option<u32>,
    /// Passes of the repeat this bar is played on; empty for every pass.
    endings: Vec<u32>,
    marker: Option<String>,
    /// Tempo changes written on the bar: (beats from its start, quarter notes per minute).
    tempos: Vec<(f64, f64)>,
    /// The feel from this bar on, when the bar sets one: 0 straight, 0.5
    /// eighth notes in a triplet feel, 0.25 sixteenths.
    feel: Option<f64>,
    /// The signs of the score written over the bar.
    signs: Signs,
}

/// Where a jump leads after the passage it sends back to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Until {
    /// The end of the song.
    End,
    Fine,
    Coda,
    DoubleCoda,
}

/// A jump back, taken once, at the end of its bar: to the start (D.C.), to
/// the segno (D.S.) or to the double segno.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Jump {
    Capo(Until),
    Segno(Until),
    SegnoSegno(Until),
}

/// The signs of the score on a bar: the places a jump goes to, the end of
/// a passage played again (fine), the bar from which a jump goes on to the
/// coda ("to coda"), and a jump back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Signs {
    segno: bool,
    segno_segno: bool,
    coda: bool,
    double_coda: bool,
    fine: bool,
    to_coda: bool,
    to_double_coda: bool,
    jump: Option<Jump>,
}

impl Signs {
    /// Sets the sign Guitar Pro (and its XML score) names so.
    fn set(&mut self, name: &str) {
        let jump = |kind: fn(Until) -> Jump, rest: &str| -> Option<Jump> {
            Some(kind(match rest {
                "" => Until::End,
                "AlFine" => Until::Fine,
                "AlCoda" => Until::Coda,
                "AlDoubleCoda" => Until::DoubleCoda,
                _ => return None,
            }))
        };
        match name {
            "Segno" => self.segno = true,
            "SegnoSegno" => self.segno_segno = true,
            "Coda" => self.coda = true,
            "DoubleCoda" => self.double_coda = true,
            "Fine" => self.fine = true,
            "DaCoda" => self.to_coda = true,
            "DaDoubleCoda" => self.to_double_coda = true,
            _ => {
                let found = if let Some(rest) = name.strip_prefix("DaSegnoSegno") {
                    jump(Jump::SegnoSegno, rest)
                } else if let Some(rest) = name.strip_prefix("DaSegno") {
                    jump(Jump::Segno, rest)
                } else if let Some(rest) = name.strip_prefix("DaCapo") {
                    jump(Jump::Capo, rest)
                } else {
                    None
                };
                if found.is_some() {
                    self.jump = found;
                }
            }
        }
    }
}

/// The names of the 19 signs of a version 5 file, in the order its header
/// gives the bar of each.
const SIGNS_OF_VERSION_5: [&str; 19] = [
    "Coda",
    "DoubleCoda",
    "Segno",
    "SegnoSegno",
    "Fine",
    "DaCapo",
    "DaCapoAlCoda",
    "DaCapoAlDoubleCoda",
    "DaCapoAlFine",
    "DaSegno",
    "DaSegnoAlCoda",
    "DaSegnoAlDoubleCoda",
    "DaSegnoAlFine",
    "DaSegnoSegno",
    "DaSegnoSegnoAlCoda",
    "DaSegnoSegnoAlDoubleCoda",
    "DaSegnoSegnoAlFine",
    "DaCoda",
    "DaDoubleCoda",
];

/// The order the bars are played in, the jumps of the score taken: the
/// repeats as `base` unfolds them up to the first jump back (taken after
/// the last pass of its bar), then the bars from where it leads, once each
/// and on their last endings, to the end, the fine, or the "to coda" sign
/// and on from the coda.
fn play_with_jumps(bars: &[BarInfo], base: Vec<usize>) -> Vec<usize> {
    let count = bars.len();
    let find = |wanted: fn(&Signs) -> bool| bars.iter().position(|b| wanted(&b.signs));
    let leads = |jump: Jump| match jump {
        Jump::Capo(until) => Some((0, until)),
        Jump::Segno(until) => find(|s| s.segno).map(|at| (at, until)),
        Jump::SegnoSegno(until) => find(|s| s.segno_segno).map(|at| (at, until)),
    };
    let Some((at, (target, mut until))) = base.iter().enumerate().rev().find_map(|(at, &bar)| Some((at, leads(bars[bar].signs.jump?)?))) else {
        return base;
    };
    // Of bars with alternate endings, only those of the last pass are played again.
    let last_pass = |bar: usize| -> bool {
        let endings = &bars[bar].endings;
        if endings.is_empty() {
            return true;
        }
        let mut highest = 0;
        for (k, later) in bars.iter().enumerate().skip(bar).take(64) {
            if k > bar && (later.repeat_start || later.endings.is_empty()) {
                break;
            }
            highest = highest.max(later.endings.iter().copied().max().unwrap_or(0));
        }
        endings.contains(&highest)
    };
    let mut order: Vec<usize> = base[..=at].to_vec();
    let mut bar = target;
    while bar < count && order.len() < base.len() + 4 * count {
        if !last_pass(bar) {
            bar += 1;
            continue;
        }
        order.push(bar);
        let signs = &bars[bar].signs;
        match until {
            Until::Fine if signs.fine => break,
            Until::Coda if signs.to_coda => match find(|s| s.coda).filter(|&coda| coda != bar) {
                Some(coda) => {
                    bar = coda;
                    until = Until::End;
                    continue;
                }
                None => break,
            },
            Until::DoubleCoda if signs.to_double_coda => match find(|s| s.double_coda).filter(|&coda| coda != bar) {
                Some(coda) => {
                    bar = coda;
                    until = Until::End;
                    continue;
                }
                None => break,
            },
            _ => {}
        }
        bar += 1;
    }
    order
}

/// The feels of a song in the order of play, as a region keeps them: a
/// change wherever the feel of the bar played differs from the one before.
fn feels(bars: &[BarInfo], order: &[usize], starts: &[f64]) -> Vec<Swing> {
    let mut feel = 0.0;
    let mut current: Vec<f64> = Vec::with_capacity(bars.len());
    for bar in bars {
        feel = bar.feel.unwrap_or(feel);
        current.push(feel);
    }
    let mut out: Vec<Swing> = Vec::new();
    let mut last = 0.0;
    for (&bar, &start) in order.iter().zip(starts) {
        let feel = current[bar];
        if (feel - last).abs() > 1e-9 {
            out.push(if feel > 0.0 { Swing::triplet(start, feel < 0.4) } else { Swing::straight(start) });
            last = feel;
        }
    }
    out
}

impl BarInfo {
    fn length(&self) -> f64 {
        f64::from(self.signature.0) * 4.0 / f64::from(self.signature.1)
    }
}

#[derive(Clone, Debug, Default)]
struct Track {
    name: String,
    /// General MIDI program, or [`DRUMS`].
    program: u32,
    /// Open strings, highest first.
    tuning: Vec<i32>,
    capo: i32,
    /// The beats of each voice of each bar: `bars[bar][voice]`.
    bars: Vec<Vec<Vec<Beat>>>,
}

impl Track {
    fn is_drums(&self) -> bool {
        self.program == DRUMS
    }
}

#[derive(Clone, Debug, Default)]
struct Beat {
    /// Beats (quarter notes) the beat lasts.
    length: f64,
    rest: bool,
    /// The whole beat is a grace note before the next one, taking no time;
    /// or, `grace_on_beat`, on the beat, the next one coming that much later.
    grace: bool,
    grace_on_beat: bool,
    /// Tremolo picking over every note of the beat, every so many beats.
    tremolo: Option<f32>,
    chord: Option<String>,
    /// A tempo change at this beat, in quarter notes per minute.
    tempo: Option<f64>,
    /// A tremolo bar over the beat, as bend points.
    whammy: Vec<(f32, f32)>,
    vibrato: Option<Vibrato>,
    /// A harmonic over every note of the beat: whether it is natural.
    harmonic: Option<bool>,
    /// Loudness of the beat, as a velocity, when the file writes one here.
    velocity: Option<u8>,
    notes: Vec<BeatNote>,
    /// The syllable sung on the beat, when the file writes it there (MusicXML).
    lyric: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct BeatNote {
    /// String, 0 being the highest.
    string: usize,
    fret: i32,
    tie: bool,
    velocity: Option<u8>,
    dead: bool,
    ghost: bool,
    accent: u8,
    slide: Option<Slide>,
    bend: Vec<(f32, f32)>,
    hammer: bool,
    vibrato: Option<Vibrato>,
    /// A harmonic: whether it is natural, and the fret the string is touched at.
    harmonic: Option<(bool, i32)>,
    staccato: bool,
    let_ring: bool,
    palm_mute: bool,
    /// A grace note led into this one.
    grace: Option<Grace>,
    /// Tremolo picking, every so many beats.
    tremolo: Option<f32>,
    /// A trill with another note, each lasting so many beats.
    trill: Option<(TrillTo, f32)>,
}

/// A grace note written on a note: its fret, loudness and length, and
/// whether it is played on the beat, the note then coming that much later.
#[derive(Clone, Copy, Debug, Default)]
struct Grace {
    fret: i32,
    velocity: u8,
    length: f64,
    on_beat: bool,
}

/// The other note of a trill: a fret on the same string, or a pitch.
#[derive(Clone, Copy, Debug)]
enum TrillTo {
    Fret(i32),
    Pitch(u8),
}

/// A Guitar Pro loudness (1 = ppp … 8 or 9 = fff) as a velocity.
fn velocity_of_dynamic(dynamic: i64) -> u8 {
    let name = match dynamic {
        i64::MIN..=1 => "ppp",
        2 => "pp",
        3 => "p",
        4 => "mp",
        5 => "mf",
        6 => "f",
        7 => "ff",
        _ => "fff",
    };
    songsterr::velocity_of(name).unwrap_or(96)
}

/// Bend points as the formats write them, positions over 0 to 60 along the
/// note and heights in hundredths of a tone, as the project keeps them.
fn bend_points(points: &[(f64, f64)], span: f64) -> Vec<(f32, f32)> {
    points.iter().map(|&(position, value)| ((position / span).clamp(0.0, 1.0) as f32, (value / 50.0).clamp(-f64::from(BEND_LIMIT), f64::from(BEND_LIMIT)) as f32)).collect()
}

/// Beats of a note value and its dots and tuplet: `value` is 1 for a whole
/// note, 4 for a quarter, 8 for an eighth and so on.
fn beats_of(value: f64, dots: u32, tuplet: Option<(u32, u32)>) -> f64 {
    let mut beats = if value > 0.0 { 4.0 / value } else { 1.0 };
    for dot in 0..dots.min(3) {
        beats += 4.0 / value / f64::from(2u32.pow(dot + 1));
    }
    if let Some((num, den)) = tuplet {
        if num > 0 && den > 0 {
            beats *= f64::from(den) / f64::from(num);
        }
    }
    beats
}

/// How many notes a tuplet of `n` takes the time of: two for a triplet,
/// four for a quintuplet, eight for the larger ones.
fn tuplet_of(n: u32) -> Option<(u32, u32)> {
    match n {
        3 => Some((3, 2)),
        5..=7 => Some((n, 4)),
        9..=15 => Some((n, 8)),
        _ => None,
    }
}

impl Song {
    /// The score: each track as a part, repeats unfolded, times in beats.
    fn score(&self) -> Score {
        // The order of play and where each bar starts the first time.
        let bars: Vec<songsterr::Bar> = self
            .bars
            .iter()
            .map(|bar| songsterr::Bar { voices: &[], length: bar.length(), signature: bar.signature, repeat_start: bar.repeat_start, repeat: bar.repeat, endings: bar.endings.clone() })
            .collect();
        let order = play_with_jumps(&self.bars, songsterr::play_order(&bars));
        let mut first_start: Vec<Option<f64>> = vec![None; bars.len()];
        let mut starts = Vec::with_capacity(order.len());
        let mut at = 0.0;
        for &index in &order {
            starts.push(at);
            if first_start[index].is_none() {
                first_start[index] = Some(at);
            }
            at += bars[index].length;
        }
        let length = at;

        // The tempo: the song's at the start, then each change where it is played.
        let mut tempo = Vec::new();
        if self.tempo > 0.0 {
            tempo.push(Point { beat: 0.0, value: self.tempo });
        }
        for (&index, &bar_start) in order.iter().zip(&starts) {
            for &(offset, bpm) in &self.bars[index].tempos {
                if bpm.is_finite() && bpm > 0.0 {
                    tempo.push(Point { beat: bar_start + offset, value: bpm });
                }
            }
        }

        // The section names, each reaching to the next.
        let mut markers: Vec<Marker> = Vec::new();
        for (index, bar) in self.bars.iter().enumerate() {
            if let (Some(name), Some(Some(start))) = (&bar.marker, first_start.get(index)) {
                if let Some(last) = markers.last_mut() {
                    last.length = (start - last.start).max(bar.length());
                }
                markers.push(Marker { name: name.clone(), start: *start, length: bar.length() });
            }
        }
        if let Some(last) = markers.last_mut() {
            last.length = (length - last.start).max(last.length);
        }

        let signature = self.bars.first().map_or((4, 4), |bar| bar.signature);
        let swing = feels(&self.bars, &order, &starts);
        let parts = self
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| {
                let (notes, chords, changes) = self.notes_of(track, &order, &starts);
                let mut tempo = tempo.clone();
                tempo.extend(changes);
                tempo.sort_by(|a, b| a.beat.total_cmp(&b.beat));
                tempo.dedup_by(|b, a| (a.beat - b.beat).abs() < 1e-9 && (a.value - b.value).abs() < 1e-9);
                Part {
                    name: track.name.clone(),
                    instrument_id: track.program,
                    notes,
                    length,
                    tempo,
                    signature,
                    key: self.key,
                    markers: markers.clone(),
                    tuning: if track.is_drums() { Vec::new() } else { track.tuning.iter().map(|t| t.clamp(&0, &127)).map(|t| *t as u8).collect() },
                    chords,
                    swing: swing.clone(),
                    bar_starts: starts.clone(),
                    lyrics: self.lyrics_of(index, &order, &starts),
                }
            })
            .collect();
        Score { title: self.title.trim().to_owned(), artist: self.artist.trim().to_owned(), parts }
    }

    /// The notes of a track in the order of play, with the chord names over
    /// them and the tempo changes written in its beats.
    /// The words sung on a track: their syllables on the beats of its first
    /// voice that strike, from the bar each line starts at, as played.
    fn lyrics_of(&self, index: usize, order: &[usize], starts: &[f64]) -> Vec<crate::lyrics::Syllable> {
        let Some(track) = self.tracks.get(index) else { return Vec::new() };
        // The beats of the first voice, as written, from bar `from` on.
        let written = |from: usize| -> Vec<crate::lyrics::Beat> {
            let mut out = Vec::new();
            let mut bar_at = 0.0;
            for (bar, voices) in track.bars.iter().enumerate().skip(from) {
                let mut position = 0.0;
                for beat in voices.first().into_iter().flatten().filter(|b| !b.grace) {
                    let sounds = !beat.rest && !beat.notes.is_empty();
                    let strikes = sounds && beat.notes.iter().any(|n| !n.tie);
                    out.push(crate::lyrics::Beat { bar, position, at: bar_at + position, length: beat.length, strikes, sounds });
                    position += beat.length;
                }
                bar_at += self.bars.get(bar).map_or(position, BarInfo::length);
            }
            out
        };
        let mut lyrics = Vec::new();
        for (_, from, text) in self.lyrics.iter().filter(|(t, _, _)| *t == index) {
            let slots = crate::lyrics::slots(&written(*from));
            crate::lyrics::place(text, &slots, order, starts, &mut lyrics);
        }
        // Syllables written on the beats themselves (MusicXML).
        for (&bar, &bar_start) in order.iter().zip(starts) {
            let mut position = 0.0;
            for beat in track.bars.get(bar).and_then(|voices| voices.first()).into_iter().flatten() {
                if let Some(text) = beat.lyric.as_ref().filter(|_| !beat.grace) {
                    let at = bar_start + position;
                    if !lyrics.iter().any(|s| (s.beat - at).abs() < 1e-6) {
                        lyrics.push(crate::lyrics::Syllable { beat: at, text: text.clone(), line_start: false });
                    }
                }
                if !beat.grace {
                    position += beat.length;
                }
            }
        }
        lyrics.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        crate::lyrics::mark_lines(&mut lyrics, self.bars.first().map_or(4.0, BarInfo::length));
        lyrics
    }

    fn notes_of(&self, track: &Track, order: &[usize], starts: &[f64]) -> (Vec<Note>, Vec<ChordMark>, Vec<Point>) {
        let drums = track.is_drums();
        let mut notes: Vec<Note> = Vec::new();
        let mut chords: Vec<ChordMark> = Vec::new();
        let mut tempo: Vec<Point> = Vec::new();
        // The last note struck on each string of each voice, for the ties.
        let mut open: HashMap<(usize, usize), usize> = HashMap::new();
        for (&index, &bar_start) in order.iter().zip(starts) {
            let bar_length = self.bars[index].length();
            let Some(voices) = track.bars.get(index) else { continue };
            for (voice_index, voice) in voices.iter().enumerate() {
                let mut position = 0.0;
                // A grace beat played on the beat delays the beat after it.
                let mut delay = 0.0f64;
                for beat in voice {
                    let mut start = bar_start + position;
                    if !beat.grace {
                        position += beat.length;
                    }
                    if let Some(name) = beat.chord.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                        if !chords.iter().any(|c| (c.beat - start).abs() < 1e-6) {
                            chords.push(ChordMark { beat: start, name: name.to_owned() });
                        }
                    }
                    if let Some(bpm) = beat.tempo.filter(|t| t.is_finite() && *t > 0.0) {
                        tempo.push(Point { beat: start, value: bpm });
                    }
                    if beat.rest || start >= bar_start + bar_length + 1e-6 {
                        continue;
                    }
                    let mut sounding = if beat.grace { beat.length.min(GRACE_LENGTH) } else { beat.length };
                    if beat.grace {
                        // Before the beat it leads to, or on it, which then comes later.
                        if beat.grace_on_beat {
                            delay = sounding;
                        } else {
                            start = (start - sounding).max(bar_start);
                        }
                    } else if delay > 0.0 {
                        let taken = delay.min(sounding * 0.5);
                        start += taken;
                        sounding -= taken;
                        delay = 0.0;
                    }
                    for note in &beat.notes {
                        if note.tie {
                            if let Some(&earlier) = open.get(&(voice_index, note.string)) {
                                let held = &mut notes[earlier];
                                held.length = (start + sounding - held.start).max(held.length);
                            }
                            continue;
                        }
                        let Some(pitch) = self.pitch_of(track, note) else { continue };
                        let base = note.velocity.or(beat.velocity).unwrap_or(96);
                        // A grace note written on the note: before it, or on its beat,
                        // the note then starting that much later.
                        let (mut start, mut sounding) = (start, sounding);
                        if let Some(grace) = note.grace.filter(|_| !beat.grace) {
                            let length = grace.length.clamp(0.0625, 0.25).min(sounding * 0.5);
                            let on = if grace.on_beat { start } else { (start - length).max(bar_start) };
                            if grace.on_beat {
                                start += length;
                                sounding -= length;
                            }
                            let lead = BeatNote { string: note.string, fret: grace.fret, ..BeatNote::default() };
                            if let Some(pitch) = self.pitch_of(track, &lead) {
                                notes.push(Note {
                                    start: on,
                                    length,
                                    pitch,
                                    velocity: grace.velocity,
                                    string: (!drums).then_some(note.string.min(255) as u8),
                                    articulation: Articulation::default(),
                                });
                            }
                        }
                        let mut velocity = f64::from(base) + 12.0 * f64::from(note.accent.min(2));
                        if note.ghost {
                            velocity *= 0.6;
                        }
                        let articulation = if drums {
                            Articulation::default()
                        } else {
                            Articulation {
                                slide: note.slide,
                                bend: if note.bend.is_empty() { beat.whammy.clone() } else { note.bend.clone() },
                                hammer: note.hammer,
                                vibrato: note.vibrato.or(beat.vibrato),
                                harmonic: note.harmonic.map(|(_, touched)| touched.clamp(0, 127) as u8),
                                dead: note.dead,
                                ghost: note.ghost,
                                staccato: note.staccato,
                                accent: note.accent.min(2),
                                let_ring: note.let_ring,
                                palm_mute: note.palm_mute,
                                tremolo: note.tremolo.or(beat.tremolo).filter(|t| (0.03..=1.0).contains(t)),
                                trill: note.trill.and_then(|(to, period)| {
                                    let other = match to {
                                        TrillTo::Pitch(pitch) => Some(pitch),
                                        TrillTo::Fret(fret) => self.pitch_of(track, &BeatNote { string: note.string, fret, ..BeatNote::default() }),
                                    }?;
                                    (other != pitch).then_some((other, period.clamp(0.03, 1.0)))
                                }),
                            }
                        };
                        open.insert((voice_index, note.string), notes.len());
                        notes.push(Note {
                            start,
                            length: sounding.max(0.0625),
                            pitch,
                            velocity: velocity.round().clamp(1.0, 127.0) as u8,
                            string: (!drums).then_some(note.string.min(255) as u8),
                            articulation,
                        });
                    }
                }
            }
        }
        notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
        chords.sort_by(|a, b| a.beat.total_cmp(&b.beat));
        (notes, chords, tempo)
    }

    /// The pitch a note sounds at: the fret on its string, raised for a
    /// harmonic; for drums the fret is the percussion key itself.
    fn pitch_of(&self, track: &Track, note: &BeatNote) -> Option<u8> {
        let pitch = if track.is_drums() {
            i64::from(note.fret)
        } else {
            let open_string = i64::from(*track.tuning.get(note.string)?);
            let above_open = match note.harmonic {
                Some((true, touched)) => songsterr::harmonic_interval(i64::from(touched)),
                // An artificial or tapped harmonic sounds over the fret held as a
                // natural one over the open string, by how far above it the
                // string is touched; an octave when that is not one of them.
                Some((false, touched)) => {
                    let above = i64::from(touched - note.fret);
                    i64::from(note.fret) + if [5, 7, 12, 19, 24].contains(&above) { songsterr::harmonic_interval(above) } else { 12 }
                }
                None => i64::from(note.fret),
            };
            open_string + above_open + i64::from(track.capo)
        };
        (0..=127).contains(&pitch).then_some(pitch as u8)
    }
}

// ----- MusicXML, read into the same song -------------------------------------------

#[path = "musicxml.rs"]
mod musicxml;

// ----- the binary files of versions 3 to 5 -----------------------------------------

mod binary {
    use super::*;

    /// The versions of the binary format, as the file names itself.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Version {
        Gp3,
        Gp4,
        Gp500,
        Gp510,
    }

    impl Version {
        fn at_least_4(self) -> bool {
            self != Version::Gp3
        }

        fn is_5(self) -> bool {
            matches!(self, Version::Gp500 | Version::Gp510)
        }
    }

    /// Caps on the counts a file announces, so that a corrupt one is refused
    /// rather than followed.
    const MAX_MEASURES: i32 = 8192;
    const MAX_TRACKS: i32 = 64;
    const MAX_BEATS: i32 = 4096;
    const MAX_POINTS: i32 = 256;
    const MAX_NOTICES: i32 = 1024;
    const MAX_TEXT: i32 = 1 << 20;

    /// A cursor over the bytes of the file; every read checks it stays inside,
    /// and an error says what was being read, and where.
    struct Reader<'a> {
        bytes: &'a [u8],
        at: usize,
        /// What is being read, for the errors: "the header of bar 3".
        context: String,
    }

    impl<'a> Reader<'a> {
        fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
            let end = self.at.checked_add(n).filter(|end| *end <= self.bytes.len()).ok_or_else(|| format!("the file ends inside {} (byte {} of {})", self.context, self.at, self.bytes.len()))?;
            let slice = &self.bytes[self.at..end];
            self.at = end;
            Ok(slice)
        }

        fn skip(&mut self, n: usize) -> Result<(), String> {
            self.take(n).map(|_| ())
        }

        fn u8(&mut self) -> Result<u8, String> {
            Ok(self.take(1)?[0])
        }

        fn i8(&mut self) -> Result<i8, String> {
            Ok(self.u8()? as i8)
        }

        fn bool(&mut self) -> Result<bool, String> {
            Ok(self.u8()? != 0)
        }

        fn i16(&mut self) -> Result<i16, String> {
            let b = self.take(2)?;
            Ok(i16::from_le_bytes([b[0], b[1]]))
        }

        fn i32(&mut self) -> Result<i32, String> {
            let b = self.take(4)?;
            Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        }

        fn f64(&mut self) -> Result<f64, String> {
            let b = self.take(8)?;
            let mut bytes = [0u8; 8];
            bytes.copy_from_slice(b);
            Ok(f64::from_le_bytes(bytes))
        }

        /// A count the file announces, refused when negative or absurd.
        fn count(&mut self, max: i32, what: &str) -> Result<usize, String> {
            let n = self.i32()?;
            if n < 0 || n > max {
                return Err(format!("the file announces {n} {what} in {} (byte {} of {}), which cannot be right", self.context, self.at, self.bytes.len()));
            }
            Ok(n as usize)
        }

        /// A string of a length byte in a field of `size` bytes.
        fn byte_string(&mut self, size: usize) -> Result<String, String> {
            let len = usize::from(self.u8()?);
            let bytes = self.take(size)?;
            Ok(text(&bytes[..len.min(size)]))
        }

        /// A string of a length byte, after the size of the field as an int.
        fn int_byte_string(&mut self) -> Result<String, String> {
            let size = self.count(MAX_TEXT, "characters")?;
            if size == 0 {
                return Ok(String::new());
            }
            self.byte_string(size - 1)
        }

        /// A string of a length as an int.
        fn int_string(&mut self) -> Result<String, String> {
            let len = self.count(MAX_TEXT, "characters")?;
            Ok(text(self.take(len)?))
        }
    }

    /// The text of a string field: the files are written in a Windows code
    /// page, which is read as Latin-1.
    fn text(bytes: &[u8]) -> String {
        bytes.iter().map(|&b| char::from(b)).collect::<String>().trim_end_matches('\0').to_owned()
    }

    /// Reads a file of versions 3 to 5.
    pub(super) fn parse(bytes: &[u8]) -> Result<Song, String> {
        let mut r = Reader { bytes, at: 0, context: "the song header".to_owned() };
        let version = read_version(&mut r)?;

        // Song information: title, subtitle, artist, album, words, (from
        // version 5) music, copyright, tab author, instructions, then the
        // lines of the notice.
        let title = r.int_byte_string()?;
        r.int_byte_string()?;
        let artist = r.int_byte_string()?;
        for _ in 0..if version.is_5() { 6 } else { 5 } {
            r.int_byte_string()?;
        }
        let notices = r.count(MAX_NOTICES, "lines of notice")?;
        for _ in 0..notices {
            r.int_byte_string()?;
        }
        let mut song_feel = false;
        if !version.is_5() {
            song_feel = r.bool()?; // triplet feel, for the whole song
        }
        let mut lyrics = Vec::new();
        if version.at_least_4() {
            // Lyrics: the track they go with, then five lines of text, each with
            // the bar it starts at (both counted from one).
            let track = r.i32()?;
            for _ in 0..5 {
                let bar = r.i32()?;
                let text = r.int_string()?;
                if track > 0 && !text.trim().is_empty() {
                    lyrics.push(((track - 1) as usize, (bar.max(1) - 1) as usize, text));
                }
            }
        }
        if version == Version::Gp510 {
            // Master volume, a reserved int and the master equaliser.
            r.i32()?;
            r.i32()?;
            r.skip(11)?;
        }
        if version.is_5() {
            // Page setup: sizes, margins, score proportion, header flags and ten texts.
            for _ in 0..7 {
                r.i32()?;
            }
            r.i16()?;
            for _ in 0..10 {
                r.int_byte_string()?;
            }
            r.int_byte_string()?; // tempo name
        }
        let tempo = f64::from(r.i32()?.clamp(1, 1000));
        if version == Version::Gp510 {
            r.bool()?; // hide tempo
        }
        let key = if version.at_least_4() {
            let key = i64::from(r.i8()?);
            r.i32()?; // octave
            key
        } else {
            i64::from(r.i32()?)
        };
        let channels = read_channels(&mut r)?;
        let mut directions: Vec<i16> = Vec::new();
        if version.is_5() {
            // The bar of each sign of the score, from 1, or -1 for none.
            for _ in 0..SIGNS_OF_VERSION_5.len() {
                directions.push(r.i16()?);
            }
            r.i32()?; // master reverb
        }
        let measures = r.count(MAX_MEASURES, "measures")?;
        let tracks = r.count(MAX_TRACKS, "tracks")?;
        let (bars, minor) = read_measure_headers(&mut r, version, measures)?;
        let mut song = Song { title, artist, tempo, key: None, bars, tracks: Vec::with_capacity(tracks), lyrics };
        if let (true, Some(first)) = (song_feel, song.bars.first_mut()) {
            first.feel = Some(0.5);
        }
        for (name, bar) in SIGNS_OF_VERSION_5.iter().zip(&directions) {
            if let Some(bar) = usize::try_from(*bar).ok().and_then(|bar| bar.checked_sub(1)).and_then(|bar| song.bars.get_mut(bar)) {
                bar.signs.set(name);
            }
        }
        if (-7..=7).contains(&key) {
            song.key = Some(songsterr::key_of(key, minor));
        }
        for index in 0..tracks {
            r.context = format!("track {}", index + 1);
            let track = read_track(&mut r, version, index, &channels)?;
            song.tracks.push(track);
        }
        if version.is_5() {
            r.skip(if version == Version::Gp500 { 2 } else { 1 })?;
        }
        read_measures(&mut r, version, &mut song)?;
        Ok(song)
    }

    /// The version string: a length byte and 30 bytes of text.
    fn read_version(r: &mut Reader) -> Result<Version, String> {
        let Ok(text) = r.byte_string(30) else {
            return Err(format!("not a Guitar Pro file: it is only {} bytes long", r.bytes.len()));
        };
        // Versions 1 and 2 wrote "FICHIER GUITARE PRO"; later ones "FICHIER GUITAR PRO".
        let Some(rest) = text.strip_prefix("FICHIER GUITAR PRO ").or_else(|| text.strip_prefix("FICHIER GUITARE PRO ")) else {
            let start: String = text.chars().take(24).filter(|c| !c.is_control()).collect();
            return Err(format!("not a Guitar Pro file: it starts with \"{start}\" instead of \"FICHIER GUITAR PRO\""));
        };
        // "v4.06" or "L4.06": the letter does not matter.
        let number: String = rest.chars().skip(1).collect();
        match number.trim() {
            "3.00" => Ok(Version::Gp3),
            "4.00" | "4.06" => Ok(Version::Gp4),
            "5.00" => Ok(Version::Gp500),
            "5.10" => Ok(Version::Gp510),
            _ => Err(format!("Guitar Pro version \"{}\" is not read yet; export the song as GP5 or GP7", text.trim_end())),
        }
    }

    /// The 64 MIDI channels: the program of each, with its mixer settings.
    fn read_channels(r: &mut Reader) -> Result<Vec<i32>, String> {
        let mut channels = Vec::with_capacity(64);
        for _ in 0..64 {
            channels.push(r.i32()?);
            r.skip(8)?; // volume, balance, chorus, reverb, phaser, tremolo and two blanks
        }
        Ok(channels)
    }

    /// The headers of the measures: signature, repeats, markers and keys. Also
    /// tells whether the first key signature written is minor.
    fn read_measure_headers(r: &mut Reader, version: Version, count: usize) -> Result<(Vec<BarInfo>, bool), String> {
        let mut bars = Vec::with_capacity(count);
        let mut signature = (4u32, 4u32);
        let mut minor = None;
        // Passes already given an alternate ending since the repeat started;
        // versions 3 and 4 write the number of the ending, 5 writes a mask.
        let mut claimed = 0u32;
        for index in 0..count {
            r.context = format!("the header of bar {}", index + 1);
            if version.is_5() && index > 0 {
                r.u8()?;
            }
            let flags = r.u8()?;
            let mut bar = BarInfo::default();
            if flags & 0x01 != 0 {
                signature.0 = u32::from(r.u8()?).clamp(1, 32);
            }
            if flags & 0x02 != 0 {
                let den = u32::from(r.u8()?);
                if [1, 2, 4, 8, 16, 32].contains(&den) {
                    signature.1 = den;
                }
            }
            bar.signature = signature;
            bar.repeat_start = flags & 0x04 != 0;
            if bar.repeat_start {
                claimed = 0;
            }
            if flags & 0x08 != 0 {
                // Versions 3 and 4 count the repetitions, version 5 the passes.
                let byte = u32::from(r.u8()?);
                bar.repeat = Some(if version.is_5() { byte } else { byte + 1 }.clamp(2, 64));
            }
            let mut endings = 0u32;
            if version.is_5() {
                if flags & 0x20 != 0 {
                    bar.marker = Some(read_marker(r)?);
                }
                if flags & 0x10 != 0 {
                    endings = u32::from(r.u8()?);
                }
            } else {
                if flags & 0x10 != 0 {
                    let number = u32::from(r.u8()?).min(8);
                    endings = ((1u32 << number) - 1) & !claimed;
                    claimed |= endings;
                }
                if flags & 0x20 != 0 {
                    bar.marker = Some(read_marker(r)?);
                }
            }
            bar.endings = (0..8).filter(|bit| endings & (1 << bit) != 0).map(|bit| bit + 1).collect();
            if flags & 0x40 != 0 {
                r.i8()?; // the key, which the song header already gave
                let kind = r.u8()?;
                minor.get_or_insert(kind == 1);
            }
            if version.is_5() {
                if flags & 0x03 != 0 {
                    r.skip(4)?; // how the eighth notes are beamed
                }
                if flags & 0x10 == 0 {
                    r.u8()?;
                }
                // The feel of the bar: none, eighth notes or sixteenths in triplets.
                bar.feel = Some(match r.u8()? {
                    1 => 0.5,
                    2 => 0.25,
                    _ => 0.0,
                });
            }
            bars.push(bar);
        }
        Ok((bars, minor.unwrap_or(false)))
    }

    /// A marker: its text and its colour.
    fn read_marker(r: &mut Reader) -> Result<String, String> {
        let name = r.int_byte_string()?;
        r.skip(4)?;
        Ok(name)
    }

    /// A track: name, strings, MIDI channel and capo; the settings of Guitar
    /// Pro's own synthesiser are passed over.
    fn read_track(r: &mut Reader, version: Version, index: usize, channels: &[i32]) -> Result<Track, String> {
        if version == Version::Gp500 || (version == Version::Gp510 && index == 0) {
            r.u8()?;
        }
        let flags = r.u8()?;
        let name = r.byte_string(40)?;
        let strings = r.count(7, "strings")?;
        let mut tuning = Vec::with_capacity(strings);
        for i in 0..7 {
            let pitch = r.i32()?;
            if i < strings {
                tuning.push(pitch);
            }
        }
        let port = r.i32()?;
        let channel = r.i32()?;
        r.i32()?; // effects channel
        r.i32()?; // fret count
        let capo = r.i32()?;
        r.skip(4)?; // colour
        if version.is_5() {
            r.skip(if version == Version::Gp500 { 44 } else { 49 })?;
            if version == Version::Gp510 {
                r.int_byte_string()?; // the name of an effect
                r.int_byte_string()?; // and its category
            }
        }
        let drums = flags & 0x01 != 0 || channel == 10;
        let program = if drums {
            DRUMS
        } else {
            // In i64: the two numbers come straight from the file.
            let index = (i64::from(port) - 1) * 16 + (i64::from(channel) - 1);
            usize::try_from(index).ok().and_then(|i| channels.get(i)).map_or(0, |p| (*p).clamp(0, 127)) as u32
        };
        Ok(Track { name, program, tuning, capo: capo.clamp(-24, 24), bars: Vec::new() })
    }

    /// The beats of every track in every measure, in the order of the file.
    fn read_measures(r: &mut Reader, version: Version, song: &mut Song) -> Result<(), String> {
        let voices = if version.is_5() { 2 } else { 1 };
        for bar in 0..song.bars.len() {
            for (index, track) in song.tracks.iter_mut().enumerate() {
                r.context = format!("the beats of bar {} of track {}", bar + 1, index + 1);
                let strings = track.tuning.len();
                let mut bar = Vec::with_capacity(voices);
                for _ in 0..voices {
                    let count = r.count(MAX_BEATS, "beats")?;
                    let mut beats = Vec::with_capacity(count);
                    for _ in 0..count {
                        beats.push(read_beat(r, version, strings)?);
                    }
                    bar.push(beats);
                }
                // A line break after the bar; the last bar of the file has none.
                if version.is_5() && r.at < r.bytes.len() {
                    r.u8()?;
                }
                track.bars.push(bar);
            }
        }
        Ok(())
    }

    fn read_beat(r: &mut Reader, version: Version, strings: usize) -> Result<Beat, String> {
        let flags = r.u8()?;
        let mut beat = Beat::default();
        if flags & 0x40 != 0 {
            r.u8()?; // empty or rest: no note sounds either way
            beat.rest = true;
        }
        // -2 is a whole note, 0 a quarter, 4 a sixty-fourth.
        let value = i32::from(r.i8()?).clamp(-2, 4);
        let dots = u32::from(flags & 0x01 != 0);
        let tuplet = if flags & 0x20 != 0 { tuplet_of(r.i32()?.clamp(0, 16) as u32) } else { None };
        beat.length = beats_of(2f64.powi(value + 2), dots, tuplet);
        if flags & 0x02 != 0 {
            beat.chord = Some(read_chord(r, version)?);
        }
        if flags & 0x04 != 0 {
            r.int_byte_string()?; // text
        }
        if flags & 0x08 != 0 {
            read_beat_effects(r, version, &mut beat)?;
        }
        if flags & 0x10 != 0 {
            beat.tempo = read_mix_table(r, version)?;
        }
        // Bit 6 is the highest string, bit 0 the seventh.
        let played = r.u8()?;
        for string in 0..strings.min(7) {
            if played & (1 << (6 - string)) != 0 {
                beat.notes.push(read_note(r, version, string)?);
            }
        }
        if let Some(natural) = beat.harmonic {
            for note in beat.notes.iter_mut().filter(|n| n.harmonic.is_none()) {
                note.harmonic = Some(if natural { (true, note.fret) } else { (false, note.fret + 12) });
            }
        }
        if version.is_5() {
            r.u8()?;
            let flags2 = r.u8()?;
            if flags2 & 0x08 != 0 {
                r.u8()?;
            }
        }
        Ok(beat)
    }

    /// A chord diagram; only its name is kept.
    fn read_chord(r: &mut Reader, version: Version) -> Result<String, String> {
        let new_format = r.bool()?;
        if !new_format {
            let name = r.int_byte_string()?;
            let base_fret = r.i32()?;
            if base_fret > 0 {
                r.skip(24)?; // six frets
            }
            return Ok(name);
        }
        if version == Version::Gp3 {
            // Version 3 writes the root, type, extension, bass and tonality
            // as ints, the name in 22 bytes, then the fifth, ninth and
            // eleventh as ints, six frets, two barres (as ints) and omissions.
            r.skip(25)?;
            let name = r.byte_string(22)?;
            r.skip(12)?;
            r.i32()?; // base fret
            r.skip(24)?;
            r.skip(36)?;
            return Ok(name);
        }
        // Sharp, three blanks, root, type, extension, bass, tonality, add.
        r.skip(16)?;
        let name = r.byte_string(21)?;
        r.skip(4)?; // fifth, ninth, eleventh and a blank
        r.i32()?; // base fret
        r.skip(28)?; // seven frets
        r.skip(32)?; // barres, omissions, fingering
        Ok(name)
    }

    /// Effects over a whole beat: vibrato and the tremolo bar are kept.
    fn read_beat_effects(r: &mut Reader, version: Version, beat: &mut Beat) -> Result<(), String> {
        let flags1 = r.u8()?;
        let flags2 = if version.at_least_4() { r.u8()? } else { 0 };
        if flags1 & 0x01 != 0 {
            beat.vibrato = Some(Vibrato::Slight);
        }
        if flags1 & 0x02 != 0 {
            beat.vibrato = Some(Vibrato::Wide);
        }
        // Version 3 writes harmonics over the whole beat; later ones on each note.
        if version == Version::Gp3 && flags1 & 0x04 != 0 {
            beat.harmonic = Some(true);
        }
        if version == Version::Gp3 && flags1 & 0x08 != 0 {
            beat.harmonic = Some(false);
        }
        if flags1 & 0x20 != 0 {
            // Tapping, slapping, popping; in version 3 also the tremolo bar, a dip.
            let effect = r.u8()?;
            if version == Version::Gp3 {
                let value = f64::from(r.i32()?);
                if effect == 0 {
                    beat.whammy = bend_points(&[(0.0, 0.0), (30.0, -value), (60.0, 0.0)], 60.0);
                }
            }
        }
        if version.at_least_4() && flags2 & 0x04 != 0 {
            beat.whammy = read_bend(r)?;
        }
        if flags1 & 0x40 != 0 {
            r.skip(2)?; // down and up strokes
        }
        if version.at_least_4() && flags2 & 0x02 != 0 {
            r.u8()?; // pick stroke
        }
        Ok(())
    }

    /// A bend or tremolo bar: its points, over 0 to 60 along the note.
    fn read_bend(r: &mut Reader) -> Result<Vec<(f32, f32)>, String> {
        r.u8()?; // type
        r.i32()?; // the height, which the points repeat
        let count = r.count(MAX_POINTS, "bend points")?;
        let mut points = Vec::with_capacity(count);
        for _ in 0..count {
            let position = f64::from(r.i32()?);
            let value = f64::from(r.i32()?);
            r.u8()?; // vibrato
            points.push((position, value));
        }
        Ok(bend_points(&points, 60.0))
    }

    /// A change of the mixer settings; only a tempo change is kept.
    fn read_mix_table(r: &mut Reader, version: Version) -> Result<Option<f64>, String> {
        r.i8()?; // instrument
        if version.is_5() {
            r.skip(16)?; // the sound of Guitar Pro's synthesiser
        }
        // Volume, balance, chorus, reverb, phaser, tremolo: -1 for no change.
        let mut values = [0i8; 6];
        for value in &mut values {
            *value = r.i8()?;
        }
        if version.is_5() {
            r.int_byte_string()?; // tempo name
        }
        let tempo = r.i32()?;
        for value in values {
            if value >= 0 {
                r.u8()?; // over how many beats the value changes
            }
        }
        if tempo >= 0 {
            r.u8()?;
            if version == Version::Gp510 {
                r.u8()?; // hide tempo
            }
        }
        if version.at_least_4() {
            r.u8()?; // which changes apply to all tracks
        }
        if version.is_5() {
            r.i8()?; // wah
        }
        if version == Version::Gp510 {
            r.int_byte_string()?; // the name of an effect
            r.int_byte_string()?; // and its category
        }
        Ok((tempo > 0).then(|| f64::from(tempo.min(1000))))
    }

    fn read_note(r: &mut Reader, version: Version, string: usize) -> Result<BeatNote, String> {
        let flags = r.u8()?;
        let mut note = BeatNote { string, ..BeatNote::default() };
        note.ghost = flags & 0x04 != 0;
        note.accent = if flags & 0x02 != 0 { 2 } else { u8::from(flags & 0x40 != 0) };
        if flags & 0x20 != 0 {
            let kind = r.u8()?; // 1 normal, 2 tied, 3 dead
            note.tie = kind == 2;
            note.dead = kind == 3;
        }
        if flags & 0x01 != 0 && !version.is_5() {
            r.skip(2)?; // its own duration and tuplet
        }
        if flags & 0x10 != 0 {
            note.velocity = Some(velocity_of_dynamic(i64::from(r.i8()?)));
        }
        if flags & 0x20 != 0 {
            note.fret = i32::from(r.i8()?);
        }
        if flags & 0x80 != 0 {
            r.skip(2)?; // fingering of both hands
        }
        if version.is_5() {
            if flags & 0x01 != 0 {
                r.f64()?; // how much of its length the note sounds
            }
            r.u8()?;
        }
        if flags & 0x08 != 0 {
            read_note_effects(r, version, &mut note)?;
        }
        Ok(note)
    }

    fn read_note_effects(r: &mut Reader, version: Version, note: &mut BeatNote) -> Result<(), String> {
        let flags1 = r.u8()?;
        let flags2 = if version.at_least_4() { r.u8()? } else { 0 };
        note.hammer = flags1 & 0x02 != 0;
        note.let_ring = flags1 & 0x08 != 0;
        if version == Version::Gp3 && flags1 & 0x04 != 0 {
            note.slide = Some(Slide::Shift);
        }
        if flags1 & 0x01 != 0 {
            note.bend = read_bend(r)?;
        }
        if flags1 & 0x10 != 0 {
            // A grace note: fret, loudness, transition, duration (a thirty-second,
            // a twenty-fourth or a sixteenth); version 5 adds flags, 0x02 for on the beat.
            let fret = i32::from(r.i8()?);
            let dynamic = i64::from(r.u8()?);
            r.u8()?; // transition
            let length = match r.u8()? {
                2 => 1.0 / 6.0,
                3 => 0.25,
                _ => 0.125,
            };
            let on_beat = version.is_5() && r.u8()? & 0x02 != 0;
            note.grace = Some(Grace { fret, velocity: velocity_of_dynamic(dynamic), length, on_beat });
        }
        if !version.at_least_4() {
            return Ok(());
        }
        note.staccato = flags2 & 0x01 != 0;
        note.palm_mute = flags2 & 0x02 != 0;
        if flags2 & 0x04 != 0 {
            // Tremolo picking in eighth notes, sixteenths or thirty-seconds.
            note.tremolo = match r.u8()? {
                1 => Some(0.5),
                2 => Some(0.25),
                3 => Some(0.125),
                _ => None,
            };
        }
        if flags2 & 0x08 != 0 {
            let kind = r.i8()?;
            note.slide = if version.is_5() { slide_of_flags(kind as u8) } else { slide_of_number(kind) };
        }
        if flags2 & 0x10 != 0 {
            // 1 natural, 2 artificial, 3 tapped, 4 pinch, 5 semi; version 4
            // writes the artificial ones as 15, 17 and 22.
            let kind = r.u8()?;
            let mut touched = note.fret
                + match kind {
                    15 => 5,
                    17 => 7,
                    _ => 12,
                };
            if version.is_5() {
                match kind {
                    2 => r.skip(3)?, // the note, its accidental and octave
                    3 => {
                        let fret = i32::from(r.u8()?);
                        if fret > 0 {
                            touched = fret;
                        }
                    }
                    _ => {}
                }
            }
            if kind != 0 {
                note.harmonic = Some(if kind == 1 { (true, note.fret) } else { (false, touched) });
            }
        }
        if flags2 & 0x20 != 0 {
            // A trill with another fret of the string, in sixteenths, thirty-seconds or sixty-fourths.
            let fret = i32::from(r.i8()?);
            let period = match r.u8()? {
                2 => 0.125,
                3 => 0.0625,
                _ => 0.25,
            };
            note.trill = Some((TrillTo::Fret(fret), period));
        }
        if flags2 & 0x40 != 0 {
            note.vibrato = Some(Vibrato::Slight);
        }
        Ok(())
    }

    /// A slide as version 4 numbers them.
    fn slide_of_number(kind: i8) -> Option<Slide> {
        match kind {
            1 => Some(Slide::Shift),
            2 => Some(Slide::Legato),
            3 => Some(Slide::OutDown),
            4 => Some(Slide::OutUp),
            -1 => Some(Slide::FromBelow),
            -2 => Some(Slide::FromAbove),
            _ => None,
        }
    }

    /// A slide as version 5 (and the XML score) flag them; the first flag set wins.
    pub(super) fn slide_of_flags(flags: u8) -> Option<Slide> {
        [(0x01, Slide::Shift), (0x02, Slide::Legato), (0x04, Slide::OutDown), (0x08, Slide::OutUp), (0x10, Slide::FromBelow), (0x20, Slide::FromAbove)]
            .into_iter()
            .find(|(bit, _)| flags & bit != 0)
            .map(|(_, slide)| slide)
    }
}

// ----- the zipped XML score of versions 7 and 8 -------------------------------------

/// What is read: the score texts, the tempo automations, the master bars
/// (signature, key, repeats, alternate endings, sections), the tracks
/// (sound, strings, capo, drum kit, chord diagrams) and the beats with
/// their rhythms, dynamics, grace notes, tremolo bar and vibrato, and the
/// notes with their string and fret, ties, hammer-ons, slides, bends,
/// harmonics, mutes, let ring, vibrato, accents and ghost marks.
/// Passed over: free text, brush and pick strokes, tapping, trills,
/// tremolo picking, ottava marks, the display transposition of a track,
/// linear tempo ramps (the change is taken at its start), and the tempo
/// position inside a bar is read as a share of the bar.
mod gpif {
    use super::*;

    /// The score inside the archive.
    const SCORE_PATH: &str = "Content/score.gpif";
    /// Longest score unpacked, in bytes.
    const MAX_SCORE: u64 = 1 << 26;
    const MAX_DEPTH: usize = 64;
    const MAX_ELEMENTS: usize = 4_000_000;
    const MAX_BARS: usize = 8192;
    const MAX_BEATS: usize = 4096;

    /// An XML element: name, attributes, children and the text between them.
    #[derive(Debug, Default)]
    pub(super) struct Element {
        pub(super) name: String,
        pub(super) attrs: Vec<(String, String)>,
        pub(super) children: Vec<Element>,
        pub(super) text: String,
    }

    impl Element {
        pub(super) fn child(&self, name: &str) -> Option<&Element> {
            self.children.iter().find(|c| c.name == name)
        }

        pub(super) fn children<'a>(&'a self, name: &str) -> impl Iterator<Item = &'a Element> + 'a {
            let name = name.to_owned();
            self.children.iter().filter(move |c| c.name == name)
        }

        /// The text of a child, trimmed; empty when there is none.
        pub(super) fn text_of(&self, name: &str) -> &str {
            self.child(name).map_or("", |c| c.text.trim())
        }

        pub(super) fn attr(&self, name: &str) -> Option<&str> {
            self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
        }

        /// A `<Property name="…">` of the element's `<Properties>`.
        fn property(&self, name: &str) -> Option<&Element> {
            self.child("Properties")?.children("Property").find(|p| p.attr("name") == Some(name))
        }

        /// The number in `<field>` of a property.
        fn property_number(&self, property: &str, field: &str) -> Option<f64> {
            number(self.property(property)?.text_of(field))
        }
    }

    pub(super) fn number(text: &str) -> Option<f64> {
        text.trim().parse::<f64>().ok().filter(|n| n.is_finite())
    }

    /// The whole numbers of a list separated by spaces.
    fn ints(text: &str) -> Vec<i64> {
        text.split_whitespace().filter_map(|t| t.parse::<f64>().ok()).filter(|n| n.is_finite()).map(|n| n as i64).collect()
    }

    /// Undoes the five named entities and the numbered ones.
    fn decode(text: &str) -> String {
        if !text.contains('&') {
            return text.to_owned();
        }
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(at) = rest.find('&') {
            out.push_str(&rest[..at]);
            rest = &rest[at..];
            let Some(end) = rest.find(';').filter(|end| *end <= 10) else {
                out.push('&');
                rest = &rest[1..];
                continue;
            };
            let entity = &rest[1..end];
            let decoded = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => entity
                    .strip_prefix('#')
                    .and_then(|n| n.strip_prefix('x').map_or_else(|| n.parse::<u32>().ok(), |hex| u32::from_str_radix(hex, 16).ok()))
                    .and_then(char::from_u32),
            };
            match decoded {
                Some(c) => out.push(c),
                None => out.push_str(&rest[..=end]),
            }
            rest = &rest[end + 1..];
        }
        out.push_str(rest);
        out
    }

    /// Reads the attributes of a tag: `name="value"` pairs, loosely.
    fn attributes(text: &str) -> Vec<(String, String)> {
        let mut attrs = Vec::new();
        let mut rest = text.trim_start();
        while !rest.is_empty() {
            let name_end = rest.find(|c: char| c == '=' || c.is_whitespace()).unwrap_or(rest.len());
            let name = &rest[..name_end];
            rest = rest[name_end..].trim_start();
            let Some(after) = rest.strip_prefix('=') else {
                if !name.is_empty() {
                    attrs.push((name.to_owned(), String::new()));
                }
                if name_end == 0 {
                    rest = &rest[rest.chars().next().map_or(0, char::len_utf8)..];
                }
                continue;
            };
            rest = after.trim_start();
            let value_end;
            let value = match rest.chars().next() {
                Some(quote @ ('"' | '\'')) => {
                    let inner = &rest[1..];
                    let end = inner.find(quote).unwrap_or(inner.len());
                    value_end = (1 + end + 1).min(rest.len());
                    &inner[..end]
                }
                _ => {
                    value_end = rest.find(char::is_whitespace).unwrap_or(rest.len());
                    &rest[..value_end]
                }
            };
            attrs.push((name.to_owned(), decode(value)));
            rest = rest[value_end..].trim_start();
        }
        attrs
    }

    /// Where a tag opened at `start` ends: the first `>` outside quotes.
    fn tag_end(bytes: &[u8], start: usize) -> Option<usize> {
        let mut quote = None;
        for (i, &b) in bytes.iter().enumerate().skip(start) {
            match (quote, b) {
                (None, b'"' | b'\'') => quote = Some(b),
                (None, b'>') => return Some(i),
                (Some(q), _) if q == b => quote = None,
                _ => {}
            }
        }
        None
    }

    fn find(text: &str, from: usize, pattern: &str) -> Option<usize> {
        text.get(from..)?.find(pattern).map(|at| from + at)
    }

    /// Reads an XML document into a tree, forgiving what it can: an unclosed
    /// element ends with its parent, a stray closing tag closes the current one.
    pub(super) fn parse_xml(text: &str) -> Result<Element, String> {
        let bytes = text.as_bytes();
        let mut stack = vec![Element { name: "document".to_owned(), ..Element::default() }];
        let (mut i, mut count) = (0usize, 0usize);
        while i < bytes.len() {
            if bytes[i] != b'<' {
                let end = find(text, i, "<").unwrap_or(text.len());
                let chunk = &text[i..end];
                if !chunk.trim().is_empty() {
                    if let Some(top) = stack.last_mut() {
                        top.text.push_str(&decode(chunk));
                    }
                }
                i = end;
                continue;
            }
            let rest = &text[i..];
            if rest.starts_with("<!--") {
                i = find(text, i + 4, "-->").map_or(text.len(), |at| at + 3);
                continue;
            }
            if rest.starts_with("<![CDATA[") {
                let end = find(text, i + 9, "]]>").unwrap_or(text.len());
                if let Some(top) = stack.last_mut() {
                    top.text.push_str(&text[(i + 9).min(end)..end]);
                }
                i = (end + 3).min(text.len());
                continue;
            }
            if rest.starts_with("<?") {
                i = find(text, i + 2, "?>").map_or(text.len(), |at| at + 2);
                continue;
            }
            if rest.starts_with("<!") {
                i = find(text, i + 2, ">").map_or(text.len(), |at| at + 1);
                continue;
            }
            let Some(end) = tag_end(bytes, i + 1) else { break };
            let tag = text[i + 1..end].trim();
            i = end + 1;
            if tag.starts_with('/') {
                if stack.len() > 1 {
                    let done = stack.pop().ok_or("the score is not well formed")?;
                    stack.last_mut().ok_or("the score is not well formed")?.children.push(done);
                }
                continue;
            }
            let self_closing = tag.ends_with('/');
            let tag = tag.trim_end_matches('/').trim_end();
            let name_end = tag.find(char::is_whitespace).unwrap_or(tag.len());
            let element = Element { name: tag[..name_end].to_owned(), attrs: attributes(&tag[name_end..]), children: Vec::new(), text: String::new() };
            count += 1;
            if count > MAX_ELEMENTS {
                return Err("the score is too large to read".to_owned());
            }
            if self_closing {
                stack.last_mut().ok_or("the score is not well formed")?.children.push(element);
            } else {
                if stack.len() > MAX_DEPTH {
                    return Err("the score is nested too deeply to be one".to_owned());
                }
                stack.push(element);
            }
        }
        while stack.len() > 1 {
            let done = stack.pop().ok_or("the score is not well formed")?;
            stack.last_mut().ok_or("the score is not well formed")?.children.push(done);
        }
        stack.pop().ok_or_else(|| "the score is not well formed".to_owned())
    }

    /// Reads a `.gp` file: a ZIP archive holding the score.
    pub(super) fn parse_archive(bytes: &[u8]) -> Result<Song, String> {
        use std::io::Read;
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("unreadable Guitar Pro archive ({e})"))?;
        let file = archive.by_name(SCORE_PATH).map_err(|_| format!("the archive holds no score ({SCORE_PATH}): not a Guitar Pro 7 file"))?;
        if file.size() > MAX_SCORE {
            return Err("the score is too large to read".to_owned());
        }
        let mut xml = Vec::with_capacity(file.size() as usize);
        file.take(MAX_SCORE).read_to_end(&mut xml).map_err(|e| format!("unreadable score ({e})"))?;
        parse_gpif(&String::from_utf8_lossy(&xml))
    }

    /// What is kept of a track before its beats are read.
    #[derive(Default)]
    struct TrackInfo {
        name: String,
        program: u32,
        /// Open strings, highest first.
        tuning: Vec<i32>,
        capo: i32,
        drums: bool,
        /// Percussion keys of the drum kit's articulations, in the order of the file.
        articulations: Vec<i32>,
        /// The same keys by element, then variation.
        elements: Vec<Vec<i32>>,
        /// Chord names by the number of their diagram.
        chords: HashMap<i64, String>,
        /// The lines of words sung on the track: the bar each starts at, its text.
        lyrics: Vec<(usize, String)>,
    }

    fn read_track(track: &Element) -> TrackInfo {
        let mut info = TrackInfo { name: track.text_of("Name").to_owned(), ..TrackInfo::default() };
        if let Some(lines) = track.child("Lyrics") {
            for line in lines.children("Line") {
                let text = line.text_of("Text");
                if !text.trim().is_empty() {
                    info.lyrics.push((number(line.text_of("Offset")).map_or(0, |b| b.max(0.0) as usize), text.to_owned()));
                }
            }
        }
        let channel = track.child("MidiConnection").or_else(|| track.child("GeneralMidi")).and_then(|c| number(c.text_of("PrimaryChannel")));
        let program = track
            .child("GeneralMidi")
            .and_then(|gm| number(gm.text_of("Program")))
            .or_else(|| number(track.child("Sounds")?.child("Sound")?.child("MIDI")?.text_of("Program")));
        let kit = track.child("InstrumentSet").is_some_and(|set| set.text_of("Type") == "drumKit");
        info.drums = kit || channel == Some(9.0) || track.child("Instrument").and_then(|i| i.attr("ref")).is_some_and(|r| r.starts_with("drm"));
        info.program = if info.drums { DRUMS } else { program.map_or(0, |p| p.clamp(0.0, 127.0) as u32) };
        // The strings and the capo are properties of the staff (or, in older
        // scores, of the track).
        let staff = track.child("Staves").and_then(|s| s.child("Staff"));
        let property = |name: &str| staff.and_then(|s| s.property(name)).or_else(|| track.property(name));
        if let Some(tuning) = property("Tuning") {
            // The file lists the strings lowest first.
            info.tuning = ints(tuning.text_of("Pitches")).iter().rev().map(|p| (*p).clamp(0, 127) as i32).collect();
        }
        info.capo = property("CapoFret").and_then(|p| number(p.text_of("Fret"))).map_or(0, |f| f.clamp(-24.0, 24.0) as i32);
        if let Some(elements) = track.child("InstrumentSet").and_then(|set| set.child("Elements")) {
            for element in elements.children("Element") {
                let keys: Vec<i32> = element
                    .child("Articulations")
                    .map(|a| a.children("Articulation").map(|art| number(art.text_of("OutputMidiNumber")).map_or(35, |n| n.clamp(0.0, 127.0) as i32)).collect())
                    .unwrap_or_default();
                info.articulations.extend(&keys);
                info.elements.push(keys);
            }
        }
        if let Some(items) = property("DiagramCollection").and_then(|p| p.child("Items")) {
            for item in items.children("Item") {
                if let (Some(id), Some(name)) = (item.attr("id").and_then(|id| id.parse::<i64>().ok()), item.attr("name")) {
                    info.chords.insert(id, name.to_owned());
                }
            }
        }
        info
    }

    /// The length in beats of a rhythm element.
    fn length_of(rhythm: Option<&Element>) -> f64 {
        let Some(rhythm) = rhythm else { return 1.0 };
        let value = match rhythm.text_of("NoteValue") {
            "Whole" => 1.0,
            "Half" => 2.0,
            "Eighth" => 8.0,
            "16th" => 16.0,
            "32nd" => 32.0,
            "64th" => 64.0,
            "128th" => 128.0,
            _ => 4.0,
        };
        let dots = rhythm.child("AugmentationDot").and_then(|d| d.attr("count")).and_then(|c| c.parse::<u32>().ok()).unwrap_or(0);
        let tuplet = rhythm.child("PrimaryTuplet").and_then(|t| Some((t.attr("num")?.parse::<u32>().ok()?, t.attr("den")?.parse::<u32>().ok()?)));
        beats_of(value, dots, tuplet)
    }

    /// The points of a bend or tremolo bar written as `<prefix>OriginValue`,
    /// `<prefix>MiddleValue`… with offsets over 0 to 100 along the note.
    fn curve(element: &Element, prefix: &str) -> Vec<(f32, f32)> {
        let get = |field: &str| element.property_number(&format!("{prefix}{field}"), "Float");
        let origin = get("OriginValue").unwrap_or(0.0);
        let mut points = vec![(get("OriginOffset").unwrap_or(0.0), origin)];
        if let Some(middle) = get("MiddleValue") {
            let (first, second) = (get("MiddleOffset1").unwrap_or(50.0), get("MiddleOffset2").unwrap_or(50.0));
            points.push((first, middle));
            if second > first {
                points.push((second, middle));
            }
        }
        points.push((get("DestinationOffset").unwrap_or(100.0), get("DestinationValue").unwrap_or(origin)));
        bend_points(&points, 100.0)
    }

    /// The percussion key of a drum note: written outright in older scores,
    /// or the articulation of the kit it points at.
    fn drum_key(note: &Element, track: &TrackInfo) -> i32 {
        if let Some(key) = note.property_number("Midi", "Number") {
            return key.clamp(0.0, 127.0) as i32;
        }
        if let Some(key) = number(note.text_of("InstrumentArticulation")).and_then(|i| track.articulations.get(i as usize)) {
            return *key;
        }
        if let Some(element) = note.property_number("Element", "Element") {
            let variation = note.property_number("Variation", "Variation").unwrap_or(0.0);
            if let Some(key) = track.elements.get(element as usize).and_then(|keys| keys.get(variation as usize)) {
                return *key;
            }
        }
        35
    }

    fn read_note(note: &Element, track: &TrackInfo) -> Option<BeatNote> {
        let mut out = BeatNote::default();
        if track.drums {
            out.fret = drum_key(note, track);
        } else {
            // The file counts the strings from the lowest.
            let string = note.property_number("String", "String")? as usize;
            out.string = track.tuning.len().checked_sub(string + 1)?;
            out.fret = note.property_number("Fret", "Fret").unwrap_or(0.0).clamp(-127.0, 127.0) as i32;
        }
        out.tie = note.child("Tie").and_then(|t| t.attr("destination")) == Some("true");
        out.hammer = note.property("HopoOrigin").is_some();
        out.slide = note.property_number("Slide", "Flags").and_then(|f| binary::slide_of_flags(f as u8));
        if note.property("Bended").is_some() {
            out.bend = curve(note, "Bend");
        }
        if let Some(kind) = note.property("Harmonic").map(|h| h.text_of("HType")) {
            let touched = note.property_number("HarmonicFret", "HFret").map(|f| f as i32);
            // The other kinds sound an octave over the fret held.
            out.harmonic = Some(if kind == "Natural" { (true, touched.unwrap_or(out.fret)) } else { (false, out.fret + 12) });
        }
        out.dead = note.property("Muted").is_some();
        out.palm_mute = note.property("PalmMuted").is_some();
        // A trill with the note of this pitch, in sixteenths.
        out.trill = number(note.text_of("Trill")).filter(|p| (0.0..=127.0).contains(p)).map(|p| (TrillTo::Pitch(p as u8), 0.25));
        out.let_ring = note.property("LetRing").is_some() || note.child("LetRing").is_some();
        out.vibrato = match note.text_of("Vibrato") {
            "Slight" => Some(Vibrato::Slight),
            "Wide" => Some(Vibrato::Wide),
            _ => None,
        };
        // Accent flags: 1 staccato, 4 an accent, 8 a heavy one.
        let accent = number(note.text_of("Accent")).unwrap_or(0.0) as i64;
        out.staccato = accent & 1 != 0;
        out.accent = if accent & 8 != 0 { 2 } else { u8::from(accent & 4 != 0) };
        out.ghost = note.child("AntiAccent").is_some();
        Some(out)
    }

    fn read_beat(beat: &Element, track: &TrackInfo, notes: &HashMap<i64, &Element>, rhythms: &HashMap<i64, &Element>) -> Beat {
        let rhythm = beat.child("Rhythm").and_then(|r| r.attr("ref")).and_then(|id| id.parse::<i64>().ok()).and_then(|id| rhythms.get(&id).copied());
        let grace = beat.child("GraceNotes").map(|g| g.text.trim());
        let mut out = Beat { length: length_of(rhythm), grace: grace.is_some(), grace_on_beat: grace == Some("OnBeat"), ..Beat::default() };
        // Tremolo picking in eighth notes, sixteenths or thirty-seconds.
        out.tremolo = match beat.text_of("Tremolo") {
            "1/2" => Some(0.5),
            "1/4" => Some(0.25),
            "1/8" => Some(0.125),
            _ => None,
        };
        out.velocity = songsterr::velocity_of(&beat.text_of("Dynamic").to_ascii_lowercase());
        out.chord = number(beat.text_of("Chord")).and_then(|id| track.chords.get(&(id as i64)).cloned());
        if beat.property("WhammyBar").is_some() {
            out.whammy = curve(beat, "WhammyBar");
        }
        out.vibrato = match beat.property("VibratoWTremBar").map(|v| v.text_of("Strength")) {
            Some("Wide") => Some(Vibrato::Wide),
            Some(_) => Some(Vibrato::Slight),
            None => None,
        };
        out.notes = ints(beat.text_of("Notes")).iter().take(64).filter_map(|id| notes.get(id)).filter_map(|n| read_note(n, track)).collect();
        out.rest = out.notes.is_empty();
        out
    }

    /// Quarter notes per minute from a tempo written as "beats unit", the
    /// unit being an eighth (1), a quarter (2), a dotted eighth (3), a half
    /// (4) or a dotted quarter (5).
    fn tempo_of(value: &str) -> Option<f64> {
        let mut parts = value.split_whitespace();
        let bpm = number(parts.next()?)?;
        let factor = match parts.next().and_then(number) {
            Some(1.0) => 0.5,
            Some(3.0) => 0.75,
            Some(4.0) => 2.0,
            Some(5.0) => 1.5,
            _ => 1.0,
        };
        Some(bpm * factor).filter(|t| *t > 0.0)
    }

    /// The elements of a list (`<Bars>`, `<Beats>`…) by their `id`.
    fn table<'a>(root: &'a Element, list: &str, item: &str) -> HashMap<i64, &'a Element> {
        root.child(list).map(|l| l.children(item).filter_map(|e| Some((e.attr("id")?.parse::<i64>().ok()?, e))).collect()).unwrap_or_default()
    }

    /// Reads the XML score itself.
    pub(super) fn parse_gpif(xml: &str) -> Result<Song, String> {
        let document = parse_xml(xml)?;
        let root = document.child("GPIF").ok_or("the score is not a Guitar Pro score: it has no GPIF element")?;
        let mut song = Song::default();
        if let Some(score) = root.child("Score") {
            song.title = score.text_of("Title").to_owned();
            song.artist = score.text_of("Artist").to_owned();
        }

        // The tempo changes, by bar, at a position that is a share of the bar.
        let mut tempos: HashMap<usize, Vec<(f64, f64)>> = HashMap::new();
        if let Some(automations) = root.child("MasterTrack").and_then(|m| m.child("Automations")) {
            for automation in automations.children("Automation").filter(|a| a.text_of("Type") == "Tempo") {
                let (Some(bar), Some(bpm)) = (number(automation.text_of("Bar")), tempo_of(automation.text_of("Value"))) else { continue };
                if bar < 0.0 || bar >= MAX_BARS as f64 {
                    continue;
                }
                let ratio = number(automation.text_of("Position")).unwrap_or(0.0).clamp(0.0, 1.0);
                tempos.entry(bar as usize).or_default().push((ratio, bpm));
            }
        }
        let first_tempo = tempos.iter().min_by_key(|(bar, _)| **bar).and_then(|(_, list)| list.first()).map(|&(_, bpm)| bpm);
        song.tempo = first_tempo.unwrap_or(120.0);

        let tracks: Vec<TrackInfo> = root.child("Tracks").map(|t| t.children("Track").map(read_track).collect()).unwrap_or_default();
        let (bars, voices, beats, notes, rhythms) =
            (table(root, "Bars", "Bar"), table(root, "Voices", "Voice"), table(root, "Beats", "Beat"), table(root, "Notes", "Note"), table(root, "Rhythms", "Rhythm"));

        // The bars, and the bar of each track in them.
        let mut signature = (4u32, 4u32);
        let mut bar_ids: Vec<Vec<i64>> = Vec::new();
        for (index, master) in root.child("MasterBars").map(|m| m.children("MasterBar").collect::<Vec<_>>()).unwrap_or_default().into_iter().enumerate() {
            if index >= MAX_BARS {
                return Err("the score has more bars than the program reads".to_owned());
            }
            if let Some((num, den)) = master.text_of("Time").split_once('/') {
                if let (Some(num), Some(den)) = (number(num), number(den)) {
                    if (1.0..=32.0).contains(&num) && [1.0, 2.0, 4.0, 8.0, 16.0, 32.0].contains(&den) {
                        signature = (num as u32, den as u32);
                    }
                }
            }
            if song.key.is_none() {
                if let Some(key) = master.child("Key") {
                    if let Some(count) = number(key.text_of("AccidentalCount")).filter(|c| (-7.0..=7.0).contains(c)) {
                        song.key = Some(songsterr::key_of(count as i64, key.text_of("Mode").eq_ignore_ascii_case("minor")));
                    }
                }
            }
            let repeat = master.child("Repeat");
            let mut bar = BarInfo {
                signature,
                repeat_start: repeat.and_then(|r| r.attr("start")) == Some("true"),
                repeat: repeat.filter(|r| r.attr("end") == Some("true")).map(|r| r.attr("count").and_then(number).unwrap_or(2.0).clamp(2.0, 64.0) as u32),
                endings: ints(master.text_of("AlternateEndings")).iter().filter(|e| (1..=64).contains(*e)).map(|e| *e as u32).collect(),
                marker: master.child("Section").map(|s| s.text_of("Text")).filter(|t| !t.is_empty()).or_else(|| master.child("Section").map(|s| s.text_of("Letter")).filter(|t| !t.is_empty())).map(str::to_owned),
                tempos: Vec::new(),
                // The feel of the bar, when it writes one: triplets of eighths or of sixteenths.
                feel: master.child("TripletFeel").map(|feel| match feel.text.trim() {
                    "Triplet8th" => 0.5,
                    "Triplet16th" => 0.25,
                    _ => 0.0,
                }),
                signs: Signs::default(),
            };
            // The signs of the score: places to jump to, and jumps.
            if let Some(directions) = master.child("Directions") {
                for sign in directions.children.iter().filter(|c| c.name == "Target" || c.name == "Jump") {
                    bar.signs.set(sign.text.trim());
                }
            }
            let length = bar.length();
            bar.tempos = tempos.remove(&index).unwrap_or_default().into_iter().map(|(ratio, bpm)| (ratio * length, bpm)).collect();
            song.bars.push(bar);
            bar_ids.push(ints(master.text_of("Bars")));
        }
        if song.bars.is_empty() {
            return Err("the score has no bars".to_owned());
        }

        for (index, info) in tracks.iter().enumerate() {
            let mut track = Track { name: info.name.clone(), program: info.program, tuning: info.tuning.clone(), capo: info.capo, bars: Vec::with_capacity(bar_ids.len()) };
            for ids in &bar_ids {
                let mut bar_voices = Vec::new();
                if let Some(bar) = ids.get(index).and_then(|id| bars.get(id)) {
                    for voice in ints(bar.text_of("Voices")).iter().filter(|v| **v >= 0).take(4).filter_map(|id| voices.get(id)) {
                        let beat_ids = ints(voice.text_of("Beats"));
                        if beat_ids.len() > MAX_BEATS {
                            return Err("the score has more beats in a bar than the program reads".to_owned());
                        }
                        bar_voices.push(beat_ids.iter().filter_map(|id| beats.get(id)).map(|b| read_beat(b, info, &notes, &rhythms)).collect());
                    }
                }
                track.bars.push(bar_voices);
            }
            song.lyrics.extend(info.lyrics.iter().map(|(bar, text)| (index, *bar, text.clone())));
            song.tracks.push(track);
        }
        Ok(song)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Content;

    /// Builds the bytes of a Guitar Pro file by hand.
    struct Writer(Vec<u8>);

    impl Writer {
        fn u8(&mut self, b: u8) {
            self.0.push(b);
        }

        fn i8(&mut self, b: i8) {
            self.0.push(b as u8);
        }

        fn i32(&mut self, n: i32) {
            self.0.extend_from_slice(&n.to_le_bytes());
        }

        fn zeros(&mut self, n: usize) {
            self.0.resize(self.0.len() + n, 0);
        }

        /// A length byte and a field of `size` bytes.
        fn byte_string(&mut self, size: usize, s: &str) {
            self.u8(s.len() as u8);
            let mut field = s.as_bytes().to_vec();
            field.resize(size, 0);
            self.0.extend(field);
        }

        /// The size of the field as an int, a length byte and the text.
        fn int_byte_string(&mut self, s: &str) {
            self.i32(s.len() as i32 + 1);
            self.u8(s.len() as u8);
            self.0.extend_from_slice(s.as_bytes());
        }

        fn int_string(&mut self, s: &str) {
            self.i32(s.len() as i32);
            self.0.extend_from_slice(s.as_bytes());
        }
    }

    /// The start of a beat: flags, the rest status, the duration and the tuplet.
    fn beat(w: &mut Writer, flags: u8, rest: bool, duration: i8, tuplet: Option<i32>) {
        w.u8(flags | if rest { 0x40 } else { 0 } | if tuplet.is_some() { 0x20 } else { 0 });
        if rest {
            w.u8(2);
        }
        w.i8(duration);
        if let Some(n) = tuplet {
            w.i32(n);
        }
    }

    /// The end of a beat: version 5 adds two flag bytes.
    fn beat_end(w: &mut Writer, five: bool) {
        if five {
            w.u8(0);
            w.u8(0);
        }
    }

    /// A note: its kind (1 normal, 2 tied, 3 dead), loudness, fret, extra
    /// flags (0x04 ghost, 0x40 accent) and effects.
    fn note(w: &mut Writer, five: bool, kind: u8, fret: i8, dynamic: Option<i8>, extra: u8, effects: Option<(u8, u8, &[u8])>) {
        let mut flags = 0x20 | extra;
        if dynamic.is_some() {
            flags |= 0x10;
        }
        if effects.is_some() {
            flags |= 0x08;
        }
        w.u8(flags);
        w.u8(kind);
        if let Some(d) = dynamic {
            w.i8(d);
        }
        w.i8(fret);
        if five {
            w.u8(0);
        }
        if let Some((flags1, flags2, body)) = effects {
            w.u8(flags1);
            w.u8(flags2);
            w.0.extend_from_slice(body);
        }
    }

    /// A full bend: three points up to a whole tone.
    fn bend() -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.u8(1);
        w.i32(100);
        w.i32(3);
        for (position, value) in [(0, 0), (30, 100), (60, 100)] {
            w.i32(position);
            w.i32(value);
            w.u8(0);
        }
        w.0
    }

    /// A chord diagram in the new format, with its name.
    fn chord(w: &mut Writer, name: &str) {
        w.u8(1);
        w.zeros(16);
        w.byte_string(21, name);
        w.zeros(4);
        w.i32(0);
        w.zeros(28);
        w.zeros(32);
    }

    /// A mix table change that only sets the tempo.
    fn tempo_change(w: &mut Writer, version: &str, tempo: i32) {
        let five = version.starts_with('5');
        w.i8(-1);
        if five {
            w.zeros(16);
        }
        for _ in 0..6 {
            w.i8(-1);
        }
        if five {
            w.int_byte_string("");
        }
        w.i32(tempo);
        w.u8(0); // over how many beats
        if version == "5.10" {
            w.u8(0);
        }
        w.u8(0); // apply to all tracks
        if five {
            w.i8(-1); // wah
        }
        if version == "5.10" {
            w.int_byte_string("");
            w.int_byte_string("");
        }
    }

    /// The beats of the guitar in each of the three measures.
    fn guitar_measure(w: &mut Writer, version: &str, measure: usize) {
        let five = version.starts_with('5');
        let slide = |kind: u8| -> Vec<u8> { vec![kind] };
        match measure {
            0 => {
                w.i32(5);
                // A quarter on the low E with a chord name and let ring.
                beat(w, 0x02, false, 0, None);
                chord(w, "Em");
                w.u8(0x02);
                note(w, five, 1, 0, None, 0, Some((0x08, 0, &[])));
                beat_end(w, five);
                // Two eighths on the high E: a hammer-on, then a legato slide.
                beat(w, 0, false, 1, None);
                w.u8(0x40);
                note(w, five, 1, 5, None, 0, Some((0x02, 0, &[])));
                beat_end(w, five);
                beat(w, 0, false, 1, None);
                w.u8(0x40);
                note(w, five, 1, 7, None, 0, Some((0, 0x08, &slide(2))));
                beat_end(w, five);
                // A dotted quarter on the B string, bent a whole tone.
                beat(w, 0x01, false, 0, None);
                w.u8(0x20);
                note(w, five, 1, 3, None, 0, Some((0x01, 0, &bend())));
                beat_end(w, five);
                // An eighth on the G string, staccato and palm muted, tied into the next bar.
                beat(w, 0, false, 1, None);
                w.u8(0x10);
                note(w, five, 1, 0, None, 0, Some((0, 0x03, &[])));
                beat_end(w, five);
            }
            1 => {
                w.i32(6);
                // The tie, then a rest.
                beat(w, 0, false, 0, None);
                w.u8(0x10);
                note(w, five, 2, 0, None, 0, None);
                beat_end(w, five);
                beat(w, 0, true, 0, None);
                w.u8(0);
                beat_end(w, five);
                // A triplet of eighths: a natural harmonic, a dead note, a ghost note.
                beat(w, 0, false, 1, Some(3));
                w.u8(0x40);
                note(w, five, 1, 12, None, 0, Some((0, 0x10, &[1])));
                beat_end(w, five);
                beat(w, 0, false, 1, Some(3));
                w.u8(0x40);
                note(w, five, 3, 8, None, 0, None);
                beat_end(w, five);
                beat(w, 0, false, 1, Some(3));
                w.u8(0x40);
                note(w, five, 1, 10, None, 0x04, None);
                beat_end(w, five);
                // A quarter with a tempo change: accented, mezzo piano, vibrato, shift slide.
                beat(w, 0x10, false, 0, None);
                tempo_change(w, version, 90);
                w.u8(0x08);
                note(w, five, 1, 2, Some(4), 0x40, Some((0, 0x48, &slide(1))));
                beat_end(w, five);
            }
            _ => {
                w.i32(3);
                beat(w, 0, false, 0, None);
                w.u8(0x04);
                note(w, five, 1, 5, None, 0, None);
                beat_end(w, five);
                // A quarter led into by a grace note from the first fret.
                let grace: &[u8] = if five { &[1, 6, 0, 1, 0] } else { &[1, 6, 0, 1] };
                beat(w, 0, false, 0, None);
                w.u8(0x40);
                note(w, five, 1, 3, None, 0, Some((0x10, 0, grace)));
                beat_end(w, five);
                // A quarter with a wide vibrato over the whole beat.
                beat(w, 0x08, false, 0, None);
                w.u8(0x02);
                w.u8(0);
                w.u8(0x20);
                note(w, five, 1, 0, None, 0, None);
                beat_end(w, five);
            }
        }
    }

    fn drum_measure(w: &mut Writer, five: bool, measure: usize) {
        let hit = |w: &mut Writer, flags: u8, duration: i8, key: i8| {
            beat(w, flags, false, duration, None);
            w.u8(0x40);
            note(w, five, 1, key, None, 0, None);
            beat_end(w, five);
        };
        match measure {
            0 => {
                w.i32(4);
                for _ in 0..4 {
                    hit(w, 0, 0, 36);
                }
            }
            1 => {
                w.i32(3);
                hit(w, 0, 0, 38);
                hit(w, 0, 0, 42);
                hit(w, 0, -1, 49);
            }
            _ => {
                w.i32(1);
                hit(w, 0x01, -1, 36);
            }
        }
    }

    /// A whole file of version "4.06", "5.00" or "5.10": three measures (the
    /// first two repeated), a guitar and a drum kit.
    fn song_file(version: &str) -> Vec<u8> {
        let five = version.starts_with('5');
        let v510 = version == "5.10";
        let mut w = Writer(Vec::new());
        w.byte_string(30, &format!("FICHIER GUITAR PRO v{version}"));
        let texts: &[&str] = if five { &["Test Song", "", "Tester", "Album", "Words", "Music", "Copyright", "Tabber", "Instructions"] } else { &["Test Song", "", "Tester", "Album", "Words", "Copyright", "Tabber", "Instructions"] };
        for text in texts {
            w.int_byte_string(text);
        }
        w.i32(1);
        w.int_byte_string("A notice");
        if !five {
            w.u8(0); // triplet feel
        }
        w.i32(1); // lyrics
        for line in 0..5 {
            w.i32(1);
            w.int_string(if line == 0 { "la la" } else { "" });
        }
        if v510 {
            w.i32(100);
            w.i32(0);
            w.zeros(11);
        }
        if five {
            for _ in 0..7 {
                w.i32(0);
            }
            w.zeros(2);
            for _ in 0..10 {
                w.int_byte_string("%TITLE%");
            }
            w.int_byte_string("Moderate");
        }
        w.i32(120);
        if v510 {
            w.u8(0);
        }
        w.i8(1); // one sharp
        w.i32(0); // octave
        for channel in 0..64 {
            w.i32(match channel {
                0 => 24,
                9 => 0,
                _ => 25,
            });
            w.zeros(8);
        }
        if five {
            w.zeros(38);
            w.i32(0);
        }
        w.i32(3);
        w.i32(2);

        // Measure 1: 4/4, repeat start, marker, key signature (minor).
        w.u8(0x01 | 0x02 | 0x04 | 0x20 | 0x40);
        w.u8(4);
        w.u8(4);
        w.int_byte_string("Intro");
        w.zeros(4);
        w.i8(1);
        w.u8(1);
        if five {
            w.zeros(4);
            w.u8(0);
            w.u8(0);
        }
        // Measure 2: repeat close, played twice.
        if five {
            w.u8(0);
        }
        w.u8(0x08);
        w.u8(if five { 2 } else { 1 });
        if five {
            w.u8(0);
            w.u8(0);
        }
        // Measure 3: 3/4 with a marker.
        if five {
            w.u8(0);
        }
        w.u8(0x01 | 0x20);
        w.u8(3);
        w.int_byte_string("Verse");
        w.zeros(4);
        if five {
            w.zeros(4);
            w.u8(0);
            w.u8(0);
        }

        // The tracks.
        for (index, (name, flags, channel)) in [("Guitar", 0u8, 1), ("Drums", 1u8, 10)].into_iter().enumerate() {
            if version == "5.00" || (v510 && index == 0) {
                w.u8(0);
            }
            w.u8(flags);
            w.byte_string(40, name);
            w.i32(6);
            for pitch in if index == 0 { [64, 59, 55, 50, 45, 40, 0] } else { [0; 7] } {
                w.i32(pitch);
            }
            w.i32(1); // port
            w.i32(channel);
            w.i32(channel + 1);
            w.i32(24); // frets
            w.i32(0); // capo
            w.zeros(4);
            if five {
                w.zeros(if v510 { 49 } else { 44 });
                if v510 {
                    w.int_byte_string("Clean");
                    w.int_byte_string("Guitar");
                }
            }
        }
        if five {
            w.zeros(if v510 { 1 } else { 2 });
        }

        // The measures.
        for measure in 0..3 {
            for track in 0..2 {
                if track == 0 {
                    guitar_measure(&mut w, version, measure);
                } else {
                    drum_measure(&mut w, five, measure);
                }
                if five {
                    w.i32(0); // the second voice
                    // A line break after each bar but the last of the file, as Guitar Pro writes it.
                    if measure < 2 || track < 1 {
                        w.u8(0);
                    }
                }
            }
        }
        w.0
    }

    fn check_song(score: &Score) {
        // The words "la la" from the first bar of the guitar: a syllable on each
        // of its first two beats that strike, sung again when the bar is.
        let words: Vec<(f64, &str)> = score.parts[0].lyrics.iter().map(|s| (s.beat, s.text.as_str())).collect();
        assert!(words.len() >= 2 && words[..2].iter().all(|(_, t)| *t == "la"), "{words:?}");
        assert!(score.parts[1].lyrics.is_empty(), "the drums sing nothing");
        assert_eq!((score.title.as_str(), score.artist.as_str()), ("Test Song", "Tester"));
        assert_eq!(score.programs(), [24, DRUMS]);
        let guitar = &score.parts[0];
        assert_eq!(guitar.name, "Guitar");
        assert_eq!(guitar.signature, (4, 4));
        assert_eq!(guitar.key, Some((4, true)), "one sharp, minor: E minor");
        assert_eq!(guitar.tuning, [64, 59, 55, 50, 45, 40]);
        assert_eq!(guitar.length, 19.0, "two bars of 4/4 played twice, then one of 3/4");
        assert_eq!(guitar.markers.len(), 2);
        assert_eq!((guitar.markers[0].name.as_str(), guitar.markers[0].start, guitar.markers[0].length), ("Intro", 0.0, 16.0));
        assert_eq!((guitar.markers[1].name.as_str(), guitar.markers[1].start, guitar.markers[1].length), ("Verse", 16.0, 3.0));
        assert_eq!(guitar.chords, [ChordMark { beat: 0.0, name: "Em".to_owned() }, ChordMark { beat: 8.0, name: "Em".to_owned() }]);
        let tempo: Vec<(f64, f64)> = guitar.tempo.iter().map(|p| (p.beat, p.value)).collect();
        assert_eq!(tempo, [(0.0, 120.0), (7.0, 90.0), (15.0, 90.0)]);

        let at = |beat: f64| -> Vec<&Note> { guitar.notes.iter().filter(|n| (n.start - beat).abs() < 1e-9).collect() };
        let open_e = at(0.0)[0];
        assert_eq!((open_e.pitch, open_e.length, open_e.velocity, open_e.string), (40, 1.0, 96, Some(5)));
        assert!(open_e.articulation.let_ring);
        assert_eq!((at(1.0)[0].pitch, at(1.0)[0].length, at(1.0)[0].articulation.hammer), (69, 0.5, true));
        assert_eq!((at(1.5)[0].pitch, at(1.5)[0].articulation.slide), (71, Some(Slide::Legato)));
        let bent = at(2.0)[0];
        assert_eq!((bent.pitch, bent.length, bent.string), (62, 1.5, Some(1)));
        assert_eq!(bent.articulation.bend, [(0.0, 0.0), (0.5, 2.0), (1.0, 2.0)], "a full bend: two semitones");
        let tied = at(3.5)[0];
        assert_eq!((tied.pitch, tied.length), (55, 1.5), "an eighth tied to a quarter in the next bar");
        assert!(tied.articulation.staccato && tied.articulation.palm_mute);
        assert!(at(4.0).is_empty() && at(5.0).is_empty(), "the tie and the rest");
        let harmonic = at(6.0)[0];
        assert_eq!((harmonic.pitch, harmonic.articulation.harmonic), (76, Some(12)), "the twelfth-fret harmonic sounds the octave");
        assert!((harmonic.length - 1.0 / 3.0).abs() < 1e-9, "{}", harmonic.length);
        let dead = at(6.0 + 1.0 / 3.0)[0];
        assert!((dead.pitch, dead.articulation.dead) == (72, true));
        let ghost = at(6.0 + 2.0 / 3.0)[0];
        assert_eq!((ghost.pitch, ghost.velocity, ghost.articulation.ghost), (74, 58, true));
        let accented = at(7.0)[0];
        assert_eq!((accented.pitch, accented.velocity, accented.string), (52, 76, Some(3)), "mezzo piano plus an accent");
        assert_eq!((accented.articulation.accent, accented.articulation.vibrato, accented.articulation.slide), (1, Some(Vibrato::Slight), Some(Slide::Shift)));
        // The second pass of the repeat.
        assert_eq!(at(8.0)[0].pitch, 40);
        assert_eq!(at(15.0)[0].pitch, 52);
        // The last bar: a plain note, a grace note and its note, a wide vibrato.
        assert_eq!((at(16.0)[0].pitch, at(16.0)[0].articulation.is_plain()), (50, true));
        let grace = at(16.875)[0];
        assert_eq!((grace.pitch, grace.length, grace.string), (65, 0.125, Some(0)));
        assert_eq!((at(17.0)[0].pitch, at(17.0)[0].length), (67, 1.0));
        assert_eq!((at(18.0)[0].pitch, at(18.0)[0].articulation.vibrato), (59, Some(Vibrato::Wide)));
        assert_eq!(guitar.notes.len(), 2 * 9 + 4);
        assert!(guitar.notes.windows(2).all(|w| w[0].start <= w[1].start), "sorted by time");

        let drums = &score.parts[1];
        assert!(drums.is_drums() && drums.tuning.is_empty() && drums.chords.is_empty());
        assert!(drums.notes.iter().all(|n| n.string.is_none() && n.articulation.is_plain()));
        let hits: Vec<(f64, u8, f64)> = drums.notes.iter().map(|n| (n.start, n.pitch, n.length)).collect();
        let pass = |offset: f64| [(0.0, 36, 1.0), (1.0, 36, 1.0), (2.0, 36, 1.0), (3.0, 36, 1.0), (4.0, 38, 1.0), (5.0, 42, 1.0), (6.0, 49, 2.0)].map(|(b, p, l)| (b + offset, p, l));
        let expected: Vec<(f64, u8, f64)> = pass(0.0).into_iter().chain(pass(8.0)).chain([(16.0, 36, 3.0)]).collect();
        assert_eq!(hits, expected);
    }

    #[test]
    fn a_version_4_file_is_read() {
        let score = parse(&song_file("4.06")).unwrap();
        check_song(&score);
    }

    #[test]
    fn version_5_files_of_both_revisions_are_read() {
        for version in ["5.00", "5.10"] {
            let score = parse(&song_file(version)).unwrap_or_else(|e| panic!("{version}: {e}"));
            check_song(&score);
        }
    }

    #[test]
    fn the_score_becomes_a_project() {
        let score = parse(&song_file("5.10")).unwrap();
        let project = score.project();
        assert_eq!(project.name, "Tester – Test Song");
        assert_eq!((project.tempo, project.tempo_points.len()), (120.0, 3));
        assert_eq!((project.beats_per_bar, project.beat_unit), (4, 4));
        assert_eq!((project.key, project.minor), (4, true));
        assert_eq!(project.markers.len(), 2);
        assert_eq!(project.tracks.len(), 2);
        assert_eq!(project.tracks[0].tuning, [64, 59, 55, 50, 45, 40]);
        assert!(project.tracks[1].tuning.is_empty());
        assert_eq!(project.tracks[0].patch, "Nylon guitar");
        assert!(matches!(&project.tracks[0].regions[0].content, Content::Midi(notes) if notes.len() == score.parts[0].notes.len()));
        assert_eq!(project.tracks[0].regions[0].chords.len(), 2);
        assert_eq!(project.tracks[0].regions[0].length, 20.0, "19 beats fit in five bars");
        assert!(is_guitar_pro(Path::new("song.GP5")) && is_guitar_pro(Path::new("a/b.gp")) && is_guitar_pro(Path::new("x.gpx")));
        assert!(!is_guitar_pro(Path::new("song.mid")) && !is_guitar_pro(Path::new("gp5")));
    }

    #[test]
    fn garbage_and_truncated_files_are_refused() {
        for version in ["4.06", "5.00", "5.10"] {
            let file = song_file(version);
            for len in 0..file.len() {
                assert!(parse(&file[..len]).is_err(), "{version}: {len} of {} bytes read as a song", file.len());
            }
            assert!(parse(&file).is_ok());
        }
        // Random bytes behind every known header.
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut random = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let headers = ["FICHIER GUITAR PRO v3.00", "FICHIER GUITAR PRO v4.00", "FICHIER GUITAR PRO v4.06", "FICHIER GUITAR PRO L4.06", "FICHIER GUITAR PRO v5.00", "FICHIER GUITAR PRO v5.10", "PK\x03\x04", "BCFZ", ""];
        for header in headers {
            for _ in 0..25 {
                let mut bytes = Vec::new();
                if header.starts_with("FICHIER") {
                    bytes.push(header.len() as u8);
                    bytes.extend_from_slice(header.as_bytes());
                    bytes.resize(31, 0);
                } else {
                    bytes.extend_from_slice(header.as_bytes());
                }
                let n = (random() % 3000) as usize;
                for _ in 0..n {
                    bytes.push(random() as u8);
                }
                assert!(parse(&bytes).is_err(), "{header:?} with {n} random bytes read as a song");
            }
        }
        assert!(parse(b"BCFZ\0\0\0\0").unwrap_err().contains("Guitar Pro 6"));
        assert!(parse(b"\x18FICHIER GUITARE PRO v1.02\0\0\0\0\0\0").is_err());
        assert!(parse(b"\x18FICHIER GUITAR PRO v2.21\0\0\0\0\0\0\0").unwrap_err().contains("not read"));
    }

    /// A version 3 file: one bar of one track, with both chord diagram
    /// layouts, a beat-wide harmonic and tremolo bar, a mix table change,
    /// a note with bend, slide and grace, and a rest.
    fn gp3_file() -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.byte_string(30, "FICHIER GUITAR PRO v3.00");
        for text in ["Three", "", "Trio", "", "", "", "", ""] {
            w.int_byte_string(text);
        }
        w.i32(0); // notices
        w.u8(0); // triplet feel
        w.i32(100);
        w.i32(0); // key
        for channel in 0..64 {
            w.i32(if channel == 0 { 25 } else { 0 });
            w.zeros(8);
        }
        w.i32(1);
        w.i32(1);
        w.u8(0x03);
        w.u8(4);
        w.u8(4);
        w.u8(0);
        w.byte_string(40, "Lead");
        w.i32(6);
        for pitch in [64, 59, 55, 50, 45, 40, 0] {
            w.i32(pitch);
        }
        w.i32(1);
        w.i32(1);
        w.i32(2);
        w.i32(24);
        w.i32(2); // capo
        w.zeros(4);
        w.i32(5);
        // A quarter with a chord diagram in the newer layout.
        beat(&mut w, 0x02, false, 0, None);
        w.u8(1);
        w.zeros(25);
        w.byte_string(22, "G");
        w.zeros(12);
        w.i32(3);
        w.zeros(24);
        w.zeros(36);
        w.u8(0x02);
        note(&mut w, false, 1, 3, None, 0, None);
        // An eighth with a chord diagram in the old layout, without frets.
        beat(&mut w, 0x02, false, 1, None);
        w.u8(0);
        w.int_byte_string("Em");
        w.i32(0);
        w.u8(0x40);
        note(&mut w, false, 1, 0, None, 0, None);
        // An eighth with a natural harmonic and a tremolo bar dip over the beat.
        beat(&mut w, 0x08, false, 1, None);
        w.u8(0x04 | 0x20);
        w.u8(0);
        w.i32(100);
        w.u8(0x40);
        note(&mut w, false, 1, 12, None, 0, None);
        // A quarter with a tempo change; its note has a bend, a slide and a grace note.
        beat(&mut w, 0x10, false, 0, None);
        for _ in 0..7 {
            w.i8(-1);
        }
        w.i32(80);
        w.u8(0);
        w.u8(0x20);
        w.u8(0x20 | 0x08);
        w.u8(1);
        w.i8(5);
        w.u8(0x01 | 0x04 | 0x10);
        w.0.extend(bend());
        w.0.extend_from_slice(&[2, 6, 0, 1]);
        // A quarter rest.
        beat(&mut w, 0, true, 0, None);
        w.u8(0);
        w.0
    }

    #[test]
    fn a_version_3_file_is_read() {
        let file = gp3_file();
        let score = parse(&file).unwrap();
        assert_eq!((score.title.as_str(), score.artist.as_str()), ("Three", "Trio"));
        assert_eq!(score.programs(), [25]);
        let lead = &score.parts[0];
        assert_eq!((lead.length, lead.signature, lead.key), (4.0, (4, 4), Some((0, false))));
        assert_eq!(lead.chords, [ChordMark { beat: 0.0, name: "G".to_owned() }, ChordMark { beat: 1.0, name: "Em".to_owned() }]);
        let tempo: Vec<(f64, f64)> = lead.tempo.iter().map(|p| (p.beat, p.value)).collect();
        assert_eq!(tempo, [(0.0, 100.0), (2.0, 80.0)]);
        let at = |beat: f64| -> Vec<&Note> { lead.notes.iter().filter(|n| (n.start - beat).abs() < 1e-9).collect() };
        // The capo at the second fret raises every note.
        assert_eq!((at(0.0)[0].pitch, at(0.0)[0].string), (45, Some(5)));
        assert_eq!(at(1.0)[0].pitch, 66);
        let harmonic = at(1.5)[0];
        assert_eq!((harmonic.pitch, harmonic.articulation.harmonic), (78, Some(12)), "a harmonic over the whole beat");
        assert_eq!(harmonic.articulation.bend, [(0.0, 0.0), (0.5, -2.0), (1.0, 0.0)], "a dip of a whole tone");
        let grace = at(1.875)[0];
        assert_eq!((grace.pitch, grace.length), (63, 0.125));
        let slid = at(2.0)[0];
        assert_eq!((slid.pitch, slid.articulation.slide), (66, Some(Slide::Shift)));
        assert_eq!(slid.articulation.bend, [(0.0, 0.0), (0.5, 2.0), (1.0, 2.0)]);
        assert_eq!(lead.notes.len(), 5);

        // Truncated files say where they end.
        for len in 0..file.len() {
            let error = parse(&file[..len]).unwrap_err();
            assert!(error.contains("byte") || error.contains("not a Guitar Pro file"), "{len}: {error}");
        }
        let error = parse(&file[..file.len() - 1]).unwrap_err();
        assert_eq!(error, format!("the file ends inside the beats of bar 1 of track 1 (byte {} of {})", file.len() - 1, file.len() - 1));
        assert!(parse(&file[..200]).unwrap_err().starts_with("the file ends inside the song header (byte "));
    }

    #[test]
    fn refusals_say_what_was_found() {
        let header = |text: &str| {
            let mut w = Writer(Vec::new());
            w.byte_string(30, text);
            w.zeros(64);
            w.0
        };
        assert_eq!(parse(&header("FICHIER GUITARE PRO v1.04")).unwrap_err(), "Guitar Pro version \"FICHIER GUITARE PRO v1.04\" is not read yet; export the song as GP5 or GP7");
        assert_eq!(parse(&header("FICHIER GUITAR PRO v2.21")).unwrap_err(), "Guitar Pro version \"FICHIER GUITAR PRO v2.21\" is not read yet; export the song as GP5 or GP7");
        assert_eq!(parse(&header("FICHIER GUITAR PRO v6.00")).unwrap_err(), "Guitar Pro version \"FICHIER GUITAR PRO v6.00\" is not read yet; export the song as GP5 or GP7");
        assert_eq!(parse(&header("RIFF")).unwrap_err(), "not a Guitar Pro file: it starts with \"RIFF\" instead of \"FICHIER GUITAR PRO\"");
        assert_eq!(parse(b"short").unwrap_err(), "not a Guitar Pro file: it is only 5 bytes long");
        let mut absurd = header("FICHIER GUITAR PRO v4.06");
        absurd.truncate(31);
        absurd.extend_from_slice(&i32::MAX.to_le_bytes());
        assert_eq!(parse(&absurd).unwrap_err(), "the file announces 2147483647 characters in the song header (byte 35 of 35), which cannot be right");
    }

    /// A score of two bars (repeated) for a guitar and a drum kit.
    const GPIF: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<GPIF>
  <GPVersion>7</GPVersion>
  <Score><Title>Seven &amp; Up</Title><SubTitle/><Artist>Someone</Artist><Album></Album></Score>
  <MasterTrack><Tracks>0 1</Tracks><Automations>
    <Automation><Type>Tempo</Type><Linear>false</Linear><Bar>0</Bar><Position>0</Position><Visible>true</Visible><Value>100 2</Value></Automation>
    <Automation><Type>Tempo</Type><Linear>false</Linear><Bar>1</Bar><Position>0</Position><Visible>true</Visible><Value>60 4</Value></Automation>
  </Automations></MasterTrack>
  <Tracks>
    <Track id="0"><Name>Lead</Name><ShortName>ld.</ShortName><Color>255 0 0</Color>
      <MidiConnection><Port>0</Port><PrimaryChannel>0</PrimaryChannel><SecondaryChannel>1</SecondaryChannel></MidiConnection>
      <Sounds><Sound><Name>Clean</Name><MIDI><Program>27</Program></MIDI></Sound></Sounds>
      <Staves><Staff><Properties>
        <Property name="CapoFret"><Fret>1</Fret></Property>
        <Property name="Tuning"><Pitches>40 45 50 55 59 64</Pitches></Property>
        <Property name="DiagramCollection"><Items><Item id="0" name="Am"><Diagram/></Item></Items></Property>
      </Properties></Staff></Staves>
      <Transpose><Chromatic>0</Chromatic><Octave>0</Octave></Transpose>
    </Track>
    <Track id="1"><Name>Kit</Name>
      <MidiConnection><Port>0</Port><PrimaryChannel>9</PrimaryChannel></MidiConnection>
      <InstrumentSet><Name>Drumkit</Name><Type>drumKit</Type><Elements>
        <Element><Name>Kick</Name><Articulations><Articulation><OutputMidiNumber>36</OutputMidiNumber></Articulation></Articulations></Element>
        <Element><Name>Snare</Name><Articulations><Articulation><OutputMidiNumber>38</OutputMidiNumber></Articulation><Articulation><OutputMidiNumber>40</OutputMidiNumber></Articulation></Articulations></Element>
      </Elements></InstrumentSet>
    </Track>
  </Tracks>
  <MasterBars>
    <MasterBar><Key><AccidentalCount>-3</AccidentalCount><Mode>Minor</Mode></Key><Time>4/4</Time><Repeat start="true" end="false" count="0"/><Section><Letter>A</Letter><Text>Intro</Text></Section><Bars>0 2</Bars></MasterBar>
    <MasterBar><Time>4/4</Time><Repeat start="false" end="true" count="2"/><Bars>1 3</Bars><!-- the second bar --></MasterBar>
  </MasterBars>
  <Bars>
    <Bar id="0"><Clef>G2</Clef><Voices>0 -1 -1 -1</Voices></Bar>
    <Bar id="1"><Voices>1 -1 -1 -1</Voices></Bar>
    <Bar id="2"><Voices>2 -1 -1 -1</Voices></Bar>
    <Bar id="3"><Voices>3 -1 -1 -1</Voices></Bar>
  </Bars>
  <Voices>
    <Voice id="0"><Beats>0 1 2 3 9</Beats></Voice>
    <Voice id="1"><Beats>4 5 6 10 11 12</Beats></Voice>
    <Voice id="2"><Beats>7 13 14 15</Beats></Voice>
    <Voice id="3"><Beats>16</Beats></Voice>
  </Voices>
  <Beats>
    <Beat id="0"><Dynamic>MF</Dynamic><Rhythm ref="0"/><Notes>0</Notes><Chord>0</Chord></Beat>
    <Beat id="1"><Rhythm ref="1"/><Notes>1</Notes></Beat>
    <Beat id="2"><Rhythm ref="1"/><Notes>2</Notes><GraceNotes>BeforeBeat</GraceNotes></Beat>
    <Beat id="3"><Rhythm ref="2"/><Notes>3 4</Notes></Beat>
    <Beat id="9"><Rhythm ref="1"/></Beat>
    <Beat id="4"><Rhythm ref="0"/><Notes>5</Notes></Beat>
    <Beat id="5"><Rhythm ref="3"/><Notes>6</Notes></Beat>
    <Beat id="6"><Rhythm ref="4"/><Notes>7</Notes></Beat>
    <Beat id="10"><Rhythm ref="4"/><Notes>8</Notes><Properties><Property name="WhammyBar"><Enable/></Property><Property name="WhammyBarOriginValue"><Float>0</Float></Property><Property name="WhammyBarDestinationValue"><Float>-200</Float></Property></Properties></Beat>
    <Beat id="11"><Rhythm ref="4"/></Beat>
    <Beat id="12"><Rhythm ref="1"/></Beat>
    <Beat id="7"><Rhythm ref="0"/><Notes>9</Notes></Beat>
    <Beat id="13"><Rhythm ref="0"/><Notes>10</Notes></Beat>
    <Beat id="14"><Rhythm ref="0"/><Notes>11</Notes></Beat>
    <Beat id="15"><Rhythm ref="0"/></Beat>
    <Beat id="16"><Rhythm ref="5"/><Notes>9</Notes></Beat>
  </Beats>
  <Notes>
    <Note id="0"><Properties><Property name="String"><String>5</String></Property><Property name="Fret"><Fret>0</Fret></Property></Properties></Note>
    <Note id="1"><Properties><Property name="String"><String>0</String></Property><Property name="Fret"><Fret>3</Fret></Property><Property name="HopoOrigin"><Enable/></Property>
      <Property name="Bended"><Enable/></Property><Property name="BendOriginValue"><Float>0</Float></Property><Property name="BendMiddleOffset1"><Float>50</Float></Property><Property name="BendMiddleOffset2"><Float>50</Float></Property><Property name="BendMiddleValue"><Float>100</Float></Property><Property name="BendDestinationOffset"><Float>100</Float></Property><Property name="BendDestinationValue"><Float>100</Float></Property></Properties></Note>
    <Note id="2"><Properties><Property name="String"><String>1</String></Property><Property name="Fret"><Fret>2</Fret></Property></Properties></Note>
    <Note id="3"><Properties><Property name="String"><String>2</String></Property><Property name="Fret"><Fret>12</Fret></Property><Property name="Harmonic"><HType>Natural</HType></Property><Property name="HarmonicFret"><HFret>12</HFret></Property></Properties></Note>
    <Note id="4"><Accent>4</Accent><Vibrato>Slight</Vibrato><Properties><Property name="String"><String>4</String></Property><Property name="Fret"><Fret>0</Fret></Property><Property name="Muted"><Enable/></Property></Properties></Note>
    <Note id="5"><Tie origin="false" destination="true"/><Properties><Property name="String"><String>4</String></Property><Property name="Fret"><Fret>0</Fret></Property></Properties></Note>
    <Note id="6"><Accent>9</Accent><LetRing/><Properties><Property name="String"><String>0</String></Property><Property name="Fret"><Fret>5</Fret></Property><Property name="Slide"><Flags>2</Flags></Property><Property name="PalmMuted"><Enable/></Property></Properties></Note>
    <Note id="7"><AntiAccent>Normal</AntiAccent><Properties><Property name="String"><String>3</String></Property><Property name="Fret"><Fret>1</Fret></Property></Properties></Note>
    <Note id="8"><Properties><Property name="String"><String>5</String></Property><Property name="Fret"><Fret>7</Fret></Property><Property name="Harmonic"><HType>Artificial</HType></Property></Properties></Note>
    <Note id="9"><InstrumentArticulation>0</InstrumentArticulation><Properties><Property name="Element"><Element>0</Element></Property></Properties></Note>
    <Note id="10"><Properties><Property name="Element"><Element>1</Element></Property><Property name="Variation"><Variation>1</Variation></Property></Properties></Note>
    <Note id="11"><Properties><Property name="Midi"><Number>42</Number></Property></Properties></Note>
  </Notes>
  <Rhythms>
    <Rhythm id="0"><NoteValue>Quarter</NoteValue></Rhythm>
    <Rhythm id="1"><NoteValue>Eighth</NoteValue></Rhythm>
    <Rhythm id="2"><NoteValue>Half</NoteValue></Rhythm>
    <Rhythm id="3"><NoteValue>Quarter</NoteValue><AugmentationDot count="1"/></Rhythm>
    <Rhythm id="4"><NoteValue>Eighth</NoteValue><PrimaryTuplet num="3" den="2"/></Rhythm>
    <Rhythm id="5"><NoteValue>Whole</NoteValue></Rhythm>
  </Rhythms>
</GPIF>"#;

    #[test]
    fn jumps_feels_tremolo_trills_and_grace_notes_on_the_beat_are_read() {
        // Five bars of one whole note each, the fret giving the bar: a segno on
        // bar 2, "to coda" on bar 3, D.S. al coda on bar 4, the coda on bar 5.
        // The feel is a triplet one from bar 1, straight from bar 4.
        let signs = ["<TripletFeel>Triplet8th</TripletFeel>", "<Directions><Target>Segno</Target></Directions>", "<Directions><Jump>DaCoda</Jump></Directions>", "<Directions><Jump>DaSegnoAlCoda</Jump></Directions><TripletFeel>NoTripletFeel</TripletFeel>", "<Directions><Target>Coda</Target></Directions>"];
        let masters: String = signs.iter().enumerate().map(|(bar, extra)| format!("<MasterBar><Time>4/4</Time><Bars>{bar}</Bars>{extra}</MasterBar>")).collect();
        let bars: String = (0..5).map(|bar| format!("<Bar id=\"{bar}\"><Voices>{bar} -1 -1 -1</Voices></Bar>")).collect();
        let voices: String = (0..5).map(|bar| format!("<Voice id=\"{bar}\"><Beats>{}</Beats></Voice>", if bar == 0 { "10 0".to_owned() } else { bar.to_string() })).collect();
        let beats: String = (0..5).map(|bar| format!("<Beat id=\"{bar}\"><Rhythm ref=\"0\"/><Notes>{bar}</Notes>{}</Beat>", if bar == 0 { "<Tremolo>1/4</Tremolo>" } else { "" })).collect();
        let notes: String = (0..5).map(|bar| format!("<Note id=\"{bar}\">{}<Properties><Property name=\"String\"><String>5</String></Property><Property name=\"Fret\"><Fret>{bar}</Fret></Property></Properties></Note>", if bar == 4 { "<Trill>70</Trill>" } else { "" })).collect();
        let score = format!(
            r#"<GPIF><MasterTrack><Tracks>0</Tracks></MasterTrack>
            <Tracks><Track id="0"><Name>G</Name><Staves><Staff><Properties><Property name="Tuning"><Pitches>40 45 50 55 59 64</Pitches></Property></Properties></Staff></Staves></Track></Tracks>
            <MasterBars>{masters}</MasterBars><Bars>{bars}</Bars><Voices>{voices}</Voices>
            <Beats>{beats}<Beat id="10"><Rhythm ref="1"/><Notes>10</Notes><GraceNotes>OnBeat</GraceNotes></Beat></Beats>
            <Notes>{notes}<Note id="10"><Properties><Property name="String"><String>5</String></Property><Property name="Fret"><Fret>7</Fret></Property></Properties></Note></Notes>
            <Rhythms><Rhythm id="0"><NoteValue>Whole</NoteValue></Rhythm><Rhythm id="1"><NoteValue>32nd</NoteValue></Rhythm></Rhythms></GPIF>"#
        );
        let part = &parse(score.as_bytes()).unwrap().parts[0];
        let open = part.notes.iter().map(|n| n.pitch).min().unwrap();
        let played: Vec<(f64, u8)> = part.notes.iter().map(|n| (n.start, n.pitch - open)).collect();
        // The grace note on the beat, then bars 1 to 4, back to the segno (bar 2),
        // on to "to coda" (bar 3), and over to the coda (bar 5).
        assert_eq!(played, [(0.0, 7), (0.125, 0), (4.0, 1), (8.0, 2), (12.0, 3), (16.0, 1), (20.0, 2), (24.0, 4)]);
        assert_eq!(part.length, 28.0);
        assert_eq!(part.notes[1].length, 3.875, "the note comes as late as its grace note lasts");
        assert_eq!(part.notes[1].articulation.tremolo, Some(0.25));
        assert_eq!(part.notes[7].articulation.trill, Some((70, 0.25)));
        // The feel where it is played: triplets, straight at bar 4, triplets again
        // when bar 2 is played again, straight at the coda.
        let feels: Vec<(f64, bool)> = part.swing.iter().map(|s| (s.beat, s.is_straight())).collect();
        assert_eq!(feels, [(0.0, false), (12.0, true), (16.0, false), (24.0, true)]);
    }

    #[test]
    fn the_jumps_of_the_score_change_the_order_of_play() {
        let bar = |signs: Signs, endings: &[u32]| BarInfo { signature: (4, 4), endings: endings.to_vec(), signs, ..BarInfo::default() };
        let mut fine = Signs::default();
        fine.set("Fine");
        let mut dc = Signs::default();
        dc.set("DaCapoAlFine");
        // D.C. al fine after a repeat with two endings: the second pass plays the last ending only.
        let mut bars = vec![bar(Signs::default(), &[]), bar(fine, &[]), bar(Signs::default(), &[1]), bar(Signs::default(), &[2]), bar(dc, &[])];
        bars[0].repeat_start = true;
        bars[2].repeat = Some(2);
        let base = vec![0, 1, 2, 0, 1, 3, 4];
        assert_eq!(play_with_jumps(&bars, base.clone()), [0, 1, 2, 0, 1, 3, 4, 0, 1], "back to the start, to the fine");
        // Without a fine, to the end, on the last endings.
        bars[1].signs.fine = false;
        assert_eq!(play_with_jumps(&bars, base.clone()), [0, 1, 2, 0, 1, 3, 4, 0, 1, 3, 4]);
        // A jump to a sign that is not there changes nothing.
        let mut ds = Signs::default();
        ds.set("DaSegno");
        bars[4].signs = ds;
        assert_eq!(play_with_jumps(&bars, base.clone()), base);
        let mut unknown = Signs::default();
        unknown.set("DaSomething");
        assert_eq!(unknown, Signs::default());
    }

    /// Zips a score as a Guitar Pro 7 file does.
    fn gp7_file(score: &str) -> Vec<u8> {
        use std::io::Write;
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("VERSION", options).unwrap();
        archive.write_all(b"7.0").unwrap();
        archive.start_file("Content/score.gpif", options).unwrap();
        archive.write_all(score.as_bytes()).unwrap();
        archive.finish().unwrap().into_inner()
    }

    #[test]
    fn a_version_7_file_is_read() {
        let score = parse(&gp7_file(GPIF)).unwrap();
        assert_eq!((score.title.as_str(), score.artist.as_str()), ("Seven & Up", "Someone"));
        assert_eq!(score.programs(), [27, DRUMS]);
        let lead = &score.parts[0];
        assert_eq!(lead.name, "Lead");
        assert_eq!(lead.tuning, [64, 59, 55, 50, 45, 40], "the file lists the strings lowest first");
        assert_eq!(lead.key, Some((0, true)), "three flats, minor: C minor");
        assert_eq!(lead.length, 16.0, "two bars played twice");
        assert_eq!(lead.markers.len(), 1);
        assert_eq!((lead.markers[0].name.as_str(), lead.markers[0].start, lead.markers[0].length), ("Intro", 0.0, 16.0));
        assert_eq!(lead.chords, [ChordMark { beat: 0.0, name: "Am".to_owned() }, ChordMark { beat: 8.0, name: "Am".to_owned() }]);
        let tempo: Vec<(f64, f64)> = lead.tempo.iter().map(|p| (p.beat, p.value)).collect();
        assert_eq!(tempo, [(0.0, 100.0), (4.0, 120.0), (8.0, 100.0), (12.0, 120.0)], "60 half notes a minute are 120 quarters");

        let at = |beat: f64| -> Vec<&Note> { lead.notes.iter().filter(|n| (n.start - beat).abs() < 1e-9).collect() };
        // The capo raises everything a semitone.
        assert_eq!((at(0.0)[0].pitch, at(0.0)[0].velocity, at(0.0)[0].string), (65, 80, Some(0)));
        let bent = at(1.0)[0];
        assert_eq!((bent.pitch, bent.length, bent.string, bent.articulation.hammer), (44, 0.5, Some(5), true));
        assert_eq!(bent.articulation.bend, [(0.0, 0.0), (0.5, 2.0), (1.0, 2.0)]);
        let grace = at(1.375)[0];
        assert_eq!((grace.pitch, grace.length), (48, 0.125), "a grace note just before the half note");
        let struck = at(1.5);
        assert_eq!(struck.len(), 2);
        let harmonic = struck.iter().find(|n| n.articulation.harmonic.is_some()).unwrap();
        assert_eq!((harmonic.pitch, harmonic.articulation.harmonic), (63, Some(12)));
        let dead = struck.iter().find(|n| n.articulation.dead).unwrap();
        assert_eq!((dead.pitch, dead.velocity, dead.length), (60, 108, 3.5), "tied into the next bar; an accent");
        assert_eq!((dead.articulation.accent, dead.articulation.vibrato), (1, Some(Vibrato::Slight)));
        assert!(at(3.5).is_empty() && at(4.0).is_empty(), "a rest, then the tie");
        let heavy = at(5.0)[0];
        assert_eq!((heavy.pitch, heavy.length, heavy.velocity), (46, 1.5, 120));
        let a = &heavy.articulation;
        assert_eq!((a.slide, a.palm_mute, a.let_ring, a.staccato, a.accent), (Some(Slide::Legato), true, true, true, 2));
        let ghost = at(6.5)[0];
        assert_eq!((ghost.pitch, ghost.velocity, ghost.articulation.ghost), (57, 58, true));
        assert!((ghost.length - 1.0 / 3.0).abs() < 1e-9);
        let pinch = at(6.5 + 1.0 / 3.0)[0];
        assert_eq!((pinch.pitch, pinch.articulation.harmonic), (84, Some(19)), "an artificial harmonic an octave over the fret");
        assert_eq!(pinch.articulation.bend, [(0.0, 0.0), (1.0, -4.0)], "a tremolo bar dive of two tones");
        assert_eq!(lead.notes.len(), 2 * 8);

        let kit = &score.parts[1];
        assert!(kit.is_drums() && kit.tuning.is_empty());
        let hits: Vec<(f64, u8, f64)> = kit.notes.iter().map(|n| (n.start, n.pitch, n.length)).collect();
        assert_eq!(hits, [(0.0, 36, 1.0), (1.0, 40, 1.0), (2.0, 42, 1.0), (4.0, 36, 4.0), (8.0, 36, 1.0), (9.0, 40, 1.0), (10.0, 42, 1.0), (12.0, 36, 4.0)]);

        let project = score.project();
        assert_eq!(project.name, "Someone – Seven & Up");
        assert_eq!((project.tracks.len(), project.tempo), (2, 100.0));
        assert_eq!(project.tracks[0].tuning, [64, 59, 55, 50, 45, 40]);

        // Archives that are not a score, and scores that are not well formed.
        assert!(parse(&gp7_file("<GPIF><Score/></GPIF>")).unwrap_err().contains("no bars"));
        assert!(parse(&gp7_file("<Other/>")).unwrap_err().contains("GPIF"));
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive.start_file("readme.txt", zip::write::SimpleFileOptions::default()).unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        assert!(parse(&bytes).unwrap_err().contains("score.gpif"));
        assert!(parse(&gp7_file("<GPIF><MasterBars><MasterBar><Time>3/4</Time></MasterBar></MasterBars>")).is_ok(), "an unclosed score is still read");
        assert_eq!(parse(GPIF.as_bytes()).unwrap(), score, "the score alone, out of its archive");

        // The same score in the container of Guitar Pro 6, packed or not.
        let system = crate::gpx::tests::system(&[("score.gpif", GPIF.as_bytes()), ("misc.xml", b"<misc/>")]);
        assert_eq!(parse(&system).unwrap(), score);
        assert_eq!(parse(&crate::gpx::tests::packed(&system)).unwrap(), score);
        let empty = crate::gpx::tests::system(&[("misc.xml", b"<misc/>")]);
        assert!(parse(&crate::gpx::tests::packed(&empty)).unwrap_err().contains("score.gpif"));
    }

    /// Real files of every format read (Guitar Pro and MusicXML), which are
    /// not in the repository. Run on demand:
    /// `OPENGARAGE_SCORES_DIR=<folder> dev test -p og-engine -- --ignored real_scores --nocapture`.
    #[test]
    #[ignore = "needs files of Guitar Pro or MusicXML"]
    fn real_scores_are_read() {
        let Ok(dir) = std::env::var("OPENGARAGE_SCORES_DIR") else { return };
        let mut failed = Vec::new();
        let mut paths: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).filter(|p| is_guitar_pro(p)).collect();
        paths.sort();
        for path in &paths {
            match parse(&std::fs::read(path).unwrap()) {
                Ok(score) => {
                    let notes = score.parts.iter().flat_map(|p| &p.notes);
                    let (tremolos, trills) = notes.fold((0, 0), |(a, b), n| (a + usize::from(n.articulation.tremolo.is_some()), b + usize::from(n.articulation.trill.is_some())));
                    let length = score.parts.first().map_or(0.0, |p| p.length);
                    let feels = score.parts.first().map_or(0, |p| p.swing.len());
                    println!("ok {}: {} parts, {} notes, {length} beats, {tremolos} tremolos, {trills} trills, {feels} feels", path.display(), score.parts.len(), score.parts.iter().map(|p| p.notes.len()).sum::<usize>());
                }
                Err(e) => {
                    println!("FAILED {}: {e}", path.display());
                    failed.push(path.clone());
                }
            }
        }
        println!("{} files, {} not read", paths.len(), failed.len());
        assert!(failed.is_empty());
    }
}
