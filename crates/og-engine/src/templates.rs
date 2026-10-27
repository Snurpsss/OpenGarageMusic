//! Starting points for new projects.

use crate::model::{
    Content, DrumStyle, DrummerParams, HatMode, Note, Perc, Project, Region, Track, TrackKind,
};
use crate::patches;

/// A track of the given kind set up with a built-in patch.
pub fn track(name: &str, kind: TrackKind, patch: &str, color: u8) -> Track {
    let mut track = Track::new(name, kind);
    track.color = color;
    // Pictograms: keyboard, drum, microphone, guitar.
    track.icon = match kind {
        TrackKind::Instrument => 0,
        TrackKind::Drummer => 1,
        TrackKind::Audio | TrackKind::Video => 2,
        TrackKind::Guitar => 3,
    };
    if let Some(def) = patches::find(patch) {
        track.patch = def.name;
        track.sound = def.sound;
        track.fx = def.fx;
    }
    track
}

/// A drummer track with one region.
pub fn drummer_track(name: &str, patch: &str, params: DrummerParams, length: f64, color: u8) -> Track {
    let mut t = track(name, TrackKind::Drummer, patch, color);
    let mut region = Region::new("Drums", 0.0, length);
    region.content = Content::Drummer(params);
    t.regions.push(region);
    t
}

pub struct Template {
    pub name: &'static str,
    pub description: &'static str,
    pub build: fn() -> Project,
}

pub fn all() -> Vec<Template> {
    vec![
        Template { name: "Empty project", description: "One instrument track, nothing else.", build: empty },
        Template { name: "Demonstration", description: "Four bars already written to discover the application.", build: demo },
        Template { name: "Keyboards", description: "Piano, organ, pad and strings ready to play.", build: keyboards },
        Template { name: "Band", description: "Drummer, bass, guitar through an amp, and keyboard.", build: band },
        Template { name: "Hip-hop", description: "Hip-hop drummer, sub bass, keyboards and synth.", build: hip_hop },
        Template { name: "Electro", description: "Electro drummer, synth bass, arpeggios and pads.", build: electro },
        Template { name: "Songwriter", description: "Voice, acoustic guitar and light drums.", build: songwriter },
    ]
}

pub fn empty() -> Project {
    let mut project = Project::empty();
    project.tracks.push(track("Track 1", TrackKind::Instrument, patches::DEFAULT_PATCH, 0));
    project
}

fn keyboards() -> Project {
    let mut project = Project::empty();
    project.name = "Keyboards".to_owned();
    for (i, patch) in ["Electric piano", "Drawbar organ", "Warm pad", "String ensemble"].into_iter().enumerate() {
        project.tracks.push(track(patch, TrackKind::Instrument, patch, i as u8));
    }
    project
}

fn band() -> Project {
    let mut project = Project::empty();
    project.name = "Band".to_owned();
    project.tempo = 110.0;
    project.tracks = vec![
        drummer_track("Drummer", "Acoustic kit", DrummerParams::default(), 32.0, 4),
        track("Bass", TrackKind::Instrument, "Electric bass", 1),
        track("Guitar", TrackKind::Guitar, "Crunch amp", 2),
        track("Keyboard", TrackKind::Instrument, "Electric piano", 0),
    ];
    project
}

fn hip_hop() -> Project {
    let mut project = Project::empty();
    project.name = "Hip-hop".to_owned();
    project.tempo = 88.0;
    project.minor = true;
    let params = DrummerParams { style: DrumStyle::HipHop, complexity: 0.5, perc: Perc::Clap, ..DrummerParams::default() };
    project.tracks = vec![
        drummer_track("Drummer", "Hip-hop kit", params, 32.0, 4),
        track("Sub bass", TrackKind::Instrument, "Sub bass", 1),
        track("Keyboards", TrackKind::Instrument, "Felt piano", 0),
        track("Synth", TrackKind::Instrument, "Square lead", 3),
        track("Voice", TrackKind::Audio, "Natural voice", 5),
    ];
    project
}

