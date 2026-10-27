//! The set list (the MIDI Pilot): the songs of a concert or a rehearsal, each
//! sending by MIDI, when it is chosen, the number of its preset to a pedal
//! switcher (the switcher's mode) or its own messages to the devices of the
//! chain (the chain's mode), and opening its project. Its window is in
//! `ui_setlist`.

use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use eframe::egui;
use og_engine::setlist::{self, CsvError, PilotMode, SendError, SetSong, StepMessage};

use crate::app::{App, Dialog};
use crate::files;
use crate::i18n::tr;
use crate::midi::{midi_inputs, midi_outputs, MidiOut};
use crate::pedal::{Job, Session};

// ----- pictures ----------------------------------------------------------------------

/// The picture of a button: a file of the user's, or one looked for on Deezer.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Cover {
    File(PathBuf),
    Song { artist: String, title: String },
    Artist(String),
}

/// The thread of the pictures: what is asked of it, and what it answers.
type CoverThread = (mpsc::Sender<Cover>, mpsc::Receiver<(Cover, Option<egui::ColorImage>)>);

enum State {
    Waiting,
    Ready(egui::TextureHandle),
    Missing,
}

/// The pictures of the buttons, read or fetched one at a time by a thread of
/// their own, and those fetched kept on the disk: once seen, a picture shows
/// without the Internet (on stage).
#[derive(Default)]
pub struct Covers {
    /// Where the pictures fetched are kept.
    pub cache: PathBuf,
    states: HashMap<Cover, State>,
    thread: Option<CoverThread>,
}

impl Covers {
    /// Takes in the pictures that came since the last frame.
    pub fn receive(&mut self, ctx: &egui::Context) {
        let Some((_, answers)) = &self.thread else { return };
        let came: Vec<_> = answers.try_iter().collect();
        for (cover, image) in came {
            let state = match image {
                Some(image) => State::Ready(ctx.load_texture(format!("cover {cover:?}"), image, egui::TextureOptions::LINEAR)),
                None => State::Missing,
            };
            self.states.insert(cover, state);
        }
    }

    /// The picture, when it is there; otherwise it is asked for.
    pub fn texture(&mut self, ctx: &egui::Context, cover: &Cover) -> Option<egui::load::SizedTexture> {
        match self.states.get(cover) {
            Some(State::Ready(texture)) => Some(egui::load::SizedTexture::from_handle(texture)),
            Some(State::Waiting | State::Missing) => None,
            None => {
                self.states.insert(cover.clone(), State::Waiting);
                let (asks, _) = self.thread.get_or_insert_with(|| start(ctx.clone(), self.cache.clone()));
                let _ = asks.send(cover.clone());
                None
            }
        }
    }

    /// Forgets a picture, for it to be read again (a file of the user's, changed).
    pub fn forget(&mut self, cover: &Cover) {
        self.states.remove(cover);
    }

    /// Empties the pictures kept: they are looked for again.
    pub fn clear(&mut self) -> Result<(), String> {
        if self.cache.is_dir() {
            std::fs::remove_dir_all(&self.cache).map_err(|e| e.to_string())?;
        }
        self.states.clear();
        Ok(())
    }
}

/// The thread that reads and fetches the pictures, one at a time, which spares Deezer.
fn start(ctx: egui::Context, cache: PathBuf) -> CoverThread {
    let (asks, asked) = mpsc::channel::<Cover>();
    let (answer, answers) = mpsc::channel();
    std::thread::spawn(move || {
        for cover in asked {
            let image = read_cover(&cover, &cache);
            if answer.send((cover, image)).is_err() {
                break;
            }
            ctx.request_repaint();
        }
    });
    (asks, answers)
}

/// A name for a picture kept, the same from one version of the program to the next.
fn cache_name(cover: &Cover) -> String {
    format!("{:016x}", crate::sessions::hash64(&format!("{cover:?}")))
}

