//! The module of The GigRig G3S pedal switcher in the MIDI Pilot: a backup
//! file of the switcher (`.g3b`) loaded, its banks, presets (loops, order,
//! name, MIDI lines) and names edited, and written back for the maker's
//! editor to restore. Nothing goes to the switcher from here. Its page is
//! in `ui_g3s`.

use std::path::{Path, PathBuf};

use og_engine::g3s::{self, Backup, BANK_NAME};
use og_engine::setlist::SetSong;

use crate::app::App;
use crate::files;
use crate::i18n::tr;

/// The module as its page shows it.
pub struct G3sUi {
    /// The device of the chain it is for.
    pub device: usize,
    pub backup: Backup,
    pub path: Option<PathBuf>,
    /// The bank and the preset chosen.
    pub bank: usize,
    pub preset: usize,
    /// The 14 hidden presets of a bank are shown too.
    pub show_hidden: bool,
    /// Changed since the file was read or written.
    pub edited: bool,
    pub message: String,
    /// The pedalboard (the pedals in the loops) is shown instead of the presets.
    pub board: bool,
}

/// A name on the switcher's two lines of ten: the words of a text, in
/// capitals, packed on the first line then the second, `|` between.
pub fn two_lines(text: &str) -> String {
    const LINE: usize = BANK_NAME / 2;
    let clean: String = text.chars().filter(|c| c.is_ascii() && !c.is_control()).collect::<String>().to_ascii_uppercase();
    let (mut first, mut second) = (String::new(), String::new());
    for word in clean.split_whitespace() {
        let line = if second.is_empty() && (first.is_empty() || first.len() + 1 + word.len() <= LINE) { &mut first } else { &mut second };
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    first.truncate(LINE);
    second.truncate(LINE);
    if second.trim().is_empty() {
        first.trim().to_owned()
    } else {
        format!("{}|{}", first.trim(), second.trim())
    }
}

impl App {
    /// Opens the module for a device of the chain, on a backup made up
    /// until one is loaded.
    pub fn open_g3s(&mut self, device: usize) {
        self.setlist.g3s = Some(G3sUi { device, backup: g3s::blank(1), path: None, bank: 0, preset: 0, show_hidden: false, edited: false, board: false, message: tr("Load a backup of your switcher (made with its editor), or start from the one bank here.").to_owned() });
        self.setlist.page = crate::setlist::Page::G3s;
    }

    pub fn close_g3s(&mut self) {
        self.setlist.g3s = None;
        self.setlist.page = crate::setlist::Page::Devices;
    }

    /// Loads a backup file of the switcher.
    pub fn load_g3s_file(&mut self) {
        let Some(path) = files::open_file(tr("G3S backup"), &["g3b", "rtx"]) else { return };
        match std::fs::read(&path) {
            Ok(bytes) => self.read_g3s_bytes(&bytes, Some(path)),
            Err(e) => {
                if let Some(ui) = &mut self.setlist.g3s {
                    ui.message = tf!("The file was not read: {e}" [e]);
                }
            }
        }
    }

    pub fn read_g3s_bytes(&mut self, bytes: &[u8], path: Option<PathBuf>) {
        let Some(ui) = &mut self.setlist.g3s else { return };
        match Backup::parse(bytes) {
            Ok(backup) => {
                let banks = backup.banks.len();
                ui.backup = backup;
                ui.path = path;
                ui.bank = 0;
                ui.preset = 0;
                ui.edited = false;
                ui.message = tf!("{banks} banks read." [banks]);
            }
            Err(e) => ui.message = tf!("The file is not a backup of the switcher: {e}" [e]),
        }
    }

    /// Writes the backup as a file the switcher's editor restores.
    pub fn save_g3s_file(&mut self) {
        let Some(ui) = &self.setlist.g3s else { return };
        let name = ui.path.as_ref().and_then(|p| p.file_name()).map_or_else(|| "G3S backup.g3b".to_owned(), |n| n.to_string_lossy().into_owned());
        let Some(path) = files::save_file(tr("G3S backup"), &["g3b"], &name) else { return };
        self.write_g3s_file(&path);
    }

    pub fn write_g3s_file(&mut self, path: &Path) {
        let Some(ui) = &mut self.setlist.g3s else { return };
        let bytes = ui.backup.write();
        ui.message = match Backup::check(&bytes).and_then(|checked| std::fs::write(path, &bytes).map(|()| checked).map_err(|e| e.to_string())) {
            Ok(checked) => {
                ui.path = Some(path.to_path_buf());
                ui.edited = false;
                tf!("Written in {} ({checked}): restore it in the switcher with its editor." [checked], path.display())
            }
            Err(e) => tf!("The file was not written: {e}" [e]),
        };
    }

    /// Checks what would be written, as the switcher does.
    pub fn check_g3s(&mut self) {
        let Some(ui) = &mut self.setlist.g3s else { return };
        ui.message = match Backup::check(&ui.backup.write()) {
            Ok(checked) => tf!("The backup is sound: {checked}." [checked]),
            Err(e) => tf!("The backup is not sound: {e}" [e]),
        };
    }

    /// One bank a song of the list, after the first bank (kept for the
    /// stompbox mode): banks added as needed, named after the songs.
    pub fn g3s_banks_from_songs(&mut self) {
        let songs: Vec<SetSong> = self.settings.set_list.songs.clone();
        let Some(ui) = &mut self.setlist.g3s else { return };
        let mut added = 0;
        for (i, song) in songs.iter().enumerate() {
            let bank = i + 1;
            while ui.backup.banks.len() <= bank {
                if ui.backup.add_bank().is_none() {
                    ui.message = tf!("{added} banks added; the switcher holds {} at most.", g3s::MOST_BANKS);
                    ui.edited = true;
                    return;
                }
                added += 1;
            }
            let name = if song.display.trim().is_empty() { song.title.clone() } else { song.display.clone() };
            ui.backup.set_bank_name(bank, &two_lines(&name));
        }
        ui.edited = true;
        ui.message = tf!("{} banks named after the songs, {added} added." [added], songs.len());
    }

    /// The songs of the list from the banks' names: a song for each bank
    /// named, after the first, not in the list yet.
    pub fn g3s_songs_from_banks(&mut self) {
        let Some(ui) = &self.setlist.g3s else { return };
        let names: Vec<(usize, String)> = (1..ui.backup.banks.len()).map(|bank| (bank, ui.backup.bank_name(bank).replace('|', " ").trim().to_owned())).filter(|(_, name)| !name.is_empty()).collect();
        let mut added = 0;
        for (bank, name) in names {
            let known = self.settings.set_list.songs.iter().any(|song| og_engine::setlist::same(&song.title, &name) || og_engine::setlist::same(&song.display, &name));
            if !known {
                self.settings.set_list.songs.push(SetSong { number: bank as u8 + 1, title: name, ..SetSong::default() });
                added += 1;
            }
        }
        self.save_settings();
        if let Some(ui) = &mut self.setlist.g3s {
            ui.message = tf!("{added} songs added to the list from the banks." [added]);
        }
    }
}
