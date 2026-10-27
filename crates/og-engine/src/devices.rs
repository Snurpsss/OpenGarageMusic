//! The MIDI devices the program can configure (the BOSS DD-500 and RV-500
//! delay and reverb pedals), each described by a JSON file written from the
//! maker's public MIDI implementation document: the identity of the device,
//! the map of its memory, and every parameter of a patch, of the system and
//! of a bank, with its place, its range and the labels of its values.
//!
//! The descriptions shipped are in `devices/` beside `src/`; the program also
//! reads those of a `devices` folder next to it. The bytes on the cable are
//! made by [`crate::roland`]; this module knows what they mean.
//!
//! A description:
//! - `format_version`: the version of this language the file is written in.
//! - `key`, `name`, `maker`, `document` (where the public document is).
//! - `model_id` (four bytes), `device_id`.
//! - `patch_size`, `banks` (99), `slots` (3: A, B, C), `pc_map_entries` (384).
//! - `addresses`, in the four-byte form of the document: `current` (the
//!   number of the patch played), `system`, `pc_map`, `temporary` (the
//!   patches played), `user` (the first bank), `bank_stride` and
//!   `patch_stride` (from one bank, one slot, to the next; a bank's own
//!   bytes come first, its patches one stride each after them), `editor_mode`.
//! - `bank`, `system`, `patch`: the parameters, each with `ofs` (bytes from
//!   the start), `name`, `bytes`, `enc` (`7bit`: one byte; `nibble`: 4 bits a
//!   byte, high first; `ascii`: text), `min`, `max`, `init`, and optionally
//!   `offset` (taken from the value shown), `labels` (the name of a list of
//!   `labels`), `shown` (the range as the document writes it), `group`.
//! - `labels`: the lists of the values' names.

use std::collections::HashMap;

use serde::Deserialize;

use crate::roland::{self, Head};

/// The version of the language of descriptions this program reads.
pub const FORMAT_VERSION: u32 = 1;

/// The descriptions shipped with the program: the name of their file and their text.
pub const BUILT_IN: [(&str, &str); 2] = [("dd500.json", include_str!("../devices/dd500.json")), ("rv500.json", include_str!("../devices/rv500.json"))];

/// Most parameters a description may hold, and most patches.
const MOST_PARAMETERS: usize = 2000;
const MOST_PATCHES: usize = 1000;

/// The version a description says it is written in, read before anything else.
pub fn format_of(text: &str) -> Option<u32> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value.get("format_version").and_then(|v| v.as_u64()).and_then(|v| u32::try_from(v).ok())
}

/// How a parameter's bytes hold its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Encoding {
    /// One byte, 0 to 127.
    #[serde(rename = "7bit")]
    Seven,
    /// Four bits a byte, the high ones first.
    Nibble,
    /// Text, a byte a character.
    Ascii,
}

/// A value at the start: a number, or a text for the names.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Init {
    Number(i64),
    Text(String),
}

/// One parameter of a patch, of the system or of a bank.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Param {
    pub ofs: usize,
    pub name: String,
    pub bytes: usize,
    pub enc: Encoding,
    pub min: i64,
    pub max: i64,
    pub init: Init,
    /// Taken from the value for the one shown: a tone of 0 to 100 shown -50 to +50.
    #[serde(default)]
    pub offset: i64,
    /// The name of its list of labels, when its values have names.
    #[serde(default)]
    pub labels: Option<String>,
    /// The range as the document writes it, for the user's eye.
    #[serde(default)]
    pub shown: Option<String>,
    #[serde(default)]
    pub group: String,
}

/// Where the memory of the device is, as the document writes the addresses.
#[derive(Clone, Debug, PartialEq, Deserialize)]
struct AddressesText {
    current: String,
    system: String,
    pc_map: String,
    temporary: String,
    user: String,
    bank_stride: String,
    patch_stride: String,
    editor_mode: String,
}