fn read_cover(cover: &Cover, cache: &Path) -> Option<egui::ColorImage> {
    let bytes = match cover {
        Cover::File(path) => std::fs::read(path).ok()?,
        _ => {
            let base = cache.join(cache_name(cover));
            let (file, none) = (base.with_extension("img"), base.with_extension("none"));
            if let Ok(bytes) = std::fs::read(&file) {
                bytes
            } else if none.exists() || cfg!(test) {
                return None;
            } else {
                let _ = std::fs::create_dir_all(cache);
                match fetch_cover(cover) {
                    Ok(Some(bytes)) => {
                        let _ = std::fs::write(&file, &bytes);
                        bytes
                    }
                    // Nothing found: not asked again.
                    Ok(None) => {
                        let _ = std::fs::write(&none, "");
                        return None;
                    }
                    // No network: asked again at the next start.
                    Err(()) => return None,
                }
            }
        }
    };
    let image = image::load_from_memory(&bytes).ok()?.thumbnail(500, 500).to_rgba8();
    Some(egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw()))
}

/// The cover of a song's album or the photograph of an artist, from Deezer's
/// public search (no account needed): `Ok(None)` when it found none.
fn fetch_cover(cover: &Cover) -> Result<Option<Vec<u8>>, ()> {
    let agent = crate::downloads::agent(15).map_err(|_| ())?;
    let (path, query, field): (&str, String, &[&str]) = match cover {
        Cover::Song { artist, title } => ("search", format!("{artist} {title}"), &["album", "cover_big"]),
        Cover::Artist(artist) => ("search/artist", artist.clone(), &["picture_big"]),
        Cover::File(_) => return Ok(None),
    };
    let text = agent.get(&format!("https://api.deezer.com/{path}")).query("q", &query).query("limit", "1").call().map_err(|_| ())?.into_string().map_err(|_| ())?;
    let json: serde_json::Value = serde_json::from_str(&text).map_err(|_| ())?;
    // An error (too many questions) is not "nothing found".
    if json.get("error").is_some() {
        return Err(());
    }
    let mut value = &json["data"][0];
    for name in field {
        value = &value[*name];
    }
    let Some(url) = value.as_str().filter(|url| url.starts_with("https://")) else { return Ok(None) };
    let mut bytes = Vec::new();
    agent.get(url).call().map_err(|_| ())?.into_reader().take(10_000_000).read_to_end(&mut bytes).map_err(|_| ())?;
    Ok(Some(bytes))
}

/// A dark colour of its own for each artist, for the buttons without a
/// picture: the hues go round by the golden angle, so that neighbours differ.
pub fn artist_colour(rank: usize) -> egui::Color32 {
    let hue = (rank as f32 * 0.381_966).fract();
    egui::ecolor::HsvaGamma { h: hue, s: 0.6, v: 0.42, a: 1.0 }.into()
}

// ----- the state of the window ----------------------------------------------------------

/// What the middle of the window shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    /// The songs' buttons.
    #[default]
    Songs,
    /// The devices of the chain.
    Devices,
    /// The module of a device of the chain, by its place.
    Pedal(usize),
    /// The module of the G3S switcher: a backup file edited.
    G3s,
    /// A script of an AI's, reviewed and applied.
    Script,
}

/// How a sending went.
#[derive(Debug, PartialEq)]
pub enum Sending {
    Done,
    /// There is no MIDI output: the messages were only kept.
    NoOutput,
    Failed(String),
}

