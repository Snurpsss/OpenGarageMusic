//! The side panels: sound library and loop browser.

use crate::i18n::tr;
use crate::userloops::LoopPick;
use eframe::egui::{self, Align, Layout};
use og_engine::loops;
use og_engine::patches::{self, PatchDef};
use og_engine::TrackKind;

use crate::app::App;

/// Library categories offered to a track; imported sound banks come last.
fn categories_for(kind: TrackKind, banks: Vec<String>) -> Vec<String> {
    let fixed: &[&str] = match kind {
        TrackKind::Instrument => &[
            patches::CAT_KEYS,
            patches::CAT_SYNTH,
            patches::CAT_BASS,
            patches::CAT_GUITAR,
            patches::CAT_ORCH,
            patches::CAT_MALLET,
            patches::CAT_DRUMS,
            patches::CAT_USER,
        ],
        TrackKind::Drummer => &[patches::CAT_DRUMS],
        TrackKind::Audio => &[patches::CAT_VOICE, patches::CAT_AMP, patches::CAT_USER],
        TrackKind::Guitar => &[patches::CAT_AMP, patches::CAT_VOICE, patches::CAT_USER],
        TrackKind::Video => &[],
    };
    let mut all: Vec<String> = fixed.iter().map(|c| (*c).to_owned()).collect();
    if !kind.is_audio() {
        all.extend(banks);
    }
    all
}

impl App {
    pub fn library_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading(tr("Library"));
        let Some(track) = self.project.tracks.get(self.selected_track) else {
            ui.weak(tr("Select a track to choose its sound."));
            return;
        };
        let (kind, current, locked) = (track.kind, track.patch.clone(), track.locked);
        if kind == TrackKind::Video {
            ui.weak(tr("A video track plays the sound of its video: it has no sound of the library."));
            return;
        }
        let categories = categories_for(kind, self.font_names());
        if !categories.contains(&self.lib_category) {
            self.lib_category.clone_from(&categories[0]);
        }
        ui.weak(tf!("Sounds for: {}", track.name));
        ui.add(egui::TextEdit::singleline(&mut self.lib_search).hint_text(tr("Search for a sound")).desired_width(f32::INFINITY));
        let search = self.lib_search.trim().to_lowercase();
        ui.add_space(2.0);
        if search.is_empty() {
            for category in &categories {
                ui.selectable_value(&mut self.lib_category, category.clone(), tr(category));
            }
            if self.font_files.is_empty() && !kind.is_audio() {
                ui.weak(tr("Real instruments: Library menu, “Download sounds”."));
            }
            ui.separator();
        }
        if locked {
            ui.weak(tr("Track locked."));
            return;
        }

