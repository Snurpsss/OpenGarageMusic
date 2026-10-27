//! The projects the user worked on, kept as sessions without being asked:
//! each project has a folder of its own (by the id it carries), written when
//! the program quits, before another project takes its place, and every two
//! minutes while it changes. The last ones are kept, as many as the user
//! chose (5 to 100), in a folder of their choosing; the latest one comes
//! back at the next start, and File → "Previous sessions…" reopens any.

use std::path::{Path, PathBuf};

use eframe::egui;
use og_engine::Project;
use serde::{Deserialize, Serialize};

use crate::app::{App, BottomTab, Dialog, RightPanel};
use crate::files;
use crate::i18n::tr;

/// How often a project that changed is kept again, in seconds.
const EVERY: f64 = 120.0;
/// How many projects are kept until the user says otherwise, and how many they may choose.
pub const KEPT_DEFAULT: u32 = 20;
pub const KEPT_RANGE: std::ops::RangeInclusive<u32> = 5..=100;
const SESSION_FILE: &str = "session.json";
const ABOUT_FILE: &str = "about.json";

/// What is kept of the program from one run to the next: the project as it
/// was, saved or not, and how the window was arranged.
#[derive(Serialize, Deserialize)]
pub(crate) struct Session {
    /// The project, as the text of a project file.
    project: String,
    /// The project file it came from, if any.
    path: Option<String>,
    /// It held changes its file did not have.
    modified: bool,
    selected_track: usize,
    controls: bool,
    show_library: bool,
    show_bottom: bool,
    loops: bool,
    notes: bool,
    show_global: bool,
    show_master: bool,
    ppb: f32,
    scroll: f64,
    position: f64,
    /// Height of the panel under the tracks, and whether it takes the whole window.
    #[serde(default)]
    bottom_height: f32,
    #[serde(default)]
    bottom_full: bool,
}

/// What the list of sessions shows of one, written beside it so that the
/// list reads quickly.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct About {
    pub name: String,
    pub artist: String,
    pub title: String,
    pub tracks: usize,
    /// How long the song lasts.
    pub seconds: f64,
    /// The project file, when there is one.
    pub path: Option<String>,
    /// When it was kept, in seconds since 1970.
    pub kept: u64,
}

impl About {
    fn of(project: &Project, path: Option<&Path>) -> About {
        About {
            name: project.name.clone(),
            artist: project.artist.clone(),
            title: project.title.clone(),
            tracks: project.tracks.len(),
            seconds: project.beat_to_sec(project.end_beat()).max(0.0),
            path: path.map(|p| p.to_string_lossy().into_owned()),
            kept: now(),
        }
    }

    /// The song: "Artist – Title" when they are known, else the name of the project.
    pub fn song(&self) -> String {
        match (self.artist.trim(), self.title.trim()) {
            ("", "") => self.name.clone(),
            ("", title) => title.to_owned(),
            (artist, "") => format!("{artist} – {}", self.name),
            (artist, title) => format!("{artist} – {title}"),
        }
    }
}

/// A session kept: its folder, what it holds, and the room it takes.
#[derive(Clone, Debug)]
pub struct Entry {
    pub dir: PathBuf,
    pub about: About,
    pub bytes: u64,
}

/// The time, in seconds since the epoch.
pub fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// A new id: the time to the nanosecond, and a count, so that two are never alike.
pub fn new_id() -> String {
    static COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    format!("{nanos:x}-{:x}", COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
}

/// The id of a project that comes from a file without one: from its path,
/// so that opening the file again finds the same session.
pub fn id_of_file(path: &Path) -> String {
    format!("file-{:016x}", hash64(&path.to_string_lossy().to_lowercase()))
}

/// A hash of a text (FNV-1a), the same from one version of the program to
/// the next: enough to tell paths and names apart.
pub fn hash64(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3))
}

