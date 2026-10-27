//! The words of a song, to be read while it plays.
//!
//! They come in three ways. Sung: each syllable on a note of the score, as
//! Guitar Pro, Songsterr and MusicXML write them, so that the syllable sung
//! is known beat by beat. Timed: lines with the second of the recording they
//! start at, as an LRC file gives them. Plain: text without times.
//!
//! The words of Guitar Pro and Songsterr are a text in which the syllables
//! follow one another, each taken by the next beat that strikes a note:
//! a space or a hyphen ends a syllable ("laugh-ter" is two), a hyphen or an
//! underscore alone holds the syllable before over one more beat, and two
//! spaces leave a beat without words.

use serde::{Deserialize, Serialize};

/// A syllable sung at a beat of the score. It ends with a hyphen when the
/// word goes on in the next one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Syllable {
    pub beat: f64,
    pub text: String,
    /// The syllable starts a line of the words as they are shown.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub line_start: bool,
}

/// A line of words and the second of the recording it starts at.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedLine {
    pub sec: f64,
    pub text: String,
}

/// The words of a song in a project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Lyrics {
    pub sung: Vec<Syllable>,
    pub timed: Vec<TimedLine>,
    pub plain: String,
    /// Where they came from: "tablature", "LRCLIB", a file, "typed".
    pub source: String,
    /// Seconds the timed lines are moved by, later when positive.
    pub offset: f64,
}

impl Lyrics {
    pub fn is_empty(&self) -> bool {
        self.sung.is_empty() && self.timed.is_empty() && self.plain.trim().is_empty()
    }
}

/// What a piece of the text of the words does to the beats it meets.
#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    /// A syllable on the next beat.
    Syllable(String),
    /// The syllable before goes on over the next beat.
    Hold,
    /// The next beat has no words.
    Skip,
}

/// The pieces of the text of the words, in order.
pub fn tokens(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    // A line of the text only separates: the spaces around it are no beats
    // without words. Its first syllable starts a line of the words shown,
    // marked by a line feed before it.
    for line in text.split('\n') {
        let line = line.replace(['\r', '\t'], " ");
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let first = out.len();
        tokens_of_line(line, &mut out);
        if text.contains('\n') {
            if let Some(Token::Syllable(s)) = out[first..].iter_mut().find(|t| matches!(t, Token::Syllable(_))) {
                s.insert(0, '\n');
            }
        }
    }
    out
}

/// The pieces of one line of the text of the words.
fn tokens_of_line(text: &str, out: &mut Vec<Token>) {
    for piece in text.split(' ') {
        if piece.is_empty() {
            out.push(Token::Skip);
        } else if piece.chars().all(|c| c == '-' || c == '_') {
            out.push(Token::Hold);
        } else {
            // "laugh-ter": "laugh-" then "ter"; a hyphen that starts the piece joins it to the one before.
            let parts: Vec<&str> = piece.split('-').collect();
            let last = parts.len() - 1;
            for (k, part) in parts.iter().enumerate().filter(|(_, p)| !p.is_empty()) {
                out.push(Token::Syllable(if k < last { format!("{part}-") } else { (*part).to_owned() }));
            }
        }
    }
}

/// Gives the syllables of a text to the beats that strike, in order: what
/// each beat gets, if anything.
pub fn bind(text: &str, beats: usize) -> Vec<Option<String>> {
    let mut given = vec![None; beats];
    for (slot, token) in given.iter_mut().zip(tokens(text)) {
        if let Token::Syllable(s) = token {
            *slot = Some(s);
        }
    }
    given
}

/// A beat of the singing part as written, as the words are fitted to it:
/// its bar, where it falls (in the bar and from the start of the part),
/// how long it lasts, whether it strikes a note (a tied note does not) and
/// whether something sounds on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Beat {
    pub bar: usize,
    pub position: f64,
    pub at: f64,
    pub length: f64,
    pub strikes: bool,
    pub sounds: bool,
}

/// A beat that strikes a note, which a syllable may take, and the silence
/// before it, in beats.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Slot {
    pub bar: usize,
    pub position: f64,
    pub gap: f64,
}

/// The beats that strike, with the silence before each.
pub fn slots(beats: &[Beat]) -> Vec<Slot> {
    let mut out = Vec::new();
    let mut sounded_until = f64::NEG_INFINITY;
    for beat in beats {
        if beat.strikes {
            out.push(Slot { bar: beat.bar, position: beat.position, gap: (beat.at - sounded_until).max(0.0) });
        }
        if beat.sounds {
            sounded_until = beat.at + beat.length;
        }
    }
    out
}

