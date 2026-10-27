//! The AI scripts of the MIDI Pilot (`og_engine::script`): a file written by
//! an AI, reviewed in a page, applied action by action through the pedal
//! module and the G3S module; the exports that write the same format; and
//! the guide that teaches the format, written on request.

use std::path::{Path, PathBuf};

use eframe::egui;
use og_engine::script::{self, Action, Kind, Script};

use crate::app::App;
use crate::files;
use crate::i18n::tr;
use crate::pedal::Job;

/// Where an action being applied stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Stage {
    /// Nothing asked of the pedal yet.
    Start,
    /// What the memory, the map or the system holds is being read first.
    Reading,
    /// The pedal is being written.
    Writing,
}

/// A script as the page shows it, and as it is applied.
pub struct ScriptUi {
    pub script: Script,
    pub path: Option<PathBuf>,
    /// For each action: its sentence, and what is wrong with it before anything is done.
    pub summaries: Vec<(String, Option<String>)>,
    /// For each action, once applied: what came of it.
    pub results: Vec<Option<Result<String, String>>>,
    /// The action being applied.
    pub running: Option<usize>,
    stage: Stage,
    pub message: String,
    ctx: egui::Context,
}

impl ScriptUi {
    pub fn errors(&self) -> usize {
        self.summaries.iter().filter(|(_, e)| e.is_some()).count()
    }
}

/// The guide for an AI, as the program writes it.
pub fn guide_text() -> String {
    script::guide(&crate::pedal::descriptions(), env!("CARGO_PKG_VERSION"))
}

impl App {
    /// Opens a script file of an AI's for review.
    pub fn load_ai_script(&mut self, ctx: &egui::Context) {
        let Some(path) = files::open_file(tr("AI script"), &["json"]) else { return };
        match std::fs::read_to_string(&path) {
            Ok(text) => self.review_ai_script(&text, Some(path), ctx),
            Err(e) => self.setlist.message = tf!("The file was not read: {e}" [e]),
        }
    }

    /// Reads a script and shows what it would do, each action checked
    /// against the descriptions and the chain, nothing done yet.
    pub fn review_ai_script(&mut self, text: &str, path: Option<PathBuf>, ctx: &egui::Context) {
        let script = match Script::parse(text) {
            Ok(script) => script,
            Err(e) => {
                self.setlist.message = tf!("The script was not read: {e}" [e]);
                return;
            }
        };
        let mut backup = self.setlist.g3s.as_ref().map_or_else(|| og_engine::g3s::blank(1), |g| g.backup.clone());
        let mut list = self.settings.set_list.clone();
        let summaries: Vec<(String, Option<String>)> = script.actions.iter().map(|action| self.check_action(action, &mut backup, &mut list)).collect();
        let count = summaries.len();
        let errors = summaries.iter().filter(|(_, e)| e.is_some()).count();
        self.setlist.script = Some(ScriptUi { results: vec![None; count], script, path, summaries, running: None, stage: Stage::Start, message: tf!("{count} actions, {errors} with errors." [count, errors]), ctx: ctx.clone() });
        self.setlist.page = crate::setlist::Page::Script;
    }

    /// An action's sentence, and what is wrong with it: a pedal not
    /// described, no device of that model in the chain, a wrong value.
    fn check_action(&self, action: &Action, backup: &mut og_engine::g3s::Backup, list: &mut og_engine::setlist::SetList) -> (String, Option<String>) {
        let kind = match action.kind() {
            Ok(kind) => kind,
            Err(e) => return (action.summary(None), Some(e)),
        };
        match kind {
            Kind::Patch | Kind::PcMap | Kind::System => {
                let key = action.pedal.clone().unwrap_or_default();
                let Some(description) = crate::pedal::description_of(&key) else {
                    return (action.summary(None), Some(tf!("the pedal “{key}” is not one the program describes" [key])));
                };
                let summary = action.summary(Some(&description));
                if !self.settings.set_list.devices.iter().any(|d| d.model == key) {
                    return (summary, Some(tf!("add a device of model {key} under Devices" [key])));
                }
                let checked = match kind {
                    Kind::Patch => description.number(action.memory.as_deref().unwrap_or_default()).ok_or_else(|| tf!("“{}” is no memory of the {}", action.memory.clone().unwrap_or_default(), description.name)).and_then(|_| script::patch_from(&description, action, None).map(|_| ())),
                    Kind::PcMap => script::pc_map_from(&description, action.pc_map.as_ref().expect("a pc_map action")).map(|_| ()),
                    Kind::System => {
                        let mut raw = vec![0; description.system_size()];
                        script::apply_set(&description, &description.system, &mut raw, action.system.as_ref().expect("a system action"))
                    }
                    _ => Ok(()),
                };
                (summary, checked.err())
            }
            Kind::G3sBank | Kind::G3sPreset | Kind::G3sRemoveBank => (action.summary(None), script::apply_g3s(backup, action).err()),
            Kind::SongAdd | Kind::SongSet | Kind::SongRemove => (action.summary(None), script::apply_song(list, action).err()),
            Kind::G3sLoops => (action.summary(None), script::apply_loops(list, action).err()),
        }
    }

