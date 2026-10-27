//! The built-in sound library. Every patch is original to this project.

use serde::{Deserialize, Serialize};

use crate::model::{
    Amp, AmpModel, DrumKit, DrumParams, Eq, Pedal, PedalKind, PluckParams, Sound, SynthParams,
    TrackFx, Wave,
};

/// A named sound with its effect settings. User patches use the same shape.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PatchDef {
    pub name: String,
    pub category: String,
    pub sound: Sound,
    #[serde(default)]
    pub fx: TrackFx,
}

#[allow(clippy::too_many_arguments)]
fn synth(
    wave: Wave,
    detune_cents: f32,
    sub: f32,
    adsr: (f32, f32, f32, f32),
    cutoff: f32,
    cutoff_env: f32,
    vibrato: f32,
    gain: f32,
) -> Sound {
    Sound::Synth(SynthParams {
        wave,
        detune_cents,
        sub,
        attack: adsr.0,
        decay: adsr.1,
        sustain: adsr.2,
        release: adsr.3,
        cutoff,
        cutoff_env,
        vibrato,
        gain,
    })
}

fn pluck(brightness: f32, sustain: f32, gain: f32) -> Sound {
    Sound::Pluck(PluckParams { brightness, sustain, gain })
}

fn drums(kit: DrumKit) -> Sound {
    Sound::Drums(DrumParams { kit, ..DrumParams::default() })
}

fn fx(comp: f32, echo: f32, reverb: f32) -> TrackFx {
    TrackFx { comp, echo, reverb, ..TrackFx::default() }
}

pub const CAT_KEYS: &str = "Keyboards";
pub const CAT_SYNTH: &str = "Synthesizers";
pub const CAT_BASS: &str = "Basses";
pub const CAT_GUITAR: &str = "Guitars";
pub const CAT_ORCH: &str = "Strings and brass";
pub const CAT_MALLET: &str = "Mallets";
pub const CAT_DRUMS: &str = "Drum kits";
pub const CAT_AMP: &str = "Amps (guitar tracks)";
pub const CAT_VOICE: &str = "Voice (audio tracks)";
pub const CAT_USER: &str = "My sounds";
pub const CAT_SFZ: &str = "SFZ instruments";

pub const DEFAULT_PATCH: &str = "Electric piano";
pub const DEFAULT_DRUMS: &str = "Acoustic kit";

