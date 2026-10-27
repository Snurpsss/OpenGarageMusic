//! Application state and everything that is not drawing: selection, undo,
//! recording, files, clipboard. The panels live in the `ui_*` modules.

use crate::files;
use crate::i18n::tr;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::{Arc, MutexGuard};

use eframe::egui::{self, Color32, Key, Pos2, Rect, Stroke};
use og_engine::capture::{self, Capture, Captures};
use og_engine::convolution::{self, Impulse};
use og_engine::loops::{self, LoopDef};
use og_engine::patches::{self, PatchDef};
use og_engine::synth::{bank_presets, load_bank, BANK_EXTENSIONS};
use og_engine::{
    analysis, codec, render_offline_full, smf, stretch, templates, wav, AudioRef, Clip, ClipData,
    Clips, Content, DrummerParams, Engine, ExternalUnit, FileRef, Fonts, Marker, Note, PluginSlot,
    Point, Project, Region, SamplerParams, Sound, Track, TrackKind,
};

use crate::clap_host::{ParamInfo, PluginInfo};
use crate::plugin::{self, from_hex, to_hex, PluginHandle};
use crate::theme::Theme;
use crate::vst3_host::is_vst3;
use crate::downloads;
use crate::lv2_host;
use crate::export::{self, Compressed};
use serde::{Deserialize, Serialize};

use crate::audio::{self, Audio};

pub const HEADER_W: f32 = 270.0;
pub const LANE_H: f32 = 62.0;
pub const GLOBAL_H: f32 = 34.0;
pub const RULER_H: f32 = 28.0;
/// Note grid in beats (a sixteenth note).
pub const SNAP: f64 = 0.25;
/// Width in pixels of the zone at the end of a region or note that resizes it.
pub const EDGE: f32 = 7.0;
const EXPORT_RATE: u32 = 44_100;
/// The project itself, in the archive that is its file.
const PROJECT_ENTRY: &str = "project.json";

pub const KEY_LABELS: [&str; 12] =
    ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"];
pub const NOTE_LETTERS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Computer keys that play notes, as semitones above C. Both QWERTY and AZERTY
/// positions of the home row are listed so the layout feels the same on either.
const TYPING: [(Key, i32); 17] = [
    (Key::A, 0),
    (Key::Q, 0),
    (Key::W, 1),
    (Key::Z, 1),
    (Key::S, 2),
    (Key::E, 3),
    (Key::D, 4),
    (Key::F, 5),
    (Key::T, 6),
    (Key::G, 7),
    (Key::Y, 8),
    (Key::H, 9),
    (Key::U, 10),
    (Key::J, 11),
    (Key::K, 12),
    (Key::O, 13),
    (Key::L, 14),
];

/// How many colours a track can take; the colours themselves are the theme's.
pub const TRACK_COLORS: usize = 8;

pub use crate::theme::palette;

/// A plug-in started: what processes the sound, the handle of its window
/// and state when it has one, and its parameters.
type StartedPlugin = (Box<dyn ExternalUnit>, Option<Box<dyn PluginHandle>>, Vec<ParamInfo>);

pub fn track_color(index: u8) -> Color32 {
    palette(usize::from(index))
}

pub fn stroke(width: f32, color: Color32) -> Stroke {
    Stroke::new(width, color)
}

pub fn snap(beat: f64, grid: f64) -> f64 {
    (beat / grid).round() * grid
}

pub fn accepts(kind: TrackKind, content: &Content) -> bool {
    match content {
        Content::Midi(_) => kind == TrackKind::Instrument,
        Content::Audio(_) => kind.is_audio(),
        Content::Drummer(_) => kind == TrackKind::Drummer,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum CurveId {
    Tempo,
    Transpose,
    Master,
    Volume(usize),
    Pan(usize),
    /// A setting of the effects of a track.
    Fx(usize, og_engine::FxParam),
}

#[derive(Clone)]
pub enum Drag {
    MoveRegion { ti: usize, ri: usize, grab: f64 },
    ResizeRegion { ti: usize, ri: usize },
    LoopRegion { ti: usize, ri: usize },
    /// Moving the selected notes: where the drag started, and the notes as they were.
    Notes { beat: f64, pitch: i32, originals: Vec<(usize, Note)> },
    ResizeNotes { beat: f64, originals: Vec<(usize, Note)> },
    Rubber { from: Pos2 },
    Point { curve: CurveId, index: usize },
    Marker { index: usize, grab: f64, resize: bool },
    Cycle { anchor: f64 },
}

/// A change to the list of tracks asked from a track's own menu.
#[derive(Clone, Copy, PartialEq)]
pub enum TrackAction {
    Delete,
    Duplicate,
    Move(isize),
}

/// How the notes of a region are shown in the editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoteView {
    Roll,
    Score,
    /// Tablature: the strings of a guitar or a bass, with fret numbers.
    Tab,
}

#[derive(Clone, Copy, PartialEq)]
pub enum BottomTab {
    Controls,
    Editor,
}

#[derive(Clone, Copy, PartialEq)]
pub enum RightPanel {
    None,
    Loops,
    Notes,
}

#[derive(Clone, Copy, PartialEq)]
pub enum AutoParam {
    Volume,
    Pan,
    Fx(og_engine::FxParam),
}

impl AutoParam {
    /// Everything the automation lanes can show, in the order offered.
    pub fn all() -> Vec<AutoParam> {
        let mut all = vec![AutoParam::Volume, AutoParam::Pan];
        all.extend(og_engine::FxParam::FIXED.into_iter().map(AutoParam::Fx));
        all.extend((0..4).map(|i| AutoParam::Fx(og_engine::FxParam::Pedal(i))));
        all
    }

    pub fn label(self) -> String {
        match self {
            AutoParam::Volume => tr("Volume").to_owned(),
            AutoParam::Pan => tr("Pan").to_owned(),
            AutoParam::Fx(og_engine::FxParam::Pedal(i)) => tf!("Pedal {}", i + 1),
            AutoParam::Fx(param) => tr(param.label()).to_owned(),
        }
    }
}

pub enum Dialog {
    NewProject,
    NewTrack,
    Tuner,
    SavePatch(String),
    /// Choosing among the plug-ins of a file.
    Plugins { file: PathBuf, options: Vec<PluginInfo> },
    /// Browsing and downloading freely licensed sounds and plug-ins.
    Downloads,
    /// Choosing a plug-in file among those installed.
    PluginPicker,
    /// Quitting with changes that are not saved, when sessions are not kept.
    Quit,
    /// Choosing the audio output and input of the program.
    Audio,
    /// The tablature sites: a search on one, the tracks of a song to make a
    /// project of, and the way in for Guitar Pro files.
    Sites,
    /// The projects kept as sessions, to reopen one.
    Sessions,
    /// What the song is exported as.
    Export,
    /// What is imported into the project.
    Import,
    /// The words of the song, typed or pasted.
    Words,
}

/// Where the instruments of a bank go in the library.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Section {
    /// Each instrument in the section of its family (General MIDI numbering).
    #[default]
    Auto,
    /// Everything in one existing section.
    Category(String),
    /// A section of its own, named after the bank.
    Own,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BankEntry {
    pub path: String,
    #[serde(default)]
    pub section: Section,
}

/// Library section of a General MIDI program.
pub fn gm_category(bank: u16, preset: u8) -> &'static str {
    if bank == 128 {
        return patches::CAT_DRUMS;
    }
    match preset {
        0..=7 | 16..=23 => patches::CAT_KEYS,
        8..=15 | 112..=119 => patches::CAT_MALLET,
        24..=31 | 104..=111 => patches::CAT_GUITAR,
        32..=39 => patches::CAT_BASS,
        40..=79 => patches::CAT_ORCH,
        _ => patches::CAT_SYNTH,
    }
}

/// State of the download window.
pub struct DownloadsUi {
    pub provider: downloads::Provider,
    /// The user ticked "I trust this publisher" for the selected plug-in.
    pub trusted: bool,
    pub search: String,
    pub listing: Option<Result<Vec<downloads::Item>, String>>,
    pub pending: Option<std::sync::mpsc::Receiver<Result<Vec<downloads::Item>, String>>>,
    pub selected: Option<usize>,
    pub section: Section,
    pub job: Option<downloads::Job>,
    pub message: String,
    /// For each listed item, whether it is already on the disk.
    pub have: Vec<bool>,
    /// An item whose licence and files are being looked up, by its place in
    /// the list.
    pub resolving: Option<(usize, std::sync::mpsc::Receiver<Result<downloads::Item, String>>)>,
    /// Why the item at this place in the list cannot be downloaded.
    pub blocked: Option<(usize, String)>,
}

/// State of the window of the tablature sites: the site searched, the
/// search, the song chosen and its tracks.
#[derive(Default)]
pub struct SitesUi {
    /// The site searched: its place among the sites known.
    pub site: usize,
    pub search: String,
    /// The songs found, and the search under way.
    pub found: Option<Result<Vec<og_engine::sites::Found>, String>>,
    pub finding: Option<std::sync::mpsc::Receiver<Result<Vec<og_engine::sites::Found>, String>>>,
    /// The song chosen among them, and which of its tracks are ticked.
    pub selected: Option<usize>,
    pub chosen: Vec<bool>,
    /// The song being read; when its tracks were ticked first, the project
    /// is made as soon as it is read.
    pub opening: Option<std::sync::mpsc::Receiver<Result<crate::sites::Opened, String>>>,
    pub create_when_read: bool,
    /// The first song found is chosen as soon as the search answers (for the
    /// pictures of the documentation).
    pub pick_first: bool,
    /// The score of the song read, whose parts are then ticked.
    pub score: Option<og_engine::guitar_pro::Score>,
    pub message: String,
    pub error: bool,
    /// The video of the song is asked for with it (Songsterr gives them).
    pub want_video: bool,
    /// A recording of the song to play it with: the one found in the music
    /// library, or one chosen; used when ticked.
    pub recording: Option<std::path::PathBuf>,
    pub use_recording: bool,
    /// The words of the song are looked for with it.
    pub want_words: bool,
    /// Signing in to a site: the code being asked for, the code to enter on
    /// the site, and the token being waited for.
    pub logging_in: Option<std::sync::mpsc::Receiver<Result<crate::login::DeviceCode, String>>>,
    pub device: Option<crate::login::DeviceCode>,
    pub waiting: Option<std::sync::mpsc::Receiver<Result<crate::login::SiteToken, String>>>,
    /// The key being typed, for a site that asks each program for one.
    pub key_text: String,
    /// The files a site that gives captures or impulses listed for the one
    /// chosen, and the folder they were written to.
    pub files: Option<Vec<(String, Vec<u8>)>>,
    pub saved: Option<PathBuf>,
}

impl SitesUi {
    /// A thread of the window is at work: the window asks to be redrawn.
    pub fn busy(&self) -> bool {
        self.finding.is_some() || self.opening.is_some() || self.logging_in.is_some() || self.waiting.is_some()
    }

    /// Forgets the results of a search, keeping the text searched.
    pub fn clear_results(&mut self) {
        self.found = None;
        self.finding = None;
        self.opening = None;
        self.create_when_read = false;
        self.score = None;
        self.selected = None;
        self.chosen.clear();
        self.message.clear();
        self.error = false;
        self.files = None;
        self.saved = None;
    }

    /// The song chosen, if the search found it.
    pub fn chosen_song(&self) -> Option<&og_engine::sites::Found> {
        self.found.as_ref()?.as_ref().ok()?.get(self.selected?)
    }
}

/// What is kept between sessions.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub user_patches: Vec<PatchDef>,
    pub loop_favorites: Vec<String>,
    /// Paths written by earlier versions; moved to `banks` when read.
    pub soundfonts: Vec<String>,
    /// Sound banks the user installed, and where their instruments are filed.
    pub banks: Vec<BankEntry>,
    /// Plug-in files the user imported from elsewhere on the disk.
    pub plugins: Vec<String>,
    /// The single row of panels of earlier versions; read into `panel_rows`.
    pub panel_order: Vec<Panel>,
    /// The rows the user arranged the panels of the sound controls in, and
    /// the panels they chose to hide.
    pub panel_rows: Vec<Vec<Panel>>,
    pub panels_hidden: Vec<Panel>,
    /// Code of the language of the interface; empty for English.
    pub language: String,
    /// Names of the audio output and input the program uses; empty for the
    /// system's default ones.
    pub output_device: String,
    pub input_device: String,
    /// Channel of the input that is recorded (1 is the first); 0 mixes them all.
    pub input_channel: u16,
    /// Project files opened or saved lately, the latest first.
    pub recent_projects: Vec<String>,
    /// Bit rate of MP3 and AAC exports, in kilobits per second; 0 for 192.
    pub export_kbps: u32,
    /// Name of the theme (the file of its colours, without ".json"); empty
    /// for the program's own.
    pub theme: String,
    /// On quitting, nothing is kept for the next start: the program then
    /// offers to save a project that has changes.
    pub forget_session: bool,
    /// Plug-ins are loaded into the application itself instead of each in a
    /// process of its own: faster to start, but a faulty one takes the
    /// application down with it.
    pub plugins_in_process: bool,
    /// Sounds and loops are named here as the library names them (in English).
    /// Settings written before the interface was translated named them in French.
    pub library_names: bool,
    /// While the song plays, the playhead of the note editors moves across
    /// the view and the view pages; otherwise (the usual way) the playhead
    /// stays put towards the left and the notes scroll under it.
    pub editor_playhead_moves: bool,
    /// While the song plays, the playhead moves across the tracks and the
    /// view pages; otherwise (the usual way) the playhead stays in the middle
    /// and the tracks slide under it.
    pub tracks_playhead_moves: bool,
    /// Size of the fret numbers of the tablature, which what it writes
    /// around them follows; 0 for the usual size.
    pub tab_text_size: f32,
    /// How many of the last projects are kept as sessions (5 to 100; 0 for 20).
    pub sessions_kept: u32,
    /// Where the sessions are kept; empty for the folder OpenGarageMusic\Sessions
    /// of the user's own folder.
    pub sessions_folder: String,
    /// Where the projects are saved, when the user chose a folder; empty for
    /// `OpenGarageMusic` in the user's documents.
    pub projects_folder: String,
    /// The folder of the user's music, searched for the recording of a song
    /// a tablature is of; empty when there is none.
    pub music_folder: String,
    /// How late the video player is on this computer, in milliseconds.
    pub video_delay_ms: i32,
    /// The video shows in a corner when no editor shows it.
    pub show_video: bool,
    /// The band of the words under the editor is hidden.
    pub hide_lyrics: bool,
    /// The band shows the whole text, rather than the lines being sung.
    pub lyrics_whole: bool,
    /// The height the user gave the band of the words; 0 for the usual one.
    pub words_height: f32,
    /// The tablature sites the user agreed to have the program read.
    pub sites_confirmed: Vec<String>,
    /// The score and the tablature are laid out over two lines of whole
    /// bars, as on a page, instead of one line that scrolls.
    pub two_lines: bool,
    /// With several tracks in the editor and auto-switch on: bars ahead in
    /// which the track being followed must have a note to come, or else the
    /// next one that plays is shown; 0 for the usual two.
    pub auto_switch_bars: u32,
    /// The set list: its songs, and how they are played.
    pub set_list: og_engine::setlist::SetList,
    /// The zoom of the note editors, in pixels a beat; 0 until the user zooms.
    pub editor_zoom: f64,
    /// The tokens of the sites the user signed in to, by the name of the site.
    pub site_logins: std::collections::BTreeMap<String, crate::login::SiteToken>,
    /// The keys the user made for the program at the sites that ask each
    /// program for one, by the name of the site.
    pub site_keys: std::collections::BTreeMap<String, String>,
}

/// What a thread sent through its channel, if it has; the slot is emptied
/// then. A thread that died without answering is an error, so that nothing
/// waits for it for ever.
pub(crate) fn take_result<T>(slot: &mut Option<std::sync::mpsc::Receiver<Result<T, String>>>, stopped: &str) -> Option<Result<T, String>> {
    let result = match slot.as_ref()?.try_recv() {
        Ok(result) => result,
        Err(std::sync::mpsc::TryRecvError::Empty) => return None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Err(stopped.to_owned()),
    };
    *slot = None;
    Some(result)
}

/// A project made from a template, with its names in the language in use.
pub fn localized(mut project: Project) -> Project {
    project.name = tr(&project.name).to_owned();
    for track in &mut project.tracks {
        track.name = tr(&track.name).to_owned();
        for region in &mut track.regions {
            region.name = tr(&region.name).to_owned();
        }
    }
    for marker in &mut project.markers {
        marker.name = tr(&marker.name).to_owned();
    }
    project
}

/// The library's name of a sound or a loop that an earlier version, which
/// spoke French only, wrote down in French.
fn library_name(french: Option<&crate::i18n::Catalog>, name: &mut String) {
    if let Some(known) = french.and_then(|catalog| catalog.source_of(name)) {
        *name = known.to_owned();
    }
}

/// Where a panel is put relative to another: in its row, or in a new row.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Side {
    Before,
    After,
    Above,
    Below,
}

/// One box of the sound controls. Boxes can be reordered and hidden.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Panel {
    Sound,
    Arp,
    Amp,
    Pedals,
    Plugins,
    Eq,
    Dynamics,
    Master,
    Save,
}

impl Panel {
    /// Every panel, in the order they have until the user moves them.
    pub const ALL: [Panel; 9] =
        [Panel::Sound, Panel::Arp, Panel::Amp, Panel::Pedals, Panel::Plugins, Panel::Eq, Panel::Dynamics, Panel::Master, Panel::Save];

    /// The rows the panels are in until the user moves them: what shapes the
    /// sound first, what treats it after.
    pub fn default_rows() -> Vec<Vec<Panel>> {
        vec![
            vec![Panel::Sound, Panel::Arp, Panel::Amp, Panel::Pedals, Panel::Plugins],
            vec![Panel::Eq, Panel::Dynamics, Panel::Master, Panel::Save],
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Panel::Sound => tr("Sound"),
            Panel::Arp => tr("Arpeggiator"),
            Panel::Amp => tr("Amp"),
            Panel::Pedals => tr("Pedals"),
            Panel::Plugins => tr("External plug-ins (CLAP, LV2, VST3)"),
            Panel::Eq => tr("Equaliser"),
            Panel::Dynamics => tr("Dynamics and space"),
            Panel::Master => tr("Master output (all tracks)"),
            Panel::Save => tr("My sounds"),
        }
    }
}

/// A folder or a file next to the program.
pub fn beside_program(name: &str) -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join(name)))
}

/// Stable id of a SoundFont file, from its path.
fn font_id(path: &Path) -> u64 {
    crate::sessions::hash64(&path.to_string_lossy().to_lowercase()) | 1
}

pub struct App {
    pub project: Project,
    /// The project as last sent to the engine.
    pub synced: Project,
    pub audio: Audio,
    pub clips: Clips,
    clips_dirty: bool,
    /// The captures and impulses the effects refer to, by the path the project writes.
    pub captures: Captures,
    captures_dirty: bool,
    /// Loaded SoundFonts, the files they came from and their presets.
    pub fonts: Fonts,
    fonts_dirty: bool,
    pub font_files: Vec<FileRef>,
    font_patches: Vec<PatchDef>,
    /// Loaded plug-in files, the parameters of each running plug-in, and the
    /// slots whose plug-in could not be started.
    libraries: plugin::Libraries,
    pub plugin_params: HashMap<u64, Vec<ParamInfo>>,
    /// The main-thread side of each running plug-in: its window and its state.
    pub plugin_handles: HashMap<u64, Box<dyn PluginHandle>>,
    pub plugin_failed: Vec<u64>,
    /// The tests load plug-ins into their own process unless they ask otherwise.
    #[cfg(test)]
    pub isolate_for_test: bool,
    slot_counter: u64,
    pub downloads: DownloadsUi,
    pub sites: SitesUi,
    /// The program keeps the projects worked on as sessions (off in the tests,
    /// which turn it on with a folder of their own).
    pub keep_sessions: bool,
    /// The program is the MIDI Pilot alone: no project is opened here, the
    /// whole program is started on it instead.
    pub pilot_only: bool,
    /// The project as last kept, and when (in seconds of the clock of the window).
    pub kept: Option<Project>,
    pub kept_at: f64,
    /// The project open is a blank one: the demonstration as it starts, or a
    /// new one from a template. Untouched, it is not worth a session.
    pub blank: bool,
    /// The sessions listed in their window, read when it opens.
    pub session_list: Vec<crate::sessions::Entry>,
    pub session_message: String,
    /// The tablature sites known, and what was wrong with the descriptions
    /// of the `sites` folder that were not read.
    pub known_sites: Vec<crate::sites::Known>,
    pub site_problems: Vec<String>,
    /// Installed banks arriving from the thread that reads them at start-up.
    bank_loading: Option<std::sync::mpsc::Receiver<(BankEntry, Option<og_engine::Bank>)>>,
    /// Where downloads are kept, in a "Banques" and an "Effets" sub-folder.
    pub(crate) data_dir: PathBuf,
    monitor_on: bool,
    pointer_down: bool,
    /// Grid used by the audio timing correction, in beats.
    pub timing_grid: f64,
    /// Waveform summaries for drawing, by clip id.
    pub waveforms: HashMap<u64, Vec<(f32, f32)>>,
    pub path: Option<PathBuf>,
    /// The project as it was last opened, made or saved: what differs from it
    /// would be lost. None when that is not known to be on the disk.
    pub(crate) clean: Option<Project>,
    /// The window is closing; nothing more is asked.
    pub quitting: bool,
    pub status: String,
    /// The themes that can be chosen, and the one in use.
    pub themes: Vec<Theme>,
    pub theme: Arc<Theme>,
    theme_applied: bool,
    /// Quick help: the status bar tells what the pointer is over, and the
    /// tips of the buttons show at once.
    pub help: bool,
    pub help_text: String,
    help_applied: Option<bool>,
    pub help_rect: Rect,
    pub(crate) settings_path: Option<PathBuf>,
    pub settings: Settings,

