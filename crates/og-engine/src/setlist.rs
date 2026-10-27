//! The set list: the songs of a concert or a rehearsal, in their order, each
//! with the number of its preset on a pedal switcher or an effects unit (sent
//! to it by MIDI when the song is chosen), its project, and its picture.
//!
//! A list is read from a CSV file of four columns: number, artist, song, and
//! the text of its button (optional). The separator is a comma or a semicolon
//! (a spreadsheet set to French writes semicolons); a first line whose number
//! is not a number is a header. In the chain mode, a further column named
//! after a device of the chain holds the program that device receives for the
//! song.
//!
//! Two modes: the switcher's, where each song sends one number to a pedal
//! switcher that sets the other pedals itself; and the chain's, where each
//! song sends its own messages to the devices of the chain, each on its channel.

use serde::{Deserialize, Serialize};

/// One song of the list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SetSong {
    /// The number of its preset (its bank, on a pedal switcher).
    pub number: u8,
    pub artist: String,
    pub title: String,
    /// The text of its button; empty for "Artist - Title".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub display: String,
    /// A picture of the user's for its button, over the one found.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub image: String,
    /// Its project file.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub project: String,
    /// In the chain mode: the messages it sends, in order, each to a device.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub steps: Vec<Step>,
}

impl SetSong {
    /// What its button says.
    pub fn label(&self) -> String {
        if !self.display.trim().is_empty() {
            self.display.clone()
        } else if self.artist.trim().is_empty() {
            self.title.clone()
        } else if self.title.trim().is_empty() {
            self.artist.clone()
        } else {
            format!("{} - {}", self.artist, self.title)
        }
    }

    /// What its button says among the songs of its artist, who is named already.
    pub fn short_label(&self) -> String {
        if !self.display.trim().is_empty() {
            self.display.clone()
        } else if !self.title.trim().is_empty() {
            self.title.clone()
        } else {
            self.artist.clone()
        }
    }
}

/// The order the songs are shown in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetOrder {
    /// As in the list.
    #[default]
    List,
    Alphabetical,
    /// The artists first, then the songs of the one chosen.
    ByArtist,
}

impl SetOrder {
    pub const ALL: [SetOrder; 3] = [SetOrder::List, SetOrder::Alphabetical, SetOrder::ByArtist];
}

/// What is sent when a song is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetMessage {
    /// Control change 0 (bank select), its value the number of the song.
    #[default]
    BankSelect,
    /// Program change, its program the number of the song.
    ProgramChange,
}

impl SetMessage {
    pub const ALL: [SetMessage; 2] = [SetMessage::BankSelect, SetMessage::ProgramChange];

    /// The bytes sent for a song, on a channel from 0 to 15.
    pub fn bytes(self, channel: u8, value: u8) -> Vec<u8> {
        match self {
            SetMessage::BankSelect => vec![0xB0 | (channel & 0x0F), 0, value & 0x7F],
            SetMessage::ProgramChange => vec![0xC0 | (channel & 0x0F), value & 0x7F],
        }
    }
}

/// Who sends what when a song is chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PilotMode {
    /// One number a song, to a pedal switcher that sets the other pedals itself.
    #[default]
    Switcher,
    /// Each song sends its own messages to the devices of the chain.
    Chain,
}

impl PilotMode {
    pub const ALL: [PilotMode; 2] = [PilotMode::Switcher, PilotMode::Chain];
}

/// A device of the chain: a pedal switcher, an effects pedal.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Device {
    pub name: String,
    /// Its model, when the program knows it (see `MODELS`); empty for any device.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub model: String,
    /// Its MIDI channel, from 0 to 15 (shown 1 to 16).
    pub channel: u8,
    /// For a switcher: the pedals in each of its loops, from loop 1.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub loops: Vec<Vec<Pedal>>,
    /// For a switcher: the order the signal goes through the loops, from 1,
    /// as the user wires it; what a new preset starts from.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub signal_order: Vec<u8>,
    /// For a switcher: the footswitches that change banks, from 1, where no preset goes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub bank_switches: Vec<u8>,
}

impl Device {
    /// The pedals of a loop (from 0), as one text: "Fuzz Face + Vox Wah".
    pub fn loop_label(&self, n: usize) -> String {
        self.loops.get(n).map_or(String::new(), |pedals| pedals.iter().map(|p| p.name.trim()).filter(|n| !n.is_empty()).collect::<Vec<_>>().join(" + "))
    }

