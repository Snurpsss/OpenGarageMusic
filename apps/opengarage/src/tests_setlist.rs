//! The set list, driven headlessly: its window (inside the main one, where a
//! window of its own cannot be had), a list read from CSV, songs chosen, and
//! their projects opened and found; the chain's mode, its devices and the
//! messages of its songs. No MIDI output is opened: the bytes a song sends
//! are read from `sent`.

use std::path::PathBuf;

use og_engine::setlist::{Device, PilotMode, SetOrder, SetSong, Step, StepMessage};

use crate::app::Dialog;
use crate::pedal::{Job, Tab};
use crate::setlist::{hex, parse_hex, Page};
use crate::tests::Harness;

/// A folder of the test's own, empty.
fn folder(name: &str) -> PathBuf {
    crate::tests::temp_folder(&format!("setlist-{name}"))
}

fn song(number: u8, artist: &str, title: &str) -> SetSong {
    SetSong { number, artist: artist.to_owned(), title: title.to_owned(), ..SetSong::default() }
}

fn device(name: &str, channel: u8) -> Device {
    Device { name: name.to_owned(), model: String::new(), channel, ..Device::default() }
}

fn program(device: &str, program: u8) -> Step {
    Step { device: device.to_owned(), message: StepMessage::Program { bank: None, program }, delay_ms: 0 }
}

#[test]
fn a_list_is_read_from_csv_and_its_songs_chosen() {
    let mut h = Harness::new();
    h.app.open_set_list();
    h.settle();
    h.app.set_list_from_csv("Numero;Artiste;Chanson;Display\n1;Abc;Song;\n2;Def;Other;Intro\n128;Ghi;Last\n", "list.csv");
    h.settle();
    let songs = &h.app.settings.set_list.songs;
    assert_eq!(songs.len(), 3);
    assert_eq!((songs[1].label(), songs[2].number), ("Intro".to_owned(), 128));
    assert!(h.app.setlist.message.contains('3') && h.app.setlist.message.contains("list.csv"), "{}", h.app.setlist.message);
    // Without a MIDI output the song is chosen all the same, and the window says so.
    h.app.choose_set_song(1);
    h.settle();
    assert_eq!(h.app.setlist.active, Some(1));
    assert!(h.app.setlist.message.contains("Intro") && h.app.setlist.message.contains("MIDI"), "{}", h.app.setlist.message);
    // 128 is sent only when the numbers start from 1.
    h.app.choose_set_song(2);
    assert!(h.app.setlist.message.contains("128"), "{}", h.app.setlist.message);
    // A wrong line is pointed at, and the list stays as it was.
    h.app.set_list_from_csv("1,Abc,Song\nx,Def,Other\n", "wrong.csv");
    assert!(h.app.setlist.message.contains("2") && h.app.setlist.message.contains("wrong.csv"));
    assert_eq!(h.app.settings.set_list.songs.len(), 3);
    // The orders and the artists' buttons draw.
    for order in SetOrder::ALL {
        h.app.settings.set_list.order = order;
        h.settle();
    }
    h.app.choose_set_song(0);
    assert_eq!(h.app.setlist.artist.as_deref(), Some("Abc"), "by artist, the songs of the artist chosen show");
    h.settle();
    h.app.close_set_list();
    h.settle();
    assert!(!h.app.setlist.open);
}

#[test]
fn a_song_added_changed_and_removed_keeps_its_project_and_picture() {
    let mut h = Harness::new();
    h.app.open_set_list();
    h.app.settings.set_list.songs = vec![song(1, "Abc", "Song"), song(2, "Def", "Other")];
    h.app.settings.set_list.songs[1].project = "x.ogproj".to_owned();
    h.app.set_song_image(1, Some(std::path::Path::new("picture.png")));
    h.app.setlist.editing = Some((Some(1), song(5, "Def", "Other (live)")));
    h.settle();
    h.app.keep_set_song();
    let changed = &h.app.settings.set_list.songs[1];
    assert_eq!((changed.number, changed.title.as_str(), changed.project.as_str(), changed.image.as_str()), (5, "Other (live)", "x.ogproj", "picture.png"));
    h.app.set_song_image(1, None);
    assert!(h.app.settings.set_list.songs[1].image.is_empty());
    h.app.setlist.editing = Some((None, song(9, "Ghi", "New")));
    h.app.keep_set_song();
    assert_eq!(h.app.settings.set_list.songs.len(), 3);
    h.app.choose_set_song(2);
    h.app.remove_set_song(0);
    assert_eq!(h.app.setlist.active, Some(1), "the song chosen moves up with the list");
    // Read again from a file, a song already there keeps its project.
    h.app.set_list_from_csv("1,def,other (live)\n2,Jkl,Song\n", "again.csv");
    assert_eq!(h.app.settings.set_list.songs[0].project, "x.ogproj");
    h.settle();
}

