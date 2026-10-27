//! The bottom panel: sound controls and the editors (notes, score, audio, drummer).

use crate::i18n::tr;
use crate::theme::record;
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Shape};
use og_engine::chords::{Chord, Shape as ChordShape, GRID_FRETS};
use og_engine::patches;
use og_engine::{
    AmpModel, ArpMode, Content, DrumKit, DrumStyle, HatMode, Note, Pedal, PedalKind, Perc, ReverbKind, Slide,
    Sound, Vibrato, Wave,
};

use crate::app::{palette, snap, stroke, track_color, App, BottomTab, Dialog, Drag, NoteView, Panel, Side, EDGE, NOTE_LETTERS, SNAP};

const ROW_H: f32 = 12.0;
const KEYS_W: f32 = 58.0;
const NOTE_LENGTHS: [(f64, &str); 6] =
    [(0.125, "1/32"), (0.25, "1/16"), (0.5, "1/8"), (1.0, "1/4"), (2.0, "1/2"), (4.0, "1/1")];
/// Where the playhead of a note editor stays while the notes scroll under
/// it: this share of the width of the view, from its left edge.
pub const PLAYHEAD_AT: f64 = 0.2;
/// The zoom of the note editors is the pixels a beat takes; this many is
/// 100 %. It goes as far either way as wanted, within `ZOOM_RANGE`, by
/// `ZOOM_STEP` at each press of "-" or "+".
pub const ZOOM_UNIT: f64 = 40.0;
pub const ZOOM_RANGE: std::ops::RangeInclusive<f64> = 0.25..=4000.0;
const ZOOM_STEP: f64 = 1.25;
/// The heights the band of the words may be dragged to.
pub const WORDS_LEAST: f32 = 60.0;
pub const WORDS_MOST: f32 = 600.0;
/// Bars a line of the score or the piano roll shows until the user zooms.
pub const CLOSEST_BARS: f64 = 4.0;
/// Beats a line of a tablature shows until the user zooms.
pub const TAB_CLOSEST: f64 = 4.0;

/// The size of the fret numbers of a tablature until the user changes it,
/// and the sizes they may choose.
pub const TAB_TEXT: f32 = 11.5;
pub const TAB_TEXT_RANGE: std::ops::RangeInclusive<f32> = 8.0..=28.0;
/// At that usual size: the space between two strings of a tablature, the
/// room over them for the chord grids and the chord names, and how far in
/// from the left the first beat stands, for its fret number to be whole.
const TAB_GAP: f32 = 18.0;
pub const TAB_INSET: f32 = 14.0;
const TAB_CHORDS: f32 = 52.0;


// ----- the lines of a score or a tablature -------------------------------------------

/// One line of a score or a tablature: the beats of the region's cycle it
/// holds, and its top, from the top of the editor.
#[derive(Clone, Debug, PartialEq)]
pub struct System {
    pub from: f64,
    pub to: f64,
    pub top: f32,
}

/// How a note editor lays its beats out: one line that scrolls sideways, or
/// lines of whole bars one under the other, as on a printed page.
#[derive(Clone, Debug, PartialEq)]
pub struct Lines {
    /// Pixels a beat.
    pub ppb: f64,
    pub systems: Vec<System>,
    /// Height of them all, at least that of the view.
    pub height: f32,
    /// Several lines of whole bars: the whole region in the full window, two
    /// of its lines otherwise.
    pub paged: bool,
    /// The line the song is being played in.
    pub current: Option<usize>,
}

/// Height of a line of the score.
pub const SCORE_LINE: f32 = 210.0;
/// Position on the staff: seven steps per octave, from the C of the lowest octave.
const MIDDLE_C: i32 = 35;
const TREBLE: [i32; 5] = [37, 39, 41, 43, 45];
const BASS: [i32; 5] = [25, 27, 29, 31, 33];
/// Pixels between two steps of the staff: half the space between two lines.
const HALF: f32 = 5.0;

/// A key signature: so many sharps (positive) or flats (negative).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Key(pub i32);

/// What is written before a note head.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Accidental {
    None,
    Sharp,
    Flat,
    Natural,
}

impl Key {
    /// The signature of a key of the project: a note (0 = C … 11 = B), major or minor.
    pub fn of(key: u8, minor: bool) -> Key {
        let major = if minor { (key + 3) % 12 } else { key };
        // Each fifth up from C adds a sharp; past six, flats are fewer.
        let fifths = (i32::from(major) * 7).rem_euclid(12);
        Key(if fifths <= 6 { fifths } else { fifths - 12 })
    }

    fn count(self) -> usize {
        self.0.unsigned_abs() as usize
    }

    /// The letters the signature alters (0 = C … 6 = B), in the order they are written.
    fn letters(self) -> &'static [usize] {
        const SHARPS: [usize; 7] = [3, 0, 4, 1, 5, 2, 6];
        const FLATS: [usize; 7] = [6, 2, 5, 1, 4, 0, 3];
        if self.0 >= 0 { &SHARPS[..self.count()] } else { &FLATS[..self.count()] }
    }

    /// The steps of the treble staff its signs sit on, in order; the bass
    /// staff has them fourteen steps lower.
    fn steps(self) -> Vec<i32> {
        const SHARPS: [i32; 7] = [45, 42, 46, 43, 40, 44, 41];
        const FLATS: [i32; 7] = [41, 44, 40, 43, 39, 42, 38];
        if self.0 >= 0 { SHARPS[..self.count()].to_vec() } else { FLATS[..self.count()].to_vec() }
    }

    fn sign(self) -> Accidental {
        if self.0 < 0 { Accidental::Flat } else { Accidental::Sharp }
    }

    /// How the key alters a letter: a semitone up, down, or not.
    fn alters(self, letter: usize) -> i32 {
        if self.letters().contains(&letter) { self.0.signum() } else { 0 }
    }

    /// Where a pitch is written: its step on the staff, and the sign before
    /// it when the signature does not already say it. Sharp keys write the
    /// notes between the letters as sharps, flat keys as flats.
    pub fn spell(self, pitch: u8) -> (i32, Accidental) {
        const SHARP_LETTERS: [(usize, i32); 12] = [(0, 0), (0, 1), (1, 0), (1, 1), (2, 0), (3, 0), (3, 1), (4, 0), (4, 1), (5, 0), (5, 1), (6, 0)];
        const FLAT_LETTERS: [(usize, i32); 12] = [(0, 0), (1, -1), (1, 0), (2, -1), (2, 0), (3, 0), (4, -1), (4, 0), (5, -1), (5, 0), (6, -1), (6, 0)];
        let table = if self.0 < 0 { &FLAT_LETTERS } else { &SHARP_LETTERS };
        let (letter, alter) = table[usize::from(pitch % 12)];
        let step = i32::from(pitch / 12) * 7 + letter as i32;
        let sign = match (alter, self.alters(letter)) {
            (alter, in_key) if alter == in_key => Accidental::None,
            (0, _) => Accidental::Natural,
            (1, _) => Accidental::Sharp,
            _ => Accidental::Flat,
        };
        (step, sign)
    }

    /// The pitch of a step of the staff in this key.
    pub fn pitch_of(self, step: i32) -> u8 {
        const SEMIS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
        let letter = step.rem_euclid(7) as usize;
        (step.div_euclid(7) * 12 + SEMIS[letter] + self.alters(letter)).clamp(0, 127) as u8
    }
}

/// The written value of a note: whole (4 beats) to sixty-fourth, dotted or
/// not, or a third of one in a triplet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Value {
    /// Beats of the plain value: 4, 2, 1, 0.5…
    pub base: f64,
    pub dotted: bool,
    pub triplet: bool,
}

impl Value {
    pub fn of(length: f64) -> Value {
        const BASES: [f64; 7] = [4.0, 2.0, 1.0, 0.5, 0.25, 0.125, 0.0625];
        let near = |a: f64, b: f64| (a - b).abs() < b * 0.02;
        for base in BASES {
            if near(length, base) {
                return Value { base, dotted: false, triplet: false };
            }
            if near(length, base * 1.5) {
                return Value { base, dotted: true, triplet: false };
            }
            if near(length * 3.0, base * 2.0) {
                return Value { base, dotted: false, triplet: true };
            }
        }
        // Anything else is written as the longest value that fits.
        let base = BASES.into_iter().find(|base| *base <= length * 1.02).unwrap_or(0.0625);
        Value { base, dotted: false, triplet: false }
    }

    /// Beats the value lasts.
    pub fn beats(self) -> f64 {
        if self.dotted {
            self.base * 1.5
        } else if self.triplet {
            self.base * 2.0 / 3.0
        } else {
            self.base
        }
    }

    /// Whether a length is written as a single value.
    pub fn exact(length: f64) -> bool {
        (Value::of(length).beats() - length).abs() < length * 0.02
    }
}

/// How the notes are written on a score: each cut at the bar lines it
/// crosses, and a length that is no single value written as values tied
/// together, the longest first. Gives the pieces, the note each comes
/// from, and the piece each one is tied from. `first_bar` is where a bar
/// starts, in beats of the region.
pub fn score_pieces(notes: &[Note], bar_len: f64, first_bar: f64) -> (Vec<Note>, Vec<usize>, Vec<Option<usize>>) {
    const WRITTEN: [f64; 12] = [4.0, 3.0, 2.0, 1.5, 1.0, 0.75, 0.5, 0.375, 0.25, 0.1875, 0.125, 0.0625];
    let (mut pieces, mut owner, mut tied) = (Vec::new(), Vec::new(), Vec::new());
    for (i, note) in notes.iter().enumerate() {
        let end = note.start + note.length;
        let mut at = note.start;
        let mut previous: Option<usize> = None;
        while end - at > 1e-6 {
            // To the next bar line, then values that fit, longest first.
            let bar_end = first_bar + ((at - first_bar) / bar_len + 1e-9).floor() * bar_len + bar_len;
            let stretch = end.min(bar_end) - at;
            let length = if Value::exact(stretch) {
                // The note's own length when it is written whole.
                if at == note.start && end <= bar_end { note.length } else { stretch }
            } else {
                WRITTEN.into_iter().find(|v| *v <= stretch + 1e-6).unwrap_or(stretch)
            };
            pieces.push(Note { start: at, length, ..note.clone() });
            owner.push(i);
            tied.push(previous);
            previous = Some(pieces.len() - 1);
            at += length;
            // A crumb shorter than any written value stays with the last piece.
            let left = end.min(bar_end) - at;
            if left > 1e-6 && left < 0.0625 - 1e-6 {
                if let Some(last) = pieces.last_mut() {
                    last.length += left;
                }
                at += left;
            }
        }
    }
    (pieces, owner, tied)
}

impl Value {
    /// Flags (or beams) on the stem: one for an eighth, two for a sixteenth…
    pub fn flags(self) -> usize {
        match self.base {
            b if b >= 1.0 => 0,
            b if b >= 0.5 => 1,
            b if b >= 0.25 => 2,
            b if b >= 0.125 => 3,
            _ => 4,
        }
    }
}

/// Notes struck together on one staff: they share a stem.
struct Stem {
    start: f64,
    treble: bool,
    notes: Vec<usize>,
    value: Value,
    low: i32,
    high: i32,
}

/// Writes an accidental before a note head.
fn accidental(painter: &egui::Painter, at: Pos2, sign: Accidental, color: Color32) {
    let font = FontId::proportional(13.0);
    match sign {
        Accidental::None => {}
        Accidental::Sharp => {
            painter.text(at - vec2(9.0, 0.0), Align2::RIGHT_CENTER, "#", font, color);
        }
        Accidental::Flat => {
            painter.text(at - vec2(9.0, 1.0), Align2::RIGHT_CENTER, "b", font, color);
        }
        Accidental::Natural => {
            let x = at.x - 13.0;
            let thin = stroke(1.2, color);
            painter.line_segment([pos2(x, at.y - 6.0), pos2(x, at.y + 3.0)], thin);
            painter.line_segment([pos2(x + 4.0, at.y - 3.0), pos2(x + 4.0, at.y + 6.0)], thin);
            painter.line_segment([pos2(x, at.y - 2.0), pos2(x + 4.0, at.y - 3.5)], thin);
            painter.line_segment([pos2(x, at.y + 3.0), pos2(x + 4.0, at.y + 1.5)], thin);
        }
    }
}

/// Draws a rest of `base` beats at `x`, on the staff whose middle line is
/// at `mid`.
fn rest(painter: &egui::Painter, x: f32, mid: f32, base: f64, color: Color32) {
    let thick = stroke(1.6, color);
    if base >= 4.0 {
        // A whole rest hangs under the fourth line.
        painter.rect_filled(Rect::from_min_size(pos2(x - 4.0, mid - 2.0 * HALF), vec2(8.0, 4.0)), 0.0, color);
    } else if base >= 2.0 {
        // A half rest sits on the middle line.
        painter.rect_filled(Rect::from_min_size(pos2(x - 4.0, mid - 4.0), vec2(8.0, 4.0)), 0.0, color);
    } else if base >= 1.0 {
        let zigzag = [pos2(x - 2.0, mid - 9.0), pos2(x + 3.0, mid - 3.0), pos2(x - 2.0, mid + 2.0), pos2(x + 3.0, mid + 7.0), pos2(x - 1.0, mid + 9.0)];
        painter.add(Shape::line(zigzag.to_vec(), thick));
    } else {
        // An eighth rest: a slanted stem and a hook with a dot; a dot more
        // for each halving.
        let dots = (1.0 / base).log2().round().max(1.0) as usize;
        let top = mid - 6.0;
        let bottom = mid + 6.0 + 4.0 * (dots - 1) as f32;
        painter.line_segment([pos2(x + 3.0, top), pos2(x - 2.0 - 1.5 * (dots - 1) as f32, bottom)], thick);
        for d in 0..dots {
            let y = top + 1.0 + 5.0 * d as f32;
            let hook_x = x + 3.0 - 1.4 * (y - top) / 2.0;
            painter.circle_filled(pos2(hook_x - 4.0, y), 1.6, color);
            painter.line_segment([pos2(hook_x - 3.0, y + 1.0), pos2(hook_x, y - 1.0)], stroke(1.2, color));
        }
    }
}

/// Where the rests of a staff go in a bar: the stretches where nothing
/// sounds, from `sounding` (start and end of each note of the staff), cut
/// on the grid of sixteenths and filled from the longest value that fits.
/// A bar with no note gets a whole rest at its start.
pub fn rests_of(sounding: &[(f64, f64)], bar_start: f64, bar_end: f64) -> Vec<(f64, f64)> {
    const GRID: f64 = 0.25;
    let mut busy: Vec<(f64, f64)> = sounding.iter().filter(|(s, e)| *e > bar_start + 1e-6 && *s < bar_end - 1e-6).map(|(s, e)| (s.max(bar_start), e.min(bar_end))).collect();
    busy.sort_by(|a, b| a.0.total_cmp(&b.0));
    if busy.is_empty() {
        return vec![(bar_start, 4.0)];
    }
    let mut gaps = Vec::new();
    let mut cursor = bar_start;
    let fill = |from: f64, to: f64, gaps: &mut Vec<(f64, f64)>| {
        let mut at = ((from - bar_start) / GRID).ceil() * GRID + bar_start;
        let end = ((to - bar_start) / GRID).floor() * GRID + bar_start;
        while end - at >= GRID - 1e-6 {
            let room = end - at + 1e-6;
            // A value starts on its own multiple within the bar, as written music does.
            let value = [2.0, 1.0, 0.5, 0.25].into_iter().find(|v| *v <= room && ((at - bar_start) / v).fract().abs() < 1e-6).unwrap_or(GRID);
            gaps.push((at, value));
            at += value;
        }
    };
    for (start, end) in busy {
        if start - cursor >= GRID - 1e-6 {
            fill(cursor, start, &mut gaps);
        }
        cursor = cursor.max(end);
    }
    if bar_end - cursor >= GRID - 1e-6 {
        fill(cursor, bar_end, &mut gaps);
    }
    gaps
}

