//! The pedal modules of the MIDI Pilot: a BOSS 500-series pedal configured
//! over MIDI from its description (`og_engine::devices`). A thread of its own
//! talks to the pedal, one job at a time, and owns the MIDI output and input
//! while the module is open; the songs' messages go through it then. Its
//! page is in `ui_pedal`.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use eframe::egui;
use og_engine::devices::{self, Description, PedalFile};
use og_engine::roland::{self, Link, Port};

use crate::app::App;
use crate::files;
use crate::i18n::tr;
use crate::midi::Ports;

/// What the thread is asked to do with the pedal.
#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    Identify,
    /// The names of every memory.
    ReadNames,
    ReadPatch(usize),
    /// The number of the patch played.
    ReadCurrent,
    /// Makes the pedal play a memory.
    SelectPatch(usize),
    /// A whole patch into a slot of the temporary bank: heard, not kept.
    SendTemporary { slot: usize, raw: Vec<u8> },
    /// Some bytes of the patch played: a knob moved.
    SendBytes { slot: usize, ofs: usize, bytes: Vec<u8> },
    /// A patch into a memory: the old one is read first, the new one read
    /// back and compared.
    Store { number: usize, raw: Vec<u8> },
    ReadPcMap,
    WritePcMap(Vec<u16>),
    ReadSystem,
    WriteSystem(Vec<u8>),
    /// Every memory, one after the other.
    ReadAll,
    /// Every patch of a file into its memory, then its map and its system.
    SendAll(PedalFile),
    /// The messages of a song, each followed by its pause in milliseconds.
    Send(Vec<(Vec<u8>, u16)>),
}

/// What the thread answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Names(Vec<String>),
    Patch(usize, Vec<u8>),
    Current(usize),
    PcMap(Vec<u16>),
    System(Vec<u8>),
    /// What a memory held before it was written.
    Backup(usize, Vec<u8>),
    Progress(usize, usize),
    Done(String),
    Failed(String),
}

/// The thread talking to the pedal: what is asked of it, what it answers.
pub type Thread = (mpsc::Sender<Job>, mpsc::Receiver<Event>);

/// Starts the thread on a port; it ends when the jobs' sender is dropped.
/// The pedal is put in its editor mode while the thread lives.
pub fn start(description: Description, port: Box<dyn Port + Send>, ctx: egui::Context) -> Thread {
    let (jobs, asked) = mpsc::channel::<Job>();
    let (events, answers) = mpsc::channel();
    std::thread::spawn(move || {
        let mut port = port;
        let mut link = Link::new(description.head(), port.as_mut());
        let _ = link.write(description.addresses.editor_mode, &[1]);
        for job in asked {
            let outcome = run(&description, &mut link, &job, &events);
            let event = match outcome {
                Ok(text) => Event::Done(text),
                Err(e) => Event::Failed(e),
            };
            if events.send(event).is_err() {
                break;
            }
            ctx.request_repaint();
        }
        let _ = link.write(description.addresses.editor_mode, &[0]);
    });
    (jobs, answers)
}

