//! The actions an AI (or anyone) writes for the MIDI Pilot: patches of the
//! BOSS pedals set parameter by parameter with the values the pedal shows,
//! their program change map and system, and the banks and presets of a G3S
//! backup. One JSON format, read from a file, from the local API, and
//! written back by the exports. The guide that teaches the format to an AI
//! is made here too, from the descriptions, so that it is always right.
//!
//! ```json
//! { "opengaragemusic": 1,
//!   "actions": [
//!     { "pedal": "rv500", "memory": "50C", "name": "COMF NUMB", "from": "init",
//!       "set": { "Reverb : MODE": "HALL", "Reverb : time": "4.0", "Reverb : Effect Level": 30 } },
//!     { "pedal": "dd500", "pc_map": { "1": "81A" } },
//!     { "pedal": "dd500", "system": { "MIDI Rx Channel": "Ch.1" } },
//!     { "g3s": "bank", "bank": 12, "name": "COMFORT.|NUMB" },
//!     { "g3s": "preset", "bank": 12, "switch": 2, "name": "INTRO", "loops": [9, 10, 12],
//!       "midi": [ { "kind": "PC down", "channel": 1, "program": 4 } ] }
//!   ] }
//! ```

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

use crate::devices::{self, Description, Encoding, Param, Value};
use crate::g3s::{self, Backup, MidiLine, LOOPS, MIDI_KINDS, MIDI_LINES, PRESETS};
use crate::setlist::{same, Device, Pedal, SetList, SetSong, Step, StepMessage, PEDAL_KINDS};

/// The version of the format this program reads and writes.
pub const FORMAT: u32 = 1;
/// Most actions in one script.
pub const MOST_ACTIONS: usize = 400;

/// A script: its actions, in order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Script {
    #[serde(rename = "opengaragemusic", default = "format")]
    pub version: u32,
    /// What the user's pedalboard is, in a file exported for an AI; not read back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pedalboard: Option<Pedalboard>,
    #[serde(default)]
    pub actions: Vec<Action>,
}

/// The user's pedalboard, as the file exported for an AI describes it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pedalboard {
    /// The devices of the chain: name, model, channel from 1.
    pub devices: Vec<BoardDevice>,
    /// The switcher of the chain, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switcher: Option<BoardSwitcher>,
    /// "switcher" or "chain".
    pub mode: String,
    /// How the switcher's mode numbers the songs.
    pub numbering: BoardNumbering,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardDevice {
    pub name: String,
    pub model: String,
    pub channel: u8,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardSwitcher {
    pub device: String,
    /// The pedals of each loop, by the loop's number from 1.
    pub loops: BTreeMap<String, Vec<Pedal>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub signal_order: Vec<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bank_switches: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BoardNumbering {
    pub channel: u8,
    pub message: String,
    pub numbers_from_one: bool,
}

fn format() -> u32 {
    FORMAT
}

/// What a patch starts from before its values are set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum From {
    /// The patch the memory holds now.
    #[default]
    Memory,
    /// Every parameter at its first value.
    Init,
}

/// One action. A pedal action names the pedal by its key and does one thing:
/// a memory (`memory` with `set`), the program change map, or the system.
/// A G3S action says what it is about with `g3s`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Action {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pedal: Option<String>,
    /// The memory written, "50C".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<String>,
    /// The name given to the patch or the preset, or the bank.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<From>,
    /// The parameters set, by name, with the values the pedal shows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set: Option<BTreeMap<String, Json>>,
    /// Program change → memory: `"1": "81A"`, `"2:5": "03A"` (bank select 2, program 5).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pc_map: Option<BTreeMap<String, String>>,
    /// The system settings set, like `set`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<BTreeMap<String, Json>>,
    /// "bank", "preset" or "remove_bank".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub g3s: Option<String>,
    /// The bank, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song_mode: Option<bool>,
    /// The footswitch of a preset, from 1 (15 to 28 the hidden ones).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switch: Option<usize>,
    /// The loops on, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loops: Option<Vec<u8>>,
    /// The twelve loops in the order of the signal, from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<u8>>,
    /// The MIDI lines of the preset, replacing all of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub midi: Option<Vec<Midi>>,
    /// For `"g3s": "loops"`: the pedals of each loop, by the loop's number from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pedals: Option<BTreeMap<String, Vec<Pedal>>>,
    /// For `"g3s": "loops"`: the order the signal goes through the twelve loops.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_order: Option<Vec<u8>>,
    /// For `"g3s": "loops"`: the footswitches that change banks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank_switches: Option<Vec<u8>>,
    /// "add", "set" or "remove": a song of the MIDI Pilot's list.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub song: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The text of the song's button; empty for "Artist - Title".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display: Option<String>,
    /// The song's number, sent in the switcher's mode (0 to 128).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<u8>,
    /// The song's MIDI messages in the chain's mode, replacing all of them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub messages: Option<Vec<Message>>,
}

/// One message of a song, to a device of the chain, as the script writes it:
/// a program change (`program`, with `bank` for a bank select before), a
/// control change (`controller` and `value`), or `sysex` in hexadecimal.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    pub device: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bank: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sysex: Option<String>,
    /// A pause after it, in milliseconds.
    #[serde(skip_serializing_if = "is_zero")]
    pub delay_ms: u16,
}

fn is_zero(n: &u16) -> bool {
    *n == 0
}

/// One MIDI line of a G3S preset, as the script writes it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Midi {
    /// One of the kinds of `g3s::MIDI_KINDS`: "PC down", "CC down"…
    pub kind: String,
    /// From 1 to 16.
    pub channel: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub controller: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u8>,
}

/// What an action is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Patch,
    PcMap,
    System,
    G3sBank,
    G3sPreset,
    G3sRemoveBank,
    /// The pedals in the loops of the switcher, in the chain's settings.
    G3sLoops,
    SongAdd,
    SongSet,
    SongRemove,
}

impl Script {
    pub fn parse(text: &str) -> Result<Script, String> {
        let value: Json = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if !value.is_object() {
            return Err("a script is a JSON object with \"actions\"".to_owned());
        }
        let script: Script = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if script.version > FORMAT {
            return Err(format!("written for a newer version of the program (format {}, this version reads up to {FORMAT})", script.version));
        }
        if script.actions.len() > MOST_ACTIONS {
            return Err(format!("more than {MOST_ACTIONS} actions"));
        }
        for (i, action) in script.actions.iter().enumerate() {
            action.kind().map_err(|e| format!("action {}: {e}", i + 1))?;
        }
        Ok(script)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }
}

impl Action {
    /// What the action is about, or why it is not one.
    pub fn kind(&self) -> Result<Kind, String> {
        if let Some(what) = &self.song {
            if self.pedal.is_some() || self.g3s.is_some() {
                return Err("an action is for a song, a pedal or the g3s, not several".to_owned());
            }
            return match what.as_str() {
                "add" => Ok(Kind::SongAdd),
                "set" => Ok(Kind::SongSet),
                "remove" => Ok(Kind::SongRemove),
                other => Err(format!("song is \"add\", \"set\" or \"remove\", not \"{other}\"")),
            };
        }
        match (&self.pedal, &self.g3s) {
            (Some(_), None) => match (self.memory.is_some(), self.pc_map.is_some(), self.system.is_some()) {
                (true, false, false) => Ok(Kind::Patch),
                (false, true, false) => Ok(Kind::PcMap),
                (false, false, true) => Ok(Kind::System),
                (false, false, false) => Err("a pedal action has a memory, a pc_map or a system".to_owned()),
                _ => Err("a pedal action does one thing: a memory, a pc_map or a system".to_owned()),
            },
            (None, Some(what)) => match what.as_str() {
                "bank" => Ok(Kind::G3sBank),
                "preset" => Ok(Kind::G3sPreset),
                "remove_bank" => Ok(Kind::G3sRemoveBank),
                "loops" => Ok(Kind::G3sLoops),
                other => Err(format!("g3s is \"bank\", \"preset\", \"remove_bank\" or \"loops\", not \"{other}\"")),
            },
            (Some(_), Some(_)) => Err("an action is for a pedal or for the g3s, not both".to_owned()),
            (None, None) => Err("an action names a pedal or the g3s".to_owned()),
        }
    }