    /// A loop's number and its pedals: "2 SD-1", or "2" when nothing is said of it.
    pub fn loop_name(&self, n: usize) -> String {
        let label = self.loop_label(n);
        if label.is_empty() { (n + 1).to_string() } else { format!("{} {label}", n + 1) }
    }
}

/// A pedal in a loop of a switcher: what it is, and the device of the chain
/// that drives it by MIDI when it has one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pedal {
    pub name: String,
    /// Its kind, one of `PEDAL_KINDS`; empty when not said.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub kind: String,
    /// The name of the device of the chain it is, when it is MIDI.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub device: String,
}

/// The kinds of pedals a loop may hold, for an AI to know what each does.
pub const PEDAL_KINDS: [&str; 16] = ["fuzz", "wah", "booster", "overdrive", "distortion", "compressor", "equalizer", "chorus", "phaser", "flanger", "tremolo", "octaver", "delay", "reverb", "volume", "other"];

/// The models the program knows, by their key: any device, The GigRig G3S,
/// BOSS DD-500, BOSS RV-500.
pub const MODELS: [&str; 4] = ["", "g3s", "dd500", "rv500"];

/// One message a song sends to a device, in the chain mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepMessage {
    /// A program change; after a bank select (control changes 0 and 32)
    /// when a bank is given.
    Program {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        bank: Option<u8>,
        program: u8,
    },
    /// A control change.
    Control { number: u8, value: u8 },
    /// A system exclusive message: its bytes, with or without F0 and F7.
    SysEx(Vec<u8>),
}

impl Default for StepMessage {
    fn default() -> Self {
        StepMessage::Program { bank: None, program: 0 }
    }
}

impl StepMessage {
    /// The MIDI messages, one after the other, on a channel from 0 to 15.
    pub fn messages(&self, channel: u8) -> Vec<Vec<u8>> {
        let channel = channel & 0x0F;
        match self {
            StepMessage::Program { bank, program } => {
                let mut out = Vec::new();
                if let Some(bank) = bank {
                    out.push(vec![0xB0 | channel, 0, bank & 0x7F]);
                    out.push(vec![0xB0 | channel, 32, 0]);
                }
                out.push(vec![0xC0 | channel, program & 0x7F]);
                out
            }
            StepMessage::Control { number, value } => vec![vec![0xB0 | channel, number & 0x7F, value & 0x7F]],
            StepMessage::SysEx(bytes) => {
                let inner = bytes.strip_prefix(&[0xF0]).unwrap_or(bytes);
                let inner = inner.strip_suffix(&[0xF7]).unwrap_or(inner);
                let mut out = Vec::with_capacity(inner.len() + 2);
                out.push(0xF0);
                out.extend(inner.iter().map(|b| b & 0x7F));
                out.push(0xF7);
                vec![out]
            }
        }
    }
}

/// One step of a song in the chain mode: a message to a device of the list.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Step {
    /// The name of the device it goes to.
    pub device: String,
    pub message: StepMessage,
    /// A pause after it, for the device to take it in, in milliseconds.
    pub delay_ms: u16,
}

/// Why the messages of a song cannot be made.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SendError {
    /// The switcher's number cannot be sent (0 to 127).
    Number(u8),
    /// A step goes to a device that is not in the list.
    Device(String),
}

/// The set list and how it is played, kept in the user's settings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SetList {
    pub songs: Vec<SetSong>,
    pub order: SetOrder,
    /// The MIDI output the messages go to; empty for none.
    pub midi_output: String,
    /// The MIDI input the pedal modules read the pedals' answers from; empty for none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub midi_input: String,
    /// The MIDI channel, from 0 to 15 (shown 1 to 16).
    pub channel: u8,
    pub message: SetMessage,
    /// The first song of the list is 1: song 1 sends 0 (the switcher's mode).
    pub numbers_from_one: bool,
    /// Who sends what when a song is chosen.
    pub mode: PilotMode,
    /// The devices of the chain, for the chain's mode.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub devices: Vec<Device>,
    /// No picture is looked for on Deezer.
    pub no_covers: bool,
    /// Choosing a song opens its project.
    pub open_project: bool,
    /// The site searched for the tablature of a song without a project.
    pub site: String,
    /// Where the projects made for the list are saved; empty for the folder
    /// OpenGarageMusic\Set List of the user's own folder.
    pub folder: String,
    /// The port of the local API for an AI's agent; 0 for none.
    pub api_port: u16,
    /// The token the agent gives; made when the API first runs.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub api_token: String,
    /// The API writes the pedals without asking the user each time.
    pub api_unattended: bool,
}