    /// Starts applying the script, action after action.
    pub fn apply_ai_script(&mut self) {
        let Some(ui) = &mut self.setlist.script else { return };
        if ui.errors() > 0 || ui.running.is_some() || ui.script.actions.is_empty() {
            return;
        }
        ui.results = vec![None; ui.script.actions.len()];
        ui.running = Some(0);
        ui.stage = Stage::Start;
        ui.message = tr("Applying…").to_owned();
    }

    /// Once a frame: goes on with the action being applied when the pedal's
    /// thread is free, and takes the outcome of what was asked.
    pub fn poll_script(&mut self) {
        let Some(ui) = &mut self.setlist.script else { return };
        let Some(i) = ui.running else { return };
        if i >= ui.script.actions.len() {
            let done = ui.results.iter().filter(|r| matches!(r, Some(Ok(_)))).count();
            let failed = ui.results.len() - done;
            ui.running = None;
            ui.message = tf!("{done} actions done, {failed} failed." [done, failed]);
            return;
        }
        let action = ui.script.actions[i].clone();
        let stage = ui.stage;
        let ctx = ui.ctx.clone();
        let outcome = match action.kind() {
            Ok(Kind::G3sBank | Kind::G3sPreset | Kind::G3sRemoveBank) => Some(self.apply_g3s_action(&action)),
            Ok(Kind::G3sLoops) => {
                let outcome = script::apply_loops(&mut self.settings.set_list, &action);
                if outcome.is_ok() {
                    self.save_settings();
                }
                Some(outcome)
            }
            Ok(Kind::SongAdd | Kind::SongSet | Kind::SongRemove) => {
                let outcome = script::apply_song(&mut self.settings.set_list, &action);
                if outcome.is_ok() {
                    self.setlist.active = None;
                    self.save_settings();
                }
                Some(outcome)
            }
            Ok(kind) => self.step_pedal_action(&action, kind, stage, &ctx),
            Err(e) => Some(Err(e)),
        };
        if let (Some(outcome), Some(ui)) = (outcome, &mut self.setlist.script) {
            ui.results[i] = Some(outcome);
            ui.running = Some(i + 1);
            ui.stage = Stage::Start;
            ctx.request_repaint();
        }
    }

    /// A G3S action on the module's backup (opened on a made-up one when none is).
    fn apply_g3s_action(&mut self, action: &Action) -> Result<String, String> {
        if self.setlist.g3s.is_none() {
            let device = self.settings.set_list.devices.iter().position(|d| d.model == "g3s").unwrap_or(0);
            self.open_g3s(device);
            self.setlist.page = crate::setlist::Page::Script;
        }
        let g = self.setlist.g3s.as_mut().expect("opened");
        let outcome = script::apply_g3s(&mut g.backup, action);
        if outcome.is_ok() {
            g.edited = true;
        }
        outcome
    }