/// One job, done; what it says when it is.
fn run(d: &Description, link: &mut Link, job: &Job, events: &mpsc::Sender<Event>) -> Result<String, String> {
    let name_param = d.name_param();
    let progress = |done: usize, total: usize| {
        let _ = events.send(Event::Progress(done, total));
    };
    match job {
        Job::Identify => {
            let (maker, family) = link.identify()?;
            let (maker, family) = (format!("{maker:02X}"), format!("{:02X} {:02X}", family[0], family[1]));
            Ok(tf!("The device answers: maker {maker}, family {family}." [maker, family]))
        }
        Job::ReadNames => {
            let mut names = Vec::with_capacity(d.patches());
            for n in 0..d.patches() {
                let raw = link.read(d.user_address(n) + name_param.ofs as u32, name_param.bytes)?;
                let mut whole = vec![0; d.patch_size];
                whole[name_param.ofs..name_param.ofs + name_param.bytes].copy_from_slice(&raw);
                names.push(d.patch_name(&whole));
                if n % 10 == 9 {
                    progress(n + 1, d.patches());
                }
            }
            let _ = events.send(Event::Names(names));
            Ok(tr("The names of the memories are read.").to_owned())
        }
        Job::ReadPatch(n) => {
            let raw = link.read(d.user_address(*n), d.patch_size)?;
            let _ = events.send(Event::Patch(*n, raw));
            Ok(tf!("{} is read.", d.label(*n)))
        }
        Job::ReadCurrent => {
            let raw = link.read(d.addresses.current, 3)?;
            let n = roland::from_nibbles(&raw) as usize;
            let _ = events.send(Event::Current(n));
            Ok(tf!("The pedal plays {}.", d.label(n)))
        }
        Job::SelectPatch(n) => {
            link.write(d.addresses.current, &roland::nibbles(*n as u32, 3))?;
            std::thread::sleep(Duration::from_millis(300));
            Ok(tf!("The pedal plays {}.", d.label(*n)))
        }
        Job::SendTemporary { slot, raw } => {
            link.write(d.temporary_address(*slot), raw)?;
            Ok(tr("The patch is sent to the pedal, to be heard; Store keeps it.").to_owned())
        }
        Job::SendBytes { slot, ofs, bytes } => {
            link.write(d.temporary_address(*slot) + *ofs as u32, bytes)?;
            Ok(String::new())
        }
        Job::Store { number, raw } => {
            let old = link.read(d.user_address(*number), d.patch_size)?;
            let _ = events.send(Event::Backup(*number, old));
            link.write(d.user_address(*number), raw)?;
            std::thread::sleep(Duration::from_secs(1));
            let check = link.read(d.user_address(*number), d.patch_size)?;
            if &check != raw {
                let differ = check.iter().zip(raw).filter(|(a, b)| a != b).count();
                return Err(tf!("{} read back differs in {differ} bytes." [differ], d.label(*number)));
            }
            let _ = events.send(Event::Patch(*number, check));
            Ok(tf!("{} is stored in the pedal, and read back the same.", d.label(*number)))
        }
        Job::ReadPcMap => {
            let raw = link.read(d.addresses.pc_map, d.pc_map_entries * 3)?;
            let map: Vec<u16> = raw.chunks(3).map(|c| roland::from_nibbles(c) as u16).collect();
            let _ = events.send(Event::PcMap(map));
            Ok(tr("The program change map is read.").to_owned())
        }
        Job::WritePcMap(map) => {
            let bytes: Vec<u8> = map.iter().flat_map(|n| roland::nibbles(u32::from(*n), 3)).collect();
            link.write(d.addresses.pc_map, &bytes)?;
            std::thread::sleep(Duration::from_millis(500));
            let check = link.read(d.addresses.pc_map, bytes.len())?;
            if check != bytes {
                return Err(tr("The program change map read back differs.").to_owned());
            }
            Ok(tr("The program change map is written in the pedal.").to_owned())
        }
        Job::ReadSystem => {
            let raw = link.read(d.addresses.system, d.system_size())?;
            let _ = events.send(Event::System(raw));
            Ok(tr("The system settings are read.").to_owned())
        }
        Job::WriteSystem(raw) => {
            link.write(d.addresses.system, raw)?;
            std::thread::sleep(Duration::from_millis(500));
            Ok(tr("The system settings are written in the pedal.").to_owned())
        }
        Job::ReadAll => {
            for n in 0..d.patches() {
                let raw = link.read(d.user_address(n), d.patch_size)?;
                let _ = events.send(Event::Patch(n, raw));
                progress(n + 1, d.patches());
            }
            Ok(tf!("{} memories are read.", d.patches()))
        }
        Job::SendAll(file) => {
            let total = file.patches.len() + usize::from(!file.pc_map.is_empty()) + usize::from(!file.system.is_empty());
            let mut done = 0;
            for label in file.patches.keys() {
                let number = d.number(label).ok_or_else(|| tf!("{label} is no memory of the {}" [label], d.name))?;
                let raw = file.patch(d, number).ok_or_else(|| tf!("the patch {label} of the file is not whole" [label]))?;
                link.write(d.user_address(number), &raw)?;
                std::thread::sleep(Duration::from_millis(200));
                let _ = events.send(Event::Patch(number, raw));
                done += 1;
                progress(done, total);
            }
            if !file.pc_map.is_empty() {
                let bytes: Vec<u8> = file.pc_map.iter().flat_map(|n| roland::nibbles(u32::from(*n), 3)).collect();
                link.write(d.addresses.pc_map, &bytes)?;
                done += 1;
                progress(done, total);
            }
            if let Some(system) = devices::from_hex(&file.system).filter(|s| !s.is_empty()) {
                link.write(d.addresses.system, &system)?;
                done += 1;
                progress(done, total);
            }
            Ok(tf!("{done} of the file are written in the pedal." [done]))
        }
        Job::Send(messages) => {
            for (bytes, pause) in messages {
                link.port.send(bytes)?;
                let pause = u64::from((*pause).min(500));
                if pause > 0 {
                    std::thread::sleep(Duration::from_millis(pause));
                }
            }
            Ok(tf!("{} messages sent.", messages.len()))
        }
    }
}