        let shown: Vec<PatchDef> = self
            .patches
            .iter()
            .filter(|p| categories.contains(&p.category))
            .filter(|p| if search.is_empty() { p.category == self.lib_category } else { tr(&p.name).to_lowercase().contains(&search) })
            // A drummer needs a drum kit; audio tracks only take the effects of a patch.
            .filter(|p| kind != TrackKind::Drummer || p.sound.is_drums())
            .cloned()
            .collect();
        let mut chosen = None;
        let mut removed = None;
        let scroll_to = self.lib_scroll_to.take();
        egui::ScrollArea::vertical().id_salt("library_list").auto_shrink([false, false]).show(ui, |ui| {
            if shown.is_empty() {
                ui.weak(if search.is_empty() { tr("No sound here yet.") } else { tr("No sound matches.") });
            }
            for patch in &shown {
                ui.horizontal(|ui| {
                    let row = ui.selectable_label(patch.name == current, tr(&patch.name));
                    if scroll_to.as_deref() == Some(patch.name.as_str()) {
                        row.scroll_to_me(Some(Align::Center));
                    }
                    if row.clicked() {
                        chosen = Some(patch.clone());
                    }
                    if patch.category == patches::CAT_USER && ui.small_button("x").on_hover_text(tr("Delete from “My sounds”")).clicked() {
                        removed = Some(patch.name.clone());
                    }
                });
            }
        });
        if let Some(patch) = chosen {
            self.apply_patch(&patch);
        }
        if let Some(name) = removed {
            self.settings.user_patches.retain(|p| p.name != name);
            self.save_settings();
            self.rebuild_patches();
        }
    }

    pub fn loops_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        ui.heading(tr("Loops"));
        ui.weak(tr("They follow the tempo and the key of the project."));
        ui.add(egui::TextEdit::singleline(&mut self.loop_search).hint_text(tr("Search for a loop")).desired_width(f32::INFINITY));

        fn chips(ui: &mut egui::Ui, label: &str, options: &[&'static str], selected: &mut Option<&'static str>) {
            ui.horizontal_wrapped(|ui| {
                ui.weak(label);
                for option in options {
                    let on = *selected == Some(*option);
                    if ui.selectable_label(on, tr(option)).clicked() {
                        *selected = if on { None } else { Some(*option) };
                    }
                }
            });
        }
        chips(ui, tr("Instrument"), &loops::INSTRUMENTS, &mut self.loop_instrument);
        chips(ui, tr("Genre"), &loops::GENRES, &mut self.loop_genre);
        chips(ui, tr("Mood"), &loops::MOODS, &mut self.loop_mood);
        ui.checkbox(&mut self.loop_favorites_only, tr("My favourites only"));
        ui.separator();

        let search = self.loop_search.trim().to_lowercase();
        let matching: Vec<(usize, &'static str, u32)> = self
            .loops
            .iter()
            .enumerate()
            .filter(|(_, l)| self.loop_instrument.is_none_or(|v| l.instrument == v))
            .filter(|(_, l)| self.loop_genre.is_none_or(|v| l.genre == v))
            .filter(|(_, l)| self.loop_mood.is_none_or(|v| l.mood == v))
            .filter(|(_, l)| search.is_empty() || tr(l.name).to_lowercase().contains(&search))
            .filter(|(_, l)| !self.loop_favorites_only || self.settings.loop_favorites.iter().any(|f| f == l.name))
            .map(|(i, l)| (i, l.name, l.bars))
            .collect();

        let (mut audition, mut add, mut favorite) = (None, None, None);
        let (mut add_audio, mut import) = (None, false);
        self.loop_rects.clear();
        egui::ScrollArea::vertical().id_salt("loops_list").auto_shrink([false, false]).show(ui, |ui| {
            // The user's own loops of recorded sound come first.
            ui.horizontal(|ui| {
                ui.strong(tr("Audio loops"));
                import = ui.small_button(tr("Add files…")).on_hover_text(tr("Copies audio files of yours into the loop browser. A tempo in the name (“Funk 98 bpm”) is used; otherwise it is worked out from the length.")).clicked();
            });
            let shown: Vec<(usize, String)> =
                self.audio_loops.iter().enumerate().filter(|(_, l)| search.is_empty() || l.name.to_lowercase().contains(&search)).map(|(i, l)| (i, l.name.clone())).collect();
            if self.audio_loops.is_empty() {
                ui.weak(tr("None yet. No recorded loop comes with the program: add your own files."));
            }
            for (index, name) in &shown {
                ui.horizontal(|ui| {
                    let label = ui.add(egui::Label::new(name).selectable(false).sense(egui::Sense::drag())).on_hover_cursor(egui::CursorIcon::Grab).on_hover_text(tr("Drag onto a track, or under the tracks for a new one"));
                    label.dnd_set_drag_payload(LoopPick::Audio(*index));
                    self.loop_rects.push((LoopPick::Audio(*index), label.rect));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(tr("Add")).on_hover_text(tr("Puts the loop at the playhead")).clicked() {
                            add_audio = Some(*index);
                        }
                    });
                });
            }
            ui.separator();
            ui.strong(tr("Loops of notes"));
            if matching.is_empty() {
                ui.weak(tr("No loop matches these criteria."));
            }
            for (index, name, bars) in &matching {
                ui.horizontal(|ui| {
                    let playing = self.auditioning.as_deref() == Some(*name);
                    if ui.small_button(if playing { tr("Stop") } else { tr("Listen") }).clicked() {
                        audition = Some(*index);
                    }
                    let label = ui
                        .add(egui::Label::new(tr(name)).selectable(false).sense(egui::Sense::drag()))
                        .on_hover_cursor(egui::CursorIcon::Grab)
                        .on_hover_text(tf!("{bars} bars" [bars]));
                    label.dnd_set_drag_payload(LoopPick::Notes(*index));
                    self.loop_rects.push((LoopPick::Notes(*index), label.rect));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.small_button(tr("Add")).on_hover_text(tr("Puts the loop at the playhead")).clicked() {
                            add = Some(*index);
                        }
                        let mut fav = self.settings.loop_favorites.iter().any(|f| f == name);
                        if ui.checkbox(&mut fav, "").on_hover_text(tr("Favourite")).changed() {
                            favorite = Some((*name, fav));
                        }
                    });
                });
            }
        });
        if let Some(index) = audition {
            self.toggle_audition(index);
        }
        if let Some(index) = add {
            self.add_loop(index);
        }
        if let Some(index) = add_audio {
            let bar = self.project.bar_len();
            let start = (self.position / bar).floor() * bar;
            self.add_loop_at(LoopPick::Audio(index), None, start);
        }
        if import {
            self.import_audio_loops();
        }
        if let Some((name, on)) = favorite {
            self.settings.loop_favorites.retain(|f| f != name);
            if on {
                self.settings.loop_favorites.push(name.to_owned());
            }
            self.save_settings();
        }
    }
}