    /// One step of a pedal action; the outcome once the pedal has answered.
    fn step_pedal_action(&mut self, action: &Action, kind: Kind, stage: Stage, ctx: &egui::Context) -> Option<Result<String, String>> {
        let key = action.pedal.clone().unwrap_or_default();
        // The module of that pedal, open.
        let device = self.settings.set_list.devices.iter().position(|d| d.model == key)?;
        if self.setlist.pedal.as_ref().is_none_or(|s| s.device != device) {
            if self.setlist.pedal.is_some() {
                self.close_pedal();
            }
            self.open_pedal(device, ctx);
            self.setlist.page = crate::setlist::Page::Script;
            if self.setlist.pedal.is_none() {
                return Some(Err(self.setlist.message.clone()));
            }
        }
        let session = self.setlist.pedal.as_mut()?;
        if session.busy() {
            return None;
        }
        if let Some(outcome) = session.outcome.take() {
            if outcome.is_err() {
                return Some(outcome);
            }
            if stage == Stage::Writing {
                return Some(outcome);
            }
        }
        let description = session.description.clone();
        match kind {
            Kind::Patch => {
                let number = description.number(action.memory.as_deref().unwrap_or_default())?;
                let current = session.file.patch(&description, number);
                if action.from.unwrap_or_default() == script::From::Memory && current.is_none() && stage == Stage::Start {
                    session.ask(Job::ReadPatch(number));
                    self.set_script_stage(Stage::Reading);
                    return None;
                }
                match script::patch_from(&description, action, current.as_deref()) {
                    Ok(raw) => {
                        session.selected = Some(number);
                        session.raw = Some(raw.clone());
                        session.ask(Job::Store { number, raw });
                        self.set_script_stage(Stage::Writing);
                        None
                    }
                    Err(e) => Some(Err(e)),
                }
            }
            Kind::PcMap => {
                if session.pc_map.is_empty() && stage == Stage::Start {
                    session.ask(Job::ReadPcMap);
                    self.set_script_stage(Stage::Reading);
                    return None;
                }
                let entries = match script::pc_map_from(&description, action.pc_map.as_ref()?) {
                    Ok(entries) => entries,
                    Err(e) => return Some(Err(e)),
                };
                let mut map = session.pc_map.clone();
                map.resize(description.pc_map_entries, 0);
                for (index, number) in entries {
                    map[index] = number;
                }
                session.pc_map.clone_from(&map);
                session.ask(Job::WritePcMap(map));
                self.set_script_stage(Stage::Writing);
                None
            }
            Kind::System => {
                if session.system.is_empty() && stage == Stage::Start {
                    session.ask(Job::ReadSystem);
                    self.set_script_stage(Stage::Reading);
                    return None;
                }
                let mut raw = session.system.clone();
                raw.resize(description.system_size(), 0);
                if let Err(e) = script::apply_set(&description, &description.system, &mut raw, action.system.as_ref()?) {
                    return Some(Err(e));
                }
                session.system.clone_from(&raw);
                session.ask(Job::WriteSystem(raw));
                self.set_script_stage(Stage::Writing);
                None
            }
            _ => Some(Err("not a pedal action".to_owned())),
        }
    }

    fn set_script_stage(&mut self, stage: Stage) {
        if let Some(ui) = &mut self.setlist.script {
            ui.stage = stage;
        }
    }

    /// Closes the page of the script.
    pub fn close_ai_script(&mut self) {
        self.setlist.script = None;
        self.setlist.page = crate::setlist::Page::Songs;
    }

    /// Writes a script for an AI to read and change.
    fn save_ai_script(&mut self, actions: Vec<Action>, name: &str) {
        let Some(path) = files::save_file(tr("AI script"), &["json"], name) else { return };
        self.write_ai_script(actions, &path);
    }

    pub fn write_ai_script(&mut self, actions: Vec<Action>, path: &Path) {
        let count = actions.len();
        let script = Script { version: script::FORMAT, pedalboard: None, actions };
        self.setlist.message = match std::fs::write(path, script.to_json()) {
            Ok(()) => tf!("{count} actions are written in {}." [count], path.display()),
            Err(e) => tf!("The file was not written: {e}" [e]),
        };
    }

    /// The patch edited in the pedal module, as a script.
    pub fn export_pedal_patch_for_ai(&mut self) {
        let Some(session) = &self.setlist.pedal else { return };
        let (Some(number), Some(raw)) = (session.selected, &session.raw) else { return };
        let action = script::export_patch(&session.description, number, raw);
        let name = format!("{}-{}.json", session.description.key, session.description.label(number));
        self.save_ai_script(vec![action], &name);
    }

    /// Every patch the pedal module's file holds, as a script.
    pub fn export_pedal_file_for_ai(&mut self) {
        let Some(session) = &self.setlist.pedal else { return };
        let d = &session.description;
        let actions: Vec<Action> = session.file.patches.keys().filter_map(|label| d.number(label).and_then(|n| session.file.patch(d, n).map(|raw| script::export_patch(d, n, &raw)))).collect();
        let name = format!("{}.json", d.key);
        self.save_ai_script(actions, &name);
    }

    /// The whole pedalboard, as a file for an AI: the chain and what is known.
    pub fn export_pedalboard_for_ai(&mut self) {
        let script = self.pedalboard_script();
        let Some(path) = files::save_file(tr("AI script"), &["json"], "pedalboard.json") else { return };
        let count = script.actions.len();
        self.setlist.message = match std::fs::write(&path, script.to_json()) {
            Ok(()) => tf!("The pedalboard is written in {}, with {count} actions: give it to your AI with the guide." [count], path.display()),
            Err(e) => tf!("The file was not written: {e}" [e]),
        };
    }

