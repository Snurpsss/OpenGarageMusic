//! The backup file of The GigRig G3S pedal switcher (`.g3b`): its banks,
//! their presets (the loops each turns on, the order of the loops in the
//! signal, its name, the MIDI messages it sends), the names of the banks
//! (the songs), and the global settings. The program edits such a file and
//! writes it back for the maker's editor to restore in the switcher.
//!
//! The format is not published: it was deduced from backup files the
//! switcher made, and checked by restoring files written this way. What is
//! known is read and written; what is not (most of the global settings, the
//! MIDI map) is carried over as it is.
//!
//! The file is a run of system exclusive messages of 50 bytes (the last one
//! of 18): `F0 00 21 37 03 kind seq a b part data… ck ck F7`, `data` being
//! 32 bytes packed in 37 (groups of 8 bytes: one byte of high bits, then 7
//! bytes), `ck` the two hexadecimal characters of the checksum (the sum of
//! the bytes from `kind` to the end of the data, taken to zero), and `seq`
//! a 7-bit counter over the whole file. Four messages (`part` 1 to 4) make a
//! record of 128 bytes. The kinds: 1 the global settings (`b` 1 to 7), 4 the
//! MIDI map (`b` 1 and 2), 3 the presets (`a` the bank from 1, `b` 1 to 28
//! the presets, 29 to 32 copies of the first four of the next bank), 6 the
//! extended MIDI of the presets (the same addressing), 7 the songs (`b` 0
//! the order, 1 to 17 the hundred names of 21 bytes), 9 the end (the sum of
//! every checksum before, in 4 little-endian bytes packed in 5).

/// The maker's identity and the device byte of the G3 and G3S.
pub const MAKER: [u8; 3] = [0x00, 0x21, 0x37];
pub const DEVICE: u8 = 0x03;
/// The bytes a message carries; four messages make a record.
const DATA: usize = 32;
pub const RECORD: usize = 128;
/// The presets of a bank: 14 shown, 14 hidden; then the copies of the next bank.
pub const PRESETS: usize = 28;
const COPIES: usize = 4;
pub const LOOPS: usize = 12;
/// The characters of a preset's name, and of a bank's (two lines, `|` between).
pub const NAME: usize = 9;
pub const BANK_NAME: usize = 20;
/// The MIDI lines of a preset: 13 in its record, 7 in the extended one.
pub const MIDI_LINES: usize = 20;
const LINES_IN_PRESET: usize = 13;
pub const SONGS: usize = 100;
const SONG_BYTES: usize = 21;
const SONG_RECORDS: usize = 17;
const GLOBAL_RECORDS: usize = 7;
const MIDI_MAP_RECORDS: usize = 2;
pub const MOST_BANKS: usize = 99;

/// The kinds of records.
const GLOBALS: u8 = 1;
const PRESET: u8 = 3;
const MIDI_MAP: u8 = 4;
const EXTENDED: u8 = 6;
const SONGS_KIND: u8 = 7;
const END: u8 = 9;

/// The kinds of MIDI message a preset's line may send, by the high four
/// bits of its status byte: a program change or a control change, when the
/// switch is pressed (down), released (up), held and the others.
pub const MIDI_KINDS: [(u8, &str); 13] = [
    (0xC0, "PC down"),
    (0xB0, "CC down"),
    (0x10, "PC up"),
    (0x00, "CC up"),
    (0x80, "PC stomp"),
    (0x70, "CC stomp"),
    (0x30, "PC sequence"),
    (0x20, "CC sequence"),
    (0x60, "PC remote"),
    (0x50, "CC remote"),
    (0xA0, "PC stomp on"),
    (0x90, "CC stomp on"),
    (0x40, "off"),
];

/// Seven bytes packed in eight: the high bits first, then the bytes without them.
fn pack7(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + bytes.len().div_ceil(7));
    for chunk in bytes.chunks(7) {
        out.push(chunk.iter().enumerate().fold(0, |bits, (k, b)| bits | (((b >> 7) & 1) << k)));
        out.extend(chunk.iter().map(|b| b & 0x7F));
    }
    out
}