/// Which pieces start a line of the words: the first of each line of the
/// text, or, a text in one line, a word written with a capital.
fn line_starts(pieces: &[Token]) -> Vec<bool> {
    let mut starts = Vec::with_capacity(pieces.len());
    let mut word_ended = true;
    for piece in pieces {
        let start = match piece {
            Token::Syllable(s) if s.starts_with('\n') => true,
            Token::Syllable(s) => word_ended && s.chars().next().is_some_and(char::is_uppercase) && !always_capital(s),
            _ => false,
        };
        if let Token::Syllable(s) = piece {
            word_ended = !s.ends_with('-');
        }
        starts.push(start);
    }
    if let Some(first) = starts.first_mut() {
        *first = true;
    }
    starts
}

/// Fits the pieces of the words to the beats that strike, over the whole
/// song at once: a line starts after a silence of the singing; a hold or
/// a space too many may take a note or not; a note may go without a
/// syllable (a vowel held over several notes); a syllable left out costs
/// most. The text of the words is written by hand and does not always
/// follow its own rules: fitting the whole keeps one mistake from moving
/// every syllable after it. Gives each slot its syllable, if any.
pub fn fit(pieces: &[Token], slots: &[Slot]) -> Vec<Option<String>> {
    const EMPTY_NOTE: f32 = 0.35;
    const UNUSED_PIECE: f32 = 0.3;
    const LOST_SYLLABLE: f32 = 4.0;
    const START_NOT_AFTER_REST: f32 = 2.0;
    const PAUSE_IN_A_LINE: f32 = 0.6;
    let (n, m) = (pieces.len(), slots.len());
    let starts = line_starts(pieces);
    let width = m + 1;
    let mut cost = vec![f32::INFINITY; (n + 1) * width];
    // How each state was reached: 0 a piece on a note, 1 a note without one, 2 a piece without a note.
    let mut from = vec![0u8; (n + 1) * width];
    cost[0] = 0.0;
    for i in 0..=n {
        for j in 0..=m {
            let here = cost[i * width + j];
            if !here.is_finite() {
                continue;
            }
            let mut reach = |to: usize, value: f32, way: u8| {
                if value < cost[to] {
                    cost[to] = value;
                    from[to] = way;
                }
            };
            if i < n && j < m {
                let rest = slots[j].gap >= 0.49;
                let penalty = match &pieces[i] {
                    Token::Syllable(_) if starts[i] && !rest => START_NOT_AFTER_REST,
                    Token::Syllable(_) if !starts[i] && slots[j].gap >= 1.0 => PAUSE_IN_A_LINE,
                    _ => 0.0,
                };
                reach((i + 1) * width + j + 1, here + penalty, 0);
            }
            if j < m {
                // Notes after the last syllable cost nothing.
                reach(i * width + j + 1, here + if i == n { 0.0 } else { EMPTY_NOTE }, 1);
            }
            if i < n {
                let lost = if matches!(pieces[i], Token::Syllable(_)) { LOST_SYLLABLE } else { UNUSED_PIECE };
                reach((i + 1) * width + j, here + lost, 2);
            }
        }
    }
    let mut given = vec![None; m];
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        match from[i * width + j] {
            0 => {
                i -= 1;
                j -= 1;
                if let Token::Syllable(s) = &pieces[i] {
                    given[j] = Some(s.clone());
                }
            }
            1 => j -= 1,
            _ => i -= 1,
        }
    }
    given
}

/// Places the syllables of a line of words on the beats played: `slots` are
/// the beats that strike, from the bar the line starts at; `order` the bars
/// in the order they are played and `starts` where each starts. A bar
/// played again sings its syllables again; a beat that has one already
/// keeps it.
pub fn place(text: &str, slots: &[Slot], order: &[usize], starts: &[f64], out: &mut Vec<Syllable>) {
    let given = fit(&tokens(text), slots);
    for (&index, &bar_start) in order.iter().zip(starts) {
        for (slot, syllable) in slots.iter().zip(&given) {
            let (bar, position) = (&slot.bar, &slot.position);
            if let (true, Some(text)) = (*bar == index, syllable) {
                let beat = bar_start + position;
                if !out.iter().any(|s| (s.beat - beat).abs() < 1e-6) {
                    // A line feed before it: it starts a line of the text.
                    let line_start = text.starts_with('\n');
                    out.push(Syllable { beat, text: text.trim_start_matches('\n').to_owned(), line_start });
                }
            }
        }
    }
}

/// The words of a score: those of its first part that has some (the singing).
pub fn from_parts(parts: &[crate::songsterr::Part]) -> Lyrics {
    match parts.iter().find(|p| !p.lyrics.is_empty()) {
        Some(part) => Lyrics { sung: part.lyrics.clone(), source: "tablature".to_owned(), ..Lyrics::default() },
        None => Lyrics::default(),
    }
}

