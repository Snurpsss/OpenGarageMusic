//! Reader for the tablatures of Songsterr, a site of guitar, bass and drum
//! tablatures written by its users. The site's web player fetches each part
//! of a song as a JSON file; this module turns those files into tracks of
//! notes and a project. It knows nothing of the network: the application
//! fetches the texts and hands them here.
//!
//! The format is not documented by the site; it was worked out from the files
//! the public player loads. Fields that are not understood are passed over,
//! and what cannot be played as plain notes (bends, slides, harmonics, let
//! ring) is left out.

use std::collections::HashMap;

use serde_json::Value;

use crate::model::{Articulation, ChordMark, Content, Marker, Note, Point, Project, Region, Slide, TrackKind, Vibrato, TEMPO_MAX, TEMPO_MIN};
use crate::{patches, templates};

/// Songsterr's number for a drum part, where other parts carry a General
/// MIDI program number (0 to 127).
pub const DRUMS: u32 = 1024;

/// Ticks per quarter note in the tempo automation of a part.
const TICKS_PER_QUARTER: f64 = 960.0;

/// One part of a song, read from its file: its notes, in beats (quarter
/// notes) from the start of the song, with repeats unfolded.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub instrument_id: u32,
    pub notes: Vec<Note>,
    /// Length of the song in beats.
    pub length: f64,
    /// Tempo, in quarter notes per minute, by beat; the first point is the
    /// tempo at the start.
    pub tempo: Vec<Point>,
    /// Time signature of the first bar.
    pub signature: (u32, u32),
    /// Key as the project writes it (0 = C … 11 = B) and whether it is minor.
    pub key: Option<(u8, bool)>,
    /// Section names written over the bars.
    pub markers: Vec<Marker>,
    /// Open strings, highest first; empty for drums.
    pub tuning: Vec<u8>,
    /// Chord names written over the beats.
    pub chords: Vec<ChordMark>,
    /// The feels of the part, by beat (see [`crate::model::Swing`]).
    pub swing: Vec<crate::model::Swing>,
    /// Where each bar starts, in beats, in the order they are played (the
    /// repeats unfolded): a recording of the song is timed bar by bar.
    pub bar_starts: Vec<f64>,
    /// The words sung on the notes of the part, if it has some.
    pub lyrics: Vec<crate::lyrics::Syllable>,
}

impl Part {
    pub fn is_drums(&self) -> bool {
        self.instrument_id == DRUMS
    }
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_owned()
}