/// Eight bytes unpacked in seven.
fn unpack7(packed: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(packed.len());
    for chunk in packed.chunks(8) {
        let bits = chunk[0];
        out.extend(chunk[1..].iter().enumerate().map(|(k, b)| b | (((bits >> k) & 1) << 7)));
    }
    out
}

/// The checksum: the bytes from the kind to the end of the data, summed, taken to zero.
fn checksum(body: &[u8]) -> u8 {
    let sum: u32 = body.iter().map(|b| u32::from(*b)).sum();
    (sum.wrapping_neg() & 0xFF) as u8
}

fn hex_pair(value: u8) -> [u8; 2] {
    let text = format!("{value:02X}");
    [text.as_bytes()[0], text.as_bytes()[1]]
}

/// One message of the file, less its sequence number, which the writing gives.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Message {
    kind: u8,
    a: u8,
    b: u8,
    part: u8,
    data: Vec<u8>,
}

/// The messages of a file, read, each checked; the end's sum is checked too.
fn parse_messages(bytes: &[u8]) -> Result<Vec<Message>, String> {
    let mut messages = Vec::new();
    let mut at = 0;
    let mut sum: u32 = 0;
    let mut ended = false;
    while at < bytes.len() {
        if ended {
            return Err("bytes after the end".to_owned());
        }
        if bytes[at] != 0xF0 {
            return Err(format!("no message at byte {at}"));
        }
        let Some(len) = bytes[at..].iter().position(|b| *b == 0xF7) else { return Err("a message without an end".to_owned()) };
        let message = &bytes[at..=at + len];
        at += len + 1;
        let number = messages.len();
        if message.len() < 14 || message[1..4] != MAKER || message[4] != DEVICE {
            return Err(format!("message {number} is not the switcher's"));
        }
        let (body, tail) = message[5..message.len() - 1].split_at(message.len() - 8);
        let ck = u8::from_str_radix(std::str::from_utf8(tail).unwrap_or("zz"), 16).map_err(|_| format!("message {number} has no checksum"))?;
        if checksum(body) != ck {
            return Err(format!("message {number} has a wrong checksum"));
        }
        let (kind, seq) = (body[0], body[1]);
        if seq != ((number + 1) & 0x7F) as u8 {
            return Err(format!("message {number} is out of sequence"));
        }
        if kind == END {
            if body.len() != 10 {
                return Err("the end is not 18 bytes".to_owned());
            }
            let packed = unpack7(&body[5..10]);
            let said = u32::from_le_bytes([packed[0], packed[1], packed[2], packed[3]]);
            if said != sum {
                return Err(format!("the sum of the checksums is {sum}, the end says {said}"));
            }
            ended = true;
            continue;
        }
        sum += u32::from(ck);
        if body.len() != 42 {
            return Err(format!("message {number} is not 50 bytes"));
        }
        let data = unpack7(&body[5..42]);
        debug_assert_eq!(data.len(), DATA);
        messages.push(Message { kind, a: body[2], b: body[3], part: body[4], data });
    }
    if !ended {
        return Err("the file has no end".to_owned());
    }
    Ok(messages)
}

/// The messages written as a file: the sequence counted, the end summed.
fn write_messages(messages: &[Message]) -> Vec<u8> {
    let mut out = Vec::with_capacity(messages.len() * 50 + 18);
    let mut sum: u32 = 0;
    let mut push = |body: &[u8]| {
        let ck = checksum(body);
        out.push(0xF0);
        out.extend_from_slice(&MAKER);
        out.push(DEVICE);
        out.extend_from_slice(body);
        out.extend_from_slice(&hex_pair(ck));
        out.push(0xF7);
        ck
    };
    for (i, m) in messages.iter().enumerate() {
        let mut body = vec![m.kind, ((i + 1) & 0x7F) as u8, m.a, m.b, m.part];
        body.extend(pack7(&m.data));
        sum += u32::from(push(&body));
    }
    let mut body = vec![END, ((messages.len() + 1) & 0x7F) as u8, 0, 0, 1];
    body.extend(pack7(&sum.to_le_bytes()));
    push(&body);
    out
}

/// One MIDI message a preset sends.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MidiLine {
    /// The high four bits of the status byte (see `MIDI_KINDS`).
    pub kind: u8,
    /// The channel, from 0 to 15.
    pub channel: u8,
    /// The program, or the controller.
    pub number: u8,
    /// The value of a control change.
    pub value: u8,
}