/// The set list as the window shows it; the list itself is in the settings.
#[derive(Default)]
pub struct SetListUi {
    pub open: bool,
    pub page: Page,
    /// The MIDI messages of the last sending, output or not, for the window to show.
    pub sent: Vec<Vec<u8>>,
    /// A device whose name is being written: its place and its name before.
    pub renaming: Option<(usize, String)>,
    /// The song chosen last.
    pub active: Option<usize>,
    /// In the order by artist: the artist whose songs are shown.
    pub artist: Option<String>,
    pub output: Option<MidiOut>,
    pub outputs: Vec<String>,
    pub inputs: Vec<String>,
    /// A pedal module open: its thread owns the ports meanwhile.
    pub pedal: Option<Session>,
    /// The G3S module open.
    pub g3s: Option<crate::g3s::G3sUi>,
    /// An AI script open.
    pub script: Option<crate::script::ScriptUi>,
    /// The local API, while it listens.
    pub api: Option<crate::api::Server>,
    /// A request of the API not answered yet.
    pub api_pending: Option<crate::api::Pending>,
    /// The window of the API's settings is shown.
    pub api_window: bool,
    pub covers: Covers,
    pub message: String,
    /// A song being written: its place (none for a new one) and its fields.
    pub editing: Option<(Option<usize>, SetSong)>,
    /// The song whose project the window of the tablature sites is making.
    pub making: Option<usize>,
    /// The song whose project was made: it is saved once its words and its
    /// recording, looked for in the background, have come.
    pub saving: Option<usize>,
    /// The songs whose projects are still to be found, the whole list being prepared.
    pub queue: VecDeque<usize>,
}

/// A file name of a song's own, without the signs Windows refuses.
fn file_name(song: &SetSong) -> String {
    let name: String = song.label().chars().map(|c| if "<>:\"/\\|?*".contains(c) || c.is_control() { '_' } else { c }).collect();
    let name = name.trim().trim_end_matches('.').to_owned();
    if name.is_empty() { format!("{}", song.number) } else { name }
}

impl App {
    /// Opens the window of the set list, and the MIDI output it sends to.
    pub fn open_set_list(&mut self) {
        self.setlist.open = true;
        self.setlist.outputs = if cfg!(test) { Vec::new() } else { midi_outputs() };
        self.setlist.inputs = if cfg!(test) { Vec::new() } else { midi_inputs() };
        if self.setlist.output.is_none() && !self.settings.set_list.midi_output.is_empty() {
            let name = self.settings.set_list.midi_output.clone();
            self.connect_set_list_output(&name);
        }
        if self.setlist.message.is_empty() {
            self.setlist.message = if self.settings.set_list.songs.is_empty() {
                tr("List > Import a CSV file… or Add a song… to make the list.").to_owned()
            } else {
                let count = self.settings.set_list.songs.len();
                tf!("{count} songs." [count])
            };
        }
    }

    /// Closes the window, and the MIDI output, for other programs to use it.
    pub fn close_set_list(&mut self) {
        self.setlist.open = false;
        self.close_pedal();
        self.setlist.g3s = None;
        self.setlist.script = None;
        self.setlist.api = None;
        self.setlist.api_pending = None;
        self.setlist.output = None;
        self.setlist.editing = None;
        self.setlist.page = Page::Songs;
    }

    /// The pedal modules read the pedals' answers on this input from now on.
    pub fn choose_set_list_input(&mut self, name: &str) {
        self.settings.set_list.midi_input = name.to_owned();
        self.save_settings();
    }

    /// Sends the set list's messages to this output from now on; an empty name for none.
    pub fn connect_set_list_output(&mut self, name: &str) {
        // An output is opened once on Windows: the one in use is closed first.
        self.setlist.output = None;
        self.settings.set_list.midi_output = name.to_owned();
        self.save_settings();
        if name.is_empty() {
            self.setlist.message = tr("No MIDI output: the songs send nothing.").to_owned();
            return;
        }
        self.setlist.message = match MidiOut::open(name) {
            Ok(output) => {
                self.setlist.output = Some(output);
                tf!("Connected to the MIDI output “{name}”." [name])
            }
            Err(e) => e,
        };
    }

    /// Reads a CSV file into the set list.
    pub fn import_set_list(&mut self) {
        let Some(path) = files::open_file(tr("CSV file"), &["csv", "txt"]) else { return };
        match std::fs::read(&path) {
            Ok(bytes) => {
                let file = path.file_name().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
                self.set_list_from_csv(&String::from_utf8_lossy(&bytes), &file);
            }
            Err(e) => self.setlist.message = tf!("The file was not read: {e}" [e]),
        }
    }