/// The same, linear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Addresses {
    pub current: u32,
    pub system: u32,
    pub pc_map: u32,
    pub temporary: u32,
    pub user: u32,
    pub bank_stride: u32,
    pub patch_stride: u32,
    pub editor_mode: u32,
}

#[derive(Deserialize)]
struct Text {
    #[serde(default = "first_version")]
    format_version: u32,
    key: String,
    name: String,
    #[serde(default)]
    maker: String,
    #[serde(default)]
    document: String,
    model_id: [u8; 4],
    device_id: u8,
    patch_size: usize,
    banks: usize,
    slots: usize,
    #[serde(default)]
    pc_map_entries: usize,
    addresses: AddressesText,
    #[serde(default)]
    bank: Vec<Param>,
    #[serde(default)]
    system: Vec<Param>,
    patch: Vec<Param>,
    #[serde(default)]
    labels: HashMap<String, Vec<String>>,
}

fn first_version() -> u32 {
    1
}

/// A device described: what it is, and the map of its memory.
#[derive(Clone, Debug, PartialEq)]
pub struct Description {
    pub key: String,
    pub name: String,
    pub maker: String,
    pub document: String,
    pub model_id: [u8; 4],
    pub device_id: u8,
    pub patch_size: usize,
    pub banks: usize,
    pub slots: usize,
    pub pc_map_entries: usize,
    pub addresses: Addresses,
    pub bank: Vec<Param>,
    pub system: Vec<Param>,
    pub patch: Vec<Param>,
    pub labels: HashMap<String, Vec<String>>,
}

/// A value of a parameter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Number(i64),
    Text(String),
}

