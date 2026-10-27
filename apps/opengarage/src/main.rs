#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[macro_use]
mod i18n;

mod api;
mod app;
mod audio;
mod clap_host;
mod downloads;
mod editor_beats;
mod editor_tracks;
mod export;
mod files;
mod icon;
mod instruments;
mod library;
mod login;
mod isolate;
mod lv2_host;
mod g3s;
mod midi;
mod pedal;
mod pilot;
mod script;
mod plugin;
mod recording;
mod sessions;
mod setlist;
mod sites;
mod startup;
mod ui_bottom;
mod ui_downloads;
mod ui_side;
mod ui_g3s;
mod ui_pedal;
mod ui_setlist;
mod ui_sites;
mod ui_top;
mod ui_transfer;
mod ui_tracks;
mod theme;
mod track_icons;
mod userloops;
mod video;
mod widgets;
mod words;
mod vst3_host;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_sites;
#[cfg(test)]
mod tests_setlist;

use eframe::egui;

/// Opens the app on a given view, for screenshots and manual checks:
/// `OPENGARAGE_VIEW=notes|score|tab|multitab|drummer|controls|loops|global|site-search|sessions|export|import|video|words|words-whole|setlist|loading`.
fn apply_view(app: &mut app::App, view: &str) {
    use app::{AutoParam, BottomTab, NoteView, RightPanel};
    match view {
        "notes" => app.select_region(2, 0),
        "score" => {
            app.select_region(2, 0);
            app.note_view = NoteView::Score;
        }
        "tab" => {
            // The chords track, as a guitar: a tablature with chord grids,
            // and a few of the things a tablature writes around its notes.
            use og_engine::{Articulation, Content, Slide, Vibrato};
            app.select_region(0, 0);
            app.project.tracks[0].icon = 3;
            if let Content::Midi(notes) = &mut app.project.tracks[0].regions[0].content {
                let lowest = |beat: f64, notes: &[og_engine::Note]| notes.iter().enumerate().filter(|(_, n)| (n.start - beat).abs() < 1e-6).min_by_key(|(_, n)| n.pitch).map(|(i, _)| i);
                let shown = [
                    Articulation { slide: Some(Slide::Shift), palm_mute: true, ..Articulation::default() },
                    Articulation { hammer: true, palm_mute: true, ..Articulation::default() },
                    Articulation { bend: vec![(0.0, 0.0), (0.5, 2.0), (1.0, 2.0)], accent: 1, ..Articulation::default() },
                    Articulation { vibrato: Some(Vibrato::Wide), let_ring: true, ..Articulation::default() },
                ];
                for (beat, articulation) in [0.0, 4.0, 8.0, 12.0].into_iter().zip(shown) {
                    if let Some(i) = lowest(beat, notes) {
                        notes[i].articulation = articulation;
                    }
                }
                // Tremolo picking on the highest note of the second chord, a
                // trill on that of the fourth.
                let highest = |beat: f64, notes: &[og_engine::Note]| notes.iter().enumerate().filter(|(_, n)| (n.start - beat).abs() < 1e-6).max_by_key(|(_, n)| n.pitch).map(|(i, _)| i);
                if let Some(i) = highest(4.0, notes) {
                    notes[i].articulation.tremolo = Some(0.25);
                }
                if let Some(i) = highest(12.0, notes) {
                    notes[i].articulation.trill = Some((notes[i].pitch + 2, 0.125));
                }
            }
            // Swung from the third chord on.
            app.project.tracks[0].regions[0].swing = vec![og_engine::Swing::triplet(8.0, false)];
            app.note_view = NoteView::Tab;
        }
        "drummer" => app.select_region(3, 0),
        "controls" => {
            app.selected_track = 2;
            app.show_bottom = true;
            app.bottom_tab = BottomTab::Controls;
        }
        "downloads" => {
            app.open_downloads(downloads::Kind::Effects);
            app.downloads.selected = Some(0);
        }
        "sounds" => {
            app.open_downloads(downloads::Kind::Banks);
            app.downloads.selected = Some(0);
        }
        "full" => {
            app.selected_track = 2;
            app.show_bottom = true;
            app.bottom_full = true;
            app.bottom_tab = BottomTab::Controls;
        }
        "loops" => {
            app.right_panel = RightPanel::Loops;
            app.show_bottom = false;
        }
        // A real search on Songsterr, the first song found chosen: the window as it is used.
        "site-search" => {
            app.open_site_named("Songsterr");
            app.confirm_site();
            app.sites.search = "nothing else matters".to_owned();
            app.sites.pick_first = true;
            app.sites_search();
        }
        // The editor in the whole window, with the chords and the bass as
        // tablatures one under the other.
        "multitab" => {
            apply_view(app, "tab");
            app.select_region(0, 0);
            app.editor_tracks = vec![0, 1];
            app.show_bottom = true;
            app.bottom_full = true;
            app.bottom_tab = BottomTab::Editor;
        }
        "sites" => app.open_sites(),
        // A few projects kept, in the demonstration's own folder only.
        "sessions" if app::demo_mode() => {
            let _ = std::fs::remove_dir_all(app.sessions_dir());
            app.keep_sessions = true;
            for (artist, title, tempo) in [("", "Idée de riff", 126.0), ("Trio du dimanche", "Ballade en la", 72.0), ("Les Garagistes", "Premier essai", 98.5)] {
                let mut project = og_engine::templates::demo();
                (project.artist, project.title) = (artist.to_owned(), title.to_owned());
                app.replace_project(project, None);
                app.project.tempo = tempo;
            }
            app.open_sessions();
        }
        // A video track beside the tablature, the song played with the video.
        "video" => {
            apply_view(app, "tab");
            app.project.end = Some(64.0);
            let bars: Vec<f64> = (0..=16).map(|bar| f64::from(bar) * 4.0).collect();
            let offer = sites::VideoOffer { id: "aqz-KE-bpKQ".to_owned(), label: String::new(), points: (0..=16).map(|bar| 60.0 + f64::from(bar) * 3.1).collect() };
            app.add_video_track(&[offer], &bars);
            let video = app.project.tracks.len() - 1;
            app.select_region(video, 0);
            app.editor_tracks = vec![video, 0];
            app.show_bottom = true;
            app.bottom_full = true;
            app.bottom_tab = BottomTab::Editor;
            app.set_follow(true);
        }
        // Words of our own on the melody, the second line being sung.
        "words" | "words-whole" => {
            let text = "Here comes the sun- ny gar- age song We play it loud we play it long Here comes the night- time gar- age tune We play it till the ri- sing moon";
            let starts: Vec<f64> = {
                let mut starts: Vec<f64> = app.project.tracks[2].regions[0].notes().iter().map(|n| n.start).collect();
                starts.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
                starts
            };
            let mut sung: Vec<og_engine::lyrics::Syllable> = og_engine::lyrics::bind(text, starts.len())
                .into_iter()
                .zip(&starts)
                .filter_map(|(s, beat)| s.map(|text| og_engine::lyrics::Syllable { beat: *beat, text, line_start: false }))
                .collect();
            og_engine::lyrics::mark_lines(&mut sung, app.project.bar_len());
            app.project.lyrics = og_engine::lyrics::Lyrics { sung, source: "tablature".to_owned(), ..Default::default() };
            app.settings.lyrics_whole = view == "words-whole";
            app.select_region(2, 0);
            app.show_bottom = true;
            app.bottom_tab = BottomTab::Editor;
            app.note_view = NoteView::Score;
            app.go_to(5.0);
        }
        // A set list of our own songs, without covers from the network; one
        // song with a picture of its own (the program's photograph), one chosen.
        "setlist" | "setlist-chain" | "setlist-devices" | "setlist-pedal" | "setlist-g3s" | "setlist-board" | "setlist-script" | "setlist-api" => {
            let songs = [
                ("Les Garagistes", "Premier essai", ""),
                ("Les Garagistes", "Riff du lundi", ""),
                ("Trio du dimanche", "Ballade en la", "Ballade"),
                ("Trio du dimanche", "Valse lente", ""),
                ("The Fuzz Boxes", "Open Garage", ""),
                ("The Fuzz Boxes", "Seven Bars Blues", ""),
                ("Nina et les Ampli", "Retour de scène", ""),
                ("Nina et les Ampli", "Dernier rappel", "Rappel !"),
            ];
            app.settings.set_list.songs = songs
                .iter()
                .enumerate()
                .map(|(i, (artist, title, display))| og_engine::setlist::SetSong { number: i as u8 + 1, artist: (*artist).to_owned(), title: (*title).to_owned(), display: (*display).to_owned(), ..Default::default() })
                .collect();
            app.settings.set_list.no_covers = true;
            app.settings.set_list.numbers_from_one = true;
            let picture = app.data_dir.join("set-list-picture.jpg");
            if std::fs::create_dir_all(&app.data_dir).is_ok() && std::fs::write(&picture, startup::PICTURE).is_ok() {
                app.settings.set_list.songs[4].image = picture.to_string_lossy().into_owned();
            }
            for song in [0, 2, 4, 5] {
                app.settings.set_list.songs[song].project = format!("{}.ogproj", app.settings.set_list.songs[song].label());
            }
            app.open_set_list();
            app.choose_set_song(4);
            if view != "setlist" {
                // The chain's mode, with two pedals and the messages of the songs.
                use og_engine::setlist::{Device, PilotMode, Step, StepMessage};
                let list = &mut app.settings.set_list;
                list.mode = PilotMode::Chain;
                list.devices = vec![Device { name: "DD-500".to_owned(), model: "dd500".to_owned(), channel: 0, ..Device::default() }, Device { name: "RV-500".to_owned(), model: "rv500".to_owned(), channel: 1, ..Device::default() }];
                for (i, song) in list.songs.iter_mut().enumerate() {
                    song.steps.push(Step { device: "DD-500".to_owned(), message: StepMessage::Program { bank: None, program: i as u8 + 1 }, delay_ms: 20 });
                    song.steps.push(Step { device: "RV-500".to_owned(), message: StepMessage::Program { bank: Some(1), program: 2 }, delay_ms: 0 });
                }
                list.songs[4].steps.push(Step { device: "RV-500".to_owned(), message: StepMessage::Control { number: 80, value: 127 }, delay_ms: 0 });
                if view == "setlist-devices" {
                    app.setlist.page = crate::setlist::Page::Devices;
                } else if view == "setlist-api" {
                    // The settings of the API, on a port and a token made up for the picture.
                    app.settings.set_list.api_port = crate::api::DEFAULT_PORT;
                    app.settings.set_list.api_token = "0123456789abcdef0123456789abcdef".to_owned();
                    app.setlist.api_window = true;
                } else if view == "setlist-script" {
                    // A script of an AI's, reviewed: one action cannot be done.
                    let ctx = egui::Context::default();
                    let text = r#"{ "opengaragemusic": 1, "actions": [
                        { "pedal": "rv500", "memory": "50C", "name": "COMF NUMB", "from": "init", "set": { "Reverb : MODE": "HALL", "Reverb : time": "4.0", "Reverb : PRE-DELAY": 40, "Reverb : Effect Level": 30 } },
                        { "pedal": "dd500", "memory": "81A", "name": "GILMOUR 440", "from": "init", "set": { "Delay MODE": "ANALOG", "Delay Time": 440, "Feedback": 45, "Effect Level": 35 } },
                        { "pedal": "dd500", "pc_map": { "1": "81A" } },
                        { "pedal": "rv500", "memory": "50C", "set": { "Reverb : MODE": "CATHEDRAL" } },
                        { "g3s": "bank", "bank": 4, "name": "COMFORT.|NUMB" },
                        { "g3s": "preset", "bank": 4, "switch": 6, "name": "SOLO1", "loops": [2, 5, 11, 12], "midi": [ { "kind": "PC down", "channel": 1, "program": 4 }, { "kind": "PC down", "channel": 2, "program": 2 } ] } ] }"#;
                    // Many actions, for the page to scroll in the picture.
                    let mut text = text.to_owned();
                    let more: String = (1..=20).map(|n| format!(r#", {{ "g3s": "preset", "bank": 4, "switch": {}, "name": "P{n}", "loops": [2] }}"#, n % 14 + 1)).collect();
                    text.insert_str(text.rfind(']').expect("the actions end"), &more);
                    app.review_ai_script(&text, Some(std::path::PathBuf::from("comfortably-numb.json")), &ctx);
                } else if view == "setlist-g3s" || view == "setlist-board" {
                    // The G3S module on a backup made up: the banks named after the songs.
                    use og_engine::g3s::MidiLine;
                    app.settings.set_list.devices.push(og_engine::setlist::Device { name: "G3S".to_owned(), model: "g3s".to_owned(), channel: 4, ..og_engine::setlist::Device::default() });
                    app.open_g3s(2);
                    if let Some(s) = app.settings.set_list.devices.get_mut(2) {
                        use og_engine::setlist::Pedal;
                        let pedal = |name: &str, kind: &str, device: &str| Pedal { name: name.to_owned(), kind: kind.to_owned(), device: device.to_owned() };
                        s.loops = vec![vec![pedal("Fuzz Face", "fuzz", ""), pedal("Vox Wah", "wah", "")], vec![pedal("SD-1", "overdrive", "")], vec![pedal("DS-1", "distortion", "")], vec![pedal("BD-2w", "overdrive", "")], vec![pedal("Big Muff", "fuzz", "")], vec![pedal("Tube Screamer", "overdrive", "")], vec![pedal("PH-2", "phaser", "")], vec![pedal("OC-5", "octaver", "")], vec![pedal("CE-5", "chorus", "")], vec![pedal("Dyna Comp", "compressor", "")], vec![pedal("DD-500", "delay", "DD-500")], vec![pedal("RV-500", "reverb", "RV-500")]];
                        s.signal_order = vec![1, 8, 10, 6, 2, 4, 3, 5, 7, 9, 11, 12];
                        s.bank_switches = vec![1, 8];
                    }
                    if view == "setlist-board" {
                        if let Some(g) = &mut app.setlist.g3s {
                            g.board = true;
                        }
                    }
                    let mut backup = og_engine::g3s::blank(1);
                    backup.set_bank_name(0, "MANUAL");
                    backup.set_song_mode(true);
                    app.read_g3s_bytes(&backup.write(), None);
                    app.g3s_banks_from_songs();
                    if let Some(g) = &mut app.setlist.g3s {
                        let preset = &mut g.backup.banks[5].presets[1];
                        preset.set_name("INTRO");
                        for n in [9, 10, 11] {
                            preset.set_loop(n, true);
                        }
                        let _ = preset.set_order(&[10, 7, 11, 12, 1, 2, 3, 4, 5, 6, 8, 9]);
                        preset.set_midi(0, MidiLine { kind: 0xC0, channel: 0, number: 4, value: 0 });
                        preset.set_midi(1, MidiLine { kind: 0xC0, channel: 1, number: 3, value: 0 });
                        g.bank = 5;
                        g.preset = 1;
                        g.message = String::new();
                    }
                } else if view == "setlist-pedal" {
                    // The module of the DD-500, on a pedal made up, with a few patches named.
                    use og_engine::roland::{fake, Head};
                    let description = crate::pedal::description_of("dd500").expect("shipped");
                    let head = Head { device_id: description.device_id, model_id: description.model_id };
                    let mut pedal = fake::Device::new(head, [0x4D, 3]);
                    for (n, name) in [(0, "GILMOUR 440"), (1, "SLAPBACK"), (2, "DOTTED 8TH"), (3, "LONG AMBIENT"), (4, "SHORT")] {
                        let mut raw = description.init_patch();
                        let _ = og_engine::devices::set(&mut raw, description.name_param(), &og_engine::devices::Value::Text(name.to_owned()));
                        let _ = og_engine::devices::set(&mut raw, description.param("Delay Time").expect("named"), &og_engine::devices::Value::Number(440 - 50 * n as i64));
                        pedal.fill(description.user_address(n), &raw);
                    }
                    let ctx = egui::Context::default();
                    app.open_pedal_on(0, Box::new(pedal), &ctx);
                    if let Some(session) = &mut app.setlist.pedal {
                        session.ask(crate::pedal::Job::ReadNames);
                    }
                } else {
                    app.setlist.editing = list.songs.get(4).cloned().map(|song| (Some(4), song));
                }
            }
        }
        "export" => app.dialog = Some(app::Dialog::Export),
        "import" => app.dialog = Some(app::Dialog::Import),
        "global" => {
            app.show_global = true;
            app.show_master = true;
            app.automation = Some(AutoParam::Volume);
            app.show_bottom = false;
        }
        _ => {}
    }
}