/// A preset: its record, and its extended MIDI record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preset {
    pub raw: Vec<u8>,
    pub extended: Vec<u8>,
}

impl Preset {
    pub fn name(&self) -> String {
        String::from_utf8_lossy(&self.raw[19..19 + NAME]).trim_end().to_owned()
    }

    /// Names the preset, at most `NAME` characters of ASCII.
    pub fn set_name(&mut self, name: &str) {
        let mut bytes: Vec<u8> = name.chars().filter(|c| c.is_ascii() && !c.is_control()).map(|c| c as u8).take(NAME).collect();
        bytes.resize(NAME, b' ');
        self.raw[19..19 + NAME].copy_from_slice(&bytes);
        self.raw[28] = 0;
    }

    /// Whether a loop (from 0) is on.
    pub fn loop_on(&self, n: usize) -> bool {
        let mask = u16::from(self.raw[5]) | (u16::from(self.raw[6]) << 8);
        n < LOOPS && mask & (1 << n) != 0
    }

    pub fn set_loop(&mut self, n: usize, on: bool) {
        if n >= LOOPS {
            return;
        }
        let mut mask = u16::from(self.raw[5]) | (u16::from(self.raw[6]) << 8);
        if on {
            mask |= 1 << n;
        } else {
            mask &= !(1 << n);
        }
        self.raw[5] = (mask & 0xFF) as u8;
        self.raw[6] = ((mask >> 8) & 0x0F) as u8;
    }

    /// The loops in the order of the signal, from 1.
    pub fn order(&self) -> [u8; LOOPS] {
        let mut order = [0; LOOPS];
        order.copy_from_slice(&self.raw[7..7 + LOOPS]);
        order
    }

    /// Puts the loops in this order: every loop from 1 to 12 once.
    pub fn set_order(&mut self, order: &[u8; LOOPS]) -> Result<(), String> {
        let mut sorted = *order;
        sorted.sort_unstable();
        if sorted.iter().enumerate().any(|(i, n)| usize::from(*n) != i + 1) {
            return Err("every loop once, from 1 to 12".to_owned());
        }
        self.raw[7..7 + LOOPS].copy_from_slice(order);
        Ok(())
    }

    /// Moves the loop at a place of the order one step up or down.
    pub fn move_loop(&mut self, place: usize, up: bool) {
        let other = if up { place.checked_sub(1) } else { (place + 1 < LOOPS).then_some(place + 1) };
        if let Some(other) = other {
            self.raw.swap(7 + place, 7 + other);
        }
    }

    /// Where a line's three bytes are: in the record, or the extended one.
    fn line_bytes(&self, line: usize) -> Option<(&[u8], usize, usize, usize)> {
        if line < LINES_IN_PRESET {
            Some((&self.raw, 64 + line, 79 + line, 94 + line))
        } else if line < MIDI_LINES {
            let n = line - LINES_IN_PRESET;
            Some((&self.extended, n, 15 + n, 30 + n))
        } else {
            None
        }
    }

    /// A MIDI line, when it is set.
    pub fn midi(&self, line: usize) -> Option<MidiLine> {
        let (bytes, status, data1, data2) = self.line_bytes(line)?;
        if bytes[data1] & 0x80 != 0 {
            return None;
        }
        Some(MidiLine { kind: bytes[status] & 0xF0, channel: bytes[status] & 0x0F, number: bytes[data1] & 0x7F, value: bytes[data2] & 0x7F })
    }

    pub fn set_midi(&mut self, line: usize, midi: MidiLine) {
        let Some((_, status, data1, data2)) = self.line_bytes(line) else { return };
        let bytes = if line < LINES_IN_PRESET { &mut self.raw } else { &mut self.extended };
        bytes[status] = (midi.kind & 0xF0) | (midi.channel & 0x0F);
        bytes[data1] = midi.number & 0x7F;
        bytes[data2] = midi.value & 0x7F;
    }

    /// Clears a MIDI line, as the switcher leaves it from the factory.
    pub fn clear_midi(&mut self, line: usize) {
        let Some((_, status, data1, data2)) = self.line_bytes(line) else { return };
        let bytes = if line < LINES_IN_PRESET { &mut self.raw } else { &mut self.extended };
        bytes[status] = 0xB0;
        bytes[data1] = 0x80;
        bytes[data2] = 0x00;
    }

