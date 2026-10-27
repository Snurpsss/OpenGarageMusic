//! The words of the song, shown under the editor while it plays.
//!
//! They come, in this order, from the tablature (its singing part, each
//! syllable on its note: see `og_engine::lyrics`), from LRCLIB (lrclib.net,
//! a free base of words timed line by line, asked by the artist and the
//! title, no account), or from the user: an LRC or text file beside their
//! recording of the song, another file they choose, or words they type.
//!
//! Shown two ways (View menu): the line being sung, the syllable sung
//! lit, with the line before and the one after; or the whole text, the
//! line being sung lit, which scrolls along. A click on a line takes the
//! song there.

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use eframe::egui::{self, RichText};
use og_engine::lyrics::{self, Line, Lyrics, TimedLine};
use og_engine::recording::{beat_at, lead, sec_at};

use crate::app::{App, Dialog};
use crate::downloads::{agent, encode_query};
use crate::files;
use crate::i18n::tr;

/// What LRCLIB gave: timed lines, or else plain text.
pub struct Found {
    pub timed: Vec<TimedLine>,
    pub plain: String,
}

/// Asks LRCLIB for the words of a song; `seconds` (how long the recording
/// lasts), when known, picks the one of the same length among several.
pub fn search_lrclib(artist: &str, title: &str, seconds: Option<f64>) -> Result<Option<Found>, String> {
    if title.trim().is_empty() {
        return Ok(None);
    }
    let url = format!("https://lrclib.net/api/search?track_name={}&artist_name={}", encode_query(title.trim()), encode_query(artist.trim()));
    let user_agent = concat!("OpenGarageMusic/", env!("OG_VERSION"));
    let answer = agent(20)?.get(&url).set("User-Agent", user_agent).call().map_err(|e| tf!("LRCLIB did not answer ({e})" [e]))?;
    let text = answer.into_string().map_err(|e| e.to_string())?;
    let list: Vec<serde_json::Value> = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    // Timed words first, then the length nearest the recording's.
    let score = |entry: &serde_json::Value| {
        let timed = entry.get("syncedLyrics").and_then(|v| v.as_str()).is_some_and(|s| !s.trim().is_empty());
        let off = match (seconds, entry.get("duration").and_then(serde_json::Value::as_f64)) {
            (Some(want), Some(has)) => (want - has).abs(),
            _ => 0.0,
        };
        (!timed, (off * 10.0) as i64)
    };
    let best = list.iter().filter(|e| !e.get("instrumental").and_then(serde_json::Value::as_bool).unwrap_or(false)).min_by_key(|e| score(e));
    Ok(best.map(|entry| Found {
        timed: lyrics::parse_lrc(entry.get("syncedLyrics").and_then(|v| v.as_str()).unwrap_or("")),
        plain: entry.get("plainLyrics").and_then(|v| v.as_str()).unwrap_or("").to_owned(),
    }))
}

/// The words in a file: an LRC file, timed, or else plain text.
pub fn read_file(path: &Path) -> Option<Lyrics> {
    let text = std::fs::read_to_string(path).ok()?;
    let timed = lyrics::parse_lrc(&text);
    let source = path.display().to_string();
    if timed.is_empty() {
        (!text.trim().is_empty()).then(|| Lyrics { plain: text, source, ..Lyrics::default() })
    } else {
        Some(Lyrics { timed, source, ..Lyrics::default() })
    }
}

/// The words beside a recording: the same name, as .lrc or .txt.
pub fn beside(recording: &Path) -> Option<Lyrics> {
    ["lrc", "LRC", "txt", "TXT"].iter().find_map(|ext| read_file(&recording.with_extension(ext)))
}

/// Words typed: timed when they carry times ("[01:23.45]"), else plain.
pub fn typed(text: &str) -> Lyrics {
    let timed = lyrics::parse_lrc(text);
    if timed.is_empty() {
        Lyrics { plain: text.to_owned(), source: "typed".to_owned(), ..Lyrics::default() }
    } else {
        Lyrics { timed, source: "typed".to_owned(), ..Lyrics::default() }
    }
}

