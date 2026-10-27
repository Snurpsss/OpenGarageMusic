//! Naming the chords that notes struck together make.

use crate::model::{ChordMark, Note};

/// A chord: its root (0 = C … 11 = B), what kind it is, and the note in
/// the bass when that is not the root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Chord {
    pub root: u8,
    pub kind: &'static str,
    pub bass: Option<u8>,
}

/// Names of the pitch classes, with sharps.
pub const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// The kinds of chord that are recognised: a suffix and the intervals,
/// in semitones from the root, that make the chord.
const KINDS: [(&str, &[u8]); 24] = [
    ("", &[0, 4, 7]),
    ("m", &[0, 3, 7]),
    ("5", &[0, 7]),
    ("dim", &[0, 3, 6]),
    ("aug", &[0, 4, 8]),
    ("sus2", &[0, 2, 7]),
    ("sus4", &[0, 5, 7]),
    ("7", &[0, 4, 7, 10]),
    ("maj7", &[0, 4, 7, 11]),
    ("m7", &[0, 3, 7, 10]),
    ("m7b5", &[0, 3, 6, 10]),
    ("dim7", &[0, 3, 6, 9]),
    ("6", &[0, 4, 7, 9]),
    ("m6", &[0, 3, 7, 9]),
    ("add9", &[0, 2, 4, 7]),
    ("madd9", &[0, 2, 3, 7]),
    ("7sus4", &[0, 5, 7, 10]),
    ("7sus2", &[0, 2, 7, 10]),
    ("9", &[0, 2, 4, 7, 10]),
    ("m9", &[0, 2, 3, 7, 10]),
    ("maj9", &[0, 2, 4, 7, 11]),
    ("mmaj7", &[0, 3, 7, 11]),
    ("7#5", &[0, 4, 8, 10]),
    ("7b5", &[0, 4, 6, 10]),
];

impl Chord {
    /// "C", "Am7", "G/B"…
    pub fn text(&self) -> String {
        let mut text = format!("{}{}", NAMES[usize::from(self.root % 12)], self.kind);
        if let Some(bass) = self.bass {
            text.push('/');
            text.push_str(NAMES[usize::from(bass % 12)]);
        }
        text
    }
}

/// Where the fingers go for a chord on a fretted instrument: the first of
/// the four frets shown (1 when the nut is), and for each string, highest
/// first as in a tuning, its fret: 0 for the open string, None for a
/// string that is not played.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shape {
    pub base: u8,
    pub frets: Vec<Option<u8>>,
}

/// The ways of playing chords found so far, by chord and tuning.
type Shapes = std::collections::HashMap<(Chord, Vec<u8>), Option<Shape>>;

/// How many frets a chord grid shows.
pub const GRID_FRETS: u8 = 4;

impl Shape {
    /// The grid for notes that are held on the strings, given as the fret of
    /// each string: it starts at the lowest fret held, or at the nut when
    /// the notes are near it. None when the notes do not fit in the grid,
    /// or when fewer than two strings sound.
    pub fn of_notes(held: &[Option<u8>]) -> Option<Shape> {
        if held.iter().flatten().count() < 2 {
            return None;
        }
        let fretted: Vec<u8> = held.iter().flatten().copied().filter(|f| *f > 0).collect();
        let lowest = fretted.iter().copied().min().unwrap_or(1);
        let highest = fretted.iter().copied().max().unwrap_or(1);
        if highest >= lowest + GRID_FRETS {
            return None;
        }
        // Near the nut, the nut is shown.
        let base = if highest <= GRID_FRETS { 1 } else { lowest };
        Some(Shape { base, frets: held.to_vec() })
    }
}

/// The pitch class of the note a name starts with ("C", "F#", "Bb"), and
/// what follows it.
fn split_root(text: &str) -> Option<(u8, &str)> {
    let letter = text.chars().next()?;
    let mut root = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let mut rest = &text[1..];
    if let Some(after) = rest.strip_prefix('#') {
        root = (root + 1) % 12;
        rest = after;
    } else if let Some(after) = rest.strip_prefix('b') {
        root = (root + 11) % 12;
        rest = after;
    }
    Some((root, rest))
}

/// Other ways of writing the kinds of chord, as tablatures and scores do,
/// and the kind each one means.
const SAME_KINDS: [(&str, &str); 22] = [
    ("M", ""),
    ("maj", ""),
    ("min", "m"),
    ("mi", "m"),
    ("-", "m"),
    ("M7", "maj7"),
    ("Maj7", "maj7"),
    ("ma7", "maj7"),
    ("min7", "m7"),
    ("-7", "m7"),
    ("m7(b5)", "m7b5"),
    ("ø", "m7b5"),
    ("ø7", "m7b5"),
    ("°", "dim"),
    ("o", "dim"),
    ("°7", "dim7"),
    ("o7", "dim7"),
    ("+", "aug"),
    ("sus", "sus4"),
    ("7sus", "7sus4"),
    ("add2", "add9"),
    ("2", "sus2"),
];

