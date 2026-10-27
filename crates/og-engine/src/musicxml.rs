//! Reader for MusicXML, the open format most score editors exchange
//! (<https://www.w3.org/2021/06/musicxml40/>): `.musicxml` and `.xml` files,
//! partwise or timewise, and the compressed `.mxl` archives.
//!
//! The song is read into the same bars, tracks and beats as a Guitar Pro
//! file, so that repeats, ties and the project come out the same way. A part
//! written as tablature (a TAB clef, or strings and frets on its notes) keeps
//! its strings and tuning; the other parts are read by pitch, each pitch
//! standing for a string of its own while the song is read, and lose the
//! strings once it is. Where a part has a staff of notes and a staff of
//! tablature with the same notes, as Guitar Pro writes it, only the
//! tablature is read.
//!
//! What a project cannot play (layout, lyrics, fingerings, most directions)
//! is passed over. Every read is bounded: a damaged file gives an error,
//! never a crash.

use super::gpif::{number, parse_xml, Element};
use super::*;

/// The largest score read, in bytes, once unpacked.
const MAX_BYTES: u64 = 1 << 26;
const MAX_BARS: usize = 8192;
/// Notes struck together on one beat, at most.
const MAX_CHORD: usize = 64;
/// The tempo when the score writes none.
const DEFAULT_TEMPO: f64 = 120.0;

/// Whether these bytes are a MusicXML score (an archive is told apart by
/// what it holds, see [`is_archive`]).
pub(super) fn is_score(bytes: &[u8]) -> bool {
    let head = text_of_bytes(&bytes[..bytes.len().min(4096)]);
    head.trim_start().starts_with('<') && (head.contains("<score-partwise") || head.contains("<score-timewise") || head.contains("MusicXML"))
}

/// Whether a ZIP archive is a compressed MusicXML file: it has the
/// `META-INF/container.xml` of one.
pub(super) fn is_archive(bytes: &[u8]) -> bool {
    zip::ZipArchive::new(std::io::Cursor::new(bytes)).is_ok_and(|archive| archive.index_for_name("META-INF/container.xml").is_some())
}

/// Reads a MusicXML score, plain or in its archive.
pub(super) fn parse(bytes: &[u8]) -> Result<Score, String> {
    let text = if bytes.starts_with(b"PK\x03\x04") { score_in_archive(bytes)? } else { text_of_bytes(bytes) };
    let (song, tabbed) = read(&text)?;
    let mut score = song.score();
    // The parts read by pitch had a string a pitch while the song was read.
    for (part, tabbed) in score.parts.iter_mut().zip(tabbed) {
        if !tabbed {
            part.tuning.clear();
            for note in &mut part.notes {
                note.string = None;
            }
        }
    }
    Ok(score)
}

/// The text of a file: UTF-8, or UTF-16 when it starts with the mark of it,
/// or Latin-1 when it is not valid UTF-8.
fn text_of_bytes(bytes: &[u8]) -> String {
    let utf16 = |bytes: &[u8], little: bool| -> String {
        let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|p| if little { u16::from_le_bytes(*p) } else { u16::from_be_bytes(*p) }).collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return utf16(rest, true);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return utf16(rest, false);
    }
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        // Cut in the middle of a character (the head of a file): the rest is fine.
        Err(e) if e.error_len().is_none() => String::from_utf8_lossy(bytes).into_owned(),
        Err(_) => bytes.iter().map(|&b| char::from(b)).collect(),
    }
}

/// The score of a `.mxl` archive: the file its container names, or else
/// the first score outside `META-INF`.
fn score_in_archive(bytes: &[u8]) -> Result<String, String> {
    use std::io::Read;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| format!("unreadable MusicXML archive ({e})"))?;
    let mut read = |name: &str| -> Option<Vec<u8>> {
        let file = archive.by_name(name).ok()?;
        let mut out = Vec::new();
        file.take(MAX_BYTES).read_to_end(&mut out).ok()?;
        Some(out)
    };
    let named = read("META-INF/container.xml").and_then(|container| {
        let document = parse_xml(&text_of_bytes(&container)).ok()?;
        let path = document.child("container")?.child("rootfiles")?.children("rootfile").find_map(|r| r.attr("full-path").map(str::to_owned))?;
        read(&path)
    });
    if let Some(score) = named {
        return Ok(text_of_bytes(&score));
    }
    let names: Vec<String> = archive.file_names().filter(|n| !n.starts_with("META-INF/") && (n.ends_with(".xml") || n.ends_with(".musicxml"))).map(str::to_owned).collect();
    for name in names {
        let Ok(file) = archive.by_name(&name) else { continue };
        let mut out = Vec::new();
        if file.take(MAX_BYTES).read_to_end(&mut out).is_ok() {
            return Ok(text_of_bytes(&out));
        }
    }
    Err("the archive holds no MusicXML score".to_owned())
}

/// What the list of parts says of one part.
#[derive(Default)]
struct PartInfo {
    name: String,
    program: Option<u32>,
    drums: bool,
    /// The drum note of each instrument of the part, by its id.
    unpitched: HashMap<String, i32>,
}

fn part_infos(list: Option<&Element>) -> HashMap<String, PartInfo> {
    let mut infos = HashMap::new();
    let Some(list) = list else { return infos };
    for part in list.children("score-part") {
        let Some(id) = part.attr("id") else { continue };
        let mut info = PartInfo { name: part.text_of("part-name").to_owned(), ..PartInfo::default() };
        if info.name.is_empty() {
            info.name = part.text_of("part-abbreviation").to_owned();
        }
        for midi in part.children("midi-instrument") {
            if number(midi.text_of("midi-channel")) == Some(10.0) {
                info.drums = true;
            }
            if let Some(program) = number(midi.text_of("midi-program")).filter(|p| (1.0..=128.0).contains(p)) {
                info.program.get_or_insert(program as u32 - 1);
            }
            if let (Some(instrument), Some(key)) = (midi.attr("id"), number(midi.text_of("midi-unpitched")).filter(|k| (1.0..=128.0).contains(k))) {
                info.unpitched.insert(instrument.to_owned(), key as i32 - 1);
            }
        }
        infos.insert(id.to_owned(), info);
    }
    infos
}

