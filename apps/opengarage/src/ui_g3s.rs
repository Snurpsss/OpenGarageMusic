//! The page of the G3S module in the MIDI Pilot: the banks of the backup,
//! the presets of the bank chosen, and the preset edited (name, loops,
//! order, MIDI lines).

use eframe::egui;
use og_engine::g3s::{MidiLine, LOOPS, MIDI_KINDS, MIDI_LINES, NAME, PRESETS};
use og_engine::setlist::{Pedal, PEDAL_KINDS};

use crate::app::App;
use crate::i18n::tr;

/// The footswitches that show presets: 14, in two rows of seven.
const SHOWN: usize = 14;

/// What the page asks of the program.
enum Asked {
    Close,
    Load,
    Save,
    Check,
    New,
    BanksFromSongs,
    SongsFromBanks,
    Export,
}

/// The pedals of each loop of the switcher, the order of the signal and the
/// bank switches, edited in the chain's settings; says whether something changed.
fn board_panel(ui: &mut egui::Ui, list: &mut og_engine::setlist::SetList, device: usize) -> bool {
    let names: Vec<String> = list.devices.iter().filter(|d| d.model != "g3s").map(|d| d.name.clone()).collect();
    let Some(switcher) = list.devices.get_mut(device) else { return false };
    let mut changed = false;
    ui.strong(tr("Pedalboard"));
    ui.label(tr("The pedals in each loop of the switcher, in the order the signal goes through them. A MIDI pedal names the device of the chain that drives it."));
    ui.add_space(4.0);
    if switcher.loops.len() < LOOPS {
        switcher.loops.resize(LOOPS, Vec::new());
    }
    egui::ScrollArea::vertical().id_salt("g3s board").auto_shrink([false, false]).show(ui, |ui| {
        let mut remove = None;
        let mut add = None;
        egui::Grid::new("g3s loops").num_columns(2).spacing([10.0, 6.0]).striped(true).show(ui, |ui| {
            for n in 0..LOOPS {
                ui.strong(tf!("Loop {}", n + 1));
                ui.vertical(|ui| {
                    for (i, pedal) in switcher.loops[n].iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.add(egui::TextEdit::singleline(&mut pedal.name).hint_text(tr("Pedal")).desired_width(180.0)).changed() {
                                changed = true;
                            }
                            egui::ComboBox::from_id_salt(("pedal kind", n, i)).selected_text(if pedal.kind.is_empty() { tr("kind") } else { pedal.kind.as_str() }).width(110.0).show_ui(ui, |ui| {
                                for kind in PEDAL_KINDS {
                                    if ui.selectable_value(&mut pedal.kind, kind.to_owned(), kind).changed() {
                                        changed = true;
                                    }
                                }
                            });
                            egui::ComboBox::from_id_salt(("pedal device", n, i)).selected_text(if pedal.device.is_empty() { tr("not MIDI") } else { pedal.device.as_str() }).width(120.0).show_ui(ui, |ui| {
                                if ui.selectable_value(&mut pedal.device, String::new(), tr("not MIDI")).changed() {
                                    changed = true;
                                }
                                for name in &names {
                                    if ui.selectable_value(&mut pedal.device, name.clone(), name).changed() {
                                        changed = true;
                                    }
                                }
                            });
                            if ui.small_button("×").on_hover_text(tr("Remove")).clicked() {
                                remove = Some((n, i));
                            }
                        });
                    }
                    if ui.small_button(tr("Add a pedal")).clicked() {
                        add = Some(n);
                    }
                });
                ui.end_row();
            }
        });
        if let Some((n, i)) = remove {
            switcher.loops[n].remove(i);
            changed = true;
        }
        if let Some(n) = add {
            switcher.loops[n].push(Pedal::default());
            changed = true;
        }
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(tr("Order of the signal")).on_hover_text(tr("The twelve loops in the order the signal goes through them, as you wired them; what a new preset starts from"));
            let mut text = switcher.signal_order.iter().map(u8::to_string).collect::<Vec<_>>().join(" ");
            if ui.add(egui::TextEdit::singleline(&mut text).hint_text("1 8 10 6 2 4 3 5 7 9 11 12").desired_width(220.0)).changed() {
                let order: Vec<u8> = text.split(|c: char| !c.is_ascii_digit()).filter_map(|p| p.parse().ok()).collect();
                let mut sorted = order.clone();
                sorted.sort_unstable();
                if order.is_empty() || (sorted.len() == LOOPS && sorted.iter().enumerate().all(|(i, n)| usize::from(*n) == i + 1)) {
                    switcher.signal_order = order;
                    changed = true;
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Bank switches")).on_hover_text(tr("The footswitches that change banks, where no preset goes; a setting of the switcher itself"));
            let mut text = switcher.bank_switches.iter().map(u8::to_string).collect::<Vec<_>>().join(" ");
            if ui.add(egui::TextEdit::singleline(&mut text).hint_text("1 8").desired_width(80.0)).changed() {
                switcher.bank_switches = text.split(|c: char| !c.is_ascii_digit()).filter_map(|p| p.parse::<u8>().ok()).filter(|s| (1..=14).contains(s)).collect();
                changed = true;
            }
        });
    });
    changed
}