/// A chord name written with the signs of printed music ("B♭7", "F♯m",
/// "CΔ7"), as the program's own names write it ("Bb7", "F#m", "Cmaj7").
pub fn plain(name: &str) -> String {
    name.trim().replace('♭', "b").replace('♯', "#").replace(['Δ', '△'], "maj")
}

impl Chord {
    /// The chord a name means: "C", "F#m7", "Bb/D", and the other usual ways
    /// of writing them ("B♭7", "CM7", "Am7(b5)"…). None for anything else,
    /// such as "N.C.".
    pub fn parse(text: &str) -> Option<Chord> {
        let text = plain(text);
        let (body, bass) = match text.split_once('/') {
            Some((body, bass)) => (body, Some(bass)),
            None => (text.as_str(), None),
        };
        let (root, kind) = split_root(body)?;
        let kind = SAME_KINDS.iter().find(|(other, _)| *other == kind).map_or(kind, |(_, same)| *same);
        let kind = KINDS.iter().map(|(k, _)| *k).find(|k| *k == kind)?;
        let bass = match bass {
            Some(bass) => match split_root(bass)? {
                (note, "") => Some(note),
                _ => return None,
            },
            None => None,
        };
        Some(Chord { root, kind, bass })
    }

    /// The pitch classes of the chord, the root first, the bass note last
    /// when it is not one of them.
    pub fn tones(&self) -> Vec<u8> {
        let (_, intervals) = KINDS.iter().find(|(k, _)| *k == self.kind).unwrap_or(&KINDS[0]);
        let mut tones: Vec<u8> = intervals.iter().map(|i| (self.root + i) % 12).collect();
        if let Some(bass) = self.bass.map(|b| b % 12) {
            if !tones.contains(&bass) {
                tones.push(bass);
            }
        }
        tones
    }

    /// A way to play the chord on strings with this tuning (highest first),
    /// within four frets: the one nearest the nut that sounds every note of
    /// the chord with its bass note lowest, on as many strings as possible,
    /// with as few fingers as possible (a barre counts as one), then as
    /// narrow as possible. None when the chord cannot be played that way.
    pub fn shape(&self, tuning: &[u8]) -> Option<Shape> {
        // The search tries many fingerings: what it found is kept, as the
        // same chords are drawn again and again.
        thread_local! {
            static FOUND: std::cell::RefCell<Shapes> = std::cell::RefCell::new(Shapes::new());
        }
        let key = (*self, tuning.to_vec());
        if let Some(found) = FOUND.with(|found| found.borrow().get(&key).cloned()) {
            return found;
        }
        let shape = self.search_shape(tuning);
        FOUND.with(|found| found.borrow_mut().insert(key, shape.clone()));
        shape
    }

    fn search_shape(&self, tuning: &[u8]) -> Option<Shape> {
        let tones = self.tones();
        let bass = self.bass.unwrap_or(self.root) % 12;
        if tuning.is_empty() {
            return None;
        }
        for base in 1..=12u8 {
            let lowest = if base == 1 { 0 } else { base };
            let highest = base + GRID_FRETS - 1;
            // What each string may do: stay silent, or sound a note of the
            // chord within the frets shown.
            let options: Vec<Vec<Option<u8>>> = tuning
                .iter()
                .map(|open| std::iter::once(None).chain((lowest..=highest).filter(|f| tones.contains(&((open + f) % 12))).map(Some)).collect())
                .collect();
            // The best fingering so far, and its score (silent strings, fingers, span).
            type Scored = (Vec<Option<u8>>, (usize, usize, u8));
            let mut best: Option<Scored> = None;
            let mut pick = vec![0usize; tuning.len()];
            'combinations: loop {
                let frets: Vec<Option<u8>> = pick.iter().enumerate().map(|(s, p)| options[s][*p]).collect();
                if let Some(score) = shape_score(&frets, tuning, &tones, bass) {
                    if best.as_ref().is_none_or(|(_, b)| score < *b) {
                        best = Some((frets, score));
                    }
                }
                for string in 0..tuning.len() {
                    pick[string] += 1;
                    if pick[string] < options[string].len() {
                        continue 'combinations;
                    }
                    pick[string] = 0;
                }
                break;
            }
            if let Some((frets, _)) = best {
                return Some(Shape { base, frets });
            }
        }
        None
    }
}