/// Draws the notes of one line of the score, from `from` to `to`: heads with
/// their accidentals and ledger lines, one stem for the notes struck
/// together on a staff, beams over the eighths and shorter notes of a beat,
/// flags on the others, dots, and a 3 over the triplets. Gives the box of
/// each head drawn (for what is written around the notes) and adds the heads
/// to `heads` (for the mouse).
#[allow(clippy::too_many_arguments)]
fn score_notes(painter: &egui::Painter, notes: &[Note], from: f64, to: f64, bar_len: f64, region_start: f64, beat_unit: u32, x_of: &dyn Fn(f64) -> f32, y_of: &dyn Fn(i32) -> f32, key: Key, ink: Color32, selected: &[usize], heads: &mut Vec<(usize, Pos2)>) -> Vec<Option<Rect>> {
    const STEM: f32 = 28.0;
    let mut labels: Vec<Option<Rect>> = vec![None; notes.len()];
    let faint = stroke(1.0, ink.gamma_multiply(0.6));
    // The notes as they are written: cut at the bar lines, tied together.
    let first_bar = ((region_start / bar_len).ceil() * bar_len - region_start).rem_euclid(bar_len);
    let (pieces, owner, tied) = score_pieces(notes, bar_len, first_bar);
    let notes = &pieces[..];
    let selected: Vec<usize> = (0..pieces.len()).filter(|p| selected.contains(&owner[*p])).collect();
    let selected = &selected[..];
    let mut piece_heads: Vec<Option<Pos2>> = vec![None; pieces.len()];
    let mut piece_up: Vec<bool> = vec![true; pieces.len()];

    // The notes struck together on a staff share a stem.
    let mut order: Vec<usize> = (0..notes.len()).filter(|i| notes[*i].start >= from - 1e-9 && notes[*i].start < to - 1e-9).collect();
    order.sort_by(|a, b| notes[*a].start.total_cmp(&notes[*b].start).then(notes[*a].pitch.cmp(&notes[*b].pitch)));
    let mut stems: Vec<Stem> = Vec::new();
    for i in order {
        let note = &notes[i];
        let (step, _) = key.spell(note.pitch);
        let treble = step >= MIDDLE_C;
        let value = Value::of(note.length);
        match stems.iter_mut().rev().take(2).find(|s| (s.start - note.start).abs() < 1e-6 && s.treble == treble) {
            Some(stem) => {
                stem.notes.push(i);
                stem.low = stem.low.min(step);
                stem.high = stem.high.max(step);
                if value.base < stem.value.base {
                    stem.value = value;
                }
            }
            None => stems.push(Stem { start: note.start, treble, notes: vec![i], value, low: step, high: step }),
        }
    }

    // The stems of a beat that are eighths or shorter and follow one
    // another without a gap are beamed together; the others get flags.
    let beat_group = |start: f64| {
        let in_bar = (region_start + start).rem_euclid(bar_len);
        let unit = if beat_unit == 8 { 1.5 } else { 1.0 };
        (in_bar / unit).floor()
    };
    let mut beamed: Vec<Vec<usize>> = Vec::new();
    for treble in [true, false] {
        let mut run: Vec<usize> = Vec::new();
        for (s, stem) in stems.iter().enumerate().filter(|(_, s)| s.treble == treble) {
            let joins = run.last().is_some_and(|&last| {
                let before = &stems[last];
                let shortest = before.notes.iter().map(|i| notes[*i].length).fold(f64::MAX, f64::min);
                (before.start + shortest - stem.start).abs() < 1e-3 && beat_group(before.start) == beat_group(stem.start)
            });
            if stem.value.flags() == 0 || !joins {
                if run.len() > 1 {
                    beamed.push(std::mem::take(&mut run));
                } else {
                    run.clear();
                }
            }
            if stem.value.flags() > 0 {
                run.push(s);
            }
        }
        if run.len() > 1 {
            beamed.push(run);
        }
    }
    let in_beam: Vec<Option<usize>> = (0..stems.len()).map(|s| beamed.iter().position(|run| run.contains(&s))).collect();

    // Stems up when the notes sit low on their staff; a beam takes the side
    // most of its notes want.
    let up_alone = |stem: &Stem| {
        let staff_mid = if stem.treble { 41 } else { 29 };
        (stem.high - staff_mid) < (staff_mid - stem.low)
    };
    let up_of: Vec<bool> = stems
        .iter()
        .enumerate()
        .map(|(s, stem)| match in_beam[s] {
            Some(b) => beamed[b].iter().filter(|s| up_alone(&stems[**s])).count() * 2 >= beamed[b].len(),
            None => up_alone(stem),
        })
        .collect();
    // Where the stems of a beam end: level, past the farthest head.
    let tip_of = |s: usize| -> f32 {
        let up = up_of[s];
        match in_beam[s] {
            Some(b) => beamed[b].iter().map(|s| if up { y_of(stems[*s].high) - STEM } else { y_of(stems[*s].low) + STEM }).fold(if up { f32::MAX } else { f32::MIN }, if up { f32::min } else { f32::max }),
            None => {
                if up {
                    y_of(stems[s].high) - STEM
                } else {
                    y_of(stems[s].low) + STEM
                }
            }
        }
    };
    let stem_x = |s: usize| {
        let x = x_of(stems[s].start);
        if up_of[s] { x + 5.0 } else { x - 5.0 }
    };

    for (s, stem) in stems.iter().enumerate() {
        let up = up_of[s];
        let x = x_of(stem.start);
        let color = if stem.notes.iter().any(|i| selected.contains(i)) { palette(1) } else { ink };
        for &i in &stem.notes {
            let note = &notes[i];
            let (step, sign) = key.spell(note.pitch);
            let at = pos2(x, y_of(step));
            let own = if selected.contains(&i) { palette(1) } else { ink };
            // Ledger lines outside and between the staves.
            let ledger = |s: i32| painter.line_segment([pos2(at.x - 9.0, y_of(s)), pos2(at.x + 9.0, y_of(s))], faint);
            if (MIDDLE_C - 1..=MIDDLE_C + 1).contains(&step) {
                ledger(MIDDLE_C);
            }
            let mut s = 47;
            while s <= step {
                ledger(s);
                s += 2;
            }
            let mut s = 23;
            while s >= step {
                ledger(s);
                s -= 2;
            }
            // Half notes and longer are hollow.
            let radii = vec2(5.5, 4.0);
            if stem.value.base >= 2.0 {
                painter.add(Shape::ellipse_stroke(at, radii, stroke(1.6, own)));
            } else {
                painter.add(Shape::ellipse_filled(at, radii, own));
            }
            if stem.value.dotted {
                // In the space over a line, beside the head.
                let dy = if step % 2 != 0 { -HALF } else { 0.0 };
                painter.circle_filled(pos2(at.x + 9.0, at.y + dy), 1.5, own);
            }
            // A note tied from the one before keeps its sign from there.
            if tied[i].is_none() {
                accidental(painter, at, sign, own);
            }
            heads.push((owner[i], at));
            piece_heads[i] = Some(at);
            piece_up[i] = up;
            if labels[owner[i]].is_none() {
                labels[owner[i]] = Some(Rect::from_center_size(at, vec2(11.0, 8.0)));
            }
        }
        // Whole notes have no stem.
        if stem.value.base >= 4.0 {
            continue;
        }
        let sx = stem_x(s);
        let tip = tip_of(s);
        let base = if up { y_of(stem.low) } else { y_of(stem.high) };
        painter.line_segment([pos2(sx, base), pos2(sx, tip)], stroke(1.3, color));
        if in_beam[s].is_none() {
            // One flag for an eighth, two for a sixteenth and so on.
            for f in 0..stem.value.flags() {
                let y = tip + if up { f as f32 * 6.0 } else { -(f as f32) * 6.0 };
                let dy = if up { 8.0 } else { -8.0 };
                painter.line_segment([pos2(sx, y), pos2(sx + 7.0, y + dy)], stroke(1.6, color));
            }
        }
    }

    // The beams: a thick line over the run, then shorter ones under it for
    // the sixteenths and shorter, reaching the neighbours that are as short.
    for run in &beamed {
        let (first, last) = (run[0], run[run.len() - 1]);
        let up = up_of[first];
        let tip = tip_of(first);
        let color = if run.iter().any(|s| stems[*s].notes.iter().any(|i| selected.contains(i))) { palette(1) } else { ink };
        let thick = stroke(2.6, color);
        painter.line_segment([pos2(stem_x(first), tip), pos2(stem_x(last), tip)], thick);
        let most = run.iter().map(|s| stems[*s].value.flags()).max().unwrap_or(1);
        for level in 2..=most {
            let y = tip + if up { 5.0 * (level - 1) as f32 } else { -5.0 * (level - 1) as f32 };
            for (k, &s) in run.iter().enumerate() {
                if stems[s].value.flags() < level {
                    continue;
                }
                let x = stem_x(s);
                let before = k.checked_sub(1).map(|k| run[k]).filter(|s| stems[*s].value.flags() >= level);
                let after = run.get(k + 1).copied().filter(|s| stems[*s].value.flags() >= level);
                match (before, after) {
                    (_, Some(next)) => {
                        painter.line_segment([pos2(x, y), pos2(stem_x(next), y)], thick);
                    }
                    (Some(_), None) => {}
                    // Alone at this level: a short beam towards the note it belongs with.
                    (None, None) => {
                        let towards = if k + 1 < run.len() { 7.0 } else { -7.0 };
                        painter.line_segment([pos2(x, y), pos2(x + towards, y)], thick);
                    }
                }
            }
        }
    }

    // Triplets: a 3 over the notes of each group, in a bracket unless one
    // beam holds them all.
    for treble in [true, false] {
        let mut group: Vec<usize> = Vec::new();
        let mut filled = 0.0;
        let triplets: Vec<usize> = (0..stems.len()).filter(|s| stems[*s].treble == treble && stems[*s].value.triplet).collect();
        for (k, &s) in triplets.iter().enumerate() {
            let follows = group.last().is_some_and(|&last| (stems[last].start + stems[last].value.beats() - stems[s].start).abs() < 1e-3);
            if !follows {
                group.clear();
                filled = 0.0;
            }
            group.push(s);
            filled += stems[s].value.beats();
            let whole = stems[group[0]].value.base * 2.0;
            let next_follows = triplets.get(k + 1).is_some_and(|&n| (stems[s].start + stems[s].value.beats() - stems[n].start).abs() < 1e-3);
            if filled + 1e-6 >= whole || !next_follows {
                let (first, last) = (group[0], s);
                let up = up_of[first];
                let color = if group.iter().any(|g| stems[*g].notes.iter().any(|i| selected.contains(i))) { palette(1) } else { ink };
                let font = FontId::proportional(9.5);
                let one_beam = in_beam[first].is_some() && group.iter().all(|g| in_beam[*g] == in_beam[first]);
                let x0 = stem_x(first) - if stems[first].value.base >= 4.0 { 6.0 } else { 3.0 };
                let x1 = stem_x(last) + 3.0;
                let tips: Vec<f32> = group.iter().map(|g| if stems[*g].value.base >= 4.0 { y_of(stems[*g].high) - 6.0 } else { tip_of(*g) }).collect();
                let y = if up { tips.iter().copied().fold(f32::MAX, f32::min) - 8.0 } else { tips.iter().copied().fold(f32::MIN, f32::max) + 8.0 };
                let middle = (x0 + x1) / 2.0;
                if one_beam {
                    painter.text(pos2(middle, y), Align2::CENTER_CENTER, "3", font, color);
                } else {
                    let thin = stroke(1.0, color);
                    let hook = if up { 4.0 } else { -4.0 };
                    painter.line_segment([pos2(x0, y + hook), pos2(x0, y)], thin);
                    painter.line_segment([pos2(x0, y), pos2(middle - 5.0, y)], thin);
                    painter.line_segment([pos2(middle + 5.0, y), pos2(x1, y)], thin);
                    painter.line_segment([pos2(x1, y), pos2(x1, y + hook)], thin);
                    painter.text(pos2(middle, y), Align2::CENTER_CENTER, "3", font, color);
                }
                group.clear();
                filled = 0.0;
            }
        }
    }

    // Ties: an arc from a head to the one it holds on to, on the side away
    // from the stem; to the end of the line, or from its start, when the
    // other head is on another line.
    let line_start = x_of(from) - 14.0;
    let line_end = x_of(to);
    for (p, previous) in tied.iter().enumerate() {
        let Some(q) = *previous else { continue };
        let (from_head, to_head) = (piece_heads[q], piece_heads[p]);
        let (Some(y), up) = (from_head.or(to_head).map(|h| h.y), piece_up[if from_head.is_some() { q } else { p }]) else { continue };
        let x0 = from_head.map_or(line_start, |h| h.x + 7.0);
        let x1 = to_head.map_or(line_end, |h| h.x - 7.0);
        if x1 <= x0 + 2.0 {
            continue;
        }
        let side = if up { 1.0 } else { -1.0 };
        let color = if selected.contains(&p) { palette(1) } else { ink };
        let bow = ((x1 - x0) * 0.12).clamp(3.0, 7.0);
        let arc: Vec<Pos2> = (0..=16)
            .map(|k| {
                let t = k as f32 / 16.0;
                pos2(x0 + (x1 - x0) * t, y + side * (4.0 + bow * 4.0 * t * (1.0 - t)))
            })
            .collect();
        painter.add(Shape::line(arc, stroke(1.2, color)));
    }
    labels
}

/// Open strings of a tablature, highest first.
fn tab_tuning(bass: bool) -> &'static [u8] {
    if bass {
        &[43, 38, 33, 28]
    } else {
        &[64, 59, 55, 50, 45, 40]
    }
}

/// Where each note goes on the strings: the string (0 is the highest) and
/// the fret. A note that was written on a string of the tablature it came
/// from stays there; any other goes on the string that gives the lowest
/// fret among those not struck at the same moment. None for a note below
/// the lowest string or past the twenty-fourth fret.
pub fn tab_positions(notes: &[Note], tuning: &[u8]) -> Vec<Option<(usize, u8)>> {
    let mut out = vec![None; notes.len()];
    let mut struck: Vec<(usize, f64)> = Vec::new();
    // Notes that know their string first, then the others, those struck
    // together highest first, so that a chord spreads from the high strings down.
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|a, b| notes[*b].string.is_some().cmp(&notes[*a].string.is_some()).then(notes[*a].start.total_cmp(&notes[*b].start)).then(notes[*b].pitch.cmp(&notes[*a].pitch)));
    for ni in order {
        let note = &notes[ni];
        let free = |string: usize| !struck.iter().any(|(s, start)| *s == string && (note.start - start).abs() < 1e-6);
        if let Some(written) = note.string.map(usize::from).filter(|s| *s < tuning.len()) {
            let open = tuning[written];
            if note.pitch >= open && note.pitch - open <= 30 && free(written) {
                struck.push((written, note.start));
                out[ni] = Some((written, note.pitch - open));
                continue;
            }
        }
        let best = tuning
            .iter()
            .enumerate()
            .filter(|(string, open)| note.pitch >= **open && note.pitch - **open <= 24 && free(*string))
            .min_by_key(|(_, open)| note.pitch - **open)
            .map(|(string, open)| (string, note.pitch - open));
        if let Some((string, fret)) = best {
            struck.push((string, note.start));
            out[ni] = Some((string, fret));
        }
    }
    out
}

/// What a tablature writes for a note at a fret: a dead note is an x, a
/// harmonic the fret it is touched at in angle brackets, a ghost note its
/// fret in parentheses.
fn fret_text(note: &Note, fret: u8) -> String {
    let art = &note.articulation;
    match (art.dead, art.harmonic, art.ghost) {
        (true, _, _) => "x".to_owned(),
        (_, Some(touched), _) => format!("<{touched}>"),
        (_, _, true) => format!("({fret})"),
        _ => fret.to_string(),
    }
}

/// The room a fret number keeps around its digits, on the patch that breaks
/// the string.
const FRET_MARGIN: f32 = 4.0;

/// A chord name with its root in the language in use: "Em" is "Mim" in French.
/// The signs of printed music (♭, ♯) are written "b" and "#", as the font
/// of the program has no glyph for them.
pub fn chord_label(name: &str) -> String {
    let name = og_engine::chords::plain(name);
    let name = name.as_str();
    let bytes = name.as_bytes();
    let mut split = 0;
    if bytes.first().is_some_and(|b| (b'A'..=b'G').contains(b)) {
        split = 1;
        if matches!(bytes.get(1), Some(b'#') | Some(b'b')) {
            split = 2;
        }
    }
    if split == 0 {
        return name.to_owned();
    }
    let rest = &name[split..];
    // "C/E": the bass note is translated too.
    let rest = match rest.split_once('/') {
        Some((kind, bass)) => format!("{kind}/{}", chord_label(bass)),
        None => rest.to_owned(),
    };
    format!("{}{rest}", tr(&name[..split]))
}

/// The grid to show over a chord named on a tablature, from its beat to
/// `until`: what the strings play then, when that shows the chord within
/// four frets, or else a usual way to play it. For a name that is not
/// understood, such as "N.C.", only what is struck at its beat counts.
pub fn chord_grid_shape(name: &str, from: f64, until: f64, notes: &[Note], positions: &[Option<(usize, u8)>], tuning: &[u8]) -> Option<ChordShape> {
    let chord = Chord::parse(name);
    let tones = chord.as_ref().map(Chord::tones);
    // The first note played on each string, among the notes of the chord.
    let mut held: Vec<Option<(f64, u8)>> = vec![None; tuning.len()];
    for (note, place) in notes.iter().zip(positions) {
        let Some((string, fret)) = *place else { continue };
        let until = if chord.is_some() { until } else { from + 2e-6 };
        if note.start < from - 1e-6 || note.start >= until - 1e-6 {
            continue;
        }
        if tones.as_ref().is_some_and(|tones| !tones.contains(&(note.pitch % 12))) {
            continue;
        }
        if string < held.len() && held[string].is_none_or(|(start, _)| note.start < start) {
            held[string] = Some((note.start, fret));
        }
    }
    let frets: Vec<Option<u8>> = held.iter().map(|h| h.map(|(_, fret)| fret)).collect();
    ChordShape::of_notes(&frets).or_else(|| chord?.shape(tuning))
}

/// Where a chord is drawn along the top of a tablature: at its start, or,
/// once that has scrolled past the `left` edge, at the edge for as long as
/// its `width` fits before its `end`, which then pushes it out.
pub fn chord_place(start: f32, end: f32, width: f32, left: f32) -> f32 {
    if start >= left {
        start
    } else {
        left.min(end - width).max(start)
    }
}

/// The width of a chord grid of so many strings.
fn chord_grid_width(strings: usize, scale: f32) -> f32 {
    (12.0 + 6.0 * strings.saturating_sub(1) as f32) * scale
}

/// Draws a chord grid with its top left corner at `origin`: the strings
/// upright, the highest at the right, four frets, a dot where a finger
/// goes, a ring over an open string, a cross over one that is not played,
/// and the number of the first fret when it is not the nut. `scale` is its
/// size against the usual one.
fn chord_grid(painter: &egui::Painter, origin: Pos2, shape: &ChordShape, scale: f32, ink: Color32) {
    let across = 6.0 * scale;
    let down = 8.0 * scale;
    let margin = 12.0 * scale;
    let small = FontId::proportional(9.0 * scale);
    let count = shape.frets.len();
    let left = origin.x + margin;
    let top = origin.y + 9.0 * scale;
    let width = across * count.saturating_sub(1) as f32;
    let x_of = |string: usize| left + (count - 1 - string) as f32 * across;
    let faint = ink.gamma_multiply(0.75);
    for string in 0..count {
        painter.line_segment([pos2(x_of(string), top), pos2(x_of(string), top + down * f32::from(GRID_FRETS))], stroke(1.0, faint));
    }
    for fret in 0..=GRID_FRETS {
        let y = top + down * f32::from(fret);
        let nut = fret == 0 && shape.base == 1;
        painter.line_segment([pos2(left, y), pos2(left + width, y)], stroke(if nut { 2.5 * scale } else { 1.0 }, faint));
    }
    if shape.base > 1 {
        painter.text(pos2(left - 2.0 * scale, top + down * 0.5), Align2::RIGHT_CENTER, shape.base.to_string(), small.clone(), ink);
    }
    for (string, fret) in shape.frets.iter().enumerate() {
        let x = x_of(string);
        match *fret {
            None => {
                painter.text(pos2(x, top - 5.0 * scale), Align2::CENTER_CENTER, "×", small.clone(), ink);
            }
            Some(0) => {
                painter.circle_stroke(pos2(x, top - 5.0 * scale), 1.9 * scale, stroke(1.0, ink));
            }
            Some(fret) => {
                let row = f32::from(fret.saturating_sub(shape.base));
                painter.circle_filled(pos2(x, top + down * (row + 0.5)), 2.6 * scale, ink);
            }
        }
    }
}

fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