impl App {
    /// The words as lines, and where the song is in their reckoning: the
    /// beat for sung words, the second of the recording for timed ones;
    /// none for plain text.
    pub fn words_lines(&self) -> (Vec<Line>, Option<f64>) {
        let words = &self.project.lyrics;
        if !words.sung.is_empty() {
            return (lyrics::sung_lines(&words.sung), Some(self.position));
        }
        if !words.timed.is_empty() {
            let lines = words.timed.iter().map(|l| Line { at: l.sec + words.offset, text: l.text.clone(), syllables: Vec::new() }).collect();
            return (lines, Some(self.recording_second()));
        }
        let lines = words.plain.lines().map(|text| Line { at: f64::NAN, text: text.to_owned(), syllables: Vec::new() }).collect();
        (lines, None)
    }

    /// The second of the recording the song is at: by the recording that
    /// leads, or the first there is, else at the tempo of the score.
    fn recording_second(&self) -> f64 {
        let timing = lead(&self.project).map(|ti| &self.project.tracks[ti].timing);
        timing.and_then(|t| sec_at(t, self.position)).unwrap_or_else(|| self.project.beat_to_sec(self.position))
    }

    /// The beat of a line, to take the song there.
    fn line_beat(&self, line: &Line) -> Option<f64> {
        if !line.at.is_finite() {
            return None;
        }
        if !self.project.lyrics.sung.is_empty() {
            return Some(line.at);
        }
        let timing = lead(&self.project).map(|ti| &self.project.tracks[ti].timing);
        timing.and_then(|t| beat_at(t, line.at)).or_else(|| Some(line.at * self.project.tempo / 60.0))
    }

    /// Asks LRCLIB for the words, in the background.
    pub fn search_words(&mut self) {
        let (artist, title) = (self.project.artist.clone(), if self.project.title.is_empty() { self.project.name.clone() } else { self.project.title.clone() });
        let seconds = lead(&self.project).and_then(|ti| {
            let clip = self.project.tracks[ti].regions.first().and_then(|r| if let og_engine::Content::Audio(a) = &r.content { Some(a.clip) } else { None })?;
            self.clips.get(&clip).map(|c| c.seconds())
        });
        let (send, receive) = mpsc::channel();
        // The tests ask nothing of the network: the search is only begun.
        if cfg!(test) {
            std::mem::forget(send);
            self.words_search = Some(receive);
            return;
        }
        std::thread::spawn(move || {
            let _ = send.send(search_lrclib(&artist, &title, seconds));
        });
        self.words_search = Some(receive);
        self.words_message = tr("Asking LRCLIB for the words…").to_owned();
    }

    /// Takes the words LRCLIB gave; failing that, those beside the recording.
    pub fn poll_words(&mut self) {
        let Some(result) = crate::app::take_result(&mut self.words_search, tr("the search stopped unexpectedly")) else { return };
        match result {
            Ok(Some(found)) if !found.timed.is_empty() || !found.plain.trim().is_empty() => {
                self.project.lyrics = Lyrics { timed: found.timed, plain: found.plain, source: "LRCLIB".to_owned(), ..Lyrics::default() };
                self.words_message.clear();
            }
            Ok(_) => {
                if !self.words_beside_recording() {
                    self.words_message = tr("LRCLIB has no words for this song. Choose a file of them, or type them.").to_owned();
                }
            }
            Err(e) => {
                if !self.words_beside_recording() {
                    self.words_message = e;
                }
            }
        }
    }

    /// The words beside the user's recording of the song, if there are some.
    pub fn words_beside_recording(&mut self) -> bool {
        let paths: Vec<PathBuf> = self.project.tracks.iter().filter(|t| !t.timing.is_empty()).flat_map(|t| t.regions.iter()).filter_map(|r| match &r.content {
            og_engine::Content::Audio(a) => self.project.clips.iter().find(|c| c.id == a.clip).map(|c| self.absolute_path(&c.path)),
            _ => None,
        }).collect();
        match paths.iter().find_map(|p| beside(p)) {
            Some(words) => {
                self.project.lyrics = words;
                self.words_message.clear();
                true
            }
            None => false,
        }
    }