impl SetList {
    /// The value sent for a song, or why there is none.
    pub fn value_of(&self, number: u8) -> Option<u8> {
        let value = if self.numbers_from_one { number.checked_sub(1)? } else { number };
        (value <= 127).then_some(value)
    }

    /// The device of a name, whatever its case.
    pub fn device(&self, name: &str) -> Option<&Device> {
        self.devices.iter().find(|d| same(&d.name, name))
    }

    /// The MIDI messages a song sends, in order, each with the pause that
    /// follows it in milliseconds: in the switcher's mode its number, in the
    /// chain's its steps on the channel of their device.
    pub fn messages_for(&self, song: &SetSong) -> Result<Vec<(Vec<u8>, u16)>, SendError> {
        match self.mode {
            PilotMode::Switcher => {
                let value = self.value_of(song.number).ok_or(SendError::Number(song.number))?;
                Ok(vec![(self.message.bytes(self.channel, value), 0)])
            }
            PilotMode::Chain => {
                let mut out = Vec::new();
                for step in &song.steps {
                    let device = self.device(&step.device).ok_or_else(|| SendError::Device(step.device.clone()))?;
                    let messages = step.message.messages(device.channel);
                    let last = messages.len().saturating_sub(1);
                    for (i, message) in messages.into_iter().enumerate() {
                        out.push((message, if i == last { step.delay_ms } else { 0 }));
                    }
                }
                Ok(out)
            }
        }
    }
}

/// Folds a name for comparing: lower case, letters and digits only, so that
/// "Guns N' Roses" and "guns n roses" are the same artist.
pub fn folded(text: &str) -> String {
    text.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

/// Whether two names are of the same artist, or the same song.
pub fn same(a: &str, b: &str) -> bool {
    folded(a) == folded(b)
}

/// The places of the songs in the order they are shown.
pub fn order(songs: &[SetSong], order: SetOrder) -> Vec<usize> {
    let mut places: Vec<usize> = (0..songs.len()).collect();
    match order {
        SetOrder::List => {}
        SetOrder::Alphabetical => places.sort_by_cached_key(|&i| songs[i].label().to_lowercase()),
        SetOrder::ByArtist => places.sort_by_cached_key(|&i| (songs[i].artist.to_lowercase(), songs[i].short_label().to_lowercase())),
    }
    places
}

/// The artists, in alphabetical order, with how many songs each has; the same
/// artist written two ways counts once, under the first way met.
pub fn artists(songs: &[SetSong]) -> Vec<(String, usize)> {
    let mut list: Vec<(String, usize)> = Vec::new();
    for song in songs {
        match list.iter_mut().find(|(artist, _)| same(artist, &song.artist)) {
            Some((_, count)) => *count += 1,
            None => list.push((song.artist.clone(), 1)),
        }
    }
    list.sort_by_cached_key(|(artist, _)| artist.to_lowercase());
    list
}

/// The fields of the lines of a CSV text, quotes undone.
fn fields(text: &str, separator: char) -> Vec<(usize, Vec<String>)> {
    let mut lines = Vec::new();
    let (mut line, mut field, mut quoted, mut number) = (Vec::new(), String::new(), false, 1);
    let mut start = 1;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' if quoted && chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            '"' if quoted => quoted = false,
            '"' if field.trim().is_empty() => {
                field.clear();
                quoted = true;
            }
            c if c == separator && !quoted => line.push(std::mem::take(&mut field)),
            '\r' if !quoted => {}
            '\n' if !quoted => {
                line.push(std::mem::take(&mut field));
                lines.push((start, std::mem::take(&mut line)));
                number += 1;
                start = number;
            }
            '\n' => {
                number += 1;
                field.push(c);
            }
            c => field.push(c),
        }
    }
    if !field.is_empty() || !line.is_empty() {
        line.push(field);
        lines.push((start, line));
    }
    for (_, line) in &mut lines {
        for field in line.iter_mut() {
            *field = field.trim().to_owned();
        }
    }
    lines
}