/// How good a way of playing a chord is, smaller being better: the strings
/// left silent, the fingers needed, the frets spanned. None when a note of
/// the chord is missing or the lowest string played does not sound its bass.
fn shape_score(frets: &[Option<u8>], tuning: &[u8], tones: &[u8], bass: u8) -> Option<(usize, usize, u8)> {
    let sounding: Vec<u8> = frets.iter().zip(tuning).filter_map(|(f, open)| f.map(|f| (open + f) % 12)).collect();
    if sounding.len() < 2 || tones.iter().any(|t| !sounding.contains(t)) {
        return None;
    }
    // Strings are listed highest first: the bass is the last one played.
    if *sounding.last()? != bass {
        return None;
    }
    let fretted: Vec<u8> = frets.iter().flatten().copied().filter(|f| *f > 0).collect();
    let lowest = fretted.iter().copied().min().unwrap_or(0);
    let highest = fretted.iter().copied().max().unwrap_or(0);
    let at_lowest = fretted.iter().filter(|f| **f == lowest).count();
    let open = frets.contains(&Some(0));
    let barre = at_lowest >= 2 && !open;
    let fingers = fretted.len() - if barre { at_lowest - 1 } else { 0 };
    Some((frets.len() - sounding.len(), fingers, highest - lowest))
}

/// The chord these pitches make, if they make one that is known. The lowest
/// note is tried as the root first; a chord whose lowest note is not its
/// root is named with that note after a slash.
pub fn name(pitches: &[u8]) -> Option<Chord> {
    let lowest = *pitches.iter().min()? % 12;
    let mut classes: Vec<u8> = pitches.iter().map(|p| p % 12).collect();
    classes.sort_unstable();
    classes.dedup();
    if classes.len() < 2 {
        return None;
    }
    let roots = std::iter::once(lowest).chain(classes.iter().copied().filter(|c| *c != lowest));
    for root in roots {
        let mut intervals: Vec<u8> = classes.iter().map(|c| (c + 12 - root) % 12).collect();
        intervals.sort_unstable();
        if let Some((kind, _)) = KINDS.iter().find(|(_, template)| intervals == *template) {
            return Some(Chord { root, kind, bass: (lowest != root).then_some(lowest) });
        }
    }
    None
}

