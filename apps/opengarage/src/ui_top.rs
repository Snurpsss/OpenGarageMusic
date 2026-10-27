//! Menu bar, transport bar, status bar, dialogs and the notepad.

use crate::i18n::tr;
use crate::widgets::{channel_items, checkbox_not, device_items, device_name, radio_list, row_list};
use eframe::egui::{self, vec2, Align, Color32, Layout, RichText, Sense};
use og_engine::patches::{self, PatchDef};
use og_engine::{templates, TrackKind};

use crate::app::{palette, App, AutoParam, BottomTab, Dialog, RightPanel, KEY_LABELS, NOTE_LETTERS};

fn red() -> Color32 {
    crate::theme::current().record()
}

impl App {
    pub fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button(tr("File"), |ui| {
                if ui.button(tr("New…")).clicked() {
                    self.dialog = Some(Dialog::NewProject);
                    ui.close_menu();
                }
                if ui.button(tr("New from a tablature site…")).on_hover_text(tr("Searches Songsterr, Guitariff or Tablature Hub and makes a project of the tracks of a tablature; or reads a Guitar Pro file")).clicked() {
                    self.open_sites();
                    ui.close_menu();
                }
                if ui.button(tr("New from a Guitar Pro or MusicXML file…")).on_hover_text(tr("Makes a project of a Guitar Pro file (.gp3, .gp4, .gp5, .gpx or .gp) or a MusicXML score (.musicxml, .xml or .mxl) you downloaded or wrote: its tracks, tuning, tempo, sections and techniques")).clicked() {
                    ui.close_menu();
                    self.import_guitar_pro();
                }
                ui.separator();
                if ui.button(tr("Open…")).clicked() {
                    ui.close_menu();
                    self.open();
                }
                let recent = self.recent_projects();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button(tr("Open a recent project"), |ui| {
                        for path in &recent {
                            let name = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                            if ui.button(name).on_hover_text(path.display().to_string()).clicked() {
                                ui.close_menu();
                                self.open_recent(path);
                            }
                        }
                        ui.separator();
                        if ui.button(tr("Empty this list")).clicked() {
                            self.settings.recent_projects.clear();
                            self.save_settings();
                            ui.close_menu();
                        }
                    });
                });
                if ui.button(tr("Previous sessions…")).on_hover_text(tr("The projects you worked on, kept as you went without your having to save them: reopen one")).clicked() {
                    ui.close_menu();
                    self.open_sessions();
                }
                if ui.button(tr("MIDI Pilot…")).on_hover_text(tr("The songs of a concert or a rehearsal, in a window of their own: each sends the number of its preset to a pedal switcher or an effects unit by MIDI, and opens its project")).clicked() {
                    ui.close_menu();
                    self.open_set_list();
                }
                ui.separator();
                if ui.button(tr("Save        Ctrl+S")).clicked() {
                    ui.close_menu();
                    self.save(false);
                }
                if ui.button(tr("Save as…")).clicked() {
                    ui.close_menu();
                    self.save(true);
                }
                ui.separator();
                if ui.button(tr("Import into this project…")).on_hover_text(tr("Adds an audio file or a MIDI file to the project")).clicked() {
                    ui.close_menu();
                    self.dialog = Some(Dialog::Import);
                }
                if ui.button(tr("Export to…")).on_hover_text(tr("The song as WAV, AIFF, MP3 or AAC, or its notes as MIDI")).clicked() {
                    ui.close_menu();
                    self.dialog = Some(Dialog::Export);
                }
                ui.separator();
                ui.menu_button(tr("Options"), |ui| {
                    if ui.button(tr("Audio outputs and inputs…")).clicked() {
                        self.refresh_audio_devices();
                        self.dialog = Some(Dialog::Audio);
                        ui.close_menu();
                    }
                    ui.separator();
                    let mut keep = !self.settings.forget_session;
                    if ui
                        .checkbox(&mut keep, tr("Reopen the program as I left it"))
                        .on_hover_text(tr("At the next start, the last project comes back as you left it, saved or not, with the layout. Without this, the program starts on the demonstration project. Either way, the projects stay in File > Previous sessions."))
                        .changed()
                    {
                        self.set_keep_session(keep);
                    }
                    ui.separator();
                    let folder = if self.settings.music_folder.trim().is_empty() { tr("none").to_owned() } else { self.settings.music_folder.clone() };
                    let found = self.library.len();
                    if ui.button(tr("Music library folder…")).on_hover_text(tf!("Where your own recordings are: when a tablature is imported, the recording of the song is looked for there. Now: {folder} ({found} files)." [folder, found])).clicked() {
                        ui.close_menu();
                        self.choose_music_folder();
                    }
                    let projects = self.projects_dir().display().to_string();
                    if ui.button(tr("Projects folder…")).on_hover_text(tf!("Where the projects are saved unless you choose elsewhere. Now: {projects}" [projects])).clicked() {
                        ui.close_menu();
                        self.choose_projects_folder();
                    }
                    let mut isolated = self.plugins_isolated();
                    if ui
                        .checkbox(&mut isolated, tr("Run each plug-in in a process of its own"))
                        .on_hover_text(tr("Safer: a faulty plug-in stops alone instead of taking the application down. Slightly slower to start."))
                        .changed()
                    {
                        self.set_plugins_isolated(isolated);
                    }
                });
            });

            ui.menu_button(tr("Edit"), |ui| {
                if ui.add_enabled(self.can_undo(), egui::Button::new(tr("Undo        Ctrl+Z"))).clicked() {
                    self.undo();
                    ui.close_menu();
                }
                if ui.add_enabled(self.can_redo(), egui::Button::new(tr("Redo        Ctrl+Y"))).clicked() {
                    self.redo();
                    ui.close_menu();
                }
                ui.separator();
                let has_region = self.selection().is_some();
                if ui.add_enabled(has_region, egui::Button::new(tr("Cut        Ctrl+X"))).clicked() {
                    self.cut_region();
                    ui.close_menu();
                }
                if ui.add_enabled(has_region, egui::Button::new(tr("Copy        Ctrl+C"))).clicked() {
                    self.copy_region();
                    ui.close_menu();
                }
                if ui.add_enabled(self.clipboard.is_some(), egui::Button::new(tr("Paste at the playhead        Ctrl+V"))).clicked() {
                    self.paste_region();
                    ui.close_menu();
                }
                if ui.add_enabled(has_region, egui::Button::new(tr("Delete        Del"))).clicked() {
                    self.delete_selected_region();
                    ui.close_menu();
                }
                ui.separator();
                if ui.add_enabled(has_region, egui::Button::new(tr("Split at the playhead        Ctrl+T"))).clicked() {
                    self.split_region();
                    ui.close_menu();
                }
                if ui.add_enabled(has_region, egui::Button::new(tr("Join with the next region        Ctrl+J"))).clicked() {
                    self.join_regions();
                    ui.close_menu();
                }
                if ui.add_enabled(has_region, egui::Button::new(tr("Loop / stop looping        L"))).clicked() {
                    self.toggle_loop();
                    ui.close_menu();
                }
            });

            ui.menu_button(tr("Track"), |ui| {
                let ti = self.selected_track;
                let has_track = ti < self.project.tracks.len();
                if ui.button(tr("New track…")).clicked() {
                    self.dialog = Some(Dialog::NewTrack);
                    ui.close_menu();
                }
                if ui.add_enabled(has_track, egui::Button::new(tr("Duplicate track"))).clicked() {
                    self.duplicate_track(ti);
                    ui.close_menu();
                }
                if ui.add_enabled(has_track, egui::Button::new(tr("Delete track"))).clicked() {
                    self.delete_track(ti);
                    ui.close_menu();
                }
                ui.separator();
                if ui.add_enabled(has_track && ti > 0, egui::Button::new(tr("Move track up"))).clicked() {
                    self.move_track(ti, -1);
                    ui.close_menu();
                }
                if ui.add_enabled(has_track && ti + 1 < self.project.tracks.len(), egui::Button::new(tr("Move track down"))).clicked() {
                    self.move_track(ti, 1);
                    ui.close_menu();
                }
                if let Some(track) = self.project.tracks.get_mut(ti) {
                    ui.separator();
                    ui.checkbox(&mut track.locked, tr("Lock track"));
                }
            });

            ui.menu_button(tr("Project"), |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr("Tone"));
                    egui::ComboBox::from_id_salt("key").width(60.0).selected_text(tr(KEY_LABELS[usize::from(self.project.key % 12)])).show_ui(ui, |ui| {
                        for (i, label) in KEY_LABELS.iter().enumerate() {
                            ui.selectable_value(&mut self.project.key, i as u8, *label);
                        }
                    });
                    ui.selectable_value(&mut self.project.minor, false, tr("major"));
                    ui.selectable_value(&mut self.project.minor, true, tr("minor"));
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Time signature"));
                    ui.add(egui::DragValue::new(&mut self.project.beats_per_bar).range(1..=16).speed(0.05));
                    ui.label("/");
                    for unit in [4u32, 8] {
                        ui.selectable_value(&mut self.project.beat_unit, unit, unit.to_string());
                    }
                });
                ui.separator();
                let bar = self.project.bar_len();
                let mut auto = self.project.end.is_none();
                if ui.checkbox(&mut auto, tr("End of the project: after the last region")).changed() {
                    self.project.end = if auto { None } else { Some(self.project.end_beat()) };
                }
                if let Some(end) = &mut self.project.end {
                    ui.horizontal(|ui| {
                        ui.label(tr("End at bar"));
                        let mut bars = *end / bar;
                        ui.add(egui::DragValue::new(&mut bars).range(1.0..=2000.0).speed(0.1).fixed_decimals(0));
                        *end = bars.round().max(1.0) * bar;
                    });
                }
                ui.separator();
                ui.checkbox(&mut self.project.count_in, tr("One bar of count-in before recording"));
                ui.checkbox(&mut self.project.metronome, tr("Metronome        K"));
            });

            ui.menu_button(tr("View"), |ui| {
                ui.checkbox(&mut self.show_library, tr("Library        Y"));
                let mut controls = self.show_bottom && self.bottom_tab == BottomTab::Controls;
                if ui.checkbox(&mut controls, tr("Sound controls        B")).changed() {
                    self.show_bottom = controls;
                    self.bottom_tab = BottomTab::Controls;
                }
                let mut editor = self.show_bottom && self.bottom_tab == BottomTab::Editor;
                if ui.checkbox(&mut editor, tr("Editor        E")).changed() {
                    self.show_bottom = editor;
                    self.bottom_tab = BottomTab::Editor;
                }
                // The words of the song, under the panel: the lines being sung, or the whole text.
                ui.menu_button(tr("Words of the song"), |ui| {
                    if checkbox_not(ui, &mut self.settings.hide_lyrics, tr("Shown under the panel")).changed() {
                        self.save_settings();
                    }
                    ui.separator();
                    let ways = [(false, tr("The lines being sung").to_owned()), (true, tr("The whole text, scrolling").to_owned())];
                    if let Some(whole) = radio_list(ui, &self.settings.lyrics_whole, ways) {
                        self.settings.lyrics_whole = whole;
                        self.save_settings();
                    }
                });
                let mut loops = self.right_panel == RightPanel::Loops;
                if ui.checkbox(&mut loops, tr("Loops        O")).changed() {
                    self.right_panel = if loops { RightPanel::Loops } else { RightPanel::None };
                }
                let mut notes = self.right_panel == RightPanel::Notes;
                if ui.checkbox(&mut notes, tr("Notepad")).changed() {
                    self.right_panel = if notes { RightPanel::Notes } else { RightPanel::None };
                }
                ui.separator();
                ui.checkbox(&mut self.show_global, tr("Arrangement, tempo and transposition tracks"));
                ui.checkbox(&mut self.show_master, tr("Master track"));
                let mut automation = self.automation.is_some();
                if ui.checkbox(&mut automation, tr("Automation        A")).changed() {
                    self.automation = automation.then_some(AutoParam::Volume);
                }
                if checkbox_not(ui, &mut self.settings.tracks_playhead_moves, tr("Playhead kept in the middle of the tracks"))
                    .on_hover_text(tr("While the song plays, the tracks slide under a playhead that stays put. Otherwise the playhead moves and the view turns the page."))
                    .changed()
                {
                    self.save_settings();
                }
                if checkbox_not(ui, &mut self.settings.editor_playhead_moves, tr("Playhead kept in place in the editor"))
                    .on_hover_text(tr("While the song plays, the notes scroll under a playhead that stays put. Otherwise the playhead moves and the editor turns the page."))
                    .changed()
                {
                    self.save_settings();
                }
                let current = self.auto_switch_bars();
                ui.menu_button(tr("Auto-switch after"), |ui| {
                    ui.weak(tr("With several tracks in the editor and auto-switch on: bars ahead in which the track shown must still have a note to come, or the next one that plays takes its place"));
                    if let Some(bars) = radio_list(ui, &current, [1u32, 2, 4, 8].map(|bars| (bars, tf!("{bars} bars" [bars])))) {
                        self.settings.auto_switch_bars = bars;
                        self.save_settings();
                        ui.close_menu();
                    }
                });
                ui.separator();
                if ui.checkbox(&mut self.typing, tr("Play on the computer keyboard        Ctrl+K")).changed() {
                    self.release_typing();
                }
                ui.separator();
                ui.menu_button(tr("Theme"), |ui| {
                    let current = self.theme_id().to_owned();
                    let chosen = radio_list(ui, &current, self.themes.iter().map(|theme| (theme.id.clone(), tr(&theme.name).to_owned())));
                    ui.separator();
                    if ui.button(tr("Import a theme (.json file)…")).clicked() {
                        ui.close_menu();
                        self.import_theme();
                    }
                    if ui.button(tr("Save this theme as a model…")).on_hover_text(tr("A file that gives every colour: change them, then import it")).clicked() {
                        ui.close_menu();
                        self.save_theme_model();
                    }
                    if ui.button(tr("Open the themes folder")).clicked() {
                        let folder = self.themes_dir();
                        let _ = std::fs::create_dir_all(&folder);
                        self.open_external(&folder.to_string_lossy());
                        ui.close_menu();
                    }
                    if let Some(id) = chosen {
                        self.set_theme(&id);
                        ui.close_menu();
                    }
                });
                // Each language is listed under its own name, whatever the language in use.
                ui.menu_button(tr("Language"), |ui| {
                    let current = self.language();
                    if let Some(code) = radio_list(ui, &current, crate::i18n::languages()) {
                        self.set_language(&code);
                        ui.close_menu();
                    }
                });
            });

            ui.menu_button(tr("Library"), |ui| {
                if ui.button(tr("Download sounds…")).on_hover_text(tr("Freely licensed sound banks, from sites that distribute them")).clicked() {
                    self.open_downloads(crate::downloads::Kind::Banks);
                    ui.close_menu();
                }
                if ui.button(tr("Download effects (plug-ins)…")).on_hover_text(tr("Free CLAP plug-ins: effects and instruments")).clicked() {
                    self.open_downloads(crate::downloads::Kind::Effects);
                    ui.close_menu();
                }
                if ui.button(tr("Captures and impulse responses from the sites…")).on_hover_text(tr("Captures of amplifiers and pedals, and impulse responses of cabinets and rooms, from the sites described to the program (TONE3000 with your account, GuitarML)")).clicked() {
                    self.open_site_named("TONE3000");
                    ui.close_menu();
                }
                ui.separator();
                if ui.button(tr("Import a sound bank (.sf2, .sf3, .sfz)…")).clicked() {
                    ui.close_menu();
                    self.import_soundfont();
                }
                if ui.button(tr("Import a CLAP plug-in (.clap file)…")).clicked() {
                    ui.close_menu();
                    self.import_plugin_file();
                }
                if ui.button(tr("Import a VST3 plug-in (.vst3 file)…")).clicked() {
                    ui.close_menu();
                    self.import_vst3_file();
                }
                if ui.button(tr("Import an LV2 plug-in (.lv2 folder)…")).clicked() {
                    ui.close_menu();
                    self.import_lv2_folder();
                }
                if ui.button(tr("Installed effects…")).on_hover_text(tr("Add an installed plug-in to the selected track")).clicked() {
                    self.pick_plugin();
                    ui.close_menu();
                }
                ui.separator();
                for (label, dir) in [(tr("Open the sound banks folder"), self.banks_dir()), (tr("Open the effects folder"), self.effects_dir())] {
                    if ui.button(label).clicked() {
                        let _ = std::fs::create_dir_all(&dir);
                        self.open_external(&dir.to_string_lossy());
                        ui.close_menu();
                    }
                }
            });

            ui.menu_button(tr("Audio"), |ui| {
                ui.weak(tr("Output of all the tracks"));
                let output = radio_list(ui, &self.settings.output_device, device_items(&self.audio_outputs, tr("System default")));
                ui.separator();
                ui.weak(tr("Input"));
                let input = radio_list(ui, &self.settings.input_device, device_items(&self.audio_inputs, tr("System default")));
                ui.separator();
                ui.weak(tr("Channel of the input"));
                let channel = radio_list(ui, &self.settings.input_channel, channel_items(self.audio_input_channels, tr("All of them, mixed")));
                ui.separator();
                ui.weak(tr("A track can also have an output or an input of its own: right-click its header."));
                if ui.button(tr("List the devices again")).on_hover_text(tr("After plugging in or unplugging a sound card")).clicked() {
                    self.refresh_audio_devices();
                }
                if let Some(name) = output {
                    self.set_output_device(&name);
                    ui.close_menu();
                }
                if let Some(name) = input {
                    self.set_input_device(&name);
                    self.refresh_audio_devices();
                    ui.close_menu();
                }
                if let Some(number) = channel {
                    self.set_input_channel(number);
                    ui.close_menu();
                }
            });

            ui.menu_button(tr("Help"), |ui| {
                ui.label(tr("Space: play / stop        Enter: back to the start"));
                ui.label(tr("R: record        C: loop        K: metronome"));
                ui.label(tr("Double-click in a track: new region"));
                ui.label(tr("Right edge of a region, bottom: length; top: looping"));
                ui.label(tr("Click in the upper half of the ruler, then drag: loop area"));
                ui.label(tr("Ctrl + wheel: zoom        Shift + wheel: scroll"));
                ui.label(tr("Editor: click = note, drag in empty space = selection, right-click = delete"));
            });
        });
    }

    pub fn transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.toggle_value(&mut self.show_library, tr("Library"));
            let mut controls = self.show_bottom && self.bottom_tab == BottomTab::Controls;
            if ui.toggle_value(&mut controls, tr("Settings")).changed() {
                self.show_bottom = controls;
                self.bottom_tab = BottomTab::Controls;
            }
            let mut editor = self.show_bottom && self.bottom_tab == BottomTab::Editor;
            if ui.toggle_value(&mut editor, tr("Editor")).changed() {
                self.show_bottom = editor;
                self.bottom_tab = BottomTab::Editor;
            }
            ui.separator();

            if ui.button(tr("Start")).on_hover_text(tr("Back to the start (Enter)")).clicked() {
                self.go_to(0.0);
            }
            let label = if self.playing { tr("Stop") } else { tr("Play") };
            if ui.button(label).on_hover_text(tr("Play / stop (Space)")).clicked() {
                self.toggle_play();
            }
            let record = egui::Button::new(RichText::new(tr("Rec")).color(if self.recording { Color32::WHITE } else { red() }))
                .fill(if self.recording { red() } else { ui.visuals().widgets.inactive.weak_bg_fill });
            if ui.add(record).on_hover_text(tr("Record on the selected track (R)")).clicked() {
                self.toggle_record();
            }
            ui.toggle_value(&mut self.project.cycle_enabled, tr("Loop")).on_hover_text(tr("Repeat the loop area (C)"));
            // As written, or with the recording of the song.
            if self.has_recording() {
                let mut follow = self.project.follow;
                let hint = tr("On: the song plays with its recording (the video or the audio file), at its pace; the other tracks are silent unless their M is turned off. Off: the song plays as written, at its own tempo.");
                if ui.toggle_value(&mut follow, tr("Original sound")).on_hover_text(hint).changed() {
                    self.set_follow(follow);
                }
            }

            ui.separator();
            let bar_len = self.project.bar_len();
            let step = 4.0 / f64::from(self.project.beat_unit.max(1));
            let bar = (self.position / bar_len).floor() as i64 + 1;
            let beat = ((self.position % bar_len) / step).floor() as i64 + 1;
            ui.label(RichText::new(format!("{bar:>3}.{beat}")).monospace().size(18.0)).on_hover_text(tr("Bar.beat"));
            ui.label(RichText::new(self.position_time()).monospace().weak()).on_hover_text(tr("Minutes:seconds"));
            match self.followed_tempo() {
                // With the original sound, the tempo is the recording's.
                Some(tempo) => {
                    ui.label(RichText::new(format!("{tempo:.0} bpm")).weak()).on_hover_text(tr("The tempo of the recording here: the song follows it"));
                }
                None => {
                    ui.add(egui::DragValue::new(&mut self.project.tempo).range(40.0..=240.0).speed(0.5).suffix(" bpm")).on_hover_text(tr("Tempo"));
                }
            }
            ui.label(format!("{}/{}", self.project.beats_per_bar, self.project.beat_unit)).on_hover_text(tr("Time signature (Project menu)"));
            let key = format!("{} {}", tr(KEY_LABELS[usize::from(self.project.key % 12)]), if self.project.minor { "min" } else { "maj" });
            ui.label(key).on_hover_text(tr("Key (Project menu)"));

            ui.separator();
            ui.toggle_value(&mut self.project.metronome, tr("Metronome")).on_hover_text("K");
            ui.toggle_value(&mut self.project.count_in, tr("Count-in")).on_hover_text(tr("One bar of metronome before recording"));
            if ui.button(tr("Tuner")).clicked() {
                if let Err(e) = self.audio.ensure_input(&self.input_for(self.selected_track)) {
                    self.status = tf!("Tuner: {e}" [e]);
                }
                self.dialog = Some(Dialog::Tuner);
            }
            if ui.toggle_value(&mut self.typing, tr("Keyboard")).on_hover_text(tr("Play with the computer keyboard (Ctrl+K)")).changed() {
                self.release_typing();
            }
            if self.typing {
                ui.add(egui::DragValue::new(&mut self.octave).range(-3..=3).speed(0.05).prefix(tr("octave ")));
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let help = ui
                    .toggle_value(&mut self.help, " ? ")
                    .on_hover_text(tr("Quick help: the bar at the bottom tells what the pointer is over, and the tips of the buttons show at once"));
                self.help_rect = help.rect;
                let mut notes = self.right_panel == RightPanel::Notes;
                if ui.toggle_value(&mut notes, tr("Notes")).changed() {
                    self.right_panel = if notes { RightPanel::Notes } else { RightPanel::None };
                }
                let mut loops = self.right_panel == RightPanel::Loops;
                if ui.toggle_value(&mut loops, tr("Loops")).changed() {
                    self.right_panel = if loops { RightPanel::Loops } else { RightPanel::None };
                }
                ui.separator();
                let (meter, resp) = ui.allocate_exact_size(vec2(70.0, 10.0), Sense::hover());
                resp.on_hover_text(tr("Output level"));
                ui.painter().rect_filled(meter, 2.0, ui.visuals().extreme_bg_color);
                let mut level = meter;
                level.set_width(meter.width() * self.peak.min(1.0));
                ui.painter().rect_filled(level, 2.0, if self.peak > 0.9 { red() } else { palette(0) });
                ui.spacing_mut().slider_width = 90.0;
                ui.add(egui::Slider::new(&mut self.project.master_volume, 0.0..=1.2).show_value(false))
                    .on_hover_text(tr("Master volume"));
            });
        });
    }

    pub fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if self.help {
                let text = if self.help_text.is_empty() { tr("Quick help: move the pointer over a part of the window.") } else { &self.help_text };
                ui.label(RichText::new(format!("?  {text}")).strong());
            } else {
                ui.label(&self.status);
            }
            if self.banks_loading() {
                ui.spinner();
                ui.weak(tr("Loading sound banks…"));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                match &self.audio.error {
                    Some(error) => {
                        ui.colored_label(red(), tf!("No sound: {error}" [error]));
                    }
                    None => {
                        ui.weak(tf!("Audio output {} Hz", self.audio.sample_rate));
                    }
                }
                if let Some(name) = &self.audio.midi_name {
                    ui.weak(tf!("MIDI keyboard: {name}  ·" [name]));
                }
            });
        });
    }

    pub fn notes_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);
        // The song, as the list of previous sessions shows it.
        egui::Grid::new("song").num_columns(2).show(ui, |ui| {
            ui.label(tr("Artist"));
            ui.add(egui::TextEdit::singleline(&mut self.project.artist).desired_width(f32::INFINITY));
            ui.end_row();
            ui.label(tr("Title"));
            ui.add(egui::TextEdit::singleline(&mut self.project.title).desired_width(f32::INFINITY));
            ui.end_row();
        });
        ui.add_space(8.0);
        ui.heading(tr("Notepad"));
        ui.weak(tr("Free text saved with the project."));
        ui.add_space(4.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut self.project.notes).desired_rows(24).desired_width(f32::INFINITY));
        });
    }

    pub fn dialogs(&mut self, ctx: &egui::Context) {
        /// A large window, centred, that never outgrows the main one, and
        /// that the user resizes by its edges: `share` of the main window
        /// at first, within `room`.
        fn fitted<'a>(window: egui::Window<'a>, ctx: &egui::Context, share: f32, most: f32) -> egui::Window<'a> {
            let screen = ctx.screen_rect().size();
            let room = screen * most;
            window
                .collapsible(false)
                .resizable(true)
                .max_size(room)
                .min_size(vec2(480.0_f32.min(room.x), 240.0_f32.min(room.y)))
                .default_size(screen * share)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        }
        let Some(dialog) = self.dialog.take() else { return };
        let mut open = true;
        let mut keep = true;
        let centered = |title: &str| {
            egui::Window::new(title.to_owned()).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        };
        let dialog = match dialog {
            Dialog::Audio => {
                let (mut output, mut input, mut refresh) = (None, None, false);
                centered(tr("Audio outputs and inputs")).open(&mut open).show(ctx, |ui| {
                    ui.label(tr("Main output: where the mix is heard"));
                    egui::ComboBox::from_id_salt("audio_output").width(360.0).selected_text(device_name(&self.settings.output_device)).show_ui(ui, |ui| {
                        output = row_list(ui, &self.settings.output_device, device_items(&self.audio_outputs, tr("System default")));
                    });
                    ui.add_space(6.0);
                    ui.label(tr("Input: what is recorded and listened to"));
                    egui::ComboBox::from_id_salt("audio_input").width(360.0).selected_text(device_name(&self.settings.input_device)).show_ui(ui, |ui| {
                        input = row_list(ui, &self.settings.input_device, device_items(&self.audio_inputs, tr("System default")));
                    });
                    ui.add_space(6.0);
                    ui.weak(tr("A track can also have an output or an input of its own: right-click its header."));
                    if let Some(error) = &self.audio.error {
                        ui.colored_label(red(), tf!("No sound: {error}" [error]));
                    }
                    refresh = ui.button(tr("List the devices again")).on_hover_text(tr("After plugging in or unplugging a sound card")).clicked();
                });
                if let Some(name) = output {
                    self.set_output_device(&name);
                }
                if let Some(name) = input {
                    self.set_input_device(&name);
                }
                if refresh {
                    self.refresh_audio_devices();
                }
                Dialog::Audio
            }
            Dialog::Quit => {
                let (mut save, mut quit) = (false, false);
                centered(tr("Quit")).open(&mut open).show(ctx, |ui| {
                    ui.label(tr("This project has changes that are not saved."));
                    ui.colored_label(red(), tr("If you quit without saving, they are lost for good."));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        save = ui.button(tr("Save and quit")).clicked();
                        quit = ui.button(tr("Quit without saving")).clicked();
                        if ui.button(tr("Cancel")).clicked() {
                            keep = false;
                        }
                    });
                });
                if save {
                    // Asks where when the project has no file yet; giving up there cancels.
                    self.save(false);
                    quit = !self.is_modified();
                    keep = false;
                }
                if quit {
                    self.quitting = true;
                    keep = false;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                Dialog::Quit
            }
            Dialog::NewProject => {
                centered(tr("New project")).open(&mut open).show(ctx, |ui| {
                    let recent = self.recent_projects();
                    if !recent.is_empty() {
                        ui.label(tr("Recent projects:"));
                        ui.horizontal_wrapped(|ui| {
                            for path in recent.iter().take(6) {
                                let name = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                                if ui.button(name).on_hover_text(path.display().to_string()).clicked() {
                                    self.open_recent(path);
                                    keep = false;
                                }
                            }
                        });
                        ui.separator();
                    }
                    ui.label(tr("Choose a starting point:"));
                    ui.add_space(4.0);
                    for template in templates::all() {
                        ui.horizontal(|ui| {
                            if ui.add_sized([150.0, 26.0], egui::Button::new(tr(template.name))).clicked() {
                                self.replace_project(crate::app::localized((template.build)()), None);
                                self.blank = true;
                                self.status = tf!("New project: {}.", tr(template.name));
                                keep = false;
                            }
                            ui.weak(tr(template.description));
                        });
                    }
                });
                Dialog::NewProject
            }
            Dialog::NewTrack => {
                centered(tr("New track")).open(&mut open).show(ctx, |ui| {
                    let kinds = [
                        (TrackKind::Instrument, tr("Sounds played on the keyboard, written note by note or coming from loops.")),
                        (TrackKind::Audio, tr("Recording from a microphone or a line input, or an imported audio file.")),
                        (TrackKind::Guitar, tr("A plugged-in guitar or bass, with an amp and effect pedals.")),
                        (TrackKind::Drummer, tr("A virtual drummer that plays by itself, following your settings.")),
                    ];
                    for (kind, description) in kinds {
                        ui.horizontal(|ui| {
                            if ui.add_sized([150.0, 26.0], egui::Button::new(tr(kind.label()))).clicked() {
                                self.add_track(kind);
                                keep = false;
                            }
                            ui.weak(description);
                        });
                    }
                });
                Dialog::NewTrack
            }
            Dialog::Tuner => {
                centered(tr("Tuner")).open(&mut open).show(ctx, |ui| {
                    ui.set_min_width(300.0);
                    if !self.audio.has_input() {
                        ui.colored_label(red(), tr("No audio input available."));
                        return;
                    }
                    let (name, cents) = match self.tuner {
                        Some((pitch, cents)) => {
                            (format!("{}{}", NOTE_LETTERS[usize::from(pitch % 12)], i32::from(pitch) / 12 - 2), cents)
                        }
                        None => ("—".to_owned(), 0.0),
                    };
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new(name).size(46.0).strong());
                        let text = match self.tuner {
                            None => tr("Play a single note").to_owned(),
                            Some(_) if cents.abs() < 5.0 => tr("In tune").to_owned(),
                            Some(_) if cents < 0.0 => tf!("{:.0} cents flat", -cents),
                            Some(_) => tf!("{cents:.0} cents sharp" [cents]),
                        };
                        ui.label(text);
                    });
                    let (bar, _) = ui.allocate_exact_size(vec2(300.0, 22.0), Sense::hover());
                    let painter = ui.painter();
                    painter.rect_filled(bar, 3.0, ui.visuals().extreme_bg_color);
                    painter.vline(bar.center().x, bar.y_range(), crate::app::stroke(1.0, ui.visuals().text_color()));
                    if self.tuner.is_some() {
                        let x = bar.center().x + (cents.clamp(-50.0, 50.0) / 50.0) * bar.width() / 2.0;
                        let color = if cents.abs() < 5.0 { palette(0) } else { palette(2) };
                        painter.circle_filled(egui::pos2(x, bar.center().y), 7.0, color);
                    }
                    ui.horizontal(|ui| {
                        ui.weak(tr("Input level"));
                        let (meter, _) = ui.allocate_exact_size(vec2(180.0, 8.0), Sense::hover());
                        ui.painter().rect_filled(meter, 2.0, ui.visuals().extreme_bg_color);
                        let mut level = meter;
                        level.set_width(meter.width() * self.input_level.min(1.0));
                        ui.painter().rect_filled(level, 2.0, palette(0));
                    });
                });
                Dialog::Tuner
            }
            Dialog::PluginPicker => {
                let files = self.known_plugin_files();
                let (mut chosen, mut import, mut download, mut import_lv2, mut import_vst3) = (None, false, false, false, false);
                centered(tr("Installed effects and instruments")).open(&mut open).show(ctx, |ui| {
                    ui.set_min_width(420.0);
                    if files.is_empty() {
                        ui.label(tr("No plug-in is installed yet."));
                    } else {
                        ui.weak(tr("Choose a plug-in to add it to the selected track."));
                        egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                            for file in &files {
                                let mut name = file.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
                                name.push_str("   ·   ");
                                name.push_str(crate::plugin::plugin_format(file));
                                if ui.add_sized([400.0, 22.0], egui::Button::new(name)).on_hover_text(file.display().to_string()).clicked() {
                                    chosen = Some(file.clone());
                                }
                            }
                        });
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        import = ui.button(tr("Import a .clap file…")).clicked();
                        import_lv2 = ui.button(tr("Import a .lv2 folder…")).clicked();
                        import_vst3 = ui.button(tr("Import a .vst3 file…")).clicked();
                        download = ui.button(tr("Download effects…")).clicked();
                    });
                    ui.weak(tr("A plug-in is a program: only add those whose origin you know."));
                });
                if let Some(file) = chosen {
                    self.dialog = Some(Dialog::PluginPicker);
                    self.open_plugin_file(&file);
                    // Added: the window closes. Otherwise it shows what happened.
                    keep = self.dialog.is_some();
                }
                if import {
                    self.import_plugin_file();
                }
                if import_lv2 {
                    self.import_lv2_folder();
                }
                if import_vst3 {
                    self.import_vst3_file();
                }
                if download {
                    self.open_downloads(crate::downloads::Kind::Effects);
                }
                Dialog::PluginPicker
            }
            Dialog::Sites => {
                fitted(egui::Window::new(tr("New from a tablature site")), ctx, 0.5, 0.5).open(&mut open).show(ctx, |ui| self.sites_window(ui));
                Dialog::Sites
            }
            Dialog::Sessions => {
                let mut done = false;
                fitted(egui::Window::new(tr("Previous sessions")), ctx, 0.7, 0.85).open(&mut open).show(ctx, |ui| done = self.sessions_window(ui));
                keep = !done;
                Dialog::Sessions
            }
            Dialog::Export => {
                let mut done = false;
                centered(tr("Export to")).open(&mut open).show(ctx, |ui| done = self.export_window(ui));
                keep = !done;
                Dialog::Export
            }
            Dialog::Import => {
                let mut done = false;
                centered(tr("Import into this project")).open(&mut open).show(ctx, |ui| done = self.import_window(ui));
                keep = !done;
                Dialog::Import
            }
            Dialog::Words => {
                let mut done = false;
                fitted(egui::Window::new(tr("The words of the song")), ctx, 0.6, 0.8).open(&mut open).show(ctx, |ui| done = self.words_window(ui));
                keep = !done;
                Dialog::Words
            }
            Dialog::Downloads => {
                fitted(egui::Window::new(tr("Download sounds and effects")), ctx, 0.65, 0.9).open(&mut open).show(ctx, |ui| self.downloads_window(ui));
                Dialog::Downloads
            }
            Dialog::Plugins { file, options } => {
                centered(tr("Choose a plug-in")).open(&mut open).show(ctx, |ui| {
                    ui.label(tr("This file holds several plug-ins:"));
                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for info in &options {
                            ui.horizontal(|ui| {
                                if ui.add_sized([220.0, 24.0], egui::Button::new(&info.name)).clicked() {
                                    self.add_plugin(&file, info);
                                    keep = false;
                                }
                                ui.weak(if info.instrument { tr("instrument") } else { tr("effect") });
                            });
                        }
                    });
                });
                Dialog::Plugins { file, options }
            }
            Dialog::SavePatch(mut name) => {
                centered(tr("Save this sound")).open(&mut open).show(ctx, |ui| {
                    ui.label(tr("Name of the sound:"));
                    ui.text_edit_singleline(&mut name);
                    let valid = !name.trim().is_empty();
                    if ui.add_enabled(valid, egui::Button::new(tr("Save to “My sounds”"))).clicked() {
                        if let Some(track) = self.project.tracks.get(self.selected_track) {
                            let patch = PatchDef {
                                name: name.trim().to_owned(),
                                category: patches::CAT_USER.to_owned(),
                                sound: track.sound,
                                fx: track.fx.clone(),
                            };
                            self.settings.user_patches.retain(|p| p.name != patch.name);
                            self.settings.user_patches.push(patch);
                            self.save_settings();
                            self.rebuild_patches();
                            self.lib_category = patches::CAT_USER.to_owned();
                            self.status = tr("Sound saved to “My sounds”.").to_owned();
                        }
                        keep = false;
                    }
                });
                Dialog::SavePatch(name)
            }
        };
        if open && keep && self.dialog.is_none() {
            self.dialog = Some(dialog);
        }
    }
}
