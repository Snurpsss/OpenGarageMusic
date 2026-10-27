//! OpenGarageMusic audio engine: project model, sound generators, effects, sequencer
//! and mixer, plus the file formats it reads and writes.
//!
//! This crate has no user-interface dependency; the app sits on top of it.

pub mod align;
pub mod analysis;
pub mod capture;
pub mod chords;
pub mod codec;
pub mod devices;
pub mod convolution;
pub mod drummer;
pub mod engine;
pub mod gpx;
pub mod guitar_pro;
pub mod fx;
pub mod g3s;
pub mod loops;
pub mod lyrics;
pub mod model;
pub mod patches;
pub mod recording;
pub mod resample;
pub mod roland;
pub mod script;
pub mod setlist;
pub mod sf3;
pub mod sfz;
pub mod sites;
pub mod smf;
pub mod songsterr;
pub mod stretch;
pub mod synth;
pub mod tabsites;
pub mod templates;
pub mod testfont;
pub mod wav;

pub use capture::Captures;
pub use engine::{render_offline, render_offline_full, render_offline_with, ClipData, Clips, Engine, ExternalUnit, MonitorFeed, OutFeed, Recorded};
pub use synth::{Bank, Fonts};
pub use model::*;