/// A part of the page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Patches,
    PcMap,
    System,
    File,
}

/// Something the user is asked to confirm before the pedal is written.
#[derive(Clone, Debug, PartialEq)]
pub enum Confirm {
    Store(usize),
    SendAll,
    WritePcMap,
    WriteSystem,
}

/// A pedal module open: its thread, and what is known of the pedal.
pub struct Session {
    /// The device of the chain it is for.
    pub device: usize,
    pub description: Description,
    pub thread: Thread,
    /// Jobs asked and not answered yet.
    pub pending: usize,
    /// The names of the memories, once read.
    pub names: Vec<String>,
    /// The memory chosen, and its patch being edited (from the pedal or a file).
    pub selected: Option<usize>,
    pub raw: Option<Vec<u8>>,
    /// The patch edited differs from the memory.
    pub edited: bool,
    pub pc_map: Vec<u16>,
    pub system: Vec<u8>,
    /// The patches known, saved and loaded as one file.
    pub file: PedalFile,
    pub file_path: Option<PathBuf>,
    pub tab: Tab,
    pub message: String,
    pub progress: Option<(usize, usize)>,
    pub confirm: Option<Confirm>,
    /// What memories held before they were written, kept in the folder of the list.
    pub backups: Vec<PathBuf>,
    /// What came of the last job, for whoever asked it (the scripts, the API).
    pub outcome: Option<Result<String, String>>,
}

impl Session {
    /// Asks the thread for a job.
    pub fn ask(&mut self, job: Job) {
        if self.thread.0.send(job).is_ok() {
            self.pending += 1;
        }
    }

    /// The slot of the memory chosen, where the pedal plays it.
    pub fn slot(&self) -> usize {
        self.selected.map_or(0, |n| n % self.description.slots)
    }

    /// Whether the pedal is being talked to.
    pub fn busy(&self) -> bool {
        self.pending > 0
    }
}

/// The descriptions known: those shipped, then those of the `devices`
/// folder next to the program.
pub fn descriptions() -> Vec<Description> {
    let mut known = Description::built_in();
    let folder = crate::app::beside_program("devices");
    let mut files: Vec<PathBuf> = folder
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
        .collect();
    files.sort();
    for path in files.into_iter().take(32) {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(description) = Description::parse(&text) {
                known.retain(|k| k.key != description.key);
                known.push(description);
            }
        }
    }
    known
}

/// The description of a model, when the program has one.
pub fn description_of(model: &str) -> Option<Description> {
    (!model.is_empty()).then(|| descriptions().into_iter().find(|d| d.key == model)).flatten()
}

impl App {
    /// Opens the module of a device of the chain on the MIDI ports chosen:
    /// the songs' output is handed to the module's thread meanwhile.
    pub fn open_pedal(&mut self, device: usize, ctx: &egui::Context) {
        let list = &self.settings.set_list;
        let (output, input) = (list.midi_output.clone(), list.midi_input.clone());
        // The output is opened once on Windows: the songs let go of it.
        self.setlist.output = None;
        match Ports::open(&output, &input) {
            Ok(ports) => self.open_pedal_on(device, Box::new(ports), ctx),
            Err(e) => {
                self.setlist.message = tf!("The module was not opened: {e}" [e]);
                let name = output;
                if !name.is_empty() {
                    self.connect_set_list_output(&name);
                }
            }
        }
    }

    /// Opens the module of a device on a port given (the tests' made-up pedal).
    pub fn open_pedal_on(&mut self, device: usize, port: Box<dyn Port + Send>, ctx: &egui::Context) {
        let Some(model) = self.settings.set_list.devices.get(device).map(|d| d.model.clone()) else { return };
        let Some(description) = description_of(&model) else {
            self.setlist.message = tr("This device has no module: give it a model under Devices.").to_owned();
            return;
        };
        if !self.pilot_only {
            // The keyboard's input is let go of: the pedal may answer on it.
            self.audio.disconnect_midi();
        }
        let thread = start(description.clone(), port, ctx.clone());
        let file = PedalFile::new(&description.key);
        let mut session = Session {
            device,
            description,
            thread,
            pending: 0,
            names: Vec::new(),
            selected: None,
            raw: None,
            edited: false,
            pc_map: Vec::new(),
            system: Vec::new(),
            file,
            file_path: None,
            tab: Tab::Patches,
            message: String::new(),
            progress: None,
            confirm: None,
            backups: Vec::new(),
            outcome: None,
        };
        // What was read or written in earlier sessions, kept in the program's data.
        if let Ok(text) = std::fs::read_to_string(self.pedal_cache_path(&session.description.key)) {
            if let Ok(file) = PedalFile::from_json(&text, &session.description) {
                if !file.pc_map.is_empty() {
                    session.pc_map.clone_from(&file.pc_map);
                }
                if let Some(system) = devices::from_hex(&file.system).filter(|s| !s.is_empty()) {
                    session.system = system;
                }
                session.names.resize(session.description.patches(), String::new());
                for (label, entry) in &file.patches {
                    if let Some(n) = session.description.number(label) {
                        session.names[n].clone_from(&entry.name);
                    }
                }
                session.file = file;
            }
        }
        session.ask(Job::Identify);
        session.ask(Job::ReadCurrent);
        self.setlist.pedal = Some(session);
        self.setlist.page = crate::setlist::Page::Pedal(device);
    }