    /// The lines set, in their order.
    pub fn midi_lines(&self) -> Vec<(usize, MidiLine)> {
        (0..MIDI_LINES).filter_map(|line| self.midi(line).map(|m| (line, m))).collect()
    }
}

/// A preset as the switcher makes it from the factory, in a bank (from 0):
/// no loop on, the loops in order, named "Preset nn" (from 1), no MIDI;
/// the bytes whose meaning is not known are those every such preset holds.
pub fn factory_preset(n: usize, bank: usize) -> Preset {
    let mut raw = vec![0; RECORD];
    raw[RECORD - 1] = bank as u8 + 1;
    raw[0] = 0x20;
    raw[2] = 0x15;
    raw[4] = 0x15;
    raw[29] = 0x01;
    raw[30] = 0x0C;
    raw[33] = 0x01;
    raw[120] = 0x78;
    // Fifteen slots of MIDI lines in each record, cleared.
    let mut extended = vec![0; RECORD];
    for slot in 0..15 {
        raw[64 + slot] = 0xB0;
        raw[79 + slot] = 0x80;
        extended[slot] = 0xB0;
        extended[15 + slot] = 0x80;
    }
    let mut preset = Preset { raw, extended };
    let _ = preset.set_order(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    preset.set_name(&format!("Preset {:02}", n + 1));
    preset
}

/// A bank: its presets, 14 shown then 14 hidden.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    pub presets: Vec<Preset>,
}

/// A backup of the switcher, as the program keeps it between a read and a write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Backup {
    /// The global settings, carried over as they are but the number of banks.
    pub globals: Vec<Vec<u8>>,
    pub midi_map: Vec<Vec<u8>>,
    pub banks: Vec<Bank>,
    /// The order of the songs, carried over as it is.
    pub song_order: Vec<u8>,
    /// The names of the banks (the songs), two lines with `|` between.
    pub song_names: Vec<String>,
    /// How the song records were addressed, carried over.
    songs_a: u8,
    /// Bytes past the hundred names, carried over.
    song_tail: Vec<u8>,
}

impl Backup {
    /// Reads a backup file.
    pub fn parse(bytes: &[u8]) -> Result<Backup, String> {
        let messages = parse_messages(bytes)?;
        // The records: four parts each, in their order.
        let mut records: Vec<(u8, u8, u8, Vec<u8>)> = Vec::new();
        for m in &messages {
            match records.last_mut() {
                Some((kind, a, b, data)) if *kind == m.kind && *a == m.a && *b == m.b && data.len() < RECORD => {
                    if m.part as usize != data.len() / DATA + 1 {
                        return Err(format!("a part of record {}/{}/{} is out of place", m.kind, m.a, m.b));
                    }
                    data.extend_from_slice(&m.data);
                }
                _ => {
                    if m.part != 1 {
                        return Err(format!("record {}/{}/{} does not start at its first part", m.kind, m.a, m.b));
                    }
                    records.push((m.kind, m.a, m.b, m.data.clone()));
                }
            }
        }
        if records.iter().any(|(_, _, _, data)| data.len() != RECORD) {
            return Err("a record is not whole".to_owned());
        }
        let of = |kind: u8| records.iter().filter(move |r| r.0 == kind);
        let globals: Vec<Vec<u8>> = of(GLOBALS).map(|r| r.3.clone()).collect();
        let midi_map: Vec<Vec<u8>> = of(MIDI_MAP).map(|r| r.3.clone()).collect();
        if globals.len() != GLOBAL_RECORDS || midi_map.len() != MIDI_MAP_RECORDS {
            return Err(format!("{} records of global settings and {} of the MIDI map", globals.len(), midi_map.len()));
        }
        let bank_count = of(PRESET).map(|r| usize::from(r.1)).max().unwrap_or(0);
        if bank_count == 0 || bank_count > MOST_BANKS {
            return Err(format!("{bank_count} banks"));
        }
        let mut banks = Vec::with_capacity(bank_count);
        for bank in 1..=bank_count {
            let mut presets = Vec::with_capacity(PRESETS);
            for b in 1..=PRESETS {
                let raw = of(PRESET).find(|r| usize::from(r.1) == bank && usize::from(r.2) == b).map(|r| r.3.clone()).ok_or_else(|| format!("bank {bank} lacks its preset {b}"))?;
                let extended = of(EXTENDED).find(|r| usize::from(r.1) == bank && usize::from(r.2) == b).map(|r| r.3.clone()).ok_or_else(|| format!("bank {bank} lacks the extended MIDI of its preset {b}"))?;
                presets.push(Preset { raw, extended });
            }
            banks.push(Bank { presets });
        }
        let songs: Vec<&(u8, u8, u8, Vec<u8>)> = of(SONGS_KIND).collect();
        if songs.len() != SONG_RECORDS + 1 {
            return Err(format!("{} records of songs", songs.len()));
        }
        let songs_a = songs[0].1;
        let song_order = songs.iter().find(|r| r.2 == 0).map(|r| r.3.clone()).ok_or("no order of the songs")?;
        let mut names_bytes = Vec::with_capacity(SONG_RECORDS * RECORD);
        for b in 1..=SONG_RECORDS {
            let record = songs.iter().find(|r| usize::from(r.2) == b).ok_or_else(|| format!("no record {b} of the songs"))?;
            names_bytes.extend_from_slice(&record.3);
        }
        let song_names = (0..SONGS).map(|i| text_of(&names_bytes[i * SONG_BYTES..(i + 1) * SONG_BYTES])).collect();
        let song_tail = names_bytes[SONGS * SONG_BYTES..].to_vec();
        Ok(Backup { globals, midi_map, banks, song_order, song_names, songs_a, song_tail })
    }

