//! The window of the set list: a window of its own, beside the program's (on a
//! second screen on stage), with a large button for each song.

use eframe::egui;
use og_engine::setlist::{self, PilotMode, SetMessage, SetOrder, SetSong, Step, StepMessage, MODELS};

use crate::app::App;
use crate::files;
use crate::i18n::tr;
use crate::widgets::{checkbox_not, device_items, radio_list};
use crate::setlist::{artist_colour, hex, message_name, mode_name, model_name, parse_hex, step_kind_name, Cover, Page};

const BUTTON: egui::Vec2 = egui::vec2(240.0, 100.0);
/// The outline of the song chosen.
const CHOSEN: egui::Color32 = egui::Color32::from_rgb(60, 220, 110);

/// What a button shows.
struct Button {
    text: String,
    chosen: bool,
    colour: egui::Color32,
    picture: Option<egui::load::SizedTexture>,
    /// The song has a project.
    project: bool,
}

/// What was done on a song's button.
enum Asked {
    Choose(usize),
    Artist(String),
    Image(usize),
    NoImage(usize),
    ProjectFile(usize),
    OpenProject(usize),
    FindProject(usize),
    ForgetProject(usize),
    Edit(usize),
    Remove(usize),
}

fn order_name(order: SetOrder) -> &'static str {
    match order {
        SetOrder::List => tr("Order of the list"),
        SetOrder::Alphabetical => tr("Alphabetical"),
        SetOrder::ByArtist => tr("By artist"),
    }
}