/// The measures of each part, in the order of the parts: the `<measure>`
/// elements of a partwise score, or the `<part>` elements of each measure
/// of a timewise one (which hold what a measure holds).
fn measures_of(root: &Element, timewise: bool) -> Vec<(String, Vec<&Element>)> {
    if !timewise {
        return root.children("part").map(|p| (p.attr("id").unwrap_or("").to_owned(), p.children("measure").take(MAX_BARS).collect())).collect();
    }
    let mut parts: Vec<(String, Vec<&Element>)> = Vec::new();
    for measure in root.children("measure").take(MAX_BARS) {
        for part in measure.children("part") {
            let id = part.attr("id").unwrap_or("").to_owned();
            match parts.iter_mut().find(|(known, _)| *known == id) {
                Some((_, measures)) => measures.push(part),
                None => parts.push((id, vec![part])),
            }
        }
    }
    parts
}

/// The pitch class of a note name.
fn step_of(step: &str) -> Option<i32> {
    Some(match step.trim() {
        "C" => 0,
        "D" => 2,
        "E" => 4,
        "F" => 5,
        "G" => 7,
        "A" => 9,
        "B" => 11,
        _ => return None,
    })
}

/// A MIDI pitch from a step, an alteration and an octave (middle C is C4).
fn pitch_of(step: &str, alter: f64, octave: f64) -> Option<i32> {
    let pitch = (octave + 1.0) * 12.0 + f64::from(step_of(step)?) + alter.round();
    (0.0..=127.0).contains(&pitch).then_some(pitch as i32)
}

/// A chord name as the project writes them, from a `<harmony>`.
fn chord_name(harmony: &Element) -> Option<String> {
    let note = |element: &Element, prefix: &str| -> Option<String> {
        let step = element.text_of(&format!("{prefix}-step"));
        step_of(step)?;
        let alter = number(element.text_of(&format!("{prefix}-alter"))).unwrap_or(0.0);
        Some(format!("{step}{}", if alter > 0.5 { "#" } else if alter < -0.5 { "b" } else { "" }))
    };
    let root = note(harmony.child("root")?, "root")?;
    let kind = harmony.child("kind");
    let suffix = match kind.map_or("", |k| k.text.trim()) {
        "minor" => "m",
        "augmented" => "aug",
        "diminished" => "dim",
        "dominant" => "7",
        "major-seventh" => "maj7",
        "minor-seventh" => "m7",
        "diminished-seventh" => "dim7",
        "half-diminished" => "m7b5",
        "suspended-second" => "sus2",
        "suspended-fourth" => "sus4",
        "major-sixth" => "6",
        "minor-sixth" => "m6",
        "power" => "5",
        "dominant-ninth" => "9",
        "major-ninth" => "maj9",
        "minor-ninth" => "m9",
        "none" => return Some("N.C.".to_owned()),
        // Any other kind is named by the text the score gives it.
        "major" | "" => "",
        _ => kind.and_then(|k| k.attr("text")).unwrap_or(""),
    };
    let bass = harmony.child("bass").and_then(|b| note(b, "bass")).map_or(String::new(), |b| format!("/{b}"));
    Some(format!("{root}{suffix}{bass}"))
}

/// The events of a bar, and the chord names written over it.
type BarEvents = (Vec<Event>, Vec<(f64, String)>);

/// A beat being gathered: what starts at one time in one voice.
struct Event {
    voice: String,
    start: f64,
    length: f64,
    grace: bool,
    /// A grace note played on the beat, the note after it coming later.
    grace_on_beat: bool,
    rest: bool,
    velocity: Option<u8>,
    notes: Vec<BeatNote>,
    /// The syllable sung on it, ending with a hyphen when the word goes on.
    lyric: Option<String>,
}

/// The syllable of the first verse a note carries: its text, with a hyphen
/// when the word goes on in the next one.
fn lyric_of(note: &Element) -> Option<String> {
    let lyric = note.children("lyric").find(|l| l.attr("number").is_none_or(|n| n == "1"))?;
    let text = lyric.text_of("text").trim();
    if text.is_empty() {
        return None;
    }
    let goes_on = matches!(lyric.text_of("syllabic").trim(), "begin" | "middle");
    Some(if goes_on { format!("{text}-") } else { text.to_owned() })
}

/// What is carried from one measure of a part to the next.
struct PartState {
    /// Divisions of a quarter note.
    divisions: f64,
    /// Semitones from the written pitch to the sounding one.
    transpose: i32,
    /// Open strings, highest first, when the part is tablature.
    tuning: Vec<i32>,
    capo: i32,
    /// The staves written as tablature.
    tab_staves: Vec<String>,
    staves: usize,
    velocity: Option<u8>,
    /// The passes of a repeat the bars now read are played on.
    endings: Vec<u32>,
}