impl Description {
    /// Reads a description, and checks that it can be used: every parameter
    /// within its area, every list of labels there and of the right length.
    pub fn parse(text: &str) -> Result<Description, String> {
        let t: Text = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if t.format_version > FORMAT_VERSION {
            return Err(format!("written for a newer version of the program (description format {}, this version reads up to {FORMAT_VERSION})", t.format_version));
        }
        if t.key.trim().is_empty() || t.name.trim().is_empty() {
            return Err("a device has a key and a name".to_owned());
        }
        if t.model_id.iter().any(|b| *b > 0x7F) || t.device_id > 0x7F {
            return Err("the model and device numbers are bytes of 7 bits".to_owned());
        }
        if t.patch_size == 0 || t.banks == 0 || t.slots == 0 || t.banks * t.slots > MOST_PATCHES || t.pc_map_entries > 4096 {
            return Err("the sizes of the memory are out of bounds".to_owned());
        }
        let address = |name: &str, text: &str| roland::parse_wire(text).ok_or_else(|| format!("the address {name} is not four bytes of 7 bits: {text}"));
        let a = &t.addresses;
        let addresses = Addresses {
            current: address("current", &a.current)?,
            system: address("system", &a.system)?,
            pc_map: address("pc_map", &a.pc_map)?,
            temporary: address("temporary", &a.temporary)?,
            user: address("user", &a.user)?,
            bank_stride: address("bank_stride", &a.bank_stride)?,
            patch_stride: address("patch_stride", &a.patch_stride)?,
            editor_mode: address("editor_mode", &a.editor_mode)?,
        };
        if addresses.patch_stride < t.patch_size as u32 || addresses.bank_stride < addresses.patch_stride * (t.slots as u32 + 1) {
            return Err("the patches overlap: the strides are smaller than a patch".to_owned());
        }
        if t.patch.len() + t.system.len() + t.bank.len() > MOST_PARAMETERS {
            return Err("too many parameters".to_owned());
        }
        for (area, params, size) in [("patch", &t.patch, Some(t.patch_size)), ("system", &t.system, None), ("bank", &t.bank, None)] {
            for p in params {
                if p.name.trim().is_empty() {
                    return Err(format!("a parameter of the {area} has no name"));
                }
                let bytes = match p.enc {
                    Encoding::Seven => (p.bytes == 1 && p.min >= 0 && p.max <= 127).then_some(1),
                    Encoding::Nibble => ((1..=8).contains(&p.bytes) && p.min >= 0 && p.max < (1i64 << (4 * p.bytes))).then_some(p.bytes),
                    Encoding::Ascii => ((1..=64).contains(&p.bytes) && (32..=126).contains(&p.min) && (p.min..=126).contains(&p.max)).then_some(p.bytes),
                };
                let Some(bytes) = bytes else { return Err(format!("the parameter {} ({area}) cannot hold its range in its bytes", p.name)) };
                if p.min > p.max {
                    return Err(format!("the parameter {} ({area}) has its range upside down", p.name));
                }
                if size.is_some_and(|size| p.ofs + bytes > size) {
                    return Err(format!("the parameter {} ({area}) is past the end of the patch", p.name));
                }
                match (&p.init, p.enc) {
                    (Init::Number(n), Encoding::Seven | Encoding::Nibble) if (p.min..=p.max).contains(n) => {}
                    (Init::Text(s), Encoding::Ascii) if s.len() <= p.bytes && s.bytes().all(|b| i64::from(b) >= p.min && i64::from(b) <= p.max) => {}
                    _ => return Err(format!("the parameter {} ({area}) starts out of its range", p.name)),
                }
                if let Some(list) = &p.labels {
                    let Some(labels) = t.labels.get(list) else { return Err(format!("the parameter {} ({area}) names a list of labels that is not there: {list}", p.name)) };
                    if labels.len() as i64 != p.max - p.min + 1 {
                        return Err(format!("the list {list} has {} labels for the {} values of {}", labels.len(), p.max - p.min + 1, p.name));
                    }
                }
            }
        }
        if t.patch.iter().all(|p| p.enc != Encoding::Ascii) {
            return Err("a patch has a name: a text parameter".to_owned());
        }
        Ok(Description {
            key: t.key,
            name: t.name,
            maker: t.maker,
            document: t.document,
            model_id: t.model_id,
            device_id: t.device_id,
            patch_size: t.patch_size,
            banks: t.banks,
            slots: t.slots,
            pc_map_entries: t.pc_map_entries,
            addresses,
            bank: t.bank,
            system: t.system,
            patch: t.patch,
            labels: t.labels,
        })
    }

    /// The descriptions shipped, in their order.
    pub fn built_in() -> Vec<Description> {
        BUILT_IN.iter().filter_map(|(_, text)| Description::parse(text).ok()).collect()
    }

    /// Who the messages are for.
    pub fn head(&self) -> Head {
        Head { device_id: self.device_id, model_id: self.model_id }
    }

    /// How many patches the memory holds.
    pub fn patches(&self) -> usize {
        self.banks * self.slots
    }

    /// The name of a patch's memory: "01A" for the first.
    pub fn label(&self, number: usize) -> String {
        let (bank, slot) = (number / self.slots + 1, number % self.slots);
        format!("{bank:02}{}", (b'A' + slot as u8) as char)
    }

    /// The number of a patch's memory from its name ("12B"), or from a number written.
    pub fn number(&self, label: &str) -> Option<usize> {
        let label = label.trim().to_ascii_uppercase();
        if let Ok(n) = label.parse::<usize>() {
            return (n < self.patches()).then_some(n);
        }
        let (bank, slot) = label.split_at(label.len().checked_sub(1)?);
        let bank: usize = bank.parse().ok()?;
        let slot = (slot.as_bytes()[0].checked_sub(b'A')?) as usize;
        ((1..=self.banks).contains(&bank) && slot < self.slots).then(|| (bank - 1) * self.slots + slot)
    }

    /// Where a bank's own bytes are (its patches follow, a stride each).
    pub fn bank_address(&self, bank: usize) -> u32 {
        self.addresses.user + self.addresses.bank_stride * bank as u32
    }