    /// The action in a sentence, for the user to review.
    pub fn summary(&self, description: Option<&Description>) -> String {
        let pedal = description.map_or_else(|| self.pedal.clone().unwrap_or_default(), |d| d.name.clone());
        match self.kind() {
            Ok(Kind::Patch) => {
                let count = self.set.as_ref().map_or(0, BTreeMap::len);
                let from = if self.from == Some(From::Init) { "from init" } else { "from the memory" };
                let name = self.name.as_deref().map_or(String::new(), |n| format!(" named \"{n}\""));
                format!("{pedal}: write the memory {}{name}, {count} parameters set {from}", self.memory.clone().unwrap_or_default())
            }
            Ok(Kind::PcMap) => format!("{pedal}: {} entries of the program change map", self.pc_map.as_ref().map_or(0, BTreeMap::len)),
            Ok(Kind::System) => format!("{pedal}: {} system settings", self.system.as_ref().map_or(0, BTreeMap::len)),
            Ok(Kind::G3sBank) => format!("G3S: bank {} named \"{}\"", self.bank.unwrap_or(0), self.name.clone().unwrap_or_default()),
            Ok(Kind::G3sPreset) => format!("G3S: bank {}, switch {} \"{}\": {} loops, {} MIDI lines", self.bank.unwrap_or(0), self.switch.unwrap_or(0), self.name.clone().unwrap_or_default(), self.loops.as_ref().map_or(0, Vec::len), self.midi.as_ref().map_or(0, Vec::len)),
            Ok(Kind::G3sRemoveBank) => format!("G3S: remove bank {}", self.bank.unwrap_or(0)),
            Ok(Kind::G3sLoops) => format!("G3S: the pedals of {} loops{}", self.pedals.as_ref().map_or(0, BTreeMap::len), if self.signal_order.is_some() || self.bank_switches.is_some() { ", the signal order or the bank switches" } else { "" }),
            Ok(Kind::SongAdd) => format!("Songs: add \"{}\" with {} messages", self.song_label(), self.messages.as_ref().map_or(0, Vec::len)),
            Ok(Kind::SongSet) => format!("Songs: change \"{}\"{}", self.song_label(), self.messages.as_ref().map_or(String::new(), |m| format!(", {} messages", m.len()))),
            Ok(Kind::SongRemove) => format!("Songs: remove \"{}\"", self.song_label()),
            Err(e) => e,
        }
    }
}

impl Action {
    /// The song an action names: "Artist - Title".
    fn song_label(&self) -> String {
        let song = SetSong { artist: self.artist.clone().unwrap_or_default(), title: self.title.clone().unwrap_or_default(), ..SetSong::default() };
        song.label()
    }
}

// ----- the songs of the MIDI Pilot -----------------------------------------------------

/// A message of the script as a step of a song.
fn step_from(message: &Message, list: &SetList) -> Result<Step, String> {
    if list.device(&message.device).is_none() {
        return Err(format!("the device \"{}\" is not in the chain (one of {})", message.device, list.devices.iter().map(|d| format!("\"{}\"", d.name)).collect::<Vec<_>>().join(", ")));
    }
    let too_big = |n: Option<u8>| n.is_some_and(|n| n > 127);
    if too_big(message.program) || too_big(message.bank) || too_big(message.controller) || too_big(message.value) {
        return Err("program, bank, controller and value go from 0 to 127".to_owned());
    }
    let kind = match (message.program, message.controller, &message.sysex) {
        (Some(program), None, None) => StepMessage::Program { bank: message.bank, program },
        (None, Some(number), None) => StepMessage::Control { number, value: message.value.unwrap_or(0) },
        (None, None, Some(hex)) => StepMessage::SysEx(crate::devices::from_hex(hex).ok_or_else(|| format!("\"{hex}\" is not hexadecimal bytes"))?),
        _ => return Err(format!("a message to \"{}\" is a program, a controller or a sysex, one of them", message.device)),
    };
    Ok(Step { device: list.device(&message.device).map_or_else(|| message.device.clone(), |d| d.name.clone()), message: kind, delay_ms: message.delay_ms.min(500) })
}

/// A step of a song as a message of the script.
fn message_from(step: &Step) -> Message {
    let mut message = Message { device: step.device.clone(), delay_ms: step.delay_ms, ..Message::default() };
    match &step.message {
        StepMessage::Program { bank, program } => {
            message.program = Some(*program);
            message.bank = *bank;
        }
        StepMessage::Control { number, value } => {
            message.controller = Some(*number);
            message.value = Some(*value);
        }
        StepMessage::SysEx(bytes) => message.sysex = Some(crate::devices::hex(bytes)),
    }
    message
}

/// The place of the song an action names, by artist and title (up to case and signs).
fn song_place(list: &SetList, action: &Action) -> Option<usize> {
    let (artist, title) = (action.artist.clone().unwrap_or_default(), action.title.clone().unwrap_or_default());
    list.songs.iter().position(|s| same(&s.artist, &artist) && same(&s.title, &title))
}

/// Applies a song action to the list; says what was done.
pub fn apply_song(list: &mut SetList, action: &Action) -> Result<String, String> {
    let kind = action.kind()?;
    if action.artist.as_deref().unwrap_or_default().trim().is_empty() && action.title.as_deref().unwrap_or_default().trim().is_empty() {
        return Err("a song has an artist or a title".to_owned());
    }
    if action.number.is_some_and(|n| n > 128) {
        return Err("a song's number goes from 0 to 128".to_owned());
    }
    let steps = match &action.messages {
        Some(messages) => Some(messages.iter().map(|m| step_from(m, list)).collect::<Result<Vec<Step>, String>>()?),
        None => None,
    };
    let label = action.song_label();
    match (kind, song_place(list, action)) {
        (Kind::SongAdd, Some(_)) => Err(format!("\"{label}\" is in the list already: use \"set\"")),
        (Kind::SongAdd, None) => {
            let number = action.number.unwrap_or_else(|| list.songs.iter().map(|s| s.number).max().map_or(1, |n| n.saturating_add(1).min(128)));
            list.songs.push(SetSong { number, artist: action.artist.clone().unwrap_or_default(), title: action.title.clone().unwrap_or_default(), display: action.display.clone().unwrap_or_default(), steps: steps.unwrap_or_default(), ..SetSong::default() });
            Ok(format!("\"{label}\" added as song {number}"))
        }
        (Kind::SongSet, Some(place)) => {
            let song = &mut list.songs[place];
            if let Some(number) = action.number {
                song.number = number;
            }
            if let Some(display) = &action.display {
                song.display.clone_from(display);
            }
            if let Some(steps) = steps {
                song.steps = steps;
            }
            Ok(format!("\"{label}\" changed"))
        }
        (Kind::SongRemove, Some(place)) => {
            list.songs.remove(place);
            Ok(format!("\"{label}\" removed"))
        }
        (_, None) => Err(format!("\"{label}\" is not in the list (the artist and the title are compared up to case and signs)")),
        _ => Err("not a song action".to_owned()),
    }
}

/// The songs of the list as the actions that would add them again.
pub fn export_songs(list: &SetList) -> Vec<Action> {
    list.songs
        .iter()
        .map(|song| Action {
            song: Some("add".to_owned()),
            artist: Some(song.artist.clone()),
            title: Some(song.title.clone()),
            display: (!song.display.is_empty()).then(|| song.display.clone()),
            number: Some(song.number),
            messages: Some(song.steps.iter().map(message_from).collect()),
            ..Action::default()
        })
        .collect()
}

// ----- the switcher's loops -----------------------------------------------------------