#[test]
fn choosing_a_song_opens_its_project_when_asked() {
    let dir = folder("open");
    let mut h = Harness::new();
    let path = dir.join("Abc - Song.ogproj");
    h.app.save_to(&path).unwrap();
    h.app.path = None;
    h.app.settings.set_list.songs = vec![song(1, "Abc", "Song"), song(2, "Def", "Other")];
    h.app.link_set_project(0, &path);
    h.app.open_set_list();
    h.app.choose_set_song(0);
    assert_eq!(h.app.path, None, "not opened unless asked");
    h.app.settings.set_list.open_project = true;
    h.app.choose_set_song(0);
    assert_eq!(h.app.path.as_deref(), Some(path.as_path()));
    h.app.choose_set_song(1);
    assert!(h.app.setlist.message.contains(crate::i18n::tr("It has no project: right-click it to find one.")), "{}", h.app.setlist.message);
    h.settle();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_project_kept_in_the_sessions_is_found_by_artist_and_title() {
    let dir = folder("sessions");
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    let mut project = og_engine::templates::demo();
    (project.artist, project.title) = ("Guns N' Roses".to_owned(), "Sweet Child O' Mine".to_owned());
    h.app.replace_project(project, None);
    // Another project comes: the first is kept as a session, without a file.
    h.app.replace_project(og_engine::templates::demo(), None);
    h.app.settings.set_list.songs = vec![song(4, "guns n roses", "Sweet child o mine")];
    h.app.open_set_list();
    assert!(h.app.find_set_project(0), "found at once, without the sites");
    let linked = PathBuf::from(&h.app.settings.set_list.songs[0].project);
    assert!(linked.is_file() && linked.starts_with(h.app.set_list_folder()), "{}", linked.display());
    assert_eq!(h.app.project.title, "Sweet Child O' Mine");
    assert!(h.app.dialog.is_none());
    h.settle();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn preparing_the_list_stops_when_the_window_of_the_sites_is_closed() {
    let dir = folder("prepare");
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    h.app.settings.set_list.songs = vec![song(1, "Abc", "Song"), song(2, "Def", "Other")];
    h.app.open_set_list();
    h.app.prepare_set_list();
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)));
    assert_eq!(h.app.setlist.making, Some(0));
    assert_eq!(h.app.sites.search, "Abc Song");
    assert_eq!(h.app.setlist.queue.len(), 1);
    h.app.dialog = None;
    h.settle();
    assert_eq!((h.app.setlist.making, h.app.setlist.queue.len()), (None, 0));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn choosing_a_song_in_chain_mode_records_the_bytes_of_its_steps_without_an_output() {
    let mut h = Harness::new();
    h.app.open_set_list();
    let list = &mut h.app.settings.set_list;
    list.mode = PilotMode::Chain;
    list.devices = vec![device("DD-500", 0), device("RV-500", 1)];
    list.songs = vec![song(1, "Pink Floyd", "Time"), song(2, "Oasis", "Wonderwall")];
    list.songs[0].steps = vec![program("DD-500", 6), program("RV-500", 2)];
    h.app.choose_set_song(0);
    assert_eq!(h.app.setlist.sent, [vec![0xC0, 6], vec![0xC1, 2]]);
    assert!(h.app.setlist.message.contains("Time") && h.app.setlist.message.contains("MIDI"), "{}", h.app.setlist.message);
    h.settle();
    // A song without a message says so; a device not in the chain is named.
    h.app.choose_set_song(1);
    assert!(h.app.setlist.sent.is_empty() && h.app.setlist.message.contains("Wonderwall") && !h.app.setlist.message.contains("MIDI"), "{}", h.app.setlist.message);
    h.app.settings.set_list.songs[1].steps = vec![program("Looper", 1)];
    h.app.choose_set_song(1);
    assert!(h.app.setlist.message.contains("Looper"), "{}", h.app.setlist.message);
    // The switcher's mode sends the number, as before, whatever the steps.
    h.app.settings.set_list.mode = PilotMode::Switcher;
    h.app.choose_set_song(0);
    assert_eq!(h.app.setlist.sent, [vec![0xB0, 0, 1]]);
    h.settle();
}

#[test]
fn the_devices_of_a_csv_file_are_added_to_the_list_and_the_mode_follows() {
    let mut h = Harness::new();
    h.app.open_set_list();
    h.app.settings.set_list.channel = 4;
    h.app.set_list_from_csv("Number;Artist;Song;Display;DD-500;RV-500\n1;Pink Floyd;Time;;6;2\n2;Oasis;Wonderwall;;;1\n", "chain.csv");
    let list = &h.app.settings.set_list;
    assert_eq!(list.mode, PilotMode::Chain);
    assert_eq!(list.devices.iter().map(|d| (d.name.as_str(), d.channel)).collect::<Vec<_>>(), [("DD-500", 4), ("RV-500", 4)], "the devices named take the channel of the settings");
    assert_eq!((list.songs[0].steps.len(), list.songs[1].steps.len()), (2, 1));
    assert!(h.app.setlist.message.contains('2') && h.app.setlist.message.contains("chain.csv"), "{}", h.app.setlist.message);
    h.settle();
    // Read again, a device known (up to case and signs) is kept as it is set.
    h.app.settings.set_list.devices[0].channel = 0;
    h.app.set_list_from_csv("Number,Artist,Song,Display,dd500\n1,A,B,,3\n", "again.csv");
    let list = &h.app.settings.set_list;
    assert_eq!((list.devices.len(), list.devices[0].channel, list.songs[0].steps[0].device.as_str()), (2, 0, "dd500"));
    assert_eq!(list.messages_for(&list.songs[0]), Ok(vec![(vec![0xC0, 3], 0)]), "the step finds its device whatever the way its name is written");
    // A wrong program is pointed at, and the list stays as it was.
    h.app.set_list_from_csv("Number,Artist,Song,Display,DD-500\n1,A,B,,x\n", "wrong.csv");
    assert!(h.app.setlist.message.contains("wrong.csv") && h.app.setlist.message.contains("DD-500") && h.app.setlist.message.contains('x'), "{}", h.app.setlist.message);
    assert_eq!(h.app.settings.set_list.songs.len(), 1);
    // A file without device columns leaves the mode alone.
    h.app.set_list_from_csv("1,A,B\n", "plain.csv");
    assert_eq!(h.app.settings.set_list.mode, PilotMode::Chain);
}

#[test]
fn a_device_renamed_or_removed_takes_its_steps_along() {
    let mut h = Harness::new();
    h.app.open_set_list();
    let list = &mut h.app.settings.set_list;
    list.mode = PilotMode::Chain;
    list.devices = vec![device("DD-500", 0), device("RV-500", 1)];
    list.songs = vec![song(1, "Pink Floyd", "Time")];
    list.songs[0].steps = vec![program("DD-500", 6), program("RV-500", 2)];
    h.app.setlist.editing = Some((Some(0), list.songs[0].clone()));
    // Renamed (the name is written in place, then the old one is given): the steps follow.
    h.app.settings.set_list.devices[0].name = "Delay".to_owned();
    h.app.rename_set_device(0, "DD-500");
    assert_eq!(h.app.settings.set_list.songs[0].steps[0].device, "Delay");
    assert_eq!(h.app.setlist.editing.as_ref().unwrap().1.steps[0].device, "Delay", "the song being written follows too");
    // An empty name goes back to the old one.
    h.app.settings.set_list.devices[0].name = "  ".to_owned();
    h.app.rename_set_device(0, "Delay");
    assert_eq!(h.app.settings.set_list.devices[0].name, "Delay");
    // Removed: its steps go, the others stay.
    h.app.remove_set_device(1);
    let list = &h.app.settings.set_list;
    assert_eq!((list.devices.len(), list.songs[0].steps.len(), h.app.setlist.editing.as_ref().unwrap().1.steps.len()), (1, 1, 1));
    assert!(h.app.setlist.message.contains("RV-500") && h.app.setlist.message.contains('1'), "{}", h.app.setlist.message);
    h.app.add_set_device();
    let names: Vec<&str> = h.app.settings.set_list.devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names.len(), 2);
    assert!(!names[1].is_empty() && names[1] != "Delay", "{names:?}");
    h.settle();
}