    /// Where a patch's memory is.
    pub fn user_address(&self, number: usize) -> u32 {
        let (bank, slot) = (number / self.slots, number % self.slots);
        self.bank_address(bank) + self.addresses.patch_stride * (slot as u32 + 1)
    }

    /// Where a patch played is: the slot of the bank chosen.
    pub fn temporary_address(&self, slot: usize) -> u32 {
        self.addresses.temporary + self.addresses.patch_stride * (slot as u32 + 1)
    }

    /// The first parameter of the patch of a name.
    pub fn param(&self, name: &str) -> Option<&Param> {
        self.patch.iter().find(|p| p.name == name)
    }

    /// The parameter that names the patch.
    pub fn name_param(&self) -> &Param {
        self.patch.iter().find(|p| p.enc == Encoding::Ascii).expect("checked when parsed")
    }

    /// The name a patch's bytes hold.
    pub fn patch_name(&self, raw: &[u8]) -> String {
        match get(raw, self.name_param()) {
            Value::Text(name) => name.trim_end().to_owned(),
            Value::Number(_) => String::new(),
        }
    }

    /// A patch as it starts: every parameter at its first value.
    pub fn init_patch(&self) -> Vec<u8> {
        let mut raw = vec![0; self.patch_size];
        for p in &self.patch {
            let value = match &p.init {
                Init::Number(n) => Value::Number(*n),
                Init::Text(s) => Value::Text(s.clone()),
            };
            let _ = set(&mut raw, p, &value);
        }
        raw
    }

    /// The labels of a parameter's values, when they have names.
    pub fn labels_of(&self, param: &Param) -> Option<&[String]> {
        param.labels.as_deref().and_then(|list| self.labels.get(list)).map(Vec::as_slice)
    }

    /// A value as the device shows it: its label, or its number with the
    /// parameter's offset taken.
    pub fn shown(&self, param: &Param, value: &Value) -> String {
        match value {
            Value::Text(text) => text.clone(),
            Value::Number(n) => match self.labels_of(param).and_then(|labels| usize::try_from(n - param.min).ok().and_then(|i| labels.get(i))) {
                Some(label) => label.clone(),
                None => (n - param.offset).to_string(),
            },
        }
    }

    /// The system's bytes, as many as its parameters take.
    pub fn system_size(&self) -> usize {
        self.system.iter().map(|p| p.ofs + p.bytes).max().unwrap_or(0)
    }
}

/// The value a parameter holds in some bytes.
pub fn get(raw: &[u8], param: &Param) -> Value {
    let bytes = raw.get(param.ofs..param.ofs + param.bytes).unwrap_or(&[]);
    match param.enc {
        Encoding::Ascii => Value::Text(bytes.iter().map(|b| if (32..=126).contains(b) { *b as char } else { ' ' }).collect()),
        Encoding::Seven => Value::Number(i64::from(bytes.first().copied().unwrap_or(0) & 0x7F)),
        Encoding::Nibble => Value::Number(i64::from(roland::from_nibbles(bytes))),
    }
}

/// Writes a value into a parameter's bytes; out of its range, nothing is written.
pub fn set(raw: &mut [u8], param: &Param, value: &Value) -> Result<(), String> {
    let Some(bytes) = raw.get_mut(param.ofs..param.ofs + param.bytes) else { return Err(format!("{} is past the end", param.name)) };
    match (param.enc, value) {
        (Encoding::Ascii, Value::Text(text)) => {
            if !text.bytes().all(|b| i64::from(b) >= param.min && i64::from(b) <= param.max) {
                return Err(format!("{}: a sign the device cannot show", param.name));
            }
            for (i, slot) in bytes.iter_mut().enumerate() {
                *slot = text.as_bytes().get(i).copied().unwrap_or(b' ');
            }
            Ok(())
        }
        (Encoding::Seven | Encoding::Nibble, Value::Number(n)) => {
            if !(param.min..=param.max).contains(n) {
                return Err(format!("{}: {n} is out of {} to {}", param.name, param.min, param.max));
            }
            if param.enc == Encoding::Seven {
                bytes[0] = *n as u8;
            } else {
                bytes.copy_from_slice(&roland::nibbles(*n as u32, param.bytes));
            }
            Ok(())
        }
        _ => Err(format!("{}: a value of the wrong kind", param.name)),
    }
}