/// What is wrong with a line of a CSV file.
#[derive(Clone, Debug, PartialEq)]
pub enum CsvError {
    /// The number of the song is not a number from 0 to 128.
    Number { line: usize, text: String },
    /// The line names no artist, no song and no text.
    Empty { line: usize },
    /// The program of a device is not a number from 0 to 127.
    Program { line: usize, device: String, text: String },
}

/// Reads a set list from a CSV text: number, artist, song, the text of its
/// button, and, in the columns the header names after those, the program each
/// device of that name receives (a step of the chain's mode; empty for none).
pub fn parse_csv(text: &str) -> Result<Vec<SetSong>, CsvError> {
    let text = text.trim_start_matches('\u{feff}');
    let first = text.lines().next().unwrap_or_default();
    let separator = if first.matches(';').count() > first.matches(',').count() { ';' } else { ',' };
    let mut songs = Vec::new();
    let mut header: Vec<String> = Vec::new();
    for (place, (line, row)) in fields(text, separator).into_iter().enumerate() {
        if row.iter().all(String::is_empty) {
            continue;
        }
        let field = |n: usize| row.get(n).cloned().unwrap_or_default();
        let number = match field(0).parse::<u8>() {
            Ok(number) if number <= 128 => number,
            _ if place == 0 => {
                header = row.clone();
                continue;
            }
            _ => return Err(CsvError::Number { line, text: field(0) }),
        };
        let mut song = SetSong { number, artist: field(1), title: field(2), display: field(3), ..SetSong::default() };
        if song.label().trim().is_empty() {
            return Err(CsvError::Empty { line });
        }
        for (column, device) in header.iter().enumerate().skip(4) {
            let text = field(column);
            if device.is_empty() || text.is_empty() {
                continue;
            }
            match text.parse::<u8>() {
                Ok(program) if program <= 127 => song.steps.push(Step { device: device.clone(), message: StepMessage::Program { bank: None, program }, delay_ms: 0 }),
                _ => return Err(CsvError::Program { line, device: device.clone(), text }),
            }
        }
        songs.push(song);
    }
    Ok(songs)
}

/// Writes a set list as a CSV text that `parse_csv` reads back, with a column
/// for each device named: the program of the song's first step to it. The
/// control changes and the system exclusive messages of the steps are not
/// written: they live in the settings only.
pub fn to_csv(songs: &[SetSong], devices: &[String]) -> String {
    let field = |text: &str| {
        if text.contains([',', ';', '"', '\n']) {
            format!("\"{}\"", text.replace('"', "\"\""))
        } else {
            text.to_owned()
        }
    };
    let mut text = String::from("Number,Artist,Song,Display");
    for device in devices {
        text.push(',');
        text.push_str(&field(device));
    }
    text.push('\n');
    for song in songs {
        text.push_str(&format!("{},{},{},{}", song.number, field(&song.artist), field(&song.title), field(&song.display)));
        for device in devices {
            text.push(',');
            let program = song.steps.iter().find_map(|step| match &step.message {
                StepMessage::Program { program, .. } if same(&step.device, device) => Some(*program),
                _ => None,
            });
            if let Some(program) = program {
                text.push_str(&program.to_string());
            }
        }
        text.push('\n');
    }
    text
}