/// Writes what went wrong to a file before the program stops: a release build
/// has no console to show it.
fn log_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if let Some(dir) = std::env::var_os("LOCALAPPDATA") {
            let dir = std::path::PathBuf::from(dir).join("OpenGarageMusic");
            let _ = std::fs::create_dir_all(&dir);
            let stamp = sessions::now();
            let text = format!("[{stamp}] OpenGarageMusic {} : {info}\n", env!("OG_VERSION"));
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log")) {
                let _ = std::io::Write::write_all(&mut file, text.as_bytes());
            }
        }
        previous(info);
    }));
}

fn main() -> eframe::Result<()> {
    // Started by the application itself to hold one plug-in: no window, no sound card.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some(isolate::FLAG) {
        isolate::serve(&args[2..]);
        return Ok(());
    }
    log_panics();
    // The guide for an AI, written where asked, and nothing else.
    if args.get(1).map(String::as_str) == Some("--ai-guide") {
        let path = args.get(2).map_or("ai-guide.md", String::as_str);
        if let Err(e) = std::fs::write(path, script::guide_text()) {
            eprintln!("{path}: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }
    // The MIDI Pilot alone, for the stage: its window, no sound card.
    if args.get(1).map(String::as_str) == Some(pilot::FLAG) {
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_title("MIDI Pilot")
                .with_icon(egui::IconData { rgba: icon::rgba(64), width: 64, height: 64 })
                .with_inner_size([1000.0, 650.0])
                .with_min_inner_size([420.0, 260.0]),
            ..Default::default()
        };
        return eframe::run_native("MIDI Pilot", options, Box::new(|_cc| Ok(Box::new(pilot::Pilot::new()))));
    }
    // A project file given on the command line (or dropped on the program's
    // icon) is opened once the program is ready.
    let project = args.get(1).filter(|a| !a.starts_with("--")).map(std::path::PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OpenGarageMusic")
            .with_icon(egui::IconData { rgba: icon::rgba(64), width: 64, height: 64 })
            // First the loading screen: the photograph, without a frame. The
            // window takes its usual size once the program is ready.
            .with_inner_size(startup::picture_size() * startup::SCALE)
            .with_decorations(false)
            .with_resizable(false),
        ..Default::default()
    };
    eframe::run_native(
        "OpenGarageMusic",
        options,
        // The program starts behind its loading screen, which hands it over
        // once ready.
        Box::new(|_cc| {
            Ok(Box::new(startup::Startup::new(|app| {
                // For screenshots and checks: a language for this run only.
                if let Ok(code) = std::env::var("OPENGARAGE_LANG") {
                    i18n::set_language(&code);
                }
                // For the pictures of the documentation, which touch no setting.
                if let (Ok(theme), true) = (std::env::var("OPENGARAGE_THEME"), app::demo_mode()) {
                    app.settings.theme = theme;
                    app.refresh_themes();
                }
                if let Ok(view) = std::env::var("OPENGARAGE_VIEW") {
                    apply_view(app, &view);
                }
                if let Some(path) = project {
                    if let Err(e) = app.open_path(&path) {
                        app.status = tf!("Cannot open {}: {e}" [e], path.display());
                    }
                }
            })))
        }),
    )
}