#[test]
fn the_devices_page_opens_from_the_settings_and_goes_back_to_the_songs() {
    let mut h = Harness::new();
    h.app.open_set_list();
    h.app.settings.set_list.devices = vec![device("DD-500", 0), device("RV-500", 1)];
    h.app.settings.set_list.devices[1].model = "rv500".to_owned();
    h.app.setlist.page = Page::Devices;
    h.settle();
    h.app.settings.set_list.mode = PilotMode::Chain;
    h.app.setlist.sent = vec![vec![0xC0, 6]];
    h.settle();
    assert_eq!(h.app.setlist.page, Page::Devices);
    h.app.close_set_list();
    assert_eq!(h.app.setlist.page, Page::Songs);
    h.settle();
}

#[test]
fn the_song_editor_sends_its_messages_now() {
    let mut h = Harness::new();
    h.app.open_set_list();
    h.app.settings.set_list.mode = PilotMode::Chain;
    h.app.settings.set_list.devices = vec![device("Amp", 5)];
    let mut tune = song(1, "A", "B");
    tune.steps = vec![
        Step { device: "Amp".to_owned(), message: StepMessage::Program { bank: Some(1), program: 2 }, delay_ms: 10 },
        Step { device: "Amp".to_owned(), message: StepMessage::Control { number: 7, value: 100 }, delay_ms: 0 },
        Step { device: "Amp".to_owned(), message: StepMessage::SysEx(vec![0x7E, 0x7F, 0x06, 0x01]), delay_ms: 0 },
    ];
    h.app.setlist.editing = Some((None, tune));
    h.settle();
    h.app.send_steps_now();
    assert_eq!(h.app.setlist.sent, [vec![0xB5, 0, 1], vec![0xB5, 32, 0], vec![0xC5, 2], vec![0xB5, 7, 100], vec![0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]]);
    assert!(h.app.setlist.editing.is_some(), "the window stays open");
    assert!(h.app.setlist.message.contains("MIDI"), "{}", h.app.setlist.message);
    h.settle();
    assert_eq!(parse_hex("f0 7e, 7F"), Some(vec![0xF0, 0x7E, 0x7F]));
    assert_eq!((parse_hex("F0 7"), parse_hex("zz")), (None, None));
    assert_eq!(hex(&[0xF0, 7]), "F0 07");
}