/// Applies a `loops` action to the switcher of the chain (the device of model
/// `g3s`); says what was done.
pub fn apply_loops(list: &mut SetList, action: &Action) -> Result<String, String> {
    let names: Vec<String> = list.devices.iter().map(|d| d.name.clone()).collect();
    let Some(switcher) = list.devices.iter_mut().find(|d| d.model == "g3s") else { return Err("no device of model g3s in the chain: add one under Devices".to_owned()) };
    let mut done = Vec::new();
    if let Some(pedals) = &action.pedals {
        if switcher.loops.len() < g3s::LOOPS {
            switcher.loops.resize(g3s::LOOPS, Vec::new());
        }
        for (key, in_loop) in pedals {
            let n: usize = key.trim().parse().ok().filter(|n| (1..=g3s::LOOPS).contains(n)).ok_or_else(|| format!("\"{key}\" is no loop: 1 to {}", g3s::LOOPS))?;
            for pedal in in_loop {
                if pedal.name.trim().is_empty() {
                    return Err(format!("loop {n}: a pedal has a name"));
                }
                if !pedal.kind.is_empty() && !PEDAL_KINDS.contains(&pedal.kind.as_str()) {
                    return Err(format!("loop {n}: \"{}\" is no kind of pedal (one of {})", pedal.kind, PEDAL_KINDS.join(", ")));
                }
                if !pedal.device.is_empty() && !names.iter().any(|d| same(d, &pedal.device)) {
                    return Err(format!("loop {n}: the device \"{}\" is not in the chain", pedal.device));
                }
            }
            switcher.loops[n - 1] = in_loop.iter().map(|p| Pedal { name: p.name.trim().to_owned(), kind: p.kind.clone(), device: p.device.clone() }).collect();
        }
        done.push(format!("{} loops", pedals.len()));
    }
    if let Some(order) = &action.signal_order {
        let mut sorted = order.clone();
        sorted.sort_unstable();
        if sorted.len() != g3s::LOOPS || sorted.iter().enumerate().any(|(i, n)| usize::from(*n) != i + 1) {
            return Err("signal_order names the twelve loops once each".to_owned());
        }
        switcher.signal_order.clone_from(order);
        done.push("the signal order".to_owned());
    }
    if let Some(switches) = &action.bank_switches {
        if switches.iter().any(|s| *s == 0 || usize::from(*s) > 14) {
            return Err("bank_switches are footswitches from 1 to 14".to_owned());
        }
        switcher.bank_switches.clone_from(switches);
        done.push("the bank switches".to_owned());
    }
    if done.is_empty() {
        return Err("a loops action has pedals, a signal_order or bank_switches".to_owned());
    }
    Ok(format!("G3S: {} set", done.join(", ")))
}

/// The switcher's loops as the action that would set them again.
pub fn export_loops(switcher: &Device) -> Action {
    let pedals: BTreeMap<String, Vec<Pedal>> = switcher.loops.iter().enumerate().filter(|(_, p)| !p.is_empty()).map(|(n, p)| ((n + 1).to_string(), p.clone())).collect();
    Action {
        g3s: Some("loops".to_owned()),
        pedals: Some(pedals),
        signal_order: (!switcher.signal_order.is_empty()).then(|| switcher.signal_order.clone()),
        bank_switches: (!switcher.bank_switches.is_empty()).then(|| switcher.bank_switches.clone()),
        ..Action::default()
    }
}

/// The whole pedalboard for an AI: what the chain is, and every setting known
/// as the actions that would make it again (songs, switcher loops and backup,
/// the memories and program change maps of the pedals).
pub fn pedalboard(list: &SetList, backup: Option<&Backup>, pedals: &[(Description, devices::PedalFile)]) -> Script {
    let switcher = list.devices.iter().find(|d| d.model == "g3s");
    let board = Pedalboard {
        devices: list.devices.iter().map(|d| BoardDevice { name: d.name.clone(), model: d.model.clone(), channel: d.channel + 1 }).collect(),
        switcher: switcher.map(|s| BoardSwitcher {
            device: s.name.clone(),
            loops: s.loops.iter().enumerate().filter(|(_, p)| !p.is_empty()).map(|(n, p)| ((n + 1).to_string(), p.clone())).collect(),
            signal_order: s.signal_order.clone(),
            bank_switches: s.bank_switches.clone(),
        }),
        mode: format!("{:?}", list.mode).to_ascii_lowercase(),
        numbering: BoardNumbering { channel: list.channel + 1, message: format!("{:?}", list.message), numbers_from_one: list.numbers_from_one },
    };
    let mut actions = Vec::new();
    if let Some(s) = switcher {
        if !s.loops.is_empty() || !s.signal_order.is_empty() || !s.bank_switches.is_empty() {
            actions.push(export_loops(s));
        }
    }
    actions.extend(export_songs(list));
    if let Some(backup) = backup {
        actions.extend(export_g3s(backup));
    }
    for (description, file) in pedals {
        for label in file.patches.keys() {
            if let Some(number) = description.number(label) {
                if let Some(raw) = file.patch(description, number) {
                    actions.push(export_patch(description, number, &raw));
                }
            }
        }
        let map: BTreeMap<String, String> = file.pc_map.iter().enumerate().filter(|(i, n)| usize::from(**n) != *i).map(|(i, n)| (if i < 128 { i.to_string() } else { format!("{}:{}", i / 128, i % 128) }, description.label(usize::from(*n)))).collect();
        if !map.is_empty() {
            actions.push(Action { pedal: Some(description.key.clone()), pc_map: Some(map), ..Action::default() });
        }
    }
    Script { version: FORMAT, pedalboard: Some(board), actions }
}

// ----- the pedals ---------------------------------------------------------------------

/// The value of a parameter from what the script writes: a label of its
/// list (whatever the case), a number as the pedal shows it, or a text.
pub fn value_from(description: &Description, param: &Param, given: &Json) -> Result<Value, String> {
    if param.enc == Encoding::Ascii {
        return match given {
            Json::String(text) => Ok(Value::Text(text.clone())),
            Json::Number(n) => Ok(Value::Text(n.to_string())),
            _ => Err(format!("{}: a text is expected", param.name)),
        };
    }
    let labels = description.labels_of(param);
    if let Json::String(text) = given {
        let wanted = text.trim();
        if let Some(labels) = labels {
            if let Some(i) = labels.iter().position(|l| l.eq_ignore_ascii_case(wanted)) {
                return Ok(Value::Number(param.min + i as i64));
            }
        }
        let number: f64 = wanted.parse().map_err(|_| {
            let among = labels.map_or(String::new(), |l| format!(" (one of {})", l.iter().take(12).map(|s| format!("\"{s}\"")).collect::<Vec<_>>().join(", ")));
            format!("{}: \"{wanted}\" is not a value it takes{among}", param.name)
        })?;
        return number_value(param, number);
    }
    match given.as_f64() {
        Some(number) => number_value(param, number),
        None => Err(format!("{}: a value is expected", param.name)),
    }
}

/// A number as the pedal shows it, taken to the raw value.
fn number_value(param: &Param, shown: f64) -> Result<Value, String> {
    let raw = shown.round() as i64 + param.offset;
    if raw < param.min || raw > param.max {
        return Err(format!("{}: {shown} is out of {} to {}", param.name, param.min - param.offset, param.max - param.offset));
    }
    Ok(Value::Number(raw))
}

/// Sets parameters of a patch (or of the system) by name.
pub fn apply_set(description: &Description, params: &[Param], raw: &mut [u8], set: &BTreeMap<String, Json>) -> Result<(), String> {
    for (name, given) in set {
        let param = params.iter().find(|p| p.name == *name).or_else(|| params.iter().find(|p| p.name.eq_ignore_ascii_case(name))).ok_or_else(|| {
            let close: Vec<&str> = params.iter().filter(|p| p.name.to_ascii_lowercase().contains(&name.to_ascii_lowercase())).take(5).map(|p| p.name.as_str()).collect();
            let hint = if close.is_empty() { String::new() } else { format!(" (did you mean {}?)", close.iter().map(|c| format!("\"{c}\"")).collect::<Vec<_>>().join(", ")) };
            format!("the parameter \"{name}\" is not one of the {}{hint}", description.name)
        })?;
        let value = value_from(description, param, given)?;
        devices::set(raw, param, &value)?;
    }
    Ok(())
}