    /// Replaces the set list with the songs of a CSV text. A song that was in
    /// the list already keeps its project and its picture.
    pub fn set_list_from_csv(&mut self, text: &str, file: &str) {
        let mut songs = match setlist::parse_csv(text) {
            Ok(songs) => songs,
            Err(CsvError::Number { line, text }) => {
                self.setlist.message = tf!("Line {line} of “{file}”: “{text}” is not a number from 0 to 128." [line, text, file]);
                return;
            }
            Err(CsvError::Empty { line }) => {
                self.setlist.message = tf!("Line {line} of “{file}” has no artist, no song and no text." [line, file]);
                return;
            }
            Err(CsvError::Program { line, device, text }) => {
                self.setlist.message = tf!("Line {line} of “{file}”: “{text}” is not a program from 0 to 127 for {device}." [line, file, text, device]);
                return;
            }
        };
        let old = std::mem::take(&mut self.settings.set_list.songs);
        for song in &mut songs {
            if let Some(known) = old.iter().find(|o| setlist::same(&o.artist, &song.artist) && setlist::same(&o.title, &song.title)) {
                song.project.clone_from(&known.project);
                song.image.clone_from(&known.image);
            }
        }
        let count = songs.len();
        // The devices the file names join the chain; a file with programs
        // for them is for the chain's mode.
        let mut added = 0;
        for name in setlist::devices_named(&songs) {
            if self.settings.set_list.device(&name).is_none() {
                let channel = self.settings.set_list.channel;
                self.settings.set_list.devices.push(setlist::Device { name, model: String::new(), channel, ..setlist::Device::default() });
                added += 1;
            }
        }
        let chained = songs.iter().any(|song| !song.steps.is_empty());
        if chained {
            self.settings.set_list.mode = PilotMode::Chain;
        }
        self.settings.set_list.songs = songs;
        // Everything that pointed into the old list is let go: the song being
        // written, those whose project was being found.
        self.setlist.active = None;
        self.setlist.artist = None;
        self.setlist.editing = None;
        self.setlist.making = None;
        self.setlist.saving = None;
        self.setlist.queue.clear();
        self.save_settings();
        self.setlist.message = if chained {
            tf!("{count} songs read from “{file}”; {added} devices added, the mode is the chain's." [count, file, added])
        } else {
            tf!("{count} songs read from “{file}”." [count, file])
        };
    }

    /// Writes the set list as a CSV file.
    pub fn export_set_list(&mut self) {
        let Some(path) = files::save_file(tr("CSV file"), &["csv"], &format!("{}.csv", tr("MIDI Pilot"))) else { return };
        let list = &self.settings.set_list;
        let devices: Vec<String> = if list.mode == PilotMode::Chain { list.devices.iter().map(|d| d.name.clone()).collect() } else { Vec::new() };
        self.setlist.message = match std::fs::write(&path, setlist::to_csv(&list.songs, &devices)) {
            Ok(()) => tf!("The list is written in {}.", path.display()),
            Err(e) => tf!("The list was not written: {e}" [e]),
        };
    }

    /// Sends messages to the MIDI output, each followed by its pause (bounded,
    /// for the window not to freeze); the bytes are kept for the window to
    /// show, output or not. The one place that sends.
    pub fn send_set_messages(&mut self, messages: &[(Vec<u8>, u16)]) -> Sending {
        self.setlist.sent = messages.iter().map(|(bytes, _)| bytes.clone()).collect();
        // A pedal module open owns the output: its thread sends.
        if let Some(session) = &mut self.setlist.pedal {
            session.ask(Job::Send(messages.to_vec()));
            return Sending::Done;
        }
        let Some(output) = &mut self.setlist.output else { return Sending::NoOutput };
        let mut waited = 0u32;
        for (bytes, pause) in messages {
            if let Err(e) = output.send(bytes) {
                return Sending::Failed(e);
            }
            let pause = u32::from((*pause).min(500));
            if pause > 0 && waited < 2000 {
                std::thread::sleep(std::time::Duration::from_millis(u64::from(pause)));
                waited += pause;
            }
        }
        Sending::Done
    }