/// Whether a syllable is a word of its own that is written with a capital
/// in the middle of a line too.
fn always_capital(text: &str) -> bool {
    let word: String = text.chars().take_while(|c| c.is_alphabetic() || *c == '\'' || *c == '’').collect();
    matches!(word.as_str(), "I" | "I'm" | "I’m" | "I've" | "I’ve" | "I'll" | "I’ll" | "I'd" | "I’d" | "Lord" | "God")
}

/// Marks where the lines of the words start: at a word written with a
/// capital, after a pause of a bar or more, or when a line grows long.
pub fn mark_lines(sung: &mut [Syllable], bar: f64) {
    // Lines the text itself gives are kept as they are.
    if sung.iter().skip(1).any(|s| s.line_start) {
        if let Some(first) = sung.first_mut() {
            first.line_start = true;
        }
        return;
    }
    let mut length = 0usize;
    for k in 0..sung.len() {
        let starts_word = k == 0 || !sung[k - 1].text.ends_with('-');
        let capital = sung[k].text.chars().next().is_some_and(char::is_uppercase) && !always_capital(&sung[k].text);
        let pause = k > 0 && sung[k].beat - sung[k - 1].beat >= bar.max(1.0) * 1.5;
        let start = k == 0 || starts_word && (capital || pause || length > 48);
        sung[k].line_start = start;
        if start {
            length = 0;
        }
        length += sung[k].text.chars().count() + 1;
    }
}

/// The words of syllables, joined: "laugh-" and "ter" make "laughter".
pub fn join(syllables: &[Syllable]) -> String {
    let mut out = String::new();
    for (k, syllable) in syllables.iter().enumerate() {
        if k > 0 && !syllables[k - 1].text.ends_with('-') {
            out.push(' ');
        }
        out.push_str(&shown(&syllable.text));
    }
    out
}

/// A syllable as it is read: without the hyphen that joins it to the next,
/// nor the vowel a note holds, written in brackets ("(o)n" is "n" after
/// "po": "upon").
pub fn shown(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text.trim_end_matches('-');
    while let Some(open) = rest.find('(') {
        match rest[open..].find(')') {
            Some(close) if close <= 4 => {
                out.push_str(&rest[..open]);
                rest = &rest[open + close + 1..];
            }
            _ => break,
        }
    }
    out.push_str(rest);
    out
}

/// The lines of an LRC file: "[01:23.45]words", several times on a line
/// for a line sung several times; "[offset:+250]" moves them all.
pub fn parse_lrc(text: &str) -> Vec<TimedLine> {
    let mut lines = Vec::new();
    let mut offset = 0.0;
    for raw in text.lines() {
        let mut rest = raw.trim();
        let mut times = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some((tag, after)) = stripped.split_once(']') else { break };
            rest = after;
            if let Some(ms) = tag.strip_prefix("offset:") {
                offset = ms.trim().parse::<f64>().unwrap_or(0.0) / 1000.0;
                continue;
            }
            let Some((minutes, seconds)) = tag.split_once(':') else { continue };
            if let (Ok(minutes), Ok(seconds)) = (minutes.trim().parse::<f64>(), seconds.trim().parse::<f64>()) {
                times.push(minutes * 60.0 + seconds);
            }
        }
        for sec in times {
            lines.push(TimedLine { sec, text: rest.trim().to_owned() });
        }
    }
    // A positive offset makes the words come sooner.
    for line in &mut lines {
        line.sec -= offset;
    }
    lines.sort_by(|a, b| a.sec.total_cmp(&b.sec));
    lines
}

/// The words of a song, laid out to be read: lines of syllables (sung) or of
/// text, each with when it starts, and which one is being sung at an instant.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// When it starts: a beat, or a second of the recording.
    pub at: f64,
    pub text: String,
    /// Sung lines: the syllables and the beat of each.
    pub syllables: Vec<Syllable>,
}

/// The lines of sung words.
pub fn sung_lines(sung: &[Syllable]) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    for syllable in sung {
        if syllable.line_start || lines.is_empty() {
            lines.push(Line { at: syllable.beat, text: String::new(), syllables: Vec::new() });
        }
        if let Some(line) = lines.last_mut() {
            line.syllables.push(syllable.clone());
        }
    }
    for line in &mut lines {
        line.text = join(&line.syllables);
    }
    lines
}