/// The names of the devices the songs send to, in the order they are first met.
pub fn devices_named(songs: &[SetSong]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for step in songs.iter().flat_map(|song| &song.steps) {
        if !names.iter().any(|name| same(name, &step.device)) {
            names.push(step.device.clone());
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(number: u8, artist: &str, title: &str) -> SetSong {
        SetSong { number, artist: artist.to_owned(), title: title.to_owned(), ..SetSong::default() }
    }

    #[test]
    fn a_list_is_read_with_its_header_and_without_the_text_of_the_buttons() {
        let songs = parse_csv("Numero,Artiste,Chanson,Display\n3,Oasis,Wonderwall,\n12,AC/DC,Highway to Hell,Intro AC/DC\n7,Nirvana,Lithium\n").unwrap();
        assert_eq!(songs.len(), 3);
        assert_eq!((songs[0].number, songs[0].label()), (3, "Oasis - Wonderwall".to_owned()));
        assert_eq!(songs[1].label(), "Intro AC/DC");
        assert_eq!((songs[2].title.as_str(), songs[2].display.as_str()), ("Lithium", ""));
    }

    #[test]
    fn semicolons_quotes_and_a_byte_order_mark_are_understood() {
        let songs = parse_csv("\u{feff}1;\"Guns N' Roses\";\"Sweet Child O' Mine\"\r\n2;Nirvana;\"Lithium; live\";\r\n3;\"Say \"\"hi\"\"\";Song\n").unwrap();
        assert_eq!(songs[0].label(), "Guns N' Roses - Sweet Child O' Mine");
        assert_eq!(songs[1].title, "Lithium; live");
        assert_eq!(songs[2].artist, "Say \"hi\"");
    }

    #[test]
    fn a_wrong_number_or_an_empty_song_is_refused_with_its_line() {
        assert_eq!(parse_csv("1,A,B\nabc,Artist,Song"), Err(CsvError::Number { line: 2, text: "abc".to_owned() }));
        assert_eq!(parse_csv("200,Artist,Song"), Ok(Vec::new()), "a first line that is not a song is a header");
        assert!(matches!(parse_csv("1,A,B\n200,Artist,Song"), Err(CsvError::Number { line: 2, .. })));
        assert_eq!(parse_csv("1,A,B\n2,,,"), Err(CsvError::Empty { line: 2 }));
    }

    #[test]
    fn a_list_written_reads_back_the_same() {
        let mut songs = vec![song(0, "Oasis", "Wonderwall"), song(1, "Guns N' Roses", "Lithium; live, \"loud\"")];
        songs[0].display = "Wall".to_owned();
        assert_eq!(parse_csv(&to_csv(&songs, &[])).unwrap(), songs);
    }

    #[test]
    fn the_orders_and_the_artists() {
        let songs = vec![song(0, "Oasis", "Wonderwall"), song(1, "AC/DC", "Highway to Hell"), song(2, "Oasis", "Don't Look Back in Anger"), song(3, "ac/dc", "Back in Black")];
        assert_eq!(order(&songs, SetOrder::List), [0, 1, 2, 3]);
        assert_eq!(order(&songs, SetOrder::Alphabetical), [3, 1, 2, 0]);
        assert_eq!(order(&songs, SetOrder::ByArtist), [3, 1, 2, 0]);
        assert_eq!(artists(&songs), [("AC/DC".to_owned(), 2), ("Oasis".to_owned(), 2)]);
        assert!(same("Guns N' Roses", "guns n roses"));
    }

    #[test]
    fn the_value_sent_and_its_bytes() {
        let mut list = SetList::default();
        assert_eq!((list.value_of(0), list.value_of(127), list.value_of(128)), (Some(0), Some(127), None));
        list.numbers_from_one = true;
        assert_eq!((list.value_of(0), list.value_of(1), list.value_of(128)), (None, Some(0), Some(127)));
        assert_eq!(SetMessage::BankSelect.bytes(4, 7), [0xB4, 0, 7]);
        assert_eq!(SetMessage::ProgramChange.bytes(4, 7), [0xC4, 7]);
    }

    fn device(name: &str, channel: u8) -> Device {
        Device { name: name.to_owned(), model: String::new(), channel, ..Device::default() }
    }

    fn program(device: &str, program: u8) -> Step {
        Step { device: device.to_owned(), message: StepMessage::Program { bank: None, program }, delay_ms: 0 }
    }

    #[test]
    fn the_switcher_mode_sends_what_it_always_sent() {
        let mut list = SetList { channel: 4, devices: vec![device("DD-500", 0)], ..SetList::default() };
        let mut song = song(7, "Oasis", "Wonderwall");
        song.steps.push(program("DD-500", 3));
        assert_eq!(list.messages_for(&song), Ok(vec![(vec![0xB4, 0, 7], 0)]), "the steps are not the switcher's business");
        list.message = SetMessage::ProgramChange;
        assert_eq!(list.messages_for(&song), Ok(vec![(vec![0xC4, 7], 0)]));
        song.number = 128;
        assert_eq!(list.messages_for(&song), Err(SendError::Number(128)));
    }

    #[test]
    fn a_chain_song_sends_its_steps_in_order_on_the_channel_of_each_device() {
        let list = SetList { mode: PilotMode::Chain, devices: vec![device("DD-500", 0), device("rv 500", 1)], ..SetList::default() };
        let mut tune = song(7, "Oasis", "Wonderwall");
        tune.steps.push(Step { delay_ms: 20, ..program("dd500", 3) });
        tune.steps.push(Step { device: "RV-500".to_owned(), message: StepMessage::Control { number: 80, value: 127 }, delay_ms: 0 });
        assert_eq!(list.messages_for(&tune), Ok(vec![(vec![0xC0, 3], 20), (vec![0xB1, 80, 127], 0)]), "the names fold their case and signs");
        tune.steps.push(program("Looper", 1));
        assert_eq!(list.messages_for(&tune), Err(SendError::Device("Looper".to_owned())));
        assert_eq!(list.messages_for(&song(1, "A", "B")), Ok(Vec::new()), "a song without steps sends nothing");
    }

    #[test]
    fn a_step_with_a_bank_is_three_messages_and_a_sysex_is_framed() {
        assert_eq!(StepMessage::Program { bank: Some(2), program: 39 }.messages(3), [vec![0xB3, 0, 2], vec![0xB3, 32, 0], vec![0xC3, 39]]);
        assert_eq!(StepMessage::Program { bank: None, program: 200 }.messages(19), [vec![0xC3, 72]], "out of range, the bits are masked");
        assert_eq!(StepMessage::SysEx(vec![0x41, 0x10, 0xFF]).messages(0), [vec![0xF0, 0x41, 0x10, 0x7F, 0xF7]]);
        assert_eq!(StepMessage::SysEx(vec![0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]).messages(0), [vec![0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7]], "framed already");
        let list = SetList { mode: PilotMode::Chain, devices: vec![device("Amp", 5)], ..SetList::default() };
        let mut song = song(1, "A", "B");
        song.steps.push(Step { device: "Amp".to_owned(), message: StepMessage::Program { bank: Some(1), program: 2 }, delay_ms: 50 });
        assert_eq!(list.messages_for(&song).unwrap().iter().map(|(_, pause)| *pause).collect::<Vec<_>>(), [0, 0, 50], "the pause follows the last message of the step");
    }

    #[test]
    fn a_device_column_of_the_csv_becomes_a_program_step_and_is_written_back() {
        let songs = parse_csv("Number;Artist;Song;Display;DD-500;RV-500\n2;Pink Floyd;Time;;6;2\n3;Oasis;Wonderwall;;;1\n4;Nirvana;Lithium\n").unwrap();
        assert_eq!(songs[0].steps, [program("DD-500", 6), program("RV-500", 2)]);
        assert_eq!(songs[1].steps, [program("RV-500", 1)], "an empty cell is no step");
        assert!(songs[2].steps.is_empty(), "a short line is no step");
        assert_eq!(devices_named(&songs), ["DD-500", "RV-500"]);
        let text = to_csv(&songs, &["DD-500".to_owned(), "RV-500".to_owned()]);
        assert_eq!(text.lines().next(), Some("Number,Artist,Song,Display,DD-500,RV-500"));
        assert_eq!(parse_csv(&text).unwrap(), songs);
        assert_eq!(to_csv(&songs, &[]).lines().nth(1), Some("2,Pink Floyd,Time,"), "without devices, the four columns of before");
        assert!(parse_csv("1;A;B;;5").unwrap()[0].steps.is_empty(), "a column without a header is no device");
    }

    #[test]
    fn a_wrong_program_in_a_device_column_is_refused_with_its_line() {
        assert_eq!(parse_csv("Number,Artist,Song,Display,DD-500\n1,A,B,,x"), Err(CsvError::Program { line: 2, device: "DD-500".to_owned(), text: "x".to_owned() }));
        assert!(matches!(parse_csv("Number,Artist,Song,Display,DD-500\n1,A,B,,5\n2,C,D,,128"), Err(CsvError::Program { line: 3, .. })));
    }

    #[test]
    fn a_list_from_before_the_chain_mode_still_reads_and_writes_nothing_new() {
        let list: SetList = serde_json::from_str(r#"{"songs":[{"number":3,"artist":"Oasis","title":"Wonderwall"}],"channel":4,"message":"ProgramChange"}"#).unwrap();
        assert_eq!((list.mode, list.devices.len(), list.songs[0].steps.len(), list.channel), (PilotMode::Switcher, 0, 0, 4));
        let text = serde_json::to_string(&list).unwrap();
        assert!(!text.contains("steps") && !text.contains("devices") && text.contains("\"mode\":\"Switcher\""), "{text}");
        let mut chain = list.clone();
        chain.mode = PilotMode::Chain;
        chain.devices.push(device("DD-500", 0));
        chain.songs[0].steps.push(program("DD-500", 1));
        let again: SetList = serde_json::from_str(&serde_json::to_string(&chain).unwrap()).unwrap();
        assert_eq!(again, chain);
    }
}