    /// Makes and sends the messages of a song; says how it went.
    fn send_set_song(&mut self, song: &SetSong) -> String {
        let label = song.label();
        self.setlist.sent.clear();
        let (mode, what, channel, value) = {
            let list = &self.settings.set_list;
            (list.mode, message_name(list.message), list.channel + 1, list.value_of(song.number).unwrap_or(0))
        };
        match self.settings.set_list.messages_for(song) {
            Err(SendError::Number(number)) => tf!("{label}: the number {number} cannot be sent (0 to 127)." [label, number]),
            Err(SendError::Device(device)) => tf!("{label}: the device “{device}” is not in the chain (Settings > Devices…)." [label, device]),
            Ok(messages) if messages.is_empty() => tf!("{label}: it has no message (right-click it, Change…)." [label]),
            Ok(messages) => {
                let count = messages.len();
                match (self.send_set_messages(&messages), mode) {
                    (Sending::Done, PilotMode::Switcher) => tf!("{label}: {what} = {value}, channel {channel}." [label, what, value, channel]),
                    (Sending::Done, PilotMode::Chain) => tf!("{label}: {count} messages sent." [label, count]),
                    (Sending::NoOutput, _) => tf!("{label} (no MIDI output: Settings > MIDI output)." [label]),
                    (Sending::Failed(e), _) => tf!("{label}: the MIDI message was not sent ({e})." [label, e]),
                }
            }
        }
    }

    /// Sends the messages of the song being written, to try them.
    pub fn send_steps_now(&mut self) {
        if let Some((_, song)) = self.setlist.editing.clone() {
            self.setlist.message = self.send_set_song(&song);
        }
    }

    /// Chooses a song: its messages go to the MIDI output, and its project
    /// opens when that is asked for.
    pub fn choose_set_song(&mut self, index: usize) {
        let Some(song) = self.settings.set_list.songs.get(index).cloned() else { return };
        self.setlist.active = Some(index);
        if self.settings.set_list.order == setlist::SetOrder::ByArtist {
            self.setlist.artist = Some(song.artist.clone());
        }
        let mut message = self.send_set_song(&song);
        if self.settings.set_list.open_project {
            let path = PathBuf::from(&song.project);
            if song.project.is_empty() {
                message.push(' ');
                message.push_str(tr("It has no project: right-click it to find one."));
            } else if !path.is_file() {
                message.push(' ');
                message.push_str(&tf!("Its project is no longer there: {}", path.display()));
            } else if self.pilot_only {
                // The pilot alone opens nothing: the whole program is started on the project.
                let started = std::env::current_exe().map_err(|e| e.to_string()).and_then(|program| std::process::Command::new(program).arg(&path).spawn().map_err(|e| e.to_string()));
                message.push(' ');
                match started {
                    Ok(_) => message.push_str(tr("Its project opens in OpenGarageMusic.")),
                    Err(e) => message.push_str(&tf!("Its project was not opened: {e}" [e])),
                }
            } else if self.path.as_deref() != Some(path.as_path()) {
                if let Err(e) = self.open_path(&path) {
                    message.push(' ');
                    message.push_str(&tf!("Its project was not opened: {e}" [e]));
                }
            }
        }
        self.setlist.message = message;
    }

    /// The folder where the projects made for the list are saved.
    pub fn set_list_folder(&self) -> PathBuf {
        let chosen = self.settings.set_list.folder.trim();
        if !chosen.is_empty() {
            return PathBuf::from(chosen);
        }
        let (base, old) = match (self.settings_path.is_some(), std::env::var_os("USERPROFILE")) {
            (true, Some(home)) => (PathBuf::from(home).join("OpenGarageMusic"), "Set List"),
            _ => (self.data_dir.clone(), "Set List"),
        };
        // The folder of the first versions, when it is there, is kept.
        let folder = base.join("MIDI Pilot");
        if !folder.is_dir() && base.join(old).is_dir() {
            return base.join(old);
        }
        folder
    }