    /// The words of a file the user chooses.
    pub fn words_from_file(&mut self) {
        let Some(path) = files::open_file(tr("Words (LRC, text)"), &["lrc", "txt"]) else { return };
        match read_file(&path) {
            Some(words) => {
                self.checkpoint();
                self.project.lyrics = words;
                self.words_message.clear();
            }
            None => self.words_message = tr("This file holds no words.").to_owned(),
        }
    }

    /// Opens the window where the words are typed or pasted.
    pub fn type_words(&mut self) {
        let words = &self.project.lyrics;
        self.words_draft = if !words.plain.is_empty() {
            words.plain.clone()
        } else if !words.timed.is_empty() {
            words.timed.iter().map(|l| format!("[{:02}:{:05.2}]{}", (l.sec / 60.0).floor() as u32, l.sec % 60.0, l.text)).collect::<Vec<_>>().join("\n")
        } else {
            lyrics::sung_lines(&words.sung).iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
        };
        self.dialog = Some(Dialog::Words);
    }

    /// The window of the words typed. Gives whether it is done.
    pub fn words_window(&mut self, ui: &mut egui::Ui) -> bool {
        ui.label(tr("Type or paste the words, a line of the song a line of text. Lines that start with a time, as “[01:23.45]”, follow the recording."));
        egui::ScrollArea::vertical().max_height(ui.available_height() - 40.0).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut self.words_draft).desired_width(f32::INFINITY).desired_rows(18));
        });
        let mut done = false;
        ui.horizontal(|ui| {
            if ui.button(tr("Keep these words")).clicked() {
                self.checkpoint();
                self.project.lyrics = typed(&self.words_draft);
                done = true;
            }
            if ui.button(tr("Cancel")).clicked() {
                done = true;
            }
        });
        done
    }

    /// The height of the band of the words under the editor; none when it is hidden.
    pub fn words_band_height(&self) -> f32 {
        if self.settings.hide_lyrics || self.bottom_tab != crate::app::BottomTab::Editor {
            0.0
        } else if self.settings.words_height > 0.0 {
            self.settings.words_height.clamp(crate::ui_bottom::WORDS_LEAST, crate::ui_bottom::WORDS_MOST)
        } else if self.settings.lyrics_whole {
            128.0
        } else {
            100.0
        }
    }

    /// The band of the words, under the editor.
    pub fn words_band(&mut self, ui: &mut egui::Ui) {
        let (lines, now) = self.words_lines();
        let current = now.and_then(|now| lyrics::current(&lines, now));
        let ink = ui.visuals().text_color();
        let lit = ui.visuals().selection.stroke.color;
        // What the words are and where they come from, and what can be done.
        ui.horizontal(|ui| {
            let source = match self.project.lyrics.source.as_str() {
                "" => String::new(),
                "tablature" => tr("from the tablature").to_owned(),
                "typed" => tr("typed").to_owned(),
                other => other.to_owned(),
            };
            ui.strong(tr("Words"));
            if !source.is_empty() {
                ui.weak(source);
            }
            if self.words_search.is_some() {
                ui.spinner();
            }
            if !self.words_message.is_empty() {
                ui.weak(&self.words_message);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button(tr("Type…")).on_hover_text(tr("Type or paste the words")).clicked() {
                    self.type_words();
                }
                if ui.small_button(tr("From a file…")).on_hover_text(tr("An LRC file (timed) or a text file")).clicked() {
                    self.words_from_file();
                }
                if ui.add_enabled(self.words_search.is_none(), egui::Button::new(tr("Search LRCLIB")).small()).on_hover_text(tr("Asks lrclib.net, a free base of timed words, by the artist and the title of the project")).clicked() {
                    self.search_words();
                }
                if !self.project.lyrics.timed.is_empty() {
                    if ui.small_button("0.1 s ▶").on_hover_text(tr("The words come later")).clicked() {
                        self.project.lyrics.offset += 0.1;
                    }
                    if ui.small_button("◀ 0.1 s").on_hover_text(tr("The words come sooner")).clicked() {
                        self.project.lyrics.offset -= 0.1;
                    }
                }
            });
        });
        if lines.is_empty() {
            ui.weak(tr("No words for this song yet."));
            return;
        }
        let mut go = None;
        if self.settings.lyrics_whole {
            // The whole text, the line being sung lit, scrolled to it.
            let changed = current != self.words_line;
            self.words_line = current;
            egui::ScrollArea::vertical().id_salt("words").auto_shrink([false, false]).show(ui, |ui| {
                for (k, line) in lines.iter().enumerate() {
                    let text = if line.text.is_empty() { " ".to_owned() } else { line.text.clone() };
                    let mut rich = RichText::new(text).size(15.0);
                    if Some(k) == current {
                        rich = rich.color(lit).strong();
                    }
                    let response = ui.add(egui::Label::new(rich).sense(egui::Sense::click()));
                    if response.clicked() {
                        go = self.line_beat(line);
                    }
                    if changed && Some(k) == current {
                        response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
            });
        } else {
            // The line before, faint; the line being sung, its sung syllables
            // lit; the next one, to see it come.
            let at = current.unwrap_or(0);
            let shown = [at.checked_sub(1), Some(at), Some(at + 1)];
            for (k, index) in shown.into_iter().enumerate() {
                let Some(line) = index.and_then(|i| lines.get(i)) else { continue };
                let response = if k == 1 && current.is_some() {
                    let mut job = egui::text::LayoutJob::default();
                    let font = egui::FontId::proportional(19.0);
                    if line.syllables.is_empty() {
                        job.append(&line.text, 0.0, egui::TextFormat::simple(font, lit));
                    } else {
                        for (s, syllable) in line.syllables.iter().enumerate() {
                            let sung = now.is_some_and(|now| syllable.beat <= now + 1e-9);
                            let space = if s > 0 && !line.syllables[s - 1].text.ends_with('-') { " " } else { "" };
                            let colour = if sung { lit } else { ink };
                            job.append(&format!("{space}{}", og_engine::lyrics::shown(&syllable.text)), 0.0, egui::TextFormat::simple(font.clone(), colour));
                        }
                    }
                    ui.add(egui::Label::new(job).sense(egui::Sense::click()))
                } else {
                    let colour = if k == 0 { ink.gamma_multiply(0.35) } else { ink.gamma_multiply(0.7) };
                    ui.add(egui::Label::new(RichText::new(&line.text).size(if k == 0 { 13.0 } else { 15.0 }).color(colour)).sense(egui::Sense::click()))
                };
                if response.clicked() {
                    go = self.line_beat(line);
                }
            }
        }
        if let Some(beat) = go {
            self.go_to(beat.max(0.0));
        }
        if self.playing {
            ui.ctx().request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_typed_or_in_a_file_are_read() {
        let plain = typed("First line\nSecond line");
        assert_eq!((plain.plain.as_str(), plain.timed.len(), plain.source.as_str()), ("First line\nSecond line", 0, "typed"));
        let timed = typed("[00:01.00]One\n[00:03.50]Two");
        assert_eq!(timed.timed.len(), 2);
        let dir = std::env::temp_dir().join(format!("opengaragemusic-words-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let song = dir.join("Song.mp3");
        assert!(beside(&song).is_none());
        std::fs::write(dir.join("Song.lrc"), "[00:10.00]Hello").unwrap();
        let found = beside(&song).unwrap();
        assert_eq!(found.timed[0].text, "Hello");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Asks the real LRCLIB. Run on demand:
    /// `dev test -p opengaragemusic -- --ignored lrclib_answers --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn lrclib_answers() {
        let found = search_lrclib("Metallica", "Nothing Else Matters", Some(388.0)).unwrap().unwrap();
        println!("{} timed lines; first {:?}", found.timed.len(), found.timed.first().map(|l| (l.sec, l.text.clone())));
        assert!(found.timed.len() > 10);
    }
}