/// The room the files of a folder take, its folders included.
fn size_of(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries
        .flatten()
        .map(|entry| match entry.metadata() {
            Ok(meta) if meta.is_dir() => size_of(&entry.path()),
            Ok(meta) => meta.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Copies a folder and what it holds.
fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)?.flatten() {
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// The sessions of a folder, the latest first.
pub fn list(folder: &Path) -> Vec<Entry> {
    let Ok(entries) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut found: Vec<Entry> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|dir| dir.join(SESSION_FILE).is_file())
        .map(|dir| {
            let about = std::fs::read_to_string(dir.join(ABOUT_FILE)).ok().and_then(|text| serde_json::from_str::<About>(&text).ok()).unwrap_or_else(|| {
                // Written before the list was: read from the project itself.
                let session = std::fs::read_to_string(dir.join(SESSION_FILE)).ok().and_then(|text| serde_json::from_str::<Session>(&text).ok());
                let kept = std::fs::metadata(dir.join(SESSION_FILE)).ok().and_then(|m| m.modified().ok()).and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs());
                match session.and_then(|s| Project::from_json(&s.project).ok().map(|p| (p, s.path))) {
                    Some((project, path)) => About { kept, ..About::of(&project, path.as_deref().map(Path::new)) },
                    None => About { name: dir.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()), kept, ..About::default() },
                }
            });
            Entry { bytes: size_of(&dir), dir, about }
        })
        .collect();
    found.sort_by_key(|e| std::cmp::Reverse(e.about.kept));
    found
}

/// A size in kilobytes, megabytes or gigabytes.
pub fn size_text(bytes: u64) -> String {
    let bytes = bytes as f64;
    if bytes >= 1e9 {
        format!("{:.1} GB", bytes / 1e9)
    } else if bytes >= 1e6 {
        format!("{:.1} MB", bytes / 1e6)
    } else {
        format!("{:.0} kB", (bytes / 1e3).ceil())
    }
}

/// When a session was kept: minutes or hours ago, else the date.
pub fn when_text(kept: u64, now: u64) -> String {
    let ago = now.saturating_sub(kept);
    if ago < 60 {
        tr("just now").to_owned()
    } else if ago < 3600 {
        tf!("{} min ago", ago / 60)
    } else if ago < 86_400 {
        tf!("{} h ago", ago / 3600)
    } else {
        // The date, from the days since 1970 (the civil calendar of Howard Hinnant).
        let days = (kept / 86_400) as i64 + 719_468;
        let era = days.div_euclid(146_097);
        let day_of_era = days - era * 146_097;
        let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_index = (5 * day_of_year + 2) / 153;
        let day = day_of_year - (153 * month_index + 2) / 5 + 1;
        let month = if month_index < 10 { month_index + 3 } else { month_index - 9 };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        format!("{day:02}/{month:02}/{year}")
    }
}

/// The line of a session in its window: the song, its tracks and length,
/// when it was kept, and its file.
pub fn line_of(about: &About, now: u64) -> String {
    let seconds = about.seconds.round() as u64;
    let file = about.path.clone().unwrap_or_else(|| tr("never saved in a file").to_owned());
    format!("{}   ·   {}   ·   {}:{:02}   ·   {}   ·   {file}", about.song(), tf!("{} tracks", about.tracks), seconds / 60, seconds % 60, when_text(about.kept, now))
}

impl App {
    /// The folder of the sessions: the user's choice, else OpenGarageMusic\Sessions
    /// in their own folder (in the program's data folder for the tests and the
    /// demonstration).
    pub fn sessions_dir(&self) -> PathBuf {
        if !self.settings.sessions_folder.trim().is_empty() {
            return PathBuf::from(self.settings.sessions_folder.trim());
        }
        match (self.settings_path.is_some(), std::env::var_os("USERPROFILE")) {
            (true, Some(home)) => PathBuf::from(home).join("OpenGarageMusic").join("Sessions"),
            _ => self.data_dir.join("Sessions"),
        }
    }

    /// How many projects are kept as sessions.
    pub fn sessions_kept(&self) -> usize {
        match self.settings.sessions_kept {
            0 => KEPT_DEFAULT as usize,
            n => n.clamp(*KEPT_RANGE.start(), *KEPT_RANGE.end()) as usize,
        }
    }

    /// The folder of the session of the project open now.
    pub(crate) fn session_dir(&self) -> PathBuf {
        self.sessions_dir().join(&self.project.id)
    }

    /// Whether the project holds changes that are not in a file.
    pub fn is_modified(&self) -> bool {
        self.clean.as_ref() != Some(&self.project)
    }

    /// Whether the project is worth keeping: any but a blank one untouched.
    fn worth_keeping(&self) -> bool {
        !self.blank || self.path.is_some() || self.is_modified()
    }

    /// Gives a project the id that names its session, when it has none.
    pub fn give_id(project: &mut Project, path: Option<&Path>) {
        if project.id.trim().is_empty() {
            project.id = path.map_or_else(new_id, id_of_file);
        }
    }

