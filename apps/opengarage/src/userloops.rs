//! Loops made of recorded sound. None is shipped: they are the user's own
//! audio files, kept in a folder of the program, and stretched to the tempo of
//! the project when they are put on a track.

use std::path::{Path, PathBuf};

use og_engine::codec::AUDIO_EXTENSIONS;

/// Name of the folder, next to the downloaded banks and plug-ins.
pub const FOLDER: &str = "Boucles";

/// What is dragged from the loop browser: one of the built-in loops of notes,
/// or one of the user's loops of recorded sound, by its place in its list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoopPick {
    Notes(usize),
    Audio(usize),
}

pub struct AudioLoop {
    pub name: String,
    pub path: PathBuf,
    /// Tempo the file's name gives ("Funk 98 bpm"), if it gives one.
    pub named_tempo: Option<f64>,
}

/// The tempo a file name states: a number followed by "bpm".
pub fn tempo_in_name(name: &str) -> Option<f64> {
    let lower = name.to_lowercase();
    let at = lower.find("bpm")?;
    let before = lower[..at].trim_end_matches([' ', '_', '-']);
    let digits: String = before.chars().rev().take_while(|c| c.is_ascii_digit() || *c == '.').collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse::<f64>().ok().filter(|tempo| (30.0..=300.0).contains(tempo))
}

/// The tempo and the number of bars of a loop that lasts `seconds`: the tempo
/// its name gives, else the one that makes it a round number of bars (1, 2, 4,
/// 8…) at a usual tempo, the nearest to 110 when several would do.
pub fn tempo_and_bars(named: Option<f64>, seconds: f64, beats_per_bar: f64) -> (f64, u32) {
    let beats_per_bar = beats_per_bar.max(1.0);
    if seconds <= 0.0 {
        return (120.0, 1);
    }
    if let Some(tempo) = named {
        let bars = (seconds * tempo / 60.0 / beats_per_bar).round().max(1.0);
        return (tempo, bars as u32);
    }
    let mut best = (120.0, 1u32);
    for bars in [1u32, 2, 4, 8, 16, 32, 64] {
        let tempo = f64::from(bars) * beats_per_bar * 60.0 / seconds;
        if (tempo - 110.0).abs() < (best.0 - 110.0f64).abs() || bars == 1 {
            best = (tempo, bars);
        }
    }
    best
}

/// The audio files of the folder, by name.
pub fn scan(folder: &Path) -> Vec<AudioLoop> {
    let mut found: Vec<AudioLoop> = std::fs::read_dir(folder)
        .map(|entries| entries.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|path| path.is_file() && path.extension().is_some_and(|e| AUDIO_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str())))
        .map(|path| {
            let name = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            AudioLoop { named_tempo: tempo_in_name(&name), name, path }
        })
        .collect();
    found.sort_by_key(|a| a.name.to_lowercase());
    found.truncate(2000);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tempo_of_a_loop_is_read_from_its_name_or_worked_out() {
        assert_eq!(tempo_in_name("Funk drums 98 bpm"), Some(98.0));
        assert_eq!(tempo_in_name("house_124BPM_Am"), Some(124.0));
        assert_eq!(tempo_in_name("groove-87.5-bpm"), Some(87.5));
        assert_eq!(tempo_in_name("no tempo here"), None);
        assert_eq!(tempo_in_name("bpm"), None);
        assert_eq!(tempo_in_name("9000bpm"), None, "not a tempo");
        // Named: the bars follow from the length.
        assert_eq!(tempo_and_bars(Some(120.0), 8.0, 4.0), (120.0, 4));
        assert_eq!(tempo_and_bars(Some(100.0), 2.4, 4.0), (100.0, 1));
        // Not named: a round number of bars at a usual tempo.
        let (tempo, bars) = tempo_and_bars(None, 8.0, 4.0);
        assert_eq!((tempo, bars), (120.0, 4));
        let (tempo, bars) = tempo_and_bars(None, 2.0, 4.0);
        assert_eq!((tempo, bars), (120.0, 1));
        let (tempo, bars) = tempo_and_bars(None, 19.2, 4.0);
        assert_eq!((tempo.round(), bars), (100.0, 8));
        assert_eq!(tempo_and_bars(None, 0.0, 4.0), (120.0, 1));
        // Three beats in a bar.
        assert_eq!(tempo_and_bars(None, 6.0, 3.0), (120.0, 4));
    }
}