/// A patch made from an action: from init or from `current`, named, set.
pub fn patch_from(description: &Description, action: &Action, current: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut raw = match (action.from.unwrap_or_default(), current) {
        (From::Init, _) | (From::Memory, None) => description.init_patch(),
        (From::Memory, Some(current)) => current.to_vec(),
    };
    if raw.len() != description.patch_size {
        return Err("the current patch is not whole".to_owned());
    }
    if let Some(name) = &action.name {
        devices::set(&mut raw, description.name_param(), &Value::Text(name.clone()))?;
    }
    if let Some(set) = &action.set {
        apply_set(description, &description.patch, &mut raw, set)?;
    }
    Ok(raw)
}

/// The entries of a program change map from an action: index → memory.
pub fn pc_map_from(description: &Description, map: &BTreeMap<String, String>) -> Result<Vec<(usize, u16)>, String> {
    let mut out = Vec::new();
    for (pc, memory) in map {
        let (bank, program) = match pc.split_once(':') {
            Some((bank, program)) => (bank.trim().parse::<usize>().map_err(|_| format!("\"{pc}\": a bank select from 0 to 2"))?, program.trim()),
            None => (0, pc.trim()),
        };
        let program: usize = program.parse().map_err(|_| format!("\"{pc}\": a program from 0 to 127"))?;
        let index = bank * 128 + program;
        if program > 127 || index >= description.pc_map_entries {
            return Err(format!("\"{pc}\": out of the program change map"));
        }
        let number = description.number(memory).ok_or_else(|| format!("\"{memory}\" is no memory of the {}", description.name))?;
        out.push((index, number as u16));
    }
    Ok(out)
}

/// A patch as values the pedal shows, by name: what `set` takes back.
pub fn export_values(description: &Description, params: &[Param], raw: &[u8]) -> BTreeMap<String, Json> {
    let mut out = BTreeMap::new();
    for param in params {
        if param.enc == Encoding::Ascii {
            continue;
        }
        let value = devices::get(raw, param);
        let shown = description.shown(param, &value);
        let json = match (description.labels_of(param), shown.parse::<i64>()) {
            (None, Ok(n)) => Json::from(n),
            _ => Json::String(shown),
        };
        out.insert(param.name.clone(), json);
    }
    out
}

/// A memory's patch as an action that writes it back from init.
pub fn export_patch(description: &Description, number: usize, raw: &[u8]) -> Action {
    Action {
        pedal: Some(description.key.clone()),
        memory: Some(description.label(number)),
        name: Some(description.patch_name(raw)),
        from: Some(From::Init),
        set: Some(export_values(description, &description.patch, raw)),
        ..Action::default()
    }
}

// ----- the G3S ------------------------------------------------------------------------

/// The kind of a MIDI line from its name, whatever the case.
fn midi_kind(name: &str) -> Result<u8, String> {
    let wanted = name.trim();
    MIDI_KINDS.iter().find(|(_, n)| n.eq_ignore_ascii_case(wanted)).map(|(k, _)| *k).ok_or_else(|| format!("\"{wanted}\" is no MIDI kind of the G3S (one of {})", MIDI_KINDS.iter().map(|(_, n)| format!("\"{n}\"")).collect::<Vec<_>>().join(", ")))
}

fn kind_is_control(kind: u8) -> bool {
    matches!(kind, 0xB0 | 0x00 | 0x70 | 0x20 | 0x50 | 0x90)
}

/// Applies a G3S action to a backup; says what was done.
pub fn apply_g3s(backup: &mut Backup, action: &Action) -> Result<String, String> {
    let bank_number = action.bank.ok_or("a bank, from 1")?;
    if bank_number == 0 || bank_number > g3s::MOST_BANKS {
        return Err(format!("the bank {bank_number} is not one of 1 to {}", g3s::MOST_BANKS));
    }
    let bank = bank_number - 1;
    match action.kind()? {
        Kind::G3sRemoveBank => {
            if bank >= backup.banks.len() {
                return Err(format!("there is no bank {bank_number}"));
            }
            backup.remove_bank(bank);
            Ok(format!("bank {bank_number} removed"))
        }
        kind => {
            let mut added = 0;
            while backup.banks.len() <= bank {
                backup.add_bank().ok_or("the switcher holds 99 banks at most")?;
                added += 1;
            }
            if let Some(song_mode) = action.song_mode {
                backup.set_song_mode(song_mode);
            }
            if kind == Kind::G3sBank {
                if let Some(name) = &action.name {
                    backup.set_bank_name(bank, name);
                }
                return Ok(format!("bank {bank_number} named \"{}\"{}", backup.bank_name(bank), if added > 0 { format!(", {added} banks added") } else { String::new() }));
            }
            let switch = action.switch.ok_or("a switch, from 1 to 28")?;
            if switch == 0 || switch > PRESETS {
                return Err(format!("the switch {switch} is not one of 1 to {PRESETS}"));
            }
            let preset = &mut backup.banks[bank].presets[switch - 1];
            if let Some(name) = &action.name {
                preset.set_name(name);
            }
            if let Some(loops) = &action.loops {
                if loops.iter().any(|n| *n == 0 || usize::from(*n) > LOOPS) {
                    return Err("the loops are numbered 1 to 12".to_owned());
                }
                for n in 0..LOOPS {
                    preset.set_loop(n, loops.contains(&(n as u8 + 1)));
                }
            }
            if let Some(order) = &action.order {
                let order: [u8; LOOPS] = order.clone().try_into().map_err(|_| "the order names the twelve loops".to_owned())?;
                preset.set_order(&order)?;
            }
            if let Some(lines) = &action.midi {
                if lines.len() > MIDI_LINES {
                    return Err(format!("at most {MIDI_LINES} MIDI lines"));
                }
                for line in 0..MIDI_LINES {
                    preset.clear_midi(line);
                }
                for (line, midi) in lines.iter().enumerate() {
                    let kind = midi_kind(&midi.kind)?;
                    if midi.channel == 0 || midi.channel > 16 {
                        return Err(format!("MIDI line {}: a channel from 1 to 16", line + 1));
                    }
                    let (number, value) = if kind_is_control(kind) {
                        (midi.controller.ok_or_else(|| format!("MIDI line {}: a controller for a CC", line + 1))?, midi.value.unwrap_or(0))
                    } else {
                        (midi.program.ok_or_else(|| format!("MIDI line {}: a program for a PC", line + 1))?, 0)
                    };
                    if number > 127 || value > 127 {
                        return Err(format!("MIDI line {}: numbers from 0 to 127", line + 1));
                    }
                    preset.set_midi(line, MidiLine { kind, channel: midi.channel - 1, number, value });
                }
            }
            Ok(format!("bank {bank_number}, switch {switch} \"{}\" written", preset.name()))
        }
    }
}

/// A backup as the actions that would make it again: the banks named, the
/// presets that differ from the factory's.
pub fn export_g3s(backup: &Backup) -> Vec<Action> {
    let mut actions = Vec::new();
    for (bank, presets) in backup.banks.iter().enumerate() {
        actions.push(Action { g3s: Some("bank".to_owned()), bank: Some(bank + 1), name: Some(backup.bank_name(bank)), song_mode: (bank == 0).then(|| backup.song_mode()), ..Action::default() });
        for (i, preset) in presets.presets.iter().enumerate() {
            let factory = g3s::factory_preset(i, bank);
            if preset.raw[..127] == factory.raw[..127] && preset.extended == factory.extended {
                continue;
            }
            let loops: Vec<u8> = (0..LOOPS).filter(|n| preset.loop_on(*n)).map(|n| n as u8 + 1).collect();
            let midi: Vec<Midi> = preset
                .midi_lines()
                .into_iter()
                .map(|(_, line)| {
                    let kind = MIDI_KINDS.iter().find(|(k, _)| *k == line.kind).map_or_else(|| format!("{:02X}", line.kind), |(_, n)| (*n).to_owned());
                    let control = kind_is_control(line.kind);
                    Midi { kind, channel: line.channel + 1, program: (!control).then_some(line.number), controller: control.then_some(line.number), value: control.then_some(line.value) }
                })
                .collect();
            actions.push(Action { g3s: Some("preset".to_owned()), bank: Some(bank + 1), switch: Some(i + 1), name: Some(preset.name()), loops: Some(loops), order: Some(preset.order().to_vec()), midi: Some(midi), ..Action::default() });
        }
    }
    actions
}