fn number(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

// ----- one part -----------------------------------------------------------------

/// A bend or a tremolo bar as the tablature writes it: points at a position
/// over the beat (0 to 60) with a height in hundredths of a tone.
fn bend_points(value: &Value) -> Option<Vec<(f32, f32)>> {
    let points: Vec<(f32, f32)> = value
        .get("points")?
        .as_array()?
        .iter()
        .filter_map(|p| Some(((p.get("position")?.as_f64()? / 60.0).clamp(0.0, 1.0) as f32, (p.get("tone")?.as_f64()? / 50.0).clamp(-24.0, 24.0) as f32)))
        .collect();
    (!points.is_empty()).then_some(points)
}

/// Semitones above the open string that a natural harmonic sounds, by the
/// fret where the string is touched.
pub(crate) fn harmonic_interval(fret: i64) -> i64 {
    match fret {
        12 => 12,
        7 | 19 => 19,
        5 | 24 => 24,
        4 | 9 | 16 => 28,
        3 => 31,
        other => other,
    }
}

/// Beats (quarter notes) a beat of the file lasts.
fn beat_length(beat: &Value) -> f64 {
    match beat.get("duration").and_then(Value::as_array) {
        Some(pair) => match (pair.first().and_then(Value::as_f64), pair.get(1).and_then(Value::as_f64)) {
            (Some(num), Some(den)) if den > 0.0 => num / den * 4.0,
            _ => 1.0,
        },
        None => beat.get("type").and_then(Value::as_f64).filter(|t| *t > 0.0).map_or(1.0, |t| 4.0 / t),
    }
}

/// The beats of the first voice of the bars as written, from bar `from` on,
/// as the words are fitted to them: which strike a note (not a rest, a grace
/// note or a note only tied over) and which sound.
fn written_beats(bars: &[Bar], from: usize) -> Vec<crate::lyrics::Beat> {
    let mut out = Vec::new();
    let mut bar_at = 0.0;
    for (index, bar) in bars.iter().enumerate().skip(from) {
        let mut position = 0.0;
        for beat in bar.voices.first().and_then(|v| v.get("beats")).and_then(Value::as_array).into_iter().flatten() {
            if beat.get("graceNote").is_some() {
                continue;
            }
            let length = beat_length(beat);
            let notes = beat.get("notes").and_then(Value::as_array);
            let rest = flag(beat, "rest");
            let sounds = !rest && notes.is_some_and(|notes| notes.iter().any(|n| !flag(n, "rest")));
            let strikes = sounds && notes.is_some_and(|notes| notes.iter().any(|n| !flag(n, "rest") && !flag(n, "tie")));
            if position < bar.length - 1e-6 {
                out.push(crate::lyrics::Beat { bar: index, position, at: bar_at + position, length, strikes, sounds });
            }
            position += length;
        }
        bar_at += bar.length;
    }
    out
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// Loudness names of the tablature, as MIDI velocities.
pub(crate) fn velocity_of(name: &str) -> Option<u8> {
    Some(match name {
        "ppp" => 16,
        "pp" => 32,
        "p" => 48,
        "mp" => 64,
        "mf" => 80,
        "f" => 96,
        "ff" => 112,
        "fff" => 127,
        _ => return None,
    })
}

/// Songsterr's drum numbers as General MIDI percussion notes. Numbers from 35
/// to 81 are already those of General MIDI; the site adds its own above.
pub fn drum_note(id: u32) -> u8 {
    match id {
        35..=81 => id as u8,
        99..=103 => 56,
        104..=107 => 39,
        31 => 37,
        33 => 52,
        82 | 118..=120 | 122 => 69,
        83 | 111..=113 => 54,
        85 => 76,
        86 | 108 | 109 => 64,
        87 => 41,
        88 | 89 | 126 | 127 => 53,
        90 => 59,
        91 => 40,
        92 => 46,
        93 | 94 => 51,
        95 => 55,
        96 => 52,
        97 | 98 | 115 | 116 => 49,
        110 => 63,
        114 => 43,
        117 => 70,
        124 => 60,
        125 => 62,
        // The site's player falls back to the lowest note for what it does not know.
        _ => 35,
    }
}

/// A bar as the file describes it, with what is inherited from earlier bars.
pub(crate) struct Bar<'a> {
    pub(crate) voices: &'a [Value],
    /// Beats (quarter notes) in the bar.
    pub(crate) length: f64,
    pub(crate) signature: (u32, u32),
    pub(crate) repeat_start: bool,
    /// How many times in all the section ending here is played.
    pub(crate) repeat: Option<u32>,
    /// Passes of the repeat this bar is played on; empty for every pass.
    pub(crate) endings: Vec<u32>,
}

/// The order the bars are played in, with repeats and alternate endings
/// unfolded. Bounded, so that a strange file cannot loop for ever.
pub(crate) fn play_order(bars: &[Bar]) -> Vec<usize> {
    let mut order = Vec::new();
    let (mut index, mut start, mut pass, mut jumped) = (0usize, 0usize, 1u32, false);
    while index < bars.len() && order.len() < 4096 {
        let bar = &bars[index];
        // A section starts here, unless this is a jump back to its start.
        if bar.repeat_start && !jumped {
            start = index;
            pass = 1;
        }
        jumped = false;
        if !bar.endings.is_empty() && !bar.endings.contains(&pass) {
            index += 1;
            continue;
        }
        order.push(index);
        if let Some(count) = bar.repeat {
            if pass < count {
                pass += 1;
                index = start;
                jumped = true;
                continue;
            }
        }
        index += 1;
    }
    order
}

/// Signature as the project writes it: so many beats over 4 or over 8.
fn project_signature((num, den): (u32, u32)) -> (u32, u32) {
    let (beats, unit) = match den {
        1 => (num * 4, 4),
        2 => (num * 2, 4),
        8 => (num, 8),
        16 => (num.div_ceil(2), 8),
        _ => (num, 4),
    };
    (beats.clamp(1, 16), unit)
}

/// The key named by a count of sharps (positive) or flats (negative).
pub(crate) fn key_of(accidentals: i64, minor: bool) -> (u8, bool) {
    let major = (accidentals * 7).rem_euclid(12) as u8;
    if minor {
        ((major + 9) % 12, true)
    } else {
        (major, false)
    }
}

/// Reads the file of one part.
pub fn parse_part(json: &str) -> Result<Part, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("unreadable part ({e})"))?;
    let measures = value.get("measures").and_then(Value::as_array).ok_or("the part has no bars")?;
    let instrument_id = number(&value, "instrumentId").map_or(0, |id| id.min(u64::from(u32::MAX)) as u32);
    let drums = instrument_id == DRUMS;
    let tuning: Vec<i64> = value.get("tuning").and_then(Value::as_array).map(|list| list.iter().filter_map(Value::as_i64).collect()).unwrap_or_default();
    let capo = value.get("capo").and_then(Value::as_i64).unwrap_or(0);
    if !drums && tuning.is_empty() {
        return Err("the part does not say how its strings are tuned".to_owned());
    }

    // The bars, with the signature carried from one to the next.
    let mut signature = (4u32, 4u32);
    let mut key = None;
    let mut bars = Vec::with_capacity(measures.len());
    let mut names: Vec<Option<String>> = Vec::with_capacity(measures.len());
    for measure in measures {
        if let Some(pair) = measure.get("signature").and_then(Value::as_array) {
            if let (Some(num), Some(den)) = (pair.first().and_then(Value::as_u64), pair.get(1).and_then(Value::as_u64)) {
                if (1..=32).contains(&num) && [1, 2, 4, 8, 16, 32].contains(&den) {
                    signature = (num as u32, den as u32);
                }
            }
        }
        if key.is_none() {
            if let Some(sig) = measure.get("keySignature") {
                if let Some(count) = sig.get("accidentalCount").and_then(Value::as_i64) {
                    key = Some(key_of(count, sig.get("mode").and_then(Value::as_str) == Some("minor")));
                }
            }
        }
        let endings = measure
            .get("alternateEnding")
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Value::as_u64).map(|n| n as u32).collect())
            .unwrap_or_default();
        bars.push(Bar {
            voices: measure.get("voices").and_then(Value::as_array).map_or(&[][..], Vec::as_slice),
            length: f64::from(signature.0) * 4.0 / f64::from(signature.1),
            signature,
            repeat_start: measure.get("repeatStart").and_then(Value::as_bool).unwrap_or(false),
            repeat: measure.get("repeat").and_then(Value::as_u64).map(|n| (n as u32).clamp(2, 64)),
            endings,
        });
        names.push(measure.get("marker").and_then(|m| m.get("text")).and_then(Value::as_str).map(str::to_owned));
    }

    // Where each bar starts the first time it is played, and the order of play.
    let order = play_order(&bars);
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

    // The notes. A tie lengthens the note before it on the same string; the
    // loudness holds until the tablature changes it.
    let mut notes: Vec<Note> = Vec::new();
    let mut chords: Vec<ChordMark> = Vec::new();
    let mut loudness: HashMap<usize, u8> = HashMap::new();
    let mut open: HashMap<(usize, String), usize> = HashMap::new();
    for (&index, &bar_start) in order.iter().zip(&starts) {
        let bar = &bars[index];
        for (voice_index, voice) in bar.voices.iter().enumerate() {
            let Some(beats) = voice.get("beats").and_then(Value::as_array) else { continue };
            let mut position = 0.0;
            for beat in beats {
                let duration = beat_length(beat);
                let grace = beat.get("graceNote").is_some();
                let sounding = if grace { duration.min(0.125) } else { duration };
                if let Some(name) = beat.get("velocity").and_then(Value::as_str).and_then(velocity_of) {
                    loudness.insert(voice_index, name);
                }
                let base = loudness.get(&voice_index).copied().unwrap_or(96);
                let start = bar_start + position;
                if !grace {
                    position += duration;
                }
                if let Some(name) = beat.get("chord").and_then(|c| c.get("text")).and_then(Value::as_str).filter(|n| !n.trim().is_empty()) {
                    if !chords.iter().any(|c| (c.beat - start).abs() < 1e-6) {
                        chords.push(ChordMark { beat: start, name: name.trim().to_owned() });
                    }
                }
                if beat.get("rest").and_then(Value::as_bool).unwrap_or(false) || start >= bar_start + bar.length + 1e-6 {
                    continue;
                }
                let Some(struck) = beat.get("notes").and_then(Value::as_array) else { continue };
                let whole_beat = Articulation {
                    bend: beat.get("tremoloBar").and_then(bend_points).unwrap_or_default(),
                    let_ring: flag(beat, "letRing"),
                    palm_mute: flag(beat, "palmMute"),
                    ..Articulation::default()
                };
                for note in struck {
                    if flag(note, "rest") {
                        continue;
                    }
                    let string = note.get("string").map(Value::to_string).unwrap_or_default();
                    let on_string = if drums { None } else { note.get("string").and_then(Value::as_u64).map(|s| s.min(255) as u8) };
                    if note.get("tie").and_then(Value::as_bool).unwrap_or(false) {
                        if let Some(&earlier) = open.get(&(voice_index, string.clone())) {
                            let held = &mut notes[earlier];
                            held.length = (start + sounding - held.start).max(held.length);
                        }
                        continue;
                    }
                    let Some(fret) = note.get("fret").and_then(Value::as_i64) else { continue };
                    let mut articulation = whole_beat.clone();
                    articulation.slide = note.get("slide").and_then(Value::as_str).and_then(|kind| match kind {
                        "legato" => Some(Slide::Legato),
                        "shift" => Some(Slide::Shift),
                        "below" => Some(Slide::FromBelow),
                        "above" => Some(Slide::FromAbove),
                        "downwards" => Some(Slide::OutDown),
                        "upwards" => Some(Slide::OutUp),
                        _ => None,
                    });
                    if let Some(points) = note.get("bend").and_then(bend_points) {
                        articulation.bend = points;
                    }
                    articulation.hammer = flag(note, "hp");
                    articulation.vibrato = note.get("leftHandVibrato").and_then(Value::as_str).map(|kind| if kind == "wide" { Vibrato::Wide } else { Vibrato::Slight });
                    articulation.dead = flag(note, "dead");
                    articulation.ghost = flag(note, "ghost");
                    articulation.staccato = flag(note, "staccato");
                    articulation.accent = note.get("accentuated").and_then(Value::as_u64).unwrap_or(0).min(2) as u8;
                    // A harmonic sounds higher than the fret it is touched at.
                    let harmonic = note.get("harmonic").and_then(Value::as_str).map(|kind| {
                        let touched = note.get("harmonicFret").and_then(Value::as_i64).unwrap_or(if kind == "natural" { fret } else { fret + 12 });
                        let above_open = if kind == "natural" { harmonic_interval(touched) } else { fret + 12 };
                        (touched, above_open)
                    });
                    if let Some((touched, _)) = harmonic {
                        articulation.harmonic = Some(touched.clamp(0, 127) as u8);
                    }
                    let pitch = if drums {
                        i64::from(drum_note(fret.clamp(0, i64::from(u32::MAX)) as u32))
                    } else {
                        let Some(open_string) = note.get("string").and_then(Value::as_u64).and_then(|s| tuning.get(s as usize)) else { continue };
                        open_string + harmonic.map_or(fret, |(_, above_open)| above_open) + capo
                    };
                    if !(0..=127).contains(&pitch) {
                        continue;
                    }
                    let mut velocity = f64::from(base);
                    match note.get("accentuated").and_then(Value::as_u64) {
                        Some(1) => velocity += 12.0,
                        Some(2) => velocity += 24.0,
                        _ => {}
                    }
                    if note.get("ghost").and_then(Value::as_bool).unwrap_or(false) {
                        velocity *= 0.6;
                    }
                    open.insert((voice_index, string), notes.len());
                    notes.push(Note {
                        start,
                        length: sounding.max(0.0625),
                        pitch: pitch as u8,
                        velocity: velocity.round().clamp(1.0, 127.0) as u8,
                        string: on_string,
                        articulation: if drums { Articulation::default() } else { articulation },
                    });
                }
            }
        }
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start).then(a.pitch.cmp(&b.pitch)));
    chords.sort_by(|a, b| a.beat.total_cmp(&b.beat));

    // The words: their syllables on the struck beats of the bars as written,
    // from the bar each line starts at; a bar played again sings them again.
    let mut lyrics: Vec<crate::lyrics::Syllable> = Vec::new();
    for line in value.get("newLyrics").and_then(Value::as_array).into_iter().flatten() {
        let text = line.get("text").and_then(Value::as_str).unwrap_or("");
        if text.trim().is_empty() {
            continue;
        }
        let from = (number(line, "offset").unwrap_or(1).max(1) - 1) as usize;
        let slots = crate::lyrics::slots(&written_beats(&bars, from));
        crate::lyrics::place(text, &slots, &order, &starts, &mut lyrics);
    }
    lyrics.sort_by(|a, b| a.beat.total_cmp(&b.beat));
    crate::lyrics::mark_lines(&mut lyrics, bars.first().map_or(4.0, |b| b.length));

    // The tempo, in quarter notes per minute whatever the beat it is written for.
    let mut tempo = Vec::new();
    if let Some(points) = value.get("automations").and_then(|a| a.get("tempo")).and_then(Value::as_array) {
        for point in points {
            let (Some(measure), Some(bpm)) = (number(point, "measure"), point.get("bpm").and_then(Value::as_f64)) else { continue };
            let Some(Some(bar_start)) = first_start.get(measure as usize) else { continue };
            let unit = point.get("type").and_then(Value::as_f64).filter(|t| *t > 0.0).unwrap_or(4.0);
            let dotted = if point.get("dotted").and_then(Value::as_bool).unwrap_or(false) { 1.5 } else { 1.0 };
            let beat = bar_start + point.get("position").and_then(Value::as_f64).unwrap_or(0.0) / TICKS_PER_QUARTER;
            let value = bpm * 4.0 / unit * dotted;
            if value.is_finite() && value > 0.0 {
                tempo.push(Point { beat, value });
            }
        }
    }
    tempo.sort_by(|a, b| a.beat.total_cmp(&b.beat));

    // The section names, each reaching to the next.
    let mut markers: Vec<Marker> = Vec::new();
    for (index, name) in names.iter().enumerate() {
        if let (Some(name), Some(Some(start))) = (name, first_start.get(index)) {
            if let Some(last) = markers.last_mut() {
                last.length = (start - last.start).max(bars[index].length);
            }
            markers.push(Marker { name: name.clone(), start: *start, length: bars[index].length });
        }
    }
    if let Some(last) = markers.last_mut() {
        last.length = (length - last.start).max(last.length);
    }

    Ok(Part {
        name: text(&value, "name"),
        instrument_id,
        notes,
        length,
        tempo,
        signature: bars.first().map_or((4, 4), |bar| bar.signature),
        key,
        markers,
        tuning: if drums { Vec::new() } else { tuning.iter().map(|t| t.clamp(&0, &127)).map(|t| *t as u8).collect() },
        chords,
        swing: Vec::new(),
        bar_starts: starts,
        lyrics,
    })
}