    /// Where what is known of a pedal is kept: `Pedals\<key>.pedal.json` in the program's data.
    pub fn pedal_cache_path(&self, key: &str) -> PathBuf {
        self.data_dir().join("Pedals").join(format!("{key}.pedal.json"))
    }

    /// Keeps what the module knows of the pedal for the next sessions and
    /// the pedalboard file.
    pub fn save_pedal_cache(&mut self) {
        let Some(session) = &mut self.setlist.pedal else { return };
        if !session.pc_map.is_empty() {
            session.file.pc_map.clone_from(&session.pc_map);
        }
        if !session.system.is_empty() {
            session.file.system = devices::hex(&session.system);
        }
        let (text, key) = (session.file.to_json(), session.description.key.clone());
        let path = self.pedal_cache_path(&key);
        let _ = path.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(path, text);
    }

    /// What is known of every pedal of the chain, from the module open or the data kept.
    pub fn known_pedals(&self) -> Vec<(Description, PedalFile)> {
        let mut known = Vec::new();
        for device in &self.settings.set_list.devices {
            let Some(description) = description_of(&device.model) else { continue };
            if known.iter().any(|(d, _): &(Description, PedalFile)| d.key == description.key) {
                continue;
            }
            let file = match &self.setlist.pedal {
                Some(session) if session.description.key == description.key => Some(session.file.clone()),
                _ => std::fs::read_to_string(self.pedal_cache_path(&description.key)).ok().and_then(|text| PedalFile::from_json(&text, &description).ok()),
            };
            if let Some(file) = file {
                known.push((description, file));
            }
        }
        known
    }

    /// Closes the module: its thread ends, the songs get their output back.
    pub fn close_pedal(&mut self) {
        if self.setlist.pedal.take().is_none() {
            return;
        }
        self.setlist.page = crate::setlist::Page::Devices;
        let name = self.settings.set_list.midi_output.clone();
        if !name.is_empty() && self.setlist.open {
            // The thread lets go of the port once it ends.
            std::thread::sleep(Duration::from_millis(50));
            self.connect_set_list_output(&name);
        }
        if !self.pilot_only {
            self.audio.connect_midi();
        }
    }

    /// Takes in what the pedal's thread answered since the last frame.
    pub fn poll_pedal(&mut self) {
        let backups = self.set_list_folder_for_backups();
        let Some(session) = &mut self.setlist.pedal else { return };
        let came: Vec<Event> = session.thread.1.try_iter().collect();
        let mut changed = false;
        for event in came {
            if matches!(event, Event::Names(_) | Event::Patch(..) | Event::PcMap(_) | Event::System(_)) {
                changed = true;
            }
            match event {
                Event::Names(names) => session.names = names,
                Event::Patch(n, raw) => {
                    if session.names.len() < session.description.patches() {
                        session.names.resize(session.description.patches(), String::new());
                    }
                    session.names[n] = session.description.patch_name(&raw);
                    session.file.put(&session.description, n, &raw);
                    if session.selected == Some(n) {
                        session.raw = Some(raw);
                        session.edited = false;
                    }
                }
                Event::Current(n) => {
                    if session.selected.is_none() {
                        session.selected = Some(n);
                        session.ask(Job::ReadPatch(n));
                    }
                }
                Event::PcMap(map) => session.pc_map = map,
                Event::System(raw) => session.system = raw,
                Event::Backup(n, old) => {
                    let folder = backups.clone();
                    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
                    let path = folder.join(format!("{}-{}-{stamp}.pedal.json", session.description.key, session.description.label(n)));
                    let mut file = PedalFile::new(&session.description.key);
                    file.put(&session.description, n, &old);
                    if std::fs::create_dir_all(&folder).and_then(|()| std::fs::write(&path, file.to_json())).is_ok() {
                        session.backups.push(path);
                    }
                }
                Event::Progress(done, total) => session.progress = Some((done, total)),
                Event::Done(text) => {
                    session.pending = session.pending.saturating_sub(1);
                    session.progress = None;
                    if !text.is_empty() {
                        session.message.clone_from(&text);
                    }
                    session.outcome = Some(Ok(text));
                }
                Event::Failed(text) => {
                    session.pending = session.pending.saturating_sub(1);
                    session.progress = None;
                    session.message = tf!("Failed: {text}" [text]);
                    session.outcome = Some(Err(text));
                }
            }
        }
        if changed {
            self.save_pedal_cache();
        }
    }