impl App {
    /// Shows the window of the set list when it is open.
    pub fn set_list_window(&mut self, ctx: &egui::Context) {
        if !self.setlist.open {
            return;
        }
        if self.setlist.covers.cache.as_os_str().is_empty() {
            self.setlist.covers.cache = self.data_dir.join("Covers");
        }
        self.setlist.covers.receive(ctx);
        if self.setlist.api.is_none() && self.settings.set_list.api_port != 0 && !cfg!(test) {
            self.start_api(ctx);
        }
        let builder = egui::ViewportBuilder::default()
            .with_title(tr("MIDI Pilot"))
            .with_inner_size([1000.0, 650.0])
            .with_min_inner_size([420.0, 260.0])
            .with_icon(egui::IconData { rgba: crate::icon::rgba(64), width: 64, height: 64 });
        ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("set list"), builder, |ctx, class| {
            if class == egui::ViewportClass::Embedded {
                // Where a window of its own cannot be had (the tests): a window inside.
                let mut open = true;
                egui::Window::new(tr("MIDI Pilot")).open(&mut open).default_size([900.0, 560.0]).show(ctx, |ui| {
                    self.set_list_menu(ui);
                    ui.separator();
                    let height = (ui.available_height() - 30.0).max(100.0);
                    ui.allocate_ui(egui::vec2(ui.available_width(), height), |ui| self.set_list_central(ui));
                    ui.separator();
                    self.set_list_status(ui);
                });
                if !open {
                    self.close_set_list();
                }
            } else {
                egui::TopBottomPanel::top("set list menu").show(ctx, |ui| self.set_list_menu(ui));
                egui::TopBottomPanel::bottom("set list status").show(ctx, |ui| self.set_list_status(ui));
                egui::CentralPanel::default().show(ctx, |ui| self.set_list_central(ui));
                if ctx.input(|i| i.viewport().close_requested()) {
                    self.close_set_list();
                }
            }
            self.set_song_editor(ctx);
            self.set_api_window(ctx);
        });
    }

    /// One frame of the MIDI Pilot alone: the pilot fills the window.
    pub fn pilot_frame(&mut self, ctx: &egui::Context) {
        if self.setlist.covers.cache.as_os_str().is_empty() {
            self.setlist.covers.cache = self.data_dir.join("Covers");
        }
        self.setlist.covers.receive(ctx);
        if self.setlist.api.is_none() && self.settings.set_list.api_port != 0 && !cfg!(test) {
            self.start_api(ctx);
        }
        self.poll_set_list();
        egui::TopBottomPanel::top("set list menu").show(ctx, |ui| self.set_list_menu(ui));
        egui::TopBottomPanel::bottom("set list status").show(ctx, |ui| self.set_list_status(ui));
        egui::CentralPanel::default().show(ctx, |ui| self.set_list_central(ui));
        self.set_song_editor(ctx);
        self.set_api_window(ctx);
        // The covers and the projects looked for arrive on other threads.
        ctx.request_repaint_after(std::time::Duration::from_millis(200));
    }

    fn set_list_menu(&mut self, ui: &mut egui::Ui) {
        let mut output = None;
        egui::menu::bar(ui, |ui| {
            ui.menu_button(tr("List"), |ui| {
                if ui.button(tr("Import a CSV file…")).on_hover_text(tr("A list of songs, one a line: number, artist, song, and the text of its button (optional), separated by commas or semicolons. A further column named after a device of the chain holds the program it receives for the song.")).clicked() {
                    ui.close_menu();
                    self.import_set_list();
                }
                if ui.add_enabled(!self.settings.set_list.songs.is_empty(), egui::Button::new(tr("Export as a CSV file…"))).clicked() {
                    ui.close_menu();
                    self.export_set_list();
                }
                ui.separator();
                if ui.button(tr("Apply an AI script…")).on_hover_text(tr("A JSON file an AI wrote from the guide: patches of the pedals, their program change map, banks and presets of the G3S. Reviewed before anything is written.")).clicked() {
                    ui.close_menu();
                    let ctx = ui.ctx().clone();
                    self.load_ai_script(&ctx);
                }
                if ui.add_enabled(!self.settings.set_list.songs.is_empty(), egui::Button::new(tr("Export the songs for an AI…"))).on_hover_text(tr("The songs and their messages as a script an AI can read and change")).clicked() {
                    ui.close_menu();
                    self.export_songs_for_ai();
                }
                if ui.button(tr("Export the pedalboard for an AI…")).on_hover_text(tr("The chain, the loops of the switcher, the songs, the G3S backup loaded and every memory known of the pedals, as one file an AI reads before writing a script")).clicked() {
                    ui.close_menu();
                    self.export_pedalboard_for_ai();
                }
                if ui.button(tr("Guide for an AI…")).on_hover_text(tr("Writes the guide that teaches an AI to write scripts for your pedals: give it to the AI with your request")).clicked() {
                    ui.close_menu();
                    self.save_ai_guide();
                }
                ui.separator();
                if ui.button(tr("Add a song…")).clicked() {
                    ui.close_menu();
                    let number = self.settings.set_list.songs.iter().map(|s| s.number).max().map_or(0, |n| n.saturating_add(1).min(128));
                    self.setlist.editing = Some((None, SetSong { number, ..SetSong::default() }));
                }
                if ui
                    .add_enabled(!self.settings.set_list.songs.is_empty(), egui::Button::new(tr("Find the projects of the whole list")))
                    .on_hover_text(tr("For each song without a project: one kept in the previous sessions, by its artist and title; otherwise its tablature, chosen by you on the site of Settings > Site searched. The projects are saved in the folder of the list, to be opened later without the Internet."))
                    .clicked()
                {
                    ui.close_menu();
                    self.prepare_set_list();
                }
            });
            ui.menu_button(tr("Settings"), |ui| {
                let list = &self.settings.set_list;
                let current = if list.midi_output.is_empty() { tr("none").to_owned() } else { list.midi_output.clone() };
                ui.menu_button(tf!("MIDI output: {current}" [current]), |ui| {
                    if let Some(chosen) = radio_list(ui, &self.settings.set_list.midi_output, device_items(&self.setlist.outputs, tr("None"))) {
                        output = Some(chosen);
                        ui.close_menu();
                    }
                    if self.setlist.outputs.is_empty() {
                        ui.weak(tr("No MIDI output found."));
                    }
                    ui.separator();
                    if ui.button(tr("Look again")).clicked() {
                        self.setlist.outputs = crate::midi::midi_outputs();
                    }
                });
                let current = if self.settings.set_list.midi_input.is_empty() { tr("none").to_owned() } else { self.settings.set_list.midi_input.clone() };
                ui.menu_button(tf!("MIDI input: {current}" [current]), |ui| {
                    if let Some(chosen) = radio_list(ui, &self.settings.set_list.midi_input, device_items(&self.setlist.inputs, tr("None"))) {
                        self.choose_set_list_input(&chosen);
                        ui.close_menu();
                    }
                    ui.weak(tr("Where the pedal modules read the pedals' answers"));
                    ui.separator();
                    if ui.button(tr("Look again")).clicked() {
                        self.setlist.inputs = crate::midi::midi_inputs();
                    }
                });
                let mode = mode_name(self.settings.set_list.mode);
                ui.menu_button(tf!("Mode: {mode}" [mode]), |ui| {
                    for mode in PilotMode::ALL {
                        if ui.radio_value(&mut self.settings.set_list.mode, mode, mode_name(mode)).clicked() {
                            self.save_settings();
                            ui.close_menu();
                        }
                    }
                })
                .response
                .on_hover_text(tr("Switcher: each song sends one number, as a bank select or a program change, to a pedal switcher that sets the other pedals itself. Chain: each song sends its own messages to the devices of the chain, each on its channel."));
                if self.settings.set_list.mode == PilotMode::Switcher {
                    let channel = self.settings.set_list.channel + 1;
                    ui.menu_button(tf!("Channel: {channel}" [channel]), |ui| {
                        for channel in 0..16u8 {
                            if ui.radio_value(&mut self.settings.set_list.channel, channel, (channel + 1).to_string()).clicked() {
                                self.save_settings();
                                ui.close_menu();
                            }
                        }
                    });
                    let message = message_name(self.settings.set_list.message);
                    ui.menu_button(tf!("Message: {message}" [message]), |ui| {
                        for message in SetMessage::ALL {
                            if ui.radio_value(&mut self.settings.set_list.message, message, message_name(message)).clicked() {
                                self.save_settings();
                                ui.close_menu();
                            }
                        }
                    });
                    if ui.checkbox(&mut self.settings.set_list.numbers_from_one, tr("Numbers from 1")).on_hover_text(tr("The first song of the list is number 1, and sends 0")).changed() {
                        self.save_settings();
                    }
                }
                if ui.button(tr("Devices…")).on_hover_text(tr("The devices of the chain: a name, a model and a MIDI channel each")).clicked() {
                    ui.close_menu();
                    self.setlist.page = Page::Devices;
                }
                ui.separator();
                if checkbox_not(ui, &mut self.settings.set_list.no_covers, tr("Covers and photographs from Deezer")).on_hover_text(tr("The buttons show the cover of the song's album, and those of the artists their photograph, looked for on Deezer by artist and title and kept on the disk. A right-click on a song gives it a picture of your own.")).changed() {
                    self.save_settings();
                }
                if ui.button(tr("Empty the pictures kept")).on_hover_text(tr("The covers and photographs are looked for again")).clicked() {
                    ui.close_menu();
                    self.setlist.message = match self.setlist.covers.clear() {
                        Ok(()) => tr("The pictures kept are emptied: they are looked for again.").to_owned(),
                        Err(e) => tf!("The pictures were not emptied: {e}" [e]),
                    };
                }
                ui.separator();
                if ui.checkbox(&mut self.settings.set_list.open_project, tr("Open the song's project")).on_hover_text(tr("Choosing a song opens its project in the main window")).changed() {
                    self.save_settings();
                }
                let site = self.set_list_site().unwrap_or_default();
                ui.menu_button(tf!("Site searched: {site}" [site]), |ui| {
                    let names: Vec<String> = self.known_sites.iter().filter(|k| !k.site.open_only && k.site.is_tablature()).map(|k| k.site.name.clone()).collect();
                    for name in names {
                        if ui.radio(site == name, &name).clicked() {
                            self.settings.set_list.site = name;
                            self.save_settings();
                            ui.close_menu();
                        }
                    }
                });
                if ui.button(tr("API for an AI…")).on_hover_text(tr("A local API an AI's agent on this computer talks to, with a token; what it may write")).clicked() {
                    ui.close_menu();
                    self.setlist.api_window = true;
                }
                let folder = self.set_list_folder();
                if ui.button(tr("Folder of the projects…")).on_hover_text(tf!("Where the projects found for the list are saved. Now: {}", folder.display())).clicked() {
                    ui.close_menu();
                    if let Some(chosen) = files::choose_folder(None, Some(&folder)) {
                        self.settings.set_list.folder = chosen.to_string_lossy().into_owned();
                        self.save_settings();
                    }
                }
            });
            ui.menu_button(tr("Order"), |ui| {
                for order in SetOrder::ALL {
                    if ui.radio_value(&mut self.settings.set_list.order, order, order_name(order)).clicked() {
                        self.setlist.artist = None;
                        self.save_settings();
                        ui.close_menu();
                    }
                }
            });
        });
        if let Some(name) = output {
            self.connect_set_list_output(&name);
        }
    }

    fn set_list_status(&mut self, ui: &mut egui::Ui) {
        if let Some(question) = self.api_question() {
            ui.horizontal(|ui| {
                ui.colored_label(egui::Color32::from_rgb(240, 200, 80), question);
                if ui.button(tr("Yes")).clicked() {
                    self.api_decide(true);
                }
                if ui.button(tr("No")).clicked() {
                    self.api_decide(false);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.label(&self.setlist.message);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let list = &self.settings.set_list;
                let output = match (&self.setlist.output, &self.setlist.pedal) {
                    (Some(output), _) => output.name.clone(),
                    (None, Some(_)) => self.settings.set_list.midi_output.clone(),
                    (None, None) => tr("No MIDI output").to_owned(),
                };
                let settings = match list.mode {
                    PilotMode::Switcher => {
                        let (channel, message) = (list.channel + 1, message_name(list.message));
                        tf!("{output} · channel {channel} · {message}" [output, channel, message])
                    }
                    PilotMode::Chain => {
                        let count = list.devices.len();
                        tf!("{output} · chain · {count} devices" [output, count])
                    }
                };
                let response = ui.weak(settings);
                if !self.setlist.sent.is_empty() {
                    let bytes = self.setlist.sent.iter().map(|m| hex(m)).collect::<Vec<_>>().join(" | ");
                    response.on_hover_text(tf!("Sent: {bytes}" [bytes]));
                }
            });
        });
    }

    /// The small window of the API's settings: on or off, its port, its token.
    fn set_api_window(&mut self, ctx: &egui::Context) {
        if !self.setlist.api_window {
            return;
        }
        let (mut open, mut restart, mut changed) = (true, false, false);
        egui::Window::new(tr("API for an AI")).open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            ui.label(tr("An agent of an AI running on this computer (Claude Code, a script…) can read and write your pedals through a local API, as the guide for an AI describes. An AI in a web page cannot reach it: give it the guide and apply its script instead."));
            ui.add_space(6.0);
            let mut on = self.settings.set_list.api_port != 0;
            if ui.checkbox(&mut on, tr("Serve the API while the MIDI Pilot is open")).changed() {
                self.settings.set_list.api_port = if on { crate::api::DEFAULT_PORT } else { 0 };
                changed = true;
                restart = true;
            }
            if on {
                ui.horizontal(|ui| {
                    ui.label(tr("Port"));
                    let mut port = self.settings.set_list.api_port;
                    if ui.add(egui::DragValue::new(&mut port).range(1024..=65535)).changed() {
                        self.settings.set_list.api_port = port;
                        changed = true;
                    }
                    if ui.button(tr("Apply")).clicked() {
                        restart = true;
                    }
                    let listening = crate::api::where_it_listens(self.setlist.api.as_ref());
                    ui.weak(tf!("Now: {listening}" [listening]));
                });
                ui.horizontal(|ui| {
                    ui.label(tr("Token"));
                    if self.settings.set_list.api_token.is_empty() {
                        self.settings.set_list.api_token = crate::api::new_token();
                        changed = true;
                    }
                    let mut token = self.settings.set_list.api_token.clone();
                    ui.add(egui::TextEdit::singleline(&mut token).desired_width(300.0).interactive(false));
                    if ui.button(tr("New token")).on_hover_text(tr("The old one stops working")).clicked() {
                        self.settings.set_list.api_token = crate::api::new_token();
                        changed = true;
                        restart = true;
                    }
                });
                if ui.checkbox(&mut self.settings.set_list.api_unattended, tr("Write the pedals without asking me each time")).on_hover_text(tr("Otherwise each write waits for your Yes at the bottom of the window")).changed() {
                    changed = true;
                }
                let token = &self.settings.set_list.api_token;
                let port = self.settings.set_list.api_port;
                ui.add_space(6.0);
                ui.weak(tr("For the agent, for example:"));
                let mut example = format!("curl -H \"Authorization: Bearer {token}\" http://127.0.0.1:{port}/pedals/rv500/memories/50C");
                ui.add(egui::TextEdit::singleline(&mut example).desired_width(520.0).interactive(false));
            }
        });
        if changed {
            self.save_settings();
        }
        if restart {
            self.start_api(ctx);
        }
        if !open {
            self.setlist.api_window = false;
        }
    }

    /// The middle of the window: the songs, or the devices of the chain.
    fn set_list_central(&mut self, ui: &mut egui::Ui) {
        // Scroll bars that are seen: the lists of the pages are long, and the
        // thin bars egui draws by default show only under the pointer.
        ui.style_mut().spacing.scroll = egui::style::ScrollStyle::solid();
        match self.setlist.page {
            Page::Songs => self.set_list_buttons(ui),
            Page::Devices => self.set_devices_page(ui),
            Page::Pedal(_) => self.set_pedal_page(ui),
            Page::G3s => self.set_g3s_page(ui),
            Page::Script => self.set_script_page(ui),
        }
    }

    /// The devices of the chain: a name, a model and a channel each.
    fn set_devices_page(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new(tr("< Songs")).size(16.0)).clicked() {
                self.setlist.page = Page::Songs;
            }
            ui.heading(tr("Devices"));
        });
        ui.add_space(6.0);
        ui.label(tr("In the chain's mode, each song sends its messages to these devices, each on its channel. A device column of a CSV file imported, named like one of them, gives each song the program it receives."));
        ui.add_space(6.0);
        let (mut started, mut finished, mut remove, mut configure) = (None, None, None, None);
        if self.settings.set_list.devices.is_empty() {
            ui.weak(tr("No device yet."));
        }
        egui::Grid::new("set devices").num_columns(5).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.strong(tr("Device"));
            ui.strong(tr("Model"));
            ui.strong(tr("Channel"));
            ui.label("");
            ui.label("");
            ui.end_row();
            for (i, device) in self.settings.set_list.devices.iter_mut().enumerate() {
                let response = ui.add_sized([200.0, ui.spacing().interact_size.y], egui::TextEdit::singleline(&mut device.name));
                if response.gained_focus() {
                    started = Some((i, device.name.clone()));
                }
                if response.lost_focus() {
                    finished = Some(i);
                }
                egui::ComboBox::from_id_salt(("set device model", i)).selected_text(model_name(&device.model)).width(150.0).show_ui(ui, |ui| {
                    for model in MODELS {
                        ui.selectable_value(&mut device.model, model.to_owned(), model_name(model));
                    }
                });
                let mut channel = device.channel + 1;
                if ui.add(egui::DragValue::new(&mut channel).range(1..=16)).changed() {
                    device.channel = channel - 1;
                }
                if ui.button(tr("Remove")).clicked() {
                    remove = Some(i);
                }
                if device.model == "g3s" {
                    if ui.button(tr("Configure…")).on_hover_text(tr("Its banks, presets and MIDI messages, in a backup file its editor restores")).clicked() {
                        configure = Some(i);
                    }
                } else if crate::pedal::description_of(&device.model).is_some() && ui.button(tr("Configure…")).on_hover_text(tr("Its patches, program change map and system, over the MIDI output and input chosen")).clicked() {
                    configure = Some(i);
                }
                ui.end_row();
            }
        });
        ui.add_space(6.0);
        if ui.button(tr("Add a device")).clicked() {
            self.add_set_device();
        }
        if let Some(renaming) = started {
            self.setlist.renaming = Some(renaming);
        }
        if let Some(i) = finished {
            if let Some((place, old)) = self.setlist.renaming.take().filter(|(place, _)| *place == i) {
                self.rename_set_device(place, &old);
            } else {
                self.save_settings();
            }
        }
        if let Some(i) = remove {
            self.remove_set_device(i);
        }
        if let Some(i) = configure {
            if self.settings.set_list.devices.get(i).is_some_and(|d| d.model == "g3s") {
                self.open_g3s(i);
            } else {
                let ctx = ui.ctx().clone();
                self.open_pedal(i, &ctx);
            }
        }
    }

    fn song_button(&mut self, ctx: &egui::Context, index: usize, short: bool, artists: &[(String, usize)]) -> Button {
        let song = &self.settings.set_list.songs[index];
        let text = if short { song.short_label() } else { song.label() };
        let rank = artists.iter().position(|(artist, _)| setlist::same(artist, &song.artist)).unwrap_or(0);
        let project = !song.project.is_empty();
        let picture = self.song_cover(index).and_then(|cover| self.setlist.covers.texture(ctx, &cover));
        Button { text, chosen: self.setlist.active == Some(index), colour: artist_colour(rank), picture, project }
    }

    fn set_list_buttons(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let list = &self.settings.set_list;
        let artists = setlist::artists(&list.songs);
        let shown = setlist::order(&list.songs, list.order);
        let mut asked = None;
        if list.songs.is_empty() {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| ui.weak(tr("The list is empty: List > Import a CSV file… or Add a song…")));
            return;
        }
        match (list.order, self.setlist.artist.clone()) {
            (SetOrder::ByArtist, None) => {
                let active = self.setlist.active.and_then(|i| list.songs.get(i)).map(|s| s.artist.clone());
                let no_covers = list.no_covers;
                let buttons: Vec<Button> = artists
                    .iter()
                    .enumerate()
                    .map(|(rank, (artist, count))| {
                        let name = if artist.is_empty() { tr("(no artist)").to_owned() } else { artist.clone() };
                        let cover = (!no_covers && !artist.is_empty()).then(|| Cover::Artist(artist.clone()));
                        Button {
                            text: tf!("{name}\n{count} songs" [name, count]),
                            chosen: active.as_deref().is_some_and(|a| setlist::same(a, artist)),
                            colour: artist_colour(rank),
                            picture: cover.and_then(|c| self.setlist.covers.texture(&ctx, &c)),
                            project: false,
                        }
                    })
                    .collect();
                if let (Some(i), _) = grid(ui, &buttons, |_, _| None) {
                    asked = Some(Asked::Artist(artists[i].0.clone()));
                }
            }
            (SetOrder::ByArtist, Some(artist)) => {
                ui.horizontal(|ui| {
                    if ui.button(egui::RichText::new(tr("< Artists")).size(16.0)).clicked() {
                        self.setlist.artist = None;
                    }
                    ui.heading(&artist);
                });
                ui.add_space(6.0);
                let places: Vec<usize> = shown.into_iter().filter(|&i| setlist::same(&self.settings.set_list.songs[i].artist, &artist)).collect();
                let buttons: Vec<Button> = places.iter().map(|&i| self.song_button(&ctx, i, true, &artists)).collect();
                asked = grid_of_songs(ui, &buttons, &places, &self.settings.set_list.songs);
            }
            _ => {
                let buttons: Vec<Button> = shown.iter().map(|&i| self.song_button(&ctx, i, false, &artists)).collect();
                asked = grid_of_songs(ui, &buttons, &shown, &self.settings.set_list.songs);
            }
        }
        match asked {
            Some(Asked::Choose(i)) => self.choose_set_song(i),
            Some(Asked::Artist(artist)) => self.setlist.artist = Some(artist),
            Some(Asked::Image(i)) => self.choose_set_image(i),
            Some(Asked::NoImage(i)) => self.set_song_image(i, None),
            Some(Asked::ProjectFile(i)) => self.choose_set_project(i),
            Some(Asked::OpenProject(i)) => self.link_open_project(i),
            Some(Asked::FindProject(i)) => {
                self.setlist.queue.clear();
                self.find_set_project(i);
            }
            Some(Asked::ForgetProject(i)) => self.link_set_project_none(i),
            Some(Asked::Edit(i)) => self.setlist.editing = self.settings.set_list.songs.get(i).cloned().map(|song| (Some(i), song)),
            Some(Asked::Remove(i)) => self.remove_set_song(i),
            None => {}
        }
    }

    /// Takes a song's project away (the file stays).
    fn link_set_project_none(&mut self, index: usize) {
        if let Some(song) = self.settings.set_list.songs.get_mut(index) {
            song.project.clear();
            self.save_settings();
        }
    }

    /// The small window where a song is added or changed; in the chain's
    /// mode, with the messages it sends.
    fn set_song_editor(&mut self, ctx: &egui::Context) {
        let chain = self.settings.set_list.mode == PilotMode::Chain;
        let devices: Vec<String> = self.settings.set_list.devices.iter().map(|d| d.name.clone()).collect();
        let Some((place, song)) = &mut self.setlist.editing else { return };
        let title = if place.is_some() { tr("Change the song") } else { tr("Add a song") };
        let (mut keep, mut cancel, mut send) = (false, false, false);
        egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| {
            egui::Grid::new("set song").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label(tr("Number"));
                let hover = if chain { tr("Its number in the list; the switcher's mode sends it") } else { tr("The number of its preset, sent by MIDI when the song is chosen") };
                ui.add(egui::DragValue::new(&mut song.number).range(0..=128)).on_hover_text(hover);
                ui.end_row();
                ui.label(tr("Artist"));
                ui.text_edit_singleline(&mut song.artist);
                ui.end_row();
                ui.label(tr("Song"));
                ui.text_edit_singleline(&mut song.title);
                ui.end_row();
                ui.label(tr("Text of the button"));
                ui.add(egui::TextEdit::singleline(&mut song.display).hint_text(tr("optional")));
                ui.end_row();
            });
            if chain {
                ui.separator();
                ui.label(tr("Messages")).on_hover_text(tr("Sent in this order when the song is chosen, each to its device on the device's channel"));
                step_rows(ui, song, &devices);
                ui.horizontal(|ui| {
                    if ui.add_enabled(!devices.is_empty(), egui::Button::new(tr("Add a message"))).clicked() {
                        song.steps.push(Step { device: devices[0].clone(), ..Step::default() });
                    }
                    send = ui.add_enabled(!song.steps.is_empty(), egui::Button::new(tr("Send now"))).on_hover_text(tr("Sends the messages to the MIDI output, to try them")).clicked();
                });
                if devices.is_empty() {
                    ui.weak(tr("No device yet: Settings > Devices…"));
                }
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                keep = ui.add_enabled(!song.label().trim().is_empty(), egui::Button::new(tr("OK"))).clicked();
                cancel = ui.button(tr("Cancel")).clicked();
            });
        });
        if keep {
            self.keep_set_song();
        } else if cancel {
            self.setlist.editing = None;
        } else if send {
            self.send_steps_now();
        }
    }
}