pub fn builtin() -> Vec<PatchDef> {
    use Wave::{Saw, Sine, Square, Triangle};
    let p = |name: &str, category: &str, sound: Sound, fx: TrackFx| PatchDef {
        name: name.to_owned(),
        category: category.to_owned(),
        sound,
        fx,
    };
    let amp = |model: AmpModel, gain: f32, pedals: Vec<Pedal>, reverb: f32| TrackFx {
        amp: Amp { on: true, model, gain, ..Amp::default() },
        pedals,
        reverb,
        ..TrackFx::default()
    };
    let pedal = |kind: PedalKind, amount: f32, rate: f32| Pedal { kind, on: true, amount, rate };
    let silent = synth(Sine, 0.0, 0.0, (0.01, 0.1, 0.5, 0.1), 2000.0, 0.0, 0.0, 0.3);
    vec![
        p(DEFAULT_PATCH, CAT_KEYS, synth(Triangle, 6.0, 0.0, (0.003, 0.9, 0.15, 0.25), 1800.0, 2500.0, 0.0, 0.5), fx(0.0, 0.0, 0.15)),
        p("Felt piano", CAT_KEYS, synth(Triangle, 3.0, 0.2, (0.004, 1.4, 0.05, 0.4), 900.0, 1500.0, 0.0, 0.55), fx(0.0, 0.0, 0.25)),
        p("Drawbar organ", CAT_KEYS, synth(Sine, 1200.0, 0.5, (0.01, 0.05, 1.0, 0.06), 6000.0, 0.0, 0.05, 0.4), fx(0.0, 0.0, 0.2)),
        p("Church organ", CAT_KEYS, synth(Square, 1200.0, 0.6, (0.05, 0.1, 1.0, 0.4), 2500.0, 0.0, 0.0, 0.3), fx(0.0, 0.0, 0.6)),
        p("Funky keyboard", CAT_KEYS, pluck(0.95, 0.25, 0.6), fx(0.3, 0.0, 0.05)),
        p("Classic lead", CAT_SYNTH, synth(Saw, 9.0, 0.0, (0.01, 0.2, 0.7, 0.15), 2200.0, 2500.0, 0.15, 0.35), fx(0.0, 0.25, 0.15)),
        p("Square lead", CAT_SYNTH, synth(Square, 5.0, 0.0, (0.005, 0.1, 0.8, 0.1), 3000.0, 1000.0, 0.2, 0.3), fx(0.0, 0.2, 0.1)),
        p("Warm pad", CAT_SYNTH, synth(Saw, 14.0, 0.0, (0.5, 0.6, 0.8, 0.9), 900.0, 900.0, 0.0, 0.3), fx(0.0, 0.1, 0.5)),
        p("Glass pad", CAT_SYNTH, synth(Triangle, 1200.0, 0.0, (0.3, 1.0, 0.6, 1.2), 3500.0, 2000.0, 0.1, 0.3), fx(0.0, 0.3, 0.6)),
        p("Synthetic pluck", CAT_SYNTH, synth(Saw, 5.0, 0.0, (0.002, 0.22, 0.0, 0.15), 500.0, 5000.0, 0.0, 0.5), fx(0.0, 0.35, 0.2)),
        p("Acid sequence", CAT_SYNTH, synth(Saw, 0.0, 0.0, (0.002, 0.15, 0.2, 0.08), 300.0, 4000.0, 0.0, 0.45), fx(0.2, 0.3, 0.1)),
        p("Synth bass", CAT_BASS, synth(Square, 0.0, 0.5, (0.004, 0.25, 0.5, 0.08), 350.0, 1400.0, 0.0, 0.5), fx(0.4, 0.0, 0.0)),
        p("Sub bass", CAT_BASS, synth(Sine, 0.0, 0.6, (0.01, 0.3, 0.9, 0.12), 400.0, 300.0, 0.0, 0.6), fx(0.3, 0.0, 0.0)),
        p("Electric bass", CAT_BASS, pluck(0.35, 0.6, 0.9), fx(0.5, 0.0, 0.0)),
        p("Slap bass", CAT_BASS, pluck(0.9, 0.45, 0.8), fx(0.5, 0.0, 0.0)),
        p("Nylon guitar", CAT_GUITAR, pluck(0.45, 0.75, 0.7), fx(0.0, 0.0, 0.25)),
        p("Steel guitar", CAT_GUITAR, pluck(0.85, 0.85, 0.6), fx(0.0, 0.0, 0.2)),
        p("Muted guitar", CAT_GUITAR, pluck(0.5, 0.1, 0.8), fx(0.3, 0.0, 0.05)),
        p("Harp", CAT_GUITAR, pluck(0.6, 1.0, 0.6), fx(0.0, 0.0, 0.45)),
        p("String ensemble", CAT_ORCH, synth(Saw, 11.0, 0.0, (0.25, 0.4, 0.85, 0.5), 1600.0, 800.0, 0.12, 0.3), fx(0.0, 0.0, 0.5)),
        p("Brass", CAT_ORCH, synth(Saw, 4.0, 0.0, (0.06, 0.25, 0.7, 0.15), 700.0, 3000.0, 0.08, 0.35), fx(0.0, 0.0, 0.3)),
        p("Flute", CAT_ORCH, synth(Triangle, 0.0, 0.0, (0.08, 0.2, 0.8, 0.2), 2500.0, 500.0, 0.15, 0.4), fx(0.0, 0.0, 0.35)),
        p("Bells", CAT_MALLET, synth(Sine, 1902.0, 0.0, (0.002, 1.6, 0.0, 1.2), 6000.0, 0.0, 0.0, 0.4), fx(0.0, 0.2, 0.5)),
        p("Marimba", CAT_MALLET, synth(Sine, 2400.0, 0.3, (0.002, 0.35, 0.0, 0.2), 3000.0, 2000.0, 0.0, 0.5), fx(0.0, 0.0, 0.25)),
        p("Music box", CAT_MALLET, pluck(1.0, 0.5, 0.5), fx(0.0, 0.2, 0.4)),
        p(DEFAULT_DRUMS, CAT_DRUMS, drums(DrumKit::Acoustic), fx(0.3, 0.0, 0.12)),
        p("Electro kit", CAT_DRUMS, drums(DrumKit::Electro), fx(0.4, 0.0, 0.05)),
        p("Hip-hop kit", CAT_DRUMS, drums(DrumKit::HipHop), fx(0.5, 0.0, 0.05)),
        p("Clean amp", CAT_AMP, silent, amp(AmpModel::Clean, 0.35, vec![pedal(PedalKind::Chorus, 0.4, 0.3)], 0.25)),
        p("Crunch amp", CAT_AMP, silent, amp(AmpModel::Crunch, 0.5, vec![], 0.15)),
        p("Lead amp", CAT_AMP, silent, amp(AmpModel::Lead, 0.6, vec![pedal(PedalKind::Overdrive, 0.4, 0.6), pedal(PedalKind::Echo, 0.3, 0.4)], 0.2)),
        p("Bass amp", CAT_AMP, silent, amp(AmpModel::Bass, 0.4, vec![], 0.0)),
        p("Natural voice", CAT_VOICE, silent, TrackFx { eq: Eq { on: true, low: -3.0, mid: 1.5, mid_freq: 2500.0, high: 2.0, ..Eq::default() }, comp: 0.4, reverb: 0.2, ..TrackFx::default() }),
        p("Radio voice", CAT_VOICE, silent, TrackFx { eq: Eq { on: true, low: -12.0, mid: 8.0, mid_freq: 1800.0, high: -10.0, ..Eq::default() }, comp: 0.8, ..TrackFx::default() }),
        p("Voice with echo", CAT_VOICE, silent, TrackFx { comp: 0.4, echo: 0.4, reverb: 0.35, ..TrackFx::default() }),
        p("No effect", CAT_VOICE, silent, TrackFx::default()),
    ]
}

/// Finds a built-in patch by name.
pub fn find(name: &str) -> Option<PatchDef> {
    builtin().into_iter().find(|p| p.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_unique_and_defaults_exist() {
        let all = builtin();
        for (i, a) in all.iter().enumerate() {
            assert!(all[i + 1..].iter().all(|b| b.name != a.name), "{}", a.name);
        }
        assert!(find(DEFAULT_PATCH).is_some());
        assert!(find(DEFAULT_DRUMS).unwrap().sound.is_drums());
    }
}