    /// Where the old contents of the memories written are kept.
    fn set_list_folder_for_backups(&self) -> PathBuf {
        self.set_list_folder().join("Pedal backups")
    }

    /// A parameter of the patch edited is changed: written in the patch, and
    /// sent to the pedal, which plays it.
    pub fn set_pedal_value(&mut self, param: &devices::Param, value: &devices::Value) {
        let Some(session) = &mut self.setlist.pedal else { return };
        let Some(raw) = &mut session.raw else { return };
        if devices::set(raw, param, value).is_ok() {
            let bytes = raw[param.ofs..param.ofs + param.bytes].to_vec();
            session.edited = true;
            let slot = session.slot();
            session.ask(Job::SendBytes { slot, ofs: param.ofs, bytes });
        }
    }

    /// The patch edited becomes `raw`: sent whole to the pedal, to be heard.
    pub fn replace_pedal_patch(&mut self, raw: Vec<u8>) {
        let Some(session) = &mut self.setlist.pedal else { return };
        let slot = session.slot();
        session.raw = Some(raw.clone());
        session.edited = true;
        session.ask(Job::SendTemporary { slot, raw });
    }

    /// Chooses a memory: the pedal plays it, and its patch is read.
    pub fn select_pedal_patch(&mut self, number: usize) {
        let Some(session) = &mut self.setlist.pedal else { return };
        session.selected = Some(number);
        session.raw = None;
        session.edited = false;
        session.confirm = None;
        session.ask(Job::SelectPatch(number));
        session.ask(Job::ReadPatch(number));
    }

    /// Saves the patches known as a file of the program's.
    pub fn save_pedal_file(&mut self) {
        let Some(session) = &self.setlist.pedal else { return };
        let name = format!("{}.pedal.json", session.description.key);
        let Some(path) = files::save_file(tr("Pedal file"), &["json"], &name) else { return };
        self.write_pedal_file(&path);
    }

    pub fn write_pedal_file(&mut self, path: &std::path::Path) {
        let Some(session) = &mut self.setlist.pedal else { return };
        if !session.pc_map.is_empty() {
            session.file.pc_map.clone_from(&session.pc_map);
        }
        if !session.system.is_empty() {
            session.file.system = devices::hex(&session.system);
        }
        session.message = match std::fs::write(path, session.file.to_json()) {
            Ok(()) => {
                session.file_path = Some(path.to_path_buf());
                tf!("{} patches are saved in {}.", session.file.patches.len(), path.display())
            }
            Err(e) => tf!("The file was not written: {e}" [e]),
        };
    }

    /// Loads a file of the program's, or a backup of the maker's editor.
    pub fn load_pedal_file(&mut self) {
        let Some(path) = files::open_file(tr("Pedal file"), &["json", "bel"]) else { return };
        match std::fs::read_to_string(&path) {
            Ok(text) => self.read_pedal_file(&text, Some(path)),
            Err(e) => {
                if let Some(session) = &mut self.setlist.pedal {
                    session.message = tf!("The file was not read: {e}" [e]);
                }
            }
        }
    }

    pub fn read_pedal_file(&mut self, text: &str, path: Option<PathBuf>) {
        let Some(session) = &mut self.setlist.pedal else { return };
        match PedalFile::from_json(text, &session.description) {
            Ok(file) => {
                let count = file.patches.len();
                session.names.resize(session.description.patches(), String::new());
                for (label, entry) in &file.patches {
                    if let Some(n) = session.description.number(label) {
                        session.names[n].clone_from(&entry.name);
                    }
                }
                if !file.pc_map.is_empty() {
                    session.pc_map.clone_from(&file.pc_map);
                }
                if let Some(system) = devices::from_hex(&file.system).filter(|s| !s.is_empty()) {
                    session.system = system;
                }
                session.file = file;
                session.file_path = path;
                session.message = tf!("{count} patches read from the file; the pedal is not written until you ask." [count]);
            }
            Err(e) => session.message = tf!("The file was not read: {e}" [e]),
        }
        self.save_pedal_cache();
    }
}