/// The steps of a song, one a row: its device, its kind, its numbers, its pause.
fn step_rows(ui: &mut egui::Ui, song: &mut SetSong, devices: &[String]) {
    let mut remove = None;
    egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, true]).show(ui, |ui| {
        for (i, step) in song.steps.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt(("step device", i)).selected_text(step.device.clone()).width(130.0).show_ui(ui, |ui| {
                    for name in devices {
                        ui.selectable_value(&mut step.device, name.clone(), name);
                    }
                });
                egui::ComboBox::from_id_salt(("step kind", i)).selected_text(step_kind_name(&step.message)).width(140.0).show_ui(ui, |ui| {
                    for kind in [StepMessage::Program { bank: None, program: 0 }, StepMessage::Control { number: 0, value: 0 }, StepMessage::SysEx(Vec::new())] {
                        let same = std::mem::discriminant(&kind) == std::mem::discriminant(&step.message);
                        if ui.selectable_label(same, step_kind_name(&kind)).clicked() && !same {
                            step.message = kind;
                        }
                    }
                });
                match &mut step.message {
                    StepMessage::Program { bank, program } => {
                        let mut with_bank = bank.is_some();
                        if ui.checkbox(&mut with_bank, tr("Bank")).on_hover_text(tr("A bank select (control changes 0 and 32) before the program change")).changed() {
                            *bank = with_bank.then_some(0);
                        }
                        if let Some(bank) = bank {
                            ui.add(egui::DragValue::new(bank).range(0..=127));
                        }
                        ui.label(tr("Program"));
                        ui.add(egui::DragValue::new(program).range(0..=127));
                    }
                    StepMessage::Control { number, value } => {
                        ui.label("CC");
                        ui.add(egui::DragValue::new(number).range(0..=127));
                        ui.label(tr("Value"));
                        ui.add(egui::DragValue::new(value).range(0..=127));
                    }
                    StepMessage::SysEx(bytes) => {
                        // The text being typed is kept between frames, for a half-typed byte not to vanish.
                        let id = ui.make_persistent_id(("step sysex", i));
                        let kept: Option<String> = ui.data(|d| d.get_temp(id));
                        let mut text = match kept {
                            Some(text) if parse_hex(&text).as_ref().is_none_or(|parsed| parsed == bytes) => text,
                            _ => hex(bytes),
                        };
                        if ui.add(egui::TextEdit::singleline(&mut text).hint_text(tr("Hex bytes (F0 … F7)")).desired_width(220.0)).changed() {
                            if let Some(parsed) = parse_hex(&text) {
                                *bytes = parsed;
                            }
                        }
                        ui.data_mut(|d| d.insert_temp(id, text));
                    }
                }
                ui.label(tr("Delay (ms)"));
                ui.add(egui::DragValue::new(&mut step.delay_ms).range(0..=500));
                if ui.button("×").on_hover_text(tr("Remove")).clicked() {
                    remove = Some(i);
                }
            });
        }
    });
    if let Some(i) = remove {
        song.steps.remove(i);
    }
}

