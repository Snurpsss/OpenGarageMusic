//! The MIDI Pilot on its own: the program started with `--midi-pilot` (as
//! `MidiPilot.exe` does) shows the window of the pilot and nothing else, with
//! no sound card, no loading screen and no project. A song whose project is to
//! be opened starts the whole program on it.

use eframe::egui;

use crate::app::App;
use crate::audio;

/// First argument of the program when it is started to be the pilot alone.
pub const FLAG: &str = "--midi-pilot";

pub struct Pilot {
    app: App,
}

impl Pilot {
    pub fn new() -> Self {
        let mut app = App::begin_with(audio::silent(og_engine::templates::demo()));
        app.pilot_only = true;
        app.keep_sessions = false;
        app.open_set_list();
        Self { app }
    }
}

impl eframe::App for Pilot {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.app.pilot_frame(ctx);
    }
}