    pub playing: bool,
    pub recording: bool,
    /// Audio take in progress: has the capture started (the count-in is over)?
    rec_audio: Option<bool>,
    pub position: f64,
    pub peak: f32,
    pub track_peaks: Vec<f32>,

    pub selected_track: usize,
    pub selected_region: Option<(usize, usize)>,
    pub selected_notes: Vec<usize>,
    pub drag: Option<Drag>,
    pub(crate) undo: Vec<Project>,
    pub(crate) redo: Vec<Project>,
    /// An explicit checkpoint was taken this frame.
    explicit_checkpoint: bool,
    /// The project was just replaced by undo, redo or a file: not an edit.
    suppress_auto_undo: bool,
    last_change: f64,
    pub clipboard: Option<Region>,
    /// Item a context menu was opened on.
    pub ctx_index: Option<usize>,
    pub pending_track: Option<(usize, TrackAction)>,

    /// Timeline zoom (pixels per beat) and left edge (beats).
    pub ppb: f32,
    pub scroll: f64,
    pub lane_w: f32,
    /// Where each track's lane was drawn last frame.
    pub lane_rects: Vec<Rect>,
    pub roll_rect: Rect,
    /// Where the sound controls and the title of each of their panels were
    /// drawn last frame.
    pub controls_rect: Rect,
    pub panel_rects: Vec<(Panel, Rect)>,
    /// Where each panel as a whole was drawn last frame.
    pub panel_frames: Vec<(Panel, Rect)>,
    /// Where the button that makes the bottom panel take the window was drawn.
    pub bottom_toggle_rect: Rect,
    /// Where the three points of the equaliser's curve were drawn last frame.
    pub eq_handles: Vec<Rect>,
    /// Where the box of each plug-in of the selected track was drawn last frame.
    pub slot_rects: Vec<(u64, Rect)>,

    pub show_library: bool,
    pub show_bottom: bool,
    /// Height of the panel under the tracks; it only changes when the user
    /// drags its top edge.
    pub bottom_height: f32,
    /// The panel takes the whole window instead of sitting under the tracks.
    pub bottom_full: bool,
    /// Audio outputs and inputs of this computer, as last listed.
    pub audio_outputs: Vec<String>,
    pub audio_inputs: Vec<String>,
    /// Number of channels of the program's input.
    pub audio_input_channels: u16,
    /// Outputs the tracks were last found to be sent to.
    routes_wanted: Vec<String>,
    /// Input that is listened to while a track monitors.
    monitor_input: String,
    pub bottom_tab: BottomTab,
    pub right_panel: RightPanel,
    pub show_global: bool,
    pub show_master: bool,
    pub automation: Option<AutoParam>,
    pub note_view: NoteView,
    /// Horizontal zoom of the note editors, as pixels a beat; none until the
    /// user zooms, when four bars show across. And the first beat they show.
    pub editor_zoom: Option<f64>,
    pub editor_scroll: f64,
    pub dialog: Option<Dialog>,

    pub note_len: f64,
    pub quantize: f64,
    /// The Q key was pressed: the note editor quantizes when it is next drawn.
    pub quantize_request: bool,
    pub roll_scroll_to: Option<f32>,
    /// Why the last Guitar Pro file was not read, for the window of the sites.
    pub import_message: String,
    /// The tracks chosen for the editor with Ctrl, in the order they were
    /// chosen: shown one under the other, or, with auto-switch on, one at a
    /// time, `followed` being the one shown.
    pub editor_tracks: Vec<usize>,
    pub auto_switch: bool,
    pub followed: Option<usize>,
    /// The beats chosen with Shift in a score or a tablature, to be looped.
    pub beat_selection: Option<crate::editor_beats::BeatSelection>,
    /// The line of a score or a tablature on several lines that the view
    /// last scrolled to while playing, and the wheel turns gathered towards
    /// the next pair of lines.
    pub editor_line: Option<usize>,
    /// The top of that line, from the top of the editor.
    pub editor_line_top: f32,
    /// How the lines were laid out (full window, how many), and for how many
    /// frames more the view is brought to the line being played: the height
    /// of the editor is only known a frame after it changes.
    pub editor_layout: (bool, usize),
    pub editor_refollow: u8,
    pub editor_wheel: f32,
    pub typing: bool,
    pub octave: i32,
    /// Computer keys currently held: key, track, pitch.
    held: Vec<(Key, usize, u8)>,
    /// Note sounding because the mouse is down on the editor's keyboard.
    pub key_preview: Option<(usize, u8)>,
    /// Short note played when a note is added: track, pitch, time to stop.
    pub preview: Option<(usize, u8, f64)>,

    pub patches: Vec<PatchDef>,
    pub lib_search: String,
    /// A sound the library is to scroll to when it is next drawn.
    pub lib_scroll_to: Option<String>,
    pub lib_category: String,
    pub loops: Vec<LoopDef>,
    /// The user's loops of recorded sound, and where each row of the loop
    /// browser was drawn last frame.
    pub audio_loops: Vec<crate::userloops::AudioLoop>,
    pub loop_rects: Vec<(crate::userloops::LoopPick, Rect)>,
    /// Left edge of the lanes, and the whole area of the tracks, as last drawn.
    pub lane_left: f32,
    pub tracks_rect: Rect,
    pub loop_search: String,
    pub loop_instrument: Option<&'static str>,
    pub loop_genre: Option<&'static str>,
    pub loop_mood: Option<&'static str>,
    pub loop_favorites_only: bool,
    pub auditioning: Option<String>,
    pub export_bits: u16,
    /// The format chosen in the export window.
    pub export_format: crate::ui_transfer::ExportFormat,
    /// A recording being read and found in step with the score.
    pub aligning: Option<std::sync::mpsc::Receiver<Result<crate::recording::Linked, String>>>,
    /// The video player, once started; whether it could not be; where an
    /// editor shows it this frame, and for which track.
    pub video: Option<crate::video::VideoPlayer>,
    pub video_failed: bool,
    pub video_rect: Option<egui::Rect>,
    pub video_editor: Option<usize>,
    /// The playhead when the video was last set, and whether the song waits for it.
    pub video_position: f64,
    pub video_waiting: bool,
    /// When the song was last moved to where the video is.
    pub video_corrected: Option<std::time::Instant>,
    /// The playhead was moved (by the user, or by the program to follow the video).
    pub playhead_moved: bool,
    /// The words being looked for on LRCLIB, what came of it, the words being
    /// typed, and the line last shown being sung.
    pub words_search: Option<std::sync::mpsc::Receiver<Result<Option<crate::words::Found>, String>>>,
    /// The window of the set list.
    pub setlist: crate::setlist::SetListUi,
    pub words_message: String,
    pub words_draft: String,
    pub words_line: Option<usize>,
    /// Where the line over the band of the words is, this frame.
    pub words_split: egui::Rect,
    /// The share of the height of the editor each of the tracks it shows takes.
    pub editor_shares: Vec<f32>,
    /// Where the lines between them are, this frame.
    pub editor_splits: Vec<egui::Rect>,
    /// The program's window, for the video player to show inside it.
    pub window_handle: isize,
    /// The recordings of the music library, and their reading under way.
    pub library: Vec<crate::library::Song>,
    pub library_scan: Option<std::sync::mpsc::Receiver<Vec<crate::library::Song>>>,
    /// Tuner reading: MIDI note and offset in cents.
    pub tuner: Option<(u8, f32)>,
    pub input_level: f32,
    tuner_clock: f64,
}

/// What the program does as it starts, after reading its settings and opening
/// the sound card, in order; the loading screen goes through them one a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartStep {
    /// The audio outputs and inputs, and the output the user chose.
    Devices,
    /// The project and the window as they were left.
    Session,
    /// The sound banks the user installed: they are read on another thread.
    Banks,
    /// The user's own audio loops.
    Loops,
}

impl StartStep {
    pub const ALL: [StartStep; 4] = [StartStep::Devices, StartStep::Session, StartStep::Banks, StartStep::Loops];
}

/// For screenshots and trials: the program as it is the very first time,
/// without reading or writing anything that belongs to the user (settings,
/// session, downloads).
pub fn demo_mode() -> bool {
    std::env::var_os("OPENGARAGE_DEMO").is_some()
}

impl App {
    /// The first part of starting: the settings read and the sound card
    /// opened. The rest is done by `start_step`, from the loading screen.
    pub fn begin() -> Self {
        Self::begin_with(audio::start(templates::demo()))
    }