// ----- the file of a pedal ------------------------------------------------------------

/// What a pedal holds, saved by the program: its patches, its program change
/// map and its system, to be loaded and sent back.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
#[serde(default)]
pub struct PedalFile {
    /// The key of the device's description.
    pub device: String,
    /// The patches by the name of their memory ("01A"), each its name and
    /// its bytes in hexadecimal.
    pub patches: std::collections::BTreeMap<String, PatchEntry>,
    /// The memory each program change leads to, by the index of the entry.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pc_map: Vec<u16>,
    /// The system's bytes, in hexadecimal; empty when not read.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub system: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, Deserialize)]
pub struct PatchEntry {
    pub name: String,
    pub raw: String,
}

/// Bytes as hexadecimal pairs separated by spaces.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// Bytes read from hexadecimal pairs, separated or not.
pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace() && *c != ',').collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    (0..digits.len()).step_by(2).map(|i| u8::from_str_radix(digits.get(i..i + 2)?, 16).ok()).collect()
}

impl PedalFile {
    pub fn new(device: &str) -> Self {
        PedalFile { device: device.to_owned(), ..Default::default() }
    }

    /// Keeps a patch's bytes under its memory's name.
    pub fn put(&mut self, description: &Description, number: usize, raw: &[u8]) {
        self.patches.insert(description.label(number), PatchEntry { name: description.patch_name(raw), raw: hex(raw) });
    }

    /// A patch's bytes, when the file holds that memory and its bytes are whole.
    pub fn patch(&self, description: &Description, number: usize) -> Option<Vec<u8>> {
        let entry = self.patches.get(&description.label(number))?;
        from_hex(&entry.raw).filter(|raw| raw.len() == description.patch_size)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Reads a file of the program's, or a backup of the maker's editor
    /// (JSON too: a list of the patches, each its bytes in hexadecimal).
    pub fn from_json(text: &str, description: &Description) -> Result<PedalFile, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        if value.get("data").is_some_and(serde_json::Value::is_array) {
            return Self::from_editor_backup(&value, description);
        }
        let file: PedalFile = serde_json::from_value(value).map_err(|e| e.to_string())?;
        if !file.device.is_empty() && file.device != description.key {
            return Err(format!("a file of the {}, not of the {}", file.device, description.key));
        }
        for (label, entry) in &file.patches {
            let whole = description.number(label).is_some() && from_hex(&entry.raw).is_some_and(|raw| raw.len() == description.patch_size);
            if !whole {
                return Err(format!("the patch {label} is not {} bytes of the {}", description.patch_size, description.name));
            }
        }
        if file.pc_map.iter().any(|n| usize::from(*n) >= description.patches()) {
            return Err("the program change map names a memory that is not there".to_owned());
        }
        Ok(file)
    }