    /// Writes the backup as a file the switcher's editor restores: the
    /// copies of the next bank's first presets, the banks' numbers, the
    /// number of banks in the global settings, the sequence and the end sum
    /// are all made here.
    pub fn write(&self) -> Vec<u8> {
        let mut messages = Vec::new();
        let mut push = |kind: u8, a: u8, b: u8, data: &[u8]| {
            for (i, chunk) in data.chunks(DATA).enumerate() {
                messages.push(Message { kind, a, b, part: i as u8 + 1, data: chunk.to_vec() });
            }
        };
        for (i, record) in self.globals.iter().enumerate() {
            let mut record = record.clone();
            if i == 0 {
                record[4] = (record[4] & 0x80) | (self.banks.len() as u8 & 0x7F);
            }
            push(GLOBALS, 0, i as u8 + 1, &record);
        }
        for (i, record) in self.midi_map.iter().enumerate() {
            push(MIDI_MAP, 0, i as u8 + 1, record);
        }
        // The last byte of a preset says which bank it was made in; the
        // switcher keeps it when a preset is copied, so it is carried over.
        for kind in [PRESET, EXTENDED] {
            for (bank, presets) in self.banks.iter().enumerate() {
                for (b, preset) in presets.presets.iter().enumerate() {
                    let record = if kind == PRESET { &preset.raw } else { &preset.extended };
                    push(kind, bank as u8 + 1, b as u8 + 1, record);
                }
                // The first presets of the next bank, copied; after the last
                // bank, the factory's, numbered as the last bank.
                for b in 0..COPIES {
                    let preset = match self.banks.get(bank + 1) {
                        Some(next) => next.presets[b].clone(),
                        None => factory_preset(b, bank),
                    };
                    let record = if kind == PRESET { &preset.raw } else { &preset.extended };
                    push(kind, bank as u8 + 1, (PRESETS + b) as u8 + 1, record);
                }
            }
        }
        push(SONGS_KIND, self.songs_a, 0, &self.song_order);
        let mut names = Vec::with_capacity(SONG_RECORDS * RECORD);
        for i in 0..SONGS {
            let mut bytes: Vec<u8> = self.song_names.get(i).map(|name| name.bytes().filter(|b| b.is_ascii() && !b.is_ascii_control()).take(BANK_NAME).collect()).unwrap_or_default();
            bytes.resize(SONG_BYTES, 0);
            names.extend(bytes);
        }
        names.extend_from_slice(&self.song_tail);
        names.resize(SONG_RECORDS * RECORD, 0);
        for (i, record) in names.chunks(RECORD).enumerate() {
            push(SONGS_KIND, self.songs_a, i as u8 + 1, record);
        }
        write_messages(&messages)
    }