/// The songs' buttons, with what a right-click on one offers; says what was asked.
fn grid_of_songs(ui: &mut egui::Ui, buttons: &[Button], places: &[usize], songs: &[SetSong]) -> Option<Asked> {
    let (clicked, chosen) = grid(ui, buttons, |i, ui| {
        let index = places[i];
        let song = &songs[index];
        let mut choice = None;
        if ui.button(tr("Choose a picture…")).clicked() {
            choice = Some(Asked::Image(index));
        }
        if !song.image.is_empty() && ui.button(tr("Remove my picture")).on_hover_text(tr("The button shows the cover found again")).clicked() {
            choice = Some(Asked::NoImage(index));
        }
        ui.separator();
        if ui.button(tr("Find its project")).on_hover_text(tr("Among the previous sessions, by its artist and title; otherwise its tablature on the site of Settings > Site searched")).clicked() {
            choice = Some(Asked::FindProject(index));
        }
        if ui.button(tr("Give it a project file…")).clicked() {
            choice = Some(Asked::ProjectFile(index));
        }
        if ui.button(tr("Give it the project open now")).on_hover_text(tr("Saved in the folder of the list when it has no file yet")).clicked() {
            choice = Some(Asked::OpenProject(index));
        }
        if !song.project.is_empty() && ui.button(tr("Take its project away")).on_hover_text(tr("The project file stays where it is")).clicked() {
            choice = Some(Asked::ForgetProject(index));
        }
        ui.separator();
        if ui.button(tr("Change…")).clicked() {
            choice = Some(Asked::Edit(index));
        }
        if ui.button(tr("Remove from the list")).clicked() {
            choice = Some(Asked::Remove(index));
        }
        if choice.is_some() {
            ui.close_menu();
        }
        choice
    });
    clicked.map(|i| Asked::Choose(places[i])).or(chosen)
}