fn drum_name(pitch: u8) -> Option<&'static str> {
    Some(match pitch {
        36 => tr("Kick"),
        37 => tr("Rim"),
        38 => tr("Snare"),
        39 => tr("Clap"),
        41 => tr("Low tom"),
        42 => tr("Closed hat"),
        45 => tr("Mid tom"),
        46 => tr("Open hat"),
        48 => tr("High tom"),
        49 => tr("Crash"),
        51 => tr("Ride"),
        54 => tr("Tambourine"),
        56 => tr("Bell"),
        70 => tr("Shaker"),
        _ => return None,
    })
}

/// A rotary control: drag up or right to raise the value.
fn knob(ui: &mut egui::Ui, value: &mut f32, min: f32, max: f32, label: &str, log: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(vec2(58.0, 60.0), Sense::click_and_drag());
    let to_unit = |v: f32| if log { (v / min).ln() / (max / min).ln() } else { (v - min) / (max - min) };
    let from_unit = |u: f32| if log { min * (max / min).powf(u) } else { min + u * (max - min) };
    let mut changed = false;
    if resp.dragged() {
        let delta = resp.drag_delta();
        let unit = (to_unit(*value) + (delta.x - delta.y) / 140.0).clamp(0.0, 1.0);
        *value = from_unit(unit);
        changed = true;
    }
    let unit = to_unit(*value).clamp(0.0, 1.0);
    let centre = pos2(rect.center().x, rect.top() + 21.0);
    let radius = 15.0;
    let painter = ui.painter();
    let angle = |u: f32| (135.0 + 270.0 * u).to_radians();
    let arc = |from: f32, to: f32| -> Vec<Pos2> {
        (0..=24).map(|i| {
            let a = angle(from + (to - from) * i as f32 / 24.0);
            centre + vec2(a.cos(), a.sin()) * (radius + 4.0)
        }).collect()
    };
    painter.add(Shape::line(arc(0.0, 1.0), stroke(2.0, ui.visuals().widgets.noninteractive.bg_stroke.color)));
    painter.add(Shape::line(arc(0.0, unit), stroke(2.0, palette(1))));
    let fill = if resp.hovered() || resp.dragged() { ui.visuals().widgets.hovered.bg_fill } else { ui.visuals().widgets.inactive.bg_fill };
    painter.circle_filled(centre, radius, fill);
    let a = angle(unit);
    painter.line_segment([centre + vec2(a.cos(), a.sin()) * 5.0, centre + vec2(a.cos(), a.sin()) * radius], stroke(2.0, ui.visuals().text_color()));
    painter.text(pos2(rect.center().x, rect.bottom()), Align2::CENTER_BOTTOM, label, FontId::proportional(10.5), ui.visuals().text_color());
    let shown = if max - min > 50.0 { format!("{:.0}", *value) } else { format!("{:.2}", *value) };
    resp.on_hover_text(shown);
    changed
}

/// What the panels of the sound controls ask the application to do.
#[derive(Default)]
struct PanelActions {
    save_patch: bool,
    /// A capture, a cabinet or a room asked for (true) or taken away (false).
    capture: Option<bool>,
    cabinet: Option<bool>,
    room: Option<bool>,
    preview: bool,
    add_plugin: bool,
    removed_plugin: Option<u64>,
    restarted_plugin: Option<u64>,
    window_toggle: Option<(u64, String)>,
    param_changes: Vec<(u64, u32, f64)>,
    hide: Option<Panel>,
    /// Move a panel one place left (-1) or right (1).
    shift: Option<(Panel, isize)>,
    /// Move a panel to the row above (-1) or below (1).
    shift_row: Option<(Panel, isize)>,
    /// A panel was dragged onto another: (dragged, target, where).
    drop: Option<(Panel, Panel, Side)>,
}

/// Where a panel dropped at `pointer` on a panel drawn in `rect` goes: beside
/// it when dropped on its left or right third, else in a row of its own
/// above or below.
fn drop_side(rect: Rect, pointer: Pos2) -> Side {
    let across = (pointer.x - rect.left()) / rect.width().max(1.0);
    if across < 1.0 / 3.0 {
        Side::Before
    } else if across > 2.0 / 3.0 {
        Side::After
    } else if pointer.y < rect.center().y {
        Side::Above
    } else {
        Side::Below
    }
}

fn panel_menu(ui: &mut egui::Ui, panel: Panel, actions: &mut PanelActions) {
    if ui.button(tr("Hide")).clicked() {
        actions.hide = Some(panel);
        ui.close_menu();
    }
    if ui.button(tr("Move left")).clicked() {
        actions.shift = Some((panel, -1));
        ui.close_menu();
    }
    if ui.button(tr("Move right")).clicked() {
        actions.shift = Some((panel, 1));
        ui.close_menu();
    }
    if ui.button(tr("Move up")).clicked() {
        actions.shift_row = Some((panel, -1));
        ui.close_menu();
    }
    if ui.button(tr("Move down")).clicked() {
        actions.shift_row = Some((panel, 1));
        ui.close_menu();
    }
    ui.separator();
    ui.weak(tr("To show a hidden panel again:\nright-click the background of the controls."));
}

/// A titled box around one group of controls. Its title is a handle: drag it
/// onto another box to put it beside, above or below that box; right-click it
/// (or the box) to hide or move. Returns the rectangles of the title and of
/// the box.
fn panel_frame(ui: &mut egui::Ui, panel: Panel, actions: &mut PanelActions, add: impl FnOnce(&mut egui::Ui, &mut PanelActions)) -> (Rect, Rect) {
    // The box itself answers clicks where it was drawn last frame: registered
    // before its contents, it stays underneath them and takes nothing from them.
    let id = ui.id().with(("panel", panel));
    let last: Option<Rect> = ui.data(|d| d.get_temp(id));
    let frame = last.map(|rect| ui.interact(rect, id, Sense::click()));
    let inner = ui.group(|ui| {
        ui.vertical(|ui| {
            let title = egui::Label::new(egui::RichText::new(panel.label()).strong()).selectable(false).sense(Sense::click_and_drag());
            let header = ui.add(title).on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(tr("Drag onto another panel to move it · right-click to hide it"));
            header.dnd_set_drag_payload(panel);
            header.context_menu(|ui| panel_menu(ui, panel, actions));
            ui.horizontal(|ui| add(ui, actions));
            header.rect
        })
        .inner
    });
    ui.data_mut(|d| d.insert_temp(id, inner.response.rect));
    if let Some(frame) = frame {
        frame.context_menu(|ui| panel_menu(ui, panel, actions));
        let side = ui.input(|i| i.pointer.interact_pos()).map(|pointer| drop_side(frame.rect, pointer));
        if let (Some(side), true) = (side, frame.dnd_hover_payload::<Panel>().is_some_and(|dragged| *dragged != panel)) {
            // A bar on the edge the panel would land on.
            let r = frame.rect;
            let bar = match side {
                Side::Before => Rect::from_min_max(r.left_top(), pos2(r.left() + 4.0, r.bottom())),
                Side::After => Rect::from_min_max(pos2(r.right() - 4.0, r.top()), r.right_bottom()),
                Side::Above => Rect::from_min_max(r.left_top(), pos2(r.right(), r.top() + 4.0)),
                Side::Below => Rect::from_min_max(pos2(r.left(), r.bottom() - 4.0), r.right_bottom()),
            };
            ui.painter().rect_filled(bar, 2.0, palette(1));
        }
        if let (Some(side), Some(dragged)) = (side, frame.dnd_release_payload::<Panel>()) {
            if *dragged != panel {
                actions.drop = Some((*dragged, panel, side));
            }
        }
    }
    (inner.inner, inner.response.rect)
}