    /// The program as it starts, on the sound card given (or none, for the
    /// MIDI Pilot alone).
    pub fn begin_with(audio: Audio) -> Self {
        if demo_mode() {
            let mut app = Self::with_audio(audio, None);
            app.settings.forget_session = true;
            if let Some(shared) = std::env::var_os("PROGRAMDATA") {
                app.data_dir = PathBuf::from(shared).join("OpenGarageMusic-demo");
            }
            return app;
        }
        let settings_path = std::env::var_os("APPDATA")
            .map(|dir| PathBuf::from(dir).join("OpenGarageMusic").join("settings.json"));
        // Settings written under the program's first name are carried over once.
        if let (Some(new), Some(roaming)) = (&settings_path, std::env::var_os("APPDATA")) {
            let old = PathBuf::from(roaming).join(concat!("Open", "Garage")).join("settings.json");
            if !new.exists() && old.exists() {
                if let Some(dir) = new.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                let _ = std::fs::copy(old, new);
            }
        }
        let mut app = Self::with_audio(audio, settings_path);
        // Downloads are large and belong to this computer: the local part of
        // AppData, which is not copied from machine to machine.
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            app.data_dir = PathBuf::from(local).join("OpenGarageMusic");
        }
        // The projects worked on are kept as sessions.
        app.keep_sessions = true;
        // The music library, as last read, then read again for what changed.
        app.load_library_index();
        app.scan_library();
        // The sites described in the folder next to the program join those shipped.
        (app.known_sites, app.site_problems) = crate::sites::load(crate::sites::folder().as_deref());
        app
    }

    /// One step of starting, after `begin`. In demonstration, only the audio
    /// devices are looked at.
    pub fn start_step(&mut self, step: StartStep) {
        if demo_mode() && step != StartStep::Devices {
            return;
        }
        match step {
            StartStep::Devices => {
                self.refresh_audio_devices();
                let chosen = self.settings.output_device.clone();
                if !chosen.is_empty() && !demo_mode() {
                    // An output that is no longer plugged in leaves the default one in use.
                    if let Err(e) = self.audio.set_output(&chosen) {
                        self.status = tf!("Audio output “{}”: {}", chosen, e);
                    }
                }
            }
            // The program comes back as it was left, unless told not to; the
            // sessions are kept either way.
            StartStep::Session => {
                if !self.settings.forget_session {
                    self.restore_session();
                }
            }
            StartStep::Banks => self.load_installed_banks(),
            StartStep::Loops => self.refresh_audio_loops(),
        }
    }

    pub fn with_audio(audio: Audio, settings_path: Option<PathBuf>) -> Self {
        let mut settings: Settings = settings_path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        if settings.panel_rows.is_empty() && !settings.panel_order.is_empty() {
            settings.panel_rows = vec![std::mem::take(&mut settings.panel_order)];
        }
        if !settings.library_names {
            let french = crate::i18n::built_in("fr");
            for name in &mut settings.loop_favorites {
                library_name(french.as_ref(), name);
            }
            for patch in &mut settings.user_patches {
                library_name(french.as_ref(), &mut patch.category);
            }
            settings.library_names = true;
        }
        // The tests keep the language they were written against.
        if !cfg!(test) {
            crate::i18n::set_language(&settings.language);
        }
        let project = localized(templates::demo());
        let mut app = Self {
            synced: project.clone(),
            clean: Some(project.clone()),
            quitting: false,
            project,
            audio,
            clips: Clips::new(),
            clips_dirty: false,
            captures: Captures::default(),
            captures_dirty: false,
            fonts: Fonts::new(),
            fonts_dirty: false,
            font_files: Vec::new(),
            font_patches: Vec::new(),
            libraries: plugin::Libraries::default(),
            plugin_params: HashMap::new(),
            plugin_handles: HashMap::new(),
            plugin_failed: Vec::new(),
            #[cfg(test)]
            isolate_for_test: false,
            slot_counter: 0,
            downloads: DownloadsUi {
                provider: downloads::Provider::MuseScore,
                trusted: false,
                search: String::new(),
                listing: None,
                pending: None,
                selected: None,
                section: Section::Auto,
                job: None,
                message: String::new(),
                have: Vec::new(),
                resolving: None,
                blocked: None,
            },
            sites: SitesUi::default(),
            keep_sessions: false,
            pilot_only: false,
            kept: None,
            kept_at: 0.0,
            blank: true,
            session_list: Vec::new(),
            session_message: String::new(),
            known_sites: crate::sites::load(None).0,
            site_problems: Vec::new(),
            bank_loading: None,
            // Tests and odd environments get a folder that is theirs alone.
            data_dir: std::env::temp_dir().join(format!("opengaragemusic-data-{}", std::process::id())),
            monitor_on: false,
            pointer_down: false,
            timing_grid: 0.5,
            waveforms: HashMap::new(),
            path: None,
            status: tr("Demonstration project loaded. Space: play.").to_owned(),
            themes: Vec::new(),
            theme: crate::theme::current(),
            theme_applied: false,
            help: false,
            help_text: String::new(),
            help_applied: None,
            help_rect: Rect::NOTHING,
            settings_path,
            settings,
            playing: false,
            recording: false,
            rec_audio: None,
            position: 0.0,
            peak: 0.0,
            track_peaks: Vec::new(),
            selected_track: 0,
            selected_region: None,
            selected_notes: Vec::new(),
            drag: None,
            undo: Vec::new(),
            redo: Vec::new(),
            explicit_checkpoint: false,
            suppress_auto_undo: false,
            last_change: f64::MIN,
            clipboard: None,
            ctx_index: None,
            pending_track: None,
            ppb: 28.0,
            scroll: 0.0,
            lane_w: 800.0,
            lane_rects: Vec::new(),
            roll_rect: Rect::NOTHING,
            controls_rect: Rect::NOTHING,
            panel_rects: Vec::new(),
            panel_frames: Vec::new(),
            bottom_toggle_rect: Rect::NOTHING,
            slot_rects: Vec::new(),
            eq_handles: Vec::new(),
            show_library: true,
            show_bottom: true,
            bottom_height: 290.0,
            bottom_full: false,
            audio_outputs: Vec::new(),
            audio_inputs: Vec::new(),
            audio_input_channels: 2,
            routes_wanted: Vec::new(),
            monitor_input: String::new(),
            bottom_tab: BottomTab::Editor,
            right_panel: RightPanel::None,
            show_global: false,
            show_master: false,
            automation: None,
            note_view: NoteView::Roll,
            editor_zoom: None,
            editor_scroll: 0.0,
            dialog: None,
            note_len: 0.5,
            quantize: 0.25,
            quantize_request: false,
            roll_scroll_to: None,
            import_message: String::new(),
            editor_tracks: Vec::new(),
            beat_selection: None,
            auto_switch: false,
            followed: None,
            editor_line: None,
            editor_line_top: 0.0,
            editor_layout: (false, 0),
            editor_refollow: 0,
            editor_wheel: 0.0,
            typing: false,
            octave: 0,
            held: Vec::new(),
            key_preview: None,
            preview: None,
            patches: Vec::new(),
            lib_search: String::new(),
            lib_scroll_to: None,
            lib_category: patches::CAT_KEYS.to_owned(),
            loops: loops::library(),
            audio_loops: Vec::new(),
            loop_rects: Vec::new(),
            lane_left: 0.0,
            tracks_rect: Rect::NOTHING,
            loop_search: String::new(),
            loop_instrument: None,
            loop_genre: None,
            loop_mood: None,
            loop_favorites_only: false,
            auditioning: None,
            export_bits: 16,
            export_format: crate::ui_transfer::ExportFormat::Wav16,
            aligning: None,
            video: None,
            video_failed: false,
            video_rect: None,
            video_editor: None,
            video_position: -1.0,
            video_waiting: false,
            video_corrected: None,
            playhead_moved: false,
            editor_shares: Vec::new(),
            words_search: None,
            setlist: Default::default(),
            words_message: String::new(),
            words_draft: String::new(),
            words_line: None,
            words_split: egui::Rect::NOTHING,
            editor_splits: Vec::new(),
            window_handle: 0,
            library: Vec::new(),
            library_scan: None,
            tuner: None,
            input_level: 0.0,
            tuner_clock: 0.0,
        };
        app.editor_zoom = Some(app.settings.editor_zoom).filter(|zoom| *zoom > 0.0);
        app.rebuild_patches();
        app.refresh_themes();
        app
    }

    pub fn engine(&self) -> MutexGuard<'_, Engine> {
        self.audio.engine.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn rebuild_patches(&mut self) {
        self.patches = patches::builtin();
        self.patches.extend(self.settings.user_patches.iter().cloned());
        self.patches.extend(self.font_patches.iter().cloned());
    }

    // ----- plug-ins ---------------------------------------------------------------

    /// Whether plug-ins run each in a process of its own.
    pub fn plugins_isolated(&self) -> bool {
        #[cfg(test)]
        return self.isolate_for_test;
        #[cfg(not(test))]
        return !self.settings.plugins_in_process;
    }

    /// Chooses between plug-ins in processes of their own and plug-ins inside
    /// the application. Plug-ins already running stay as they are.
    pub fn set_plugins_isolated(&mut self, isolated: bool) {
        self.settings.plugins_in_process = !isolated;
        self.save_settings();
        self.status = tr("This applies to the plug-ins started from now on.").to_owned();
    }

    /// Starts again a plug-in that stopped or could not be started.
    pub fn restart_plugin(&mut self, slot_id: u64) {
        self.plugin_failed.retain(|id| *id != slot_id);
        self.plugin_handles.remove(&slot_id);
        self.engine().remove_unit(slot_id);
    }

    /// Starts the plug-in of a slot, in the state and with the parameter values
    /// the slot remembers.
    fn start_plugin(&mut self, slot: &PluginSlot, sample_rate: f64) -> Result<StartedPlugin, String> {
        let path = Path::new(&slot.file);
        let (mut unit, handle, params): StartedPlugin = if self.plugins_isolated() {
            let (unit, handle, params) = crate::isolate::start(path, &slot.plugin, sample_rate)?;
            (Box::new(unit), Some(Box::new(handle)), params)
        } else {
            self.libraries.load(path, &slot.plugin, sample_rate)?
        };
        if let (Some(handle), Some(state)) = (&handle, slot.state.as_deref().and_then(from_hex)) {
            handle.load_state(&state);
        }
        for (id, value) in &slot.params {
            unit.set_param(*id, *value);
        }
        Ok((unit, handle, params))
    }

    /// Asks every running plug-in for its state and keeps it in the project, so
    /// that what was set in a plug-in's own window is saved and exported.
    /// `window_closed` is set when a plug-in's window was just used: what was
    /// set there replaces what the host's sliders had set before.
    pub fn capture_plugin_states(&mut self, window_closed: bool) {
        let states: Vec<(u64, Vec<u8>)> = self.plugin_handles.iter().filter_map(|(id, handle)| Some((*id, handle.save_state()?))).collect();
        for (id, state) in states {
            let text = to_hex(&state);
            let slots = self.project.tracks.iter_mut().flat_map(|t| t.plugins.iter_mut().chain(&mut t.instrument));
            for slot in slots.filter(|s| s.id == id) {
                if slot.state.as_deref() != Some(text.as_str()) {
                    slot.state = Some(text.clone());
                }
                // Slider values are otherwise kept: they are applied after the
                // state, and may not have reached the plug-in yet.
                if window_closed {
                    slot.params.clear();
                }
            }
        }
    }

    /// Opens or closes the window of a plug-in.
    pub fn toggle_plugin_window(&mut self, slot_id: u64, title: &str) {
        let Some(handle) = self.plugin_handles.get_mut(&slot_id) else { return };
        if handle.window_open() {
            handle.close_window();
            self.capture_plugin_states(true);
        } else if let Err(e) = handle.open_window(title) {
            self.status = tf!("Plug-in window: {e}" [e]);
        }
    }

    /// Lets plug-ins do what they asked of the main thread.
    fn idle_plugins(&mut self) {
        let mut closed = false;
        let mut changes = Vec::new();
        let mut failed = Vec::new();
        for (id, handle) in &mut self.plugin_handles {
            let idle = handle.idle();
            closed |= idle.closed;
            changes.extend(idle.changes.into_iter().map(|(param, value)| (*id, param, value)));
            failed.extend(idle.failed.map(|reason| (*id, reason)));
        }
        // A plug-in whose process stopped: the project goes on without it until
        // it is started again.
        for (id, reason) in failed {
            self.plugin_handles.remove(&id);
            if !self.plugin_failed.contains(&id) {
                self.plugin_failed.push(id);
            }
            let name = self.project.tracks.iter().flat_map(|t| t.plugins.iter().chain(&t.instrument)).find(|s| s.id == id).map_or_else(String::new, |s| s.name.clone());
            self.status = tf!("Plug-in “{}”: {}", name, reason);
        }
        // What was turned in a plug-in's own window is a setting like any other.
        for (slot, param, value) in changes {
            self.set_plugin_param(slot, param, value);
        }
        if closed {
            self.capture_plugin_states(true);
        }
    }

    /// Starts the plug-ins the project refers to and that are not running yet
    /// (new slots, a project just opened, an undo that brought a slot back).
    fn ensure_plugins(&mut self) {
        let slots: Vec<PluginSlot> =
            self.project.tracks.iter().flat_map(|t| t.plugins.iter().chain(&t.instrument)).cloned().collect();
        self.plugin_failed.retain(|id| slots.iter().any(|s| s.id == *id));
        // A plug-in whose slot is gone loses its window with it.
        self.plugin_handles.retain(|id, _| slots.iter().any(|s| s.id == *id));
        for slot in slots {
            if self.plugin_failed.contains(&slot.id) || self.engine().has_unit(slot.id) {
                continue;
            }
            match self.start_plugin(&slot, f64::from(self.audio.sample_rate)) {
                Ok((instance, handle, params)) => {
                    self.plugin_params.insert(slot.id, params);
                    if let Some(handle) = handle {
                        self.plugin_handles.insert(slot.id, handle);
                    }
                    self.engine().add_unit(slot.id, instance);
                }
                Err(e) => {
                    self.plugin_failed.push(slot.id);
                    self.status = tf!("Plug-in “{}” unavailable: {e}" [e], slot.name);
                }
            }
        }
    }

    /// Opens the list of installed plug-ins, to add one to the selected track.
    pub fn pick_plugin(&mut self) {
        self.dialog = Some(Dialog::PluginPicker);
    }

    /// Every plug-in file the application knows about: downloaded ones, those
    /// imported by hand, and those in the folders where CLAP plug-ins are
    /// normally installed.
    pub fn known_plugin_files(&self) -> Vec<PathBuf> {
        let mut files = downloads::find_files(&self.effects_dir(), &["clap"]);
        files.extend(downloads::find_bundles(&self.effects_dir()));
        files.extend(downloads::find_vst3(&self.effects_dir()));
        files.extend(self.settings.plugins.iter().map(PathBuf::from).filter(|p| p.exists()));
        let standard = [
            ("COMMONPROGRAMFILES", "CLAP"),
            ("LOCALAPPDATA", "Programs\\Common\\CLAP"),
            ("COMMONPROGRAMFILES", "LV2"),
            ("APPDATA", "LV2"),
            ("COMMONPROGRAMFILES", "VST3"),
            ("LOCALAPPDATA", "Programs\\Common\\VST3"),
        ];
        for (variable, folder) in standard {
            if let Some(base) = std::env::var_os(variable).filter(|_| !cfg!(test)) {
                let dir = PathBuf::from(base).join(folder);
                files.extend(downloads::find_files(&dir, &["clap"]));
                files.extend(downloads::find_bundles(&dir));
                files.extend(downloads::find_vst3(&dir));
            }
        }
        files.sort();
        files.dedup();
        files
    }

    /// Loads a plug-in file and adds its plug-in to the selected track, or asks
    /// which one when the file holds several. Loading runs code from the file.
    pub fn open_plugin_file(&mut self, path: &Path) {
        match self.libraries.describe(path) {
            Ok(plugins) if plugins.len() == 1 => {
                self.add_plugin(path, &plugins[0]);
                self.dialog = None;
            }
            Ok(plugins) => self.dialog = Some(Dialog::Plugins { file: path.to_path_buf(), options: plugins }),
            Err(e) => self.status = tf!("Plug-in refused: {e}" [e]),
        }
    }

    /// Asks for an LV2 folder (`something.lv2`) and adds it to the installed effects.
    pub fn import_lv2_folder(&mut self) {
        let Some(path) = files::choose_folder(Some(tr("Folder of the LV2 plug-in (ends with .lv2)")), None) else { return };
        match lv2_host::describe(&path) {
            Ok(plugins) => {
                self.register_plugin_file(&path);
                self.status = tf!("LV2 plug-in added to the installed effects: {}.", plugins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>().join(", "));
            }
            Err(e) => self.status = tf!("Plug-in refused: {e}" [e]),
        }
    }

    /// Remembers a plug-in file that lives outside the application's folders.
    pub fn register_plugin_file(&mut self, path: &Path) {
        let text = path.to_string_lossy().into_owned();
        if !self.settings.plugins.contains(&text) {
            self.settings.plugins.push(text);
            self.save_settings();
        }
    }

    /// Asks for a VST3 plug-in and adds it to the installed effects. A VST3
    /// plug-in that is a folder is chosen by the file of the same name inside it.
    pub fn import_vst3_file(&mut self) {
        let Some(path) = files::open_file(tr("VST3 plug-in"), &["vst3"]) else { return };
        // The folder `X.vst3/Contents/x86_64-win/X.vst3` is the plug-in, not the file.
        let bundle = path.ancestors().nth(3).filter(|folder| is_vst3(folder) && folder.join("Contents").is_dir()).map(Path::to_path_buf);
        self.register_plugin_file(&bundle.unwrap_or(path));
        self.status = tr("Plug-in added to the list of installed effects.").to_owned();
    }

    pub fn import_plugin_file(&mut self) {
        let Some(path) = files::open_file(tr("CLAP plug-in"), &["clap"]) else { return };
        self.register_plugin_file(&path);
        self.status = tr("Plug-in added to the list of installed effects.").to_owned();
    }

    /// Adds a plug-in to the selected track: an instrument replaces the track's
    /// sound, an effect goes at the end of its chain.
    pub fn add_plugin(&mut self, file: &Path, info: &PluginInfo) {
        let ti = self.selected_track;
        let Some(track) = self.project.tracks.get(ti) else { return };
        if track.locked {
            return;
        }
        if info.instrument && track.kind != TrackKind::Instrument {
            self.status = tr("An external instrument goes on a software instrument track.").to_owned();
            return;
        }
        self.checkpoint();
        self.slot_counter += 1;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos() as u64);
        let slot = PluginSlot {
            id: (stamp << 8) | (self.slot_counter & 0xFF),
            file: file.to_string_lossy().into_owned(),
            plugin: info.id.clone(),
            name: info.name.clone(),
            on: true,
            params: Vec::new(),
            state: None,
        };
        let track = &mut self.project.tracks[ti];
        if info.instrument {
            track.instrument = Some(slot);
        } else {
            track.plugins.push(slot);
        }
        self.status = tf!("Plug-in “{}” added to the track.", info.name);
    }

    pub fn remove_plugin(&mut self, slot_id: u64) {
        self.checkpoint();
        for track in &mut self.project.tracks {
            track.plugins.retain(|s| s.id != slot_id);
            if track.instrument.as_ref().is_some_and(|s| s.id == slot_id) {
                track.instrument = None;
            }
        }
        self.plugin_params.remove(&slot_id);
        self.plugin_handles.remove(&slot_id);
    }

    pub fn set_plugin_param(&mut self, slot_id: u64, param: u32, value: f64) {
        let slots = self.project.tracks.iter_mut().flat_map(|t| t.plugins.iter_mut().chain(&mut t.instrument));
        for slot in slots.filter(|s| s.id == slot_id) {
            slot.params.retain(|p| p.0 != param);
            slot.params.push((param, value));
        }
        if let Some(handle) = self.plugin_handles.get_mut(&slot_id) {
            handle.param_changed(param, value);
        }
        self.engine().unit_param(slot_id, param, value);
    }

    #[cfg(test)]
    pub fn render_song_for_test(&mut self) -> Vec<f32> {
        self.render_song()
    }

    /// Renders the song for export, with fresh instances of the plug-ins.
    fn render_song(&mut self) -> Vec<f32> {
        self.capture_plugin_states(false);
        let (project, clips, fonts, captures) = (self.project.clone(), self.clips.clone(), self.fonts.clone(), self.captures.clone());
        render_offline_full(&project, &clips, &fonts, &captures, EXPORT_RATE, &mut |slot| {
            // The export's own instance needs no window: its handle is let go.
            let (instance, _, _) = self.start_plugin(slot, f64::from(EXPORT_RATE)).ok()?;
            Some(instance)
        })
    }

    // ----- language ---------------------------------------------------------------

    /// Code of the language of the interface.
    pub fn language(&self) -> String {
        crate::i18n::language()
    }

    /// Switches the interface to another language and remembers it.
    pub fn set_language(&mut self, code: &str) {
        crate::i18n::set_language(code);
        self.settings.language = if code == crate::i18n::SOURCE { String::new() } else { code.to_owned() };
        self.save_settings();
    }

    // ----- themes -----------------------------------------------------------------

    /// Folder of the themes the user added.
    pub fn themes_dir(&self) -> PathBuf {
        match self.settings_path.as_ref().and_then(|path| path.parent()) {
            Some(folder) => folder.join("themes"),
            None => self.data_dir.join("themes"),
        }
    }

    /// Name of the theme in use.
    pub fn theme_id(&self) -> &str {
        if self.settings.theme.is_empty() {
            crate::theme::DEFAULT
        } else {
            &self.settings.theme
        }
    }

    /// Reads the themes again and takes the chosen one; the program's own if
    /// the chosen one is gone.
    pub fn refresh_themes(&mut self) {
        self.themes = crate::theme::all(&self.themes_dir());
        let wanted = self.theme_id().to_owned();
        let found = self.themes.iter().find(|t| t.id == wanted).or_else(|| self.themes.first()).cloned();
        if let Some(theme) = found {
            self.theme = Arc::new(theme);
            crate::theme::set(Arc::clone(&self.theme));
        }
        self.theme_applied = false;
    }

    pub fn set_theme(&mut self, id: &str) {
        self.settings.theme = if id == crate::theme::DEFAULT { String::new() } else { id.to_owned() };
        self.refresh_themes();
        self.save_settings();
    }

    /// Copies a theme file among the user's themes and switches to it.
    pub fn install_theme(&mut self, file: &Path) -> Result<String, String> {
        let id = file.file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty()).ok_or_else(|| tr("Not a theme file").to_owned())?;
        let text = std::fs::read_to_string(file).map_err(|e| e.to_string())?;
        let theme = Theme::parse(&id, &text)?;
        let folder = self.themes_dir();
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        std::fs::write(folder.join(format!("{id}.json")), text).map_err(|e| e.to_string())?;
        self.set_theme(&id);
        Ok(theme.name)
    }

    pub fn import_theme(&mut self) {
        let Some(file) = files::open_file(tr("Theme (JSON)"), &["json"]) else { return };
        self.status = match self.install_theme(&file) {
            Ok(name) => tf!("Theme added: {name}" [name]),
            Err(e) => tf!("This theme cannot be read: {e}" [e]),
        };
    }

    /// Writes the theme in use as a file that gives every colour.
    pub fn save_theme_model(&mut self) {
        let name = format!("{}-copy.json", self.theme.id);
        let Some(file) = files::save_file(tr("Theme (JSON)"), &["json"], &name) else { return };
        self.status = match std::fs::write(&file, self.theme.to_json()) {
            Ok(()) => tf!("Theme written to {}. Change its colours, then import it.", file.display()),
            Err(e) => tf!("Could not write the file: {e}" [e]),
        };
    }

    // ----- panels of the sound controls -------------------------------------------
    /// Every panel, in the rows the user arranged them in. A panel the saved
    /// layout does not know joins the last row.
    pub fn panel_rows(&self) -> Vec<Vec<Panel>> {
        let mut seen: Vec<Panel> = Vec::new();
        let mut rows: Vec<Vec<Panel>> = Vec::new();
        for row in &self.settings.panel_rows {
            let mut kept = Vec::new();
            for panel in row {
                if !seen.contains(panel) {
                    seen.push(*panel);
                    kept.push(*panel);
                }
            }
            if !kept.is_empty() {
                rows.push(kept);
            }
        }
        if rows.is_empty() {
            return Panel::default_rows();
        }
        let last = rows.len() - 1;
        rows[last].extend(Panel::ALL.into_iter().filter(|panel| !seen.contains(panel)));
        rows
    }

    /// Every panel, row after row.
    fn panel_sequence(&self) -> Vec<Panel> {
        self.panel_rows().concat()
    }

    /// The rows of panels shown for a track; rows with nothing to show are left out.
    pub fn visible_rows(&self, ti: usize) -> Vec<Vec<Panel>> {
        self.panel_rows()
            .into_iter()
            .map(|row| row.into_iter().filter(|p| self.panel_applies(ti, *p) && !self.settings.panels_hidden.contains(p)).collect::<Vec<_>>())
            .filter(|row| !row.is_empty())
            .collect()
    }

    /// Whether a panel has anything to offer for this track.
    fn panel_applies(&self, ti: usize, panel: Panel) -> bool {
        let Some(track) = self.project.tracks.get(ti) else { return false };
        match panel {
            Panel::Sound => !track.kind.is_audio(),
            Panel::Arp => track.kind == TrackKind::Instrument && !track.sound.is_drums(),
            Panel::Amp => track.kind.is_audio(),
            _ => true,
        }
    }

    /// The panels shown for a track, in order.
    #[cfg(test)]
    pub fn visible_panels(&self, ti: usize) -> Vec<Panel> {
        self.panel_sequence().into_iter().filter(|p| self.panel_applies(ti, *p) && !self.settings.panels_hidden.contains(p)).collect()
    }

    /// The panels this track would show had the user not hidden them.
    pub fn hidden_panels(&self, ti: usize) -> Vec<Panel> {
        self.panel_sequence().into_iter().filter(|p| self.panel_applies(ti, *p) && self.settings.panels_hidden.contains(p)).collect()
    }

    pub fn hide_panel(&mut self, panel: Panel) {
        if !self.settings.panels_hidden.contains(&panel) {
            self.settings.panels_hidden.push(panel);
            self.save_settings();
            self.status = tf!("“{}” is hidden. Right-click the background of the controls to show it again.", panel.label());
        }
    }

    pub fn show_panel(&mut self, panel: Panel) {
        self.settings.panels_hidden.retain(|p| *p != panel);
        self.save_settings();
    }

    pub fn show_all_panels(&mut self) {
        self.settings.panels_hidden.clear();
        self.save_settings();
    }

    /// Puts `dragged` next to `target`: before or after it in its row, or alone
    /// in a new row above or below that row. The other panels close the gap.
    pub fn place_panel(&mut self, dragged: Panel, target: Panel, side: Side) {
        if dragged == target {
            return;
        }
        let mut rows = self.panel_rows();
        for row in &mut rows {
            row.retain(|p| *p != dragged);
        }
        let Some((r, at)) = rows.iter().enumerate().find_map(|(r, row)| Some((r, row.iter().position(|p| *p == target)?))) else { return };
        match side {
            Side::Before => rows[r].insert(at, dragged),
            Side::After => rows[r].insert(at + 1, dragged),
            Side::Above => rows.insert(r, vec![dragged]),
            Side::Below => rows.insert(r + 1, vec![dragged]),
        }
        rows.retain(|row| !row.is_empty());
        self.settings.panel_rows = rows;
        self.save_settings();
    }

    /// Moves a panel one place left (-1) or right (1) in its row.
    pub fn move_panel(&mut self, panel: Panel, by: isize, ti: usize) {
        let rows = self.visible_rows(ti);
        let Some((row, at)) = rows.iter().find_map(|row| Some((row, row.iter().position(|p| *p == panel)?))) else { return };
        if let Some(target) = at.checked_add_signed(by).and_then(|i| row.get(i)) {
            self.place_panel(panel, *target, if by < 0 { Side::Before } else { Side::After });
        }
    }

    /// Moves a panel to the end of the row above (-1) or below (1); past the
    /// first or the last row, it gets a row of its own.
    pub fn move_panel_row(&mut self, panel: Panel, by: isize, ti: usize) {
        let rows = self.visible_rows(ti);
        let Some(r) = rows.iter().position(|row| row.contains(&panel)) else { return };
        match r.checked_add_signed(by).and_then(|i| rows.get(i)) {
            Some(row) => self.place_panel(panel, row[row.len() - 1], Side::After),
            None if rows[r].len() > 1 => {
                let neighbour = rows[r].iter().copied().find(|p| *p != panel).unwrap_or(panel);
                self.place_panel(panel, neighbour, if by < 0 { Side::Above } else { Side::Below });
            }
            None => {}
        }
    }

    // ----- sound banks ------------------------------------------------------------

    /// Where a bank file that is no longer at `path` is now: a file of the same
    /// name in the folder of downloaded banks.
    fn relocated_bank(&self, path: &Path) -> Option<PathBuf> {
        let name = path.file_name()?.to_string_lossy().to_lowercase();
        downloads::find_files(&self.banks_dir(), &BANK_EXTENSIONS)
            .into_iter()
            .find(|file| file.file_name().is_some_and(|n| n.to_string_lossy().to_lowercase() == name))
    }

    /// Follows the installed banks whose folder has moved.
    fn relocate_installed_banks(&mut self) {
        let mut moved = false;
        for i in 0..self.settings.banks.len() {
            let path = PathBuf::from(&self.settings.banks[i].path);
            if path.exists() {
                continue;
            }
            if let Some(found) = self.relocated_bank(&path) {
                self.settings.banks[i].path = found.to_string_lossy().into_owned();
                moved = true;
            }
        }
        if moved {
            let mut seen: Vec<String> = Vec::new();
            self.settings.banks.retain(|bank| {
                let key = bank.path.to_lowercase();
                let first = !seen.contains(&key);
                seen.push(key);
                first
            });
            self.save_settings();
        }
    }

    /// The project used the bank `old`, which is now the bank `new` at `path`.
    fn remap_font(&mut self, old: u64, new: u64, path: &Path) {
        for track in &mut self.project.tracks {
            if let Sound::Sampler(sampler) = &mut track.sound {
                if sampler.font == old {
                    sampler.font = new;
                }
            }
        }
        let known = self.project.fonts.iter().any(|f| f.id == new);
        self.project.fonts.retain(|f| f.id != old || !known);
        for font in &mut self.project.fonts {
            if font.id == old {
                font.id = new;
                font.path = path.to_string_lossy().into_owned();
            }
        }
    }

    /// How the user asked the instruments of an installed bank to be filed;
    /// a bank that was not installed gets a section of its own.
    pub fn installed_section(&self, path: &Path) -> Section {
        let wanted = path.to_string_lossy().to_lowercase().replace('\\', "/");
        self.settings
            .banks
            .iter()
            .find(|b| b.path.to_lowercase().replace('\\', "/") == wanted)
            .map_or(Section::Own, |b| b.section.clone())
    }

    /// Loads a sound bank file into the library: filed as the user asked when
    /// they installed it, else in a section of its own.
    pub fn load_font(&mut self, path: &Path) -> Result<u64, String> {
        let section = self.installed_section(path);
        self.load_font_in(path, &section)
    }

    /// Loads a sound bank file and files its instruments in the library as
    /// `section` says.
    pub fn load_font_in(&mut self, path: &Path, section: &Section) -> Result<u64, String> {
        let id = font_id(path);
        if self.fonts.contains_key(&id) {
            return Ok(id);
        }
        let font = load_bank(path)?;
        Ok(self.adopt_bank(path, section, font))
    }

    /// Adds a bank that has been read from disk to the library.
    fn adopt_bank(&mut self, path: &Path, section: &Section, font: og_engine::Bank) -> u64 {
        let id = font_id(path);
        if self.fonts.contains_key(&id) {
            return id;
        }
        let name = path.file_stem().map_or_else(|| tr("Bank").to_owned(), |s| s.to_string_lossy().into_owned());
        self.file_bank(id, &name, section, &font);
        self.fonts.insert(id, font);
        self.fonts_dirty = true;
        self.font_files.push(FileRef { id, name, path: path.to_string_lossy().into_owned() });
        self.rebuild_patches();
        id
    }

    /// Puts the instruments of a bank in the library, where `section` says.
    fn file_bank(&mut self, id: u64, name: &str, section: &Section, font: &og_engine::Bank) {
        // An SFZ file is a single instrument: they share one section by default.
        let sfz = matches!(font, og_engine::Bank::Sfz(_));
        for (preset_name, bank, preset) in bank_presets(font) {
            let category = match section {
                Section::Category(category) => category.clone(),
                _ if sfz => patches::CAT_SFZ.to_owned(),
                Section::Own => name.to_owned(),
                Section::Auto => gm_category(bank, preset).to_owned(),
            };
            self.font_patches.push(PatchDef {
                name: if preset_name.is_empty() { tf!("Preset {bank}:{preset}" [bank, preset]) } else { preset_name },
                category,
                sound: Sound::Sampler(SamplerParams { font: id, bank, preset, gain: 1.0 }),
                fx: og_engine::TrackFx::default(),
            });
        }
    }

    /// Files again the instruments of a bank that is already loaded, as
    /// `section` says: a bank installed once more goes where it is now asked.
    fn refile_bank(&mut self, id: u64, section: &Section) {
        let Some((font, name)) = self.fonts.remove(&id).map(|font| (font, self.font_files.iter().find(|f| f.id == id).map_or_else(|| tr("Bank").to_owned(), |f| f.name.clone()))) else { return };
        self.font_patches.retain(|p| !matches!(p.sound, Sound::Sampler(s) if s.font == id));
        self.file_bank(id, &name, section, &font);
        self.fonts.insert(id, font);
        self.rebuild_patches();
    }

    /// Adds a bank to the library for good: it is loaded again at each start.
    /// A bank that is there already is filed again as `section` says.
    pub fn install_bank(&mut self, path: &Path, section: &Section) -> Result<usize, String> {
        let loaded = self.fonts.contains_key(&font_id(path));
        let id = self.load_font_in(path, section)?;
        if loaded {
            self.refile_bank(id, section);
        }
        let text = path.to_string_lossy().into_owned();
        match self.settings.banks.iter_mut().find(|b| b.path == text) {
            Some(entry) => entry.section = section.clone(),
            None => self.settings.banks.push(BankEntry { path: text, section: section.clone() }),
        }
        self.save_settings();
        Ok(self.font_patches.iter().filter(|p| matches!(p.sound, Sound::Sampler(s) if s.font == id)).count())
    }

    /// Whether a track plays a guitar or a bass: a tablature can show its notes.
    pub fn is_guitar_track(&self, ti: usize) -> bool {
        let Some(track) = self.project.tracks.get(ti) else { return false };
        if track.kind != TrackKind::Instrument {
            return false;
        }
        if track.icon == 3 {
            return true;
        }
        if let Sound::Sampler(sampler) = track.sound {
            if sampler.bank < 128 && (24..=39).contains(&sampler.preset) {
                return true;
            }
        }
        self.patches.iter().find(|p| p.name == track.patch).is_some_and(|p| p.category == patches::CAT_GUITAR || p.category == patches::CAT_BASS)
    }

    /// Whether a track plays a bass: its tablature has four strings.
    pub fn is_bass_track(&self, ti: usize) -> bool {
        let Some(track) = self.project.tracks.get(ti) else { return false };
        if track.kind != TrackKind::Instrument {
            return false;
        }
        if let Sound::Sampler(sampler) = track.sound {
            if sampler.bank < 128 && (32..=39).contains(&sampler.preset) {
                return true;
            }
        }
        self.patches.iter().find(|p| p.name == track.patch).is_some_and(|p| p.category == patches::CAT_BASS)
    }

    /// Shows the library on the sound of a track, in its section.
    pub fn reveal_patch(&mut self, ti: usize) {
        self.select_track(ti);
        self.show_library = true;
        self.lib_search.clear();
        let Some(name) = self.project.tracks.get(ti).map(|t| t.patch.clone()) else { return };
        if let Some(patch) = self.patches.iter().find(|p| p.name == name) {
            self.lib_category.clone_from(&patch.category);
            self.lib_scroll_to = Some(name);
        }
    }

    /// The instrument of an installed bank for a General MIDI program (or
    /// `og_engine::songsterr::DRUMS` for a drum kit), if any bank has it.
    pub fn gm_patch(&self, program: u32) -> Option<&PatchDef> {
        let wanted = |sound: &Sound| match sound {
            Sound::Sampler(s) if program == og_engine::songsterr::DRUMS => s.bank == 128 && s.preset == 0,
            Sound::Sampler(s) => s.bank == 0 && u32::from(s.preset) == program,
            _ => false,
        };
        self.font_patches.iter().find(|p| wanted(&p.sound)).or_else(|| {
            // Any variation bank, or any drum kit, rather than a synthesized sound.
            self.font_patches.iter().find(|p| match &p.sound {
                Sound::Sampler(s) if program == og_engine::songsterr::DRUMS => s.bank == 128,
                Sound::Sampler(s) => s.bank < 128 && u32::from(s.preset) == program,
                _ => false,
            })
        })
    }

    /// Gives the tracks of a project the real instruments of an installed bank
    /// for their General MIDI programs; tracks whose program no bank has keep
    /// their built-in sound. Returns how many tracks got one.
    pub fn assign_gm_sounds(&self, project: &mut Project, programs: &[u32]) -> usize {
        let mut given = 0;
        for (track, program) in project.tracks.iter_mut().zip(programs) {
            let Some(patch) = self.gm_patch(*program) else { continue };
            track.patch.clone_from(&patch.name);
            track.sound = patch.sound;
            if let Sound::Sampler(sampler) = patch.sound {
                if !project.fonts.iter().any(|f| f.id == sampler.font) {
                    if let Some(file) = self.font_files.iter().find(|f| f.id == sampler.font) {
                        project.fonts.push(file.clone());
                    }
                }
            }
            given += 1;
        }
        given
    }

    pub fn import_soundfont(&mut self) {
        let Some(path) = files::open_file(tr("Sound bank (SF2, SF3, SFZ)"), &BANK_EXTENSIONS) else { return };
        match self.install_bank(&path, &Section::Own) {
            Ok(count) => {
                if let Some(patch) = self.font_patches.iter().rev().find(|p| matches!(p.sound, Sound::Sampler(s) if s.font == font_id(&path))) {
                    self.lib_category.clone_from(&patch.category);
                }
                self.show_library = true;
                self.status = tf!("Sound bank loaded: {count} instruments added to the library." [count]);
            }
            Err(e) => self.status = tf!("Import failed: {e}" [e]),
        }
    }

    /// Library sections that exist only because of loaded banks, in loading order.
    pub fn font_names(&self) -> Vec<String> {
        let builtin = [
            patches::CAT_KEYS,
            patches::CAT_SYNTH,
            patches::CAT_BASS,
            patches::CAT_GUITAR,
            patches::CAT_ORCH,
            patches::CAT_MALLET,
            patches::CAT_DRUMS,
            patches::CAT_USER,
            patches::CAT_AMP,
            patches::CAT_VOICE,
        ];
        let mut names: Vec<String> = Vec::new();
        for patch in &self.font_patches {
            if !builtin.contains(&patch.category.as_str()) && !names.contains(&patch.category) {
                names.push(patch.category.clone());
            }
        }
        names
    }

    /// Loads the banks the user installed (imports and downloads).
    pub fn load_installed_banks(&mut self) {
        // Settings written by earlier versions listed plain paths.
        for path in std::mem::take(&mut self.settings.soundfonts) {
            if !self.settings.banks.iter().any(|b| b.path == path) {
                self.settings.banks.push(BankEntry { path, section: Section::Own });
            }
        }
        self.relocate_installed_banks();
        // Large banks take seconds to read: that happens on another thread, so
        // that the window opens at once. Each bank joins the library when ready.
        let banks = self.settings.banks.clone();
        if banks.is_empty() {
            return;
        }
        let (send, receive) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for bank in banks {
                // A bank that has been moved or deleted is simply not offered.
                let loaded = load_bank(Path::new(&bank.path)).ok();
                if send.send((bank, loaded)).is_err() {
                    break;
                }
            }
        });
        self.bank_loading = Some(receive);
    }

    /// Takes in the banks that the loading thread has finished reading, and
    /// says how many came.
    pub fn poll_bank_loading(&mut self) -> usize {
        let Some(receive) = &self.bank_loading else { return 0 };
        let mut arrived = Vec::new();
        let finished = loop {
            match receive.try_recv() {
                Ok(item) => arrived.push(item),
                Err(std::sync::mpsc::TryRecvError::Empty) => break false,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
            }
        };
        let count = arrived.len();
        for (bank, loaded) in arrived {
            if let Some(font) = loaded {
                let id = font_id(Path::new(&bank.path));
                if self.fonts.contains_key(&id) {
                    // Loaded already, for a project that uses it: filed as installed.
                    self.refile_bank(id, &bank.section);
                } else {
                    self.adopt_bank(Path::new(&bank.path), &bank.section, font);
                }
            }
        }
        if finished {
            self.bank_loading = None;
        }
        count
    }

    /// Whether installed banks are still being read at start-up.
    pub fn banks_loading(&self) -> bool {
        self.bank_loading.is_some()
    }

    // ----- downloads --------------------------------------------------------------

    #[cfg(test)]
    pub fn set_data_dir_for_test(&mut self, dir: &Path) {
        self.data_dir = dir.to_path_buf();
        self.keep_sessions = true;
    }

    /// Folder that holds everything downloaded.
    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Folder where downloaded sound banks are kept.
    pub fn banks_dir(&self) -> PathBuf {
        self.data_dir.join(downloads::Kind::Banks.folder())
    }

    /// Folder where downloaded effects and instruments (plug-ins) are kept.
    pub fn effects_dir(&self) -> PathBuf {
        self.data_dir.join(downloads::Kind::Effects.folder())
    }

    pub fn captures_dir(&self) -> PathBuf {
        self.data_dir.join(downloads::Kind::Captures.folder())
    }

    pub fn impulses_dir(&self) -> PathBuf {
        self.data_dir.join(downloads::Kind::Impulses.folder())
    }

    /// Opens the download window on sounds or on plug-ins.
    pub fn open_downloads(&mut self, kind: downloads::Kind) {
        self.dialog = Some(Dialog::Downloads);
        let wrong_kind = self.downloads.provider.kind() != kind;
        if wrong_kind {
            self.downloads.provider = downloads::Provider::of(kind)[0];
        }
        if wrong_kind || (self.downloads.listing.is_none() && self.downloads.pending.is_none()) {
            self.refresh_downloads();
        }
    }

    pub fn refresh_downloads(&mut self) {
        let ui = &mut self.downloads;
        ui.selected = None;
        ui.trusted = false;
        ui.have.clear();
        ui.resolving = None;
        ui.blocked = None;
        ui.listing = None;
        ui.pending = ui.provider.direct().then(|| downloads::start_listing(ui.provider, ui.search.clone()));
    }

    pub fn start_download(&mut self, item: downloads::Item) {
        if self.downloads.job.is_none() {
            self.downloads.message.clear();
            self.downloads.job = Some(downloads::start(item, self.data_dir.clone()));
        }
    }

    /// Notes which of the listed items are already on the disk.
    pub fn mark_downloaded(&mut self) {
        self.downloads.have = match &self.downloads.listing {
            Some(Ok(items)) => items.iter().map(|item| downloads::is_downloaded(&self.data_dir, item)).collect(),
            _ => Vec::new(),
        };
    }

    /// Selects an item of the list. Sites that only give a list of projects
    /// are asked, for that item alone, about its licence and its files.
    pub fn select_download(&mut self, index: usize) {
        let ui = &mut self.downloads;
        ui.selected = Some(index);
        ui.trusted = false;
        ui.blocked = None;
        ui.resolving = None;
        let item = ui.listing.as_ref().and_then(|l| l.as_ref().ok()).and_then(|items| items.get(index));
        if let Some(item) = item.filter(|item| item.url.is_empty()) {
            ui.resolving = Some((index, downloads::start_resolving(item.clone())));
        }
    }

    /// Takes in what is now known about an item that was being looked up.
    pub fn resolved_download(&mut self, index: usize, result: Result<downloads::Item, String>) {
        match result {
            Ok(item) => {
                if let Some(Ok(items)) = &mut self.downloads.listing {
                    if let Some(slot) = items.get_mut(index) {
                        *slot = item;
                    }
                }
            }
            Err(reason) => self.downloads.blocked = Some((index, reason)),
        }
    }

    /// Collects what the download threads have finished.
    fn poll_downloads(&mut self) {
        // A thread that died is an answer too: the window stops waiting.
        let stopped = tr("the download stopped unexpectedly");
        fn answered<T>(receive: &std::sync::mpsc::Receiver<Result<T, String>>, stopped: &str) -> Option<Result<T, String>> {
            match receive.try_recv() {
                Ok(result) => Some(result),
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err(stopped.to_owned())),
            }
        }
        if let Some(listing) = take_result(&mut self.downloads.pending, stopped) {
            self.downloads.listing = Some(listing);
            self.mark_downloaded();
        }
        let looked_up = self.downloads.resolving.as_ref().and_then(|(index, receive)| Some((*index, answered(receive, stopped)?)));
        if let Some((index, result)) = looked_up {
            self.downloads.resolving = None;
            self.resolved_download(index, result);
        }
        let finished = self.downloads.job.as_ref().and_then(|job| answered(&job.done, stopped));
        let Some(result) = finished else { return };
        let Some(job) = self.downloads.job.take() else { return };
        let section = self.downloads.section.clone();
        self.downloads.message = match result {
            Ok(files) if job.item.provider.kind() == downloads::Kind::Effects => {
                // Plug-ins are not loaded here: that happens when one is chosen.
                let names: Vec<String> = files.iter().filter_map(|f| f.file_stem()).map(|s| s.to_string_lossy().into_owned()).collect();
                tf!(
                    "“{}”: {} plug-in(s) installed ({}). Add them to a track from the sound controls, “Add a plug-in…”.",
                    job.item.name,
                    names.len(),
                    names.join(", ")
                )
            }
            Ok(files) => {
                let (mut instruments, mut failures) = (0, Vec::new());
                for file in &files {
                    match self.install_bank(file, &section) {
                        Ok(count) => instruments += count,
                        Err(e) => failures.push(e),
                    }
                }
                self.show_library = true;
                if instruments == 0 {
                    tf!("“{}” was downloaded but cannot be read: {}", job.item.name, failures.join(" ; "))
                } else {
                    tf!("“{}”: {instruments} instruments added to the library." [instruments], job.item.name)
                }
            }
            Err(e) => tf!("“{}”: {e}" [e], job.item.name),
        };
        self.mark_downloaded();
        self.status.clone_from(&self.downloads.message);
    }

    /// Opens a folder or a web page with the system's default program.
    pub fn open_external(&mut self, target: &str) {
        #[cfg(windows)]
        let launched = std::process::Command::new("explorer").arg(target).spawn();
        #[cfg(not(windows))]
        let launched = std::process::Command::new("xdg-open").arg(target).spawn();
        if let Err(e) = launched {
            self.status = tf!("Cannot open: {e}" [e]);
        }
    }

    pub fn save_settings(&mut self) {
        let Some(path) = &self.settings_path else { return };
        let result = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string()).and_then(|text| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::write(path, text).map_err(|e| e.to_string())
        });
        if let Err(e) = result {
            self.status = tf!("Settings not saved: {e}" [e]);
        }
    }

    // ----- undo and selection ---------------------------------------------------

    /// Remembers the current project so the next edit can be undone.
    pub fn checkpoint(&mut self) {
        self.explicit_checkpoint = true;
        self.push_undo(self.project.clone());
    }

    fn push_undo(&mut self, state: Project) {
        if self.undo.last() != Some(&state) {
            self.undo.push(state);
        }
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.project, previous));
            self.suppress_auto_undo = true;
            self.fix_selection();
        }
    }

    pub fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.project, next));
            self.suppress_auto_undo = true;
            self.fix_selection();
        }
    }

    pub fn fix_selection(&mut self) {
        self.drag = None;
        self.selected_notes.clear();
        self.selected_track = self.selected_track.min(self.project.tracks.len().saturating_sub(1));
        if let Some((ti, ri)) = self.selected_region {
            if self.project.tracks.get(ti).and_then(|t| t.regions.get(ri)).is_none() {
                self.selected_region = None;
            }
        }
    }

    pub fn region(&self, ti: usize, ri: usize) -> Option<&Region> {
        self.project.tracks.get(ti)?.regions.get(ri)
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selected_region.filter(|&(ti, ri)| self.region(ti, ri).is_some())
    }

    pub fn track_locked(&self, ti: usize) -> bool {
        self.project.tracks.get(ti).is_some_and(|t| t.locked)
    }

    /// Selects a track by its header. The editor follows: unless a region of
    /// that track is selected already, the one under the playhead is, or else
    /// the track's first; a track without regions leaves none selected.
    pub fn select_track(&mut self, ti: usize) {
        let Some(track) = self.project.tracks.get(ti) else { return };
        if self.selected_region.is_some_and(|(selected, _)| selected == ti) && self.selection().is_some() {
            self.selected_track = ti;
            return;
        }
        let position = self.position;
        let under = track.regions.iter().position(|r| r.start <= position && position < r.end());
        let first = (0..track.regions.len()).min_by(|a, b| track.regions[*a].start.total_cmp(&track.regions[*b].start));
        match under.or(first) {
            Some(ri) => self.select_region(ti, ri),
            None => {
                self.selected_track = ti;
                self.selected_region = None;
                self.selected_notes.clear();
            }
        }
    }

    pub fn select_region(&mut self, ti: usize, ri: usize) {
        self.selected_track = ti;
        if self.selected_region != Some((ti, ri)) {
            self.selected_region = Some((ti, ri));
            self.selected_notes.clear();
            // Open the editor centred on the region's notes.
            let Some(region) = self.region(ti, ri) else { return };
            let notes = region.notes();
            let centre = if notes.is_empty() {
                60.0
            } else {
                notes.iter().map(|n| f32::from(n.pitch)).sum::<f32>() / notes.len() as f32
            };
            self.roll_scroll_to = Some(((127.0 - centre) * 12.0 - 90.0).max(0.0));
        }
    }

    pub fn hit_region(&self, ti: usize, beat: f64) -> Option<usize> {
        self.project.tracks.get(ti)?.regions.iter().rposition(|r| beat >= r.start && beat < r.end())
    }

    pub fn curve(&self, id: CurveId) -> &[Point] {
        match id {
            CurveId::Tempo => &self.project.tempo_points,
            CurveId::Transpose => &self.project.transpose_points,
            CurveId::Master => &self.project.master_auto,
            CurveId::Volume(ti) => self.project.tracks.get(ti).map_or(&[][..], |t| t.vol_auto.as_slice()),
            CurveId::Pan(ti) => self.project.tracks.get(ti).map_or(&[][..], |t| t.pan_auto.as_slice()),
            CurveId::Fx(ti, param) => self.project.tracks.get(ti).map_or(&[][..], |t| t.fx_curve(param)),
        }
    }

    pub fn curve_mut(&mut self, id: CurveId) -> Option<&mut Vec<Point>> {
        Some(match id {
            CurveId::Tempo => &mut self.project.tempo_points,
            CurveId::Transpose => &mut self.project.transpose_points,
            CurveId::Master => &mut self.project.master_auto,
            CurveId::Volume(ti) => &mut self.project.tracks.get_mut(ti)?.vol_auto,
            CurveId::Pan(ti) => &mut self.project.tracks.get_mut(ti)?.pan_auto,
            CurveId::Fx(ti, param) => self.project.tracks.get_mut(ti)?.fx_curve_mut(param),
        })
    }

    // ----- transport ------------------------------------------------------------

    pub fn replace_project(&mut self, mut project: Project, path: Option<PathBuf>) {
        // The project open until now is kept as a session before it goes.
        self.keep_session();
        Self::give_id(&mut project, path.as_deref());
        // The words, and the recording, looked for were for the project that goes.
        self.words_search = None;
        self.words_message.clear();
        self.aligning = None;
        self.video_waiting = false;
        self.words_line = None;
        self.engine().stop();
        self.engine().stop_audition();
        self.auditioning = None;
        self.clean = Some(project.clone());
        self.project = project;
        self.path = path;
        self.undo.clear();
        self.redo.clear();
        self.suppress_auto_undo = true;
        self.selected_track = 0;
        self.selected_region = None;
        self.selected_notes.clear();
        self.drag = None;
        self.scroll = 0.0;
        self.clips.clear();
        self.waveforms.clear();
        self.clips_dirty = true;
        self.captures = Captures::default();
        self.captures_dirty = true;
        // A project opened or made from a file or a site is kept from the
        // next frame on, changed or not.
        self.blank = false;
        self.kept_at = f64::NEG_INFINITY;
        self.engine().set_position(0.0);
    }

    /// Sends pending changes to the engine. Edits made without an explicit
    /// checkpoint (sliders, knobs, text fields) become one undo step per burst.
    fn sync(&mut self, now: f64) {
        self.ensure_variants();
        if self.fonts_dirty {
            self.fonts_dirty = false;
            self.engine().set_fonts(self.fonts.clone());
        }
        if self.clips_dirty {
            self.clips_dirty = false;
            self.engine().set_clips(self.clips.clone());
        }
        if self.captures_dirty {
            self.captures_dirty = false;
            self.engine().set_captures(self.captures.clone());
        }
        if self.project != self.synced {
            if !self.explicit_checkpoint && !self.suppress_auto_undo && now - self.last_change > 0.8 {
                self.push_undo(self.synced.clone());
            }
            self.last_change = now;
            self.synced = self.project.clone();
            // As written, or with the original sound.
            self.engine().set_project(og_engine::recording::as_played(&self.synced));
        }
        self.explicit_checkpoint = false;
        self.suppress_auto_undo = false;
        self.ensure_plugins();
    }

    /// Makes the transposed or pitch-corrected copies of clips that regions
    /// ask for, and forgets the ones nothing uses any more.
    fn ensure_variants(&mut self) {
        let wanted: Vec<AudioRef> = self
            .project
            .tracks
            .iter()
            .flat_map(|t| t.regions.iter())
            .filter_map(|r| match &r.content {
                Content::Audio(audio) if audio.key() != audio.clip => Some(*audio),
                _ => None,
            })
            .collect();
        let before = self.clips.len();
        self.clips.retain(|key, _| key >> 63 == 0 || wanted.iter().any(|a| a.key() == *key));
        self.clips_dirty |= self.clips.len() != before;
        // Processing a whole clip takes a moment: wait until the knob is released.
        if self.pointer_down {
            return;
        }
        for audio in wanted {
            if self.clips.contains_key(&audio.key()) {
                continue;
            }
            let Some(base) = self.clips.get(&audio.clip).cloned() else { continue };
            let processed = stretch::process_pitch(&base, audio.transpose, f32::from(audio.tune) / 100.0);
            self.clips.insert(audio.key(), Arc::new(processed));
            self.clips_dirty = true;
        }
    }

    pub fn toggle_play(&mut self) {
        // Asked by the user: the song no longer waits for the video.
        self.video_waiting = false;
        let mut engine = self.engine();
        if engine.is_playing() {
            engine.stop();
        } else {
            engine.play();
        }
    }

    pub fn go_to(&mut self, beat: f64) {
        let beat = beat.max(0.0);
        self.engine().set_position(beat);
        self.position = beat;
        // The video, if one plays with the song, goes there too.
        self.playhead_moved = true;
    }

    /// Starts recording on the selected track, or stops the take in progress.
    pub fn toggle_record(&mut self) {
        if self.recording {
            self.engine().stop();
            return;
        }
        let ti = self.selected_track;
        let Some(track) = self.project.tracks.get(ti) else { return };
        if track.locked {
            self.status = tr("This track is locked.").to_owned();
            return;
        }
        match track.kind {
            TrackKind::Drummer => {
                self.status = tr("Drummer tracks cannot be recorded: set the drummer in the editor.").to_owned();
                return;
            }
            TrackKind::Video => {
                self.status = tr("A video track is not recorded: it plays the video of the song.").to_owned();
                return;
            }
            TrackKind::Audio | TrackKind::Guitar => {
                self.audio.input_channel = self.channel_for(ti);
                if let Err(e) = self.audio.ensure_input(&self.input_for(ti)) {
                    self.status = tf!("Cannot record: {e}" [e]);
                    return;
                }
                self.rec_audio = Some(false);
            }
            TrackKind::Instrument => self.rec_audio = None,
        }
        let count_in = self.project.count_in;
        self.sync(self.last_change);
        self.engine().record(ti, count_in);
        self.recording = true;
        self.status = tr("Recording… (Space or R to stop)").to_owned();
    }

    /// Starts an audio take without opening an input device.
    #[cfg(test)]
    pub fn begin_audio_take_for_test(&mut self, ti: usize) {
        self.engine().record(ti, false);
        self.recording = true;
        self.rec_audio = Some(true);
    }

    fn finish_recording(&mut self) {
        self.recording = false;
        let recorded = self.engine().take_recording();
        let audio_take = self.rec_audio.take();
        let Some(rec) = recorded else { return };
        let bar = self.project.bar_len();
        if audio_take.is_some() {
            let (samples, rate, lost) = self.audio.stop_capture();
            if samples.len() < rate as usize / 10 {
                self.status = tr("Nothing was recorded.").to_owned();
                return;
            }
            let seconds = samples.len() as f64 / f64::from(rate);
            let beat_seconds = 60.0 / self.project.tempo_at(rec.start);
            // With the cycle on, each pass over the cycle is a take of its own.
            let cycle = self.project.cycle_end - self.project.cycle_start;
            let pass = (cycle * beat_seconds * f64::from(rate)) as usize;
            let cycling = self.project.cycle_enabled && cycle > 0.0 && pass > 0 && (rec.start - self.project.cycle_start).abs() < 0.26;
            let passes: Vec<Vec<f32>> = if cycling && samples.len() > pass + pass / 4 {
                samples.chunks(pass).filter(|c| c.len() >= pass / 2).map(<[f32]>::to_vec).collect()
            } else {
                vec![samples]
            };
            self.checkpoint();
            let mut takes = Vec::new();
            for pass in passes {
                match self.store_clip(tr("Take"), ClipData { sample_rate: rate, channels: 1, samples: pass }) {
                    Ok(id) => takes.push(Content::Audio(AudioRef::new(id))),
                    Err(e) => {
                        self.status = tf!("The take could not be written to disk: {e}" [e]);
                        return;
                    }
                }
            }
            let Some(current) = takes.pop() else { return };
            let count = takes.len() + 1;
            let length = if count > 1 { cycle } else { (seconds / beat_seconds).max(0.25) };
            let mut region = Region::new(tr("Take"), rec.start, length);
            region.content = current;
            region.takes = takes;
            self.push_region(rec.track, region);
            self.status = if lost > 0 {
                let seconds = lost as f64 / f64::from(rate);
                tf!("Take recorded, but {seconds:.1} s of it was lost: the window stood still too long." [seconds])
            } else if count > 1 {
                tf!("{count} takes recorded; the last one is selected (choose in the editor)." [count])
            } else {
                tf!("Take of {seconds:.1} s recorded." [seconds])
            };
        } else if rec.notes.is_empty() {
            self.status = tr("No note was played during the recording.").to_owned();
        } else {
            let first = rec.notes.iter().map(|n| n.start).fold(rec.start, f64::min).floor();
            let last = rec.notes.iter().map(|n| n.start + n.length).fold(rec.end, f64::max);
            let end = ((last / bar).ceil() * bar).max(first + bar);
            self.checkpoint();
            let count = rec.notes.len();
            let mut region = Region::new(tr("Recording"), first, end - first);
            region.content = Content::Midi(rec.notes.into_iter().map(|n| Note { start: n.start - first, ..n }).collect());
            self.push_region(rec.track, region);
            self.status = tf!("{count} notes recorded." [count]);
        }
    }

    fn push_region(&mut self, ti: usize, region: Region) {
        if let Some(track) = self.project.tracks.get_mut(ti) {
            track.regions.push(region);
            let ri = track.regions.len() - 1;
            self.select_region(ti, ri);
        }
    }

    // ----- clips ------------------------------------------------------------------

    /// Keeps new audio as a clip of the project, written to the temporary folder
    /// until the project is saved.
    fn store_clip(&mut self, label: &str, data: ClipData) -> Result<u64, String> {
        let dir = std::env::temp_dir().join("opengarage");
        let id = self.project.next_clip_id();
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis());
        let file = dir.join(format!("audio-{stamp}-{id}.wav"));
        std::fs::create_dir_all(&dir)
            .and_then(|()| wav::write_wav(&file, &data.samples, data.sample_rate, data.channels, 24))
            .map_err(|e| e.to_string())?;
        self.add_clip(id, &format!("{label} {id}"), &file, data);
        Ok(id)
    }

    /// Makes another take of the selected region the one that is heard.
    pub fn select_take(&mut self, index: usize) {
        let Some((ti, ri)) = self.selection() else { return };
        if self.track_locked(ti) || index >= self.project.tracks[ti].regions[ri].takes.len() {
            return;
        }
        self.checkpoint();
        let region = &mut self.project.tracks[ti].regions[ri];
        std::mem::swap(&mut region.content, &mut region.takes[index]);
        self.selected_notes.clear();
    }

    /// Moves the notes or hits of the selected audio region onto the grid.
    pub fn quantize_audio_region(&mut self) {
        let Some((ti, ri)) = self.selection() else { return };
        let region = &self.project.tracks[ti].regions[ri];
        let Content::Audio(audio) = region.content else { return };
        if self.track_locked(ti) {
            return;
        }
        let Some(base) = self.clips.get(&audio.clip).cloned() else {
            self.status = tr("The audio file of this region cannot be found.").to_owned();
            return;
        };
        let beat_seconds = 60.0 / self.project.tempo_at(region.start);
        // Where the first sample of the clip sits on the grid.
        let first_beat = region.start - audio.offset / beat_seconds;
        let fixed = stretch::quantize_timing(&base, first_beat, beat_seconds, self.timing_grid);
        self.checkpoint();
        match self.store_clip(tr("Snapped"), fixed) {
            Ok(id) => {
                if let Content::Audio(audio) = &mut self.project.tracks[ti].regions[ri].content {
                    audio.clip = id;
                }
                self.status = tr("Timing corrected onto the grid (Ctrl+Z to go back).").to_owned();
            }
            Err(e) => self.status = tf!("Correction failed: {e}" [e]),
        }
    }

    pub fn add_clip(&mut self, id: u64, name: &str, path: &Path, data: ClipData) {
        self.waveforms.insert(id, analysis::peaks(&data.samples, usize::from(data.channels), 4000));
        self.clips.insert(id, Arc::new(data));
        self.clips_dirty = true;
        if !self.project.clips.iter().any(|c| c.id == id) {
            self.project.clips.push(Clip { id, name: name.to_owned(), path: path.to_string_lossy().into_owned() });
        }
    }

    /// Loads the audio of every clip of the project; returns the names not found.
    /// Where a file the project names is: a relative path is from the
    /// project's own folder.
    pub fn absolute_path(&self, text: &str) -> PathBuf {
        let path = PathBuf::from(text);
        match (self.path.as_ref().and_then(|p| p.parent()), path.is_relative()) {
            (Some(base), true) => base.join(path),
            _ => path,
        }
    }

    /// Loads the capture (or the impulse) a project refers to by `key`, the
    /// path it writes, unless it is loaded already.
    pub(crate) fn load_effect_file(&mut self, key: &str, impulse: bool) -> Result<(), String> {
        let loaded = if impulse { self.captures.impulses.contains_key(key) } else { self.captures.gear.contains_key(key) };
        if key.is_empty() || loaded {
            return Ok(());
        }
        let path = self.absolute_path(key);
        let name = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        if impulse {
            let clip = codec::decode_file(&path)?;
            self.captures.impulses.insert(key.to_owned(), Arc::new(Impulse::from_clip(&name, &clip)?));
        } else {
            self.captures.gear.insert(key.to_owned(), Arc::new(Capture::load(&path)?));
        }
        self.captures_dirty = true;
        Ok(())
    }

    /// Gives the amp of a track a capture of real gear, or takes it away.
    pub fn set_capture(&mut self, ti: usize, path: Option<&Path>) {
        self.set_effect_file(ti, path, false);
    }

    /// Gives the amp of a track the impulse of a cabinet, or takes it away.
    pub fn set_cabinet(&mut self, ti: usize, path: Option<&Path>) {
        self.set_effect_file(ti, path, true);
    }

    fn set_effect_file(&mut self, ti: usize, path: Option<&Path>, impulse: bool) {
        let key = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        if let Err(e) = self.load_effect_file(&key, impulse) {
            self.status = tf!("This file cannot be used: {e}" [e]);
            return;
        }
        self.checkpoint();
        let Some(track) = self.project.tracks.get_mut(ti) else { return };
        if impulse {
            track.fx.cabinet = key;
        } else {
            track.fx.capture = key;
        }
        if path.is_some() {
            track.fx.amp.on = true;
        }
    }

    /// Gives the reverb the impulse of a room, or takes it away.
    pub fn set_reverb_impulse(&mut self, path: Option<&Path>) {
        let key = path.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
        if let Err(e) = self.load_effect_file(&key, true) {
            self.status = tf!("This file cannot be used: {e}" [e]);
            return;
        }
        self.checkpoint();
        self.project.reverb_impulse = key;
    }

    /// Asks for a capture of real gear for the amp of a track.
    pub fn pick_capture(&mut self, ti: usize) {
        if let Some(path) = files::open_file_in(tr("Capture of an amplifier or a pedal (NAM, Proteus, AIDA-X)"), &capture::EXTENSIONS, &self.captures_dir()) {
            self.set_capture(ti, Some(&path));
        }
    }

    /// Asks for the impulse of a cabinet for the amp of a track.
    pub fn pick_cabinet(&mut self, ti: usize) {
        if let Some(path) = files::open_file_in(tr("Impulse response of a cabinet (WAV, FLAC)"), &convolution::EXTENSIONS, &self.impulses_dir()) {
            self.set_cabinet(ti, Some(&path));
        }
    }

    /// Asks for the impulse of a room for the reverb.
    pub fn pick_reverb_impulse(&mut self) {
        if let Some(path) = files::open_file_in(tr("Impulse response of a room (WAV, FLAC)"), &convolution::EXTENSIONS, &self.impulses_dir()) {
            self.set_reverb_impulse(Some(&path));
        }
    }

    pub(crate) fn load_clips(&mut self) -> Vec<String> {
        let mut missing = Vec::new();
        for clip in self.project.clips.clone() {
            let path = self.absolute_path(&clip.path);
            match codec::decode_file(&path) {
                Ok(data) => self.add_clip(clip.id, &clip.name, &path, data),
                Err(_) => missing.push(clip.name),
            }
        }
        // Sound banks the project uses and that are not loaded yet.
        for font in self.project.fonts.clone() {
            let path = self.absolute_path(&font.path);
            if !path.exists() {
                // The bank has moved: the same file among the installed banks,
                // or in the folder of downloaded banks, stands in for it.
                let name = path.file_name().map(|n| n.to_string_lossy().to_lowercase());
                let installed = self.settings.banks.iter().map(|b| PathBuf::from(&b.path)).find(|p| {
                    p.exists() && p.file_name().map(|n| n.to_string_lossy().to_lowercase()) == name
                });
                if let Some(found) = installed.or_else(|| self.relocated_bank(&path)) {
                    let section = self.installed_section(&found);
                    if let Ok(id) = self.load_font_in(&found, &section) {
                        self.remap_font(font.id, id, &found);
                        continue;
                    }
                }
            }
            if self.load_font(&path).is_err() {
                missing.push(font.name);
            }
        }
        // The captures and impulses the effects refer to.
        let mut wanted: Vec<(String, bool)> = vec![(self.project.reverb_impulse.clone(), true)];
        for track in &self.project.tracks {
            wanted.push((track.fx.capture.clone(), false));
            wanted.push((track.fx.cabinet.clone(), true));
        }
        for (key, impulse) in wanted {
            if self.load_effect_file(&key, impulse).is_err() {
                missing.push(Path::new(&key).file_name().map_or_else(|| key.clone(), |n| n.to_string_lossy().into_owned()));
            }
        }
        missing
    }

    pub fn import_audio(&mut self) {
        let Some(path) = files::audio_file() else { return };
        self.import_audio_file(&path, None, None);
    }

    /// Puts an audio file on a track: `target` when it holds audio, else the
    /// selected track when it does, else a new one; at `start`, or at the playhead.
    pub fn import_audio_file(&mut self, path: &Path, target: Option<usize>, start: Option<f64>) {
        let data = match codec::decode_file(path) {
            Ok(data) => data,
            Err(e) => {
                self.status = tf!("Import failed: {e}" [e]);
                return;
            }
        };
        self.checkpoint();
        let takes_audio = |track: &Track| track.kind.is_audio() && !track.locked;
        let ti = match target.or(Some(self.selected_track)).filter(|ti| self.project.tracks.get(*ti).is_some_and(takes_audio)) {
            Some(ti) => ti,
            None => {
                self.project.tracks.push(templates::track(tr("Audio"), TrackKind::Audio, "No effect", 5));
                self.project.tracks.len() - 1
            }
        };
        let id = self.project.next_clip_id();
        let name = path.file_stem().map_or_else(|| tr("Audio").to_owned(), |s| s.to_string_lossy().into_owned());
        let start = start.unwrap_or_else(|| snap(self.position, 1.0)).max(0.0);
        let length = (data.seconds() * self.project.tempo_at(start) / 60.0).max(0.25);
        self.add_clip(id, &name, path, data);
        let mut region = Region::new(name, start, length);
        region.content = Content::Audio(AudioRef::new(id));
        self.push_region(ti, region);
        self.status = tr("Imported audio file.").to_owned();
    }

    pub fn import_midi(&mut self) {
        let Some(path) = files::open_file(tr("MIDI file"), &["mid", "midi"]) else { return };
        self.import_midi_file(&path);
    }

    /// Adds the tracks of a MIDI file to the project.
    pub fn import_midi_file(&mut self, path: &Path) {
        let file = match std::fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| smf::parse(&bytes)) {
            Ok(file) => file,
            Err(e) => {
                self.status = tf!("Import failed: {e}" [e]);
                return;
            }
        };
        if file.tracks.is_empty() {
            self.status = tr("This MIDI file holds no notes.").to_owned();
            return;
        }
        self.checkpoint();
        let bar = self.project.bar_len();
        let empty = self.project.tracks.iter().all(|t| t.regions.is_empty());
        if let (true, Some(tempo)) = (empty, file.tempo) {
            self.project.tempo = tempo.clamp(40.0, 240.0).round();
        }
        let count = file.tracks.len();
        for (i, imported) in file.tracks.into_iter().enumerate() {
            let patch = if imported.drums { patches::DEFAULT_DRUMS } else { patches::DEFAULT_PATCH };
            let name = if imported.name.is_empty() { tf!("MIDI {}", i + 1) } else { imported.name };
            let mut track = templates::track(&name, TrackKind::Instrument, patch, (self.project.tracks.len() % 8) as u8);
            let end = imported.notes.iter().map(|n| n.start + n.length).fold(0.0, f64::max);
            let mut region = Region::new(name, 0.0, ((end / bar).ceil() * bar).max(bar));
            region.content = Content::Midi(imported.notes);
            track.regions.push(region);
            self.project.tracks.push(track);
        }
        self.status = tf!("{count} tracks imported." [count]);
    }

    // ----- files ------------------------------------------------------------------

    /// Where the projects are saved unless the user says otherwise: the
    /// folder chosen in the options, else `OpenGarageMusic` in the user's
    /// documents (in the program's data folder for the tests).
    pub fn projects_dir(&self) -> PathBuf {
        let chosen = self.settings.projects_folder.trim();
        if !chosen.is_empty() {
            return PathBuf::from(chosen);
        }
        match (self.settings_path.is_some(), std::env::var_os("USERPROFILE")) {
            (true, Some(home)) => PathBuf::from(home).join("Documents").join("OpenGarageMusic"),
            _ => self.data_dir.join("Projects"),
        }
    }

    /// Chooses the folder the projects are saved in.
    pub fn choose_projects_folder(&mut self) {
        let current = self.projects_dir();
        let Some(folder) = files::choose_folder(None, Some(current.as_path()).filter(|c| c.is_dir())) else { return };
        self.settings.projects_folder = folder.to_string_lossy().into_owned();
        self.save_settings();
        self.status = tf!("Projects are saved in {}.", folder.display());
    }

    pub fn save(&mut self, ask: bool) {
        let path = match (&self.path, ask) {
            (Some(path), false) => Some(path.clone()),
            _ => {
                let folder = self.projects_dir();
                let _ = std::fs::create_dir_all(&folder);
                files::save_project(&self.project.name, &folder)
            }
        };
        let Some(path) = path else { return };
        match self.save_to(&path) {
            Ok(()) => {
                self.status = tf!("Project saved: {}", path.display());
                self.note_recent(&path);
                self.path = Some(path);
            }
            Err(e) => self.status = tf!("Saving failed: {e}" [e]),
        }
    }

    /// Writes the project file; takes still in the temporary folder are copied
    /// next to it first, so the project stays whole.
    /// Writes the project as one file: an archive holding `project.json` and,
    /// under `media/`, the audio of its takes and the files of its effects that
    /// came with it. A file of the user's elsewhere on the disk stays where it
    /// is, named by its path. The project in memory keeps working from its own
    /// files; a take still in the temporary folder moves to the session's.
    pub fn save_to(&mut self, path: &Path) -> Result<(), String> {
        self.capture_plugin_states(false);
        self.prune_clips();
        // The file keeps the id of its session, so that opening it finds it again.
        self.give_project_id(Some(path));
        let temp = std::env::temp_dir().join("opengarage");
        let session_media = self.session_dir().join("media");
        // Paths relative to the file the project came from (still `self.path`
        // here: it moves once the file is written), as older projects wrote them.
        let base = self.path.as_ref().and_then(|p| p.parent()).map(Path::to_path_buf);
        // The files that go into the archive: read from where, named how.
        let mut inside: Vec<(PathBuf, String)> = Vec::new();
        let mut place = |path: &mut String| -> Result<Option<String>, String> {
            if path.is_empty() {
                return Ok(None);
            }
            let written = PathBuf::from(&*path);
            let source = match (&base, written.is_relative()) {
                (Some(base), true) => base.join(&written),
                _ => written.clone(),
            };
            if !source.is_file() {
                return Ok(None);
            }
            // What came with the project goes with it: the takes, what was
            // beside its file; what is the user's elsewhere stays there.
            let comes_along = written.is_relative() || source.starts_with(&temp) || source.starts_with(&session_media) || base.as_ref().is_some_and(|b| source.starts_with(b));
            if !comes_along {
                return Ok(None);
            }
            let Some(file) = source.file_name().map(|f| f.to_string_lossy().into_owned()) else { return Ok(None) };
            // A take of the temporary folder moves to the session's for good.
            let working = if source.starts_with(&temp) {
                std::fs::create_dir_all(&session_media).map_err(|e| e.to_string())?;
                let moved = session_media.join(&file);
                std::fs::copy(&source, &moved).map_err(|e| e.to_string())?;
                moved
            } else {
                source
            };
            // Each file once in the archive, under a name of its own.
            let name = match inside.iter().find(|(from, _)| *from == working) {
                Some((_, name)) => name.clone(),
                None => {
                    let mut name = format!("media/{file}");
                    let mut n = 2;
                    while inside.iter().any(|(_, taken)| *taken == name) {
                        name = format!("media/{n}-{file}");
                        n += 1;
                    }
                    inside.push((working.clone(), name.clone()));
                    name
                }
            };
            *path = working.to_string_lossy().into_owned();
            Ok(Some(name))
        };
        // The project as the archive writes it: its files named inside it.
        let mut saved = self.project.clone();
        for (clip, written) in self.project.clips.iter_mut().zip(&mut saved.clips) {
            if let Some(name) = place(&mut clip.path)? {
                written.path = name;
            }
        }
        if let Some(name) = place(&mut self.project.reverb_impulse)? {
            saved.reverb_impulse = name;
        }
        for (track, written) in self.project.tracks.iter_mut().zip(&mut saved.tracks) {
            if let Some(name) = place(&mut track.fx.capture)? {
                written.fx.capture = name;
            }
            if let Some(name) = place(&mut track.fx.cabinet)? {
                written.fx.cabinet = name;
            }
        }
        let text = saved.to_json().map_err(|e| e.to_string())?;
        // Written beside, then put in place, so that a failure leaves the old file whole.
        let partial = path.with_extension("ogproj.part");
        let written = (|| -> Result<(), String> {
            let file = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
            let mut archive = zip::ZipWriter::new(std::io::BufWriter::new(file));
            let packed = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
            // Sound compresses poorly: stored as it is, and read as fast.
            let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
            archive.start_file(PROJECT_ENTRY, packed).map_err(|e| e.to_string())?;
            archive.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
            for (source, name) in &inside {
                archive.start_file(name.as_str(), stored).map_err(|e| e.to_string())?;
                let mut from = std::fs::File::open(source).map_err(|e| e.to_string())?;
                std::io::copy(&mut from, &mut archive).map_err(|e| e.to_string())?;
            }
            archive.finish().map_err(|e| e.to_string())?.flush().map_err(|e| e.to_string())?;
            if path.exists() {
                std::fs::remove_file(path).map_err(|e| e.to_string())?;
            }
            std::fs::rename(&partial, path).map_err(|e| e.to_string())
        })();
        if written.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        written?;
        self.clean = Some(self.project.clone());
        Ok(())
    }

    /// Forgets the audio files no region plays any more, not even as
    /// another take: they would go with the project for nothing.
    pub fn prune_clips(&mut self) {
        let used: HashSet<u64> = self
            .project
            .tracks
            .iter()
            .flat_map(|t| &t.regions)
            .flat_map(|r| std::iter::once(&r.content).chain(&r.takes))
            .filter_map(|content| match content {
                Content::Audio(audio) => Some(audio.clip),
                _ => None,
            })
            .collect();
        self.project.clips.retain(|clip| used.contains(&clip.id));
    }

    /// Forgets another take of the selected region.
    pub fn remove_take(&mut self, index: usize) {
        let Some((ti, ri)) = self.selection() else { return };
        if self.track_locked(ti) || index >= self.project.tracks[ti].regions[ri].takes.len() {
            return;
        }
        self.checkpoint();
        self.project.tracks[ti].regions[ri].takes.remove(index);
    }

    // ----- audio outputs and inputs -----------------------------------------------

    /// Lists again the audio outputs and inputs of the computer.
    pub fn refresh_audio_devices(&mut self) {
        self.audio_outputs = audio::output_devices();
        self.audio_inputs = audio::input_devices();
        self.audio_input_channels = audio::input_channel_count(&self.settings.input_device);
        self.audio.input_channel = self.settings.input_channel;
    }

    /// Moves the main mix to another output; an empty name is the system's default one.
    pub fn set_output_device(&mut self, name: &str) {
        match self.audio.set_output(name) {
            Ok(()) => {
                self.settings.output_device = name.to_owned();
                self.save_settings();
                // The tracks sent apart are looked at again.
                self.routes_wanted.clear();
                self.routes_wanted.push(String::from("\u{0}"));
            }
            Err(e) => self.status = tf!("Audio output “{}”: {}", name, e),
        }
    }

    /// Chooses the input the program records and listens to; an empty name is
    /// the system's default one.
    pub fn set_input_device(&mut self, name: &str) {
        self.settings.input_device = name.to_owned();
        self.save_settings();
        if self.audio.has_input() {
            if let Err(e) = self.audio.ensure_input(name) {
                self.status = tf!("Cannot listen to the input: {e}" [e]);
            }
        }
    }

    /// Channel of the input a track records: its own, else the program's (0 mixes all).
    pub fn channel_for(&self, ti: usize) -> u16 {
        self.project.tracks.get(ti).map(|t| t.input_channel).filter(|channel| *channel > 0).unwrap_or(self.settings.input_channel)
    }

    /// Chooses the channel of the input the program records; 0 mixes them all.
    pub fn set_input_channel(&mut self, channel: u16) {
        self.settings.input_channel = channel;
        self.save_settings();
        self.audio.input_channel = channel;
        if self.audio.has_input() {
            let name = self.settings.input_device.clone();
            if let Err(e) = self.audio.ensure_input(&name) {
                self.status = tf!("Cannot listen to the input: {e}" [e]);
            }
        }
    }

    /// Gives an audio track a channel of its own; 0 is the program's.
    pub fn set_track_input_channel(&mut self, ti: usize, channel: u16) {
        if self.project.tracks.get(ti).is_some_and(|t| t.input_channel != channel) {
            self.checkpoint();
            self.project.tracks[ti].input_channel = channel;
        }
    }

    /// Bit rate of compressed exports, in kilobits per second.
    pub fn export_kbps(&self) -> u32 {
        if self.settings.export_kbps == 0 { 192 } else { self.settings.export_kbps }
    }

    /// Puts a project file at the top of the list of recent projects.
    pub fn note_recent(&mut self, path: &Path) {
        let text = path.to_string_lossy().into_owned();
        self.settings.recent_projects.retain(|known| !known.eq_ignore_ascii_case(&text));
        self.settings.recent_projects.insert(0, text);
        self.settings.recent_projects.truncate(10);
        self.save_settings();
    }

    /// The recent project files that are still there.
    pub fn recent_projects(&self) -> Vec<PathBuf> {
        self.settings.recent_projects.iter().map(PathBuf::from).filter(|path| path.is_file()).collect()
    }

    /// Opens a project file, saying so when it cannot.
    pub fn open_recent(&mut self, path: &Path) {
        if let Err(e) = self.open_path(path) {
            self.status = tf!("Cannot open {}: {e}" [e], path.display());
        }
    }

    /// The playhead's position as minutes, seconds and tenths.
    pub fn position_time(&self) -> String {
        // With the original sound, the time of the song as the recording plays it.
        let followed = self.followed_timing().and_then(|timing| Some(og_engine::recording::sec_at(timing, self.position)? - og_engine::recording::sec_at(timing, 0.0)?));
        let seconds = followed.unwrap_or_else(|| self.project.beat_to_sec(self.position)).max(0.0);
        format!("{}:{:04.1}", (seconds / 60.0).floor() as u64, seconds % 60.0)
    }

    /// Name of the input a track records and listens to: its own, else the program's.
    pub fn input_for(&self, ti: usize) -> String {
        self.project.tracks.get(ti).map(|t| t.input.as_str()).filter(|name| !name.is_empty()).unwrap_or(&self.settings.input_device).to_owned()
    }

    /// Sends a track to an output of its own; an empty name is the main mix.
    pub fn set_track_output(&mut self, ti: usize, name: &str) {
        if self.project.tracks.get(ti).is_some_and(|t| t.output != name) {
            self.checkpoint();
            self.project.tracks[ti].output = name.to_owned();
        }
    }

    /// Gives an audio track an input of its own; an empty name is the program's.
    pub fn set_track_input(&mut self, ti: usize, name: &str) {
        if self.project.tracks.get(ti).is_some_and(|t| t.input != name) {
            self.checkpoint();
            self.project.tracks[ti].input = name.to_owned();
        }
    }

    /// Opens the outputs the tracks are sent to and closes those no track
    /// uses any more, when that changed.
    fn sync_routes(&mut self) {
        let mut wanted: Vec<String> = self.project.tracks.iter().map(|t| t.output.clone()).filter(|name| !name.is_empty()).collect();
        wanted.sort();
        wanted.dedup();
        if wanted == self.routes_wanted {
            return;
        }
        self.routes_wanted.clone_from(&wanted);
        // The tests have no sound card.
        if cfg!(test) {
            return;
        }
        let failed = self.audio.set_extra_outputs(&wanted);
        if let Some(name) = failed.first() {
            self.status = tf!("Audio output “{}” is not available: its tracks play in the main mix.", name);
        }
    }

    pub fn open(&mut self) {
        let Some(path) = files::project_file(&self.projects_dir()) else { return };
        if let Err(e) = self.open_path(&path) {
            self.status = tf!("Cannot open {}: {e}" [e], path.display());
        }
    }

    pub fn open_path(&mut self, path: &Path) -> Result<(), String> {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let mut project = if bytes.starts_with(b"PK\x03\x04") {
            self.unpack_project(path, bytes)?
        } else {
            // A project written before the archive: the JSON alone, its audio
            // in a folder beside it.
            Project::from_json(&String::from_utf8_lossy(&bytes)).map_err(|e| e.to_string())?
        };
        // Projects saved before the interface was translated name their sounds in French.
        let french = crate::i18n::built_in("fr");
        for track in &mut project.tracks {
            if patches::find(&track.patch).is_none() {
                let mut name = track.patch.clone();
                library_name(french.as_ref(), &mut name);
                if patches::find(&name).is_some() {
                    track.patch = name;
                }
            }
        }
        self.replace_project(project, Some(path.to_path_buf()));
        let missing = self.load_clips();
        // Loading may have followed a bank that moved: that is not an edit.
        self.clean = Some(self.project.clone());
        self.note_recent(path);
        self.status = if missing.is_empty() {
            tf!("Project opened: {}", path.display())
        } else {
            tf!("Project opened, but {} audio file(s) cannot be found: {}", missing.len(), missing.join(", "))
        };
        Ok(())
    }

    /// Reads the project of an archive and takes its files out into the
    /// session's folder, where the project works from; what the archive does
    /// not hold keeps its name, and is told missing when the clips are loaded.
    fn unpack_project(&mut self, path: &Path, bytes: Vec<u8>) -> Result<Project, String> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| tf!("unreadable archive ({e})" [e]))?;
        let text = {
            let mut entry = archive.by_name(PROJECT_ENTRY).map_err(|_| tr("the archive holds no project").to_owned())?;
            let mut text = String::new();
            std::io::Read::read_to_string(&mut entry, &mut text).map_err(|e| e.to_string())?;
            text
        };
        let mut project = Project::from_json(&text).map_err(|e| e.to_string())?;
        Self::give_id(&mut project, Some(path));
        let media = self.sessions_dir().join(&project.id).join("media");
        std::fs::create_dir_all(&media).map_err(|e| e.to_string())?;
        let names: Vec<String> = archive.file_names().filter(|n| n.starts_with("media/") && !n.ends_with('/')).map(str::to_owned).collect();
        let mut taken_out: HashMap<String, String> = HashMap::new();
        for name in names {
            let Some(file) = Path::new(&name).file_name() else { continue };
            let to = media.join(file);
            let mut entry = archive.by_name(&name).map_err(|e| e.to_string())?;
            let mut out = std::fs::File::create(&to).map_err(|e| e.to_string())?;
            std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
            taken_out.insert(name, to.to_string_lossy().into_owned());
        }
        let mut files: Vec<&mut String> = vec![&mut project.reverb_impulse];
        for track in &mut project.tracks {
            files.push(&mut track.fx.capture);
            files.push(&mut track.fx.cabinet);
        }
        for clip in &mut project.clips {
            files.push(&mut clip.path);
        }
        for file in files {
            if let Some(out) = taken_out.get(file.as_str()) {
                file.clone_from(out);
            }
        }
        Ok(project)
    }

    /// Asks where to write an export named after the song, with the first
    /// of `extensions`, writes it with `write`, and says how it went.
    fn export_with(&mut self, label: &str, extensions: &[&str], write: impl FnOnce(&mut Self, &Path) -> Result<(), String>) {
        let Some(path) = files::save_file(label, extensions, &format!("{}.{}", self.project.name, extensions[0])) else { return };
        self.status = match write(self, &path) {
            Ok(()) => tf!("Export finished: {}", path.display()),
            Err(e) => tf!("Export failed: {e}" [e]),
        };
    }

    pub fn export_compressed(&mut self, format: Compressed) {
        self.export_with(format.label(), &[format.extension()], |app, path| export::encode(path, &app.render_song(), EXPORT_RATE, format, app.export_kbps()));
    }

    pub fn export_aiff(&mut self) {
        self.export_with(tr("AIFF audio"), &["aiff", "aif"], |app, path| wav::write_aiff(path, &app.render_song(), EXPORT_RATE, 2, app.export_bits).map_err(|e| e.to_string()));
    }

    pub fn export_wav(&mut self) {
        self.export_with(tr("WAV audio"), &["wav"], |app, path| wav::write_wav(path, &app.render_song(), EXPORT_RATE, 2, app.export_bits).map_err(|e| e.to_string()));
    }

    /// The song as a Standard MIDI File: the tempo, the time signature, and
    /// a track of notes for each track that plays notes, as it plays them.
    pub fn midi_bytes(&self) -> Vec<u8> {
        let project = &self.project;
        let mut tempo: Vec<(f64, f64)> = project.tempo_points.iter().map(|p| (p.beat, p.value)).collect();
        if tempo.first().is_none_or(|(beat, _)| *beat > 1e-9) {
            tempo.insert(0, (0.0, project.tempo));
        }
        let tracks: Vec<og_engine::smf::WriteTrack> = project
            .tracks
            .iter()
            .filter(|track| !track.kind.is_audio())
            .map(|track| {
                let drums = track.sound.is_drums() || track.kind == TrackKind::Drummer;
                let program = match track.sound {
                    Sound::Sampler(sampler) if sampler.bank < 128 => sampler.preset,
                    _ => 0,
                };
                og_engine::smf::WriteTrack { name: track.name.clone(), drums, program, notes: og_engine::engine::played_notes(track, project) }
            })
            .filter(|track| !track.notes.is_empty())
            .collect();
        og_engine::smf::write(&tempo, (project.beats_per_bar, project.beat_unit), &tracks)
    }

    pub fn export_midi(&mut self) {
        self.export_with(tr("MIDI file"), &["mid"], |app, path| std::fs::write(path, app.midi_bytes()).map_err(|e| e.to_string()));
    }

    // ----- tracks -----------------------------------------------------------------

    pub fn add_track(&mut self, kind: TrackKind) {
        self.checkpoint();
        let n = self.project.tracks.len();
        let (name, patch) = match kind {
            TrackKind::Instrument => (tf!("Track {}", n + 1), patches::DEFAULT_PATCH),
            // The sounds are named as the library knows them, whatever the language.
            TrackKind::Audio => (tr("Audio").to_owned(), "Natural voice"),
            TrackKind::Guitar => (tr("Guitar").to_owned(), "Clean amp"),
            TrackKind::Drummer => (tr("Drummer").to_owned(), patches::DEFAULT_DRUMS),
            TrackKind::Video => ("YouTube".to_owned(), ""),
        };
        let mut track = templates::track(&name, kind, patch, (n % 8) as u8);
        track.icon = match kind {
            TrackKind::Instrument => 0,
            TrackKind::Drummer => 1,
            TrackKind::Audio | TrackKind::Video => 2,
            TrackKind::Guitar => 3,
        };
        if kind == TrackKind::Drummer {
            let mut region = Region::new(tr("Drums"), 0.0, self.project.bar_len() * 8.0);
            region.content = Content::Drummer(DrummerParams::default());
            track.regions.push(region);
        }
        self.project.tracks.push(track);
        self.selected_track = n;
        self.selected_region = None;
    }

    pub fn delete_track(&mut self, ti: usize) {
        if ti >= self.project.tracks.len() {
            return;
        }
        self.release_typing();
        self.checkpoint();
        let selected = self.selected_track;
        self.project.tracks.remove(ti);
        self.selected_region = None;
        if selected > ti {
            self.selected_track = selected - 1;
        }
        self.fix_selection();
        self.editor_track_deleted(ti, selected == ti);
    }

    pub fn duplicate_track(&mut self, ti: usize) {
        let Some(track) = self.project.tracks.get(ti).cloned() else { return };
        self.checkpoint();
        self.project.tracks.insert(ti + 1, Track { name: tf!("{} (copy)", track.name), ..track });
        self.editor_track_inserted(ti + 1);
        self.selected_track = ti + 1;
        self.selected_region = None;
    }

    /// Moves a track one place up (`-1`) or down (`1`).
    pub fn move_track(&mut self, ti: usize, by: isize) {
        let Some(target) = ti.checked_add_signed(by).filter(|t| *t < self.project.tracks.len()) else { return };
        self.checkpoint();
        self.project.tracks.swap(ti, target);
        self.editor_tracks_swapped(ti, target);
        self.selected_track = target;
        self.selected_region = None;
        self.selected_notes.clear();
    }

    /// Gives the selected track the sound and effects of a library patch.
    pub fn apply_patch(&mut self, patch: &PatchDef) {
        let ti = self.selected_track;
        if self.track_locked(ti) || ti >= self.project.tracks.len() {
            return;
        }
        self.checkpoint();
        let track = &mut self.project.tracks[ti];
        track.patch.clone_from(&patch.name);
        if !track.kind.is_audio() {
            track.sound = patch.sound;
        }
        track.fx = patch.fx.clone();
        let audible = !track.kind.is_audio();
        // The project remembers which sound bank file the preset comes from.
        if let Sound::Sampler(sampler) = patch.sound {
            if !self.project.fonts.iter().any(|f| f.id == sampler.font) {
                if let Some(file) = self.font_files.iter().find(|f| f.id == sampler.font) {
                    self.project.fonts.push(file.clone());
                }
            }
        }
        self.sync(self.last_change);
        if audible {
            let pitch = if patch.sound.is_drums() { 38 } else { 60 };
            self.play_preview(ti, pitch, 0.4);
        }
    }

    pub fn play_preview(&mut self, ti: usize, pitch: u8, seconds: f64) {
        let mut engine = self.engine();
        if let Some((track, old, _)) = self.preview {
            engine.live_note_off(track, old);
        }
        engine.live_note_on(ti, pitch, 100);
        drop(engine);
        self.preview = Some((ti, pitch, -seconds));
    }

    // ----- regions ----------------------------------------------------------------

    pub fn delete_selected_region(&mut self) {
        let Some((ti, ri)) = self.selection() else { return };
        if self.track_locked(ti) {
            return;
        }
        self.checkpoint();
        self.project.tracks[ti].regions.remove(ri);
        self.selected_region = None;
        self.selected_notes.clear();
        self.drag = None;
    }

    pub fn copy_region(&mut self) {
        if let Some((ti, ri)) = self.selection() {
            self.clipboard = self.region(ti, ri).cloned();
            self.status = tr("Region copied.").to_owned();
        }
    }

    pub fn cut_region(&mut self) {
        self.copy_region();
        self.delete_selected_region();
    }

    /// Pastes the copied region at the playhead on the selected track.
    pub fn paste_region(&mut self) {
        let Some(mut region) = self.clipboard.clone() else { return };
        let ti = self.selected_track;
        let Some(track) = self.project.tracks.get(ti) else { return };
        if track.locked || !accepts(track.kind, &region.content) {
            self.status = tr("This region cannot be pasted on this kind of track.").to_owned();
            return;
        }
        self.checkpoint();
        region.start = snap(self.position, SNAP);
        let end = region.end();
        self.push_region(ti, region);
        self.go_to(end);
    }

    /// Turns a looped MIDI region into one long region with every repeat written out.
    fn flatten_loop(region: &mut Region) {
        let Some(cycle) = region.loop_len.take() else { return };
        let (length, cycle) = (region.length, cycle.max(0.0625));
        if let Some(notes) = region.notes_mut() {
            let source = std::mem::take(notes);
            let repeats = ((length / cycle).ceil() as usize).min(4096);
            for k in 0..repeats {
                let base = k as f64 * cycle;
                notes.extend(source.iter().filter(|n| n.start < cycle && base + n.start < length).map(|n| Note {
                    start: base + n.start,
                    length: n.length.min(length - base - n.start),
                    ..n.clone()
                }));
            }
        }
    }

    /// Cuts the selected region in two at the playhead.
    pub fn split_region(&mut self) {
        let Some((ti, ri)) = self.selection() else { return };
        let at = snap(self.position, SNAP);
        let region = &self.project.tracks[ti].regions[ri];
        if self.project.tracks[ti].locked || at <= region.start + 1e-9 || at >= region.end() - 1e-9 {
            self.status = tr("Put the playhead inside the selected region.").to_owned();
            return;
        }
        if region.loop_len.is_some() && !region.is_midi() {
            self.status = tr("Stop looping this region first.").to_owned();
            return;
        }
        self.checkpoint();
        let seconds = self.project.beat_to_sec(at) - self.project.beat_to_sec(self.project.tracks[ti].regions[ri].start);
        let region = &mut self.project.tracks[ti].regions[ri];
        Self::flatten_loop(region);
        let cut = at - region.start;
        let mut right = region.clone();
        right.start = at;
        right.length = region.length - cut;
        region.length = cut;
        match (&mut region.content, &mut right.content) {
            (Content::Midi(left), Content::Midi(rest)) => {
                rest.retain(|n| n.start >= cut - 1e-9);
                for note in rest.iter_mut() {
                    note.start -= cut;
                }
                left.retain(|n| n.start < cut - 1e-9);
                for note in left.iter_mut() {
                    note.length = note.length.min(cut - note.start);
                }
            }
            (_, Content::Audio(audio)) => audio.offset += seconds,
            _ => {}
        }
        self.project.tracks[ti].regions.insert(ri + 1, right);
    }

    /// Merges the selected MIDI region with the next one on its track.
    pub fn join_regions(&mut self) {
        let Some((ti, ri)) = self.selection() else { return };
        let regions = &self.project.tracks[ti].regions;
        let first = &regions[ri];
        let next = regions
            .iter()
            .enumerate()
            .filter(|(i, r)| *i != ri && r.start >= first.start && r.is_midi())
            .min_by(|a, b| a.1.start.total_cmp(&b.1.start))
            .map(|(i, _)| i);
        let (Some(other), true, false) = (next, first.is_midi(), self.project.tracks[ti].locked) else {
            self.status = tr("Joining needs two note regions on the same track.").to_owned();
            return;
        };
        self.checkpoint();
        let regions = &mut self.project.tracks[ti].regions;
        let mut second = regions[other].clone();
        Self::flatten_loop(&mut second);
        let first = &mut regions[ri];
        Self::flatten_loop(first);
        let shift = second.start - first.start;
        first.length = first.length.max(second.end() - first.start);
        if let Some(notes) = first.notes_mut() {
            notes.extend(second.notes().iter().map(|n| Note { start: n.start + shift, ..n.clone() }));
        }
        regions.remove(other);
        let ri = if other < ri { ri - 1 } else { ri };
        self.selected_region = Some((ti, ri));
        self.selected_notes.clear();
    }

    /// Loops the selected region (doubling its length) or stops looping it.
    pub fn toggle_loop(&mut self) {
        let Some((ti, ri)) = self.selection() else { return };
        if self.track_locked(ti) {
            return;
        }
        self.checkpoint();
        let region = &mut self.project.tracks[ti].regions[ri];
        match region.loop_len.take() {
            Some(cycle) => region.length = cycle,
            None => {
                region.loop_len = Some(region.length);
                region.length *= 2.0;
            }
        }
    }

    // ----- arrangement sections ----------------------------------------------------

    /// Moves a section and the regions that start inside it.
    pub fn move_section(&mut self, index: usize, new_start: f64) {
        let Some(marker) = self.project.markers.get(index) else { return };
        let (old, end, shift) = (marker.start, marker.start + marker.length, new_start - marker.start);
        if shift == 0.0 {
            return;
        }
        for track in self.project.tracks.iter_mut().filter(|t| !t.locked) {
            for region in track.regions.iter_mut().filter(|r| r.start >= old - 1e-9 && r.start < end - 1e-9) {
                region.start += shift;
            }
        }
        self.project.markers[index].start = new_start;
    }

    pub fn delete_section(&mut self, index: usize, with_content: bool) {
        if index >= self.project.markers.len() {
            return;
        }
        self.checkpoint();
        let marker = self.project.markers.remove(index);
        if with_content {
            let end = marker.start + marker.length;
            for track in self.project.tracks.iter_mut().filter(|t| !t.locked) {
                track.regions.retain(|r| r.start < marker.start - 1e-9 || r.start >= end - 1e-9);
            }
            self.selected_region = None;
            self.selected_notes.clear();
        }
    }

    /// Repeats a section and its regions right after it.
    pub fn duplicate_section(&mut self, index: usize) {
        let Some(marker) = self.project.markers.get(index).cloned() else { return };
        self.checkpoint();
        let end = marker.start + marker.length;
        for track in self.project.tracks.iter_mut().filter(|t| !t.locked) {
            let copies: Vec<Region> = track
                .regions
                .iter()
                .filter(|r| r.start >= marker.start - 1e-9 && r.start < end - 1e-9)
                .map(|r| Region { start: r.start + marker.length, ..r.clone() })
                .collect();
            track.regions.extend(copies);
        }
        self.project.markers.insert(index + 1, Marker { start: end, ..marker });
    }

    /// Adds a loop from the library at the playhead, on a new track with its sound.
    pub fn add_loop(&mut self, index: usize) {
        let Some(def) = self.loops.get(index) else { return };
        let bar = self.project.bar_len();
        let (name, length) = (tr(def.name), def.length(bar));
        let start = (self.position / bar).floor() * bar;
        self.add_loop_at(crate::userloops::LoopPick::Notes(index), None, start);
        // The next loop added lands right after this one.
        self.go_to(start + length);
        self.status = tf!("Loop “{name}” added. Drag the top of its right edge to repeat it." [name]);
    }

    /// Puts a loop at `start`: on the track `target` when that track can take
    /// it there (the right kind, not locked, nothing in the way), else on a
    /// track that already has the loop's sound, else on a new track.
    pub fn add_loop_at(&mut self, pick: crate::userloops::LoopPick, target: Option<usize>, start: f64) {
        use crate::userloops::LoopPick;
        let bar = self.project.bar_len();
        let start = start.max(0.0);
        match pick {
            LoopPick::Notes(index) => {
                let Some(def) = self.loops.get(index) else { return };
                let notes = def.notes(self.project.key, self.project.minor, bar);
                let (name, patch, length) = (tr(def.name), def.patch, def.length(bar));
                self.checkpoint();
                let fits = |t: &Track| t.kind == TrackKind::Instrument && !t.locked && t.regions.iter().all(|r| r.end() <= start || r.start >= start + length);
                let chosen = target.filter(|ti| self.project.tracks.get(*ti).is_some_and(fits));
                // Without a track to go to: one that already has this loop's sound.
                let reused = if target.is_none() { self.project.tracks.iter().position(|t| t.patch == patch && fits(t)) } else { None };
                let ti = chosen.or(reused).unwrap_or_else(|| {
                    let color = (self.project.tracks.len() % 8) as u8;
                    self.project.tracks.push(templates::track(name, TrackKind::Instrument, patch, color));
                    self.project.tracks.len() - 1
                });
                let mut region = Region::new(name, start, length);
                region.content = Content::Midi(notes);
                self.push_region(ti, region);
            }
            LoopPick::Audio(index) => self.add_audio_loop(index, target, start),
        }
    }

    // ----- loops of recorded sound -------------------------------------------------

    /// Folder where the user's loops of recorded sound are kept.
    pub fn loops_dir(&self) -> PathBuf {
        self.data_dir.join(crate::userloops::FOLDER)
    }

    pub fn refresh_audio_loops(&mut self) {
        self.audio_loops = crate::userloops::scan(&self.loops_dir());
    }

    /// Copies audio files into the folder of loops.
    pub fn install_audio_loops(&mut self, files: &[PathBuf]) -> usize {
        let dir = self.loops_dir();
        let mut copied = 0;
        if std::fs::create_dir_all(&dir).is_ok() {
            for file in files {
                if let Some(name) = file.file_name() {
                    copied += usize::from(std::fs::copy(file, dir.join(name)).is_ok());
                }
            }
        }
        self.refresh_audio_loops();
        copied
    }

    pub fn import_audio_loops(&mut self) {
        let Some(files) = files::audio_files() else { return };
        let copied = self.install_audio_loops(&files);
        self.status = tf!("{} audio loop(s) added to the loop browser.", copied);
    }

    /// Puts one of the user's audio loops on an audio track at `start`,
    /// stretched so that it lasts a whole number of bars at the project's tempo.
    fn add_audio_loop(&mut self, index: usize, target: Option<usize>, start: f64) {
        let Some(found) = self.audio_loops.get(index) else { return };
        let (name, path, named) = (found.name.clone(), found.path.clone(), found.named_tempo);
        let data = match codec::decode_file(&path) {
            Ok(data) => data,
            Err(e) => {
                self.status = tf!("Import failed: {e}" [e]);
                return;
            }
        };
        let bar = self.project.bar_len();
        let (tempo, bars) = crate::userloops::tempo_and_bars(named, data.seconds(), f64::from(self.project.beats_per_bar.max(1)));
        let project_tempo = self.project.tempo_at(start);
        // One channel, as long as the bars it fills take at the project's tempo.
        let channels = usize::from(data.channels.max(1));
        let mono: Vec<f32> = data.samples.chunks(channels).map(|frame| frame.iter().sum::<f32>() / channels as f32).collect();
        let factor = tempo / project_tempo;
        let samples = if (factor - 1.0).abs() < 0.005 { mono } else { stretch::time_stretch(&mono, data.sample_rate as f32, factor) };
        let stretched = ClipData { sample_rate: data.sample_rate, channels: 1, samples };
        self.checkpoint();
        let id = match self.store_clip(&name, stretched) {
            Ok(id) => id,
            Err(e) => {
                self.status = tf!("The take could not be written to disk: {e}" [e]);
                return;
            }
        };
        let length = f64::from(bars) * bar;
        let fits = |t: &Track| t.kind.is_audio() && !t.locked && t.regions.iter().all(|r| r.end() <= start || r.start >= start + length);
        let ti = target.filter(|ti| self.project.tracks.get(*ti).is_some_and(fits)).unwrap_or_else(|| {
            self.project.tracks.push(templates::track(&name, TrackKind::Audio, "No effect", 5));
            self.project.tracks.len() - 1
        });
        let mut region = Region::new(name.clone(), start, length);
        region.content = Content::Audio(AudioRef::new(id));
        self.push_region(ti, region);
        self.status = tf!("Loop “{name}” added. Drag the top of its right edge to repeat it." [name]);
    }

    pub fn toggle_audition(&mut self, index: usize) {
        let Some(def) = self.loops.get(index) else { return };
        if self.auditioning.as_deref() == Some(def.name) {
            self.engine().stop_audition();
            self.auditioning = None;
            return;
        }
        let bar = self.project.bar_len();
        let notes = def.notes(self.project.key, self.project.minor, bar);
        let Some(patch) = patches::find(def.patch) else { return };
        let (name, length) = (def.name, def.length(bar));
        self.engine().audition(&patch.sound, &notes, length);
        self.auditioning = Some(name.to_owned());
    }

    // ----- keyboard ---------------------------------------------------------------

    pub fn release_typing(&mut self) {
        let held = std::mem::take(&mut self.held);
        let mut engine = self.engine();
        for (_, ti, pitch) in held {
            engine.live_note_off(ti, pitch);
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.wants_keyboard_input() {
            self.release_typing();
            return;
        }
        let (cmd, shift, alt) = ctx.input(|i| (i.modifiers.command, i.modifiers.shift, i.modifiers.alt));
        let pressed = |key: Key| ctx.input(|i| i.key_pressed(key));

        if pressed(Key::Space) {
            self.toggle_play();
        }
        if pressed(Key::Enter) || pressed(Key::Home) {
            self.go_to(0.0);
        }
        // Q snaps the notes of the editor onto the grid, unless the computer
        // keyboard is being played, where Q is a note.
        if pressed(Key::Q) && !cmd && !self.typing {
            self.quantize_request = true;
        }
        if pressed(Key::Escape) {
            self.beat_selection = None;
        }
        if pressed(Key::Delete) || pressed(Key::Backspace) {
            if self.selected_notes.is_empty() {
                self.delete_selected_region();
            } else {
                self.delete_selected_notes();
            }
        }
        if cmd {
            if pressed(Key::Z) && !shift {
                self.undo();
            }
            if pressed(Key::Y) || (shift && pressed(Key::Z)) {
                self.redo();
            }
            if pressed(Key::S) {
                self.save(false);
            }
            if pressed(Key::X) {
                self.cut_region();
            }
            if pressed(Key::C) {
                self.copy_region();
            }
            if pressed(Key::V) {
                self.paste_region();
            }
            if pressed(Key::T) {
                self.split_region();
            }
            if pressed(Key::J) {
                self.join_regions();
            }
            if pressed(Key::K) {
                self.typing = !self.typing;
                self.release_typing();
            }
            if pressed(Key::A) {
                if let Some((ti, ri)) = self.selection() {
                    self.selected_notes = (0..self.project.tracks[ti].regions[ri].notes().len()).collect();
                }
            }
            return;
        }

        if self.typing {
            // Musical typing on the selected track.
            let ti = self.selected_track;
            for (key, semitone) in TYPING {
                let down = !alt && ctx.input(|i| i.key_down(key));
                let held = self.held.iter().position(|h| h.0 == key);
                match (down, held) {
                    (true, None) => {
                        let pitch = (60 + self.octave * 12 + semitone).clamp(0, 127) as u8;
                        self.engine().live_note_on(ti, pitch, 100);
                        self.held.push((key, ti, pitch));
                    }
                    (false, Some(index)) => {
                        let (_, ti, pitch) = self.held.remove(index);
                        self.engine().live_note_off(ti, pitch);
                    }
                    _ => {}
                }
            }
            if pressed(Key::X) {
                self.octave = (self.octave + 1).min(3);
            }
            if pressed(Key::C) {
                self.octave = (self.octave - 1).max(-3);
            }
            return;
        }

        if pressed(Key::R) {
            self.toggle_record();
        }
        if pressed(Key::C) {
            self.project.cycle_enabled = !self.project.cycle_enabled;
        }
        if pressed(Key::K) {
            self.project.metronome = !self.project.metronome;
        }
        if pressed(Key::L) {
            self.toggle_loop();
        }
        if pressed(Key::A) {
            self.automation = if self.automation.is_some() { None } else { Some(AutoParam::Volume) };
        }
        if pressed(Key::Y) {
            self.show_library = !self.show_library;
        }
        if pressed(Key::O) {
            self.right_panel = if self.right_panel == RightPanel::Loops { RightPanel::None } else { RightPanel::Loops };
        }
        for (key, tab) in [(Key::B, BottomTab::Controls), (Key::E, BottomTab::Editor)] {
            if pressed(key) {
                if self.show_bottom && self.bottom_tab == tab {
                    self.show_bottom = false;
                } else {
                    self.show_bottom = true;
                    self.bottom_tab = tab;
                }
            }
        }
    }

    pub fn delete_selected_notes(&mut self) {
        let Some((ti, ri)) = self.selection() else { return };
        if self.selected_notes.is_empty() || self.track_locked(ti) {
            return;
        }
        self.checkpoint();
        let selected = std::mem::take(&mut self.selected_notes);
        if let Some(notes) = self.project.tracks[ti].regions[ri].notes_mut() {
            let mut index = 0;
            notes.retain(|_| {
                index += 1;
                !selected.contains(&(index - 1))
            });
        }
    }

    // ----- one frame --------------------------------------------------------------

    pub fn ui(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
        if !self.theme_applied {
            let kind = if self.theme.dark { egui::Theme::Dark } else { egui::Theme::Light };
            ctx.set_theme(kind);
            ctx.set_visuals_of(kind, self.theme.visuals());
            self.theme_applied = true;
            self.help_applied = None;
        }
        if self.help_applied != Some(self.help) {
            // With the quick help on, the tips do not wait.
            let on = self.help;
            let usual = egui::style::Interaction::default();
            ctx.all_styles_mut(|style| {
                style.interaction.tooltip_delay = if on { 0.0 } else { usual.tooltip_delay };
                style.interaction.show_tooltips_only_when_still = !on && usual.show_tooltips_only_when_still;
            });
            self.help_applied = Some(on);
        }
        let now = ctx.input(|i| i.time);
        let (playing, position, peak, capturing) = {
            let mut engine = self.engine();
            let mut peaks = Vec::new();
            engine.take_track_peaks(&mut peaks);
            let capturing = engine.is_recording() && engine.preroll() <= 0.0;
            let state = (engine.is_playing(), engine.position(), engine.take_peak(), capturing);
            drop(engine);
            if self.track_peaks.len() != peaks.len() {
                self.track_peaks = peaks;
            } else {
                for (shown, new) in self.track_peaks.iter_mut().zip(peaks) {
                    *shown = new.max(*shown * 0.88);
                }
            }
            state
        };
        self.playing = playing;
        self.position = position;
        self.peak = peak.max(self.peak * 0.9);
        if self.rec_audio == Some(false) && capturing {
            // The count-in is over: start keeping what comes from the input.
            self.audio.start_capture();
            self.rec_audio = Some(true);
        }
        self.audio.poll_capture();
        if self.recording && !playing {
            self.finish_recording();
        }
        self.audio.midi_track.store(self.selected_track, Ordering::Relaxed);
        self.pointer_down = ctx.input(|i| i.pointer.any_down());
        self.poll_downloads();
        self.poll_sites();
        self.poll_set_list();
        self.poll_bank_loading();
        if self.bank_loading.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.idle_plugins();
        if self.downloads.pending.is_some() || self.downloads.job.is_some() || self.sites.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }

        self.sync_routes();
        // Input monitoring follows the tracks' "listen" switches, and the
        // input of the first track that listens.
        let listening = self.project.tracks.iter().position(|t| t.monitor && t.kind.is_audio());
        let wanted = listening.is_some();
        let input = listening.map_or_else(String::new, |ti| self.input_for(ti));
        let channel = listening.map_or(0, |ti| self.channel_for(ti));
        if wanted != self.monitor_on || (wanted && (input != self.monitor_input || channel != self.audio.input_channel)) {
            self.audio.input_channel = channel;
            let opened = if wanted && !cfg!(test) { self.audio.ensure_input(&input) } else { Ok(()) };
            match opened {
                Ok(()) => {
                    self.monitor_input = input;
                    self.monitor_on = wanted;
                    self.audio.monitoring.store(wanted, Ordering::Relaxed);
                    let feed = wanted.then(|| Arc::clone(&self.audio.feed));
                    self.engine().set_monitor(feed);
                }
                Err(e) => {
                    for track in &mut self.project.tracks {
                        track.monitor = false;
                    }
                    self.status = tf!("Cannot listen to the input: {e}" [e]);
                }
            }
        }

        if let Some((ti, pitch, until)) = self.preview {
            if until < 0.0 {
                // A negative value is a duration waiting for its start time.
                self.preview = Some((ti, pitch, now - until));
            } else if now >= until {
                self.engine().live_note_off(ti, pitch);
                self.preview = None;
            }
        }
        if matches!(self.dialog, Some(Dialog::Tuner)) && now - self.tuner_clock > 0.08 {
            self.tuner_clock = now;
            let (samples, rate, level) = self.audio.recent_input();
            self.input_level = level;
            let window = &samples[samples.len().saturating_sub(4096)..];
            self.tuner = analysis::detect_pitch(window, rate as f32).map(analysis::nearest_note);
        }

        self.shortcuts(ctx);

        let top_area = egui::TopBottomPanel::top("top")
            .show(ctx, |ui| {
                self.menu_bar(ui);
                ui.add_space(2.0);
                self.transport(ui);
                ui.add_space(4.0);
            })
            .response
            .rect;
        let (mut bottom_area, mut library_area, mut right_area) = (Rect::NOTHING, Rect::NOTHING, Rect::NOTHING);
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| self.status_bar(ui));
        // The sound controls and the editor sit under the tracks, at a height
        // that only the user changes; or, on request, take the whole window.
        let full = self.show_bottom && self.bottom_full;
        if self.show_bottom && !full {
            let room = ctx.screen_rect().height();
            self.bottom_height = self.bottom_height.clamp(140.0, (room - 260.0).max(140.0));
            // The words, when shown, add their band to the panel rather than take the editor's room.
            let height = (self.bottom_height + self.words_band_height()).min((room - 200.0).max(140.0));
            let shown = egui::TopBottomPanel::bottom("bottom_panel").resizable(false).exact_height(height).show(ctx, |ui| {
                // The top edge is a handle: dragging it changes the height.
                let (edge, handle) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 5.0), egui::Sense::drag());
                let handle = handle.on_hover_cursor(egui::CursorIcon::ResizeVertical);
                if handle.dragged() {
                    self.bottom_height -= handle.drag_delta().y;
                }
                if handle.hovered() || handle.dragged() {
                    ui.painter().rect_filled(edge.shrink2(egui::vec2(0.0, 1.5)), 1.0, palette(1));
                }
                self.bottom_panel(ui);
            });
            bottom_area = shown.response.rect;
        }
        if !full {
            if self.show_library {
                library_area = egui::SidePanel::left("library").resizable(false).exact_width(200.0).show(ctx, |ui| self.library_panel(ui)).response.rect;
            }
            right_area = match self.right_panel {
                RightPanel::Loops => egui::SidePanel::right("loops").resizable(false).exact_width(300.0).show(ctx, |ui| self.loops_panel(ui)).response.rect,
                RightPanel::Notes => egui::SidePanel::right("notes").resizable(false).exact_width(260.0).show(ctx, |ui| self.notes_panel(ui)).response.rect,
                RightPanel::None => Rect::NOTHING,
            };
        }
        let central = egui::CentralPanel::default().show(ctx, |ui| if full { self.bottom_panel(ui) } else { self.tracks_area(ui) }).response.rect;
        self.file_drop(ctx);
        self.dialogs(ctx);
        self.set_list_window(ctx);

        // Quick help: what the pointer is over, for the status bar.
        self.help_text.clear();
        if let Some(at) = ctx.pointer_hover_pos().filter(|_| self.help) {
            let bottom = if self.bottom_tab == BottomTab::Controls {
                tr("Sound controls of the selected track: sound, equaliser, effects, plug-ins. Drag a box by its title to move it; right-click to hide it or to show the others.")
            } else {
                tr("Editor of the selected region. With notes: a click adds a note, dragging in empty space selects, right-click deletes, Q puts the selection back on the grid.")
            };
            let text = if top_area.contains(at) {
                tr("Menus and transport: play, record, loop, position, tempo, and the buttons that show or hide the panels.")
            } else if bottom_area.contains(at) || (full && central.contains(at)) {
                bottom
            } else if library_area.contains(at) {
                tr("Library: the sounds, by category. Choosing one gives it to the selected track.")
            } else if right_area.contains(at) && self.right_panel == RightPanel::Loops {
                tr("Loop browser: drag a loop onto a track, or under the tracks to make a new track for it.")
            } else if right_area.contains(at) {
                tr("Notepad: free text kept with the project.")
            } else if !central.contains(at) {
                tr("Status bar: what the program last did, and the state of the audio output.")
            } else if at.x < self.lane_left {
                tr("Track header: name, icon, mute, solo, volume and pan. Right-click it for the track's menu.")
            } else if self.lane_rects.first().is_some_and(|lane| at.y < lane.top()) {
                tr("Ruler: click to move the playhead; click in its upper half, then drag, to set the loop area.")
            } else {
                tr("Tracks: double-click makes a region, dragging moves it; its right edge changes its length (bottom) or repeats it (top). Loops can be dropped here.")
            };
            self.help_text = text.to_owned();
        }

        self.poll_recording();
        self.poll_words();
        self.poll_library();
        self.drive_video(ctx);
        self.fix_stale();
        self.sync(now);
        self.keep_session_now_and_then(now);
        let busy = self.playing || self.preview.is_some() || self.peak > 0.001 || self.auditioning.is_some();
        if busy || matches!(self.dialog, Some(Dialog::Tuner)) {
            ctx.request_repaint();
        } else if self.audio.midi_name.is_some() {
            // Keep the meters alive while a MIDI keyboard may be played.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// Drops selections that point at things an edit removed.
    fn fix_stale(&mut self) {
        // A request the note editor did not take (it is not shown) is forgotten.
        self.quantize_request = false;
        self.selected_track = self.selected_track.min(self.project.tracks.len().saturating_sub(1));
        if self.selected_region.is_some() && self.selection().is_none() {
            self.selected_region = None;
            self.selected_notes.clear();
        }
        if let Some((ti, ri)) = self.selection() {
            let count = self.project.tracks[ti].regions[ri].notes().len();
            self.selected_notes.retain(|i| *i < count);
        }
    }
}