// ----- the guide ----------------------------------------------------------------------

/// The guide that teaches an AI the format: Markdown, in English, made from
/// the descriptions so that every parameter named exists.
pub fn guide(descriptions: &[Description], version: &str) -> String {
    let mut g = String::new();
    g.push_str(&format!("# OpenGarageMusic {version} — guide for an AI that programs MIDI pedals\n\n"));
    let keys: Vec<String> = descriptions.iter().map(|d| format!("`{}`", d.key)).collect();
    g.push_str(&format!("**Version.** This guide was written by OpenGarageMusic **{version}** and describes the script format **{FORMAT}** and the pedals that version knows ({}), plus the G3S. Use the guide that came with the user's version of the program (`ai-guide.md` in its archive, List → \"Guide for an AI…\" in the MIDI Pilot, `GET /guide` on its API): a newer program may know more parameters, an older one fewer. Scripts say the format with `\"opengaragemusic\": {FORMAT}`.\n\n", keys.join(", ")));
    g.push_str("OpenGarageMusic is a music studio for Windows (its source is published, free for personal use). Its **MIDI Pilot** window drives the user's MIDI pedals. You, the AI, do not talk to the pedals: you write a **script**, a JSON file the user applies in the MIDI Pilot (List → \"Apply an AI script…\"); the program shows what will be written, the user confirms, and the program writes the pedal and reads it back. The user may also give you an **export** in the same format (what a memory holds now) for you to change. When the user's program runs a local API (see the end), an agent on the same computer can call it with the same JSON.\n\n");
    g.push_str("## The script\n\n```json\n{ \"opengaragemusic\": 1,\n  \"actions\": [\n    { \"pedal\": \"rv500\", \"memory\": \"50C\", \"name\": \"COMF NUMB\", \"from\": \"init\",\n      \"set\": { \"Reverb : MODE\": \"HALL\", \"Reverb : time\": \"4.0\", \"Reverb : PRE-DELAY\": 40, \"Reverb : Effect Level\": 30 } },\n    { \"pedal\": \"dd500\", \"pc_map\": { \"1\": \"81A\", \"2:5\": \"03A\" } },\n    { \"pedal\": \"dd500\", \"system\": { \"MIDI Rx Channel\": \"Ch.1\" } },\n    { \"g3s\": \"bank\", \"bank\": 12, \"name\": \"COMFORT.|NUMB\", \"song_mode\": true },\n    { \"g3s\": \"preset\", \"bank\": 12, \"switch\": 2, \"name\": \"INTRO\", \"loops\": [9, 10, 12],\n      \"order\": [10, 7, 11, 12, 1, 2, 3, 4, 5, 6, 8, 9],\n      \"midi\": [ { \"kind\": \"PC down\", \"channel\": 1, \"program\": 4 },\n                { \"kind\": \"CC down\", \"channel\": 2, \"controller\": 80, \"value\": 127 } ] }\n  ] }\n```\n\n");
    g.push_str("Rules:\n\n- `actions` run in order. A pedal action names the pedal by its key (below) and does one thing: a `memory` with `set` (and optionally `name`, `from`), a `pc_map`, or a `system`.\n- `memory` is the memory written, \"01A\" to \"99C\" (99 banks of 3 slots A, B, C). `from` is `\"memory\"` (default: start from what the memory holds) or `\"init\"` (every parameter at its first value; use it when you set a whole sound).\n- `set` maps **exact parameter names** (listed below for each pedal) to values **as the pedal shows them**: a label of the parameter's list (case does not matter), or a **whole number in the range given** (\"0 to 120\" means exactly that: not a percentage). When a parameter says \"shown by the pedal as …\", that text gives the unit and the scale of the number: `Delay Time` 1 to 10000 shown as 1ms - 10.0s is in milliseconds (440 for 440 ms); `BPM` 15 to 2400000 shown as 1.5 - 240000.0 is in tenths (1200 for 120.0); a range like -50 to 50 is written as is (the pedal shows it so). Do not invent parameter names. Parameters you leave out keep their value.\n- `pc_map` maps a program change to a memory: the key is the program (0 to 127), or `\"bank:program\"` with the bank select (0 to 2). The pedal's own `MIDI Program Change IN` must be on for program changes to work.\n- `name` is the patch's name, at most 16 ASCII characters for the BOSS pedals, 9 for a G3S preset; a G3S bank name is two lines of ten with `|` between.\n- Always propose a plan in words first, then the script; keep scripts small (a few memories), and do not write memories the user did not name.\n\n");
    for d in descriptions {
        g.push_str(&format!("## {} (key `{}`)\n\n", d.name, d.key));
        g.push_str(&format!("{} memories, \"01A\" to \"{}\". The document the description comes from: {}\n\n", d.patches(), d.label(d.patches() - 1), d.document));
        g.push_str("### Patch parameters\n\nBy group; each with its shown range or its values, and its first value in brackets.\n\n");
        let mut groups: Vec<&str> = Vec::new();
        for p in &d.patch {
            if !groups.contains(&p.group.as_str()) {
                groups.push(&p.group);
            }
        }
        for group in groups {
            g.push_str(&format!("**{group}**\n\n"));
            for p in d.patch.iter().filter(|p| p.group == group) {
                g.push_str(&param_line(d, p));
            }
            g.push('\n');
        }
        if !d.system.is_empty() {
            g.push_str("### System settings (`system`)\n\n");
            for p in &d.system {
                g.push_str(&param_line(d, p));
            }
            g.push('\n');
        }
    }
    g.push_str("## The GigRig G3S pedal switcher (`g3s`)\n\n");
    g.push_str(&format!("The G3S is edited through a **backup file** the user loads in the MIDI Pilot and restores in the switcher with its editor. It has up to {} banks of {PRESETS} presets ({} on the footswitches, the rest hidden), {LOOPS} loops, and each preset sends up to {MIDI_LINES} MIDI lines. Actions:\n\n", g3s::MOST_BANKS, 14));
    g.push_str("- `{ \"g3s\": \"bank\", \"bank\": n, \"name\": \"LINE ONE|LINE TWO\", \"song_mode\": true }`: names bank n (from 1; banks are added up to it), two lines of ten characters; `song_mode` makes the switcher show names instead of numbers.\n");
    g.push_str("- `{ \"g3s\": \"preset\", \"bank\": n, \"switch\": s, \"name\": \"...\", \"loops\": [..], \"order\": [..], \"midi\": [..] }`: switch s (1 to 28), `name` at most 9 characters, `loops` the loops on (1 to 12), `order` the twelve loops in the order of the signal, `midi` replaces the preset's MIDI lines: each `kind` one of ");
    g.push_str(&MIDI_KINDS.iter().map(|(_, n)| format!("\"{n}\"")).collect::<Vec<_>>().join(", "));
    g.push_str(", `channel` 1 to 16, then `program` for a PC kind or `controller` and `value` for a CC kind. Lines you leave out are cleared.\n");
    g.push_str("- `{ \"g3s\": \"remove_bank\", \"bank\": n }`.\n");
    g.push_str(&format!("- `{{ \"g3s\": \"loops\", \"pedals\": {{ \"1\": [ {{ \"name\": \"Fuzz Face\", \"kind\": \"fuzz\" }}, {{ \"name\": \"Vox Wah\", \"kind\": \"wah\" }} ], \"11\": [ {{ \"name\": \"DD-500\", \"kind\": \"delay\", \"device\": \"DD-500\" }} ] }}, \"signal_order\": [1, 8, 10, 6, 2, 4, 3, 5, 7, 9, 11, 12], \"bank_switches\": [1, 8] }}`: says which pedals sit in which loops of the switcher (kinds: {}), which loop holds which MIDI device of the chain, the order the signal goes through the loops (what a new preset should use), and the footswitches that change banks. Kept in the program's settings, not in the switcher.\n\n", PEDAL_KINDS.join(", ")));
    g.push_str("Which footswitches change banks is a setting of the switcher itself, not of this file: ask the user. When 1 and 8 do (a common choice), presets go on switches 2 to 7 and 9 to 14; otherwise all of 1 to 14 are presets. Many users keep bank 1 as their stompbox bank and give one bank to each song. Each line of `midi` has its own channel: a preset may drive several devices.\n\n");
    g.push_str("## The pedalboard file\n\n");
    g.push_str("The user may give you their **pedalboard file** (MIDI Pilot → List → \"Export the pedalboard for an AI…\", or `GET /pedalboard`): a script with a `pedalboard` section and `actions`. `pedalboard.devices` are the devices of the chain with their MIDI channels; `pedalboard.switcher` says which pedals sit in each loop of the switcher (`loops`, by loop number, each pedal with its `name`, `kind` and the `device` of the chain when it is MIDI), the order the signal goes through the loops (`signal_order`) and the footswitches that change banks (`bank_switches`); `mode` and `numbering` say how songs are sent. The `actions` are everything known, in the format above: the songs, the switcher's loops and backup (banks and presets), and each memory of the pedals with all its values and their program change maps. Read it before writing: a preset turns loops on by their number, so the loops tell you which pedal each number is; a `device` in a loop is where to send program changes; use the memories that exist (and their numbers in the program change map) before writing new ones; keep the user's conventions (bank 1, the bank switches, the signal order).\n\n");
    g.push_str("## The songs of the MIDI Pilot (`song`)\n\n");
    g.push_str("The MIDI Pilot shows the songs of a concert as large buttons; choosing one sends MIDI to the user's devices. In the switcher's mode a song sends its `number` (a bank select or a program change to the switcher); in the chain's mode it sends its `messages`, each to a device of the user's chain by the device's name (ask the user for the names, or `GET /devices` on the API: for example \"DD-500\" on channel 1 and \"RV-500\" on channel 2). Actions:\n\n");
    g.push_str("- `{ \"song\": \"add\", \"artist\": \"Pink Floyd\", \"title\": \"Comfortably Numb\", \"display\": \"COMF NUMB\", \"number\": 12, \"messages\": [ { \"device\": \"DD-500\", \"program\": 4, \"delay_ms\": 20 }, { \"device\": \"RV-500\", \"bank\": 0, \"program\": 2 }, { \"device\": \"G3S\", \"controller\": 0, \"value\": 12 } ] }`: adds a song (`display` and `number` optional; the number follows the last song's when left out). A message is a `program` (with `bank` for a bank select before it), or a `controller` with a `value`, or a `sysex` in hexadecimal; `delay_ms` is a pause after it (up to 500).\n");
    g.push_str("- `{ \"song\": \"set\", \"artist\": \"…\", \"title\": \"…\", \"number\": 13, \"display\": \"…\", \"messages\": [ … ] }`: changes the song of that artist and title (compared up to case and signs); only the fields given change, `messages` replaces them all.\n");
    g.push_str("- `{ \"song\": \"remove\", \"artist\": \"…\", \"title\": \"…\" }`.\n\n");
    g.push_str("**Bank numbering.** What a switcher needs to reach a bank is its own business and the user's settings: never assume an offset. In the switcher's mode the program sends the song's `number` as a bank select (controller 0) or a program change, minus one when the user ticked \"Numbers from 1\" (`GET /songs` says so under `switcher`); in the chain's mode the song's `messages` go out as written. Copy the convention of the user's existing songs (their export, or `GET /songs`): if their bank 12 is reached by `number` 12 or by controller 0 value 12, do the same for new songs; when there are no songs yet, ask the user, and have them try one song with \"Send now\" before writing many. A CSV file imported in the MIDI Pilot follows the same rule: its number column is the song's `number`.\n\n");
    g.push_str("The user's list exported for you (List → \"Export the songs for an AI…\", or `GET /songs`) is made of `add` actions you can change and send back as `set` actions.\n\n");
    g.push_str("## Local API (for an agent on the user's computer)\n\n");
    g.push_str("When the user turns it on (MIDI Pilot → Settings → \"API for an AI…\"), the program listens on `http://127.0.0.1:<port>/` with a token the user gives you: send `Authorization: Bearer <token>`. Bodies and answers use the format above.\n\n");
    g.push_str("- `GET /` what runs; `GET /guide` this text; `GET /devices` the devices of the chain; `GET /pedals/{key}` the parameters.\n- `GET /pedals/{key}/memories` the names; `GET /pedals/{key}/memories/{50C}` a memory as an action; `PUT /pedals/{key}/memories/{50C}` with `{ \"name\", \"from\", \"set\" }` writes it (the user confirms in the window unless they allowed unattended writes); `POST /pedals/{key}/try` with `{ \"set\" }` plays the values without writing.\n- `GET|PUT /pedals/{key}/pc_map`, `GET|PUT /pedals/{key}/system`.\n- `GET /g3s` the backup loaded as actions; `POST /g3s/actions` with `{ \"actions\": [..] }`; `POST /g3s/save` writes the backup file.\n- `GET /songs` the songs of the list as actions; `GET /pedalboard` the whole pedalboard file.\n- `POST /actions` with a whole script (songs included).\n\nAnswers: 200 done, 202 waiting for the user's confirmation, 400 a wrong script (the reason in `error`), 401 token, 404, 409 the module is busy, 503 the MIDI ports are not open.\n");
    g
}