/// The line being sung at `at` (a beat for sung lines, a second for timed
/// ones): the last that started; none before the first.
pub fn current(lines: &[Line], at: f64) -> Option<usize> {
    lines.partition_point(|line| line.at <= at + 1e-9).checked_sub(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_text_of_the_words_is_cut_into_syllables() {
        let words = |text: &str| tokens(text);
        assert_eq!(words("Eve- ry time"), [Token::Syllable("Eve-".into()), Token::Syllable("ry".into()), Token::Syllable("time".into())]);
        assert_eq!(words("laugh-ter"), [Token::Syllable("laugh-".into()), Token::Syllable("ter".into())]);
        assert_eq!(words("li- - ke"), [Token::Syllable("li-".into()), Token::Hold, Token::Syllable("ke".into())]);
        assert_eq!(words("time  that"), [Token::Syllable("time".into()), Token::Skip, Token::Syllable("that".into())]);
        assert_eq!(words("dream on ___ Ah!"), [Token::Syllable("dream".into()), Token::Syllable("on".into()), Token::Hold, Token::Syllable("Ah!".into())]);
        // A line of the text only separates, however many spaces and empty lines
        // come with it; its first syllable starts a line.
        assert_eq!(words("one

   two  three"), [Token::Syllable("
one".into()), Token::Syllable("
two".into()), Token::Skip, Token::Syllable("three".into())]);
        // The vowel a note holds, in brackets, is not read.
        assert_eq!((shown("u-"), shown("po-"), shown("(o)n"), shown("(Live)")), ("u".into(), "po".into(), "n".into(), "(Live)".into()));
        // Each beat that strikes takes the next piece.
        assert_eq!(bind("Eve- ry time  that", 6), [Some("Eve-".into()), Some("ry".into()), Some("time".into()), None, Some("that".into()), None]);
    }

    #[test]
    fn the_words_are_laid_out_in_lines() {
        let text = "Eve- ry time that I look in the mirr- or All these lines on my face";
        let mut sung: Vec<Syllable> = bind(text, 30).into_iter().enumerate().filter_map(|(k, s)| s.map(|text| Syllable { beat: k as f64, text, line_start: false })).collect();
        mark_lines(&mut sung, 4.0);
        let lines = sung_lines(&sung);
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["Every time that I look in the mirror", "All these lines on my face"], "a capital starts a line, not “I”");
        assert_eq!(current(&lines, 0.5), Some(0));
        assert_eq!(current(&lines, lines[1].at), Some(1));
        assert_eq!(current(&lines, -1.0), None);
    }

    #[test]
    fn an_lrc_file_is_read() {
        let lines = parse_lrc("[ar:Someone]\n[offset:+500]\n[00:12.50]First line\n[00:15.00][01:15.00]Chorus\nnot a line\n");
        let seen: Vec<(f64, &str)> = lines.iter().map(|l| (l.sec, l.text.as_str())).collect();
        assert_eq!(seen, [(12.0, "First line"), (14.5, "Chorus"), (74.5, "Chorus")]);
    }
}

#[cfg(test)]
mod fitting {
    use super::*;

    /// Beats of a tune: two phrases of four notes, a rest of two beats between them.
    fn tune() -> Vec<Beat> {
        let mut beats = Vec::new();
        for (k, at) in [0.0, 1.0, 2.0, 3.0, 6.0, 7.0, 8.0, 9.0].iter().enumerate() {
            beats.push(Beat { bar: k / 4, position: *at % 4.0, at: *at, length: 1.0, strikes: true, sounds: true });
        }
        beats
    }

    #[test]
    fn a_space_too_many_does_not_move_the_lines_after_it() {
        let slots = slots(&tune());
        assert_eq!(slots.iter().map(|s| s.gap).collect::<Vec<_>>()[3..5], [0.0, 2.0], "the rest before the second phrase");
        // "time  that": two spaces that are no beat without words in this tune.
        let given = fit(&tokens("Eve- ry time  that All these lines on"), &slots);
        let words: Vec<&str> = given.iter().map(|g| g.as_deref().unwrap_or("·")).collect();
        assert_eq!(words, ["Eve-", "ry", "time", "that", "All", "these", "lines", "on"], "the second line starts after the rest");
        // Read one after the other, the space would have pushed it off.
        let one_by_one: Vec<Option<String>> = bind("Eve- ry time  that All these lines on", 8);
        assert_eq!(one_by_one[5].as_deref(), Some("All"));
    }

    #[test]
    fn a_held_vowel_leaves_a_note_without_a_syllable() {
        let slots = slots(&tune());
        let given = fit(&tokens("Oh my love All the way"), &slots);
        let words: Vec<&str> = given.iter().map(|g| g.as_deref().unwrap_or("·")).collect();
        assert_eq!(words[4], "All", "{words:?}");
        assert!(given[..4].iter().filter(|g| g.is_some()).count() == 3);
    }
}