// ----- files dropped from the file explorer ---------------------------------------------

impl App {
    /// A file dragged over the window from the file explorer: a line shows
    /// where on the tracks it would land, and a word says what would be done.
    /// Released, the file is taken in according to its kind.
    fn file_drop(&mut self, ctx: &egui::Context) {
        let (hovering, dropped) = ctx.input(|i| (!i.raw.hovered_files.is_empty(), i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect::<Vec<PathBuf>>()));
        if !hovering && dropped.is_empty() {
            return;
        }
        let pointer = ctx.pointer_hover_pos().or_else(|| ctx.pointer_interact_pos());
        let over_tracks = pointer.filter(|p| self.tracks_rect.contains(*p) && p.x >= self.lane_left);
        let bar = self.project.bar_len();
        let start = over_tracks.map(|p| ((self.scroll + f64::from((p.x - self.lane_left) / self.ppb)) / bar).floor().max(0.0) * bar);
        let target = over_tracks.and_then(|p| self.lane_rects.iter().position(|lane| lane.y_range().contains(p.y)));
        if hovering {
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("file_drop")));
            if let Some(start) = start {
                let x = self.lane_left + ((start - self.scroll) * f64::from(self.ppb)) as f32;
                let span = target.map_or(self.tracks_rect.y_range(), |ti| self.lane_rects[ti].y_range());
                painter.vline(x, span, stroke(2.0, palette(2)));
            }
            if let Some(p) = pointer {
                let text = if start.is_some() {
                    tr("Drop: audio or MIDI onto the tracks; a sound bank into the library")
                } else {
                    tr("Drop: audio or MIDI at the playhead; a sound bank into the library; a project to open it")
                };
                let galley = painter.layout_no_wrap(text.to_owned(), egui::FontId::proportional(13.0), self.theme.visuals().text_color());
                let rect = Rect::from_min_size(p + egui::vec2(14.0, 14.0), galley.size() + egui::vec2(12.0, 8.0));
                painter.rect_filled(rect, 4.0, self.theme.visuals().window_fill);
                painter.galley(rect.min + egui::vec2(6.0, 4.0), galley, self.theme.visuals().text_color());
            }
        }
        for path in dropped {
            self.drop_file(&path, target, start);
        }
    }

    /// Takes in a file by its kind: audio or MIDI go to the tracks, a sound
    /// bank to the library, a project is opened.
    pub fn drop_file(&mut self, path: &Path, target: Option<usize>, start: Option<f64>) {
        let extension = codec::extension_of(path);
        if extension == "ogproj" {
            self.open_recent(path);
        } else if codec::AUDIO_EXTENSIONS.contains(&extension.as_str()) {
            self.import_audio_file(path, target, start);
        } else if extension == "mid" || extension == "midi" {
            self.import_midi_file(path);
        } else if og_engine::guitar_pro::is_guitar_pro(path) {
            self.import_guitar_pro_file(path);
        } else if BANK_EXTENSIONS.contains(&extension.as_str()) {
            match self.install_bank(path, &Section::Own) {
                Ok(count) => {
                    self.show_library = true;
                    self.status = tf!("Sound bank loaded: {count} instruments added to the library." [count]);
                }
                Err(e) => self.status = tf!("Import failed: {e}" [e]),
            }
        } else {
            self.status = tf!("“{}” is not a file the program takes: audio (WAV, MP3, AAC, FLAC, Ogg), MIDI, sound bank (SF2, SF3, SFZ) or project.", path.display());
        }
    }
}