/// One parameter of the guide.
fn param_line(d: &Description, p: &Param) -> String {
    let init = match &p.init {
        devices::Init::Number(n) => d.shown(p, &Value::Number(*n)),
        devices::Init::Text(t) => t.trim().to_owned(),
    };
    let values = match d.labels_of(p) {
        Some(labels) if labels.len() <= 24 => labels.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join(", "),
        Some(labels) => format!("`{}` … `{}` ({} values)", labels[0], labels[labels.len() - 1], labels.len()),
        None if p.enc == Encoding::Ascii => format!("text, {} characters", p.bytes),
        None => {
            let range = format!("{} to {}", p.min - p.offset, p.max - p.offset);
            match &p.shown {
                Some(shown) if shown.replace(" - ", " to ") != range => format!("{range}, shown by the pedal as {shown}"),
                _ => range,
            }
        }
    };
    format!("- `{}`: {values} [{init}]\n", p.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rv500() -> Description {
        Description::parse(devices::BUILT_IN[1].1).unwrap()
    }

    #[test]
    fn a_script_is_read_and_its_actions_told_apart() {
        let text = r#"{ "opengaragemusic": 1, "actions": [
            { "pedal": "rv500", "memory": "50C", "name": "COMF NUMB", "from": "init", "set": { "Reverb : MODE": "HALL" } },
            { "pedal": "dd500", "pc_map": { "1": "81A" } },
            { "pedal": "dd500", "system": { "MIDI Rx Channel": "Ch.1" } },
            { "g3s": "bank", "bank": 12, "name": "COMFORT.|NUMB" },
            { "g3s": "preset", "bank": 12, "switch": 2, "name": "INTRO", "loops": [9, 10, 12], "midi": [ { "kind": "PC down", "channel": 1, "program": 4 } ] },
            { "g3s": "remove_bank", "bank": 13 } ] }"#;
        let script = Script::parse(text).unwrap();
        let kinds: Vec<Kind> = script.actions.iter().map(|a| a.kind().unwrap()).collect();
        assert_eq!(kinds, [Kind::Patch, Kind::PcMap, Kind::System, Kind::G3sBank, Kind::G3sPreset, Kind::G3sRemoveBank]);
        assert!(script.actions[0].summary(Some(&rv500())).contains("50C"));
        assert_eq!(Script::parse(&script.to_json()).unwrap(), script);
        assert!(Script::parse(r#"{ "actions": [ { "pedal": "rv500" } ] }"#).unwrap_err().contains("action 1"));
        assert!(Script::parse(r#"{ "actions": [ { "pedal": "rv500", "g3s": "bank" } ] }"#).unwrap_err().contains("not both"));
        assert!(Script::parse(r#"{ "actions": [ { "g3s": "song" } ] }"#).unwrap_err().contains("\"song\""));
        assert!(Script::parse(r#"{ "opengaragemusic": 9 }"#).unwrap_err().contains("newer"));
        assert!(Script::parse("[]").is_err());
    }

    #[test]
    fn values_are_taken_as_the_pedal_shows_them() {
        let d = rv500();
        let mode = d.param("Reverb : MODE").unwrap();
        let time = d.param("Reverb : time").unwrap();
        let tone = d.param("Reverb : Tone Low").unwrap();
        assert_eq!(value_from(&d, mode, &Json::from("hall")), Ok(Value::Number(1)), "a label, whatever its case");
        assert_eq!(value_from(&d, time, &Json::from("4.0")), Ok(Value::Number(39)), "a label that looks like a number");
        assert_eq!(value_from(&d, tone, &Json::from(-20)), Ok(Value::Number(30)), "the offset taken: shown -20 is raw 30");
        assert_eq!(value_from(&d, tone, &Json::from("+10")), Ok(Value::Number(60)));
        assert!(value_from(&d, tone, &Json::from(80)).unwrap_err().contains("-50 to 50"));
        assert!(value_from(&d, mode, &Json::from("CATHEDRAL")).unwrap_err().contains("\"ROOM\""));
        assert!(value_from(&d, d.name_param(), &Json::from(true)).is_err());
        let mut raw = d.init_patch();
        let mut set = BTreeMap::new();
        set.insert("Reverb : MODE".to_owned(), Json::from("Plate"));
        set.insert("reverb : effect level".to_owned(), Json::from(30));
        apply_set(&d, &d.patch, &mut raw, &set).unwrap();
        assert_eq!((devices::get(&raw, mode), devices::get(&raw, d.param("Reverb : Effect Level").unwrap())), (Value::Number(2), Value::Number(30)));
        set.insert("Reverb : tim".to_owned(), Json::from(1));
        assert!(apply_set(&d, &d.patch, &mut raw, &set).unwrap_err().contains("\"Reverb : time\""), "a hint");
    }

    #[test]
    fn a_patch_comes_from_an_action_and_goes_back_as_one() {
        let d = rv500();
        let mut set = BTreeMap::new();
        set.insert("Reverb : MODE".to_owned(), Json::from("HALL"));
        set.insert("Reverb : PRE-DELAY".to_owned(), Json::from(40));
        let action = Action { pedal: Some("rv500".into()), memory: Some("50C".into()), name: Some("COMF NUMB".into()), from: Some(From::Init), set: Some(set), ..Action::default() };
        let raw = patch_from(&d, &action, None).unwrap();
        assert_eq!(d.patch_name(&raw), "COMF NUMB");
        assert_eq!(devices::get(&raw, d.param("Reverb : PRE-DELAY").unwrap()), Value::Number(40));
        // From the memory: the other values stay.
        let again = Action { from: None, set: Some(BTreeMap::from([("Reverb : Effect Level".to_owned(), Json::from(60))])), name: None, ..action.clone() };
        let changed = patch_from(&d, &again, Some(&raw)).unwrap();
        assert_eq!((d.patch_name(&changed), devices::get(&changed, d.param("Reverb : MODE").unwrap())), ("COMF NUMB".to_owned(), Value::Number(1)));
        // Exported, set again, the same bytes.
        let export = export_patch(&d, 149, &changed);
        assert_eq!((export.memory.as_deref(), export.name.as_deref(), export.from), (Some("50C"), Some("COMF NUMB"), Some(From::Init)));
        assert_eq!(export.set.as_ref().unwrap()["Reverb : MODE"], Json::from("HALL"));
        assert_eq!(export.set.as_ref().unwrap()["Reverb : Tone Low"], Json::from(0), "shown with its offset");
        assert_eq!(patch_from(&d, &export, None).unwrap(), changed);
        let map = pc_map_from(&d, &BTreeMap::from([("1".to_owned(), "81A".to_owned()), ("2:5".to_owned(), "03a".to_owned())])).unwrap();
        assert_eq!(map, [(1, 240), (261, 6)]);
        assert!(pc_map_from(&d, &BTreeMap::from([("3:1".to_owned(), "01A".to_owned())])).is_err());
        assert!(pc_map_from(&d, &BTreeMap::from([("1".to_owned(), "99D".to_owned())])).unwrap_err().contains("99D"));
    }

    #[test]
    fn g3s_actions_make_banks_and_presets_and_come_back_as_actions() {
        let mut backup = g3s::blank(1);
        let bank = Action { g3s: Some("bank".into()), bank: Some(3), name: Some("COMFORT.|NUMB".into()), song_mode: Some(true), ..Action::default() };
        assert!(apply_g3s(&mut backup, &bank).unwrap().contains("2 banks added"));
        assert_eq!((backup.banks.len(), backup.bank_name(2).as_str(), backup.song_mode()), (3, "COMFORT.|NUMB", true));
        let preset = Action {
            g3s: Some("preset".into()),
            bank: Some(3),
            switch: Some(2),
            name: Some("INTRO".into()),
            loops: Some(vec![9, 10, 12]),
            order: Some(vec![10, 7, 11, 12, 1, 2, 3, 4, 5, 6, 8, 9]),
            midi: Some(vec![Midi { kind: "PC down".into(), channel: 1, program: Some(4), ..Midi::default() }, Midi { kind: "cc down".into(), channel: 2, controller: Some(80), value: Some(127), ..Midi::default() }]),
            ..Action::default()
        };
        apply_g3s(&mut backup, &preset).unwrap();
        let p = &backup.banks[2].presets[1];
        assert_eq!((p.name(), p.loop_on(8), p.loop_on(9), p.loop_on(0)), ("INTRO".to_owned(), true, true, false));
        assert_eq!(p.midi_lines(), [(0, MidiLine { kind: 0xC0, channel: 0, number: 4, value: 0 }), (1, MidiLine { kind: 0xB0, channel: 1, number: 80, value: 127 })]);
        let actions = export_g3s(&backup);
        assert_eq!(actions.len(), 4, "three banks and one preset that differs from the factory's");
        assert_eq!((actions[3].switch, actions[3].loops.as_deref(), actions[3].midi.as_ref().unwrap()[1].controller), (Some(2), Some(&[9u8, 10, 12][..]), Some(80)));
        let mut again = g3s::blank(1);
        for action in &actions {
            apply_g3s(&mut again, action).unwrap();
        }
        assert_eq!(again.write(), backup.write(), "the export makes the backup again");
        // Refusals.
        assert!(apply_g3s(&mut backup, &Action { g3s: Some("preset".into()), bank: Some(3), switch: Some(29), ..Action::default() }).is_err());
        assert!(apply_g3s(&mut backup, &Action { g3s: Some("preset".into()), bank: Some(3), switch: Some(1), midi: Some(vec![Midi { kind: "PC sideways".into(), channel: 1, program: Some(1), ..Midi::default() }]), ..Action::default() }).unwrap_err().contains("PC down"));
        assert!(apply_g3s(&mut backup, &Action { g3s: Some("bank".into()), bank: Some(0), ..Action::default() }).is_err());
        apply_g3s(&mut backup, &Action { g3s: Some("remove_bank".into()), bank: Some(3), ..Action::default() }).unwrap();
        assert_eq!(backup.banks.len(), 2);
    }

    #[test]
    fn song_actions_add_change_and_remove_the_songs_of_the_list_and_come_back_as_actions() {
        use crate::setlist::{Device, PilotMode};
        let mut list = SetList { mode: PilotMode::Chain, devices: vec![Device { name: "DD-500".into(), model: "dd500".into(), channel: 0, ..Device::default() }, Device { name: "RV-500".into(), model: String::new(), channel: 1, ..Device::default() }], ..SetList::default() };
        let text = r#"{ "actions": [
            { "song": "add", "artist": "Pink Floyd", "title": "Comfortably Numb", "display": "COMF NUMB", "messages": [ { "device": "dd500", "program": 4, "delay_ms": 20 }, { "device": "RV-500", "bank": 0, "program": 2 }, { "device": "RV-500", "controller": 80, "value": 127 }, { "device": "DD-500", "sysex": "F0 7E 7F 06 01 F7" } ] },
            { "song": "add", "artist": "Pink Floyd", "title": "Time", "number": 7 },
            { "song": "set", "artist": "pink floyd", "title": "comfortably numb", "number": 12, "messages": [ { "device": "DD-500", "program": 1 } ] },
            { "song": "remove", "artist": "Pink Floyd", "title": "Time" } ] }"#;
        let script = Script::parse(text).unwrap();
        assert_eq!(script.actions.iter().map(|a| a.kind().unwrap()).collect::<Vec<_>>(), [Kind::SongAdd, Kind::SongAdd, Kind::SongSet, Kind::SongRemove]);
        assert!(apply_song(&mut list, &script.actions[0]).unwrap().contains("song 1"));
        assert_eq!(list.songs[0].steps.len(), 4);
        assert_eq!(list.songs[0].steps[0].device, "DD-500", "the device's name as the chain writes it");
        assert_eq!(list.songs[0].steps[2].message, StepMessage::Control { number: 80, value: 127 });
        assert_eq!(list.songs[0].steps[3].message, StepMessage::SysEx(vec![0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]));
        apply_song(&mut list, &script.actions[1]).unwrap();
        assert_eq!((list.songs.len(), list.songs[1].number), (2, 7));
        apply_song(&mut list, &script.actions[2]).unwrap();
        assert_eq!((list.songs[0].number, list.songs[0].display.as_str(), list.songs[0].steps.len()), (12, "COMF NUMB", 1));
        apply_song(&mut list, &script.actions[3]).unwrap();
        assert_eq!(list.songs.len(), 1);
        // Refusals, each with its reason.
        assert!(apply_song(&mut list, &script.actions[0]).unwrap_err().contains("already"));
        assert!(apply_song(&mut list, &script.actions[3]).unwrap_err().contains("not in the list"));
        let mut wrong = script.actions[1].clone();
        wrong.messages = Some(vec![Message { device: "Looper".into(), program: Some(1), ..Message::default() }]);
        assert!(apply_song(&mut list, &wrong).unwrap_err().contains("\"Looper\""));
        wrong.messages = Some(vec![Message { device: "DD-500".into(), ..Message::default() }]);
        assert!(apply_song(&mut list, &wrong).unwrap_err().contains("one of them"));
        wrong.messages = None;
        wrong.number = Some(200);
        assert!(apply_song(&mut list, &wrong).unwrap_err().contains("0 to 128"));
        assert!(Script::parse(r#"{ "actions": [ { "song": "list" } ] }"#).unwrap_err().contains("\"list\""));
        assert!(Script::parse(r#"{ "actions": [ { "song": "add", "pedal": "rv500" } ] }"#).unwrap_err().contains("not several"));
        // Exported, the list is made again.
        let actions = export_songs(&list);
        assert_eq!((actions.len(), actions[0].song.as_deref(), actions[0].number), (1, Some("add"), Some(12)));
        let mut again = SetList { devices: list.devices.clone(), ..SetList::default() };
        for action in &actions {
            apply_song(&mut again, action).unwrap();
        }
        assert_eq!(again.songs, list.songs);
        assert!(Script { version: FORMAT, pedalboard: None, actions }.to_json().contains("\"song\": \"add\""));
    }

    #[test]
    fn the_loops_of_the_switcher_are_set_exported_and_the_pedalboard_file_made() {
        use crate::setlist::{Device, PilotMode};
        let mut list = SetList { mode: PilotMode::Chain, devices: vec![Device { name: "G3S".into(), model: "g3s".into(), channel: 4, ..Device::default() }, Device { name: "DD-500".into(), model: "dd500".into(), channel: 0, ..Device::default() }], ..SetList::default() };
        let text = r#"{ "actions": [ { "g3s": "loops", "pedals": { "1": [ { "name": "Fuzz Face", "kind": "fuzz" }, { "name": "Vox Wah", "kind": "wah" } ], "11": [ { "name": "DD-500", "kind": "delay", "device": "dd500" } ] }, "signal_order": [1, 8, 10, 6, 2, 4, 3, 5, 7, 9, 11, 12], "bank_switches": [1, 8] } ] }"#;
        let script = Script::parse(text).unwrap();
        assert_eq!(script.actions[0].kind(), Ok(Kind::G3sLoops));
        assert!(apply_loops(&mut list, &script.actions[0]).unwrap().contains("2 loops"));
        let switcher = &list.devices[0];
        assert_eq!((switcher.loop_label(0).as_str(), switcher.loop_name(1).as_str(), switcher.loop_name(10).as_str()), ("Fuzz Face + Vox Wah", "2", "11 DD-500"));
        assert_eq!((switcher.signal_order.len(), switcher.bank_switches.as_slice()), (12, &[1u8, 8][..]));
        // Refusals.
        let mut wrong = script.actions[0].clone();
        wrong.pedals = Some(BTreeMap::from([("13".to_owned(), vec![])]));
        assert!(apply_loops(&mut list, &wrong).unwrap_err().contains("no loop"));
        wrong.pedals = Some(BTreeMap::from([("2".to_owned(), vec![Pedal { name: "X".into(), kind: "laser".into(), device: String::new() }])]));
        assert!(apply_loops(&mut list, &wrong).unwrap_err().contains("laser"));
        wrong.pedals = Some(BTreeMap::from([("2".to_owned(), vec![Pedal { name: "X".into(), kind: String::new(), device: "Looper".into() }])]));
        assert!(apply_loops(&mut list, &wrong).unwrap_err().contains("Looper"));
        wrong.pedals = None;
        wrong.signal_order = Some(vec![1, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        assert!(apply_loops(&mut list, &wrong).is_err());
        let mut no_switcher = SetList::default();
        assert!(apply_loops(&mut no_switcher, &script.actions[0]).unwrap_err().contains("g3s"));
        // The pedalboard file: the chain, the loops, and what is known, as actions.
        let d = Description::parse(devices::BUILT_IN[0].1).unwrap();
        let mut file = devices::PedalFile::new(&d.key);
        let mut raw = d.init_patch();
        devices::set(&mut raw, d.name_param(), &Value::Text("GILMOUR 440".into())).unwrap();
        file.put(&d, 240, &raw);
        file.pc_map = (0..384u16).collect();
        file.pc_map[1] = 240;
        let mut backup = g3s::blank(2);
        backup.set_bank_name(1, "TIME");
        let board = pedalboard(&list, Some(&backup), &[(d.clone(), file)]);
        let section = board.pedalboard.as_ref().unwrap();
        assert_eq!((section.devices.len(), section.mode.as_str(), section.numbering.channel), (2, "chain", 1));
        let switcher = section.switcher.as_ref().unwrap();
        assert_eq!((switcher.device.as_str(), switcher.loops["1"].len(), switcher.loops["11"][0].device.as_str()), ("G3S", 2, "dd500"));
        let kinds: Vec<Kind> = board.actions.iter().map(|a| a.kind().unwrap()).collect();
        assert_eq!(kinds, [Kind::G3sLoops, Kind::G3sBank, Kind::G3sBank, Kind::Patch, Kind::PcMap]);
        assert_eq!(board.actions[3].memory.as_deref(), Some("81A"));
        assert_eq!(board.actions[4].pc_map.as_ref().unwrap()["1"], "81A");
        let text = board.to_json();
        assert!(text.contains("\"pedalboard\"") && text.contains("\"signal_order\""));
        let again = Script::parse(&text).unwrap();
        assert_eq!(again.actions.len(), 5);
        assert_eq!(again.pedalboard, board.pedalboard);
    }

    #[test]
    fn the_guide_names_every_pedal_and_every_parameter() {
        let descriptions = Description::built_in();
        let text = guide(&descriptions, "0.9.19");
        for d in &descriptions {
            assert!(text.contains(&format!("key `{}`", d.key)));
            for p in d.patch.iter().chain(&d.system) {
                assert!(text.contains(&format!("`{}`", p.name)), "{} is in the guide", p.name);
            }
        }
        assert!(text.contains("\"PC down\"") && text.contains("Authorization: Bearer") && text.contains("`HALL`"));
        assert!(text.contains("OpenGarageMusic **0.9.19**") && text.contains("script format **1**"), "the guide says which version wrote it");
    }
}