    /// The pedalboard file as a script: the chain, the switcher's loops, the
    /// songs, the G3S backup loaded, and the memories known of the pedals.
    pub fn pedalboard_script(&self) -> Script {
        let backup = self.setlist.g3s.as_ref().map(|g| &g.backup);
        let pedals = self.known_pedals();
        script::pedalboard(&self.settings.set_list, backup, &pedals)
    }

    /// The songs of the list, as a script.
    pub fn export_songs_for_ai(&mut self) {
        let actions = script::export_songs(&self.settings.set_list);
        self.save_ai_script(actions, "songs.json");
    }

    /// The G3S backup loaded, as a script.
    pub fn export_g3s_for_ai(&mut self) {
        let Some(g) = &self.setlist.g3s else { return };
        let actions = script::export_g3s(&g.backup);
        self.save_ai_script(actions, "g3s.json");
    }

    /// Writes the guide where the user wants it.
    pub fn save_ai_guide(&mut self) {
        let Some(path) = files::save_file(tr("Guide for an AI"), &["md"], "ai-guide.md") else { return };
        self.setlist.message = match std::fs::write(&path, guide_text()) {
            Ok(()) => tf!("The guide is written in {}: give it to your AI, then apply the script it writes.", path.display()),
            Err(e) => tf!("The file was not written: {e}" [e]),
        };
    }

    /// The page of a script: its actions, each with its sentence and what
    /// came of it; Apply when nothing is wrong.
    pub fn set_script_page(&mut self, ui: &mut egui::Ui) {
        let Some(s) = &self.setlist.script else {
            self.setlist.page = crate::setlist::Page::Songs;
            return;
        };
        let (mut close, mut apply) = (false, false);
        ui.horizontal(|ui| {
            if ui.button(egui::RichText::new(tr("< Songs")).size(16.0)).clicked() {
                close = true;
            }
            ui.heading(tr("AI script"));
            if s.running.is_some() {
                ui.spinner();
            }
        });
        if let Some(path) = &s.path {
            ui.weak(path.display().to_string());
        }
        ui.label(&s.message);
        ui.horizontal(|ui| {
            let can = s.errors() == 0 && s.running.is_none() && !s.script.actions.is_empty();
            if ui.add_enabled(can, egui::Button::new(tr("Apply"))).on_hover_text(tr("Writes the pedals and the G3S backup as the actions say; each memory written is kept before and read back after")).clicked() {
                apply = true;
            }
            if s.errors() > 0 {
                ui.weak(tr("Correct the script (or the chain) before applying it."));
            }
        });
        ui.separator();
        let height = ui.available_height();
        egui::ScrollArea::vertical().id_salt("ai script").max_height(height).auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("ai actions").num_columns(3).spacing([12.0, 6.0]).max_col_width(440.0).striped(true).show(ui, |ui| {
                for (i, (summary, error)) in s.summaries.iter().enumerate() {
                    ui.label(format!("{}", i + 1));
                    ui.label(summary);
                    match (error, s.results.get(i).and_then(|r| r.as_ref()), s.running == Some(i)) {
                        (Some(e), _, _) => ui.add(egui::Label::new(egui::RichText::new(e).color(egui::Color32::from_rgb(230, 90, 90))).wrap()),
                        (None, Some(Ok(text)), _) => ui.colored_label(egui::Color32::from_rgb(60, 220, 110), text),
                        (None, Some(Err(e)), _) => ui.add(egui::Label::new(egui::RichText::new(e).color(egui::Color32::from_rgb(230, 90, 90))).wrap()),
                        (None, None, true) => ui.weak(tr("Running…")),
                        (None, None, false) => ui.label(""),
                    };
                    ui.end_row();
                }
            });
        });
        if apply {
            self.apply_ai_script();
        } else if close {
            self.close_ai_script();
        }
    }
}

#[cfg(test)]
mod tests {
    /// The guide kept in the documents is the one this version writes:
    /// regenerate it with `opengaragemusic.exe --ai-guide docs/en/ai-guide.md`.
    #[test]
    fn the_guide_in_the_documents_is_the_one_the_program_writes() {
        let kept = include_str!("../../../docs/en/ai-guide.md").replace("\r\n", "\n");
        let written = super::guide_text().replace("\r\n", "\n");
        assert!(kept == written, "docs/en/ai-guide.md is behind the program: run opengaragemusic.exe --ai-guide docs/en/ai-guide.md");
    }
}