/// The chords found where notes are struck together, by beat: two notes a
/// fifth apart make a power chord, three or more make the others. A chord
/// that repeats the one before it is not named again.
pub fn detect(notes: &[Note]) -> Vec<ChordMark> {
    let mut order: Vec<&Note> = notes.iter().collect();
    order.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut found: Vec<ChordMark> = Vec::new();
    let mut index = 0;
    while index < order.len() {
        let start = order[index].start;
        let mut end = index;
        while end < order.len() && (order[end].start - start).abs() < 1e-6 {
            end += 1;
        }
        let pitches: Vec<u8> = order[index..end].iter().map(|n| n.pitch).collect();
        if let Some(chord) = name(&pitches) {
            let text = chord.text();
            if (pitches.len() >= 3 || chord.kind == "5")
                && found.last().is_none_or(|last| last.name != text) {
                    found.push(ChordMark { beat: start, name: text });
                }
        }
        index = end;
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_are_named_from_their_pitches() {
        let text = |pitches: &[u8]| name(pitches).map(|c| c.text());
        assert_eq!(text(&[60, 64, 67]), Some("C".to_owned()));
        assert_eq!(text(&[57, 60, 64]), Some("Am".to_owned()));
        assert_eq!(text(&[64, 67, 72]), Some("C/E".to_owned()), "the lowest note is not the root");
        assert_eq!(text(&[62, 65, 69, 72]), Some("Dm7".to_owned()));
        assert_eq!(text(&[67, 71, 74, 77]), Some("G7".to_owned()));
        assert_eq!(text(&[40, 47]), Some("E5".to_owned()));
        assert_eq!(text(&[62, 67, 69]), Some("Dsus4".to_owned()));
        assert_eq!(text(&[60, 63, 66, 69]), Some("Cdim7".to_owned()));
        assert_eq!(text(&[60, 64, 67, 72, 76]), Some("C".to_owned()), "doubled notes change nothing");
        assert_eq!(text(&[60]), None);
        assert_eq!(text(&[60, 61]), None);
        assert_eq!(text(&[]), None);
    }

    #[test]
    fn chords_are_found_where_notes_are_struck_together() {
        let note = |start: f64, pitch: u8| Note { start, length: 1.0, pitch, velocity: 100, string: None, ..Default::default() };
        let notes = vec![note(0.0, 60), note(0.0, 64), note(0.0, 67), note(1.0, 62), note(2.0, 57), note(2.0, 60), note(2.0, 64), note(3.0, 57), note(3.0, 60), note(3.0, 64), note(4.0, 40), note(4.0, 47)];
        let found = detect(&notes);
        let names: Vec<(f64, &str)> = found.iter().map(|c| (c.beat, c.name.as_str())).collect();
        assert_eq!(names, [(0.0, "C"), (2.0, "Am"), (4.0, "E5")], "a lone note is no chord, a repeated chord is named once");
    }

    #[test]
    fn chord_names_are_read_back() {
        let chord = |text: &str| Chord::parse(text);
        assert_eq!(chord("C"), Some(Chord { root: 0, kind: "", bass: None }));
        assert_eq!(chord("F#m7"), Some(Chord { root: 6, kind: "m7", bass: None }));
        assert_eq!(chord("Bb"), Some(Chord { root: 10, kind: "", bass: None }));
        assert_eq!(chord("G#5"), Some(Chord { root: 8, kind: "5", bass: None }));
        assert_eq!(chord("C/E"), Some(Chord { root: 0, kind: "", bass: Some(4) }));
        assert_eq!(chord("N.C."), None);
        assert_eq!(chord("Cxyz"), None);
        assert_eq!(chord(""), None);
        assert_eq!(Chord::parse("G7").unwrap().tones(), [7, 11, 2, 5]);
        assert_eq!(Chord::parse("C/D").unwrap().tones(), [0, 4, 7, 2]);
    }

    #[test]
    fn chords_are_laid_on_the_strings_as_guitarists_play_them() {
        const GUITAR: [u8; 6] = [64, 59, 55, 50, 45, 40];
        // Frets from the high string to the low one, as the tuning is written.
        let shape = |text: &str| Chord::parse(text).unwrap().shape(&GUITAR).unwrap();
        let frets = |text: &str| shape(text).frets;
        let x = None;
        assert_eq!(frets("C"), [Some(0), Some(1), Some(0), Some(2), Some(3), x]);
        assert_eq!(frets("G"), [Some(3), Some(0), Some(0), Some(0), Some(2), Some(3)]);
        assert_eq!(frets("E"), [Some(0), Some(0), Some(1), Some(2), Some(2), Some(0)]);
        assert_eq!(frets("Am"), [Some(0), Some(1), Some(2), Some(2), Some(0), x]);
        assert_eq!(frets("D"), [Some(2), Some(3), Some(2), Some(0), x, x]);
        assert_eq!(frets("F"), [Some(1), Some(1), Some(2), Some(3), Some(3), Some(1)], "a barre");
        assert_eq!(frets("F#"), [Some(2), Some(2), Some(3), Some(4), Some(4), Some(2)]);
        assert_eq!(frets("B"), [Some(2), Some(4), Some(4), Some(4), Some(2), x]);
        assert_eq!(shape("C").base, 1, "near the nut");
        assert!(Chord::parse("Cdim7").unwrap().shape(&[43, 38, 33, 28]).is_some(), "four notes on four strings");
        assert_eq!(Chord::parse("C").unwrap().shape(&[]), None);
        // From notes already on the strings: the grid starts where they are.
        assert_eq!(Shape::of_notes(&[Some(0), Some(1), Some(0), Some(2), Some(3), None]).unwrap().base, 1);
        assert_eq!(Shape::of_notes(&[None, None, None, Some(7), Some(7), Some(5)]).unwrap().base, 5);
        assert_eq!(Shape::of_notes(&[Some(0), Some(0), None, None, Some(7), Some(5)]).unwrap().base, 5, "an open string beside a high position");
        assert_eq!(Shape::of_notes(&[Some(1), None, None, None, None, Some(5)]), None, "too wide for the grid");
        assert_eq!(Shape::of_notes(&[Some(3), None, None, None, None, None]), None, "one note is no chord");
    }

    #[test]
    fn names_written_as_printed_music_does_are_understood() {
        let guitar = [64, 59, 55, 50, 45, 40];
        let b_flat = Chord::parse("B♭7sus4").unwrap();
        assert_eq!((b_flat.root, b_flat.kind, b_flat.text()), (10, "7sus4", "A#7sus4".to_owned()));
        assert!(b_flat.shape(&guitar).is_some(), "a grid for it");
        assert!(Chord::parse("B♭7").unwrap().shape(&guitar).is_some());
        for (written, kind) in [("CM7", "maj7"), ("CΔ7", "maj7"), ("Cmin", "m"), ("C-7", "m7"), ("Cø", "m7b5"), ("Cm7(b5)", "m7b5"), ("C°", "dim"), ("C+", "aug"), ("Csus", "sus4"), ("C9", "9"), ("Cmaj9", "maj9"), ("F♯m", "m")] {
            assert_eq!(Chord::parse(written).map(|c| c.kind), Some(kind), "{written}");
        }
        assert_eq!(Chord::parse("F♯m/C♯").unwrap().bass, Some(1));
        assert_eq!(plain(" B♭7 "), "Bb7");
        assert_eq!(name(&[48, 52, 55, 58, 62]).map(|c| c.text()), Some("C9".to_owned()));
    }
}