// ----- tablature sites ---------------------------------------------------------------

impl App {
    /// What became of the instruments of an imported song: `real` of its
    /// `count` tracks got one from a sound bank.
    fn instruments_sentence(real: usize, count: usize) -> String {
        if real == count {
            tr("Instruments from the installed sound bank.").to_owned()
        } else if real == 0 {
            tr("Built-in sounds stand in for the instruments: install a General MIDI bank (Library > Download sounds) for real ones.").to_owned()
        } else {
            tf!("{real} of {count} tracks got an instrument of the installed sound bank; the others keep a built-in sound." [real, count])
        }
    }

    /// Opens the window of the tablature sites, on the site last searched.
    pub fn open_sites(&mut self) {
        self.import_message.clear();
        self.sites.message.clear();
        self.sites.error = false;
        self.dialog = Some(Dialog::Sites);
    }

    /// Changes the site searched in the window; the results of the other are forgotten.
    pub fn sites_choose(&mut self, site: usize) {
        if self.sites.site != site && site < self.known_sites.len() {
            self.sites.site = site;
            self.sites.clear_results();
        }
    }

    /// Opens the window of the tablature sites on the site of this name.
    pub fn open_site_named(&mut self, name: &str) {
        if let Some(index) = self.known_sites.iter().position(|k| k.site.name.eq_ignore_ascii_case(name)) {
            self.sites_choose(index);
        }
        self.open_sites();
    }