#[test]
fn old_settings_without_a_mode_load_as_the_switcher() {
    let dir = folder("old-settings");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.json");
    std::fs::write(&file, r#"{"set_list":{"songs":[{"number":3,"artist":"Oasis","title":"Wonderwall"}],"channel":4}}"#).unwrap();
    let app = crate::app::App::with_audio(crate::audio::silent(og_engine::templates::demo()), Some(file));
    let list = &app.settings.set_list;
    assert_eq!((list.mode, list.channel, list.devices.len(), list.songs[0].steps.len()), (PilotMode::Switcher, 4, 0, 0));
    let _ = std::fs::remove_dir_all(dir);
}

/// Waits for the pedal's thread to answer everything asked.
fn settle_pedal(h: &mut Harness) {
    for _ in 0..400 {
        h.app.poll_pedal();
        if h.app.setlist.pedal.as_ref().is_none_or(|s| !s.busy()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the pedal's thread did not answer");
}

#[test]
fn a_pedal_module_reads_edits_and_stores_through_a_made_up_pedal() {
    use og_engine::devices::Value;
    use og_engine::roland::{fake, Head};
    let dir = folder("pedal");
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    h.app.open_set_list();
    h.app.settings.set_list.mode = PilotMode::Chain;
    h.app.settings.set_list.devices = vec![device("Delay", 0), device("Looper", 2)];
    h.app.settings.set_list.devices[0].model = "dd500".to_owned();
    let description = crate::pedal::description_of("dd500").unwrap();
    assert!(crate::pedal::description_of("").is_none() && crate::pedal::description_of("rv500").is_some());
    // The made-up pedal plays 02B, whose memory holds a patch named TEST.
    let head = Head { device_id: description.device_id, model_id: description.model_id };
    let mut pedal = fake::Device::new(head, [0x4D, 3]);
    let mut raw = description.init_patch();
    og_engine::devices::set(&mut raw, description.name_param(), &Value::Text("TEST".into())).unwrap();
    pedal.fill(description.user_address(4), &raw);
    pedal.fill(description.addresses.current, &og_engine::roland::nibbles(4, 3));
    let ctx = eframe::egui::Context::default();
    h.app.open_pedal_on(1, Box::new(fake::Device::new(head, [0, 0])), &ctx);
    assert!(h.app.setlist.pedal.is_none() && h.app.setlist.message.contains(crate::i18n::tr("This device has no module: give it a model under Devices.")), "{}", h.app.setlist.message);
    h.app.open_pedal_on(0, Box::new(pedal), &ctx);
    assert_eq!(h.app.setlist.page, Page::Pedal(0));
    settle_pedal(&mut h);
    let session = h.app.setlist.pedal.as_ref().unwrap();
    assert_eq!((session.selected, session.raw.as_ref().map(Vec::len)), (Some(4), Some(475)), "the patch played is chosen and read");
    assert_eq!(session.names[4], "TEST");
    assert!(session.message.contains("02B"), "{}", session.message);
    h.settle();
    // A knob moved goes to the pedal; stored, the memory is read back the same.
    let time = description.param("Delay Time").unwrap().clone();
    h.app.set_pedal_value(&time, &Value::Number(440));
    settle_pedal(&mut h);
    let session = h.app.setlist.pedal.as_ref().unwrap();
    assert!(session.edited && og_engine::devices::get(session.raw.as_ref().unwrap(), &time) == Value::Number(440));
    h.app.setlist.pedal.as_mut().unwrap().confirm = Some(crate::pedal::Confirm::Store(4));
    h.app.confirm_pedal();
    settle_pedal(&mut h);
    let session = h.app.setlist.pedal.as_ref().unwrap();
    assert!(!session.edited && session.message.contains("02B"), "{}", session.message);
    assert_eq!(session.backups.len(), 1, "what the memory held is kept");
    assert!(session.backups[0].is_file());
    assert_eq!(og_engine::devices::get(&session.file.patch(&description, 4).unwrap(), &time), Value::Number(440), "the file follows the pedal");
    // The names, the map and the system are read; a song's messages go through the module.
    for job in [Job::ReadNames, Job::ReadPcMap, Job::ReadSystem] {
        h.app.setlist.pedal.as_mut().unwrap().ask(job);
    }
    settle_pedal(&mut h);
    let session = h.app.setlist.pedal.as_ref().unwrap();
    assert_eq!((session.names.len(), session.pc_map.len(), session.system.len()), (297, 384, 0x2C));
    h.app.settings.set_list.songs = vec![song(1, "A", "B")];
    h.app.settings.set_list.songs[0].steps = vec![program("Delay", 3)];
    h.app.choose_set_song(0);
    assert_eq!(h.app.setlist.sent, [vec![0xC0, 3]]);
    settle_pedal(&mut h);
    assert!(h.app.setlist.pedal.as_ref().unwrap().message.contains('1'), "{}", h.app.setlist.pedal.as_ref().unwrap().message);
    // The file is written and read back; the maker's backup is read too.
    let path = dir.join("delay.pedal.json");
    h.app.write_pedal_file(&path);
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("\"02B\"") && text.contains("pc_map"));
    h.app.read_pedal_file("{\"patches\":{}}", None);
    assert!(h.app.setlist.pedal.as_ref().unwrap().file.patches.is_empty());
    h.app.read_pedal_file(&text, Some(path.clone()));
    let session = h.app.setlist.pedal.as_ref().unwrap();
    assert_eq!((session.file.patches.len(), session.file_path.as_deref()), (1, Some(path.as_path())));
    h.app.read_pedal_file("not json", None);
    assert!(h.app.setlist.pedal.as_ref().unwrap().message.contains(crate::i18n::tr("The file was not read: {e}").split(':').next().unwrap()));
    // Every tab draws.
    for tab in [Tab::Patches, Tab::PcMap, Tab::System, Tab::File] {
        h.app.setlist.pedal.as_mut().unwrap().tab = tab;
        h.settle();
    }
    h.app.close_pedal();
    assert!(h.app.setlist.pedal.is_none());
    assert_eq!(h.app.setlist.page, Page::Devices);
    h.settle();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_g3s_module_edits_a_backup_and_writes_one_its_editor_restores() {
    use og_engine::g3s::{self, MidiLine};
    let dir = folder("g3s");
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::new();
    h.app.open_set_list();
    h.app.settings.set_list.devices = vec![device("G3S", 4)];
    h.app.settings.set_list.devices[0].model = "g3s".to_owned();
    h.app.settings.set_list.songs = vec![song(1, "Pink Floyd", "Shine On You Crazy Diamond"), song(2, "Pink Floyd", "Time")];
    h.app.open_g3s(0);
    assert_eq!(h.app.setlist.page, Page::G3s);
    h.settle();
    // A backup of the switcher's is read; the banks are named after the songs.
    let mut backup = g3s::blank(2);
    backup.set_bank_name(0, "MANUAL");
    backup.banks[1].presets[1].set_name("NAPPE");
    backup.banks[1].presets[1].set_midi(0, MidiLine { kind: 0xC0, channel: 0, number: 4, value: 0 });
    h.app.read_g3s_bytes(&backup.write(), None);
    assert!(h.app.setlist.g3s.as_ref().unwrap().message.contains('2'), "{}", h.app.setlist.g3s.as_ref().unwrap().message);
    h.app.g3s_banks_from_songs();
    let g = h.app.setlist.g3s.as_ref().unwrap();
    assert_eq!(g.backup.banks.len(), 3, "a bank added for the second song");
    assert_eq!((g.backup.bank_name(0).as_str(), g.backup.bank_name(1).as_str(), g.backup.bank_name(2).as_str()), ("MANUAL", "SHINE ON|YOU CRAZY", "TIME"));
    assert_eq!(g.backup.banks[1].presets[1].name(), "NAPPE", "the presets stay");
    assert_eq!(crate::g3s::two_lines("Comfortably Numb"), "COMFORTABL|NUMB");
    assert_eq!(crate::g3s::two_lines("a b"), "A B");
    // Written, the file checks out and reads back the same; the songs come from the banks.
    let path = dir.join("test.g3b");
    h.app.write_g3s_file(&path);
    let g = h.app.setlist.g3s.as_ref().unwrap();
    assert!(!g.edited && g.path.as_deref() == Some(path.as_path()), "{}", g.message);
    let bytes = std::fs::read(&path).unwrap();
    assert!(g3s::Backup::check(&bytes).is_ok());
    assert_eq!(g3s::Backup::parse(&bytes).unwrap().banks.len(), 3);
    h.app.settings.set_list.songs.clear();
    h.app.g3s_songs_from_banks();
    let titles: Vec<&str> = h.app.settings.set_list.songs.iter().map(|s| s.title.as_str()).collect();
    assert_eq!(titles, ["SHINE ON YOU CRAZY", "TIME"]);
    h.app.g3s_songs_from_banks();
    assert_eq!(h.app.settings.set_list.songs.len(), 2, "known already");
    // Edited and drawn: the hidden presets too; then a broken file is refused.
    h.app.setlist.g3s.as_mut().unwrap().show_hidden = true;
    h.app.setlist.g3s.as_mut().unwrap().bank = 1;
    h.app.setlist.g3s.as_mut().unwrap().preset = 1;
    h.settle();
    h.app.read_g3s_bytes(b"not a backup", None);
    assert!(h.app.setlist.g3s.as_ref().unwrap().message.contains(crate::i18n::tr("The file is not a backup of the switcher: {e}").split(':').next().unwrap()));
    h.app.check_g3s();
    h.app.close_g3s();
    assert!(h.app.setlist.g3s.is_none() && h.app.setlist.page == Page::Devices);
    h.settle();
    let _ = std::fs::remove_dir_all(dir);
}

/// Waits for a script to be applied: the pedal's thread and the script's steps.
fn settle_script(h: &mut Harness) {
    // Each memory stored takes the pedal a second, as the real ones do.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::time::Instant::now() < deadline {
        h.app.poll_set_list();
        if h.app.setlist.script.as_ref().is_none_or(|s| s.running.is_none()) {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("the script did not end");
}

#[test]
fn an_ai_script_is_reviewed_applied_on_the_made_up_pedal_and_exported() {
    use og_engine::devices::Value;
    use og_engine::roland::{fake, Head};
    let dir = folder("script");
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    h.app.open_set_list();
    let ctx = eframe::egui::Context::default();
    // Reviewed: a pedal not in the chain, a wrong value, a wrong memory are named; nothing is written.
    let text = r#"{ "opengaragemusic": 1, "actions": [
        { "pedal": "rv500", "memory": "50C", "name": "COMF NUMB", "from": "init", "set": { "Reverb : MODE": "HALL", "Reverb : PRE-DELAY": 40 } },
        { "pedal": "rv500", "memory": "50C", "set": { "Reverb : MODE": "CATHEDRAL" } },
        { "pedal": "rv500", "memory": "99D", "set": {} },
        { "pedal": "md500", "memory": "01A", "set": {} },
        { "g3s": "bank", "bank": 2, "name": "COMFORT.|NUMB" } ] }"#;
    h.app.review_ai_script(text, None, &ctx);
    assert_eq!(h.app.setlist.page, Page::Script);
    let s = h.app.setlist.script.as_ref().unwrap();
    assert_eq!(s.summaries.len(), 5);
    assert!(s.summaries[0].1.as_deref().is_some_and(|e| e.contains("rv500")), "no device of that model yet: {:?}", s.summaries[0]);
    assert!(s.summaries[4].1.is_none(), "{:?}", s.summaries[4]);
    h.settle();
    h.app.settings.set_list.devices = vec![device("Reverb", 1)];
    h.app.settings.set_list.devices[0].model = "rv500".to_owned();
    h.app.review_ai_script(text, None, &ctx);
    let s = h.app.setlist.script.as_ref().unwrap();
    assert!(s.summaries[0].1.is_none(), "{:?}", s.summaries[0]);
    assert!(s.summaries[1].1.as_deref().is_some_and(|e| e.contains("CATHEDRAL")));
    assert!(s.summaries[2].1.as_deref().is_some_and(|e| e.contains("99D")));
    assert!(s.summaries[3].1.as_deref().is_some_and(|e| e.contains("md500")));
    assert_eq!(s.errors(), 3);
    h.app.apply_ai_script();
    assert!(h.app.setlist.script.as_ref().unwrap().running.is_none(), "not applied with errors");
    // A sound script, applied: the memory is written in the made-up pedal, the G3S bank named.
    let good = r#"{ "actions": [
        { "pedal": "rv500", "memory": "50C", "name": "COMF NUMB", "from": "init", "set": { "Reverb : MODE": "HALL", "Reverb : PRE-DELAY": 40 } },
        { "pedal": "rv500", "memory": "50C", "set": { "Reverb : Effect Level": 30 } },
        { "pedal": "rv500", "pc_map": { "3": "50C" } },
        { "pedal": "rv500", "system": { "MIDI Rx Channel": 1 } },
        { "g3s": "bank", "bank": 2, "name": "COMFORT.|NUMB" },
        { "g3s": "preset", "bank": 2, "switch": 2, "name": "INTRO", "loops": [12], "midi": [ { "kind": "PC down", "channel": 2, "program": 3 } ] } ] }"#;
    h.app.review_ai_script(good, Some(dir.join("good.json")), &ctx);
    assert_eq!(h.app.setlist.script.as_ref().unwrap().errors(), 0, "{:?}", h.app.setlist.script.as_ref().unwrap().summaries);
    let description = crate::pedal::description_of("rv500").unwrap();
    let head = Head { device_id: description.device_id, model_id: description.model_id };
    h.app.open_pedal_on(0, Box::new(fake::Device::new(head, [0x42, 3])), &ctx);
    h.app.setlist.page = Page::Script;
    settle_pedal(&mut h);
    h.app.apply_ai_script();
    settle_script(&mut h);
    let s = h.app.setlist.script.as_ref().unwrap();
    let results: Vec<bool> = s.results.iter().map(|r| matches!(r, Some(Ok(_)))).collect();
    assert_eq!(results, [true; 6], "{:?}", s.results);
    assert!(s.message.contains('6'), "{}", s.message);
    let session = h.app.setlist.pedal.as_ref().unwrap();
    let raw = session.file.patch(&description, 149).unwrap();
    let (pc_map, system) = (session.pc_map.clone(), session.system.clone());
    assert_eq!(description.patch_name(&raw), "COMF NUMB");
    assert_eq!(og_engine::devices::get(&raw, description.param("Reverb : MODE").unwrap()), Value::Number(1));
    assert_eq!(og_engine::devices::get(&raw, description.param("Reverb : PRE-DELAY").unwrap()), Value::Number(40), "kept by the second action, from the memory");
    assert_eq!(og_engine::devices::get(&raw, description.param("Reverb : Effect Level").unwrap()), Value::Number(30));
    assert_eq!(pc_map[3], 149);
    let rx = description.system.iter().find(|p| p.name == "MIDI Rx Channel").unwrap();
    assert_eq!(og_engine::devices::get(&system, rx), Value::Number(1), "MIDI Rx Channel");
    let backup = h.app.setlist.g3s.as_ref().unwrap().backup.clone();
    assert_eq!((backup.banks.len(), backup.bank_name(1).as_str(), backup.banks[1].presets[1].name().as_str()), (2, "COMFORT.|NUMB", "INTRO"));
    h.settle();
    // Exported: the memory as a script that makes it again; the G3S too; the guide.
    let path = dir.join("export.json");
    let action = og_engine::script::export_patch(&description, 149, &raw);
    h.app.write_ai_script(vec![action], &path);
    let exported = og_engine::script::Script::parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(og_engine::script::patch_from(&description, &exported.actions[0], None).unwrap(), raw);
    assert_eq!(og_engine::script::export_g3s(&backup).len(), 3);
    assert!(crate::script::guide_text().contains("key `rv500`"));
    h.app.close_ai_script();
    assert!(h.app.setlist.script.is_none() && h.app.setlist.page == Page::Songs);
    h.settle();
    let _ = std::fs::remove_dir_all(dir);
}