    /// The switcher shows the banks as songs (their names) or as numbers.
    pub fn song_mode(&self) -> bool {
        self.globals[0][4] & 0x80 != 0
    }

    pub fn set_song_mode(&mut self, on: bool) {
        self.globals[0][4] = (self.globals[0][4] & 0x7F) | if on { 0x80 } else { 0 };
    }

    /// The name of a bank: the song shown for it.
    pub fn bank_name(&self, bank: usize) -> String {
        self.song_names.get(bank).cloned().unwrap_or_default()
    }

    pub fn set_bank_name(&mut self, bank: usize, name: &str) {
        if bank < SONGS {
            if self.song_names.len() < SONGS {
                self.song_names.resize(SONGS, String::new());
            }
            self.song_names[bank] = name.chars().filter(|c| c.is_ascii() && !c.is_control()).take(BANK_NAME).collect();
        }
    }

    /// Adds a bank at the end, its presets as the factory's. None past the most.
    pub fn add_bank(&mut self) -> Option<usize> {
        if self.banks.len() >= MOST_BANKS {
            return None;
        }
        self.banks.push(factory_bank(self.banks.len()));
        let number = self.banks.len() - 1;
        self.set_bank_name(number, "");
        Some(number)
    }

    /// Takes a bank out, with its name; the names after it move up.
    pub fn remove_bank(&mut self, bank: usize) {
        if bank < self.banks.len() && self.banks.len() > 1 {
            self.banks.remove(bank);
            if bank < self.song_names.len() {
                self.song_names.remove(bank);
                self.song_names.push(String::new());
            }
        }
    }

    /// Swaps a bank with the one before or after it, names included.
    pub fn move_bank(&mut self, bank: usize, up: bool) {
        let other = if up { bank.checked_sub(1) } else { (bank + 1 < self.banks.len()).then_some(bank + 1) };
        if let Some(other) = other {
            self.banks.swap(bank, other);
            if other < self.song_names.len() && bank < self.song_names.len() {
                self.song_names.swap(bank, other);
            }
        }
    }

    /// Checks a file as the switcher does when it is restored: every
    /// checksum, the sequence, the end's sum, and the banks declared.
    pub fn check(bytes: &[u8]) -> Result<String, String> {
        let backup = Backup::parse(bytes)?;
        let declared = usize::from(backup.globals[0][4] & 0x7F);
        if declared != backup.banks.len() {
            return Err(format!("{declared} banks declared, {} in the file", backup.banks.len()));
        }
        Ok(format!("{} banks, {} messages", backup.banks.len(), bytes.len() / 50 + 1))
    }
}

/// A text up to its first zero byte, as the switcher keeps names.
fn text_of(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|b| *b as char).collect()
}

/// A backup made up, as a new one the tests and the pictures start from:
/// its global settings zeroed but the number of banks, its presets as the
/// factory leaves them.
/// A bank (from 0) as the factory leaves it.
pub fn factory_bank(bank: usize) -> Bank {
    Bank { presets: (0..PRESETS).map(|n| factory_preset(n, bank)).collect() }
}