fn electro() -> Project {
    let mut project = Project::empty();
    project.name = "Electro".to_owned();
    project.tempo = 126.0;
    project.minor = true;
    let params = DrummerParams { style: DrumStyle::Electro, complexity: 0.5, perc: Perc::Clap, ..DrummerParams::default() };
    project.tracks = vec![
        drummer_track("Drummer", "Electro kit", params, 32.0, 4),
        track("Bass", TrackKind::Instrument, "Synth bass", 1),
        track("Arpeggio", TrackKind::Instrument, "Synthetic pluck", 2),
        track("Pad", TrackKind::Instrument, "Warm pad", 3),
    ];
    project.tracks[2].arp.on = true;
    project
}

fn songwriter() -> Project {
    let mut project = Project::empty();
    project.name = "Song".to_owned();
    project.tempo = 96.0;
    let params = DrummerParams { style: DrumStyle::Pop, complexity: 0.2, loudness: 0.4, hats: HatMode::Hats, perc: Perc::Shaker, ..DrummerParams::default() };
    project.tracks = vec![
        track("Voice", TrackKind::Audio, "Natural voice", 5),
        track("Guitar", TrackKind::Instrument, "Steel guitar", 2),
        track("Piano", TrackKind::Instrument, "Felt piano", 0),
        drummer_track("Drummer", "Acoustic kit", params, 32.0, 4),
    ];
    project
}

/// A short original four-bar song so a fresh install makes sound straight away.
pub fn demo() -> Project {
    let midi = |name: &str, notes: Vec<Note>| {
        let mut region = Region::new(name, 0.0, 16.0);
        region.content = Content::Midi(notes);
        region
    };

    let mut keys = track("Chords", TrackKind::Instrument, "Electric piano", 0);
    let chords: [[u8; 3]; 4] = [[57, 60, 64], [53, 57, 60], [55, 60, 64], [55, 59, 62]];
    let mut notes = Vec::new();
    for (bar, chord) in chords.iter().enumerate() {
        for &pitch in chord {
            notes.push(Note { start: bar as f64 * 4.0, length: 3.5, pitch, velocity: 80, string: None, ..Default::default() });
        }
    }
    keys.regions.push(midi("Chords", notes));
    keys.volume = 0.8;

    let mut bass = track("Bass", TrackKind::Instrument, "Electric bass", 1);
    let mut notes = Vec::new();
    for (bar, root) in [33u8, 29, 36, 31].into_iter().enumerate() {
        for eighth in 0..8 {
            notes.push(Note {
                start: bar as f64 * 4.0 + f64::from(eighth) * 0.5,
                length: 0.4,
                pitch: root,
                velocity: if eighth % 2 == 0 { 105 } else { 80 },
                string: None, ..Default::default()
            });
        }
    }
    bass.regions.push(midi("Bass", notes));

    let mut lead = track("Melody", TrackKind::Instrument, "Classic lead", 2);
    let melody: [(f64, f64, u8); 15] = [
        (0.0, 1.0, 76),
        (1.0, 0.5, 72),
        (1.5, 0.5, 74),
        (2.0, 2.0, 76),
        (4.0, 1.0, 77),
        (5.0, 0.5, 72),
        (5.5, 0.5, 69),
        (6.0, 2.0, 72),
        (8.0, 1.0, 79),
        (9.0, 0.5, 76),
        (9.5, 0.5, 72),
        (10.0, 2.0, 76),
        (12.0, 1.0, 74),
        (13.0, 1.0, 71),
        (14.0, 2.0, 67),
    ];
    let notes = melody.iter().map(|&(start, length, pitch)| Note { start, length, pitch, velocity: 95, string: None, ..Default::default() }).collect();
    lead.regions.push(midi("Melody", notes));
    lead.volume = 0.7;
    lead.pan = 0.2;

    let drummer = drummer_track("Drummer", "Acoustic kit", DrummerParams { complexity: 0.35, ..DrummerParams::default() }, 16.0, 4);

    let mut project = Project::empty();
    project.name = "Demo".to_owned();
    project.tempo = 100.0;
    project.key = 9;
    project.minor = true;
    project.tracks = vec![keys, bass, lead, drummer];
    project
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_template_builds_and_round_trips() {
        for template in all() {
            let project = (template.build)();
            assert!(!project.tracks.is_empty(), "{}", template.name);
            assert!(project.tracks.iter().all(|t| !t.patch.is_empty()), "{}", template.name);
            let text = project.to_json().unwrap();
            assert_eq!(Project::from_json(&text).unwrap(), project, "{}", template.name);
        }
    }
}