/// What a MIDI kind is called.
fn kind_name(kind: u8) -> &'static str {
    MIDI_KINDS.iter().find(|(k, _)| *k == kind).map_or("?", |(_, name)| name)
}

impl App {
    /// The page of the module open.
    pub fn set_g3s_page(&mut self, ui: &mut egui::Ui) {
        let Some(g) = &mut self.setlist.g3s else {
            self.setlist.page = crate::setlist::Page::Devices;
            return;
        };
        let device_name = self.settings.set_list.devices.get(g.device).map(|d| d.name.clone()).unwrap_or_default();
        let loop_names: Vec<String> = (0..LOOPS).map(|n| self.settings.set_list.devices.get(g.device).map_or_else(|| (n + 1).to_string(), |d| d.loop_name(n))).collect();
        let mut asked = None;
        let mut board_changed = false;
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new(tr("< Devices")).size(16.0)).clicked() {
                asked = Some(Asked::Close);
            }
            ui.heading(format!("The GigRig G3S · {device_name}"));
            if g.edited {
                ui.weak(tr("(edited)"));
            }
        });
        ui.horizontal(|ui| {
            if ui.button(tr("Load a backup…")).on_hover_text(tr("A backup made by the switcher's editor (.g3b)")).clicked() {
                asked = Some(Asked::Load);
            }
            if ui.button(tr("Save as…")).on_hover_text(tr("A backup for the switcher's editor to restore")).clicked() {
                asked = Some(Asked::Save);
            }
            if ui.button(tr("Check")).on_hover_text(tr("As the switcher checks a backup restored")).clicked() {
                asked = Some(Asked::Check);
            }
            if ui.button(tr("New")).on_hover_text(tr("One bank, cleared; the global settings of a real backup are better kept: load one first")).clicked() {
                asked = Some(Asked::New);
            }
            ui.separator();
            if ui.button(tr("Banks from the songs")).on_hover_text(tr("A bank a song of the list, after the first bank, named after it")).clicked() {
                asked = Some(Asked::BanksFromSongs);
            }
            if ui.button(tr("Songs from the banks")).on_hover_text(tr("A song of the list for each bank named")).clicked() {
                asked = Some(Asked::SongsFromBanks);
            }
            if ui.button(tr("Export for an AI…")).on_hover_text(tr("The backup as a script an AI can read and change")).clicked() {
                asked = Some(Asked::Export);
            }
            ui.separator();
            ui.toggle_value(&mut g.board, tr("Pedalboard")).on_hover_text(tr("Which pedals sit in which loops, the order of the signal, the bank switches: what an AI needs to know of your board"));
        });
        if let Some(path) = &g.path {
            ui.weak(path.display().to_string());
        }
        if !g.message.is_empty() {
            ui.label(&g.message);
        }
        ui.separator();
        let height = ui.available_height();
        ui.horizontal_top(|ui| {
            // The banks.
            ui.allocate_ui_with_layout(egui::vec2(230.0, height), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.horizontal(|ui| {
                    ui.strong(tr("Banks"));
                    let mut song_mode = g.backup.song_mode();
                    if ui.checkbox(&mut song_mode, tr("Song mode")).on_hover_text(tr("The switcher shows the names of the banks, not their numbers")).changed() {
                        g.backup.set_song_mode(song_mode);
                        g.edited = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(tr("Add")).clicked() {
                        if let Some(bank) = g.backup.add_bank() {
                            g.bank = bank;
                            g.edited = true;
                        }
                    }
                    if ui.add_enabled(g.backup.banks.len() > 1, egui::Button::new(tr("Remove"))).clicked() {
                        g.backup.remove_bank(g.bank);
                        g.bank = g.bank.min(g.backup.banks.len() - 1);
                        g.edited = true;
                    }
                    if ui.add_enabled(g.bank > 0, egui::Button::new(tr("Up"))).clicked() {
                        g.backup.move_bank(g.bank, true);
                        g.bank -= 1;
                        g.edited = true;
                    }
                    if ui.add_enabled(g.bank + 1 < g.backup.banks.len(), egui::Button::new(tr("Down"))).clicked() {
                        g.backup.move_bank(g.bank, false);
                        g.bank += 1;
                        g.edited = true;
                    }
                });
                let mut name = g.backup.bank_name(g.bank);
                if ui.add(egui::TextEdit::singleline(&mut name).hint_text(tr("Name of the bank, two lines of ten with | between")).desired_width(220.0)).changed() {
                    g.backup.set_bank_name(g.bank, &name);
                    g.edited = true;
                }
                egui::ScrollArea::vertical().id_salt("g3s banks").auto_shrink([false, false]).show(ui, |ui| {
                    for bank in 0..g.backup.banks.len() {
                        let name = g.backup.bank_name(bank).replace('|', " ");
                        if ui.selectable_label(g.bank == bank, format!("{:02}  {name}", bank + 1)).clicked() {
                            g.bank = bank;
                            g.preset = 0;
                        }
                    }
                });
            });
            ui.separator();
            // The pedalboard: the pedals of each loop, the signal's order, the bank switches.
            if g.board {
                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), egui::Layout::top_down(egui::Align::Min), |ui| {
                    if board_panel(ui, &mut self.settings.set_list, g.device) {
                        board_changed = true;
                    }
                });
                return;
            }
            // The presets of the bank, and the one edited.
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), height), egui::Layout::top_down(egui::Align::Min), |ui| {
                let Some(bank) = g.backup.banks.get_mut(g.bank) else { return };
                ui.horizontal(|ui| {
                    ui.strong(tr("Presets"));
                    ui.checkbox(&mut g.show_hidden, tr("Show the 14 hidden presets")).on_hover_text(tr("The presets 15 to 28 of a bank, reached by the editor only"));
                });
                let shown = if g.show_hidden { PRESETS } else { SHOWN };
                for row in 0..shown.div_ceil(7) {
                    ui.horizontal(|ui| {
                        for i in row * 7..(row * 7 + 7).min(shown) {
                            let name = bank.presets[i].name();
                            let text = format!("{}\n{name}", i + 1);
                            if ui.add_sized([96.0, 40.0], egui::SelectableLabel::new(g.preset == i, text)).clicked() {
                                g.preset = i;
                            }
                        }
                    });
                }
                ui.separator();
                let preset = &mut bank.presets[g.preset];
                egui::ScrollArea::vertical().id_salt("g3s preset").auto_shrink([false, false]).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(tr("Name"));
                        let mut name = preset.name();
                        if ui.add(egui::TextEdit::singleline(&mut name).char_limit(NAME).desired_width(120.0)).changed() {
                            preset.set_name(&name);
                            g.edited = true;
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(tr("Loops on"));
                        for (n, name) in loop_names.iter().enumerate() {
                            let mut on = preset.loop_on(n);
                            if ui.checkbox(&mut on, name).changed() {
                                preset.set_loop(n, on);
                                g.edited = true;
                            }
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(tr("Order of the loops in the signal")).on_hover_text(tr("Every loop, on or off, in the order the signal goes through them"));
                        let order = preset.order();
                        let mut moved = None;
                        for (place, n) in order.iter().enumerate() {
                            let on = preset.loop_on(usize::from(*n) - 1);
                            let text = egui::RichText::new(&loop_names[usize::from(*n) - 1]).strong();
                            let text = if on { text.color(egui::Color32::from_rgb(60, 220, 110)) } else { text.weak() };
                            ui.label(text);
                            if ui.small_button("◀").clicked() {
                                moved = Some((place, true));
                            }
                            if ui.small_button("▶").clicked() {
                                moved = Some((place, false));
                            }
                        }
                        if let Some((place, up)) = moved {
                            preset.move_loop(place, up);
                            g.edited = true;
                        }
                    });
                    ui.add_space(4.0);
                    ui.label(tr("MIDI messages of the preset")).on_hover_text(tr("Up to twenty: a program change or a control change, when the switch is pressed, released or held"));
                    let mut cleared = None;
                    egui::Grid::new("g3s midi").num_columns(6).spacing([10.0, 4.0]).show(ui, |ui| {
                        for (line, mut midi) in preset.midi_lines() {
                            ui.label(format!("{}", line + 1));
                            let mut changed = false;
                            egui::ComboBox::from_id_salt(("g3s kind", line)).selected_text(kind_name(midi.kind)).width(120.0).show_ui(ui, |ui| {
                                for (kind, name) in MIDI_KINDS {
                                    if ui.selectable_label(midi.kind == kind, name).clicked() && midi.kind != kind {
                                        midi.kind = kind;
                                        changed = true;
                                    }
                                }
                            });
                            let mut channel = midi.channel + 1;
                            if ui.add(egui::DragValue::new(&mut channel).range(1..=16).prefix(format!("{} ", tr("ch")))).changed() {
                                midi.channel = channel - 1;
                                changed = true;
                            }
                            let is_cc = midi.kind != 0x40 && (midi.kind & 0x10 == 0 && midi.kind != 0xC0 && midi.kind != 0x80 && midi.kind != 0x60 && midi.kind != 0xA0);
                            let label = if is_cc { tr("CC") } else { tr("PC") };
                            if ui.add(egui::DragValue::new(&mut midi.number).range(0..=127).prefix(format!("{label} "))).changed() {
                                changed = true;
                            }
                            if is_cc {
                                if ui.add(egui::DragValue::new(&mut midi.value).range(0..=127).prefix(format!("{} ", tr("value")))).changed() {
                                    changed = true;
                                }
                            } else {
                                ui.label("");
                            }
                            if ui.small_button("×").on_hover_text(tr("Remove")).clicked() {
                                cleared = Some(line);
                            }
                            if changed {
                                preset.set_midi(line, midi);
                                g.edited = true;
                            }
                            ui.end_row();
                        }
                    });
                    if let Some(line) = cleared {
                        preset.clear_midi(line);
                        g.edited = true;
                    }
                    if let Some(free) = (0..MIDI_LINES).find(|line| preset.midi(*line).is_none()) {
                        if ui.button(tr("Add a MIDI message")).clicked() {
                            preset.set_midi(free, MidiLine { kind: 0xC0, channel: 0, number: 0, value: 0 });
                            g.edited = true;
                        }
                    }
                });
            });
        });
        if board_changed {
            self.save_settings();
        }
        match asked {
            Some(Asked::Close) => self.close_g3s(),
            Some(Asked::Load) => self.load_g3s_file(),
            Some(Asked::Save) => self.save_g3s_file(),
            Some(Asked::Check) => self.check_g3s(),
            Some(Asked::New) => {
                if let Some(g) = &mut self.setlist.g3s {
                    g.backup = og_engine::g3s::blank(1);
                    g.path = None;
                    g.bank = 0;
                    g.preset = 0;
                    g.edited = true;
                    g.message = tr("One bank, cleared.").to_owned();
                }
            }
            Some(Asked::BanksFromSongs) => self.g3s_banks_from_songs(),
            Some(Asked::SongsFromBanks) => self.g3s_songs_from_banks(),
            Some(Asked::Export) => self.export_g3s_for_ai(),
            None => {}
        }
    }
}