    /// Gives the project open now an id, when it has none. The id is no
    /// change of the project: it is neither an undo step nor a modification.
    pub(crate) fn give_project_id(&mut self, path: Option<&Path>) {
        if !self.project.id.trim().is_empty() {
            return;
        }
        Self::give_id(&mut self.project, path);
        let id = self.project.id.clone();
        for project in self.clean.iter_mut().chain(self.kept.iter_mut()).chain(std::iter::once(&mut self.synced)).chain(self.undo.iter_mut()).chain(self.redo.iter_mut()) {
            project.id.clone_from(&id);
        }
    }

    /// Chooses whether the program comes back as it was left at the next start.
    pub fn set_keep_session(&mut self, keep: bool) {
        self.settings.forget_session = !keep;
        self.save_settings();
    }

    /// Writes down the project as it is, saved or not, and the arrangement of
    /// the window, in its session. Recorded takes that only exist in the
    /// temporary folder are copied along. The oldest sessions beyond those
    /// kept go.
    pub fn write_session(&mut self) -> Result<(), String> {
        let path = self.path.clone();
        self.give_project_id(path.as_deref());
        let modified = self.is_modified();
        self.capture_plugin_states(false);
        let dir = self.session_dir();
        let media = dir.join("media");
        std::fs::create_dir_all(&media).map_err(|e| e.to_string())?;
        let absolute = |path: &str| self.absolute_path(path);
        let temp = std::env::temp_dir().join("opengarage");
        let mut project = self.project.clone();
        let mut kept = Vec::new();
        for clip in &mut project.clips {
            let mut source = absolute(&clip.path);
            if let (true, Some(file)) = (source.starts_with(&temp), source.file_name()) {
                let copy = media.join(file);
                if std::fs::copy(&source, &copy).is_ok() {
                    source = copy;
                }
            }
            if source.starts_with(&media) {
                kept.push(source.clone());
            }
            clip.path = source.to_string_lossy().into_owned();
        }
        for font in &mut project.fonts {
            font.path = absolute(&font.path).to_string_lossy().into_owned();
        }
        let mut effect_files = vec![&mut project.reverb_impulse];
        for track in &mut project.tracks {
            effect_files.push(&mut track.fx.capture);
            effect_files.push(&mut track.fx.cabinet);
        }
        for path in effect_files.into_iter().filter(|p| !p.is_empty()) {
            *path = absolute(path).to_string_lossy().into_owned();
        }
        // Takes of an earlier session that this project does not use go.
        if let Ok(entries) = std::fs::read_dir(&media) {
            for file in entries.flatten().map(|e| e.path()).filter(|file| !kept.contains(file)) {
                let _ = std::fs::remove_file(file);
            }
        }
        let session = Session {
            project: project.to_json().map_err(|e| e.to_string())?,
            path: self.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            modified,
            selected_track: self.selected_track,
            controls: self.bottom_tab == BottomTab::Controls,
            show_library: self.show_library,
            show_bottom: self.show_bottom,
            loops: self.right_panel == RightPanel::Loops,
            notes: self.right_panel == RightPanel::Notes,
            show_global: self.show_global,
            show_master: self.show_master,
            ppb: self.ppb,
            scroll: self.scroll,
            position: self.position,
            bottom_height: self.bottom_height,
            bottom_full: self.bottom_full,
        };
        let text = serde_json::to_string(&session).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(SESSION_FILE), text).map_err(|e| e.to_string())?;
        let about = serde_json::to_string_pretty(&About::of(&self.project, self.path.as_deref())).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(ABOUT_FILE), about).map_err(|e| e.to_string())?;
        self.kept = Some(self.project.clone());
        self.prune_sessions();
        Ok(())
    }

    /// Takes away the oldest sessions beyond those kept, never the one open now.
    fn prune_sessions(&self) {
        let current = self.session_dir();
        for old in list(&self.sessions_dir()).into_iter().filter(|e| e.dir != current).skip(self.sessions_kept().saturating_sub(1)) {
            let _ = std::fs::remove_dir_all(old.dir);
        }
    }

    /// Keeps the project open now, if it is worth it, before another takes
    /// its place; says so when it cannot.
    pub fn keep_session(&mut self) {
        if !self.keep_sessions || !self.worth_keeping() || self.kept.as_ref() == Some(&self.project) {
            return;
        }
        if let Err(e) = self.write_session() {
            self.status = tf!("The session was not kept: {e}" [e]);
        }
    }

    /// Keeps the project again every two minutes while it changes.
    pub fn keep_session_now_and_then(&mut self, now: f64) {
        if now - self.kept_at >= EVERY {
            self.kept_at = now;
            self.keep_session();
        }
    }

    /// Brings back the latest session, or that of an earlier version of the
    /// program (one session in its data folder). Returns whether there was one.
    pub fn restore_session(&mut self) -> bool {
        if list(&self.sessions_dir()).into_iter().any(|e| self.restore_session_from(&e.dir)) {
            return true;
        }
        let old = self.data_dir.join("Session");
        old.join(SESSION_FILE).is_file() && self.restore_session_from(&old)
    }

    /// Brings back the session written in this folder. Returns whether there was one.
    pub fn restore_session_from(&mut self, dir: &Path) -> bool {
        let Some(session) = std::fs::read_to_string(dir.join(SESSION_FILE)).ok().and_then(|text| serde_json::from_str::<Session>(&text).ok()) else {
            return false;
        };
        let Ok(project) = Project::from_json(&session.project) else { return false };
        self.replace_project(project, session.path.map(PathBuf::from));
        let missing = self.load_clips();
        // What its file did not have is still not in a file.
        self.clean = if session.modified { None } else { Some(self.project.clone()) };
        self.kept = Some(self.project.clone());
        self.selected_track = session.selected_track.min(self.project.tracks.len().saturating_sub(1));
        self.bottom_tab = if session.controls { BottomTab::Controls } else { BottomTab::Editor };
        self.show_library = session.show_library;
        self.show_bottom = session.show_bottom;
        self.right_panel = match (session.loops, session.notes) {
            (true, _) => RightPanel::Loops,
            (_, true) => RightPanel::Notes,
            _ => RightPanel::None,
        };
        self.show_global = session.show_global;
        self.show_master = session.show_master;
        self.bottom_full = session.bottom_full;
        if session.bottom_height.is_finite() && session.bottom_height >= 100.0 {
            self.bottom_height = session.bottom_height;
        }
        if session.ppb.is_finite() && session.scroll.is_finite() && session.position.is_finite() {
            self.ppb = session.ppb.clamp(6.0, 160.0);
            self.scroll = session.scroll.max(0.0);
            self.go_to(session.position);
        }
        self.status = if missing.is_empty() {
            tr("The program is as you left it.").to_owned()
        } else {
            tf!("The program is as you left it, but {} audio file(s) cannot be found: {}", missing.len(), missing.join(", "))
        };
        true
    }

    /// The window is asked to close. The project is kept as a session and the
    /// window closes; only when it cannot be kept is the user offered to save it.
    pub fn closing(&mut self) -> bool {
        if self.quitting {
            return true;
        }
        if self.keep_sessions && self.worth_keeping() {
            if let Err(e) = self.write_session() {
                self.status = tf!("The session was not kept: {e}" [e]);
                if self.is_modified() {
                    self.dialog = Some(Dialog::Quit);
                    return false;
                }
            }
        } else if !self.keep_sessions && self.is_modified() {
            self.dialog = Some(Dialog::Quit);
            return false;
        }
        self.quitting = true;
        true
    }

    /// Opens the window of the previous sessions, read anew.
    pub fn open_sessions(&mut self) {
        self.keep_session();
        self.session_list = list(&self.sessions_dir());
        self.session_message.clear();
        self.dialog = Some(Dialog::Sessions);
    }

    /// Reopens a session; the project open now is kept first.
    pub fn reopen_session(&mut self, dir: &Path) {
        self.session_message.clear();
        self.keep_session();
        if self.restore_session_from(dir) {
            self.dialog = None;
        } else {
            self.session_message = tr("This session cannot be read.").to_owned();
        }
    }

    /// Deletes a session for good.
    pub fn delete_session(&mut self, dir: &Path) {
        if let Err(e) = std::fs::remove_dir_all(dir) {
            self.session_message = tf!("The session was not deleted: {e}" [e]);
        }
        if dir == self.session_dir() {
            self.kept = None;
        }
        self.session_list = list(&self.sessions_dir());
    }

    /// Puts the sessions in another folder, from now on, and moves those kept there.
    pub fn move_sessions(&mut self, to: &Path) {
        let from = self.sessions_dir();
        if from == to {
            return;
        }
        let mut failed = 0;
        for entry in list(&from) {
            let Some(name) = entry.dir.file_name() else { continue };
            let target = to.join(name);
            let moved = std::fs::create_dir_all(to).is_ok() && (std::fs::rename(&entry.dir, &target).is_ok() || (copy_dir(&entry.dir, &target).is_ok() && std::fs::remove_dir_all(&entry.dir).is_ok()));
            if !moved {
                failed += 1;
            }
        }
        self.settings.sessions_folder = to.to_string_lossy().into_owned();
        self.save_settings();
        self.session_list = list(&self.sessions_dir());
        self.session_message = if failed == 0 { tf!("The sessions are now kept in {}.", to.display()) } else { tf!("{failed} sessions could not be moved to {}." [failed], to.display()) };
    }

    /// The window of the previous sessions: one line each, the latest first.
    pub fn sessions_window(&mut self, ui: &mut egui::Ui) -> bool {
        let now = now();
        let current = self.session_dir();
        let total: u64 = self.session_list.iter().map(|e| e.bytes).sum();
        ui.horizontal(|ui| {
            ui.label(tf!("{} sessions, {} in all.", self.session_list.len(), size_text(total)));
            ui.separator();
            ui.label(tr("Keep the last"));
            let mut kept = self.sessions_kept() as u32;
            if ui.add(egui::DragValue::new(&mut kept).range(KEPT_RANGE)).on_hover_text(tr("Between 5 and 100; the oldest ones go")).changed() {
                self.settings.sessions_kept = kept;
                self.save_settings();
            }
            ui.label(tr("projects"));
        });
        let mut action: Option<(bool, PathBuf)> = None;
        let mut reopened = false;
        egui::ScrollArea::vertical().id_salt("sessions").max_height((ui.available_height() - 80.0).max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
            if self.session_list.is_empty() {
                ui.weak(tr("No session yet: the projects you work on are kept here as you go."));
            }
            for entry in &self.session_list {
                ui.horizontal(|ui| {
                    if ui.small_button(tr("Reopen")).clicked() {
                        action = Some((true, entry.dir.clone()));
                    }
                    if ui.small_button(tr("Delete")).clicked() {
                        action = Some((false, entry.dir.clone()));
                    }
                    let mut text = line_of(&entry.about, now);
                    if entry.dir == current {
                        text.push_str(&format!("   ({})", tr("open now")));
                    }
                    let line = ui.add(egui::Label::new(text).sense(egui::Sense::click())).on_hover_text(tf!("Double-click to reopen it · {}", size_text(entry.bytes)));
                    if line.double_clicked() {
                        action = Some((true, entry.dir.clone()));
                    }
                });
            }
        });
        match action {
            Some((true, dir)) => {
                self.reopen_session(&dir);
                reopened = self.session_message.is_empty();
            }
            Some((false, dir)) => self.delete_session(&dir),
            None => {}
        }
        ui.separator();
        let folder = self.sessions_dir();
        ui.horizontal(|ui| {
            ui.label(tf!("Kept in {}", folder.display()));
            if ui.small_button(tr("Open the folder")).clicked() {
                let _ = std::fs::create_dir_all(&folder);
                self.open_external(&folder.to_string_lossy());
            }
            if ui.small_button(tr("Change the folder…")).clicked() {
                if let Some(to) = files::choose_folder(None, Some(&folder)) {
                    self.move_sessions(&to);
                }
            }
        });
        if !self.session_message.is_empty() {
            ui.label(&self.session_message);
        }
        reopened
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_lines_sizes_and_dates() {
        assert_ne!(new_id(), new_id());
        let a = id_of_file(Path::new(r"C:\Musique\Morceau.ogproj"));
        assert_eq!(a, id_of_file(Path::new(r"c:\musique\MORCEAU.ogproj")), "the same file, whatever the case");
        assert_ne!(a, id_of_file(Path::new(r"C:\Musique\Autre.ogproj")));
        let about = About { name: "Projet".into(), artist: "Metallica".into(), title: "Nothing Else Matters".into(), tracks: 4, seconds: 388.4, path: Some(r"D:\nem.ogproj".into()), kept: 1_000_000 };
        let line = line_of(&about, 1_000_000 + 7200);
        assert!(line.starts_with("Metallica – Nothing Else Matters") && line.contains("6:28") && line.contains(r"D:\nem.ogproj"), "{line}");
        assert_eq!(About { artist: String::new(), title: String::new(), ..about.clone() }.song(), "Projet");
        assert!(line_of(&About { path: None, ..about.clone() }, 1_000_000).contains(tr("never saved in a file")));
        assert_eq!(when_text(1_760_486_400, 1_760_486_400 + 3 * 86_400), "15/10/2025");
        assert_eq!(when_text(951_782_400, 951_782_400 + 90_000), "29/02/2000", "a leap day");
        assert_eq!((size_text(1500), size_text(2_500_000), size_text(3_200_000_000)), ("2 kB".to_owned(), "2.5 MB".to_owned(), "3.2 GB".to_owned()));
    }
}
