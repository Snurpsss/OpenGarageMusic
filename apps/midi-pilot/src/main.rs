//! MidiPilot.exe: the MIDI Pilot of OpenGarageMusic on its own, for the
//! stage. The window is the program's: this launcher only starts
//! `OpenGarageMusic.exe`, next to it, with `--midi-pilot`, which shows that
//! window alone, with no sound card and no project.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

/// The program next to the launcher, whatever the case of its name.
fn program() -> Option<PathBuf> {
    let folder = std::env::current_exe().ok()?.parent()?.to_path_buf();
    ["OpenGarageMusic.exe", "opengaragemusic.exe", "opengaragemusic"].iter().map(|name| folder.join(name)).find(|p| p.is_file())
}

fn main() {
    let Some(program) = program() else {
        eprintln!("OpenGarageMusic.exe was not found next to MidiPilot.exe");
        std::process::exit(1);
    };
    if let Err(e) = std::process::Command::new(&program).arg("--midi-pilot").spawn() {
        eprintln!("{} could not be started: {e}", program.display());
        std::process::exit(1);
    }
}