/// The song, and for each track whether it is read as tablature.
fn read(text: &str) -> Result<(Song, Vec<bool>), String> {
    let document = parse_xml(text)?;
    let (root, timewise) = match (document.child("score-partwise"), document.child("score-timewise")) {
        (Some(root), _) => (root, false),
        (None, Some(root)) => (root, true),
        _ => return Err("not a MusicXML score: it has no score-partwise or score-timewise element".to_owned()),
    };
    let infos = part_infos(root.child("part-list"));
    let parts = measures_of(root, timewise);
    if parts.is_empty() {
        return Err("the MusicXML score has no parts".to_owned());
    }
    let count = parts.iter().map(|(_, m)| m.len()).max().unwrap_or(0);
    if count == 0 {
        return Err("the MusicXML score has no bars".to_owned());
    }

    let mut song = Song {
        title: [root.child("work").map_or("", |w| w.text_of("work-title")), root.text_of("movement-title")].into_iter().find(|t| !t.is_empty()).unwrap_or("").to_owned(),
        artist: root
            .child("identification")
            .and_then(|i| i.children("creator").find(|c| matches!(c.attr("type"), Some("composer" | "artist"))).or_else(|| i.child("creator")))
            .map_or(String::new(), |c| c.text.trim().to_owned()),
        bars: vec![BarInfo { signature: (4, 4), ..BarInfo::default() }; count],
        ..Song::default()
    };
    let mut tabbed = Vec::new();
    let mut first_tempo: Option<f64> = None;
    for (part_index, (id, measures)) in parts.iter().enumerate() {
        let master = part_index == 0;
        let info = infos.get(id);
        let mut state = PartState { divisions: 1.0, transpose: 0, tuning: Vec::new(), capo: 0, tab_staves: Vec::new(), staves: 1, velocity: None, endings: Vec::new() };
        let mut signature = (4, 4);
        let mut track_bars: Vec<Vec<Vec<Beat>>> = Vec::with_capacity(count);
        let mut drums = info.is_some_and(|i| i.drums);
        let mut fretted = false;
        // The events of each bar, kept until the part is known to be tablature or not.
        let mut all_events: Vec<BarEvents> = Vec::with_capacity(count);
        for (bar_index, measure) in measures.iter().enumerate() {
            let mut events: Vec<Event> = Vec::new();
            let mut harmonies: Vec<(f64, String)> = Vec::new();
            let (mut position, mut last_start, mut length) = (0.0f64, 0.0f64, 0.0f64);
            let mut bar = song.bars[bar_index].clone();
            let mut endings_close = false;
            for element in &measure.children {
                match element.name.as_str() {
                    "attributes" => {
                        if let Some(d) = number(element.text_of("divisions")).filter(|d| *d > 0.0) {
                            state.divisions = d;
                        }
                        if let Some(key) = element.child("key") {
                            if let Some(fifths) = number(key.text_of("fifths")).filter(|f| (-7.0..=7.0).contains(f)) {
                                if song.key.is_none() || (master && bar_index == 0) {
                                    song.key = Some(songsterr::key_of(fifths as i64, key.text_of("mode").eq_ignore_ascii_case("minor")));
                                }
                            }
                        }
                        if let Some(time) = element.child("time") {
                            let beats: f64 = time.text_of("beats").split('+').filter_map(number).sum();
                            if let Some(unit) = number(time.text_of("beat-type")).filter(|u| [1.0, 2.0, 4.0, 8.0, 16.0, 32.0].contains(u)) {
                                if (1.0..=64.0).contains(&beats) {
                                    signature = (beats as u32, unit as u32);
                                }
                            }
                        }
                        if let Some(staves) = number(element.text_of("staves")).filter(|s| (1.0..=8.0).contains(s)) {
                            state.staves = staves as usize;
                        }
                        if let Some(transpose) = element.child("transpose") {
                            let chromatic = number(transpose.text_of("chromatic")).unwrap_or(0.0);
                            let octaves = number(transpose.text_of("octave-change")).unwrap_or(0.0);
                            state.transpose = (chromatic + 12.0 * octaves).clamp(-48.0, 48.0) as i32;
                        }
                        for clef in element.children("clef") {
                            if clef.text_of("sign").eq_ignore_ascii_case("TAB") {
                                let staff = clef.attr("number").unwrap_or("1").to_owned();
                                if !state.tab_staves.contains(&staff) {
                                    state.tab_staves.push(staff);
                                }
                            }
                            if clef.text_of("sign").eq_ignore_ascii_case("percussion") {
                                drums = true;
                            }
                        }
                        for details in element.children("staff-details") {
                            let mut lines: Vec<(i64, i32)> = details
                                .children("staff-tuning")
                                .filter_map(|t| {
                                    let line = number(t.attr("line")?)? as i64;
                                    let pitch = pitch_of(t.text_of("tuning-step"), number(t.text_of("tuning-alter")).unwrap_or(0.0), number(t.text_of("tuning-octave"))?)?;
                                    Some((line, pitch))
                                })
                                .collect();
                            if !lines.is_empty() {
                                // Line 1 is the lowest string; the project lists the highest first.
                                lines.sort_by_key(|(line, _)| std::cmp::Reverse(*line));
                                state.tuning = lines.into_iter().map(|(_, pitch)| pitch).collect();
                                let staff = details.attr("number").unwrap_or("1").to_owned();
                                if !state.tab_staves.contains(&staff) {
                                    state.tab_staves.push(staff);
                                }
                            }
                            if let Some(capo) = number(details.text_of("capo")).filter(|c| (0.0..=24.0).contains(c)) {
                                state.capo = capo as i32;
                            }
                        }
                    }
                    "note" => {
                        if element.child("cue").is_some() {
                            continue;
                        }
                        let staff = element.child("staff").map_or("1", |s| s.text.trim()).to_owned();
                        // A staff of notes beside one of tablature: the tablature has them all.
                        let skipped = state.staves > 1 && !state.tab_staves.is_empty() && !state.tab_staves.contains(&staff);
                        let chord = element.child("chord").is_some();
                        let grace = element.child("grace").is_some();
                        let grace_on_beat = element.child("grace").is_some_and(|g| g.attr("steal-time-following").is_some());
                        let duration = if grace { 0.0 } else { number(element.text_of("duration")).unwrap_or(0.0).max(0.0) / state.divisions };
                        let start = if chord { last_start } else { position };
                        if !chord {
                            last_start = start;
                            position += duration;
                            length = length.max(position);
                        }
                        if skipped {
                            continue;
                        }
                        let voice = format!("{staff}/{}", element.child("voice").map_or("1", |v| v.text.trim()));
                        let rest = element.child("rest").is_some();
                        let note = if rest { None } else { read_note(element, info, &state, &mut fretted, &mut drums) };
                        let velocity = element.attr("dynamics").and_then(number).map(|d| (d * 0.9).clamp(1.0, 127.0) as u8);
                        let found = events.iter_mut().rev().find(|e| e.voice == voice && (e.start - start).abs() < 1e-6 && e.grace == grace && !e.rest && !rest);
                        match (found, note) {
                            (Some(event), Some(mut note)) => {
                                if event.notes.len() < MAX_CHORD {
                                    note.velocity = velocity.or(note.velocity);
                                    event.notes.push(note);
                                }
                            }
                            (Some(_), None) => {}
                            (None, note) => {
                                let mut notes: Vec<BeatNote> = note.into_iter().collect();
                                for note in &mut notes {
                                    note.velocity = velocity.or(note.velocity);
                                }
                                events.push(Event { voice, start, length: if grace { GRACE_LENGTH } else { duration }, grace, grace_on_beat, rest: rest || notes.is_empty(), velocity: state.velocity, notes, lyric: lyric_of(element) });
                            }
                        }
                    }
                    "backup" => position = (position - number(element.text_of("duration")).unwrap_or(0.0).max(0.0) / state.divisions).max(0.0),
                    "forward" => {
                        position += number(element.text_of("duration")).unwrap_or(0.0).max(0.0) / state.divisions;
                        length = length.max(position);
                    }
                    "harmony" => {
                        let offset = number(element.text_of("offset")).unwrap_or(0.0) / state.divisions;
                        if let Some(name) = chord_name(element) {
                            harmonies.push(((position + offset).max(0.0), name));
                        }
                    }
                    "direction" | "sound" => {
                        let sound = if element.name == "sound" { Some(element) } else { element.child("sound") };
                        let offset = number(element.text_of("offset")).unwrap_or(0.0) / state.divisions;
                        let at = (position + offset).max(0.0);
                        let tempo = sound.and_then(|s| s.attr("tempo")).and_then(number).or_else(|| {
                            // A metronome mark without a sound: its beats per minute, as quarter notes.
                            let metronome = element.children("direction-type").find_map(|d| d.child("metronome"))?;
                            let unit = match metronome.text_of("beat-unit") {
                                "half" => 2.0,
                                "eighth" => 0.5,
                                _ => 1.0,
                            };
                            let dotted = if metronome.child("beat-unit-dot").is_some() { 1.5 } else { 1.0 };
                            number(metronome.text_of("per-minute")).map(|bpm| bpm * unit * dotted)
                        });
                        if let Some(bpm) = tempo.filter(|t| (10.0..=600.0).contains(t)) {
                            if bar_index == 0 && at < 1e-6 && first_tempo.is_none() {
                                first_tempo = Some(bpm);
                            } else if !bar.tempos.iter().any(|(o, _)| (o - at).abs() < 1e-6) {
                                bar.tempos.push((at, bpm));
                            }
                        }
                        if let Some(dynamics) = sound.and_then(|s| s.attr("dynamics")).and_then(number) {
                            state.velocity = Some((dynamics * 0.9).clamp(1.0, 127.0) as u8);
                        }
                        if let (true, Some(sound)) = (master, sound) {
                            read_sound(sound, &mut bar);
                        }
                        for kind in element.children("direction-type") {
                            if let Some(mark) = kind.child("dynamics").and_then(|d| d.children.first()) {
                                if let Some(velocity) = songsterr::velocity_of(&mark.name) {
                                    state.velocity = Some(velocity);
                                }
                            }
                            if master {
                                if let Some(text) = kind.child("rehearsal").map(|r| r.text.trim()).filter(|t| !t.is_empty()) {
                                    bar.marker.get_or_insert_with(|| text.to_owned());
                                }
                            }
                        }
                    }
                    "barline" if master => {
                        if let Some(sound) = element.child("sound") {
                            read_sound(sound, &mut bar);
                        }
                        if let Some(repeat) = element.child("repeat") {
                            match repeat.attr("direction") {
                                Some("forward") => bar.repeat_start = true,
                                // "times" counts every pass of the section.
                                Some("backward") => bar.repeat = Some(repeat.attr("times").and_then(number).map_or(2, |t| t.clamp(2.0, 64.0) as u32)),
                                _ => {}
                            }
                        }
                        if let Some(ending) = element.child("ending") {
                            match ending.attr("type") {
                                Some("start") => {
                                    state.endings = ending.attr("number").unwrap_or("").split([',', ' ']).filter_map(|n| n.trim().parse::<u32>().ok()).filter(|n| (1..=64).contains(n)).collect();
                                }
                                Some("stop" | "discontinue") => endings_close = true,
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            if master {
                bar.signature = signature;
                bar.endings = state.endings.clone();
                if endings_close {
                    state.endings.clear();
                }
            }
            song.bars[bar_index] = bar;
            let _ = length;
            all_events.push((events, harmonies));
        }
        // A bar the part does not reach is empty for it.
        all_events.resize_with(count, || (Vec::new(), Vec::new()));

        let tab = fretted && !drums && !state.tuning.is_empty();
        for (events, harmonies) in all_events {
            track_bars.push(lanes(events, harmonies, tab, &state.tuning));
        }
        let info_name = info.map_or("", |i| i.name.trim());
        let program = if drums { DRUMS } else { info.and_then(|i| i.program).unwrap_or(0) };
        let tuning = if drums {
            Vec::new()
        } else if tab {
            state.tuning.clone()
        } else {
            // Read by pitch: each pitch is a string of its own, open on that pitch.
            (0..128).collect()
        };
        song.tracks.push(Track { name: if info_name.is_empty() { id.clone() } else { info_name.to_owned() }, program, tuning, capo: if tab { state.capo } else { 0 }, bars: track_bars });
        tabbed.push(tab);
    }
    song.tempo = first_tempo.unwrap_or(DEFAULT_TEMPO);
    // A D.C. or a D.S. goes on to the coda when the score sends there, else to the fine.
    let to_coda = song.bars.iter().any(|b| b.signs.to_coda);
    let fine = song.bars.iter().any(|b| b.signs.fine);
    let until = if to_coda { Until::Coda } else if fine { Until::Fine } else { Until::End };
    for bar in &mut song.bars {
        bar.signs.jump = match bar.signs.jump {
            Some(Jump::Capo(_)) => Some(Jump::Capo(until)),
            Some(Jump::Segno(_)) => Some(Jump::Segno(until)),
            other => other,
        };
    }
    // The signature holds until the score changes it.
    let mut signature = (4, 4);
    for (index, bar) in song.bars.iter_mut().enumerate() {
        if index > 0 && bar.signature == (4, 4) && parts[0].1.get(index).is_none() {
            bar.signature = signature;
        }
        signature = bar.signature;
    }
    Ok((song, tabbed))
}

/// What a `<sound>` says of the playback: the signs of the score (segno,
/// coda, fine, D.C., D.S., to coda), whose jumps learn later where they lead,
/// and the swing from this bar on.
fn read_sound(sound: &Element, bar: &mut BarInfo) {
    let has = |name: &str| sound.attr(name).is_some_and(|value| !value.trim().is_empty() && value.trim() != "no");
    if has("segno") {
        bar.signs.set("Segno");
    }
    if has("coda") {
        bar.signs.set("Coda");
    }
    if has("fine") {
        bar.signs.set("Fine");
    }
    if has("tocoda") {
        bar.signs.set("DaCoda");
    }
    if has("dacapo") {
        bar.signs.set("DaCapo");
    }
    if has("dalsegno") {
        bar.signs.set("DaSegno");
    }
    if let Some(swing) = sound.child("swing") {
        let first = number(swing.text_of("first")).unwrap_or(0.0);
        let second = number(swing.text_of("second")).unwrap_or(0.0);
        bar.feel = Some(if swing.child("straight").is_some() || first <= second {
            0.0
        } else if swing.text_of("swing-type") == "16th" {
            0.25
        } else {
            0.5
        });
    }
}

/// One note as the reader of beats knows it: on a string and a fret when
/// the part is tablature, else on the string of its pitch (a drum's string
/// and fret are its key).
fn read_note(element: &Element, info: Option<&PartInfo>, state: &PartState, fretted: &mut bool, drums: &mut bool) -> Option<BeatNote> {
    let mut note = BeatNote::default();
    let notations: Vec<&Element> = element.children("notations").collect();
    let technical = notations.iter().find_map(|n| n.child("technical"));
    let articulations = notations.iter().find_map(|n| n.child("articulations"));
    let pitch = if let Some(unpitched) = element.child("unpitched") {
        *drums = true;
        let by_instrument = element.child("instrument").and_then(|i| i.attr("id")).and_then(|id| info?.unpitched.get(id).copied());
        by_instrument.or_else(|| pitch_of(unpitched.text_of("display-step"), 0.0, number(unpitched.text_of("display-octave"))?))?
    } else {
        let pitch = element.child("pitch")?;
        pitch_of(pitch.text_of("step"), number(pitch.text_of("alter")).unwrap_or(0.0), number(pitch.text_of("octave"))?)? + state.transpose
    };
    if !(0..=127).contains(&pitch) {
        return None;
    }
    let on_string = technical.and_then(|t| Some((number(t.text_of("string"))?, number(t.text_of("fret"))?)));
    if *drums {
        note.string = pitch as usize;
        note.fret = pitch;
    } else if let Some((string, fret)) = on_string.filter(|(s, f)| (1.0..=12.0).contains(s) && (0.0..=36.0).contains(f)) {
        *fretted = true;
        note.string = string as usize - 1;
        note.fret = fret as i32;
    } else {
        // By pitch; a part that turns out to be tablature places it later.
        note.string = pitch as usize;
        note.fret = 0;
    }
    note.tie = element.children("tie").any(|t| t.attr("type") == Some("stop")) || notations.iter().any(|n| n.children("tied").any(|t| t.attr("type") == Some("stop")));
    note.staccato = articulations.is_some_and(|a| a.child("staccato").is_some() || a.child("staccatissimo").is_some());
    note.accent = match articulations {
        Some(a) if a.child("strong-accent").is_some() => 2,
        Some(a) if a.child("accent").is_some() => 1,
        _ => 0,
    };
    let head = element.child("notehead");
    note.dead = head.is_some_and(|h| h.text.trim() == "x");
    note.ghost = head.is_some_and(|h| h.attr("parentheses") == Some("yes"));
    if let Some(technical) = technical {
        note.hammer = ["hammer-on", "pull-off"].iter().any(|name| technical.children(name).any(|e| e.attr("type") != Some("stop")));
        if let Some(bend) = technical.child("bend") {
            let alter = number(bend.text_of("bend-alter")).unwrap_or(0.0).clamp(-f64::from(BEND_LIMIT), f64::from(BEND_LIMIT)) as f32;
            note.bend = if bend.child("release").is_some() { vec![(0.0, 0.0), (0.4, alter), (1.0, 0.0)] } else { vec![(0.0, 0.0), (0.5, alter), (1.0, alter)] };
        }
        if technical.child("harmonic").is_some() && on_string.is_some() {
            note.harmonic = Some((technical.child("harmonic").is_some_and(|h| h.child("artificial").is_none()), note.fret));
        }
    }
    if notations.iter().any(|n| n.children("slide").chain(n.children("glissando")).any(|s| s.attr("type") == Some("start"))) {
        note.slide = Some(Slide::Shift);
    }
    let ornaments: Vec<&Element> = notations.iter().filter_map(|n| n.child("ornaments")).collect();
    if ornaments.iter().any(|o| o.child("vibrato").is_some() || (o.child("wavy-line").is_some() && o.child("trill-mark").is_none())) {
        note.vibrato = Some(Vibrato::Slight);
    }
    // Tremolo picking: one slash for eighth notes, two for sixteenths, three for thirty-seconds.
    if let Some(slashes) = ornaments.iter().find_map(|o| o.child("tremolo")).filter(|t| t.attr("type").is_none_or(|k| k == "single")).and_then(|t| number(&t.text)) {
        note.tremolo = Some(0.5 / 2f32.powi((slashes.clamp(1.0, 4.0) as i32) - 1));
    }
    // A trill with the note a tone above, in thirty-seconds.
    if ornaments.iter().any(|o| o.child("trill-mark").is_some()) && (0..=125).contains(&pitch) {
        note.trill = Some((TrillTo::Pitch(pitch as u8 + 2), 0.125));
    }
    Some(note)
}

/// Places a note read by pitch on the strings of a tablature: the string
/// that gives the lowest fret.
fn place(note: &mut BeatNote, tuning: &[i32]) -> bool {
    let pitch = note.string as i32;
    let best = tuning.iter().enumerate().filter(|(_, open)| pitch >= **open && pitch - **open <= 24).min_by_key(|(_, open)| pitch - **open);
    match best {
        Some((string, open)) => {
            note.string = string;
            note.fret = pitch - open;
            true
        }
        None => false,
    }
}

/// The voices of a bar from its events: each voice of the score, split in
/// two where its notes overlap, with rests where nothing sounds. The chord
/// names go on the first voice, on the beat they start at or the next one.
fn lanes(mut events: Vec<Event>, harmonies: Vec<(f64, String)>, tab: bool, tuning: &[i32]) -> Vec<Vec<Beat>> {
    events.sort_by(|a, b| a.voice.cmp(&b.voice).then(a.start.total_cmp(&b.start)).then(b.grace.cmp(&a.grace)));
    // (voice, where it has reached, its beats)
    let mut lanes: Vec<(String, f64, Vec<Beat>)> = Vec::new();
    for mut event in events {
        if tab {
            event.notes.retain_mut(|note| note.fret != 0 || note.string < tuning.len() || place(note, tuning));
            // Notes still on the string of their pitch were read by pitch.
            for note in &mut event.notes {
                if note.string >= tuning.len() {
                    place(note, tuning);
                }
            }
            event.notes.retain(|note| note.string < tuning.len());
            if event.notes.is_empty() {
                event.rest = true;
            }
        }
        let lane = lanes.iter().position(|(voice, reached, _)| *voice == event.voice && *reached <= event.start + 1e-6).unwrap_or_else(|| {
            lanes.push((event.voice.clone(), 0.0, Vec::new()));
            lanes.len() - 1
        });
        let (_, reached, beats) = &mut lanes[lane];
        if event.start > *reached + 1e-6 {
            beats.push(Beat { length: event.start - *reached, rest: true, ..Beat::default() });
            *reached = event.start;
        }
        if !event.grace {
            *reached = event.start + event.length;
        }
        beats.push(Beat { length: event.length, rest: event.rest, grace: event.grace, grace_on_beat: event.grace_on_beat, velocity: event.velocity, notes: event.notes, lyric: event.lyric, ..Beat::default() });
    }
    let mut voices: Vec<Vec<Beat>> = lanes.into_iter().map(|(_, _, beats)| beats).collect();
    if let Some(first) = voices.first_mut() {
        for (at, name) in harmonies {
            let mut position = 0.0;
            for beat in first.iter_mut() {
                if position + 1e-6 >= at && !beat.grace {
                    beat.chord.get_or_insert(name);
                    break;
                }
                if !beat.grace {
                    position += beat.length;
                }
            }
        }
    }
    voices
}

#[cfg(test)]
mod tests {
    use super::super::parse as parse_any;
    use super::*;

    /// A guitar written as a staff of notes and a staff of tablature, as
    /// Guitar Pro writes it, then a piano by pitch in a second part.
    const SCORE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">
<score-partwise version="4.0">
  <work><work-title>Essai &amp; Co</work-title></work>
  <identification><creator type="composer">Quelqu'un</creator></identification>
  <part-list>
    <score-part id="P1"><part-name>Guitare</part-name>
      <midi-instrument id="P1-I1"><midi-channel>1</midi-channel><midi-program>26</midi-program></midi-instrument></score-part>
    <score-part id="P2"><part-name>Piano</part-name>
      <midi-instrument id="P2-I1"><midi-channel>2</midi-channel><midi-program>1</midi-program></midi-instrument></score-part>
  </part-list>
  <part id="P1">
    <measure number="1">
      <attributes><divisions>2</divisions><key><fifths>1</fifths><mode>minor</mode></key><time><beats>4</beats><beat-type>4</beat-type></time>
        <staves>2</staves><clef number="1"><sign>G</sign><line>2</line></clef><clef number="2"><sign>TAB</sign><line>5</line></clef>
        <staff-details number="2"><staff-lines>6</staff-lines>
          <staff-tuning line="1"><tuning-step>E</tuning-step><tuning-octave>2</tuning-octave></staff-tuning>
          <staff-tuning line="2"><tuning-step>A</tuning-step><tuning-octave>2</tuning-octave></staff-tuning>
          <staff-tuning line="3"><tuning-step>D</tuning-step><tuning-octave>3</tuning-octave></staff-tuning>
          <staff-tuning line="4"><tuning-step>G</tuning-step><tuning-octave>3</tuning-octave></staff-tuning>
          <staff-tuning line="5"><tuning-step>B</tuning-step><tuning-octave>3</tuning-octave></staff-tuning>
          <staff-tuning line="6"><tuning-step>E</tuning-step><tuning-octave>4</tuning-octave></staff-tuning>
        </staff-details></attributes>
      <barline location="left"><repeat direction="forward"/></barline>
      <direction><direction-type><rehearsal>Intro</rehearsal></direction-type><sound tempo="90"/></direction>
      <harmony><root><root-step>A</root-step></root><kind>minor</kind></harmony>
      <note><pitch><step>A</step><octave>3</octave></pitch><duration>4</duration><voice>1</voice><type>half</type><staff>1</staff></note>
      <note><chord/><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice><type>half</type><staff>1</staff></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><tie type="start"/><voice>1</voice><staff>1</staff></note>
      <backup><duration>8</duration></backup>
      <note><pitch><step>A</step><octave>3</octave></pitch><duration>4</duration><voice>5</voice><staff>2</staff>
        <notations><technical><string>3</string><fret>2</fret></technical><articulations><accent/></articulations></notations></note>
      <note><chord/><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>5</voice><staff>2</staff>
        <notations><technical><string>2</string><fret>1</fret><hammer-on type="start">H</hammer-on></technical></notations></note>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><tie type="start"/><voice>5</voice><staff>2</staff>
        <notations><technical><string>1</string><fret>0</fret></technical></notations></note>
      <barline location="right"><repeat direction="backward"/></barline>
    </measure>
    <measure number="2">
      <attributes><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
      <note><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><tie type="stop"/><voice>5</voice><staff>2</staff>
        <notations><technical><string>1</string><fret>0</fret></technical></notations></note>
      <note><rest/><duration>2</duration><voice>5</voice><staff>2</staff></note>
      <note><grace/><pitch><step>G</step><octave>3</octave></pitch><voice>5</voice><staff>2</staff>
        <notations><technical><string>3</string><fret>0</fret></technical></notations></note>
      <note><pitch><step>A</step><octave>3</octave></pitch><duration>2</duration><voice>5</voice><staff>2</staff>
        <notations><technical><string>3</string><fret>2</fret><bend><bend-alter>2</bend-alter></bend></technical></notations></note>
    </measure>
  </part>
  <part id="P2">
    <measure number="1">
      <attributes><divisions>1</divisions></attributes>
      <direction><direction-type><dynamics><p/></dynamics></direction-type></direction>
      <note><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><voice>1</voice><lyric number="1"><syllabic>begin</syllabic><text>Ma</text></lyric><lyric number="2"><text>No</text></lyric></note>
      <note><pitch><step>D</step><alter>1</alter><octave>4</octave></pitch><duration>1</duration><voice>1</voice><lyric number="1"><syllabic>end</syllabic><text>ma</text></lyric></note>
      <forward><duration>2</duration></forward>
      <backup><duration>4</duration></backup>
      <note><pitch><step>C</step><octave>3</octave></pitch><duration>4</duration><voice>2</voice></note>
    </measure>
    <measure number="2">
      <note><pitch><step>B</step><alter>-1</alter><octave>3</octave></pitch><duration>3</duration><voice>1</voice><notations><articulations><staccato/></articulations></notations><lyric><syllabic>single</syllabic><text>Oh</text></lyric></note>
    </measure>
  </part>
</score-partwise>"#;

    #[test]
    fn a_musicxml_score_becomes_parts_of_notes() {
        assert!(is_score(SCORE.as_bytes()) && !is_score(b"<GPIF/>"));
        let score = parse_any(SCORE.as_bytes()).unwrap();
        assert_eq!((score.title.as_str(), score.artist.as_str()), ("Essai & Co", "Quelqu'un"));
        // The words of the first verse, on their notes: sung again with the repeat.
        let words: Vec<(f64, &str)> = score.parts[1].lyrics.iter().map(|s| (s.beat, s.text.as_str())).collect();
        assert_eq!(words, [(0.0, "Ma-"), (1.0, "ma"), (4.0, "Ma-"), (5.0, "ma"), (8.0, "Oh")]);
        assert_eq!(crate::lyrics::join(&score.parts[1].lyrics[..2]), "Mama");
        assert_eq!(score.parts.len(), 2);

        let guitar = &score.parts[0];
        assert_eq!((guitar.name.as_str(), guitar.instrument_id), ("Guitare", 25));
        assert_eq!(guitar.tuning, [64, 59, 55, 50, 45, 40], "the tablature's tuning, highest string first");
        assert_eq!(guitar.key, Some((4, true)), "one sharp, minor: E minor");
        assert_eq!(guitar.signature, (4, 4));
        assert_eq!(guitar.tempo[0].value, 90.0);
        assert_eq!(guitar.markers[0].name, "Intro");
        assert_eq!(guitar.chords[0].name, "Am");
        // The repeat is played twice: 4 beats, 4 again, then the 3/4 bar.
        assert_eq!(guitar.length, 11.0);
        let at = |beat: f64| guitar.notes.iter().filter(|n| (n.start - beat).abs() < 1e-6).collect::<Vec<_>>();
        let first = at(0.0);
        assert_eq!(first.len(), 2, "the staff of notes is not read twice");
        let a = first.iter().find(|n| n.pitch == 57).unwrap();
        assert_eq!((a.string, a.length, a.articulation.accent), (Some(2), 2.0, 1));
        assert!(first.iter().any(|n| n.pitch == 60 && n.string == Some(1) && n.articulation.hammer));
        let held = at(6.0);
        assert_eq!((held[0].pitch, held[0].length), (64, 3.0), "tied over the bar line on the second pass");
        assert!(at(9.0).is_empty(), "a rest");
        let bent = at(10.0);
        assert_eq!((bent[0].pitch, bent[0].articulation.bend.len()), (57, 3));
        let grace = guitar.notes.iter().find(|n| n.pitch == 55).unwrap();
        assert!(grace.start < 10.0 && grace.length <= GRACE_LENGTH);

        let piano = &score.parts[1];
        assert!(piano.tuning.is_empty() && piano.notes.iter().all(|n| n.string.is_none()), "read by pitch");
        let notes: Vec<(f64, u8, f64)> = piano.notes.iter().map(|n| (n.start, n.pitch, n.length)).collect();
        assert!(notes.contains(&(0.0, 60, 1.0)) && notes.contains(&(1.0, 63, 1.0)) && notes.contains(&(0.0, 48, 4.0)));
        assert!(notes.contains(&(8.0, 58, 3.0)), "the second bar comes after the repeat");
        assert!(piano.notes.iter().all(|n| n.velocity < 96), "played piano");
    }

    #[test]
    fn the_signs_of_the_score_swing_tremolo_and_trills_are_read() {
        // Five bars of a whole note, the step giving the bar: a segno on bar 2,
        // "to coda" on bar 3, D.S. on bar 4, the coda on bar 5; swing from bar 1.
        let sounds = [r#"<sound><swing><first>2</first><second>1</second><swing-type>eighth</swing-type></swing></sound>"#, r#"<sound segno="s"/>"#, r#"<sound tocoda="c"/>"#, r#"<sound dalsegno="s"/>"#, r#"<sound coda="c"/>"#];
        let steps = ["C", "D", "E", "F", "G"];
        let measures: String = sounds
            .iter()
            .zip(steps)
            .enumerate()
            .map(|(bar, (sound, step))| {
                let attributes = if bar == 0 { "<attributes><divisions>1</divisions></attributes>" } else { "" };
                let ornaments = match bar {
                    0 => "<notations><ornaments><tremolo type=\"single\">2</tremolo></ornaments></notations>",
                    4 => "<notations><ornaments><trill-mark/></ornaments></notations>",
                    _ => "",
                };
                format!("<measure>{attributes}<direction><direction-type><words>x</words></direction-type>{sound}</direction><note><pitch><step>{step}</step><octave>4</octave></pitch><duration>4</duration>{ornaments}</note></measure>")
            })
            .collect();
        let score = format!(r#"<score-partwise><part-list><score-part id="P"><part-name>P</part-name></score-part></part-list><part id="P">{measures}</part></score-partwise>"#);
        let part = &parse_any(score.as_bytes()).unwrap().parts[0];
        let played: Vec<(f64, u8)> = part.notes.iter().map(|n| (n.start, n.pitch)).collect();
        assert_eq!(played, [(0.0, 60), (4.0, 62), (8.0, 64), (12.0, 65), (16.0, 62), (20.0, 64), (24.0, 67)], "D.S. al coda");
        assert_eq!(part.notes[0].articulation.tremolo, Some(0.25), "two slashes: sixteenths");
        assert_eq!(part.notes[6].articulation.trill, Some((69, 0.125)), "a tone above");
        assert_eq!(part.swing.len(), 1);
        assert!(!part.swing[0].is_straight() && part.swing[0].unit == 0.5);
    }

    #[test]
    fn archives_timewise_scores_and_odd_files_are_read_or_refused() {
        use std::io::Write;
        // A compressed MusicXML file: its container names the score.
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("META-INF/container.xml", options).unwrap();
        archive.write_all(br#"<container><rootfiles><rootfile full-path="song.musicxml"/></rootfiles></container>"#).unwrap();
        archive.start_file("song.musicxml", options).unwrap();
        archive.write_all(SCORE.as_bytes()).unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        assert!(is_archive(&bytes));
        assert_eq!(parse_any(&bytes).unwrap(), parse_any(SCORE.as_bytes()).unwrap());

        // Timewise: the parts inside the measures.
        let timewise = r#"<score-timewise><part-list><score-part id="A"><part-name>Flûte</part-name></score-part></part-list>
            <measure number="1"><part id="A"><attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>
            <note><pitch><step>G</step><octave>5</octave></pitch><duration>3</duration></note></part></measure></score-timewise>"#;
        let score = parse_any(timewise.as_bytes()).unwrap();
        assert_eq!((score.parts[0].name.as_str(), score.parts[0].signature, score.parts[0].notes[0].pitch), ("Flûte", (3, 4), 79));

        // UTF-16, as some editors write it.
        let mut utf16 = vec![0xFF, 0xFE];
        utf16.extend(timewise.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(parse_any(&utf16).unwrap(), score);

        // Drums: the key of each instrument.
        let drums = r#"<score-partwise><part-list><score-part id="D"><part-name>Drums</part-name>
            <midi-instrument id="D-36"><midi-channel>10</midi-channel><midi-unpitched>37</midi-unpitched></midi-instrument></score-part></part-list>
            <part id="D"><measure><attributes><divisions>1</divisions><clef><sign>percussion</sign></clef></attributes>
            <note><unpitched><display-step>F</display-step><display-octave>4</display-octave></unpitched><duration>1</duration><instrument id="D-36"/></note></measure></part></score-partwise>"#;
        let score = parse_any(drums.as_bytes()).unwrap();
        assert!(score.parts[0].is_drums());
        assert_eq!(score.parts[0].notes[0].pitch, 36);

        assert!(parse_any(b"<score-partwise></score-partwise>").unwrap_err().contains("no parts"));
        assert!(parse_any(b"<score-partwise><part id=\"P\"/></score-partwise>").unwrap_err().contains("no bars"));
        // Anything at all gives an answer, never a crash.
        for cut in (0..SCORE.len()).step_by(97) {
            let _ = parse_any(&SCORE.as_bytes()[..cut]);
        }
    }
}
