//! The file dialogs, asked the same way everywhere: a label for the kind of
//! file and its extensions.

use std::path::{Path, PathBuf};

use crate::i18n::tr;

/// The extension of a project file.
pub const PROJECT: &[&str] = &["ogproj"];

/// Asks for a file of the kind named.
pub fn open_file(label: &str, extensions: &[&str]) -> Option<PathBuf> {
    rfd::FileDialog::new().add_filter(label, extensions).pick_file()
}

/// Asks for a file of the kind named, the dialog opened in `inside` when
/// that folder exists.
pub fn open_file_in(label: &str, extensions: &[&str], inside: &Path) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().add_filter(label, extensions);
    if inside.is_dir() {
        dialog = dialog.set_directory(inside);
    }
    dialog.pick_file()
}

/// Asks for one or more files of the kind named.
pub fn open_files(label: &str, extensions: &[&str]) -> Option<Vec<PathBuf>> {
    rfd::FileDialog::new().add_filter(label, extensions).pick_files()
}

/// Asks where to write a file of the kind named, `name` offered.
pub fn save_file(label: &str, extensions: &[&str], name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().add_filter(label, extensions).set_file_name(name).save_file()
}

/// Asks where to write a file of the kind named, `name` offered, the dialog
/// opened in `inside` when that folder exists.
pub fn save_file_in(label: &str, extensions: &[&str], name: &str, inside: &Path) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new().add_filter(label, extensions).set_file_name(name);
    if inside.is_dir() {
        dialog = dialog.set_directory(inside);
    }
    dialog.save_file()
}

/// Asks for a folder, the dialog opened in `inside` when it is given.
pub fn choose_folder(title: Option<&str>, inside: Option<&Path>) -> Option<PathBuf> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(title) = title {
        dialog = dialog.set_title(title);
    }
    if let Some(inside) = inside {
        dialog = dialog.set_directory(inside);
    }
    dialog.pick_folder()
}

/// Asks for a project file, the dialog opened in the projects folder.
pub fn project_file(inside: &Path) -> Option<PathBuf> {
    open_file_in(tr("OpenGarageMusic project"), PROJECT, inside)
}

/// Asks where to write a project file, the dialog opened in the projects folder.
pub fn save_project(name: &str, inside: &Path) -> Option<PathBuf> {
    save_file_in(tr("OpenGarageMusic project"), PROJECT, &format!("{name}.ogproj"), inside)
}

/// Asks for an audio file, of the formats the program reads.
pub fn audio_file() -> Option<PathBuf> {
    open_file(tr("Audio (WAV, MP3, M4A, AAC, OGG, FLAC)"), &og_engine::codec::AUDIO_EXTENSIONS)
}

/// Asks for audio files, of the formats the program reads.
pub fn audio_files() -> Option<Vec<PathBuf>> {
    open_files(tr("Audio (WAV, MP3, M4A, AAC, OGG, FLAC)"), &og_engine::codec::AUDIO_EXTENSIONS)
}