    /// The description of the site searched.
    pub fn site(&self) -> Option<&og_engine::sites::Description> {
        self.known_sites.get(self.sites.site).map(|k| &k.site)
    }

    /// Whether the user agreed to have the program read the site searched.
    pub fn site_confirmed(&self) -> bool {
        self.site().is_some_and(|site| self.settings.sites_confirmed.iter().any(|name| name.eq_ignore_ascii_case(&site.name)))
    }

    /// The user agrees to have the program read the site searched.
    pub fn confirm_site(&mut self) {
        let Some(name) = self.site().map(|s| s.name.clone()) else { return };
        if !self.site_confirmed() {
            self.settings.sites_confirmed.push(name);
            self.save_settings();
        }
    }

    pub fn import_guitar_pro(&mut self) {
        let Some(path) = files::open_file(tr("Guitar Pro or MusicXML score"), &og_engine::guitar_pro::Score::EXTENSIONS) else { return };
        self.import_guitar_pro_file(&path);
    }

    /// Makes a new project of a Guitar Pro file: one track a part.
    pub fn import_guitar_pro_file(&mut self, path: &Path) {
        let score = match std::fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| og_engine::guitar_pro::parse(&bytes)) {
            Ok(score) => score,
            Err(e) => {
                let file = path.file_name().map_or_else(String::new, |f| f.to_string_lossy().into_owned());
                self.import_message = tf!("“{file}” was not read: {e}" [file, e]);
                self.status.clone_from(&self.import_message);
                return;
            }
        };
        let mut project = score.project();
        if project.name.trim().is_empty() {
            project.name = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        }
        let count = project.tracks.len();
        let name = project.name.clone();
        let real = self.assign_gm_sounds(&mut project, &score.programs());
        self.replace_project(project, None);
        self.status = format!("{} {}", tf!("“{name}”: {count} tracks read from the file." [name, count]), Self::instruments_sentence(real, count));
        // The words: the file's, or else asked of LRCLIB.
        if self.project.lyrics.is_empty() {
            self.search_words();
        }
        self.import_message.clear();
        if matches!(self.dialog, Some(Dialog::Sites)) {
            self.dialog = None;
        }
    }

    /// Starts a search on the chosen site, once the user agreed to it; a site
    /// that lists what it has is asked with no words too.
    pub fn sites_search(&mut self) {
        let Some(site) = self.site().filter(|s| !s.open_only).cloned() else { return };
        if (self.sites.search.trim().is_empty() && !site.lists_only()) || !self.site_confirmed() {
            return;
        }
        let token = self.site_token();
        if site.login.is_some() && token.is_none() {
            return;
        }
        self.sites.clear_results();
        self.sites.finding = Some(crate::sites::start_search(site, self.sites.search.clone(), crate::i18n::language(), token));
    }

    /// The key of the program at the site searched, when the site asks for
    /// one: the description's, or the one the user made.
    pub fn site_key(&self) -> String {
        let Some(site) = self.site() else { return String::new() };
        match &site.login {
            Some(login) if !login.client_id.is_empty() => login.client_id.clone(),
            Some(_) => self.settings.site_keys.get(&site.name).cloned().unwrap_or_default(),
            None => String::new(),
        }
    }

    /// Whether the user signed in to the site searched.
    pub fn site_signed_in(&self) -> bool {
        self.site().and_then(|s| self.settings.site_logins.get(&s.name)).is_some_and(|t| !t.access.is_empty())
    }

    /// The token of the user at the site searched, renewed when it expired;
    /// none when the site needs none or the user is not signed in.
    pub fn site_token(&mut self) -> Option<String> {
        let site = self.site()?.clone();
        let login = site.login.as_ref()?;
        let token = self.settings.site_logins.get(&site.name)?.clone();
        if token.fresh() {
            return Some(token.access);
        }
        match crate::login::refresh(login, &self.site_key(), &token) {
            Ok(renewed) => {
                let access = renewed.access.clone();
                self.settings.site_logins.insert(site.name.clone(), renewed);
                self.save_settings();
                Some(access)
            }
            Err(_) => {
                self.settings.site_logins.remove(&site.name);
                self.save_settings();
                None
            }
        }
    }

    /// Asks the site searched for a code to enter on its page.
    pub fn sites_sign_in(&mut self) {
        let Some(login) = self.site().and_then(|s| s.login.clone()) else { return };
        let key = self.site_key();
        if key.is_empty() {
            return;
        }
        self.sites.message.clear();
        self.sites.error = false;
        self.sites.device = None;
        self.sites.logging_in = Some(crate::login::start_device_code(login, key));
    }

    /// Forgets the token of the site searched.
    pub fn sites_sign_out(&mut self) {
        if let Some(name) = self.site().map(|s| s.name.clone()) {
            self.settings.site_logins.remove(&name);
            self.save_settings();
            self.sites.clear_results();
        }
    }

    /// Keeps the key typed for the site searched; an empty one forgets it.
    pub fn sites_keep_key(&mut self) {
        let key = self.sites.key_text.trim().to_owned();
        if let Some(name) = self.site().map(|s| s.name.clone()) {
            if key.is_empty() {
                self.settings.site_keys.remove(&name);
            } else {
                self.settings.site_keys.insert(name, key);
            }
            self.save_settings();
        }
    }

    /// Writes the ticked files the site listed into the library, in a folder
    /// of the site and of what they are, with where they came from.
    pub fn sites_save_files(&mut self) {
        let (Some(site), Some(found), Some(files)) = (self.site().cloned(), self.sites.chosen_song().cloned(), self.sites.files.as_ref()) else { return };
        let kind = if site.gives == "impulse" { downloads::Kind::Impulses } else { downloads::Kind::Captures };
        let folder = self.data_dir.join(kind.folder()).join(downloads::safe_name(&site.name)).join(downloads::safe_name(&found.label));
        let ticked: Vec<&(String, Vec<u8>)> = files.iter().zip(&self.sites.chosen).filter(|(_, c)| **c).map(|(f, _)| f).collect();
        if ticked.is_empty() {
            return;
        }
        let written = (|| -> Result<usize, String> {
            std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
            for (name, bytes) in &ticked {
                std::fs::write(folder.join(downloads::safe_name(name)), bytes).map_err(|e| e.to_string())?;
            }
            let get = |key: &str| found.vars.get(key).cloned().unwrap_or_default();
            let source = format!("Site: {}\nName: {}\nBy: {}\nLicence: {}\nPage: {}\nDate (Unix seconds): {}\n", site.name, found.label, get("user"), get("license"), get("url"), crate::sessions::now());
            std::fs::write(folder.join("SOURCE.txt"), source).map_err(|e| e.to_string())?;
            Ok(ticked.len())
        })();
        match written {
            Ok(count) => {
                let where_ = folder.display().to_string();
                self.sites.saved = Some(folder);
                self.sites.message = tf!("{count} file(s) added to the library: {where_}" [count, where_]);
                self.sites.error = false;
                self.status.clone_from(&self.sites.message);
            }
            Err(e) => {
                self.sites.message = tf!("The files were not written: {e}" [e]);
                self.sites.error = true;
            }
        }
    }

    /// Chooses a song among the results: when the site listed its tracks,
    /// they are all ticked; otherwise its file is read in the background,
    /// and its parts are ticked once read.
    pub fn sites_select(&mut self, index: usize) {
        let Some(found) = self.sites.found.as_ref().and_then(|r| r.as_ref().ok()).and_then(|list| list.get(index)).cloned() else { return };
        let Some(site) = self.site().cloned() else { return };
        self.sites.selected = Some(index);
        self.sites.score = None;
        self.sites.opening = None;
        self.sites.create_when_read = false;
        self.sites.files = None;
        self.sites.saved = None;
        let token = self.site_token();
        // The recording of the song in the music library, if it is there.
        let field = |name: &str| found.vars.get(name).cloned().unwrap_or_default();
        self.sites.recording = self.library_recording(&field("artist"), &field("title"));
        self.sites.use_recording = self.sites.recording.is_some();
        self.sites.want_words = true;
        self.sites.message.clear();
        self.sites.error = false;
        if found.tracks.is_empty() {
            self.sites.chosen.clear();
            self.sites.opening = Some(crate::sites::start_open(site, found, Vec::new(), false, None, token));
        } else {
            self.sites.chosen = vec![true; found.tracks.len()];
        }
    }

    /// Makes the project of the ticked tracks: from the score read, or,
    /// when the site listed the tracks, once the ticked ones are read.
    pub fn sites_create(&mut self) {
        let ticked: Vec<usize> = self.sites.chosen.iter().enumerate().filter(|(_, c)| **c).map(|(i, _)| i).collect();
        if ticked.is_empty() || self.sites.opening.is_some() {
            return;
        }
        let Some(found) = self.sites.chosen_song().cloned() else { return };
        if !found.tracks.is_empty() {
            let Some(site) = self.site().cloned() else { return };
            self.sites.message.clear();
            self.sites.error = false;
            self.sites.create_when_read = true;
            // The singing part is fetched for the words of the song, ticked or not.
            let words_from = found.tracks.iter().position(|t| t.vocal).filter(|_| self.sites.want_words);
            let token = self.site_token();
            self.sites.opening = Some(crate::sites::start_open(site, found, ticked, self.sites.want_video, words_from, token));
            return;
        }
        let Some(score) = self.sites.score.as_ref() else { return };
        let parts: Vec<og_engine::songsterr::Part> = ticked.iter().filter_map(|i| score.parts.get(*i).cloned()).collect();
        // The words, from whichever part sings, ticked or not.
        let words = og_engine::lyrics::from_parts(&score.parts);
        let score = og_engine::guitar_pro::Score { title: score.title.clone(), artist: score.artist.clone(), parts };
        self.sites_make_project(&score, &[], &[], words);
    }

    /// Replaces the project with the parts of a score read from the site searched.
    fn sites_make_project(&mut self, score: &og_engine::guitar_pro::Score, messages: &[String], videos: &[crate::sites::VideoOffer], words: og_engine::lyrics::Lyrics) {
        let mut project = og_engine::songsterr::build_project(&score.artist, &score.title, &score.parts);
        let count = project.tracks.len();
        let name = project.name.clone();
        let site = self.site().map_or_else(String::new, |s| s.name.clone());
        let real = self.assign_gm_sounds(&mut project, &score.programs());
        self.replace_project(project, None);
        let mut status = tf!("“{name}”: {count} tracks read from {site}." [name, count, site]);
        for message in messages {
            status.push(' ');
            status.push_str(message);
        }
        status.push(' ');
        status.push_str(&Self::instruments_sentence(real, count));
        self.status = status;
        // The videos of the song, in step with the tablature: the original one
        // played, the others kept to be chosen.
        if !videos.is_empty() {
            let bars = score.parts.first().map(|p| p.bar_starts.clone()).unwrap_or_default();
            self.add_video_track(videos, &bars);
            // Played with it at once: the tracks of the tablature silent, unless turned on.
            self.project.follow = true;
        } else if self.sites.want_video && self.site().is_some_and(|s| s.videos.is_some()) {
            self.status.push(' ');
            self.status.push_str(tr("The site has no video in step with this tablature."));
        }
        // The words: the tablature's, or else asked of LRCLIB.
        if self.sites.want_words {
            if words.is_empty() {
                self.search_words();
            } else {
                self.project.lyrics = words;
            }
        }
        // A recording of the song, found in step in the background.
        if let (true, Some(path)) = (self.sites.use_recording, self.sites.recording.clone()) {
            self.link_recording(path);
        }
        self.sites.message.clear();
        self.dialog = None;
        self.set_project_made();
    }

    fn poll_sites(&mut self) {
        if let Some(result) = take_result(&mut self.sites.finding, tr("the search stopped unexpectedly")) {
            self.sites.selected = None;
            self.sites.chosen.clear();
            self.sites.score = None;
            self.sites.found = Some(result);
            if std::mem::take(&mut self.sites.pick_first) {
                self.sites_select(0);
            }
        }
        if let Some(result) = take_result(&mut self.sites.logging_in, tr("the sign-in stopped unexpectedly")) {
            match result {
                Ok(code) => {
                    if let Some(login) = self.site().and_then(|s| s.login.clone()) {
                        self.sites.waiting = Some(crate::login::start_waiting(login, self.site_key(), code.clone()));
                    }
                    self.sites.device = Some(code);
                }
                Err(e) => {
                    self.sites.message = tf!("Sign-in failed: {e}" [e]);
                    self.sites.error = true;
                }
            }
        }
        if let Some(result) = take_result(&mut self.sites.waiting, tr("the sign-in stopped unexpectedly")) {
            self.sites.device = None;
            match (result, self.site().map(|s| s.name.clone())) {
                (Ok(token), Some(name)) => {
                    self.settings.site_logins.insert(name, token);
                    self.save_settings();
                    self.sites.message = tr("Signed in.").to_owned();
                    self.sites.error = false;
                }
                (Err(e), _) => {
                    self.sites.message = tf!("Sign-in failed: {e}" [e]);
                    self.sites.error = true;
                }
                _ => {}
            }
        }
        if let Some(result) = take_result(&mut self.sites.opening, tr("the import stopped unexpectedly")) {
            match result {
                Ok(opened) if !self.site().is_none_or(|s| s.is_tablature()) => {
                    self.sites.chosen = vec![true; opened.files.len()];
                    self.sites.files = Some(opened.files);
                    for message in &opened.messages {
                        self.sites.message.push_str(message);
                        self.sites.message.push(' ');
                    }
                }
                Ok(opened) if std::mem::take(&mut self.sites.create_when_read) => self.sites_make_project(&opened.score, &opened.messages, &opened.videos, opened.lyrics),
                Ok(opened) => {
                    self.sites.chosen = vec![true; opened.score.parts.len()];
                    self.sites.score = Some(opened.score);
                }
                Err(e) => {
                    self.sites.create_when_read = false;
                    self.sites.message = tf!("The tablature was not read: {e}" [e]);
                    self.sites.error = true;
                }
            }
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // The window, for the video player to show inside it.
        #[cfg(windows)]
        if self.window_handle == 0 {
            use raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(RawWindowHandle::Win32(window)) = frame.window_handle().map(|h| h.as_raw()) {
                self.window_handle = window.hwnd.get();
            }
        }
        #[cfg(not(windows))]
        let _ = frame;
        self.ui(ctx);
    }
}