    /// Gives a song its project.
    pub fn link_set_project(&mut self, index: usize, path: &Path) {
        if let Some(song) = self.settings.set_list.songs.get_mut(index) {
            song.project = path.to_string_lossy().into_owned();
            let label = song.label();
            self.setlist.message = tf!("{label}: its project is {}." [label], path.display());
            self.save_settings();
        }
    }

    /// Gives a song a project file chosen.
    pub fn choose_set_project(&mut self, index: usize) {
        let Some(path) = files::project_file(&self.projects_dir()) else { return };
        self.link_set_project(index, &path);
    }

    /// Gives a song the project open now; saved in the folder of the list
    /// when it has no file yet.
    pub fn link_open_project(&mut self, index: usize) {
        match self.path.clone() {
            Some(path) => self.link_set_project(index, &path),
            None => self.save_for_song(index),
        }
    }

    /// Saves the project open now in the folder of the list, under the name
    /// of the song, and gives it to the song.
    fn save_for_song(&mut self, index: usize) {
        let Some(song) = self.settings.set_list.songs.get(index) else { return };
        let folder = self.set_list_folder();
        let name = file_name(song);
        let mut path = folder.join(format!("{name}.ogproj"));
        let mut n = 2;
        while path.exists() {
            path = folder.join(format!("{name} ({n}).ogproj"));
            n += 1;
        }
        let saved = std::fs::create_dir_all(&folder).map_err(|e| e.to_string()).and_then(|()| self.save_to(&path));
        match saved {
            Ok(()) => {
                self.path = Some(path.clone());
                self.note_recent(&path);
                self.link_set_project(index, &path);
            }
            Err(e) => self.setlist.message = tf!("The project was not saved: {e}" [e]),
        }
    }

    /// Looks for a song's project: among the projects kept as sessions, by
    /// its artist and title; otherwise on the site chosen, in the window of
    /// the tablature sites, where the user picks the tablature. Says whether
    /// it is found already.
    pub fn find_set_project(&mut self, index: usize) -> bool {
        let Some(song) = self.settings.set_list.songs.get(index).cloned() else { return false };
        let mut kept: Vec<crate::sessions::Entry> = crate::sessions::list(&self.sessions_dir())
            .into_iter()
            .filter(|entry| setlist::same(&entry.about.artist, &song.artist) && setlist::same(&entry.about.title, &song.title))
            .collect();
        // One with its file first.
        kept.sort_by_key(|entry| !entry.about.path.as_deref().is_some_and(|p| Path::new(p).is_file()));
        if let Some(entry) = kept.first() {
            match entry.about.path.as_deref().map(PathBuf::from).filter(|path| path.is_file()) {
                Some(path) => self.link_set_project(index, &path),
                None => {
                    if self.restore_session_from(&entry.dir) {
                        self.save_for_song(index);
                    }
                }
            }
            return true;
        }
        let name = self.set_list_site().unwrap_or_default();
        self.open_site_named(&name);
        self.sites.clear_results();
        self.sites.search = format!("{} {}", song.artist, song.title).trim().to_owned();
        self.sites_search();
        self.setlist.making = Some(index);
        let label = song.label();
        self.setlist.message = tf!("{label}: choose its tablature in the main window." [label]);
        false
    }

    /// The site searched for the songs without a project: the one chosen,
    /// else the first that can be searched.
    pub fn set_list_site(&self) -> Option<String> {
        let searchable = |name: &str| self.known_sites.iter().any(|k| !k.site.open_only && k.site.is_tablature() && k.site.name.eq_ignore_ascii_case(name));
        let chosen = &self.settings.set_list.site;
        if searchable(chosen) {
            return Some(chosen.clone());
        }
        self.known_sites.iter().find(|k| !k.site.open_only && k.site.is_tablature()).map(|k| k.site.name.clone())
    }

    /// Finds the projects of all the songs that have none, one after the other.
    pub fn prepare_set_list(&mut self) {
        self.setlist.queue = self.settings.set_list.songs.iter().enumerate().filter(|(_, song)| song.project.is_empty() || !Path::new(&song.project).is_file()).map(|(i, _)| i).collect();
        if self.setlist.queue.is_empty() {
            self.setlist.message = tr("Every song has its project.").to_owned();
            return;
        }
        self.next_set_project();
    }