    /// The backup of the maker's editor: `data`, a list of entries with
    /// `patchNum` and `paramSet`, where the key that holds `PAT` is the
    /// patch's bytes, each a hexadecimal text.
    fn from_editor_backup(value: &serde_json::Value, description: &Description) -> Result<PedalFile, String> {
        let mut file = PedalFile::new(&description.key);
        let Some(entries) = value.get("data").and_then(serde_json::Value::as_array) else { return Err("no data".to_owned()) };
        for (i, entry) in entries.iter().enumerate().take(MOST_PATCHES) {
            let number = entry.get("patchNum").and_then(serde_json::Value::as_u64).map_or(i, |n| n as usize);
            let Some(set) = entry.get("paramSet").and_then(serde_json::Value::as_object) else { continue };
            let Some((_, bytes)) = set.iter().find(|(key, _)| key.contains("PAT")) else { continue };
            let Some(bytes) = bytes.as_array() else { continue };
            let raw: Option<Vec<u8>> = bytes.iter().map(|b| b.as_str().and_then(|s| u8::from_str_radix(s, 16).ok())).collect();
            match raw {
                Some(raw) if raw.len() == description.patch_size && number < description.patches() => file.put(description, number, &raw),
                _ => return Err(format!("the patch {} of the backup is not {} bytes of the {}", i + 1, description.patch_size, description.name)),
            }
        }
        if file.patches.is_empty() {
            return Err("no patch in the backup".to_owned());
        }
        Ok(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dd500() -> Description {
        Description::parse(BUILT_IN[0].1).unwrap()
    }

    #[test]
    fn every_description_shipped_is_read_and_says_its_version() {
        for (file, text) in BUILT_IN {
            let d = Description::parse(text).unwrap_or_else(|e| panic!("{file}: {e}"));
            assert_eq!(format_of(text), Some(FORMAT_VERSION), "{file}");
            assert_eq!((d.maker.as_str(), d.banks, d.slots, d.patches()), ("BOSS", 99, 3, 297), "{file}");
            assert!(d.document.starts_with("https://"), "{file} says where its document is");
            assert_eq!(d.user_address(0), 0x0601_0800, "{file}: the first memory, 30 04 10 00 on the wire");
            assert_eq!(d.temporary_address(1), 0x0600_1000, "{file}: the temporary patch B");
            assert!(d.patch.iter().all(|p| !p.group.is_empty()), "{file}: every patch parameter has its group");
        }
        assert_eq!(Description::built_in().len(), BUILT_IN.len());
        let (dd, rv) = (dd500(), Description::parse(BUILT_IN[1].1).unwrap());
        assert_eq!((dd.model_id, dd.patch_size, dd.patch.len()), ([0, 0, 0, 0x4D], 475, 338));
        assert_eq!((rv.model_id, rv.patch_size, rv.patch.len()), ([0, 0, 0, 0x42], 359, 232));
        assert_eq!(dd.user_address(295), 0x0601_0000 + 0x1_0000 * 98 + 0x1000, "99B");
        assert_eq!(dd.system_size(), 0x2C, "as the document says of the system common");
    }

    #[test]
    fn the_names_of_the_memories_and_their_numbers() {
        let d = dd500();
        assert_eq!((d.label(0), d.label(1), d.label(295), d.label(296)), ("01A".into(), "01B".into(), "99B".into(), "99C".into()));
        assert_eq!((d.number("01A"), d.number("99b"), d.number(" 12 "), d.number("99D"), d.number("00A"), d.number("297"), d.number("")), (Some(0), Some(295), Some(12), None, None, None, None));
    }

    #[test]
    fn a_patch_starts_as_the_document_says_and_its_values_are_read_shown_and_set() {
        let d = dd500();
        let mut raw = d.init_patch();
        assert_eq!(d.patch_name(&raw), "INIT");
        let (mode, time, tone, name) = (d.param("Delay MODE").unwrap(), d.param("Delay Time").unwrap(), d.param("Tone").unwrap(), d.name_param());
        assert_eq!((get(&raw, mode), get(&raw, time), get(&raw, tone)), (Value::Number(0), Value::Number(500), Value::Number(50)));
        assert_eq!(&raw[17..21], &[0, 1, 0xF, 4], "500 ms in nibbles");
        assert_eq!((d.shown(mode, &Value::Number(11)), d.shown(tone, &Value::Number(30)), d.shown(time, &Value::Number(440))), ("ANALOG".into(), "-20".into(), "440".into()));
        set(&mut raw, time, &Value::Number(440)).unwrap();
        set(&mut raw, mode, &Value::Number(11)).unwrap();
        set(&mut raw, name, &Value::Text("GILMOUR 440".into())).unwrap();
        assert_eq!((&raw[17..21], raw[16], d.patch_name(&raw)), (&[0u8, 1, 0xB, 8][..], 11, "GILMOUR 440".into()));
        assert_eq!(&raw[0..16], b"GILMOUR 440     ");
        assert!(set(&mut raw, time, &Value::Number(10001)).is_err() && set(&mut raw, mode, &Value::Text("x".into())).is_err() && set(&mut raw, name, &Value::Text("é".into())).is_err());
        assert_eq!(get(&raw, time), Value::Number(440), "refused, nothing is written");
        let labels = d.labels_of(mode).unwrap();
        assert_eq!((labels.len(), labels[10].as_str()), (12, "TAPE"));
    }

    #[test]
    fn a_description_that_cannot_be_used_is_refused() {
        let broken = |change: fn(&mut serde_json::Value)| {
            let mut value: serde_json::Value = serde_json::from_str(BUILT_IN[0].1).unwrap();
            change(&mut value);
            Description::parse(&value.to_string()).unwrap_err()
        };
        assert!(broken(|v| v["format_version"] = 9.into()).contains("newer"));
        assert!(broken(|v| v["patch_size"] = 400.into()).contains("past the end"));
        assert!(broken(|v| v["addresses"]["temporary"] = "30 00 00".into()).contains("address"));
        assert!(broken(|v| v["patch"][1]["labels"] = "Nowhere".into()).contains("Nowhere"));
        assert!(broken(|v| v["patch"][1]["max"] = 200.into()).contains("cannot hold"));
        assert!(broken(|v| v["addresses"]["patch_stride"] = "00 00 01 00".into()).contains("overlap"));
        assert!(Description::parse("{}").is_err());
    }

    #[test]
    fn a_pedal_file_keeps_the_patches_and_reads_the_editor_backup() {
        let d = dd500();
        let mut raw = d.init_patch();
        set(&mut raw, d.name_param(), &Value::Text("SLAPBACK".into())).unwrap();
        let mut file = PedalFile::new(&d.key);
        file.put(&d, 4, &raw);
        file.pc_map = vec![0, 4];
        file.system = hex(&[1, 2, 3]);
        let again = PedalFile::from_json(&file.to_json(), &d).unwrap();
        assert_eq!(again, file);
        assert_eq!(again.patches["02B"].name, "SLAPBACK");
        assert_eq!(again.patch(&d, 4), Some(raw.clone()));
        assert_eq!(again.patch(&d, 5), None);
        assert!(PedalFile::from_json(&file.to_json(), &Description::parse(BUILT_IN[1].1).unwrap()).unwrap_err().contains("dd500"));
        assert!(PedalFile::from_json("{\"patches\":{\"01A\":{\"name\":\"x\",\"raw\":\"00 01\"}}}", &d).unwrap_err().contains("475"));
        // The maker's editor writes its backup as a list of the patches, their bytes in hexadecimal texts.
        let bytes: Vec<String> = raw.iter().map(|b| format!("\"{b:02X}\"")).collect();
        let backup = format!("{{\"model\":\"Editor\",\"version\":\"1\",\"data\":[{{\"memo\":\"\",\"patchNum\":7,\"paramSet\":{{\"UserBank%BankBNK\":[\"01\"],\"UserBank%BankPAT(2)\":[{}]}}}}]}}", bytes.join(","));
        let read = PedalFile::from_json(&backup, &d).unwrap();
        assert_eq!((read.device.as_str(), read.patches.len(), read.patch(&d, 7)), ("dd500", 1, Some(raw)));
        assert_eq!(from_hex("F0 41, 7f"), Some(vec![0xF0, 0x41, 0x7F]));
        assert_eq!(from_hex("F0 4"), None);
    }
}