/// A grid of large buttons; says which was clicked, and what was chosen in
/// the right-click menu of one, which `menu` draws.
fn grid(ui: &mut egui::Ui, buttons: &[Button], mut menu: impl FnMut(usize, &mut egui::Ui) -> Option<Asked>) -> (Option<usize>, Option<Asked>) {
    let (mut clicked, mut chosen) = (None, None);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
            for (i, button) in buttons.iter().enumerate() {
                let response = draw_button(ui, button);
                if response.clicked() {
                    clicked = Some(i);
                }
                response.context_menu(|ui| {
                    if let Some(asked) = menu(i, ui) {
                        chosen = Some(asked);
                    }
                });
            }
        });
    });
    (clicked, chosen)
}

/// A button with its picture behind (cut to fill it), darkened for the text to
/// stay readable; the song chosen is outlined.
fn draw_button(ui: &mut egui::Ui, button: &Button) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(BUTTON, egui::Sense::click());
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let rounding = 6.0;
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, rounding, button.colour);
    if let Some(texture) = button.picture {
        egui::Image::from_texture(texture).uv(cover_uv(texture.size, BUTTON)).rounding(rounding).paint_at(ui, rect);
    }
    let veil = match (response.is_pointer_button_down_on(), response.hovered()) {
        (true, _) => 60,
        (false, true) => 90,
        _ if button.picture.is_some() => 150,
        _ => 0,
    };
    if veil > 0 {
        painter.rect_filled(rect, rounding, egui::Color32::from_black_alpha(veil));
    }
    let mut job = egui::text::LayoutJob::simple(button.text.clone(), egui::FontId::proportional(18.0), egui::Color32::WHITE, BUTTON.x - 20.0);
    job.halign = egui::Align::Center;
    let galley = ui.fonts(|f| f.layout_job(job));
    let at = rect.center() - galley.rect.center().to_vec2();
    painter.galley_with_override_text_color(at + egui::vec2(1.5, 1.5), galley.clone(), egui::Color32::from_black_alpha(220));
    painter.galley(at, galley, egui::Color32::WHITE);
    // A dot in the corner: the song has its project.
    if button.project {
        painter.circle_filled(rect.right_top() + egui::vec2(-12.0, 12.0), 5.0, egui::Color32::from_rgb(240, 200, 80));
    }
    if button.chosen {
        painter.rect_stroke(rect.shrink(2.0), rounding, egui::Stroke::new(4.0_f32, CHOSEN));
    }
    response
}

/// The part of a picture that fills a button without being stretched.
fn cover_uv(picture: egui::Vec2, button: egui::Vec2) -> egui::Rect {
    let (picture, button) = (picture.x / picture.y, button.x / button.y);
    if picture > button {
        let width = button / picture;
        egui::Rect::from_min_max(egui::pos2((1.0 - width) / 2.0, 0.0), egui::pos2((1.0 + width) / 2.0, 1.0))
    } else {
        let height = picture / button;
        egui::Rect::from_min_max(egui::pos2(0.0, (1.0 - height) / 2.0), egui::pos2(1.0, (1.0 + height) / 2.0))
    }
}