impl App {
    pub fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Controls, tr("Sound controls"));
            ui.selectable_value(&mut self.bottom_tab, BottomTab::Editor, tr("Editor"));
            ui.separator();
            if let Some(track) = self.project.tracks.get(self.selected_track) {
                ui.weak(tf!("Track: {}  ·  {}", track.name, tr(track.kind.label())));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let (label, hint) = if self.bottom_full {
                    (tr("Back under the tracks"), tr("Shows the tracks again, with this panel under them"))
                } else {
                    (tr("Full window"), tr("Shows only this panel; the tracks come back with the same button"))
                };
                let button = ui.button(label).on_hover_text(hint);
                self.bottom_toggle_rect = button.rect;
                if button.clicked() {
                    self.bottom_full = !self.bottom_full;
                }
                // The video of the song, in a corner when no editor shows it.
                if self.project.tracks.iter().any(|t| t.kind == og_engine::TrackKind::Video) {
                    let shown = ui.toggle_value(&mut self.settings.show_video, tr("Video")).on_hover_text(tr("Shows the video of the song in a corner of the window when the editor does not show it"));
                    if shown.changed() {
                        self.save_settings();
                    }
                }
                // The words of the song, under the panel.
                let mut words = !self.settings.hide_lyrics;
                if ui.toggle_value(&mut words, tr("Words")).on_hover_text(tr("Shows the words of the song under the panel, as the song plays")).changed() {
                    self.settings.hide_lyrics = !words;
                    self.save_settings();
                }
            });
        });
        ui.separator();
        if self.settings.hide_lyrics || self.bottom_tab != BottomTab::Editor {
            match self.bottom_tab {
                BottomTab::Controls => self.controls_panel(ui),
                BottomTab::Editor => self.editor_panel(ui),
            }
            return;
        }
        // The words take the bottom of the panel; the editor keeps some room.
        let band = self.words_band_height().min((ui.available_height() - 130.0).max(60.0));
        let room = (ui.available_height() - band - 10.0).max(80.0);
        let width = ui.available_width();
        ui.allocate_ui(vec2(width, room), |ui| {
            ui.set_min_height(room);
            ui.set_max_height(room);
            match self.bottom_tab {
                BottomTab::Controls => self.controls_panel(ui),
                BottomTab::Editor => self.editor_panel(ui),
            }
        });
        // The line over the words is dragged to make their band taller or lower.
        let (line, drag) = ui.allocate_exact_size(vec2(width, 8.0), Sense::drag());
        let drag = drag.on_hover_cursor(egui::CursorIcon::ResizeVertical);
        self.words_split = line;
        let ink = if drag.hovered() || drag.dragged() { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
        ui.painter().line_segment([pos2(line.left(), line.center().y), pos2(line.right(), line.center().y)], stroke(if drag.dragged() { 2.0 } else { 1.0 }, ink));
        if drag.dragged() {
            self.settings.words_height = (band - drag.drag_delta().y).clamp(WORDS_LEAST, WORDS_MOST);
        }
        if drag.drag_stopped() {
            self.save_settings();
        }
        ui.allocate_ui(vec2(width, band), |ui| {
            ui.set_min_height(band);
            ui.set_max_height(band);
            self.words_band(ui);
        });
    }

    // ----- sound controls -----------------------------------------------------------

    fn controls_panel(&mut self, ui: &mut egui::Ui) {
        let ti = self.selected_track;
        if ti >= self.project.tracks.len() {
            ui.label(tr("Select a track."));
            return;
        }
        if self.project.tracks[ti].locked {
            ui.label(tr("This track is locked."));
            return;
        }
        let rows = self.visible_rows(ti);
        let hidden = self.hidden_panels(ti);

        self.controls_rect = ui.available_rect_before_wrap();
        let mut show: Option<Option<Panel>> = None;
        // The background's menu brings hidden panels back.
        let background_menu = |ui: &mut egui::Ui, show: &mut Option<Option<Panel>>| {
            if hidden.is_empty() {
                ui.weak(tr("No panel is hidden."));
                ui.weak(tr("Right-click a panel to hide it."));
                return;
            }
            ui.weak(tr("Hidden panels"));
            for panel in &hidden {
                if ui.button(tf!("Show “{}”", panel.label())).clicked() {
                    *show = Some(Some(*panel));
                    ui.close_menu();
                }
            }
            if hidden.len() > 1 {
                ui.separator();
                if ui.button(tr("Show all")).clicked() {
                    *show = Some(None);
                    ui.close_menu();
                }
            }
        };

        let mut actions = PanelActions::default();
        self.panel_rects.clear();
        self.panel_frames.clear();
        egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
            // The background: registered before the panels, it lies under them.
            let background = ui.interact(ui.clip_rect(), ui.id().with("controls_background"), Sense::click());
            background.context_menu(|ui| background_menu(ui, &mut show));
            // Rows scroll sideways instead of wrapping.
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
            // Where each row was drawn: its panels, its top and its bottom.
            let mut drawn: Vec<(Panel, f32, f32)> = Vec::new();
            for row in &rows {
                ui.horizontal_top(|ui| {
                    let (mut top, mut bottom) = (f32::INFINITY, f32::NEG_INFINITY);
                    for panel in row {
                        let (title, frame) = panel_frame(ui, *panel, &mut actions, |ui, actions| self.panel_contents(ui, ti, *panel, actions));
                        self.panel_rects.push((*panel, title));
                        self.panel_frames.push((*panel, frame));
                        (top, bottom) = (top.min(frame.top()), bottom.max(frame.bottom()));
                    }
                    drawn.push((row[row.len() - 1], top, bottom));
                });
            }
            if rows.is_empty() {
                ui.weak(tr("Every panel is hidden: right-click here to show them again."));
            } else if !hidden.is_empty() {
                ui.weak(tf!("{} hidden: right-click the background", hidden.len()));
            }
            // A panel dropped beside the rows: at the end of the row it is level
            // with, or in a new row above the first or below the last.
            let dropped = background.dnd_release_payload::<Panel>().zip(ui.input(|i| i.pointer.interact_pos()));
            if let (None, Some((dragged, pointer)), Some(last)) = (actions.drop, dropped, drawn.last()) {
                actions.drop = Some(match drawn.iter().find(|(_, _, bottom)| pointer.y <= *bottom) {
                    Some((panel, top, _)) if pointer.y >= *top => (*dragged, *panel, Side::After),
                    Some((panel, _, _)) => (*dragged, *panel, Side::Above),
                    None => (*dragged, last.0, Side::Below),
                });
            }
        });

        match show {
            Some(Some(panel)) => self.show_panel(panel),
            Some(None) => self.show_all_panels(),
            None => {}
        }
        if let Some(panel) = actions.hide {
            self.hide_panel(panel);
        }
        if let Some((panel, by)) = actions.shift {
            self.move_panel(panel, by, ti);
        }
        if let Some((panel, by)) = actions.shift_row {
            self.move_panel_row(panel, by, ti);
        }
        if let Some((dragged, target, side)) = actions.drop {
            self.place_panel(dragged, target, side);
        }
        if actions.save_patch {
            self.dialog = Some(Dialog::SavePatch(self.project.tracks[ti].patch.clone()));
        }
        for (slot, param, value) in actions.param_changes {
            self.set_plugin_param(slot, param, value);
        }
        if let Some(slot) = actions.removed_plugin {
            self.remove_plugin(slot);
        }
        if let Some(slot) = actions.restarted_plugin {
            self.restart_plugin(slot);
        }
        if let Some((slot, title)) = actions.window_toggle {
            self.toggle_plugin_window(slot, &title);
        }
        if actions.add_plugin {
            self.pick_plugin();
        }
        match actions.capture {
            Some(true) => self.pick_capture(ti),
            Some(false) => self.set_capture(ti, None),
            None => {}
        }
        match actions.cabinet {
            Some(true) => self.pick_cabinet(ti),
            Some(false) => self.set_cabinet(ti, None),
            None => {}
        }
        match actions.room {
            Some(true) => self.pick_reverb_impulse(),
            Some(false) => self.set_reverb_impulse(None),
            None => {}
        }
        if actions.preview {
            let pitch = if self.project.tracks[ti].sound.is_drums() { 38 } else { 60 };
            self.play_preview(ti, pitch, 0.5);
        }
    }

    /// The controls inside one panel.
    fn panel_contents(&mut self, ui: &mut egui::Ui, ti: usize, panel: Panel, actions: &mut PanelActions) {
        let track = &mut self.project.tracks[ti];
        match panel {
            Panel::Sound => match &mut track.sound {
                Sound::Synth(p) => {
                    ui.vertical(|ui| {
                        let label = |w: Wave| match w {
                            Wave::Sine => tr("Sine"),
                            Wave::Saw => tr("Saw"),
                            Wave::Square => tr("Square"),
                            Wave::Triangle => tr("Triangle"),
                        };
                        egui::ComboBox::from_id_salt("wave").width(78.0).selected_text(label(p.wave)).show_ui(ui, |ui| {
                            for wave in [Wave::Sine, Wave::Saw, Wave::Square, Wave::Triangle] {
                                ui.selectable_value(&mut p.wave, wave, label(wave));
                            }
                        });
                        actions.preview |= ui.button(tr("Listen")).clicked();
                    });
                    knob(ui, &mut p.attack, 0.001, 2.0, tr("Attack"), true);
                    knob(ui, &mut p.decay, 0.01, 3.0, tr("Decay"), true);
                    knob(ui, &mut p.sustain, 0.0, 1.0, tr("Sustain"), false);
                    knob(ui, &mut p.release, 0.01, 3.0, tr("Release"), true);
                    knob(ui, &mut p.cutoff, 80.0, 8000.0, tr("Filter"), true);
                    knob(ui, &mut p.cutoff_env, 0.0, 6000.0, tr("Filter env."), false);
                    knob(ui, &mut p.detune_cents, 0.0, 50.0, tr("Detune"), false);
                    knob(ui, &mut p.sub, 0.0, 1.0, tr("Sub"), false);
                    knob(ui, &mut p.vibrato, 0.0, 1.0, tr("Vibrato"), false);
                    knob(ui, &mut p.gain, 0.05, 1.0, tr("Level"), false);
                }
                Sound::Pluck(p) => {
                    actions.preview |= ui.button(tr("Listen")).clicked();
                    knob(ui, &mut p.brightness, 0.0, 1.0, tr("Brightness"), false);
                    knob(ui, &mut p.sustain, 0.0, 1.0, tr("Hold"), false);
                    knob(ui, &mut p.gain, 0.05, 1.5, tr("Level"), false);
                }
                Sound::Drums(p) => {
                    ui.vertical(|ui| {
                        for (kit, label) in [(DrumKit::Acoustic, tr("Acoustic")), (DrumKit::Electro, tr("Electro")), (DrumKit::HipHop, tr("Hip-hop"))] {
                            ui.selectable_value(&mut p.kit, kit, label);
                        }
                    });
                    actions.preview |= ui.button(tr("Listen")).clicked();
                    knob(ui, &mut p.tune, -12.0, 12.0, tr("Tune"), false);
                    knob(ui, &mut p.decay, 0.3, 3.0, tr("Length"), false);
                    knob(ui, &mut p.gain, 0.1, 1.5, tr("Level"), false);
                }
                Sound::Sampler(p) => {
                    ui.vertical(|ui| {
                        ui.weak(tr("Sampled instrument"));
                        actions.preview |= ui.button(tr("Listen")).clicked();
                    });
                    knob(ui, &mut p.gain, 0.1, 2.0, tr("Level"), false);
                }
            },
            Panel::Arp => {
                ui.vertical(|ui| {
                    ui.checkbox(&mut track.arp.on, tr("On"));
                    let mode = |m: ArpMode| match m {
                        ArpMode::Up => tr("Up"),
                        ArpMode::Down => tr("Down"),
                        ArpMode::UpDown => tr("Up and down"),
                        ArpMode::Random => tr("Random"),
                    };
                    egui::ComboBox::from_id_salt("arp_mode").width(96.0).selected_text(mode(track.arp.mode)).show_ui(ui, |ui| {
                        for m in [ArpMode::Up, ArpMode::Down, ArpMode::UpDown, ArpMode::Random] {
                            ui.selectable_value(&mut track.arp.mode, m, mode(m));
                        }
                    });
                });
                ui.vertical(|ui| {
                    let rate = NOTE_LENGTHS.iter().find(|l| l.0 == track.arp.rate).map_or("?", |l| l.1);
                    egui::ComboBox::from_id_salt("arp_rate").width(56.0).selected_text(rate).show_ui(ui, |ui| {
                        for (value, label) in &NOTE_LENGTHS[..4] {
                            ui.selectable_value(&mut track.arp.rate, *value, *label);
                        }
                    });
                    ui.add(egui::DragValue::new(&mut track.arp.octaves).range(1..=4).speed(0.03).suffix(tr(" octave(s)")));
                });
            }
            Panel::Amp => {
                let fx = &mut track.fx;
                let captured = !fx.capture.is_empty();
                let amp = &mut fx.amp;
                ui.vertical(|ui| {
                    ui.checkbox(&mut amp.on, tr("On"));
                    if let Some(capture) = self.captures.gear.get(&fx.capture).filter(|_| captured) {
                        let about = &capture.about;
                        let details = format!("{} · {} · {} · {} Hz", about.maker, about.gear, about.format, about.sample_rate);
                        ui.horizontal(|ui| {
                            ui.add(egui::Label::new(&about.name).truncate()).on_hover_text(details);
                            if ui.small_button("✕").on_hover_text(tr("Back to the built-in amp")).clicked() {
                                actions.capture = Some(false);
                            }
                        });
                    } else if captured {
                        ui.horizontal(|ui| {
                            ui.colored_label(record(), tr("Capture not found")).on_hover_text(&fx.capture);
                            if ui.small_button("✕").clicked() {
                                actions.capture = Some(false);
                            }
                        });
                    } else {
                        let label = |m: AmpModel| match m {
                            AmpModel::Clean => tr("Clean"),
                            AmpModel::Crunch => tr("Crunch"),
                            AmpModel::Lead => tr("Lead"),
                            AmpModel::Bass => tr("Bass"),
                        };
                        egui::ComboBox::from_id_salt("amp_model").width(72.0).selected_text(label(amp.model)).show_ui(ui, |ui| {
                            for m in [AmpModel::Clean, AmpModel::Crunch, AmpModel::Lead, AmpModel::Bass] {
                                ui.selectable_value(&mut amp.model, m, label(m));
                            }
                        });
                        if ui.small_button(tr("Capture…")).on_hover_text(tr("A capture of a real amplifier or pedal takes the place of the built-in amp: a .nam file of Neural Amp Modeler, or a .json one of Proteus or AIDA-X")).clicked() {
                            actions.capture = Some(true);
                        }
                    }
                });
                knob(ui, &mut amp.gain, 0.0, 1.0, if captured { tr("Level") } else { tr("Gain") }, false);
                knob(ui, &mut amp.bass, 0.0, 1.0, tr("Lows"), false);
                knob(ui, &mut amp.mid, 0.0, 1.0, tr("Mids"), false);
                knob(ui, &mut amp.treble, 0.0, 1.0, tr("Highs"), false);
                knob(ui, &mut amp.presence, 0.0, 1.0, tr("Presence"), false);
                knob(ui, &mut amp.master, 0.0, 1.0, tr("Volume"), false);
                // The loudspeaker: the built-in filter, or the impulse of a cabinet.
                ui.vertical(|ui| {
                    ui.weak(tr("Speaker"));
                    if let Some(impulse) = self.captures.impulses.get(&fx.cabinet).filter(|_| !fx.cabinet.is_empty()) {
                        ui.horizontal(|ui| {
                            ui.add(egui::Label::new(&impulse.name).truncate()).on_hover_text(tf!("Impulse response, {:.2} s", impulse.seconds()));
                            if ui.small_button("✕").on_hover_text(tr("Back to the built-in speaker filter")).clicked() {
                                actions.cabinet = Some(false);
                            }
                        });
                    } else if !fx.cabinet.is_empty() {
                        ui.horizontal(|ui| {
                            ui.colored_label(record(), tr("Impulse not found")).on_hover_text(&fx.cabinet);
                            if ui.small_button("✕").clicked() {
                                actions.cabinet = Some(false);
                            }
                        });
                    } else if ui.small_button(tr("Cabinet IR…")).on_hover_text(tr("An impulse response of a loudspeaker cabinet (a WAV or FLAC file) takes the place of the built-in speaker filter")).clicked() {
                        actions.cabinet = Some(true);
                    }
                });
            }
            Panel::Pedals => {
                let pedals = &mut track.fx.pedals;
                let mut action: Option<(usize, i8)> = None;
                let count = pedals.len();
                for (i, pedal) in pedals.iter_mut().enumerate() {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut pedal.on, tr(pedal.kind.label()));
                        });
                        ui.horizontal(|ui| {
                            let (a, b) = match pedal.kind {
                                PedalKind::Overdrive => (tr("Gain"), tr("Tone")),
                                PedalKind::Echo => (tr("Level"), tr("Duration")),
                                PedalKind::Tremolo | PedalKind::Flanger => (tr("Depth"), tr("Speed")),
                                PedalKind::Chorus | PedalKind::Phaser => (tr("Intensity"), tr("Speed")),
                                PedalKind::Gate => (tr("Threshold"), tr("Release")),
                                PedalKind::Wah => (tr("Pedal"), tr("Resonance")),
                                PedalKind::AutoWah => (tr("Sensitivity"), tr("Speed")),
                                PedalKind::TapeEcho | PedalKind::BbdEcho => (tr("Repeats"), tr("Duration")),
                            };
                            knob(ui, &mut pedal.amount, 0.0, 1.0, a, false);
                            knob(ui, &mut pedal.rate, 0.0, 1.0, b, false);
                        });
                        ui.horizontal(|ui| {
                            if ui.add_enabled(i > 0, egui::Button::new("<").small()).on_hover_text(tr("Earlier in the chain")).clicked() {
                                action = Some((i, -1));
                            }
                            if ui.add_enabled(i + 1 < count, egui::Button::new(">").small()).on_hover_text(tr("Later in the chain")).clicked() {
                                action = Some((i, 1));
                            }
                            if ui.small_button(tr("Remove")).clicked() {
                                action = Some((i, 0));
                            }
                        });
                    });
                    ui.separator();
                }
                match action {
                    Some((i, 0)) => {
                        pedals.remove(i);
                    }
                    Some((i, by)) => pedals.swap(i, (i as isize + by as isize) as usize),
                    None => {}
                }
                ui.vertical(|ui| {
                    ui.menu_button(tr("Add a pedal…"), |ui| {
                        for kind in PedalKind::ALL {
                            if ui.button(tr(kind.label())).clicked() {
                                if pedals.len() < 8 {
                                    pedals.push(Pedal { kind, on: true, amount: 0.5, rate: 0.5 });
                                }
                                ui.close_menu();
                            }
                        }
                    });
                });
            }
            Panel::Plugins => {
                self.slot_rects.clear();
                for slot in track.instrument.iter_mut().chain(track.plugins.iter_mut()) {
                    // The plug-in's box answers right-clicks where it was drawn last
                    // frame: registered before its contents, it lies under them.
                    let id = ui.id().with(("plugin_slot", slot.id));
                    let last: Option<Rect> = ui.data(|d| d.get_temp(id));
                    let frame = last.map(|rect| ui.interact(rect, id, Sense::click()));
                    let handle = self.plugin_handles.get(&slot.id);
                    let (has_window, open) = handle.map_or((false, false), |h| (h.has_window(), h.window_open()));
                    let drawn = ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut slot.on, &slot.name);
                            if ui.small_button(tr("Remove")).clicked() {
                                actions.removed_plugin = Some(slot.id);
                            }
                        });
                        if has_window {
                            let label = if open { tr("Close the plug-in window") } else { tr("Open the plug-in window") };
                            if ui.button(label).on_hover_text(tr("Also by right-clicking the plug-in's box")).clicked() {
                                actions.window_toggle = Some((slot.id, slot.name.clone()));
                            }
                        }
                        if self.plugin_failed.contains(&slot.id) {
                            // It could not be started, or its process stopped.
                            ui.weak(tr("Plug-in stopped"));
                            if ui.button(tr("Start it again")).clicked() {
                                actions.restarted_plugin = Some(slot.id);
                            }
                            return;
                        }
                        let Some(params) = self.plugin_params.get(&slot.id) else {
                            ui.weak(tr("Plug-in not loaded"));
                            return;
                        };
                        if open {
                            // Window mode: the plug-in's own window has every setting.
                            ui.weak(tr("Its settings are in its own window."));
                            return;
                        }
                        ui.spacing_mut().slider_width = 110.0;
                        // The first parameters; the plug-in's own window shows them all.
                        for param in params.iter().take(6) {
                            // What the slider last set, else what the plug-in says
                            // (its own window may have changed it), else the default.
                            let set_here = slot.params.iter().find(|p| p.0 == param.id).map(|p| p.1);
                            let mut value = set_here
                                .or_else(|| handle.and_then(|h| h.param_value(param.id)))
                                .unwrap_or(param.default)
                                .clamp(param.min, param.max);
                            if ui.add(egui::Slider::new(&mut value, param.min..=param.max).text(&param.name)).changed() {
                                actions.param_changes.push((slot.id, param.id, value));
                            }
                        }
                        if params.len() > 6 {
                            ui.weak(tf!("… and {} more settings", params.len() - 6));
                        }
                    });
                    ui.data_mut(|d| d.insert_temp(id, drawn.response.rect));
                    self.slot_rects.push((slot.id, drawn.response.rect));
                    if let Some(frame) = frame {
                        let (slot_id, name) = (slot.id, slot.name.clone());
                        frame.context_menu(|ui| {
                            if has_window {
                                let label = if open { tr("Back to the sliders") } else { tr("Use the plug-in's own window") };
                                if ui.button(label).clicked() {
                                    actions.window_toggle = Some((slot_id, name.clone()));
                                    ui.close_menu();
                                }
                            } else {
                                ui.weak(tr("This plug-in has no window of its own: it is set with the sliders."));
                            }
                            if ui.button(tr("Remove")).clicked() {
                                actions.removed_plugin = Some(slot_id);
                                ui.close_menu();
                            }
                        });
                    }
                    ui.separator();
                }
                ui.vertical(|ui| {
                    actions.add_plugin = ui.button(tr("Add a plug-in…")).on_hover_text(tr("Effect or instrument in the CLAP, LV2 or VST3 format")).clicked();
                    ui.weak(tr("Effects: every track."));
                    ui.weak(tr("Instruments: instrument tracks."));
                });
            }
            Panel::Eq => {
                let eq = &mut track.fx.eq;
                ui.vertical(|ui| {
                    ui.checkbox(&mut eq.on, tr("On"));
                    if ui.small_button(tr("Flat")).on_hover_text(tr("Puts the three points back on the middle line")).clicked() {
                        (eq.low, eq.mid, eq.high) = (0.0, 0.0, 0.0);
                    }
                });
                // The curve, with a point to drag for each of the three bands:
                // sideways for the frequency, up and down for the gain.
                const SPAN: f32 = 18.0;
                let (area, _) = ui.allocate_exact_size(vec2(300.0, 112.0), Sense::hover());
                let x_of = |hz: f32| area.left() + area.width() * (hz / 20.0).ln() / 1000f32.ln();
                let hz_of = |x: f32| 20.0 * (1000f32.ln() * ((x - area.left()) / area.width()).clamp(0.0, 1.0)).exp();
                let y_of = |db: f32| area.center().y - db / SPAN * area.height() / 2.0;
                let db_of = |y: f32| ((area.center().y - y) / (area.height() / 2.0) * SPAN).clamp(-SPAN, SPAN);
                let painter = ui.painter().with_clip_rect(area);
                let ink = ui.visuals().text_color();
                painter.rect_filled(area, 4.0, ui.visuals().extreme_bg_color);
                for (hz, label) in [(100.0, "100"), (1000.0, "1k"), (10_000.0, "10k")] {
                    painter.vline(x_of(hz), area.y_range(), stroke(1.0, ink.gamma_multiply(0.12)));
                    painter.text(pos2(x_of(hz) + 2.0, area.bottom() - 1.0), Align2::LEFT_BOTTOM, label, FontId::proportional(9.0), ink.gamma_multiply(0.45));
                }
                painter.hline(area.x_range(), area.center().y, stroke(1.0, ink.gamma_multiply(0.25)));
                let colour = if eq.on { palette(1) } else { ink.gamma_multiply(0.35) };
                let curve: Vec<Pos2> = (0..=75)
                    .map(|i| {
                        let hz = hz_of(area.left() + area.width() * i as f32 / 75.0);
                        pos2(x_of(hz), y_of(og_engine::fx::eq_response_db(eq, 48_000.0, hz)))
                    })
                    .collect();
                painter.add(Shape::line(curve, stroke(2.0, colour)));
                self.eq_handles.clear();
                for band in 0..3 {
                    let (hz, db, limits) = match band {
                        0 => (eq.low_freq, eq.low, (30.0, 600.0)),
                        1 => (eq.mid_freq, eq.mid, (100.0, 8000.0)),
                        _ => (eq.high_freq, eq.high, (1000.0, 18_000.0)),
                    };
                    let at = pos2(x_of(hz), y_of(db));
                    let handle = ui
                        .interact(Rect::from_center_size(at, vec2(18.0, 18.0)), ui.id().with(("eq_point", band)), Sense::click_and_drag())
                        .on_hover_cursor(egui::CursorIcon::Grab)
                        .on_hover_text(format!("{hz:.0} Hz   {db:+.1} dB"));
                    self.eq_handles.push(handle.rect);
                    let (mut new_hz, mut new_db) = (hz, db);
                    if let Some(pointer) = handle.interact_pointer_pos().filter(|_| handle.dragged()) {
                        (new_hz, new_db) = (hz_of(pointer.x).clamp(limits.0, limits.1), db_of(pointer.y));
                        eq.on = true;
                    }
                    if handle.double_clicked() {
                        new_db = 0.0;
                    }
                    match band {
                        0 => (eq.low_freq, eq.low) = (new_hz, new_db),
                        1 => (eq.mid_freq, eq.mid) = (new_hz, new_db),
                        _ => (eq.high_freq, eq.high) = (new_hz, new_db),
                    }
                    painter.circle_filled(at, 6.0, colour);
                    painter.circle_stroke(at, 6.0, stroke(1.5, ui.visuals().extreme_bg_color));
                }
            }
            Panel::Dynamics => {
                let fx = &mut track.fx;
                knob(ui, &mut fx.comp, 0.0, 1.0, tr("Compressor"), false);
                knob(ui, &mut fx.echo, 0.0, 1.0, tr("Echo"), false);
                knob(ui, &mut fx.reverb, 0.0, 1.0, tr("Reverb"), false);
            }
            Panel::Master => {
                let room_set = !self.project.reverb_impulse.is_empty();
                let master = &mut self.project.master_fx;
                ui.vertical(|ui| {
                    ui.checkbox(&mut master.eq.on, tr("Equaliser"));
                    ui.checkbox(&mut master.limiter, tr("Limiter"));
                    if !room_set {
                        let label = |kind: ReverbKind| match kind {
                            ReverbKind::Room => tr("Room"),
                            ReverbKind::Plate => tr("Plate"),
                        };
                        egui::ComboBox::from_id_salt("reverb_kind").width(72.0).selected_text(label(master.reverb_kind)).show_ui(ui, |ui| {
                            for kind in [ReverbKind::Room, ReverbKind::Plate] {
                                ui.selectable_value(&mut master.reverb_kind, kind, label(kind));
                            }
                        });
                    }
                });
                knob(ui, &mut master.eq.low, -12.0, 12.0, tr("Lows dB"), false);
                knob(ui, &mut master.eq.mid, -12.0, 12.0, tr("Mids dB"), false);
                knob(ui, &mut master.eq.high, -12.0, 12.0, tr("Highs dB"), false);
                knob(ui, &mut master.comp, 0.0, 1.0, tr("Compressor"), false);
                let mut beats = master.echo_beats as f32;
                if knob(ui, &mut beats, 0.125, 2.0, tr("Echo: time"), false) {
                    master.echo_beats = snap(f64::from(beats), 0.125).max(0.125);
                }
                knob(ui, &mut master.echo_feedback, 0.0, 0.9, tr("Echo: repeats"), false);
                let room = &self.project.reverb_impulse;
                if room.is_empty() {
                    knob(ui, &mut master.reverb_size, 0.0, 1.0, tr("Reverb size"), false);
                    knob(ui, &mut master.reverb_damp, 0.0, 1.0, tr("Reverb damp."), false);
                }
                ui.vertical(|ui| {
                    ui.weak(tr("Reverb"));
                    if let Some(impulse) = self.captures.impulses.get(room).filter(|_| !room.is_empty()) {
                        ui.horizontal(|ui| {
                            ui.add(egui::Label::new(&impulse.name).truncate()).on_hover_text(tf!("Impulse response, {:.2} s", impulse.seconds()));
                            if ui.small_button("✕").on_hover_text(tr("Back to the built-in reverb")).clicked() {
                                actions.room = Some(false);
                            }
                        });
                    } else if !room.is_empty() {
                        ui.horizontal(|ui| {
                            ui.colored_label(record(), tr("Impulse not found")).on_hover_text(room);
                            if ui.small_button("✕").clicked() {
                                actions.room = Some(false);
                            }
                        });
                    } else if ui.small_button(tr("Room IR…")).on_hover_text(tr("An impulse response of a room, a plate or a spring (a WAV or FLAC file) takes the place of the built-in reverb")).clicked() {
                        actions.room = Some(true);
                    }
                });
            }
            Panel::Save => {
                actions.save_patch = ui.button(tr("Save this sound…")).on_hover_text(tr("Adds it to “My sounds” in the library")).clicked();
            }
        }
    }

    // ----- editors ------------------------------------------------------------------

    fn editor_panel(&mut self, ui: &mut egui::Ui) {
        let (shown, several) = self.editor_shown();
        if several {
            self.editor_tracks_bar(ui);
        }
        if shown.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(tr("Select a region to edit it.\nDouble-click in an instrument or drummer track to create one."));
            });
            return;
        }
        if shown.len() == 1 {
            let (ti, ri) = shown[0];
            self.one_editor(ui, ti, ri, several && ti != self.selected_track);
            return;
        }
        // The tracks one under the other, each with its share of the height;
        // the line between two is dragged to share it otherwise.
        const SPLIT: f32 = 8.0;
        const LEAST: f32 = 80.0;
        let count = shown.len();
        let room = (ui.available_height() - SPLIT * (count - 1) as f32).max(LEAST * count as f32);
        if self.editor_shares.len() != count {
            self.editor_shares = vec![1.0 / count as f32; count];
        }
        self.editor_splits.clear();
        for (k, (ti, ri)) in shown.into_iter().enumerate() {
            if k > 0 {
                let (line, drag) = ui.allocate_exact_size(vec2(ui.available_width(), SPLIT), Sense::drag());
                let drag = drag.on_hover_cursor(egui::CursorIcon::ResizeVertical);
                self.editor_splits.push(line);
                let ink = if drag.hovered() || drag.dragged() { ui.visuals().strong_text_color() } else { ui.visuals().weak_text_color() };
                ui.painter().line_segment([pos2(line.left(), line.center().y), pos2(line.right(), line.center().y)], stroke(if drag.dragged() { 2.0 } else { 1.0 }, ink));
                if drag.dragged() {
                    // Taken from one, given to the other, each keeping some room.
                    let least = LEAST / room;
                    let (above, below) = (self.editor_shares[k - 1], self.editor_shares[k]);
                    let moved = (drag.drag_delta().y / room).clamp(least - above, below - least);
                    self.editor_shares[k - 1] = above + moved;
                    self.editor_shares[k] = below - moved;
                }
            }
            let each = (room * self.editor_shares[k]).max(LEAST);
            ui.push_id(ti, |ui| {
                ui.allocate_ui(vec2(ui.available_width(), each), |ui| {
                    ui.set_min_height(each);
                    ui.set_max_height(each);
                    self.one_editor(ui, ti, ri, ti != self.selected_track);
                });
            });
        }
    }

    /// The line over the editors of several tracks: which they are, and the
    /// auto-switch that shows one at a time.
    fn editor_tracks_bar(&mut self, ui: &mut egui::Ui) {
        let tracks = self.editor_track_list();
        let names: Vec<String> = tracks.iter().filter_map(|ti| self.project.tracks.get(*ti)).map(|t| t.name.clone()).collect();
        let bars = self.auto_switch_bars();
        ui.horizontal(|ui| {
            ui.weak(tf!("Tracks in the editor: {}", names.join(" · ")));
            ui.separator();
            let hint = tf!("Shows one of the tracks at a time: the one being played, the first chosen having priority. When nothing more comes on the one shown within the next {bars} bars, the next one that plays takes its place as its last note is struck. View > “Auto-switch after” sets the bars." [bars]);
            if ui.toggle_value(&mut self.auto_switch, tr("Auto-switch")).on_hover_text(hint).changed() {
                self.followed = None;
            }
            if let Some(name) = self.followed.filter(|_| self.auto_switch).and_then(|ti| self.project.tracks.get(ti)).map(|t| t.name.clone()) {
                ui.weak(tf!("Following: {}", name));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(tr("Ctrl and a click on a track header adds the track here, or takes it away"));
            });
        });
        ui.separator();
    }

    /// The editor of one region. For a track that is not the selected one
    /// (`secondary`), the notes show without a selection, and selecting one
    /// there brings that track to the front.
    fn one_editor(&mut self, ui: &mut egui::Ui, ti: usize, ri: usize, secondary: bool) {
        let saved = if secondary { Some((std::mem::take(&mut self.selected_notes), self.drag.is_some())) } else { None };
        if secondary {
            ui.strong(&self.project.tracks[ti].name);
        }
        match self.project.tracks[ti].regions[ri].content {
            // A video track shows its video.
            _ if self.project.tracks[ti].kind == og_engine::TrackKind::Video => self.video_editor_panel(ui, ti),
            Content::Midi(_) => self.midi_editor(ui, ti, ri),
            Content::Audio(_) => {
                if self.is_recording_track(ti) {
                    self.recording_shift_row(ui, ti);
                }
                self.audio_editor(ui, ti, ri);
            }
            Content::Drummer(_) => self.drummer_editor(ui, ti, ri),
        }
        if let Some((notes, dragging)) = saved {
            if !self.selected_notes.is_empty() || (self.drag.is_some() && !dragging) {
                self.selected_track = ti;
                self.selected_region = Some((ti, ri));
            } else {
                self.selected_notes = notes;
            }
        }
    }

    fn midi_editor(&mut self, ui: &mut egui::Ui, ti: usize, ri: usize) {
        let locked = self.project.tracks[ti].locked;
        let guitar = self.is_guitar_track(ti);
        if self.note_view == NoteView::Tab && !guitar && ti == self.selected_track {
            self.note_view = NoteView::Roll;
        }
        let view = if self.note_view == NoteView::Tab && !guitar { NoteView::Score } else { self.note_view };
        let (mut quantize, mut transpose, mut velocity) = (false, 0i32, None);
        let (mut text_size, mut text_size_chosen) = (self.tab_text_size(), false);
        let (mut two_lines_chosen, mut loop_beats) = (false, false);
        let (beats_chosen, looping) = (self.beats_selected(ti, ri).is_some(), self.looping_beats(ti, ri));
        ui.horizontal(|ui| {
            let region = &mut self.project.tracks[ti].regions[ri];
            ui.add(egui::TextEdit::singleline(&mut region.name).desired_width(120.0));
            ui.selectable_value(&mut self.note_view, NoteView::Roll, tr("Notes"));
            ui.selectable_value(&mut self.note_view, NoteView::Score, tr("Score"));
            if guitar {
                ui.selectable_value(&mut self.note_view, NoteView::Tab, tr("Tab")).on_hover_text(tr("Tablature: the notes on the strings of the track's guitar or bass"));
            }
            if self.note_view == NoteView::Tab {
                ui.spacing_mut().slider_width = 70.0;
                let slider = ui
                    .add(egui::Slider::new(&mut text_size, TAB_TEXT_RANGE).step_by(0.5).fixed_decimals(1).text(tr("Text size")))
                    .on_hover_text(tr("Size of what the tablature writes: the fret numbers, the chord names and the chord grids"));
                if slider.changed() {
                    self.settings.tab_text_size = text_size;
                }
                // Kept once chosen, not at every step of a drag.
                text_size_chosen = slider.drag_stopped() || (slider.changed() && !slider.dragged());
            }
            if self.note_view != NoteView::Roll && !self.bottom_full {
                let mut two = self.settings.two_lines;
                let hint = tr("The score or the tablature over two lines of whole bars, as on a page, instead of one line that scrolls. The full window lays the whole song out that way.");
                if ui.checkbox(&mut two, tr("Two lines")).on_hover_text(hint).changed() {
                    self.settings.two_lines = two;
                    two_lines_chosen = true;
                }
            }
            if view != NoteView::Roll {
                let chosen = beats_chosen;
                let label = if looping { tr("Stop the loop") } else { tr("Loop the beats") };
                let hint = if chosen {
                    tr("Plays the chosen beats over and over: the loop area of the song is set to them")
                } else {
                    tr("Shift and a click on a beat, then on another, chooses the beats to loop")
                };
                if ui.add_enabled(chosen, egui::Button::new(label).selected(looping)).on_hover_text(hint).on_disabled_hover_text(hint).clicked() {
                    loop_beats = true;
                }
            }
            ui.separator();
            ui.label(tr("New notes"));
            let current = NOTE_LENGTHS.iter().find(|l| l.0 == self.note_len).map_or("?", |l| l.1);
            egui::ComboBox::from_id_salt("note_len").width(52.0).selected_text(current).show_ui(ui, |ui| {
                for (value, label) in NOTE_LENGTHS {
                    ui.selectable_value(&mut self.note_len, value, label);
                }
            });
            ui.separator();
            let current = NOTE_LENGTHS.iter().find(|l| l.0 == self.quantize).map_or("?", |l| l.1);
            egui::ComboBox::from_id_salt("quantize").width(52.0).selected_text(current).show_ui(ui, |ui| {
                for (value, label) in &NOTE_LENGTHS[..4] {
                    ui.selectable_value(&mut self.quantize, *value, *label);
                }
            });
            quantize = ui.button(tr("Quantize")).on_hover_text(tr("Snaps the selected notes (or all of them) onto the chosen grid. Key: Q")).clicked();
            // The Q key is for the selected track, whichever editor draws first.
            if ti == self.selected_track {
                quantize |= std::mem::take(&mut self.quantize_request);
            }
            ui.separator();
            ui.label(tr("Transpose"));
            for (label, by) in [("-12", -12), ("-1", -1), ("+1", 1), ("+12", 12)] {
                if ui.small_button(label).clicked() {
                    transpose = by;
                }
            }
            ui.separator();
            if let Some(&first) = self.selected_notes.first() {
                if let Some(note) = region.notes().get(first) {
                    let mut value = f32::from(note.velocity);
                    ui.spacing_mut().slider_width = 90.0;
                    if ui.add(egui::Slider::new(&mut value, 1.0..=127.0).fixed_decimals(0).text(tr("Velocity"))).changed() {
                        velocity = Some(value as u8);
                    }
                }
            } else {
                ui.weak(tr("Select notes to set their velocity"));
            }
        });
        if text_size_chosen || two_lines_chosen {
            self.save_settings();
        }
        if loop_beats {
            self.toggle_loop_beats(ti, ri);
        }

        if !locked && (quantize || transpose != 0 || velocity.is_some()) {
            self.checkpoint();
            let grid = self.quantize;
            let selected = self.selected_notes.clone();
            if let Some(notes) = self.project.tracks[ti].regions[ri].notes_mut() {
                for (i, note) in notes.iter_mut().enumerate() {
                    if !selected.is_empty() && !selected.contains(&i) {
                        continue;
                    }
                    if quantize {
                        note.start = snap(note.start, grid).max(0.0);
                        note.length = snap(note.length, grid).max(grid);
                    }
                    note.pitch = (i32::from(note.pitch) + transpose).clamp(0, 127) as u8;
                    if let Some(v) = velocity {
                        note.velocity = v;
                    }
                }
            }
        }

        match view {
            // Taller than the panel over several lines, or when the text is
            // large: they then scroll.
            NoteView::Score => {
                egui::ScrollArea::vertical().id_salt("score").auto_shrink([false, false]).show(ui, |ui| self.score_canvas(ui, ti, ri));
            }
            NoteView::Tab => {
                egui::ScrollArea::vertical().id_salt("tab").auto_shrink([false, false]).show(ui, |ui| self.tab_canvas(ui, ti, ri));
            }
            NoteView::Roll => {
                let mut area = egui::ScrollArea::vertical().id_salt("roll").auto_shrink([false, false]);
                if let Some(offset) = self.roll_scroll_to.take() {
                    area = area.vertical_scroll_offset(offset);
                }
                area.show(ui, |ui| self.roll_canvas(ui, ti, ri));
            }
        }
    }

    /// The pixels a beat takes in an editor `width` wide: the zoom the user
    /// set, or else enough for `closest` beats across. The zoom is sideways
    /// only: the height of the notes and the size of the text do not follow it.
    fn editor_ppb(&self, width: f32, closest: f64) -> f64 {
        self.editor_zoom.unwrap_or_else(|| (f64::from(width.max(1.0)) / closest.max(1.0)).max(1.0))
    }

    /// Horizontal scale of a note editor: pixels a beat, and the first beat
    /// shown, from the zoom and the scroll of the editors. While the song
    /// plays through the region, the notes scroll under the playhead, or
    /// the view pages along with it.
    fn editor_scale(&mut self, width: f32, cycle: f64, region_start: f64, region_length: f64, closest: f64) -> (f64, f64) {
        let ppb = self.editor_ppb(width, closest);
        let most = (cycle - f64::from(width) / ppb).max(0.0);
        if self.playing {
            let local = self.position - region_start;
            if local >= 0.0 && local <= region_length {
                let shown = local % cycle;
                let visible = f64::from(width) / ppb;
                if !self.settings.editor_playhead_moves {
                    // The playhead stays put, towards the left so that what
                    // comes next shows: the notes scroll under it, past the
                    // edges of the region if need be.
                    self.editor_scroll = shown - visible * PLAYHEAD_AT;
                    return (ppb, self.editor_scroll);
                }
                if most > 0.0 && (shown < self.editor_scroll || shown > self.editor_scroll + visible) {
                    self.editor_scroll = (shown / visible).floor() * visible;
                }
            }
        }
        self.editor_scroll = self.editor_scroll.clamp(0.0, most);
        (ppb, self.editor_scroll)
    }

    /// Lays a score or a tablature out: one line that scrolls sideways (the
    /// usual way), two lines of whole bars when the user asked for them, or,
    /// in the full window, the whole region over as many lines as it takes.
    /// `closest` is the beats a line shows until the user zooms, `line` the
    /// height of a line and `room` that of the view.
    #[allow(clippy::too_many_arguments)]
    pub fn editor_lines(&mut self, width: f32, cycle: f64, region_start: f64, region_length: f64, closest: f64, line: f32, room: f32) -> Lines {
        let wanted = if self.bottom_full { usize::MAX } else if self.settings.two_lines { 2 } else { 1 };
        if wanted == 1 {
            let (ppb, offset) = self.editor_scale(width, cycle, region_start, region_length, closest);
            let system = System { from: offset, to: offset + f64::from(width.max(1.0)) / ppb, top: 0.0 };
            return Lines { ppb, systems: vec![system], height: room.max(line), paged: false, current: Some(0) };
        }
        let bar_len = self.project.bar_len();
        // A line holds whole bars, at least one.
        let ppb = self.editor_ppb(width, closest.max(bar_len));
        let per_line = (f64::from(width) / (ppb * bar_len)).floor().max(1.0) * bar_len;
        let count = ((cycle / per_line).ceil() as usize).max(1);
        let local = self.position - region_start;
        let playing = (self.playing && local >= 0.0 && local <= region_length).then(|| (((local % cycle) / per_line) as usize).min(count - 1));
        let (first, shown) = if wanted == usize::MAX {
            (0, count)
        } else {
            // Two lines at a time, as the pages of a book: the next two come
            // when both are played, or with the wheel.
            let line = playing.unwrap_or_else(|| (self.editor_scroll / per_line).floor().max(0.0) as usize);
            let first = (line / 2 * 2).min(count.saturating_sub(2));
            self.editor_scroll = first as f64 * per_line;
            (first, count.min(first + 2) - first)
        };
        let systems = (0..shown).map(|i| System { from: (first + i) as f64 * per_line, to: (first + i + 1) as f64 * per_line, top: i as f32 * line }).collect();
        let current = playing.and_then(|line| line.checked_sub(first)).filter(|line| *line < shown);
        Lines { ppb, systems, height: (shown as f32 * line).max(room), paged: true, current }
    }

    /// The names of the sections of the song that start within a region,
    /// with the beat of each from the start of the region.
    fn section_names(&self, region_start: f64, cycle: f64) -> Vec<(f64, String)> {
        self.project.markers.iter().map(|m| (m.start - region_start, m.name.clone())).filter(|(beat, name)| *beat >= -1e-6 && *beat < cycle && !name.trim().is_empty()).collect()
    }

    /// On several lines, the wheel turns the pages while the song is not
    /// playing, and the view scrolls to the line being played when that
    /// changes, so that the user may still look elsewhere meanwhile.
    fn follow_lines(&mut self, ui: &mut egui::Ui, resp: &egui::Response, rect: Rect, lines: &Lines, line: f32) {
        if !lines.paged {
            self.editor_line = None;
            self.editor_layout = (false, 0);
            return;
        }
        let per_line = lines.systems.first().map_or(1.0, |s| s.to - s.from);
        // Only when the two lines fit the view: otherwise the wheel scrolls them.
        let fits = ui.clip_rect().intersect(rect).height() >= rect.height() - 1.0;
        if resp.hovered() && fits && !self.bottom_full && !self.playing {
            let delta = ui.input(|i| i.smooth_scroll_delta);
            self.editor_wheel += delta.y + delta.x;
            if self.editor_wheel.abs() >= 40.0 {
                let pages = f64::from((self.editor_wheel / 40.0).trunc());
                self.editor_wheel = 0.0;
                self.editor_scroll = (self.editor_scroll - pages * 2.0 * per_line).max(0.0);
            }
        }
        if let Some(current) = lines.current {
            self.editor_line_top = lines.systems[current].top;
        }
        let layout = (self.bottom_full, lines.systems.len());
        if self.editor_layout != layout {
            self.editor_layout = layout;
            self.editor_refollow = 3;
        }
        match lines.current.filter(|_| self.playing) {
            Some(current) if self.editor_line != Some(current) || self.editor_refollow > 0 => {
                if self.editor_line != Some(current) {
                    self.editor_refollow = self.editor_refollow.max(2);
                }
                self.editor_refollow = self.editor_refollow.saturating_sub(1);
                self.editor_line = Some(current);
                let top = rect.top() + lines.systems[current].top;
                ui.scroll_to_rect(Rect::from_min_size(pos2(rect.left(), top), vec2(1.0, line)), Some(egui::Align::Min));
            }
            Some(_) => {}
            None => self.editor_line = None,
        }
    }

    /// The playhead through the line it is in, when that line shows.
    #[allow(clippy::too_many_arguments)]
    fn lines_playhead(&self, painter: &egui::Painter, rect: Rect, lines: &Lines, left: f32, line: f32, region_start: f64, region_length: f64, cycle: f64, ink: Color32) {
        let local = self.position - region_start;
        if local < 0.0 || local > region_length {
            return;
        }
        let shown = local % cycle;
        if let Some(system) = lines.systems.iter().find(|s| shown >= s.from && shown < s.to) {
            let x = left + ((shown - system.from) * lines.ppb) as f32;
            painter.line_segment([pos2(x, rect.top() + system.top), pos2(x, rect.top() + system.top + line)], stroke(1.5, ink));
        }
    }

    /// The chords of a region: the ones written over it, or else the ones
    /// its notes make.
    fn chords_of(&self, ti: usize, ri: usize) -> Vec<og_engine::ChordMark> {
        let region = &self.project.tracks[ti].regions[ri];
        if region.chords.is_empty() {
            og_engine::chords::detect(region.notes())
        } else {
            region.chords.clone()
        }
    }

    /// The size of the fret numbers of the tablature: the one the user
    /// chose, or else the usual one.
    pub fn tab_text_size(&self) -> f32 {
        let size = self.settings.tab_text_size;
        if size > 0.0 {
            size.clamp(*TAB_TEXT_RANGE.start(), *TAB_TEXT_RANGE.end())
        } else {
            TAB_TEXT
        }
    }

    /// Draws the chord names of a region (see `chords_of`), from beat `from`
    /// to beat `to` and within its cycle, along the top of an editor.
    #[allow(clippy::too_many_arguments)]
    fn chord_names(painter: &egui::Painter, chords: &[og_engine::ChordMark], cycle: f64, x_of: &dyn Fn(f64) -> f32, y: f32, from: f64, to: f64, ink: Color32) {
        for chord in chords.iter().filter(|c| c.beat < cycle && c.beat >= from - 1e-6 && c.beat < to) {
            painter.text(pos2(x_of(chord.beat), y), Align2::LEFT_CENTER, chord_label(&chord.name), FontId::proportional(12.0), ink);
        }
    }

    /// Draws the chords of a tablature along its top, from `top` down: each
    /// name, and over it a grid of where the fingers go, where there is
    /// room for them. A chord whose start has scrolled past `left` waits
    /// there while it lasts, so that the chord being played always shows;
    /// the chords from beat `to` on belong to another line. `scale` is the
    /// size of it all against the usual one.
    #[allow(clippy::too_many_arguments)]
    fn tab_chords(&self, painter: &egui::Painter, chords: &[og_engine::ChordMark], cycle: f64, notes: &[Note], positions: &[Option<(usize, u8)>], tuning: &[u8], x_of: &dyn Fn(f64) -> f32, left: f32, to: f64, top: f32, scale: f32, ink: Color32) {
        let font = FontId::proportional(12.0 * scale);
        let between = 6.0 * scale;
        let right = painter.clip_rect().right();
        let mut free_from = f32::MIN;
        for (i, chord) in chords.iter().enumerate().filter(|(_, c)| c.beat < cycle && c.beat < to) {
            let until = chords.get(i + 1).map_or(cycle, |next| next.beat);
            let (start, end) = (x_of(chord.beat), x_of(until));
            if end <= left || start > right {
                continue;
            }
            let name = painter.layout_no_wrap(chord_label(&chord.name), font.clone(), ink);
            let shape = chord_grid_shape(&chord.name, chord.beat, until, notes, positions, tuning);
            let width = name.size().x.max(shape.as_ref().map_or(0.0, |shape| chord_grid_width(shape.frets.len(), scale)));
            let x = chord_place(start, end - between, width, left);
            if x < free_from {
                continue;
            }
            if let Some(shape) = &shape {
                chord_grid(painter, pos2(x, top + 4.0), shape, scale, ink);
            }
            painter.galley(pos2(x, top + (TAB_CHORDS + 11.0) * scale - name.size().y / 2.0), name, ink);
            free_from = x + width + between;
        }
    }

    /// Sets the zoom of the note editors, kept in the settings; `None` is the
    /// usual one, four bars across.
    pub fn set_editor_zoom(&mut self, zoom: Option<f64>) {
        let zoom = zoom.map(|z| z.clamp(*ZOOM_RANGE.start(), *ZOOM_RANGE.end()));
        self.editor_zoom = zoom;
        self.settings.editor_zoom = zoom.unwrap_or(0.0);
        self.save_settings();
    }

    /// Ctrl and the wheel zoom an editor around the pointer, the wheel
    /// sideways scrolls it; at the bottom right, "-" and "+" zoom by steps,
    /// as far either way as wanted, between them the zoom (100 % is
    /// `ZOOM_UNIT` pixels a beat), which a click puts back to four bars.
    fn editor_zoom_controls(&mut self, ui: &mut egui::Ui, resp: &egui::Response, rect: Rect, left: f32, ppb: f64) {
        let (least, most) = (*ZOOM_RANGE.start(), *ZOOM_RANGE.end());
        let mut wanted = None;
        if resp.hovered() {
            let (zoom, scroll, pointer) = ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
            if zoom != 1.0 {
                let after = (ppb * f64::from(zoom)).clamp(least, most);
                wanted = Some(Some(after));
                // The beat under the pointer stays under it.
                if let Some(p) = pointer.filter(|_| (after - ppb).abs() > 1e-9) {
                    let anchor = self.editor_scroll + f64::from(p.x - left) / ppb;
                    self.editor_scroll = anchor - f64::from(p.x - left) / after;
                }
            }
            if scroll.x != 0.0 {
                self.editor_scroll -= f64::from(scroll.x) / ppb;
            }
        }
        let bars = format!("{:.1}", (f64::from(rect.right() - left) / (ppb * self.project.bar_len())).max(0.0));
        let percent = (ppb / ZOOM_UNIT * 100.0).round();
        let strip = Rect::from_min_size(pos2(rect.right() - 150.0, rect.bottom() - 24.0), vec2(142.0, 20.0));
        ui.painter().rect_filled(strip.expand(2.0), 3.0, ui.visuals().extreme_bg_color.gamma_multiply(0.9));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(strip), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(tr("Zoom"));
                if ui.small_button("-").on_hover_text(tr("Zoom out: more bars across the line")).clicked() {
                    wanted = Some(Some((ppb / ZOOM_STEP).clamp(least, most)));
                }
                let value = ui.add(egui::Label::new(format!("{percent} %")).sense(Sense::click()));
                if value.on_hover_text(tf!("{bars} bars across the line. A click: four bars again; Ctrl and the mouse wheel zoom too" [bars])).clicked() {
                    wanted = Some(None);
                }
                if ui.small_button("+").on_hover_text(tr("Zoom in: fewer bars across the line")).clicked() {
                    wanted = Some(Some((ppb * ZOOM_STEP).clamp(least, most)));
                }
            });
        });
        if let Some(zoom) = wanted {
            self.set_editor_zoom(zoom);
        }
    }

    // ----- tablature ------------------------------------------------------------------

    /// Draws what a tablature writes around its fret numbers: slides, the
    /// slurs of hammer-ons and pull-offs, bends with their height, vibrato,
    /// staccato dots and accents, and the "P.M." and "let ring" lines over
    /// the strings, tremolo picking and trills, and where the feel of the
    /// region changes. `scale` is the size of the tablature against its usual one.
    #[allow(clippy::too_many_arguments)]
    fn tab_articulations(&self, painter: &egui::Painter, notes: &[Note], positions: &[Option<(usize, u8)>], labels: &[Option<Rect>], feels: &[og_engine::Swing], feel_y: f32, x_of: &dyn Fn(f64) -> f32, top: f32, scale: f32, ink: Color32) {
        let gap = TAB_GAP * scale;
        let thin = stroke(1.0, ink);
        let small = FontId::proportional(9.5 * scale);
        // The next note on each string, as the tablature lays the notes out.
        let placed: Vec<Note> = notes.iter().zip(positions).map(|(note, place)| Note { string: place.map(|(string, _)| string as u8), ..note.clone() }).collect();
        let next = og_engine::next_on_string(&placed);
        // A slur: an arc over the two notes it joins.
        let slur = |from: Pos2, to: Pos2| -> Vec<Pos2> {
            let lift = (to.x - from.x).abs().min(24.0) * 0.3 + 3.0;
            (0..=12)
                .map(|k| {
                    let t = k as f32 / 12.0;
                    pos2(from.x + (to.x - from.x) * t, from.y - lift * 4.0 * t * (1.0 - t))
                })
                .collect()
        };
        for (i, note) in notes.iter().enumerate() {
            let (Some(label), art) = (labels[i], &note.articulation) else { continue };
            let y = label.center().y;
            let following = next[i].and_then(|j| labels[j].map(|rect| (j, rect)));
            match art.slide {
                Some(Slide::Legato | Slide::Shift) => {
                    if let Some((j, next_label)) = following {
                        let (from, to) = match notes[j].pitch.cmp(&note.pitch) {
                            std::cmp::Ordering::Greater => (y + 4.0, y - 4.0),
                            std::cmp::Ordering::Less => (y - 4.0, y + 4.0),
                            std::cmp::Ordering::Equal => (y - 3.0, y - 3.0),
                        };
                        painter.line_segment([pos2(label.right() + 1.0, from), pos2(next_label.left() - 1.0, to)], thin);
                        if art.slide == Some(Slide::Legato) {
                            painter.add(Shape::line(slur(pos2(label.center().x, label.top()), pos2(next_label.center().x, next_label.top())), thin));
                        }
                    }
                }
                Some(Slide::FromBelow) => {
                    painter.line_segment([pos2(label.left() - 10.0, y + 4.0), pos2(label.left() - 2.0, y - 4.0)], thin);
                }
                Some(Slide::FromAbove) => {
                    painter.line_segment([pos2(label.left() - 10.0, y - 4.0), pos2(label.left() - 2.0, y + 4.0)], thin);
                }
                Some(Slide::OutDown) => {
                    painter.line_segment([pos2(label.right() + 2.0, y - 4.0), pos2(label.right() + 10.0, y + 4.0)], thin);
                }
                Some(Slide::OutUp) => {
                    painter.line_segment([pos2(label.right() + 2.0, y + 4.0), pos2(label.right() + 10.0, y - 4.0)], thin);
                }
                None => {}
            }
            if let (true, Some((j, next_label))) = (art.hammer, following) {
                let arc = slur(pos2(label.center().x, label.top()), pos2(next_label.center().x, next_label.top()));
                let peak = arc[6];
                painter.add(Shape::line(arc, thin));
                // Up is a hammer-on, down a pull-off: the letters tablatures use.
                painter.text(peak - vec2(0.0, 5.0), Align2::CENTER_CENTER, if notes[j].pitch >= note.pitch { "h" } else { "p" }, small.clone(), ink);
            }
            if !art.bend.is_empty() {
                let peak = art.bend_peak();
                let dive = peak <= 0.0;
                let lowest = art.bend.iter().map(|p| p.1).fold(0.0, f32::min);
                let from = pos2(label.right(), if dive { label.bottom() - 2.0 } else { label.top() + 2.0 });
                let tip = pos2(from.x + 8.0, from.y + if dive { gap * 0.45 } else { -gap * 0.45 });
                let curve: Vec<Pos2> = (0..=8)
                    .map(|k| {
                        let t = k as f32 / 8.0;
                        pos2(from.x + (tip.x - from.x) * t * (2.0 - t), from.y + (tip.y - from.y) * t * t)
                    })
                    .collect();
                painter.add(Shape::line(curve, thin));
                let head = if dive { 3.0 } else { -3.0 };
                painter.line_segment([tip, pos2(tip.x - 3.0, tip.y - head)], thin);
                painter.line_segment([tip, pos2(tip.x + 3.0, tip.y - head)], thin);
                // The height in tones, as tablatures write it.
                let tones = if dive { lowest } else { peak } / 2.0;
                let text = match (tones * 4.0).round() as i32 {
                    1 => "¼".to_owned(),
                    2 => "½".to_owned(),
                    4 => "full".to_owned(),
                    6 => "1½".to_owned(),
                    quarters => format!("{}", f32::from(quarters as i16) / 4.0),
                };
                painter.text(pos2(tip.x + 3.0, tip.y), Align2::LEFT_CENTER, text, small.clone(), ink);
            }
            if let Some(vibrato) = art.vibrato {
                let amplitude = if vibrato == Vibrato::Wide { 3.0 } else { 2.0 };
                let x0 = label.right() + 1.0;
                let x1 = x_of(note.start + note.length).max(x0 + 14.0);
                let wave: Vec<Pos2> = (0..)
                    .map(|k| x0 + k as f32 * 2.0)
                    .take_while(|x| *x <= x1)
                    .map(|x| pos2(x, y - amplitude * ((x - x0) / 2.5).sin()))
                    .collect();
                painter.add(Shape::line(wave, thin));
            }
            if art.staccato {
                painter.circle_filled(pos2(label.center().x, label.top() - 3.0), 1.3, ink);
            }
            if art.accent > 0 {
                painter.text(pos2(label.center().x, label.top() - 5.0), Align2::CENTER_CENTER, if art.accent > 1 { "^" } else { ">" }, small.clone(), ink);
            }
            if let Some(step) = art.tremolo {
                // Tremolo picking: a slash for eighth notes, two for sixteenths, three for thirty-seconds.
                let slashes = if step >= 0.5 { 1 } else if step >= 0.25 { 2 } else { 3 };
                let x = label.center().x;
                for k in 0..slashes {
                    let y = label.top() - 4.0 * scale - k as f32 * 3.0 * scale;
                    painter.line_segment([pos2(x - 3.0 * scale, y + 1.5 * scale), pos2(x + 3.0 * scale, y - 1.5 * scale)], stroke(1.2, ink));
                }
            }
            if art.trill.is_some() {
                painter.text(pos2(label.right() + 2.0 * scale, label.top() + 1.0 * scale), Align2::LEFT_BOTTOM, "tr", small.clone(), ink);
            }
        }
        // Where the feel changes: swung or straight from there on.
        for feel in feels {
            let text = if feel.is_straight() {
                tr("Straight")
            } else if feel.unit < 0.4 {
                tr("Swing (sixteenths)")
            } else {
                tr("Swing (eighths)")
            };
            painter.text(pos2(x_of(feel.beat) + 2.0, feel_y), Align2::LEFT_CENTER, text, small.clone(), ink.gamma_multiply(0.8));
        }
        // Palm muting and let ring: a word and a dashed line over the notes, as
        // long as they follow one another closely.
        for (flag, word, y) in [(0, "P.M.", top - 9.0 * scale), (1, "let ring", top - 18.0 * scale)] {
            let mut order: Vec<&Note> = notes.iter().filter(|n| if flag == 0 { n.articulation.palm_mute } else { n.articulation.let_ring }).collect();
            order.sort_by(|a, b| a.start.total_cmp(&b.start));
            let mut runs: Vec<(f64, f64)> = Vec::new();
            for note in order {
                match runs.last_mut() {
                    Some(run) if note.start <= run.1 + 1.0 => run.1 = run.1.max(note.start + note.length),
                    _ => runs.push((note.start, note.start + note.length)),
                }
            }
            for (start, end) in runs {
                let x0 = x_of(start).max(painter.clip_rect().left() + 2.0);
                let x1 = x_of(end);
                if x1 <= x0 {
                    continue;
                }
                let text = painter.text(pos2(x0, y), Align2::LEFT_CENTER, word, small.clone(), ink);
                if x1 > text.right() + 4.0 {
                    painter.extend(Shape::dashed_line(&[pos2(text.right() + 3.0, y), pos2(x1, y)], thin, 3.0, 2.0));
                }
            }
        }
    }

    fn tab_canvas(&mut self, ui: &mut egui::Ui, ti: usize, ri: usize) {
        const LEFT: f32 = 46.0;
        // What scrolls is drawn at the size the user chose for the text.
        let text_size = self.tab_text_size();
        let scale = text_size / TAB_TEXT;
        let gap = TAB_GAP * scale;
        // Room at the top for the chord grids and the chord names.
        let chords = TAB_CHORDS * scale;
        let bass = self.is_bass_track(ti);
        let tuning: Vec<u8> = match &self.project.tracks[ti].tuning {
            own if !own.is_empty() => own.clone(),
            _ => tab_tuning(bass).to_vec(),
        };
        let tuning = tuning.as_slice();
        let region = &self.project.tracks[ti].regions[ri];
        let cycle = region.cycle_len().min(region.length).max(SNAP);
        let region_start = region.start;
        let region_length = region.length;
        let notes: Vec<Note> = region.notes().to_vec();
        let feels = region.swing.clone();
        // Over the chords, room for the names of the sections when there are some.
        let sections = self.section_names(region_start, cycle);
        let names = if sections.is_empty() { 0.0 } else { 14.0 * scale };
        // Under the strings, room for the help line and the zoom controls.
        let line = names + chords + gap * (tuning.len() as f32 + 2.0) + 26.0;
        let positions = tab_positions(&notes, tuning);
        let font = FontId::monospace(text_size);
        let inset = TAB_INSET * scale;
        let width = ui.available_width();
        let lines = self.editor_lines(width - LEFT - inset - 16.0, cycle, region_start, region_length, TAB_CLOSEST, line, ui.available_height());
        let (resp, painter) = ui.allocate_painter(vec2(width, lines.height), Sense::click());
        let rect = resp.rect;
        let visible = ui.clip_rect().intersect(rect);
        self.roll_rect = rect;
        let painter = painter.with_clip_rect(rect);
        let color = track_color(self.project.tracks[ti].color);
        let left = rect.left() + LEFT + inset;
        let ppb = lines.ppb;
        let ink = ui.visuals().text_color();
        let background = ui.visuals().extreme_bg_color;
        painter.rect_filled(rect, 0.0, background);
        let strings = painter.with_clip_rect(Rect::from_min_max(pos2(left - inset, rect.top()), rect.max));
        let hovered_beat = self.shade_beats(ui, &strings, &resp, rect, &lines, left, line, cycle, ti, ri);
        let bar_len = self.project.bar_len();
        let last = tuning.len() - 1;
        let out_of_reach = positions.iter().filter(|p| p.is_none()).count();

        // The chords, found once for all the lines.
        let chord_marks = self.chords_of(ti, ri);
        let mut boxes: Vec<(usize, Rect)> = Vec::new();
        for system in &lines.systems {
            // Only the lines in view are drawn: in the full window, the song
            // takes many.
            let (line_top, line_bottom) = (rect.top() + system.top, rect.top() + system.top + line);
            if line_bottom < visible.top() || line_top > visible.bottom() {
                continue;
            }
            let from = system.from;
            let x_of = move |beat: f64| left + ((beat - from) * ppb) as f32;
            let top = rect.top() + system.top + names + gap * 2.5 + chords;
            let y_of = move |string: usize| top + string as f32 * gap;
            let until = system.to.min(cycle);
            let end = if lines.paged { x_of(until) } else { rect.right() };

            // Bars and beats, then the strings with their open notes.
            let mut beat = from.floor().max(0.0);
            while beat <= until + 1e-9 {
                let x = x_of(beat);
                let in_bar = (region_start + beat) % bar_len;
                let shade = if in_bar.abs() < 1e-6 { 0.7 } else { 0.2 };
                strings.line_segment([pos2(x, y_of(0)), pos2(x, y_of(last))], stroke(1.0, ink.gamma_multiply(shade)));
                beat += 1.0;
            }
            for (string, open) in tuning.iter().enumerate() {
                strings.line_segment([pos2(left - inset, y_of(string)), pos2(end, y_of(string))], stroke(1.0, ink.gamma_multiply(0.55)));
                let name = format!("{}{}", tr(NOTE_LETTERS[usize::from(open % 12)]), i32::from(*open) / 12 - 2);
                painter.text(pos2(rect.left() + 8.0, y_of(string)), Align2::LEFT_CENTER, name, FontId::proportional(11.0), ink);
            }
            for (beat, name) in sections.iter().filter(|(beat, _)| *beat >= from - 1e-6 && *beat < system.to) {
                strings.text(pos2(x_of(*beat), rect.top() + system.top + 7.0 * scale), Align2::LEFT_CENTER, name, FontId::proportional(11.0 * scale), ink);
            }

            // Each note: a faint bar for its length, and its fret on a patch of
            // background that breaks the string, as printed tablatures do.
            let mut labels: Vec<Option<Rect>> = vec![None; notes.len()];
            for (i, (note, place)) in notes.iter().zip(&positions).enumerate() {
                let Some((string, fret)) = *place else { continue };
                if note.start >= system.to || note.start + note.length <= from {
                    continue;
                }
                let at = pos2(x_of(note.start), y_of(string));
                let selected = self.selected_notes.contains(&i);
                let bar = Rect::from_min_max(pos2(at.x, at.y - 2.5), pos2(x_of(note.start + note.length).max(at.x + 4.0), at.y + 2.5));
                strings.rect_filled(bar, 1.0, color.gamma_multiply(if selected { 0.8 } else { 0.35 }));
                let galley = painter.layout_no_wrap(fret_text(note, fret), font.clone(), if selected { palette(1) } else { ink });
                let label = Rect::from_center_size(at, galley.size() + vec2(FRET_MARGIN, 0.0));
                strings.rect_filled(label, 0.0, background);
                strings.galley(label.min + vec2(2.0, 0.0), galley, ink);
                boxes.push((i, label.expand(3.0)));
                labels[i] = Some(label);
            }
            self.tab_articulations(&strings, &notes, &positions, &labels, &feels, y_of(last) + gap * 0.9, &x_of, top, scale, ink);
            self.tab_chords(&strings, &chord_marks, cycle, &notes, &positions, tuning, &x_of, left, system.to, rect.top() + system.top + names, scale, ink);
        }
        self.lines_playhead(&strings, rect, &lines, left, line, region_start, region_length, cycle, ink);
        self.follow_lines(ui, &resp, rect, &lines, line);

        let mut help = tr("Click: select · Ctrl and a click: add to the selection · Shift and a click: choose beats to loop · Ctrl and the wheel, or - and +: zoom").to_owned();
        help.push_str(" · ");
        help.push_str(tr("a click elsewhere: the playhead goes there"));
        if out_of_reach > 0 {
            help.push_str(" · ");
            help.push_str(&tf!("{out_of_reach} notes are below the lowest string or past the last fret" [out_of_reach]));
        }
        // Over the strings when the tablature is taller than its panel.
        let help = painter.layout_no_wrap(help, FontId::proportional(10.5), ink.gamma_multiply(0.6));
        let help_at = visible.left_bottom() + vec2(8.0, -4.0 - help.size().y);
        painter.rect_filled(Rect::from_min_size(help_at, help.size()).expand(3.0), 0.0, background);
        painter.galley(help_at, help, ink);
        self.editor_zoom_controls(ui, &resp, visible, left, ppb);

        if let (true, Some(p)) = (resp.clicked(), resp.interact_pointer_pos()) {
            let (shift, ctrl) = ui.input(|i| (i.modifiers.shift, i.modifiers.command));
            if shift {
                if let Some(beat) = hovered_beat {
                    self.choose_beat(ti, ri, beat);
                }
                return;
            }
            match boxes.iter().rev().find(|(_, label)| label.contains(p)).map(|(i, _)| *i) {
                Some(ni) if ctrl => {
                    if let Some(at) = self.selected_notes.iter().position(|i| *i == ni) {
                        self.selected_notes.remove(at);
                    } else {
                        self.selected_notes.push(ni);
                    }
                }
                Some(ni) => {
                    self.selected_notes = vec![ni];
                    self.play_preview(ti, notes[ni].pitch, 0.2);
                }
                None => {
                    self.selected_notes.clear();
                    self.beat_selection = None;
                    // Elsewhere, a click moves the playhead there.
                    if let Some(beat) = Self::instant_at(&lines, rect, left, line, cycle, p) {
                        self.go_to(region_start + beat);
                    }
                }
            }
        }
    }

    fn roll_canvas(&mut self, ui: &mut egui::Ui, ti: usize, ri: usize) {
        let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), 128.0 * ROW_H), Sense::click_and_drag());
        let rect = resp.rect;
        let visible = ui.clip_rect().intersect(rect);
        self.roll_rect = visible;
        let painter = painter.with_clip_rect(visible);
        let grid = Rect::from_min_max(pos2(rect.left() + KEYS_W, rect.top()), rect.max);
        let track = &self.project.tracks[ti];
        let (drums, locked, color) = (track.sound.is_drums(), track.locked, track_color(track.color));
        let region = &track.regions[ri];
        let cycle = region.cycle_len().min(region.length).max(SNAP);
        let region_start = region.start;
        let region_length = region.length;
        let notes: Vec<Note> = region.notes().to_vec();
        let (ppb, offset) = self.editor_scale(grid.width(), cycle, region_start, region_length, CLOSEST_BARS * self.project.bar_len());
        // What sits on the grid is drawn within it, not over the keyboard.
        let on_grid = painter.with_clip_rect(visible.intersect(grid));

        let x_of = move |beat: f64| grid.left() + ((beat - offset) * ppb) as f32;
        let beat_of = move |x: f32| offset + f64::from(x - grid.left()) / ppb;
        let y_of = move |pitch: u8| rect.top() + f32::from(127 - pitch.min(127)) * ROW_H;
        let pitch_of = move |y: f32| (127.0 - ((y - rect.top()) / ROW_H).floor()).clamp(0.0, 127.0) as u8;

        let line = ui.visuals().widgets.noninteractive.bg_stroke.color;
        let text_color = ui.visuals().text_color();
        let bar_len = self.project.bar_len();

        // Rows and the keyboard on the left.
        painter.rect_filled(grid, 0.0, ui.visuals().extreme_bg_color);
        for pitch in pitch_of(visible.bottom())..=pitch_of(visible.top()) {
            let y = y_of(pitch);
            let row = Rect::from_min_max(pos2(grid.left(), y), pos2(grid.right(), y + ROW_H));
            if is_black_key(pitch) && !drums {
                painter.rect_filled(row, 0.0, Color32::from_black_alpha(60));
            }
            if pitch % 12 == 0 || drums {
                painter.line_segment([pos2(grid.left(), y + ROW_H), pos2(grid.right(), y + ROW_H)], stroke(1.0, line.gamma_multiply(if drums { 0.4 } else { 1.0 })));
            }
            let key = Rect::from_min_max(pos2(rect.left(), y), pos2(grid.left() - 1.0, y + ROW_H - 1.0));
            let sounding = self.key_preview == Some((ti, pitch));
            let fill = if sounding {
                color
            } else if drums {
                Color32::from_gray(if drum_name(pitch).is_some() { 205 } else { 120 })
            } else if is_black_key(pitch) {
                Color32::from_gray(35)
            } else {
                Color32::from_gray(225)
            };
            painter.rect_filled(key, 1.0, fill);
            let label = if drums {
                drum_name(pitch).map(str::to_owned)
            } else {
                // Octave numbering where middle C (MIDI 60) is C3.
                (pitch % 12 == 0).then(|| format!("C{}", i32::from(pitch) / 12 - 2))
            };
            if let Some(label) = label {
                painter.text(pos2(key.right() - 3.0, key.center().y), Align2::RIGHT_CENTER, label, FontId::proportional(9.0), Color32::from_gray(40));
            }
        }

        // Time grid.
        let step = if ppb * SNAP >= 7.0 { SNAP } else { 1.0 };
        let mut beat = 0.0;
        while beat <= cycle {
            let in_bar = (region_start + beat) % bar_len;
            let shade = if in_bar < 1e-6 { 1.0 } else if beat.fract() == 0.0 { 0.5 } else { 0.2 };
            let x = x_of(beat);
            on_grid.line_segment([pos2(x, visible.top()), pos2(x, visible.bottom())], stroke(1.0, line.gamma_multiply(shade)));
            beat += step;
        }

        let note_rect = |note: &Note| {
            let y = y_of(note.pitch);
            Rect::from_min_max(
                pos2(x_of(note.start), y + 1.0),
                pos2(x_of(note.start + note.length).max(x_of(note.start) + 3.0), y + ROW_H - 1.0),
            )
        };
        for (i, note) in notes.iter().enumerate() {
            let body = note_rect(note);
            let strength = 0.45 + 0.55 * f32::from(note.velocity.min(127)) / 127.0;
            on_grid.rect_filled(body, 2.0, color.gamma_multiply(strength));
            let selected = self.selected_notes.contains(&i);
            on_grid.rect_stroke(body, 2.0, if selected { stroke(1.5, crate::theme::current().outline()) } else { stroke(1.0, Color32::from_black_alpha(120)) });
        }

        // Playhead, wrapped into the loop when the region repeats.
        let local = self.position - region_start;
        if local >= 0.0 && local <= self.project.tracks[ti].regions[ri].length {
            let x = x_of(local % cycle);
            on_grid.line_segment([pos2(x, visible.top()), pos2(x, visible.bottom())], stroke(1.5, text_color));
        }
        self.editor_zoom_controls(ui, &resp, visible, grid.left(), ppb);

        // Keyboard strip: hold the mouse to hear a pitch.
        let pointer = ui.input(|i| i.pointer.latest_pos());
        let wanted = if resp.is_pointer_button_down_on() && self.drag.is_none() {
            pointer.filter(|p| p.x < grid.left()).map(|p| (ti, pitch_of(p.y)))
        } else {
            None
        };
        if wanted != self.key_preview {
            let mut engine = self.engine();
            if let Some((track, pitch)) = self.key_preview {
                engine.live_note_off(track, pitch);
            }
            if let Some((track, pitch)) = wanted {
                engine.live_note_on(track, pitch, 100);
            }
            drop(engine);
            self.key_preview = wanted;
        }
        if locked {
            return;
        }

        let hit_note = |beat: f64, pitch: u8| notes.iter().rposition(|n| n.pitch == pitch && beat >= n.start && beat < n.start + n.length);
        let shift = ui.input(|i| i.modifiers.shift);

        if resp.drag_started() {
            if let Some(p) = ui.input(|i| i.pointer.press_origin()).filter(|p| p.x >= grid.left()) {
                let (beat, pitch) = (beat_of(p.x), pitch_of(p.y));
                match hit_note(beat, pitch) {
                    Some(ni) => {
                        if !self.selected_notes.contains(&ni) {
                            if !shift {
                                self.selected_notes.clear();
                            }
                            self.selected_notes.push(ni);
                        }
                        let body = note_rect(&notes[ni]);
                        let resize = body.right() - p.x <= EDGE && body.width() > EDGE * 2.0;
                        let mut originals: Vec<(usize, Note)> = self.selected_notes.iter().filter_map(|&i| notes.get(i).map(|n| (i, n.clone()))).collect();
                        self.checkpoint();
                        // Dragged with Alt or Ctrl held: copies of the selected notes are what moves.
                        let copy = !resize && ui.input(|i| i.modifiers.alt || i.modifiers.ctrl);
                        if copy {
                            if let Some(all) = self.project.tracks[ti].regions[ri].notes_mut() {
                                let first = all.len();
                                all.extend(originals.iter().map(|(_, n)| n.clone()));
                                originals = originals.into_iter().enumerate().map(|(k, (_, n))| (first + k, n)).collect();
                                self.selected_notes = originals.iter().map(|(i, _)| *i).collect();
                            }
                        }
                        self.drag = Some(if resize { Drag::ResizeNotes { beat, originals } } else { Drag::Notes { beat, pitch: i32::from(pitch), originals } });
                    }
                    None => self.drag = Some(Drag::Rubber { from: p }),
                }
            }
        }
        if let (true, Some(p)) = (resp.dragged(), resp.interact_pointer_pos()) {
            let (beat, pitch) = (beat_of(p.x), pitch_of(p.y));
            match self.drag.clone() {
                Some(Drag::Notes { beat: from, pitch: from_pitch, originals }) => {
                    let (shift_beats, shift_pitch) = (snap(beat - from, SNAP), i32::from(pitch) - from_pitch);
                    if let Some(notes) = self.project.tracks[ti].regions[ri].notes_mut() {
                        for (i, original) in originals {
                            if let Some(note) = notes.get_mut(i) {
                                note.start = (original.start + shift_beats).clamp(0.0, (cycle - original.length).max(0.0));
                                note.pitch = (i32::from(original.pitch) + shift_pitch).clamp(0, 127) as u8;
                            }
                        }
                    }
                }
                Some(Drag::ResizeNotes { beat: from, originals }) => {
                    let delta = snap(beat - from, SNAP);
                    if let Some(notes) = self.project.tracks[ti].regions[ri].notes_mut() {
                        for (i, original) in originals {
                            if let Some(note) = notes.get_mut(i) {
                                note.length = (original.length + delta).max(SNAP);
                            }
                        }
                    }
                }
                Some(Drag::Rubber { from }) => {
                    let area = Rect::from_two_pos(from, p);
                    painter.rect_filled(area, 0.0, Color32::from_white_alpha(25));
                    painter.rect_stroke(area, 0.0, stroke(1.0, crate::theme::current().outline()));
                    self.selected_notes = notes.iter().enumerate().filter(|(_, n)| note_rect(n).intersects(area)).map(|(i, _)| i).collect();
                }
                _ => {}
            }
        }
        if resp.drag_stopped() && matches!(self.drag, Some(Drag::Notes { .. } | Drag::ResizeNotes { .. } | Drag::Rubber { .. })) {
            self.drag = None;
        }

        if let Some(p) = resp.interact_pointer_pos().filter(|p| p.x >= grid.left()) {
            let (beat, pitch) = (beat_of(p.x), pitch_of(p.y));
            let hit = hit_note(beat, pitch);
            if resp.secondary_clicked() {
                if let Some(ni) = hit {
                    self.checkpoint();
                    if let Some(notes) = self.project.tracks[ti].regions[ri].notes_mut() {
                        notes.remove(ni);
                    }
                    self.selected_notes.clear();
                }
            } else if resp.clicked() && ui.input(|i| i.modifiers.alt) {
                // Alt and a click move the playhead there.
                self.go_to(region_start + beat.max(0.0));
            } else if resp.clicked() {
                match hit {
                    Some(ni) if shift => {
                        if let Some(at) = self.selected_notes.iter().position(|i| *i == ni) {
                            self.selected_notes.remove(at);
                        } else {
                            self.selected_notes.push(ni);
                        }
                    }
                    Some(ni) => {
                        self.selected_notes = vec![ni];
                        self.play_preview(ti, notes[ni].pitch, 0.2);
                    }
                    None if beat < cycle => {
                        self.checkpoint();
                        let start = (beat / SNAP).floor() * SNAP;
                        let note = Note { start, length: self.note_len.min(cycle - start), pitch, velocity: 100, string: None, ..Default::default() };
                        if let Some(notes) = self.project.tracks[ti].regions[ri].notes_mut() {
                            notes.push(note);
                            self.selected_notes = vec![notes.len() - 1];
                        }
                        self.play_preview(ti, pitch, 0.25);
                    }
                    None => {}
                }
            }
        }
    }

    // ----- score --------------------------------------------------------------------

    fn score_canvas(&mut self, ui: &mut egui::Ui, ti: usize, ri: usize) {
        let track = &self.project.tracks[ti];
        let locked = track.locked;
        let region = &track.regions[ri];
        let cycle = region.cycle_len().min(region.length).max(SNAP);
        let region_start = region.start;
        let region_length = region.length;
        let notes: Vec<Note> = region.notes().to_vec();
        let feels = region.swing.clone();
        let key = Key::of(self.project.key, self.project.minor);
        let sections = self.section_names(region_start, cycle);
        let signature = (self.project.beats_per_bar, self.project.beat_unit);
        let bar_len = self.project.bar_len();
        let ink = ui.visuals().text_color();
        // Room on the left for the clefs, the key signature and the time
        // signature, and a little more so that the first note, its accidental
        // included, is whole.
        let margin = 56.0 + 7.0 * key.count() as f32 + 8.0;
        let width = ui.available_width();
        let lines = self.editor_lines((width - margin - 16.0).max(50.0), cycle, region_start, region_length, CLOSEST_BARS * self.project.bar_len(), SCORE_LINE, ui.available_height());
        let (resp, painter) = ui.allocate_painter(vec2(width, lines.height), Sense::click_and_drag());
        let rect = resp.rect;
        let visible = ui.clip_rect().intersect(rect);
        self.roll_rect = rect;
        let painter = painter.with_clip_rect(rect);
        let left = rect.left() + margin;
        let ppb = lines.ppb;
        let faint = stroke(1.0, ink.gamma_multiply(0.6));
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let staves = painter.with_clip_rect(Rect::from_min_max(pos2(left - 12.0, rect.top()), rect.max));
        let hovered_beat = self.shade_beats(ui, &staves, &resp, rect, &lines, left, SCORE_LINE, cycle, ti, ri);

        // Where the heads are drawn, for the mouse.
        let mut heads: Vec<(usize, Pos2)> = Vec::new();
        let mids: Vec<f32> = lines.systems.iter().map(|s| rect.top() + s.top + SCORE_LINE / 2.0).collect();
        // What every line shares, found once: the signs of the key, the chords,
        // what sounds on each staff, the strings of the notes.
        let steps = key.steps();
        let chord_marks = self.chords_of(ti, ri);
        let sounding: [Vec<(f64, f64)>; 2] = [true, false].map(|treble| notes.iter().filter(|n| (key.spell(n.pitch).0 >= MIDDLE_C) == treble).map(|n| (n.start, n.start + n.length)).collect());
        let strings: Vec<Option<(usize, u8)>> = notes.iter().map(|n| n.string.map(|s| (usize::from(s), 0))).collect();
        for (index, system) in lines.systems.iter().enumerate() {
            // Only the lines in view are drawn: in the full window, the song
            // takes many.
            let (line_top, line_bottom) = (rect.top() + system.top, rect.top() + system.top + SCORE_LINE);
            if line_bottom < visible.top() || line_top > visible.bottom() {
                continue;
            }
            let (from, to) = (system.from, system.to);
            let mid = mids[index];
            let x_of = move |beat: f64| left + ((beat - from) * ppb) as f32;
            let y_of = move |step: i32| mid - (step - MIDDLE_C) as f32 * HALF;
            let end = if lines.paged { x_of(to.min(cycle)) + 8.0 } else { rect.right() - 8.0 };
            for step in TREBLE.iter().chain(&BASS) {
                painter.line_segment([pos2(rect.left() + 8.0, y_of(*step)), pos2(end, y_of(*step))], faint);
            }
            painter.text(pos2(rect.left() + 12.0, y_of(41)), Align2::LEFT_CENTER, tr("G"), FontId::proportional(15.0), ink);
            painter.text(pos2(rect.left() + 12.0, y_of(29)), Align2::LEFT_CENTER, tr("F"), FontId::proportional(15.0), ink);
            // The key signature on both staves, then the time signature on the first line.
            for (k, step) in steps.iter().enumerate() {
                let x = rect.left() + 34.0 + k as f32 * 7.0;
                for staff in [0, -14] {
                    accidental(&painter, pos2(x + 9.0, y_of(*step + staff)), key.sign(), ink);
                }
            }
            if index == 0 && from <= 1e-6 {
                for base in [41, 29] {
                    painter.text(pos2(left - 10.0, y_of(base + 2)), Align2::CENTER_CENTER, signature.0.to_string(), FontId::proportional(13.0), ink);
                    painter.text(pos2(left - 10.0, y_of(base - 2)), Align2::CENTER_CENTER, signature.1.to_string(), FontId::proportional(13.0), ink);
                }
            }
            let mut bar = ((region_start / bar_len).ceil() * bar_len) - region_start;
            while bar <= cycle.min(to) + 1e-9 {
                if bar >= from - 1e-9 {
                    let x = x_of(bar) - 6.0;
                    staves.line_segment([pos2(x, y_of(45)), pos2(x, y_of(37))], faint);
                    staves.line_segment([pos2(x, y_of(33)), pos2(x, y_of(25))], faint);
                }
                bar += bar_len;
            }
            // The names of the sections, then of the chords, over the staff.
            let names_y = rect.top() + system.top + 11.0;
            for (beat, name) in sections.iter().filter(|(beat, _)| *beat >= from - 1e-6 && *beat < to) {
                staves.text(pos2(x_of(*beat), names_y), Align2::LEFT_CENTER, name, FontId::proportional(12.0), ink);
            }
            Self::chord_names(&staves, &chord_marks, cycle, &x_of, if sections.is_empty() { names_y } else { names_y + 13.0 }, from, to, ink);

            // The rests of each staff the region uses, bar by bar.
            for (treble, sounding) in [true, false].into_iter().zip(&sounding) {
                if sounding.is_empty() {
                    continue;
                }
                let staff_mid = y_of(if treble { 41 } else { 29 });
                let mut bar_start = ((region_start / bar_len).ceil() * bar_len) - region_start - bar_len;
                while bar_start < cycle.min(to) {
                    let bar_end = (bar_start + bar_len).min(cycle);
                    if bar_end > from {
                        for (at, value) in rests_of(sounding, bar_start.max(0.0), bar_end) {
                            if at >= from - 1e-6 && at < to {
                                rest(&staves, x_of(at) + 4.0, staff_mid, value, ink);
                            }
                        }
                    }
                    bar_start += bar_len;
                }
            }

            let labels = score_notes(&staves, &notes, from, to, bar_len, region_start, signature.1, &x_of, &y_of, key, ink, &self.selected_notes, &mut heads);
            self.tab_articulations(&staves, &notes, &strings, &labels, &feels, y_of(19), &x_of, y_of(45) - 4.0, 1.0, ink);
        }
        self.lines_playhead(&staves, rect, &lines, left, SCORE_LINE, region_start, region_length, cycle, ink);
        self.follow_lines(ui, &resp, rect, &lines, SCORE_LINE);
        painter.text(visible.left_bottom() + vec2(8.0, -4.0), Align2::LEFT_BOTTOM, format!("{} · {}", tr("Click: add · drag: move · right-click: delete · Shift and a click: choose beats to loop"), tr("Alt and a click: the playhead goes there")), FontId::proportional(10.5), ink.gamma_multiply(0.6));
        self.editor_zoom_controls(ui, &resp, visible, left, ppb);
        // Alt and a click move the playhead there.
        if ui.input(|i| i.modifiers.alt) {
            if let Some(beat) = resp.interact_pointer_pos().filter(|_| resp.clicked()).and_then(|p| Self::instant_at(&lines, rect, left, SCORE_LINE, cycle, p)) {
                self.go_to(region_start + beat);
            }
            return;
        }
        // Beats are chosen in a locked track too: nothing of it changes.
        if ui.input(|i| i.modifiers.shift) {
            if let (true, Some(beat)) = (resp.clicked(), hovered_beat) {
                self.choose_beat(ti, ri, beat);
            }
            return;
        }
        if locked {
            return;
        }

        // The line under the pointer gives the beat and the step it points at.
        let system_at = |p: Pos2| lines.systems.iter().zip(&mids).find(|(s, _)| p.y >= rect.top() + s.top && p.y < rect.top() + s.top + SCORE_LINE);
        let beat_of = |p: Pos2| system_at(p).map(|(s, _)| s.from + f64::from(p.x - left) / ppb);
        let step_at = |p: Pos2| system_at(p).map(|(_, mid)| MIDDLE_C + ((mid - p.y) / HALF).round() as i32);
        let hit = |p: Pos2| heads.iter().rev().find(|(_, at)| (*at - p).length() <= 7.5).map(|(i, _)| *i);
        if resp.drag_started() {
            if let Some((p, ni)) = ui.input(|i| i.pointer.press_origin()).and_then(|p| hit(p).map(|ni| (p, ni))) {
                if let Some(beat) = beat_of(p) {
                    self.selected_notes = vec![ni];
                    self.checkpoint();
                    self.drag = Some(Drag::Notes { beat: beat - notes[ni].start, pitch: 0, originals: vec![(ni, notes[ni].clone())] });
                }
            }
        }
        if let (true, Some(Drag::Notes { beat: grab, originals, .. }), Some(p)) = (resp.dragged(), self.drag.clone(), resp.interact_pointer_pos()) {
            if let (Some((ni, original)), Some(beat), Some(step), Some(list)) = (originals.first(), beat_of(p), step_at(p), self.project.tracks[ti].regions[ri].notes_mut()) {
                if let Some(note) = list.get_mut(*ni) {
                    note.start = snap(beat - grab, SNAP).clamp(0.0, (cycle - original.length).max(0.0));
                    // Keep a sharp or a flat while the note stays on its step.
                    if step != key.spell(note.pitch).0 {
                        note.pitch = key.pitch_of(step);
                    }
                }
            }
        }
        if resp.drag_stopped() && matches!(self.drag, Some(Drag::Notes { .. })) {
            self.drag = None;
        }
        if let Some(p) = resp.interact_pointer_pos() {
            let target = hit(p);
            if let (true, Some(ni)) = (resp.secondary_clicked(), target) {
                self.checkpoint();
                if let Some(list) = self.project.tracks[ti].regions[ri].notes_mut() {
                    list.remove(ni);
                }
                self.selected_notes.clear();
            } else if resp.clicked() {
                match (target, beat_of(p), step_at(p)) {
                    (Some(ni), _, _) => self.selected_notes = vec![ni],
                    (None, Some(beat), Some(step)) if beat >= 0.0 && beat < cycle => {
                        self.checkpoint();
                        let start = snap(beat, SNAP).min(cycle - SNAP).max(0.0);
                        let pitch = key.pitch_of(step);
                        let note = Note { start, length: self.note_len.min(cycle - start), pitch, velocity: 100, string: None, ..Default::default() };
                        if let Some(list) = self.project.tracks[ti].regions[ri].notes_mut() {
                            list.push(note);
                            self.selected_notes = vec![list.len() - 1];
                        }
                        self.play_preview(ti, pitch, 0.25);
                    }
                    _ => {}
                }
            }
        }
    }

    // ----- audio --------------------------------------------------------------------

    fn audio_editor(&mut self, ui: &mut egui::Ui, ti: usize, ri: usize) {
        let locked = self.project.tracks[ti].locked;
        let clip_seconds = match &self.project.tracks[ti].regions[ri].content {
            Content::Audio(audio) => self.clips.get(&audio.clip).map(|c| c.seconds()),
            _ => None,
        };
        let mut toggle_loop = false;
        let mut fix_timing = false;
        let mut take = None;
        let mut forget = None;
        ui.horizontal_wrapped(|ui| {
            let region = &mut self.project.tracks[ti].regions[ri];
            ui.add(egui::TextEdit::singleline(&mut region.name).desired_width(120.0));
            let looped = region.loop_len.is_some();
            let takes = region.takes.len();
            let Content::Audio(audio) = &mut region.content else { return };
            ui.add_enabled_ui(!locked, |ui| {
                if takes > 0 {
                    ui.menu_button(tf!("Takes ({})", takes + 1), |ui| {
                        ui.weak(tr("Choose the take to hear"));
                        for index in 0..takes {
                            ui.horizontal(|ui| {
                                if ui.button(tf!("Other take {}", index + 1)).clicked() {
                                    take = Some(index);
                                    ui.close_menu();
                                }
                                if ui.small_button("✕").on_hover_text(tr("Forget this take")).clicked() {
                                    forget = Some(index);
                                    ui.close_menu();
                                }
                            });
                        }
                    });
                }
                ui.separator();
                ui.spacing_mut().slider_width = 80.0;
                ui.add(egui::Slider::new(&mut audio.gain, 0.0..=2.0).text(tr("Gain")));
                ui.add(egui::DragValue::new(&mut audio.transpose).range(-12..=12).speed(0.05).prefix(tr("Transposition ")).suffix(tr(" semitones")))
                    .on_hover_text(tr("Changes the pitch without changing the length"));
                let mut tune = f32::from(audio.tune);
                if ui.add(egui::Slider::new(&mut tune, 0.0..=100.0).fixed_decimals(0).text(tr("Tuning %"))).on_hover_text(tr("Brings each note closer to the nearest note in tune")).changed() {
                    audio.tune = tune as u8;
                }
                ui.separator();
                let current = NOTE_LENGTHS.iter().find(|l| l.0 == self.timing_grid).map_or("?", |l| l.1);
                egui::ComboBox::from_id_salt("timing_grid").width(52.0).selected_text(current).show_ui(ui, |ui| {
                    for (value, label) in &NOTE_LENGTHS[1..5] {
                        ui.selectable_value(&mut self.timing_grid, *value, *label);
                    }
                });
                fix_timing = ui.button(tr("Snap to grid")).on_hover_text(tr("Moves each attack onto the chosen grid")).clicked();
                ui.separator();
                ui.checkbox(&mut audio.reverse, tr("Play backwards"));
                let max = clip_seconds.unwrap_or(0.0).max(0.0);
                ui.add(egui::DragValue::new(&mut audio.offset).range(0.0..=max).speed(0.01).fixed_decimals(2).prefix(tr("Start at ")).suffix(" s"));
                if ui.button(if looped { tr("Stop looping") } else { tr("Loop region") }).clicked() {
                    toggle_loop = true;
                }
            });
            if let Some(seconds) = clip_seconds {
                ui.weak(tf!("File of {seconds:.1} s" [seconds]));
            }
        });
        if toggle_loop {
            self.toggle_loop();
        }
        if fix_timing {
            self.quantize_audio_region();
        }
        if let Some(index) = take {
            self.select_take(index);
        }
        if let Some(index) = forget {
            self.remove_take(index);
        }

        let (resp, painter) = ui.allocate_painter(vec2(ui.available_width(), ui.available_height().max(80.0)), Sense::hover());
        let rect = resp.rect;
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let Some(region) = self.region(ti, ri) else { return };
        let Content::Audio(audio) = &region.content else { return };
        let ink = track_color(self.project.tracks[ti].color);
        let (Some(peaks), Some(total)) = (self.waveforms.get(&audio.clip), clip_seconds.filter(|s| *s > 0.0)) else {
            painter.text(rect.center(), Align2::CENTER_CENTER, tr("The audio file of this region cannot be found."), FontId::proportional(13.0), ui.visuals().text_color());
            return;
        };
        // The part of the clip the region plays, from its start offset.
        let cycle = region.cycle_len().min(region.length);
        let seconds = self.project.beat_to_sec(region.start + cycle) - self.project.beat_to_sec(region.start);
        let mid = rect.center().y;
        let half = rect.height() / 2.0 - 6.0;
        for px in 0..rect.width() as usize {
            let mut at = (audio.offset + seconds * px as f64 / f64::from(rect.width())) / total;
            if audio.reverse {
                at = 1.0 - at;
            }
            if !(0.0..1.0).contains(&at) {
                continue;
            }
            let (lo, hi) = peaks[((at * peaks.len() as f64) as usize).min(peaks.len() - 1)];
            let x = rect.left() + px as f32 + 0.5;
            let gain = audio.gain.min(2.0);
            painter.line_segment([pos2(x, mid - (hi * gain).clamp(-1.0, 1.0) * half), pos2(x, mid - (lo * gain).clamp(-1.0, 1.0) * half + 0.5)], stroke(1.0, ink));
        }
        let local = self.position - region.start;
        if local >= 0.0 && local <= region.length {
            let x = rect.left() + ((local % cycle) / cycle) as f32 * rect.width();
            painter.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], stroke(1.5, ui.visuals().text_color()));
        }
    }

    // ----- drummer ------------------------------------------------------------------

    fn drummer_editor(&mut self, ui: &mut egui::Ui, ti: usize, ri: usize) {
        if self.project.tracks[ti].locked {
            ui.label(tr("This track is locked."));
            return;
        }
        let drum_patches: Vec<String> = self.patches.iter().filter(|p| p.category == patches::CAT_DRUMS).map(|p| p.name.clone()).collect();
        let current_patch = self.project.tracks[ti].patch.clone();
        let mut new_patch = None;
        // Tracks of notes the drummer can follow: place in the project and name.
        let followable: Vec<(u16, String)> =
            self.project.tracks.iter().enumerate().filter(|(i, t)| *i != ti && t.kind == og_engine::TrackKind::Instrument).map(|(i, t)| (i as u16 + 1, t.name.clone())).collect();
        let region = &mut self.project.tracks[ti].regions[ri];
        let Content::Drummer(p) = &mut region.content else { return };
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.strong(tr("Style"));
                for style in DrumStyle::ALL {
                    ui.selectable_value(&mut p.style, style, tr(style.label()));
                }
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.strong(tr("Playing"));
                ui.weak(tr("Drag the dot: simple to complex, soft to loud"));
                let (pad, resp) = ui.allocate_exact_size(vec2(170.0, 150.0), Sense::click_and_drag());
                if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
                    p.complexity = ((pos.x - pad.left()) / pad.width()).clamp(0.0, 1.0);
                    p.loudness = ((pad.bottom() - pos.y) / pad.height()).clamp(0.0, 1.0);
                }
                let painter = ui.painter();
                painter.rect_filled(pad, 6.0, ui.visuals().extreme_bg_color);
                let dim = ui.visuals().text_color().gamma_multiply(0.5);
                painter.text(pad.left_bottom() + vec2(5.0, -3.0), Align2::LEFT_BOTTOM, tr("Simple"), FontId::proportional(10.0), dim);
                painter.text(pad.right_bottom() + vec2(-5.0, -3.0), Align2::RIGHT_BOTTOM, tr("Complex"), FontId::proportional(10.0), dim);
                painter.text(pad.left_top() + vec2(5.0, 3.0), Align2::LEFT_TOP, tr("Loud"), FontId::proportional(10.0), dim);
                let dot = pos2(pad.left() + p.complexity * pad.width(), pad.bottom() - p.loudness * pad.height());
                painter.circle_filled(dot, 9.0, palette(4));
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.strong(tr("Kit pieces"));
                ui.checkbox(&mut p.kick_snare, tr("Kick and snare"));
                ui.horizontal(|ui| {
                    for (mode, label) in [(HatMode::Hats, tr("Hi-hat")), (HatMode::Ride, tr("Ride")), (HatMode::Toms, tr("Toms")), (HatMode::Off, tr("None"))] {
                        ui.selectable_value(&mut p.hats, mode, label);
                    }
                });
                ui.horizontal(|ui| {
                    for (perc, label) in [(Perc::Off, tr("No percussion")), (Perc::Tambourine, tr("Tambourine")), (Perc::Shaker, tr("Shaker")), (Perc::Clap, tr("Claps"))] {
                        ui.selectable_value(&mut p.perc, perc, label);
                    }
                });
                ui.add_space(6.0);
                ui.spacing_mut().slider_width = 150.0;
                ui.add(egui::Slider::new(&mut p.fills, 0.0..=1.0).show_value(false).text(tr("Fills")));
                ui.add(egui::Slider::new(&mut p.swing, 0.0..=1.0).show_value(false).text(tr("Swing")));
                if ui.button(tr("Other variation")).on_hover_text(tr("Plays the same idea differently")).clicked() {
                    p.seed = p.seed.wrapping_add(1);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(tr("Kick follows"));
                    let current = followable.iter().find(|f| f.0 == p.follow).map_or_else(|| tr("its own pattern").to_owned(), |f| f.1.clone());
                    egui::ComboBox::from_id_salt("drummer_follow").width(130.0).selected_text(current).show_ui(ui, |ui| {
                        ui.selectable_value(&mut p.follow, 0, tr("its own pattern"));
                        for (number, name) in &followable {
                            ui.selectable_value(&mut p.follow, *number, name);
                        }
                    })
                    .response
                    .on_hover_text(tr("The kick drum plays where the notes of that track begin, so that the drums and, say, the bass move together."));
                });
            });
            ui.separator();
            ui.vertical(|ui| {
                ui.strong(tr("Kit"));
                for name in &drum_patches {
                    if ui.selectable_label(*name == current_patch, tr(name)).clicked() {
                        new_patch = Some(name.clone());
                    }
                }
            });
        });
        if let Some(patch) = new_patch.and_then(|name| self.patches.iter().find(|p| p.name == name).cloned()) {
            self.apply_patch(&patch);
        }
    }
}
