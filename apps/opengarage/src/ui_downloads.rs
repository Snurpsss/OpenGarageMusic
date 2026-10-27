//! The download window: sites on the left, what they offer under a free
//! licence in the middle, and on the right where it goes — a section of the
//! library for sounds, the list of installed effects for plug-ins.

use crate::i18n::tr;
use std::sync::atomic::Ordering;

use eframe::egui;
use og_engine::patches;

use crate::app::{App, Section};
use crate::downloads::{Kind, Provider};
use crate::theme::{done, warning};
const COLUMN_H: f32 = 420.0;

fn megabytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        tf!("{:.1} GB", bytes as f64 / 1e9)
    } else {
        tf!("{:.0} MB", (bytes as f64 / 1e6).max(1.0))
    }
}

impl App {
    pub fn downloads_window(&mut self, ui: &mut egui::Ui) {
        let provider = self.downloads.provider;
        let kind = provider.kind();
        let folder = match kind {
            Kind::Banks => self.banks_dir(),
            Kind::Effects => self.effects_dir(),
            Kind::Captures => self.captures_dir(),
            Kind::Impulses => self.impulses_dir(),
        };
        ui.horizontal(|ui| {
            for (candidate, label) in [(Kind::Banks, tr("Sound banks")), (Kind::Effects, tr("Effects and instruments (plug-ins)"))] {
                if ui.selectable_label(kind == candidate, label).clicked() && kind != candidate {
                    self.open_downloads(candidate);
                }
            }
            ui.separator();
            ui.weak(tf!("Folder: {}", folder.display()));
            if ui.small_button(tr("Open")).clicked() {
                let _ = std::fs::create_dir_all(&folder);
                self.open_external(&folder.to_string_lossy());
            }
        });
        ui.separator();
        ui.horizontal_top(|ui| {
            // ----- sites ---------------------------------------------------------
            ui.vertical(|ui| {
                ui.set_width(190.0);
                ui.set_min_height(COLUMN_H);
                ui.strong(tr("Sites"));
                for candidate in Provider::of(kind) {
                    if ui.selectable_label(provider == *candidate, candidate.label()).clicked() && provider != *candidate {
                        self.downloads.provider = *candidate;
                        self.refresh_downloads();
                    }
                }
                ui.add_space(10.0);
                ui.label(provider.about());
                if ui.link(tr("Open the site in the browser")).clicked() {
                    self.open_external(provider.site());
                }
            });
            ui.separator();

            // ----- what the site offers --------------------------------------------
            ui.vertical(|ui| {
                ui.set_width(380.0);
                ui.strong(tr("Available under a free licence"));
                if provider.direct() && provider != Provider::MuseScore {
                    ui.horizontal(|ui| {
                        let field = ui.add(egui::TextEdit::singleline(&mut self.downloads.search).hint_text(tr("Search")).desired_width(240.0));
                        let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                        if ui.button(tr("Find")).clicked() || enter {
                            self.refresh_downloads();
                        }
                    });
                }
                let mut retry = false;
                let mut clicked = None;
                egui::ScrollArea::vertical().id_salt("download_items").max_height(COLUMN_H - 40.0).auto_shrink([false, false]).show(ui, |ui| {
                    if !provider.direct() {
                        ui.weak(tr("The application does not download directly from this site: see the explanation on the left."));
                        return;
                    }
                    match &self.downloads.listing {
                        None => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.weak(tr("Asking the site…"));
                            });
                        }
                        Some(Err(error)) => {
                            ui.colored_label(warning(), tf!("List unavailable: {error}" [error]));
                            retry = ui.button(tr("Retry")).clicked();
                        }
                        Some(Ok(items)) if items.is_empty() => {
                            ui.weak(tr("Nothing matches under a free licence."));
                        }
                        Some(Ok(items)) => {
                            for (index, item) in items.iter().enumerate() {
                                let have = self.downloads.have.get(index).copied().unwrap_or(false);
                                let mut label = egui::RichText::new(format!("{}{}   ·   {}", if have { "✔ " } else { "" }, item.name, item.license));
                                if have {
                                    label = label.color(done());
                                }
                                let row = ui.selectable_label(self.downloads.selected == Some(index), label);
                                if have {
                                    row.clone().on_hover_text(tr("Already downloaded"));
                                }
                                if row.clicked() {
                                    clicked = Some(index);
                                }
                            }
                        }
                    }
                });
                if retry {
                    self.refresh_downloads();
                }
                if let Some(index) = clicked {
                    self.select_download(index);
                }
            });
            ui.separator();