    /// Goes on with the songs still without a project, until one waits for the user.
    fn next_set_project(&mut self) {
        while let Some(index) = self.setlist.queue.pop_front() {
            if !self.find_set_project(index) {
                return;
            }
        }
        if self.setlist.making.is_none() && self.setlist.saving.is_none() {
            let linked = self.settings.set_list.songs.iter().filter(|s| !s.project.is_empty()).count();
            let count = self.settings.set_list.songs.len();
            self.setlist.message = tf!("The list is prepared: {linked} songs of {count} have their project." [linked, count]);
        }
    }

    /// The project made for a song is just made (by the window of the tablature sites).
    pub fn set_project_made(&mut self) {
        if let Some(index) = self.setlist.making.take() {
            self.setlist.saving = Some(index);
        }
    }

    /// Once a frame: saves the project made for a song once it is whole, and
    /// goes on with the list being prepared; the window of the sites closed
    /// without a project stops it.
    pub fn poll_set_list(&mut self) {
        self.poll_pedal();
        self.poll_script();
        self.poll_api();
        if let Some(index) = self.setlist.saving {
            if self.aligning.is_none() && self.words_search.is_none() {
                self.setlist.saving = None;
                self.save_for_song(index);
                self.next_set_project();
            }
            return;
        }
        if self.setlist.making.is_some() && !matches!(self.dialog, Some(Dialog::Sites)) {
            self.setlist.making = None;
            if !self.setlist.queue.is_empty() {
                self.setlist.queue.clear();
                self.setlist.message = tr("The preparation of the list is stopped.").to_owned();
            }
        }
    }

    /// Gives a song a picture of the user's, over the one found.
    pub fn choose_set_image(&mut self, index: usize) {
        let Some(path) = files::open_file(tr("Picture"), &["jpg", "jpeg", "png"]) else { return };
        self.set_song_image(index, Some(&path));
    }

    /// Gives a song a picture of the user's, or takes it away for the one found.
    pub fn set_song_image(&mut self, index: usize, path: Option<&Path>) {
        let Some(song) = self.settings.set_list.songs.get_mut(index) else { return };
        if !song.image.is_empty() {
            self.setlist.covers.forget(&Cover::File(PathBuf::from(&song.image)));
        }
        song.image = path.map_or_else(String::new, |p| p.to_string_lossy().into_owned());
        if let Some(path) = path {
            self.setlist.covers.forget(&Cover::File(path.to_path_buf()));
        }
        self.save_settings();
    }

    /// The picture of a song's button: the user's, else its album's.
    pub fn song_cover(&self, index: usize) -> Option<Cover> {
        let song = self.settings.set_list.songs.get(index)?;
        if !song.image.is_empty() {
            return Some(Cover::File(PathBuf::from(&song.image)));
        }
        (!self.settings.set_list.no_covers && !song.title.trim().is_empty()).then(|| Cover::Song { artist: song.artist.clone(), title: song.title.clone() })
    }

    /// Writes the song being added or changed into the list.
    pub fn keep_set_song(&mut self) {
        let Some((place, song)) = self.setlist.editing.take() else { return };
        let songs = &mut self.settings.set_list.songs;
        match place.filter(|&i| i < songs.len()) {
            Some(i) => {
                let (project, image) = (songs[i].project.clone(), songs[i].image.clone());
                songs[i] = SetSong { project, image, ..song };
            }
            None => songs.push(song),
        }
        self.save_settings();
    }

    /// Adds a device to the chain, named after its rank until the user names it.
    pub fn add_set_device(&mut self) {
        let devices = &mut self.settings.set_list.devices;
        let mut n = devices.len() + 1;
        let mut name = tf!("Device {n}" [n]);
        while devices.iter().any(|d| setlist::same(&d.name, &name)) {
            n += 1;
            name = tf!("Device {n}" [n]);
        }
        devices.push(setlist::Device { name, model: String::new(), channel: 0, ..setlist::Device::default() });
        self.save_settings();
    }