// ----- the project -----------------------------------------------------------------

/// The built-in sound that stands for a General MIDI program.
pub fn patch_for(instrument_id: u32) -> &'static str {
    match instrument_id {
        DRUMS => patches::DEFAULT_DRUMS,
        0..=7 | 120..=127 => patches::DEFAULT_PATCH,
        10 => "Music box",
        12 | 13 | 108 | 112..=119 => "Marimba",
        8..=15 => "Bells",
        19 => "Church organ",
        16..=23 => "Drawbar organ",
        24 => "Nylon guitar",
        28 => "Muted guitar",
        29..=31 => "Classic lead",
        25..=27 | 104..=107 | 109..=111 => "Steel guitar",
        36 | 37 => "Slap bass",
        38 | 39 => "Synth bass",
        32..=35 => "Electric bass",
        40..=55 => "String ensemble",
        56..=71 => "Brass",
        72..=79 => "Flute",
        80 => "Square lead",
        81..=87 => "Classic lead",
        88..=95 => "Warm pad",
        96..=103 => "Glass pad",
        _ => patches::DEFAULT_PATCH,
    }
}

/// The pictogram of a track for a General MIDI program: drum, guitar (and
/// bass), note (strings, brass, winds) or keyboard (pianos, organs, synths).
pub fn icon_for(instrument_id: u32) -> u8 {
    match instrument_id {
        DRUMS => 1,
        24..=39 => 3,
        40..=79 => 5,
        _ => 0,
    }
}

