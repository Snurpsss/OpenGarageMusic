//! The window of the sites: the site to search, the songs found, the tracks
//! of the chosen one to make a project of, and the way in for the Guitar
//! Pro and MusicXML files the user downloads themselves; or, on a site that
//! gives captures or impulse responses, the files of the one chosen, to
//! download into the library. The sites are those the descriptions of
//! `crate::sites` name; the program reads a site only once the user agreed
//! to it, and a site that asks for an account once the user signed in.

use eframe::egui;

use crate::app::App;
use crate::files;
use crate::i18n::tr;
use crate::sites::Source;
use crate::theme::warning;

impl App {
    /// Three columns: the sites, the songs found on the chosen one, and the
    /// tracks of the chosen song. The columns take the height the main
    /// window leaves, so that the window never outgrows it.
    pub fn sites_window(&mut self, ui: &mut egui::Ui) {
        // The columns share the window as it is, half the main window at
        // first or as the user resized it; under them, room for the lines
        // of the file and the message.
        let height = (ui.available_height() - 112.0).max(100.0);
        let width = ui.available_width();
        let left_w = (width * 0.2).clamp(100.0, 150.0);
        let right_w = (width * 0.34).clamp(170.0, 290.0);
        let middle_w = (width - left_w - right_w - 28.0).max(150.0);
        let language = crate::i18n::language();
        let (mut open_site, mut choose, mut import) = (None, None, false);
        let searching = self.sites.finding.is_some();
        let gives = self.site().map_or_else(|| "tablature".to_owned(), |s| s.gives.clone());
        let lists = self.site().is_some_and(|s| s.lists_only());
        ui.horizontal_top(|ui| {
            // ----- the sites ----------------------------------------------------------
            ui.vertical(|ui| {
                ui.set_width(left_w);
                ui.set_min_height(height);
                ui.strong(tr("Sites"));
                for (kind, heading) in [("tablature", tr("Tablatures")), ("capture", tr("Captures")), ("impulse", tr("Impulse responses"))] {
                    if !self.known_sites.iter().any(|k| k.site.gives == kind) {
                        continue;
                    }
                    ui.add_space(4.0);
                    ui.weak(heading);
                for (index, known) in self.known_sites.iter().enumerate().filter(|(_, k)| k.site.gives == kind) {
                    let site = &known.site;
                    let mut hint = site.offers.get(&language).to_owned();
                    if let Source::File(path) = &known.source {
                        let file = path.display().to_string();
                        hint.push_str(&format!("\n{}", tf!("Described by “{file}”." [file])));
                    }
                    if site.open_only {
                        hint.push_str(&format!(" {}", tr("Its terms of use forbid programs to read it: the program only opens the site.")));
                        if ui.selectable_label(false, format!("{} ↗", site.name)).on_hover_text(hint).clicked() {
                            open_site = Some(site.home.clone());
                        }
                    } else if ui.selectable_label(self.sites.site == index, &site.name).on_hover_text(hint).clicked() {
                        choose = Some(index);
                    }
                }
                }
                ui.add_space(8.0);
                if let Some(site) = self.site().filter(|s| !s.open_only) {
                    if ui.link(site.home.trim_start_matches("https://").trim_start_matches("www.")).on_hover_text(tr("Opens the site in your browser")).clicked() {
                        open_site = Some(site.home.clone());
                    }
                }
                ui.add_space(8.0);
                if self.site().is_some_and(|s| s.login.is_some()) {
                    ui.weak(tr("The site is read with your account, for what you ask only."));
                } else {
                    ui.weak(tr("The public tablatures are read; no account is needed."));
                }
                if !self.site_problems.is_empty() {
                    ui.add_space(8.0);
                    ui.colored_label(warning(), tr("Some descriptions of sites were not read:")).on_hover_text(self.site_problems.join("\n"));
                }
            });
            ui.separator();

            // ----- the search and the songs found --------------------------------------
            ui.vertical(|ui| {
                ui.set_width(middle_w);
                ui.set_min_height(height);
                ui.strong(match gives.as_str() {
                    "capture" => tr("Captures"),
                    "impulse" => tr("Impulse responses"),
                    _ => tr("Songs or artists"),
                });
                if !self.site_confirmed() {
                    self.site_agreement(ui);
                    return;
                }
                if !self.sites_login(ui) {
                    return;
                }
                ui.horizontal(|ui| {
                    let hint = if lists { tr("A word to sift the list (optional)") } else { tr("Artist or title") };
                    let field = ui.add(egui::TextEdit::singleline(&mut self.sites.search).hint_text(hint).desired_width(middle_w - 70.0));
                    let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if ui.add_enabled(!searching, egui::Button::new(if lists { tr("List") } else { tr("Find") })).clicked() || enter {
                        self.sites_search();
                    }
                });
                let (mut retry, mut pick) = (false, None);
                egui::ScrollArea::vertical().id_salt("sites_songs").max_height(height - 50.0).auto_shrink([false, false]).show(ui, |ui| {
                    if searching {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.weak(tr("Asking the site…"));
                        });
                        return;
                    }
                    match &self.sites.found {
                        None if lists => {
                            ui.weak(tr("Press List: your favourites on the site, what is trending and what is new. A word in the box sifts the list."));
                        }
                        None => {
                            let site = self.site().map_or_else(String::new, |s| s.name.clone());
                            ui.weak(tf!("Type an artist or a title, then Find. The tablatures come from {site}, where its users shared them." [site]));
                        }
                        Some(Err(error)) => {
                            ui.colored_label(warning(), tf!("Search failed: {error}" [error]));
                            retry = ui.button(tr("Retry")).clicked();
                        }
                        Some(Ok(found)) if found.is_empty() => {
                            ui.weak(tr("Nothing found."));
                        }
                        Some(Ok(found)) => {
                            for (index, item) in found.iter().enumerate() {
                                let label = if item.badge.is_empty() { item.label.clone() } else { format!("{}   ·   {}", item.label, item.badge) };
                                if ui.selectable_label(self.sites.selected == Some(index), label).clicked() {
                                    pick = Some(index);
                                }
                            }
                        }
                    }
                });
                if retry {
                    self.sites_search();
                }
                if let Some(index) = pick {
                    self.sites_select(index);
                }
            });
            ui.separator();

            // ----- the tracks of the chosen song ------------------------------------------
            ui.vertical(|ui| {
                ui.set_width(right_w);
                ui.set_min_height(height);
                if gives == "tablature" {
                    ui.strong(tr("Tracks to import"));
                    self.sites_tracks(ui, height - 110.0);
                } else {
                    ui.strong(tr("Files"));
                    self.sites_files(ui, height - 110.0);
                }
            });
        });
        if let Some(site) = choose {
            self.sites_choose(site);
        }
        if !self.sites.message.is_empty() {
            ui.separator();
            if self.sites.error {
                ui.colored_label(warning(), &self.sites.message);
            } else {
                ui.label(&self.sites.message);
            }
        }
        ui.separator();

        // ----- the files, and the one thing to remember ---------------------------------
        if gives == "tablature" {
            ui.horizontal(|ui| {
                if ui.button(tr("Import a Guitar Pro file…")).on_hover_text(tr("Makes a project of a Guitar Pro file (.gp3, .gp4, .gp5, .gpx or .gp) or a MusicXML score (.musicxml, .xml or .mxl) you downloaded or wrote: its tracks, tuning, tempo, sections and techniques")).clicked() {
                    import = true;
                }
                ui.weak(tr("A file downloaded from one of these sites, or made with Guitar Pro."));
            });
            if !self.import_message.is_empty() {
                ui.colored_label(warning(), &self.import_message);
            }
            ui.weak(tr("Tablatures written by the users of these sites are of songs that are, for the most part, under copyright: what you import is for your own practice, not to be passed on with a project."));
        } else {
            ui.weak(tr("Each capture and impulse response is under the licence its author chose, written beside it in the library: most may be used and played freely, not passed on."));
        }
        if let Some(url) = open_site {
            self.open_external(&url);
        }
        if import {
            self.import_guitar_pro();
        }
    }

    /// The way in to a site that asks for an account: the key of the program
    /// at the site, then the sign-in by a code entered on the site. Whether
    /// the site can be read now.
    fn sites_login(&mut self, ui: &mut egui::Ui) -> bool {
        let Some(site) = self.site().cloned() else { return false };
        let Some(login) = site.login.clone() else { return true };
        let name = site.name.clone();
        if self.site_signed_in() {
            ui.horizontal(|ui| {
                ui.weak(tf!("Signed in to {name}." [name]));
                if ui.small_button(tr("Sign out")).on_hover_text(tr("The program forgets the token the site gave it")).clicked() {
                    self.sites_sign_out();
                }
            });
            return true;
        }
        if self.site_key().is_empty() {
            ui.label(tr("This site asks each program for a key of its own. Make one in your account on the site (a “publishable key”) and paste it here; the program keeps it."));
            ui.hyperlink_to(tr("Where keys are made"), &login.keys);
            ui.horizontal(|ui| {
                ui.add(egui::TextEdit::singleline(&mut self.sites.key_text).hint_text(tr("Key")).desired_width(260.0));
                if ui.button(tr("Keep the key")).clicked() {
                    self.sites_keep_key();
                }
            });
            return false;
        }
        match self.sites.device.clone() {
            Some(code) => {
                let user_code = code.user_code.clone();
                ui.label(tf!("On the site's page, enter the code {user_code}, signed in to your account." [user_code]));
                ui.horizontal(|ui| {
                    if ui.button(tr("Open the page")).clicked() {
                        let url = if code.verification_uri_complete.is_empty() { code.verification_uri.clone() } else { code.verification_uri_complete.clone() };
                        self.open_external(&url);
                    }
                    ui.spinner();
                    ui.weak(tr("Waiting for you to sign in…"));
                });
            }
            None => {
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.sites.logging_in.is_none(), egui::Button::new(tf!("Sign in to {name}…" [name]))).on_hover_text(tr("The program asks the site for a code; you enter it on the site, in your browser, signed in to your account. The program then reads the site with your account and downloads only what you ask for.")).clicked() {
                        self.sites_sign_in();
                    }
                    if self.sites.logging_in.is_some() {
                        ui.spinner();
                    }
                });
            }
        }
        false
    }

    /// The right column of a site that gives files: the files it listed for
    /// the one chosen, to download into the library.
    fn sites_files(&mut self, ui: &mut egui::Ui, list_height: f32) {
        let Some(found) = self.sites.chosen_song().cloned() else {
            ui.weak(tr("Choose one on the left."));
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(&found.label);
            if !found.badge.is_empty() {
                egui::Frame::none().fill(ui.visuals().faint_bg_color).rounding(egui::Rounding::same(4.0)).inner_margin(egui::Margin::symmetric(5.0, 1.0)).show(ui, |ui| {
                    ui.small(&found.badge);
                });
            }
        });
        if self.sites.opening.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(tr("Asking the site…"));
            });
            return;
        }
        let Some(files) = self.sites.files.clone() else {
            if self.sites.error && !self.sites.message.is_empty() {
                ui.colored_label(warning(), &self.sites.message);
                if ui.button(tr("Retry")).clicked() {
                    if let Some(index) = self.sites.selected {
                        self.sites_select(index);
                    }
                }
            }
            return;
        };
        if files.is_empty() {
            ui.weak(tr("The site lists no file for it."));
            return;
        }
        self.all_or_none(ui);
        egui::ScrollArea::vertical().id_salt("sites_files").max_height(list_height.max(60.0)).auto_shrink([false, false]).show(ui, |ui| {
            for (index, (name, bytes)) in files.iter().enumerate() {
                let Some(chosen) = self.sites.chosen.get_mut(index) else { continue };
                ui.checkbox(chosen, name).on_hover_text(tf!("{} kB", bytes.len() / 1024));
            }
        });
        ui.add_space(6.0);
        let any = self.sites.chosen.iter().any(|c| *c);
        if ui.add_enabled(any && self.sites.saved.is_none(), egui::Button::new(tr("Download to the library")))
            .on_hover_text(tr("The files go into the library's folder of the site, with where they came from; the amp of a track then takes them with “Capture…” or “Cabinet IR…”, the reverb with “Room IR…”"))
            .clicked()
        {
            self.sites_save_files();
        }
        if let Some(folder) = self.sites.saved.clone() {
            if ui.small_button(tr("Open the folder")).clicked() {
                self.open_external(&folder.to_string_lossy());
            }
        }
    }

    /// Before the program reads a site for the first time: what it will do,
    /// and the user's agreement.
    fn site_agreement(&mut self, ui: &mut egui::Ui) {
        let Some(known) = self.known_sites.get(self.sites.site) else { return };
        let site = known.site.name.clone();
        let home = known.site.home.clone();
        ui.add_space(6.0);
        ui.label(tf!("The program has not read {site} yet. It will search it and fetch the tablatures you choose, as its description says, from {home}." [site, home]));
        if let Source::File(path) = &known.source {
            let file = path.display().to_string();
            ui.colored_label(warning(), tf!("This description was added to the program by a file of the sites folder (“{file}”), not by the program." [file]));
        }
        ui.weak(tr("Whether a site may be read this way is for you to check: the program reads it at your request, for your own use."));
        ui.add_space(6.0);
        if ui.button(tf!("Use {site}" [site])).on_hover_text(tr("The program will not ask again for this site")).clicked() {
            self.confirm_site();
        }
    }

    /// The right column: the tracks the site listed with the chosen song, or
    /// the parts of its file once read.
    fn sites_tracks(&mut self, ui: &mut egui::Ui, list_height: f32) {
        let Some(found) = self.sites.chosen_song().cloned() else {
            ui.weak(tr("Choose a song on the left."));
            return;
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(&found.label);
            if !found.badge.is_empty() {
                egui::Frame::none().fill(ui.visuals().faint_bg_color).rounding(egui::Rounding::same(4.0)).inner_margin(egui::Margin::symmetric(5.0, 1.0)).show(ui, |ui| {
                    ui.small(&found.badge);
                });
            }
        });
        let reading = self.sites.opening.is_some();
        if found.tracks.is_empty() {
            if reading {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.weak(tr("Reading the tablature…"));
                });
                return;
            }
            if self.sites.score.is_none() {
                // The file was not read: say so here, where the parts would be.
                if self.sites.error && !self.sites.message.is_empty() {
                    ui.colored_label(warning(), &self.sites.message);
                    if ui.button(tr("Retry")).clicked() {
                        if let Some(index) = self.sites.selected {
                            self.sites_select(index);
                        }
                    }
                }
                return;
            }
        }
        let names: Vec<(String, String)> = if found.tracks.is_empty() {
            self.sites.score.as_ref().map_or_else(Vec::new, |score| {
                score
                    .parts
                    .iter()
                    .enumerate()
                    .map(|(index, part)| {
                        let name = if part.name.trim().is_empty() { tf!("Part {}", index + 1) } else { part.name.clone() };
                        let what = if part.is_drums() { tr("Drums") } else { tr(og_engine::songsterr::patch_for(part.instrument_id)) };
                        (name, what.to_owned())
                    })
                    .collect()
            })
        } else {
            found
                .tracks
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    let name = if track.name.trim().is_empty() { tf!("Part {}", index + 1) } else { track.name.clone() };
                    let what = if track.drums { tr("Drums").to_owned() } else { track.detail.clone() };
                    (name, what)
                })
                .collect()
        };
        self.all_or_none(ui);
        egui::ScrollArea::vertical().id_salt("sites_parts").max_height(list_height.max(60.0)).auto_shrink([false, false]).show(ui, |ui| {
            for (index, (name, what)) in names.into_iter().enumerate() {
                let Some(chosen) = self.sites.chosen.get_mut(index) else { continue };
                ui.checkbox(chosen, name).on_hover_text(what);
            }
        });
        ui.add_space(6.0);
        self.original_sound(ui);
        ui.add_space(6.0);
        if reading {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(tr("Reading the tablature…"));
            });
        } else if self.create_button(ui) {
            self.sites_create();
        }
    }

    /// The recordings the song can come with, to play it with its original
    /// sound: the video the site gives, and a recording of the user's, found
    /// in their music library or chosen.
    fn original_sound(&mut self, ui: &mut egui::Ui) {
        ui.strong(tr("Original sound"));
        ui.checkbox(&mut self.sites.want_words, tr("The words of the song"))
            .on_hover_text(tr("From the singing part of the tablature, fetched even when it is not ticked; or else from LRCLIB, a free base of words"));
        if self.site().is_some_and(|s| s.videos.is_some()) {
            ui.add(egui::Checkbox::new(&mut self.sites.want_video, tr("YouTube video in step with the tablature")))
                .on_hover_text(tr("The site's video of the song, as a track: selected, it shows in the editor; the song can be played with its sound, the tablature following it"));
        }
        match self.sites.recording.clone() {
            Some(path) => {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.sites.use_recording, tf!("Your recording: {}", crate::recording::file_name(&path))).on_hover_text(path.display().to_string());
                    if ui.small_button("✕").on_hover_text(tr("Not this one")).clicked() {
                        self.sites.recording = None;
                        self.sites.use_recording = false;
                    }
                });
            }
            None => {
                ui.horizontal(|ui| {
                    if ui.small_button(tr("A recording of the song…")).on_hover_text(tr("An MP3, OGG, FLAC, M4A or WAV file of yours: it is found in step with the tablature once the project is made")).clicked() {
                        if let Some(path) = files::audio_file() {
                            self.sites.recording = Some(path);
                            self.sites.use_recording = true;
                        }
                    }
                });
                if self.settings.music_folder.trim().is_empty() {
                    ui.add(egui::Label::new(egui::RichText::new(tr("(or choose your music library folder in File > Options)")).weak()).wrap());
                }
            }
        }
    }

    fn all_or_none(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.small_button(tr("All")).clicked() {
                self.sites.chosen.iter_mut().for_each(|c| *c = true);
            }
            if ui.small_button(tr("None")).clicked() {
                self.sites.chosen.iter_mut().for_each(|c| *c = false);
            }
        });
    }

    /// The button that makes the project; enabled when a track is ticked.
    fn create_button(&self, ui: &mut egui::Ui) -> bool {
        let any = self.sites.chosen.iter().any(|c| *c);
        ui.add_enabled(any, egui::Button::new(tr("Create the project")))
            .on_hover_text(tr("Replaces the current project with the chosen tracks, one track of notes each, with what the tablature writes around the notes."))
            .clicked()
    }
}