pub fn blank(banks: usize) -> Backup {
    let mut backup = Backup {
        globals: vec![vec![0; RECORD]; GLOBAL_RECORDS],
        midi_map: vec![vec![0; RECORD]; MIDI_MAP_RECORDS],
        banks: (0..banks.clamp(1, MOST_BANKS)).map(factory_bank).collect(),
        song_order: vec![0; RECORD],
        song_names: vec![String::new(); SONGS],
        songs_a: 1,
        song_tail: Vec::new(),
    };
    backup.globals[0][4] = backup.banks.len() as u8;
    backup
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_are_packed_seven_in_eight_and_summed() {
        let bytes: Vec<u8> = (0..32).map(|i| (i * 37 % 256) as u8).collect();
        let packed = pack7(&bytes);
        assert_eq!(packed.len(), 37);
        assert!(packed.iter().all(|b| *b < 0x80));
        assert_eq!(unpack7(&packed), bytes);
        assert_eq!(checksum(&[1, 2, 3]), 0xFA);
        assert_eq!(hex_pair(0xFA), *b"FA");
    }

    #[test]
    fn a_backup_made_up_is_written_read_back_and_checked() {
        let mut backup = blank(3);
        backup.set_song_mode(true);
        backup.set_bank_name(1, "SHINE ON|DIAMOND");
        let preset = &mut backup.banks[1].presets[1];
        preset.set_name("NAPPE");
        preset.set_loop(9, true);
        preset.set_loop(6, true);
        preset.set_order(&[10, 7, 11, 12, 1, 2, 3, 4, 5, 6, 8, 9]).unwrap();
        preset.set_midi(0, MidiLine { kind: 0xC0, channel: 0, number: 4, value: 0 });
        preset.set_midi(15, MidiLine { kind: 0xB0, channel: 1, number: 80, value: 127 });
        let bytes = backup.write();
        assert_eq!(bytes.len(), (28 + 8 + 3 * 128 * 2 + 72) * 50 + 18);
        assert!(Backup::check(&bytes).unwrap().contains("3 banks"));
        let again = Backup::parse(&bytes).unwrap();
        assert_eq!(again.banks.len(), 3);
        assert!(again.song_mode());
        assert_eq!((again.bank_name(1), again.bank_name(2)), ("SHINE ON|DIAMOND".to_owned(), String::new()));
        let preset = &again.banks[1].presets[1];
        assert_eq!(preset.name(), "NAPPE");
        assert_eq!((preset.loop_on(9), preset.loop_on(6), preset.loop_on(0)), (true, true, false));
        assert_eq!(preset.order(), [10, 7, 11, 12, 1, 2, 3, 4, 5, 6, 8, 9]);
        assert_eq!(preset.midi_lines(), [(0, MidiLine { kind: 0xC0, channel: 0, number: 4, value: 0 }), (15, MidiLine { kind: 0xB0, channel: 1, number: 80, value: 127 })]);
        assert_eq!(preset.raw[127], 2, "the bank's number");
        assert_eq!(again.write(), bytes, "written again the same");
        // A loop order with a loop twice is refused.
        assert!(again.banks[0].presets[0].clone().set_order(&[1, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]).is_err());
    }

    #[test]
    fn the_copies_of_the_next_bank_follow_its_first_presets() {
        let mut backup = blank(2);
        backup.banks[1].presets[0].set_name("SECOND");
        let bytes = backup.write();
        let messages = parse_messages(&bytes).unwrap();
        let copy = messages.iter().find(|m| m.kind == PRESET && m.a == 1 && m.b == 29 && m.part == 1).unwrap();
        let first = messages.iter().find(|m| m.kind == PRESET && m.a == 2 && m.b == 1 && m.part == 1).unwrap();
        assert_eq!(copy.data, first.data);
        assert_eq!(&copy.data[19..25], b"SECOND");
        let last_copy = messages.iter().find(|m| m.kind == PRESET && m.a == 2 && m.b == 29 && m.part == 4).unwrap();
        assert_eq!(last_copy.data[31], 2, "after the last bank, the factory's presets, numbered as the last bank");
        let last_copy = messages.iter().find(|m| m.kind == PRESET && m.a == 2 && m.b == 29 && m.part == 1).unwrap();
        assert_eq!(&last_copy.data[19..28], b"Preset 01");
    }

    #[test]
    fn banks_are_added_moved_and_removed_with_their_names() {
        let mut backup = blank(1);
        backup.set_bank_name(0, "MANUAL");
        backup.banks[0].presets[2].set_loop(3, true);
        assert_eq!(backup.add_bank(), Some(1));
        assert!(!backup.banks[1].presets[2].loop_on(3), "the new bank starts cleared");
        assert_eq!(backup.banks[1].presets[2].name(), "Preset 03");
        backup.set_bank_name(1, "TIME");
        backup.move_bank(1, true);
        assert_eq!((backup.bank_name(0).as_str(), backup.bank_name(1).as_str()), ("TIME", "MANUAL"));
        assert!(backup.banks[1].presets[2].loop_on(3), "moved with its presets");
        backup.remove_bank(0);
        assert_eq!((backup.banks.len(), backup.bank_name(0).as_str()), (1, "MANUAL"));
        backup.remove_bank(0);
        assert_eq!(backup.banks.len(), 1, "the last bank stays");
        let mut full = blank(MOST_BANKS);
        assert_eq!(full.add_bank(), None);
    }

    #[test]
    fn a_broken_file_is_refused_with_a_reason() {
        let bytes = blank(1).write();
        assert!(Backup::parse(&bytes[..100]).unwrap_err().contains("end"));
        let mut wrong = bytes.clone();
        wrong[20] ^= 1;
        assert!(Backup::parse(&wrong).unwrap_err().contains("checksum"));
        let mut swapped = bytes.clone();
        swapped[6] = 2;
        assert!(Backup::parse(&swapped).unwrap_err().contains("sequence") || Backup::parse(&swapped).unwrap_err().contains("checksum"));
        assert!(Backup::parse(b"\xF0\x00\x21\x37\x03\x01\xF7").is_err());
        let mut declared = blank(2);
        declared.globals[0][4] = 5;
        assert!(Backup::check(&declared.write()).is_ok(), "the number of banks is made at the writing");
    }

    /// Applies the G3S actions of a script file to a real backup and writes the
    /// result, checked as the switcher checks it. Run on demand:
    /// `OG_G3S_BACKUP=in.g3b OG_SCRIPT=script.json OG_G3S_OUT=out.g3b dev test -p og-engine -- --ignored apply_a_script`.
    #[test]
    #[ignore = "needs files"]
    fn apply_a_script_to_a_real_backup() {
        let backup = std::fs::read(std::env::var("OG_G3S_BACKUP").expect("OG_G3S_BACKUP")).expect("read the backup");
        let script = std::fs::read_to_string(std::env::var("OG_SCRIPT").expect("OG_SCRIPT")).expect("read the script");
        let out = std::env::var("OG_G3S_OUT").expect("OG_G3S_OUT");
        let mut backup = Backup::parse(&backup).expect("parse the backup");
        let script = crate::script::Script::parse(&script).expect("parse the script");
        for action in script.actions.iter().filter(|a| a.g3s.is_some()) {
            println!("{}", crate::script::apply_g3s(&mut backup, action).expect("apply"));
        }
        let bytes = backup.write();
        println!("{}", Backup::check(&bytes).expect("check"));
        for bank in backup.banks.len().saturating_sub(2)..backup.banks.len() {
            println!("bank {} \"{}\"", bank + 1, backup.bank_name(bank));
            for (i, preset) in backup.banks[bank].presets.iter().enumerate().take(14) {
                let loops: Vec<usize> = (0..LOOPS).filter(|n| preset.loop_on(*n)).map(|n| n + 1).collect();
                if !loops.is_empty() || !preset.midi_lines().is_empty() {
                    println!("  sw {:2} {:9} loops {:?} midi {:?}", i + 1, preset.name(), loops, preset.midi_lines().iter().map(|(_, m)| format!("{:02X} ch{} {} {}", m.kind, m.channel + 1, m.number, m.value)).collect::<Vec<_>>());
                }
            }
        }
        std::fs::write(&out, bytes).expect("write");
    }

    /// Reads a real backup and writes it back the same. Run on demand, on a
    /// file of your own: `OG_G3S_BACKUP=path dev test -p og-engine -- --ignored g3s`.
    #[test]
    #[ignore = "needs a backup file"]
    fn a_real_backup_is_written_back_byte_for_byte() {
        let path = std::env::var("OG_G3S_BACKUP").expect("OG_G3S_BACKUP");
        let bytes = std::fs::read(path).expect("read");
        let backup = Backup::parse(&bytes).expect("parse");
        println!("{} banks, song mode {}, first names: {:?}", backup.banks.len(), backup.song_mode(), &backup.song_names[..3.min(backup.song_names.len())]);
        let written = backup.write();
        if let Some(at) = written.iter().zip(&bytes).position(|(a, b)| a != b) {
            let (m, k) = (at / 50, at % 50);
            println!("first difference at byte {at}: message {m}, byte {k}
  file:    {:02X?}
  written: {:02X?}", &bytes[m * 50..(m * 50 + 50).min(bytes.len())], &written[m * 50..(m * 50 + 50).min(written.len())]);
        }
        assert_eq!(written.len(), bytes.len(), "the lengths");
        assert!(written == bytes, "a difference");
    }
}