/// One HTTP request to the API, on a thread of its own; the window's
/// thread is polled meanwhile, as the program does each frame.
fn http(h: &mut Harness, port: u16, method: &str, path: &str, token: Option<&str>, body: &str) -> (u16, String) {
    use std::io::{Read, Write};
    let what = format!("{method} {path}");
    let (method, path, token, body) = (method.to_owned(), path.to_owned(), token.map(str::to_owned), body.to_owned());
    let call = std::thread::spawn(move || {
        let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).expect("connect");
        let auth = token.map_or(String::new(), |t| format!("Authorization: Bearer {t}\r\n"));
        let request = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\n{auth}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len());
        stream.write_all(request.as_bytes()).expect("write");
        let mut answer = Vec::new();
        stream.read_to_end(&mut answer).expect("read");
        let answer = String::from_utf8_lossy(&answer).into_owned();
        let status: u16 = answer.split_whitespace().nth(1).and_then(|s| s.parse().ok()).expect("a status");
        let body = answer.split_once("\r\n\r\n").map_or(String::new(), |(_, b)| b.to_owned());
        (status, body)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(40);
    while !call.is_finished() {
        assert!(std::time::Instant::now() < deadline, "the API did not answer {what}");
        h.app.poll_set_list();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    call.join().expect("the request")
}

#[test]
fn the_local_api_reads_and_writes_the_made_up_pedal_and_the_g3s_with_a_token() {
    use og_engine::roland::{fake, Head};
    let dir = folder("api");
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    h.app.open_set_list();
    h.app.settings.set_list.devices = vec![device("Reverb", 1), device("Switcher", 4)];
    h.app.settings.set_list.devices[0].model = "rv500".to_owned();
    h.app.settings.set_list.devices[1].model = "g3s".to_owned();
    let ctx = eframe::egui::Context::default();
    let port = h.app.start_api_on_any_port(&ctx);
    let token = h.app.settings.set_list.api_token.clone();
    assert_eq!(token.len(), 32);
    // The token is asked; the root says what runs; a wrong route is a 404.
    assert_eq!(http(&mut h, port, "GET", "/", None, "").0, 401);
    assert_eq!(http(&mut h, port, "GET", "/", Some("nope"), "").0, 401);
    let (status, body) = http(&mut h, port, "GET", "/", Some(&token), "");
    assert_eq!(status, 200);
    assert!(body.contains("\"rv500\"") && body.contains("\"Reverb\""), "{body}");
    assert_eq!(http(&mut h, port, "GET", "/nowhere", Some(&token), "").0, 404);
    let (status, body) = http(&mut h, port, "GET", "/guide", Some(&token), "");
    assert!(status == 200 && body.contains("key `rv500`"));
    let (status, body) = http(&mut h, port, "GET", "/pedals/rv500", Some(&token), "");
    assert!(status == 200 && body.contains("\"Reverb : MODE\"") && body.contains("\"HALL\""), "{body}");
    assert_eq!(http(&mut h, port, "GET", "/pedals/md500", Some(&token), "").0, 404);
    assert_eq!(http(&mut h, port, "GET", "/pedals/dd500/memories/01A", Some(&token), "").0, 404, "no device of that model");
    // The pedal: its module open on the made-up one (the real ports are not there).
    let description = crate::pedal::description_of("rv500").unwrap();
    let head = Head { device_id: description.device_id, model_id: description.model_id };
    h.app.open_pedal_on(0, Box::new(fake::Device::new(head, [0x42, 3])), &ctx);
    h.app.setlist.page = Page::Songs;
    settle_pedal(&mut h);
    let (status, body) = http(&mut h, port, "GET", "/pedals/rv500/memories/50C", Some(&token), "");
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"memory\":\"50C\"") && body.contains("\"Reverb : MODE\""), "{body}");
    // A write asks the user: refused, then allowed unattended.
    let put = r#"{ "name": "COMF NUMB", "from": "init", "set": { "Reverb : MODE": "HALL", "Reverb : PRE-DELAY": 40 } }"#;
    let (status, body) = {
        let (method, path, body, t) = ("PUT", "/pedals/rv500/memories/50C", put.to_owned(), token.clone());
        let call = std::thread::spawn(move || {
            use std::io::{Read, Write};
            let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
            stream.write_all(format!("{method} {path} HTTP/1.1\r\nAuthorization: Bearer {t}\r\nContent-Length: {}\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
            let mut answer = String::new();
            stream.read_to_string(&mut answer).unwrap();
            (answer.split_whitespace().nth(1).unwrap().parse::<u16>().unwrap(), answer)
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while h.app.api_question().is_none() {
            assert!(std::time::Instant::now() < deadline, "no question asked");
            h.app.poll_set_list();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(h.app.api_question().unwrap().contains("50C"), "{:?}", h.app.api_question());
        h.settle();
        h.app.api_decide(false);
        while !call.is_finished() {
            h.app.poll_set_list();
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        call.join().unwrap()
    };
    assert_eq!(status, 403, "{body}");
    h.app.settings.set_list.api_unattended = true;
    let (status, body) = http(&mut h, port, "PUT", "/pedals/rv500/memories/50C", Some(&token), put);
    assert_eq!(status, 200, "{body}");
    let (status, body) = http(&mut h, port, "GET", "/pedals/rv500/memories/50C", Some(&token), "");
    assert!(status == 200 && body.contains("COMF NUMB") && body.contains("\"HALL\""), "{body}");
    assert_eq!(http(&mut h, port, "PUT", "/pedals/rv500/memories/50C", Some(&token), r#"{ "set": { "Reverb : MODE": "CATHEDRAL" } }"#).0, 400);
    let (status, body) = http(&mut h, port, "POST", "/pedals/rv500/try", Some(&token), r#"{ "set": { "Reverb : Effect Level": 20 } }"#);
    assert_eq!(status, 200, "{body}");
    let (status, body) = http(&mut h, port, "PUT", "/pedals/rv500/pc_map", Some(&token), r#"{ "3": "50C" }"#);
    assert_eq!(status, 200, "{body}");
    let (status, body) = http(&mut h, port, "GET", "/pedals/rv500/pc_map", Some(&token), "");
    assert!(status == 200 && body.contains("\"3\":\"50C\""), "{body}");
    let (status, body) = http(&mut h, port, "GET", "/pedals/rv500/system", Some(&token), "");
    assert!(status == 200 && body.contains("\"MIDI Rx Channel\""), "{body}");
    let (status, body) = http(&mut h, port, "GET", "/pedals/rv500/memories", Some(&token), "");
    assert!(status == 200 && body.contains("\"50C\":\"COMF NUMB\""), "{body}");
    // The G3S: actions, then the backup read back and saved.
    assert_eq!(http(&mut h, port, "GET", "/g3s", Some(&token), "").0, 404, "nothing loaded yet");
    let (status, body) = http(&mut h, port, "POST", "/g3s/actions", Some(&token), r#"{ "actions": [ { "g3s": "bank", "bank": 2, "name": "TIME" }, { "g3s": "preset", "bank": 2, "switch": 2, "name": "INTRO", "loops": [12] } ] }"#);
    assert_eq!(status, 200, "{body}");
    let (status, body) = http(&mut h, port, "GET", "/g3s", Some(&token), "");
    assert!(status == 200 && body.contains("\"TIME\"") && body.contains("\"INTRO\""), "{body}");
    assert_eq!(http(&mut h, port, "POST", "/g3s/actions", Some(&token), r#"{ "actions": [ { "g3s": "preset", "bank": 2, "switch": 99 } ] }"#).0, 400);
    assert_eq!(http(&mut h, port, "POST", "/g3s/save", Some(&token), "{}").0, 400, "never saved: a path is needed");
    let path = dir.join("api.g3b").display().to_string().replace('\\', "\\\\");
    let (status, body) = http(&mut h, port, "POST", "/g3s/save", Some(&token), &format!("{{ \"path\": \"{path}\" }}"));
    assert_eq!(status, 200, "{body}");
    assert!(dir.join("api.g3b").is_file());
    // A whole script: refused with its errors, done when sound.
    let (status, body) = http(&mut h, port, "POST", "/actions", Some(&token), r#"{ "actions": [ { "pedal": "rv500", "memory": "99D", "set": {} } ] }"#);
    assert!(status == 400 && body.contains("99D"), "{body}");
    let (status, body) = http(&mut h, port, "POST", "/actions", Some(&token), r#"{ "actions": [ { "pedal": "rv500", "memory": "50C", "set": { "Reverb : Effect Level": 35 } }, { "g3s": "bank", "bank": 3, "name": "MONEY" } ] }"#);
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("\"done\""), "{body}");
    h.settle();
    h.app.close_set_list();
    assert!(h.app.setlist.api.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_ai_script_adds_changes_and_removes_the_songs_of_the_list() {
    let dir = folder("script-songs");
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    h.app.open_set_list();
    h.app.settings.set_list.mode = PilotMode::Chain;
    h.app.settings.set_list.devices = vec![device("DD-500", 0), device("RV-500", 1)];
    h.app.settings.set_list.songs = vec![song(1, "Pink Floyd", "Time")];
    let ctx = eframe::egui::Context::default();
    let text = r#"{ "actions": [
        { "song": "add", "artist": "Pink Floyd", "title": "Comfortably Numb", "display": "COMF NUMB", "messages": [ { "device": "DD-500", "program": 4, "delay_ms": 20 }, { "device": "rv500", "program": 2 } ] },
        { "song": "set", "artist": "Pink Floyd", "title": "Time", "number": 9, "messages": [ { "device": "DD-500", "program": 1 } ] },
        { "song": "add", "artist": "Pink Floyd", "title": "Money", "messages": [ { "device": "Looper", "program": 1 } ] },
        { "song": "remove", "artist": "Nobody", "title": "Nothing" } ] }"#;
    h.app.review_ai_script(text, None, &ctx);
    let s = h.app.setlist.script.as_ref().unwrap();
    assert!(s.summaries[0].1.is_none() && s.summaries[1].1.is_none(), "{:?}", s.summaries);
    assert!(s.summaries[2].1.as_deref().is_some_and(|e| e.contains("\"Looper\"")), "{:?}", s.summaries[2]);
    assert!(s.summaries[3].1.as_deref().is_some_and(|e| e.contains("not in the list")), "{:?}", s.summaries[3]);
    assert_eq!(h.app.settings.set_list.songs.len(), 1, "reviewed, nothing changed yet");
    h.settle();
    // The sound part, applied.
    let good = r#"{ "actions": [
        { "song": "add", "artist": "Pink Floyd", "title": "Comfortably Numb", "display": "COMF NUMB", "messages": [ { "device": "DD-500", "program": 4, "delay_ms": 20 }, { "device": "rv500", "program": 2 } ] },
        { "song": "set", "artist": "Pink Floyd", "title": "Time", "number": 9, "messages": [ { "device": "DD-500", "program": 1 } ] } ] }"#;
    h.app.review_ai_script(good, None, &ctx);
    assert_eq!(h.app.setlist.script.as_ref().unwrap().errors(), 0);
    h.app.apply_ai_script();
    settle_script(&mut h);
    let songs = &h.app.settings.set_list.songs;
    assert_eq!(songs.len(), 2);
    assert_eq!((songs[1].label().as_str(), songs[1].number, songs[1].steps.len(), songs[1].steps[1].device.as_str()), ("COMF NUMB", 2, 2, "RV-500"));
    assert_eq!((songs[0].number, songs[0].steps.len()), (9, 1));
    h.app.choose_set_song(1);
    assert_eq!(h.app.setlist.sent, [vec![0xC0, 4], vec![0xC1, 2]], "the song sends what the AI wrote");
    // Removed, and exported.
    h.app.review_ai_script(r#"{ "actions": [ { "song": "remove", "artist": "pink floyd", "title": "time" } ] }"#, None, &ctx);
    h.app.apply_ai_script();
    settle_script(&mut h);
    assert_eq!(h.app.settings.set_list.songs.len(), 1);
    let exported = og_engine::script::export_songs(&h.app.settings.set_list);
    assert_eq!((exported.len(), exported[0].title.as_deref(), exported[0].messages.as_ref().map(Vec::len)), (1, Some("Comfortably Numb"), Some(2)));
    let port = h.app.start_api_on_any_port(&ctx);
    let token = h.app.settings.set_list.api_token.clone();
    let (status, body) = http(&mut h, port, "GET", "/songs", Some(&token), "");
    assert!(status == 200 && body.contains("\"Comfortably Numb\"") && body.contains("\"chain\""), "{body}");
    let (status, body) = http(&mut h, port, "POST", "/actions", Some(&token), r#"{ "actions": [ { "song": "add", "artist": "Dire Straits", "title": "Sultans of Swing" } ] }"#);
    assert_eq!(status, 200, "{body}");
    assert_eq!(h.app.settings.set_list.songs.len(), 2);
    h.settle();
    h.app.close_set_list();
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_pedalboard_is_described_kept_and_exported_for_an_ai() {
    use og_engine::roland::{fake, Head};
    let dir = folder("pedalboard");
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    h.app.open_set_list();
    h.app.settings.set_list.mode = PilotMode::Chain;
    h.app.settings.set_list.devices = vec![device("G3S", 4), device("DD-500", 0)];
    h.app.settings.set_list.devices[0].model = "g3s".to_owned();
    h.app.settings.set_list.devices[1].model = "dd500".to_owned();
    h.app.settings.set_list.songs = vec![song(2, "Deep Purple", "Smoke on the Water")];
    let ctx = eframe::egui::Context::default();
    // The loops, written by a script, shown in the G3S page.
    h.app.review_ai_script(r#"{ "actions": [ { "g3s": "loops", "pedals": { "1": [ { "name": "Fuzz Face", "kind": "fuzz" }, { "name": "Vox Wah", "kind": "wah" } ], "11": [ { "name": "DD-500", "kind": "delay", "device": "DD-500" } ] }, "signal_order": [1, 8, 10, 6, 2, 4, 3, 5, 7, 9, 11, 12], "bank_switches": [1, 8] } ] }"#, None, &ctx);
    assert_eq!(h.app.setlist.script.as_ref().unwrap().errors(), 0, "{:?}", h.app.setlist.script.as_ref().unwrap().summaries);
    h.app.apply_ai_script();
    settle_script(&mut h);
    let switcher = &h.app.settings.set_list.devices[0];
    assert_eq!((switcher.loop_name(0).as_str(), switcher.loops[10][0].device.as_str(), switcher.bank_switches.as_slice()), ("1 Fuzz Face + Vox Wah", "DD-500", &[1u8, 8][..]));
    h.app.open_g3s(0);
    h.settle();
    h.app.setlist.g3s.as_mut().unwrap().board = true;
    h.settle();
    h.app.setlist.g3s.as_mut().unwrap().board = false;
    // What the pedal module reads is kept for the next session and the pedalboard.
    let description = crate::pedal::description_of("dd500").unwrap();
    let head = Head { device_id: description.device_id, model_id: description.model_id };
    let mut pedal = fake::Device::new(head, [0x4D, 3]);
    let mut raw = description.init_patch();
    og_engine::devices::set(&mut raw, description.name_param(), &og_engine::devices::Value::Text("SLAPBACK".into())).unwrap();
    pedal.fill(description.user_address(241), &raw);
    h.app.open_pedal_on(1, Box::new(pedal), &ctx);
    settle_pedal(&mut h);
    h.app.setlist.pedal.as_mut().unwrap().ask(Job::ReadPatch(241));
    settle_pedal(&mut h);
    let cache = h.app.pedal_cache_path("dd500");
    assert!(cache.is_file(), "{}", cache.display());
    h.app.close_pedal();
    assert_eq!(h.app.known_pedals().len(), 1);
    assert!(h.app.known_pedals()[0].1.patches.contains_key("81B"), "kept after the module is closed");
    h.app.open_pedal_on(1, Box::new(fake::Device::new(head, [0x4D, 3])), &ctx);
    settle_pedal(&mut h);
    assert_eq!(h.app.setlist.pedal.as_ref().unwrap().names[241], "SLAPBACK", "loaded again when the module opens");
    h.app.close_pedal();
    // The pedalboard file, and the same through the API.
    let script = h.app.pedalboard_script();
    let board = script.pedalboard.as_ref().unwrap();
    assert_eq!(board.switcher.as_ref().unwrap().loops["1"][1].name, "Vox Wah");
    let kinds: Vec<og_engine::script::Kind> = script.actions.iter().map(|a| a.kind().unwrap()).collect();
    assert_eq!(kinds, [og_engine::script::Kind::G3sLoops, og_engine::script::Kind::SongAdd, og_engine::script::Kind::G3sBank, og_engine::script::Kind::Patch, og_engine::script::Kind::Patch], "the memory read and the one the pedal played");
    let path = dir.join("pedalboard.json");
    std::fs::write(&path, script.to_json()).unwrap();
    assert!(og_engine::script::Script::parse(&std::fs::read_to_string(&path).unwrap()).unwrap().pedalboard.is_some());
    let port = h.app.start_api_on_any_port(&ctx);
    let token = h.app.settings.set_list.api_token.clone();
    let (status, body) = http(&mut h, port, "GET", "/pedalboard", Some(&token), "");
    assert!(status == 200 && body.contains("\"signal_order\"") && body.contains("SLAPBACK"), "{body}");
    h.settle();
    h.app.close_set_list();
    let _ = std::fs::remove_dir_all(dir);
}
