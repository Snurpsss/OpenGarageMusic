//! The page of a pedal module in the MIDI Pilot: the memories of the pedal
//! and the patch edited, its program change map, its system settings, and
//! the file the patches are saved in.

use eframe::egui;
use og_engine::devices::{self, Encoding, Param, Value};

use crate::app::App;
use crate::i18n::tr;
use crate::pedal::{Confirm, Job, Tab};

/// What the page asks of the program, once it is drawn.
enum Asked {
    Close,
    Job(Job),
    Select(usize),
    Value(Param, Value),
    Replace(Vec<u8>),
    Save,
    Load,
    ExportPatch,
    ExportFile,
    Confirm(Confirm),
    Confirmed,
    Unconfirmed,
}

impl App {
    /// The page of the module open.
    pub fn set_pedal_page(&mut self, ui: &mut egui::Ui) {
        let Some(session) = &mut self.setlist.pedal else {
            self.setlist.page = crate::setlist::Page::Devices;
            return;
        };
        let device_name = self.settings.set_list.devices.get(session.device).map(|d| d.name.clone()).unwrap_or_default();
        let ports = (self.settings.set_list.midi_output.clone(), self.settings.set_list.midi_input.clone());
        let mut asked = Vec::new();
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new(tr("< Devices")).size(16.0)).clicked() {
                asked.push(Asked::Close);
            }
            ui.heading(format!("{} · {device_name}", session.description.name));
            if session.busy() {
                ui.spinner();
            }
            if let Some((done, total)) = session.progress {
                ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).desired_width(160.0).text(format!("{done} / {total}")));
            }
        });
        ui.horizontal(|ui| {
            let (output, input) = &ports;
            ui.weak(tf!("Output {output} · input {input}" [output, input]));
            if ui.small_button(tr("Identify")).on_hover_text(tr("Asks the pedal who it is")).clicked() {
                asked.push(Asked::Job(Job::Identify));
            }
        });
        if !session.message.is_empty() {
            ui.label(&session.message);
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut session.tab, Tab::Patches, tr("Patches"));
            ui.selectable_value(&mut session.tab, Tab::PcMap, tr("Program change map"));
            ui.selectable_value(&mut session.tab, Tab::System, tr("System"));
            ui.selectable_value(&mut session.tab, Tab::File, tr("File"));
        });
        ui.separator();
        // Something to confirm comes before anything else.
        if let Some(confirm) = session.confirm.clone() {
            let what = match confirm {
                Confirm::Store(n) => tf!("Store the patch edited in the memory {} of the pedal? What it holds is kept in the folder of the list first.", session.description.label(n)),
                Confirm::SendAll => tf!("Write the {} patches of the file in the memories of the pedal, each in the place its name says?", session.file.patches.len()),
                Confirm::WritePcMap => tr("Write the program change map in the pedal?").to_owned(),
                Confirm::WriteSystem => tr("Write the system settings in the pedal?").to_owned(),
            };
            ui.horizontal(|ui| {
                ui.label(what);
                if ui.button(tr("Yes")).clicked() {
                    asked.push(Asked::Confirmed);
                }
                if ui.button(tr("No")).clicked() {
                    asked.push(Asked::Unconfirmed);
                }
            });
            ui.separator();
        }
        let tab = session.tab;
        let description = session.description.clone();
        match tab {
            Tab::Patches => {
                ui.horizontal(|ui| {
                    if ui.add_enabled(!session.busy(), egui::Button::new(tr("Read the names"))).on_hover_text(tr("The name of each memory of the pedal")).clicked() {
                        asked.push(Asked::Job(Job::ReadNames));
                    }
                    if ui.add_enabled(!session.busy(), egui::Button::new(tr("Read every memory"))).on_hover_text(tr("Every patch of the pedal, into the file")).clicked() {
                        asked.push(Asked::Job(Job::ReadAll));
                    }
                });
                ui.add_space(4.0);
                let height = ui.available_height();
                ui.horizontal_top(|ui| {
                    // The memories.
                    ui.allocate_ui_with_layout(egui::vec2(200.0, height), egui::Layout::top_down(egui::Align::Min), |ui| {
                        egui::ScrollArea::vertical().id_salt("pedal memories").auto_shrink([false, false]).show(ui, |ui| {
                            for n in 0..description.patches() {
                                let name = session.names.get(n).cloned().unwrap_or_default();
                                let text = if name.is_empty() { description.label(n) } else { format!("{}  {name}", description.label(n)) };
                                if ui.selectable_label(session.selected == Some(n), text).clicked() && session.selected != Some(n) {
                                    asked.push(Asked::Select(n));
                                }
                            }
                        });
                    });
                    ui.separator();
                    // The patch edited.
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), egui::Layout::top_down(egui::Align::Min), |ui| match (&session.selected, &session.raw) {
                        (Some(n), Some(raw)) => {
                            let raw = raw.clone();
                            ui.horizontal(|ui| {
                                ui.strong(description.label(*n));
                                ui.label(description.patch_name(&raw));
                                if session.edited {
                                    ui.weak(tr("(edited: the pedal plays it, the memory is unchanged)"));
                                }
                                if ui.add_enabled(!session.busy(), egui::Button::new(tr("Store…"))).on_hover_text(tr("Writes the patch edited in this memory of the pedal")).clicked() {
                                    asked.push(Asked::Confirm(Confirm::Store(*n)));
                                }
                                if ui.button(tr("Init")).on_hover_text(tr("Every parameter at its first value")).clicked() {
                                    asked.push(Asked::Replace(description.init_patch()));
                                }
                                if ui.add_enabled(!session.busy(), egui::Button::new(tr("Read again"))).clicked() {
                                    asked.push(Asked::Job(Job::ReadPatch(*n)));
                                }
                                if ui.button(tr("Export for an AI…")).on_hover_text(tr("This patch as a script an AI can read and change")).clicked() {
                                    asked.push(Asked::ExportPatch);
                                }
                                if let Some(kept) = session.file.patch(&description, *n).filter(|kept| kept != &raw) {
                                    if ui.button(tr("Take the file's")).on_hover_text(tr("The patch the file holds for this memory")).clicked() {
                                        asked.push(Asked::Replace(kept));
                                    }
                                }
                            });
                            egui::ScrollArea::vertical().id_salt("pedal parameters").auto_shrink([false, false]).show(ui, |ui| {
                                if let Some((param, value)) = parameters(ui, &description, &description.patch, &raw, "patch") {
                                    asked.push(Asked::Value(param, value));
                                }
                            });
                        }
                        (Some(_), None) => {
                            ui.weak(tr("Reading the patch…"));
                        }
                        _ => {
                            ui.weak(tr("Choose a memory on the left: the pedal plays it, and its patch is read."));
                        }
                    });
                });
            }
            Tab::PcMap => {
                ui.horizontal(|ui| {
                    if ui.add_enabled(!session.busy(), egui::Button::new(tr("Read"))).clicked() {
                        asked.push(Asked::Job(Job::ReadPcMap));
                    }
                    if ui.add_enabled(!session.busy() && !session.pc_map.is_empty(), egui::Button::new(tr("Write…"))).clicked() {
                        asked.push(Asked::Confirm(Confirm::WritePcMap));
                    }
                    ui.weak(tr("Which memory each program change calls, by bank select (0 to 2) and program (0 to 127)"));
                });
                if session.pc_map.is_empty() {
                    ui.weak(tr("Not read yet."));
                } else {
                    egui::ScrollArea::vertical().id_salt("pedal pc map").auto_shrink([false, false]).show(ui, |ui| {
                        egui::Grid::new("pc map").num_columns(4).spacing([16.0, 4.0]).striped(true).show(ui, |ui| {
                            for i in 0..session.pc_map.len() {
                                let (bank, program) = (i / 128, i % 128);
                                ui.label(tf!("Bank {bank}, program {program}" [bank, program]));
                                let last = description.patches().saturating_sub(1) as u16;
                                let d = &description;
                                ui.add(egui::DragValue::new(&mut session.pc_map[i]).range(0..=last).custom_formatter(|v, _| d.label(v as usize)).custom_parser(|text| d.number(text).map(|n| n as f64)));
                                let name = session.names.get(usize::from(session.pc_map[i])).cloned().unwrap_or_default();
                                ui.label(name);
                                ui.end_row();
                            }
                        });
                    });
                }
            }
            Tab::System => {
                ui.horizontal(|ui| {
                    if ui.add_enabled(!session.busy(), egui::Button::new(tr("Read"))).clicked() {
                        asked.push(Asked::Job(Job::ReadSystem));
                    }
                    if ui.add_enabled(!session.busy() && !session.system.is_empty(), egui::Button::new(tr("Write…"))).clicked() {
                        asked.push(Asked::Confirm(Confirm::WriteSystem));
                    }
                });
                if session.system.is_empty() {
                    ui.weak(tr("Not read yet."));
                } else {
                    let raw = session.system.clone();
                    egui::ScrollArea::vertical().id_salt("pedal system").auto_shrink([false, false]).show(ui, |ui| {
                        if let Some((param, value)) = parameters(ui, &description, &description.system, &raw, "system") {
                            let _ = devices::set(&mut session.system, &param, &value);
                        }
                    });
                }
            }
            Tab::File => {
                ui.horizontal(|ui| {
                    if ui.button(tr("Save as…")).on_hover_text(tr("The patches known, the program change map and the system, as a JSON file")).clicked() {
                        asked.push(Asked::Save);
                    }
                    if ui.button(tr("Load…")).on_hover_text(tr("A file saved here, or a backup of the maker's editor (.bel)")).clicked() {
                        asked.push(Asked::Load);
                    }
                    if ui.add_enabled(!session.busy() && !session.file.patches.is_empty(), egui::Button::new(tr("Send the file to the pedal…"))).clicked() {
                        asked.push(Asked::Confirm(Confirm::SendAll));
                    }
                    if ui.add_enabled(!session.file.patches.is_empty(), egui::Button::new(tr("Export for an AI…"))).on_hover_text(tr("Every patch of the file as a script an AI can read and change")).clicked() {
                        asked.push(Asked::ExportFile);
                    }
                });
                match &session.file_path {
                    Some(path) => ui.weak(path.display().to_string()),
                    None => ui.weak(tr("No file yet: the patches read go in it.")),
                };
                if !session.backups.is_empty() {
                    ui.weak(tf!("{} memories kept before they were written, in the folder of the list.", session.backups.len()));
                }
                egui::ScrollArea::vertical().id_salt("pedal file").auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("pedal file patches").num_columns(3).spacing([16.0, 4.0]).striped(true).show(ui, |ui| {
                        for (label, entry) in &session.file.patches {
                            ui.label(label);
                            ui.label(&entry.name);
                            if let Some(n) = description.number(label) {
                                if ui.small_button(tr("Edit")).on_hover_text(tr("Into the memory chosen, to be heard")).clicked() {
                                    if let Some(raw) = devices::from_hex(&entry.raw) {
                                        if session.selected.is_none() {
                                            asked.push(Asked::Select(n));
                                        }
                                        asked.push(Asked::Replace(raw));
                                    }
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
            }
        }
        for ask in asked {
            match ask {
                Asked::Close => self.close_pedal(),
                Asked::Job(job) => {
                    if let Some(session) = &mut self.setlist.pedal {
                        session.ask(job);
                    }
                }
                Asked::Select(n) => self.select_pedal_patch(n),
                Asked::Value(param, value) => self.set_pedal_value(&param, &value),
                Asked::Replace(raw) => self.replace_pedal_patch(raw),
                Asked::Save => self.save_pedal_file(),
                Asked::Load => self.load_pedal_file(),
                Asked::ExportPatch => self.export_pedal_patch_for_ai(),
                Asked::ExportFile => self.export_pedal_file_for_ai(),
                Asked::Confirm(confirm) => {
                    if let Some(session) = &mut self.setlist.pedal {
                        session.confirm = Some(confirm);
                    }
                }
                Asked::Confirmed => self.confirm_pedal(),
                Asked::Unconfirmed => {
                    if let Some(session) = &mut self.setlist.pedal {
                        session.confirm = None;
                    }
                }
            }
        }
    }

    /// Does what was confirmed.
    pub fn confirm_pedal(&mut self) {
        let Some(session) = &mut self.setlist.pedal else { return };
        match session.confirm.take() {
            Some(Confirm::Store(n)) => {
                if let Some(raw) = session.raw.clone() {
                    session.ask(Job::Store { number: n, raw });
                }
            }
            Some(Confirm::SendAll) => session.ask(Job::SendAll(session.file.clone())),
            Some(Confirm::WritePcMap) => session.ask(Job::WritePcMap(session.pc_map.clone())),
            Some(Confirm::WriteSystem) => session.ask(Job::WriteSystem(session.system.clone())),
            None => {}
        }
    }
}

/// The parameters of an area, by group, each with its widget; says which one
/// was changed, and to what.
fn parameters(ui: &mut egui::Ui, description: &devices::Description, params: &[Param], raw: &[u8], salt: &str) -> Option<(Param, Value)> {
    let mut changed = None;
    let mut groups: Vec<&str> = Vec::new();
    for p in params {
        if !groups.contains(&p.group.as_str()) {
            groups.push(&p.group);
        }
    }
    for group in groups {
        let mut rows = |ui: &mut egui::Ui| {
            egui::Grid::new((salt, group)).num_columns(2).spacing([12.0, 4.0]).show(ui, |ui| {
                for param in params.iter().filter(|p| p.group == group) {
                    ui.label(&param.name).on_hover_text(param.shown.clone().unwrap_or_default());
                    if let Some(value) = widget(ui, description, param, raw) {
                        changed = Some((param.clone(), value));
                    }
                    ui.end_row();
                }
            });
        };
        if group.is_empty() {
            rows(ui);
        } else {
            egui::CollapsingHeader::new(group).id_salt((salt, "group", group)).default_open(group == params[0].group).show(ui, rows);
        }
    }
    changed
}

/// One parameter's widget: a list of its labels, a text, or a slider shown
/// as the pedal shows it. Says the new value when it is changed.
fn widget(ui: &mut egui::Ui, description: &devices::Description, param: &Param, raw: &[u8]) -> Option<Value> {
    let value = devices::get(raw, param);
    match (param.enc, &value) {
        (Encoding::Ascii, Value::Text(text)) => {
            let mut text = text.trim_end().to_owned();
            let response = ui.add(egui::TextEdit::singleline(&mut text).char_limit(param.bytes).desired_width(180.0));
            (response.lost_focus() && text.trim_end() != devices::get(raw, param).to_text().trim_end()).then_some(Value::Text(text))
        }
        (_, Value::Number(n)) => {
            let mut n = *n;
            let changed = match description.labels_of(param) {
                Some(labels) if labels.len() <= 32 => {
                    let shown = description.shown(param, &value);
                    let mut chosen = None;
                    egui::ComboBox::from_id_salt(("pedal param", &param.name, param.ofs)).selected_text(shown).width(180.0).show_ui(ui, |ui| {
                        for (i, label) in labels.iter().enumerate() {
                            let v = param.min + i as i64;
                            if ui.selectable_label(v == n, label).clicked() && v != n {
                                chosen = Some(v);
                            }
                        }
                    });
                    if let Some(v) = chosen {
                        n = v;
                        true
                    } else {
                        false
                    }
                }
                _ => {
                    let d = description;
                    ui.add(egui::Slider::new(&mut n, param.min..=param.max).custom_formatter(|v, _| d.shown(param, &Value::Number(v as i64)))).changed()
                }
            };
            changed.then_some(Value::Number(n))
        }
        _ => None,
    }
}

trait AsText {
    fn to_text(&self) -> String;
}

impl AsText for Value {
    fn to_text(&self) -> String {
        match self {
            Value::Text(t) => t.clone(),
            Value::Number(n) => n.to_string(),
        }
    }
}
