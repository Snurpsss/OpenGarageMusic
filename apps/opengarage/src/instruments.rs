//! The kind of instrument a track plays, which its header shows as a
//! pictogram: found from the track and its sound, never chosen by hand.

use og_engine::{patches, Sound, TrackKind};

use crate::app::App;
use crate::i18n::tr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instrument {
    Piano,
    Synth,
    Bass,
    Guitar,
    Strings,
    Brass,
    Winds,
    Mallets,
    Drums,
    Voice,
    Audio,
    Video,
    Other,
}

impl Instrument {
    pub fn label(self) -> &'static str {
        match self {
            Instrument::Piano => tr("Piano or keyboard"),
            Instrument::Synth => tr("Synthesizer"),
            Instrument::Bass => tr("Bass"),
            Instrument::Guitar => tr("Guitar"),
            Instrument::Strings => tr("Strings"),
            Instrument::Brass => tr("Brass"),
            Instrument::Winds => tr("Winds"),
            Instrument::Mallets => tr("Mallets"),
            Instrument::Drums => tr("Drums"),
            Instrument::Voice => tr("Voice"),
            Instrument::Audio => tr("Audio"),
            Instrument::Video => tr("Video"),
            Instrument::Other => tr("Other instrument"),
        }
    }

    /// The instrument of a General MIDI program.
    pub fn of_program(program: u8) -> Instrument {
        match program {
            0..=7 | 16..=23 => Instrument::Piano,
            8..=15 => Instrument::Mallets,
            24..=31 => Instrument::Guitar,
            32..=39 => Instrument::Bass,
            40..=51 | 55 => Instrument::Strings,
            52..=54 => Instrument::Voice,
            56..=63 => Instrument::Brass,
            64..=79 => Instrument::Winds,
            80..=103 => Instrument::Synth,
            104..=111 => Instrument::Strings,
            112..=119 => Instrument::Drums,
            _ => Instrument::Other,
        }
    }

    /// The instrument of a sound of the library, from its category and,
    /// for the orchestra, its name.
    fn of_patch(category: &str, name: &str) -> Option<Instrument> {
        let name = name.to_lowercase();
        let has = |words: &[&str]| words.iter().any(|w| name.contains(w));
        Some(match category {
            patches::CAT_KEYS => Instrument::Piano,
            patches::CAT_SYNTH => Instrument::Synth,
            patches::CAT_BASS => Instrument::Bass,
            patches::CAT_GUITAR | patches::CAT_AMP => Instrument::Guitar,
            patches::CAT_MALLET => Instrument::Mallets,
            patches::CAT_DRUMS => Instrument::Drums,
            patches::CAT_VOICE => Instrument::Voice,
            patches::CAT_ORCH if has(&["brass", "trumpet", "horn", "trombone", "tuba"]) => Instrument::Brass,
            patches::CAT_ORCH if has(&["flute", "sax", "clarinet", "oboe", "bassoon", "recorder", "pipe"]) => Instrument::Winds,
            patches::CAT_ORCH => Instrument::Strings,
            _ => return None,
        })
    }
}

impl App {
    /// What a track plays: a drummer track, a guitar track and an audio
    /// track are what they are; an instrument track is what its sound is.
    pub fn track_instrument(&self, ti: usize) -> Instrument {
        let Some(track) = self.project.tracks.get(ti) else { return Instrument::Other };
        let from_patch = || self.patches.iter().find(|p| p.name == track.patch).and_then(|p| Instrument::of_patch(&p.category, &p.name));
        match track.kind {
            TrackKind::Drummer => Instrument::Drums,
            TrackKind::Guitar => Instrument::Guitar,
            TrackKind::Video => Instrument::Video,
            TrackKind::Audio => match from_patch() {
                Some(Instrument::Voice) | None => Instrument::Voice,
                Some(_) => Instrument::Audio,
            },
            TrackKind::Instrument => {
                if track.sound.is_drums() {
                    return Instrument::Drums;
                }
                if let Sound::Sampler(sampler) = track.sound {
                    if sampler.bank < 128 {
                        return Instrument::of_program(sampler.preset);
                    }
                }
                from_patch().unwrap_or(match track.icon {
                    // What an import knew of the part, when the sound tells nothing.
                    1 => Instrument::Drums,
                    3 => Instrument::Guitar,
                    5 => Instrument::Strings,
                    _ => Instrument::Piano,
                })
            }
        }
    }
}