/// "Artist – Title", or just the title.
pub fn song_label(artist: &str, title: &str) -> String {
    if artist.trim().is_empty() {
        title.to_owned()
    } else {
        format!("{} – {}", artist.trim(), title.trim())
    }
}

/// A new project holding the parts of a song, one track each.
pub fn build_project(artist: &str, title: &str, parts: &[Part]) -> Project {
    let mut project = Project::empty();
    let label = song_label(artist, title);
    if !label.trim().is_empty() {
        project.name = label;
    }
    project.artist = artist.trim().to_owned();
    project.title = title.trim().to_owned();
    if let Some(first) = parts.first() {
        let (beats, unit) = project_signature(first.signature);
        project.beats_per_bar = beats;
        project.beat_unit = unit;
        if let Some((key, minor)) = first.key {
            project.key = key;
            project.minor = minor;
        }
        project.markers = first.markers.clone();
    }
    // The tempo comes from the part that has one; a tempo that changes is a
    // tempo track.
    if let Some(tempo) = parts.iter().map(|p| &p.tempo).find(|t| !t.is_empty()) {
        project.tempo = tempo[0].value.clamp(TEMPO_MIN, TEMPO_MAX).round();
        if tempo.iter().any(|p| (p.value - tempo[0].value).abs() > 0.5) {
            project.tempo_points = tempo.iter().map(|p| Point { beat: p.beat, value: p.value.clamp(TEMPO_MIN, TEMPO_MAX).round() }).collect();
        }
    }
    let bar = project.bar_len();
    for (index, part) in parts.iter().enumerate() {
        let name = if part.name.trim().is_empty() { format!("Part {}", index + 1) } else { part.name.trim().to_owned() };
        let mut track = templates::track(&name, TrackKind::Instrument, patch_for(part.instrument_id), (index % 8) as u8);
        track.icon = icon_for(part.instrument_id);
        let end = part.notes.iter().map(|n| n.start + n.length).fold(part.length, f64::max);
        let mut region = Region::new(name, 0.0, ((end / bar).ceil() * bar).max(bar));
        region.content = Content::Midi(part.notes.clone());
        region.chords.clone_from(&part.chords);
        region.swing.clone_from(&part.swing);
        track.tuning.clone_from(&part.tuning);
        track.regions.push(region);
        project.tracks.push(track);
    }
    project
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two bars of 6/8 then one of 4/4, a repeat with two endings, a tie, a
    /// triplet, a rest, a ghost note, a dead note, a loudness that holds.
    const GUITAR: &str = r##"{"name":"Lead Guitar","instrumentId":27,"instrument":"Electric Guitar (clean)","tuning":[64,59,55,50,45,40],
        "strings":6,"frets":24,"partId":3,"songId":1,"revisionId":2,"version":8,
        "automations":{"tempo":[{"measure":0,"position":0,"bpm":71,"type":4},{"measure":2,"position":960,"bpm":60,"type":8,"dotted":true}]},
        "measures":[
          {"signature":[6,8],"marker":{"text":"Intro","width":38},"keySignature":{"accidentalCount":1,"mode":"minor","transposeAs":"#"},"repeatStart":true,
           "voices":[{"beats":[
             {"notes":[{"fret":0,"string":5}],"velocity":"mf","type":8,"duration":[1,8],"chord":{"text":"Em","width":21}},
             {"notes":[{"fret":0,"string":2,"slide":"shift"}],"type":8,"duration":[1,8]},
             {"notes":[{"fret":0,"string":1,"accentuated":1,"leftHandVibrato":"wide"}],"type":8,"duration":[1,8],"letRing":true},
             {"notes":[{"rest":true}],"rest":true,"type":8,"duration":[1,8]},
             {"notes":[{"fret":3,"string":5,"ghost":true},{"fret":5,"string":4,"dead":true}],"type":8,"duration":[1,8]},
             {"notes":[{"fret":7,"string":0,"bend":{"tone":100,"points":[{"position":0,"tone":0},{"position":30,"tone":100},{"position":60,"tone":100}]}}],"type":8,"duration":[1,8]}]}]},
          {"alternateEnding":[1],"repeat":2,"voices":[{"beats":[
             {"notes":[{"fret":7,"string":0,"tie":true}],"type":4,"dots":1,"duration":[3,8]},
             {"notes":[{"fret":5,"string":0,"hp":true}],"type":16,"tuplet":3,"duration":[1,24],"tupletStart":true,"palmMute":true},
             {"notes":[{"fret":3,"string":0,"staccato":true}],"type":16,"tuplet":3,"duration":[1,24],"palmMute":true},
             {"notes":[{"fret":2,"string":0}],"type":16,"tuplet":3,"duration":[1,24],"tupletStop":true},
             {"notes":[{"rest":true}],"rest":true,"type":4,"duration":[1,4]}]}]},
          {"alternateEnding":[2],"signature":[4,4],"marker":{"text":"Verse","width":40},"voices":[{"beats":[
             {"notes":[{"fret":12,"string":0,"harmonic":"natural"}],"velocity":"ff","type":1,"duration":[1,1]}]}]}
        ]}"##;

    const DRUM_PART: &str = r#"{"name":"Drums","instrumentId":1024,"instrument":"Drums","strings":6,"tuningFlat":true,
        "automations":{"tempo":[]},
        "measures":[{"voices":[{"beats":[
             {"notes":[{"fret":36,"string":1.5},{"fret":92,"string":-1}],"type":4,"duration":[1,4]},
             {"notes":[{"fret":38,"string":0.5}],"type":4,"duration":[1,4]},
             {"notes":[{"fret":999,"string":0.5}],"type":2,"duration":[1,2]}]}]}]}"#;

    #[test]
    fn a_guitar_part_becomes_notes_in_beats() {
        let part = parse_part(GUITAR).unwrap();
        assert_eq!(part.name, "Lead Guitar");
        assert_eq!(part.signature, (6, 8));
        assert_eq!(part.key, Some((4, true)), "one sharp, minor: E minor");
        // Bars: intro (3 beats), ending 1 (3 beats), intro again, ending 2 (4 beats).
        assert_eq!(part.length, 13.0);
        assert_eq!(part.markers.len(), 2);
        assert_eq!((part.markers[0].name.as_str(), part.markers[0].start, part.markers[0].length), ("Intro", 0.0, 9.0));
        assert_eq!((part.markers[1].name.as_str(), part.markers[1].start, part.markers[1].length), ("Verse", 9.0, 4.0));
        // The tempo in quarter notes: a dotted eighth at 60 is 45 quarters a minute.
        assert_eq!(part.tempo.len(), 2);
        assert_eq!((part.tempo[0].beat, part.tempo[0].value), (0.0, 71.0));
        assert_eq!((part.tempo[1].beat, part.tempo[1].value), (10.0, 45.0));

        assert_eq!(part.tuning, [64, 59, 55, 50, 45, 40]);
        assert_eq!(part.chords, [ChordMark { beat: 0.0, name: "Em".to_owned() }, ChordMark { beat: 6.0, name: "Em".to_owned() }], "once per pass of the repeat");
        let at = |beat: f64| -> Vec<&Note> { part.notes.iter().filter(|n| (n.start - beat).abs() < 1e-9).collect() };
        // Open low E, then open G, then an accented open B; each on its string.
        assert_eq!((at(0.0)[0].pitch, at(0.0)[0].velocity, at(0.0)[0].string), (40, 80, Some(5)));
        assert_eq!(at(0.5)[0].string, Some(2));
        assert_eq!(at(0.5)[0].pitch, 55);
        assert_eq!((at(1.0)[0].pitch, at(1.0)[0].velocity), (59, 92), "accent, loudness held from the first beat");
        assert!(at(1.5).is_empty(), "a rest");
        let struck = at(2.0);
        assert_eq!(struck.len(), 2, "the dead note is kept, for the tablature");
        let ghost = struck.iter().find(|n| n.articulation.ghost).unwrap();
        assert_eq!((ghost.pitch, ghost.velocity), (43, 48));
        let dead = struck.iter().find(|n| n.articulation.dead).unwrap();
        assert_eq!((dead.pitch, dead.string), (50, Some(4)));
        // The high E at fret 7 is tied into the next bar: an eighth plus a dotted quarter.
        let held = at(2.5);
        assert_eq!(held[0].pitch, 71);
        assert!((held[0].length - 2.0).abs() < 1e-9, "{}", held[0].length);
        // The triplet of sixteenths, a sixth of a beat each.
        let triplet: Vec<&Note> = part.notes.iter().filter(|n| n.start > 4.49 && n.start < 5.0).collect();
        assert_eq!(triplet.iter().map(|n| n.pitch).collect::<Vec<_>>(), [69, 67, 66]);
        assert!((triplet[1].start - (4.5 + 1.0 / 6.0)).abs() < 1e-9);
        // Second pass of the repeat: the intro again from beat 6, then the second ending.
        assert_eq!(at(6.0)[0].pitch, 40);
        assert!(at(9.0 + 0.5).is_empty(), "the first ending is not played again");
        let last = at(9.0);
        assert_eq!((last[0].pitch, last[0].velocity, last[0].length), (76, 112, 4.0));
        assert_eq!(last[0].articulation.harmonic, Some(12), "a natural harmonic at the twelfth fret: the octave");
        assert!(part.notes.windows(2).all(|w| w[0].start <= w[1].start), "sorted by time");

        // What the tablature writes around the notes.
        assert_eq!(at(0.0)[0].articulation, Articulation::default(), "a plain note");
        assert_eq!(at(0.5)[0].articulation.slide, Some(Slide::Shift));
        let b = &at(1.0)[0].articulation;
        assert_eq!((b.accent, b.vibrato, b.let_ring), (1, Some(Vibrato::Wide), true));
        assert_eq!(held[0].articulation.bend, [(0.0, 0.0), (0.5, 2.0), (1.0, 2.0)], "a full bend: two semitones");
        assert_eq!((triplet[0].articulation.hammer, triplet[0].articulation.palm_mute), (true, true));
        assert_eq!((triplet[1].articulation.staccato, triplet[1].articulation.palm_mute), (true, true));
        assert!(!triplet[2].articulation.palm_mute);
    }

    #[test]
    fn harmonics_sound_above_the_fret_they_are_touched_at() {
        let part = |note: &str| {
            let json = format!(r##"{{"name":"g","instrumentId":27,"tuning":[64,59,55,50,45,40],"measures":[{{"voices":[{{"beats":[{{"notes":[{note}],"type":4,"duration":[1,4]}}]}}]}}]}}"##);
            parse_part(&json).unwrap().notes
        };
        assert_eq!(part(r#"{"fret":7,"string":5,"harmonic":"natural"}"#)[0].pitch, 40 + 19);
        assert_eq!(part(r#"{"fret":5,"string":5,"harmonic":"natural","harmonicFret":5}"#)[0].pitch, 40 + 24);
        let artificial = &part(r#"{"fret":2,"string":5,"harmonic":"artificial","harmonicFret":14}"#)[0];
        assert_eq!((artificial.pitch, artificial.articulation.harmonic), (40 + 14, Some(14)));
        let tremolo = &part(r#"{"fret":2,"string":5}"#);
        assert!(tremolo[0].articulation.is_plain());
        let json = r##"{"name":"g","instrumentId":27,"tuning":[64,59,55,50,45,40],"measures":[{"voices":[{"beats":[{"notes":[{"fret":2,"string":5},{"fret":2,"string":4}],"type":4,"duration":[1,4],"tremoloBar":{"points":[{"position":0,"tone":0},{"position":60,"tone":-350}]}}]}]}]}"##;
        let dive = parse_part(json).unwrap().notes;
        assert_eq!(dive.len(), 2);
        assert!(dive.iter().all(|n| n.articulation.bend == [(0.0, 0.0), (1.0, -7.0)]), "a dive of three tones and a half, on every note of the beat");
    }

    #[test]
    fn a_drum_part_uses_general_midi_percussion() {
        let part = parse_part(DRUM_PART).unwrap();
        assert!(part.is_drums());
        assert_eq!(part.signature, (4, 4));
        assert_eq!(part.length, 4.0);
        assert!(part.tempo.is_empty());
        assert!(part.tuning.is_empty() && part.chords.is_empty());
        assert!(part.notes.iter().all(|n| n.string.is_none()), "drums have no strings");
        let pitches: Vec<(f64, u8)> = part.notes.iter().map(|n| (n.start, n.pitch)).collect();
        assert_eq!(pitches, [(0.0, 36), (0.0, 46), (1.0, 38), (2.0, 35)], "the half-open hi-hat becomes an open one; an unknown number, a kick");
        assert_eq!(drum_note(57), 57);
        assert_eq!(drum_note(102), 56);
        assert!(parse_part(r#"{"name":"x","instrumentId":27,"measures":[]}"#).is_err(), "a guitar without a tuning");
        assert!(parse_part(r#"{"name":"x"}"#).is_err() && parse_part("nonsense").is_err());
    }

    #[test]
    fn the_words_of_the_singing_part_go_on_its_struck_notes() {
        let json = r#"{"name":"Chant","instrumentId":53,"tuning":[64,59,55,50,45,40],"measures":[
            {"signature":[4,4],"voices":[{"beats":[{"type":4,"notes":[{"string":0,"fret":0}]},{"type":4,"notes":[{"string":0,"fret":2}]},
                {"type":4,"rest":true,"notes":[{"rest":true}]},{"type":4,"notes":[{"string":0,"fret":3}]}]}]},
            {"voices":[{"beats":[{"type":2,"notes":[{"string":0,"fret":3,"tie":true}]},{"type":2,"notes":[{"string":0,"fret":5}]}]}]}],
            "newLyrics":[{"line":1,"offset":1,"text":"Hel- lo  world"},{"line":2,"offset":1,"text":""}]}"#;
        let part = parse_part(json).unwrap();
        let words: Vec<(f64, &str)> = part.lyrics.iter().map(|s| (s.beat, s.text.as_str())).collect();
        // The rest and the tied note take no syllable; two spaces leave a note without one.
        assert_eq!(words, [(0.0, "Hel-"), (1.0, "lo"), (6.0, "world")]);
        assert_eq!(crate::lyrics::join(&part.lyrics), "Hello world");
    }

    #[test]
    fn repeats_are_unfolded_within_bounds() {
        let bar = |repeat_start: bool, repeat: Option<u32>, endings: Vec<u32>| Bar { voices: &[], length: 4.0, signature: (4, 4), repeat_start, repeat, endings };
        // |: A B :|x3 C
        let bars = [bar(true, None, vec![]), bar(false, Some(3), vec![]), bar(false, None, vec![])];
        assert_eq!(play_order(&bars), [0, 1, 0, 1, 0, 1, 2]);
        // A repeat sign without a start goes back to the beginning.
        let bars = [bar(false, None, vec![]), bar(false, Some(2), vec![])];
        assert_eq!(play_order(&bars), [0, 1, 0, 1]);
        // A file whose repeats never end is cut short.
        let bars = [bar(true, Some(64), vec![]), bar(true, Some(64), vec![])];
        assert!(play_order(&bars).len() <= 4096);
    }

    #[test]
    fn a_project_is_built_from_the_parts() {
        let guitar = parse_part(GUITAR).unwrap();
        let drums = parse_part(DRUM_PART).unwrap();
        let project = build_project("Metallica", "Nothing Else Matters", &[guitar.clone(), drums.clone()]);
        assert_eq!(project.name, "Metallica – Nothing Else Matters");
        assert_eq!((project.beats_per_bar, project.beat_unit), (6, 8));
        assert_eq!((project.key, project.minor), (4, true));
        assert_eq!(project.tempo, 71.0);
        assert_eq!(project.tempo_points.len(), 2, "a tempo that changes becomes a tempo track");
        assert_eq!(project.markers.len(), 2);
        assert_eq!(project.tracks.len(), 2);
        assert_eq!(project.tracks[0].patch, "Steel guitar");
        assert_eq!(project.tracks[1].patch, patches::DEFAULT_DRUMS);
        assert_eq!((project.tracks[0].icon, project.tracks[1].icon), (3, 1), "a guitar and a drum");
        assert_eq!((icon_for(0), icon_for(33), icon_for(48), icon_for(67), icon_for(80)), (0, 3, 5, 5, 0));
        assert!(project.tracks.iter().all(|t| t.kind == TrackKind::Instrument));
        let region = &project.tracks[0].regions[0];
        assert_eq!(region.name, "Lead Guitar");
        assert_eq!(region.chords.len(), 2);
        assert_eq!(project.tracks[0].tuning, [64, 59, 55, 50, 45, 40]);
        assert!(project.tracks[1].tuning.is_empty());
        assert_eq!(region.length, 15.0, "whole bars of 6/8: 13 beats fit in 15");
        assert!(matches!(&region.content, Content::Midi(notes) if notes.len() == guitar.notes.len()));
        // The drum part, shorter, still fills whole bars of the project.
        assert_eq!(project.tracks[1].regions[0].length, 6.0);
        // No tempo anywhere: the project keeps its own.
        let only_drums = build_project("", "Beat", &[drums]);
        assert_eq!((only_drums.tempo, only_drums.name.as_str(), only_drums.tempo_points.len()), (120.0, "Beat", 0));
        assert_eq!((only_drums.beats_per_bar, only_drums.beat_unit), (4, 4));
        // The sounds that stand for the General MIDI programs.
        assert_eq!(patch_for(0), patches::DEFAULT_PATCH);
        assert_eq!(patch_for(33), "Electric bass");
        assert_eq!(patch_for(36), "Slap bass");
        assert_eq!(patch_for(48), "String ensemble");
        assert_eq!(patch_for(DRUMS), patches::DEFAULT_DRUMS);
        for id in 0..128 {
            assert!(patches::find(patch_for(id)).is_some(), "program {id} has a sound");
        }
        // Signatures the project cannot write as they are.
        assert_eq!(project_signature((2, 2)), (4, 4));
        assert_eq!(project_signature((7, 16)), (4, 8));
        assert_eq!(project_signature((3, 1)), (12, 4));
        assert_eq!(project_signature((40, 4)), (16, 4));
        assert_eq!(key_of(0, false), (0, false));
        assert_eq!(key_of(-3, false), (3, false), "three flats: E flat major");
        assert_eq!(key_of(-3, true), (0, true), "three flats, minor: C minor");
    }

    /// Reads real files of the site, kept outside the repository. Run on demand:
    /// `OPENGARAGE_SONGSTERR_DIR=<folder of N.json files> dev test -p og-engine -- --ignored real_parts --nocapture`
    #[test]
    #[ignore = "needs files of the site"]
    fn real_parts_are_read() {
        let Ok(dir) = std::env::var("OPENGARAGE_SONGSTERR_DIR") else { return };
        let mut parts = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let part = parse_part(&std::fs::read_to_string(&path).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                println!(
                    "{}: {} notes over {} beats in {:?}, tempo {:?}, {} markers, key {:?}",
                    part.name,
                    part.notes.len(),
                    part.length,
                    part.signature,
                    part.tempo.first().map(|p| p.value),
                    part.markers.len(),
                    part.key
                );
                parts.push(part);
            }
        }
        let project = build_project("", "Real", &parts);
        assert_eq!(project.tracks.len(), parts.len());
        assert!(project.tracks.iter().any(|t| matches!(&t.regions[0].content, Content::Midi(n) if !n.is_empty())));
        println!("project: {} bpm, {}/{}, {} markers", project.tempo, project.beats_per_bar, project.beat_unit, project.markers.len());
    }
}