            // ----- where it goes -------------------------------------------------------
            ui.vertical(|ui| {
                ui.set_min_width(280.0);
                match kind {
                    Kind::Captures | Kind::Impulses => {}
                    Kind::Banks => {
                        ui.strong(tr("Filing in the library"));
                        let section = &mut self.downloads.section;
                        ui.radio_value(section, Section::Auto, tr("By instrument family (keyboards, basses, strings…)"))
                            .on_hover_text(tr("For General MIDI banks: each instrument joins the section of its family"));
                        ui.radio_value(section, Section::Own, tr("In a section named after the bank"));
                        let mut single = matches!(section, Section::Category(_));
                        if ui.radio_value(&mut single, true, tr("Everything in one section:")).clicked() && !matches!(section, Section::Category(_)) {
                            *section = Section::Category(patches::CAT_KEYS.to_owned());
                        }
                        if let Section::Category(chosen) = section {
                            egui::ComboBox::from_id_salt("download_section").width(190.0).selected_text(chosen.as_str()).show_ui(ui, |ui| {
                                for category in [patches::CAT_KEYS, patches::CAT_SYNTH, patches::CAT_BASS, patches::CAT_GUITAR, patches::CAT_ORCH, patches::CAT_MALLET, patches::CAT_DRUMS] {
                                    ui.selectable_value(chosen, category.to_owned(), category);
                                }
                            });
                        }
                    }
                    Kind::Effects => {
                        ui.strong(tr("Filing"));
                        ui.label(tr("List of installed effects: the plug-in becomes available in “Add a plug-in…”, for every track (instruments: for instrument tracks)."));
                        ui.colored_label(warning(), tr("A plug-in is a program. It is neither checked nor isolated: it runs inside the application with your rights, and a faulty plug-in can crash it."));
                    }
                }
                ui.separator();

                let item = self.downloads.selected.and_then(|i| self.downloads.listing.as_ref()?.as_ref().ok()?.get(i)).cloned();
                let have = self.downloads.selected.and_then(|i| self.downloads.have.get(i).copied()).unwrap_or(false);
                // An item of a list of projects has no address until it is looked up.
                let looking_up = self.downloads.resolving.as_ref().is_some_and(|r| Some(r.0) == self.downloads.selected);
                let blocked = self.downloads.blocked.clone().filter(|b| Some(b.0) == self.downloads.selected).map(|b| b.1);
                let mut look_again = false;
                match &item {
                    None => {
                        ui.weak(tr("Choose an item in the middle list."));
                    }
                    Some(item) => {
                        ui.label(egui::RichText::new(&item.name).strong());
                        if !item.author.is_empty() {
                            ui.label(tf!("Author: {}", item.author));
                        }
                        ui.label(tf!("Licence: {}", item.license));
                        ui.label(match item.size {
                            Some(size) => tf!("Size: about {}", megabytes(size)),
                            None => tr("Size: unknown before the download").to_owned(),
                        });
                        if !item.note.is_empty() {
                            ui.weak(&item.note);
                        }
                        if !item.page.is_empty() && ui.link(tr("See the page of this item")).clicked() {
                            self.open_external(&item.page);
                        }
                        if have {
                            ui.colored_label(done(), tr("✔ Already downloaded"));
                            let folder = crate::downloads::item_folder(self.data_dir(), item);
                            if ui.link(tr("Open its folder")).clicked() {
                                self.open_external(&folder.to_string_lossy());
                            }
                        }
                        if looking_up {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.weak(tr("Checking the licence and the published files…"));
                            });
                        }
                        if let Some(reason) = &blocked {
                            ui.colored_label(warning(), tf!("Direct download not possible: {reason}" [reason]));
                            look_again = ui.small_button(tr("Check again")).clicked();
                        }
                    }
                }
                if look_again {
                    if let Some(index) = self.downloads.selected {
                        self.select_download(index);
                    }
                }
                ui.add_space(6.0);

                let mut cancel = false;
                match &self.downloads.job {
                    Some(job) => {
                        let (received, total, stage) = job.progress.lock().map_or((0, None, ""), |p| (p.received, p.total, p.stage));
                        ui.label(tf!("{stage}: {}" [stage], job.item.name));
                        let fraction = total.map_or(0.0, |t| received as f32 / t.max(1) as f32);
                        let text = match total {
                            Some(total) => tf!("{} of {}", megabytes(received), megabytes(total)),
                            None => megabytes(received),
                        };
                        ui.add(egui::ProgressBar::new(fraction.clamp(0.0, 1.0)).text(text).desired_width(250.0));
                        cancel = ui.button(tr("Cancel")).clicked();
                    }
                    None => {
                        let mut ready = item.as_ref().is_some_and(|item| !item.url.is_empty());
                        let label = match (have, kind) {
                            (true, _) => tr("Download again"),
                            (false, Kind::Banks | Kind::Captures | Kind::Impulses) => tr("Download and add to the library"),
                            (false, Kind::Effects) => tr("Download and install"),
                        };
                        if kind == Kind::Effects && item.is_some() {
                            ui.checkbox(&mut self.downloads.trusted, tr("I trust the author of this plug-in"));
                            ready &= self.downloads.trusted;
                        }
                        if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                            if let Some(item) = item {
                                self.start_download(item);
                            }
                        }
                    }
                }
                if cancel {
                    if let Some(job) = &self.downloads.job {
                        job.cancel.store(true, Ordering::Relaxed);
                    }
                }
                if !self.downloads.message.is_empty() {
                    ui.add_space(4.0);
                    ui.label(&self.downloads.message);
                }
            });
        });
    }
}