    /// A device's name is written (it was `old`): the steps that went to it
    /// follow; an empty name goes back to the old one.
    pub fn rename_set_device(&mut self, index: usize, old: &str) {
        let Some(device) = self.settings.set_list.devices.get_mut(index) else { return };
        let name = device.name.trim().to_owned();
        if name.is_empty() {
            device.name = old.to_owned();
            return;
        }
        device.name = name.clone();
        if !setlist::same(old, &name) {
            let steps = self.settings.set_list.songs.iter_mut().flat_map(|song| song.steps.iter_mut());
            let editing = self.setlist.editing.iter_mut().flat_map(|(_, song)| song.steps.iter_mut());
            for step in steps.chain(editing) {
                if setlist::same(&step.device, old) {
                    step.device = name.clone();
                }
            }
        }
        self.save_settings();
    }

    /// Takes a device out of the chain, with the messages that went to it.
    pub fn remove_set_device(&mut self, index: usize) {
        if index >= self.settings.set_list.devices.len() {
            return;
        }
        let device = self.settings.set_list.devices.remove(index);
        let mut dropped = 0;
        for song in &mut self.settings.set_list.songs {
            let before = song.steps.len();
            song.steps.retain(|step| !setlist::same(&step.device, &device.name));
            dropped += before - song.steps.len();
        }
        if let Some((_, song)) = &mut self.setlist.editing {
            song.steps.retain(|step| !setlist::same(&step.device, &device.name));
        }
        let name = device.name;
        self.setlist.message = tf!("{name} is taken out of the chain, with {dropped} messages of the songs." [name, dropped]);
        self.save_settings();
    }

    /// Takes a song out of the list.
    pub fn remove_set_song(&mut self, index: usize) {
        if index >= self.settings.set_list.songs.len() {
            return;
        }
        self.settings.set_list.songs.remove(index);
        let shift = |place: usize| (place != index).then(|| if place > index { place - 1 } else { place });
        self.setlist.active = self.setlist.active.and_then(shift);
        self.setlist.making = self.setlist.making.and_then(shift);
        self.setlist.saving = self.setlist.saving.and_then(shift);
        self.setlist.queue = self.setlist.queue.iter().filter_map(|&place| shift(place)).collect();
        // The song being changed moves with the list; removed, its window closes.
        self.setlist.editing = match self.setlist.editing.take() {
            Some((Some(place), song)) => shift(place).map(|place| (Some(place), song)),
            other => other,
        };
        self.save_settings();
    }
}

/// What a message is called in the window.
pub fn message_name(message: setlist::SetMessage) -> &'static str {
    match message {
        setlist::SetMessage::BankSelect => tr("CC 0 (bank select)"),
        setlist::SetMessage::ProgramChange => tr("Program change"),
    }
}

/// What a mode is called in the window.
pub fn mode_name(mode: PilotMode) -> &'static str {
    match mode {
        PilotMode::Switcher => tr("Switcher"),
        PilotMode::Chain => tr("Chain"),
    }
}

/// What a kind of step is called in the window.
pub fn step_kind_name(message: &StepMessage) -> &'static str {
    match message {
        StepMessage::Program { .. } => tr("Program change"),
        StepMessage::Control { .. } => tr("Control change"),
        StepMessage::SysEx(_) => tr("System exclusive"),
    }
}

/// What a model of device is called: the makers' names, or any device.
pub fn model_name(model: &str) -> &'static str {
    match model {
        "g3s" => "The GigRig G3S",
        "dd500" => "BOSS DD-500",
        "rv500" => "BOSS RV-500",
        _ => tr("Any device"),
    }
}

/// Bytes as hexadecimal pairs, "F0 41 7F".
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// Bytes read from hexadecimal pairs, spaces and commas between them or not;
/// none when the text is not that.
pub fn parse_hex(text: &str) -> Option<Vec<u8>> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace() && *c != ',').collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok()).collect()
}
