//! The user's music library: a folder of their own recordings (MP3, OGG,
//! FLAC, M4A, WAV), searched for the song a tablature is of, so that the
//! song can be played with its original sound.
//!
//! The folder and its subfolders are read once, in the background, and what
//! was found is kept in an index beside the settings: the next reading only
//! looks again at the files that changed. A file is known by the artist and
//! the title its tags give, or else by its name ("Artist - Title.mp3").

use std::path::{Path, PathBuf};
use std::sync::mpsc;

use serde::{Deserialize, Serialize};

/// The most files looked at, to keep within reason on a disk of everything.
const MOST: usize = 100_000;

/// A recording of the library.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Song {
    pub path: PathBuf,
    pub artist: String,
    pub title: String,
    /// When the file was last changed, in seconds since 1970.
    pub changed: u64,
}

fn changed(path: &Path) -> u64 {
    std::fs::metadata(path).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_secs())
}

/// What a file says it holds: its tags, or else its name.
fn read(path: &Path) -> Song {
    let (mut artist, mut title) = og_engine::codec::tags(path);
    if title.is_empty() {
        let stem = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        // "01 - Artist - Title", "Artist - Title", or the title alone.
        let parts: Vec<&str> = stem.split(" - ").map(str::trim).filter(|p| !p.is_empty() && !p.chars().all(|c| c.is_ascii_digit())).collect();
        match parts.as_slice() {
            [name, rest @ ..] if !rest.is_empty() => {
                if artist.is_empty() {
                    artist = (*name).to_owned();
                }
                title = rest.join(" - ");
            }
            [only] => title = (*only).to_owned(),
            _ => title = stem,
        }
    }
    Song { path: path.to_path_buf(), artist, title, changed: changed(path) }
}

/// The recordings of a folder and its subfolders; the files of `known` that
/// did not change are not read again.
pub fn scan(folder: &Path, known: &[Song]) -> Vec<Song> {
    let mut songs = Vec::new();
    let mut folders = vec![folder.to_path_buf()];
    let mut seen = 0usize;
    while let Some(dir) = folders.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else { continue };
            if kind.is_dir() {
                folders.push(path);
                continue;
            }
            let audio = path.extension().is_some_and(|e| og_engine::codec::AUDIO_EXTENSIONS.contains(&e.to_string_lossy().to_lowercase().as_str()));
            if !audio {
                continue;
            }
            seen += 1;
            if seen > MOST {
                return songs;
            }
            let when = changed(&path);
            match known.iter().find(|s| s.path == path && s.changed == when) {
                Some(same) => songs.push(same.clone()),
                None => songs.push(read(&path)),
            }
        }
    }
    songs.sort_by(|a, b| a.path.cmp(&b.path));
    songs
}

/// Reads a library in the background.
pub fn start_scan(folder: PathBuf, known: Vec<Song>) -> mpsc::Receiver<Vec<Song>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(scan(&folder, &known));
    });
    receive
}

/// A name as it is compared: in small letters, without accents, without
/// what is between brackets ("(Remastered 2011)", "[Live]"), without "feat."
/// and what follows, without a leading "The", and with only letters and
/// digits, one space between words.
pub fn plain(name: &str) -> String {
    let mut text = String::new();
    let mut depth = 0usize;
    for c in name.to_lowercase().chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth > 0 => {}
            _ => text.push(match c {
                'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
                'ç' => 'c',
                'è' | 'é' | 'ê' | 'ë' => 'e',
                'ì' | 'í' | 'î' | 'ï' => 'i',
                'ñ' => 'n',
                'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
                'ù' | 'ú' | 'û' | 'ü' => 'u',
                'ý' | 'ÿ' => 'y',
                '&' => ' ',
                c if c.is_alphanumeric() => c,
                _ => ' ',
            }),
        }
    }
    for cut in [" feat ", " ft ", " featuring ", " remaster", " live at ", " - live"] {
        if let Some(at) = format!(" {text} ").find(cut) {
            text.truncate(at.saturating_sub(1).min(text.len()));
        }
    }
    let words: Vec<&str> = text.split_whitespace().collect();
    let words = if words.first() == Some(&"the") && words.len() > 1 { &words[1..] } else { &words[..] };
    words.join(" ")
}

/// The recording of a song in the library: its title the same, and its
/// artist too when both are known (or one holding the other: "Metallica"
/// and "Metallica & Michael Kamen"). Several alike: the one whose artist
/// is exactly the same, then the first.
pub fn find<'a>(songs: &'a [Song], artist: &str, title: &str) -> Option<&'a Song> {
    let (artist, title) = (plain(artist), plain(title));
    if title.is_empty() {
        return None;
    }
    let same_artist = |song: &Song| {
        let theirs = plain(&song.artist);
        artist.is_empty() || theirs.is_empty() || theirs == artist || theirs.contains(&artist) || artist.contains(&theirs)
    };
    let matches: Vec<&Song> = songs.iter().filter(|s| plain(&s.title) == title && same_artist(s)).collect();
    matches.iter().find(|s| plain(&s.artist) == artist).or_else(|| matches.first()).copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(artist: &str, title: &str, path: &str) -> Song {
        Song { path: PathBuf::from(path), artist: artist.to_owned(), title: title.to_owned(), changed: 0 }
    }

    #[test]
    fn names_are_compared_plainly() {
        assert_eq!(plain("Dream On (Remastered 2011)"), "dream on");
        assert_eq!(plain("The Beatles"), "beatles");
        assert_eq!(plain("Café del Mar [Live]"), "cafe del mar");
        assert_eq!(plain("Smooth feat. Rob Thomas"), "smooth");
        assert_eq!(plain("AC/DC"), "ac dc");
        assert_eq!(plain("Highway to Hell - Live at Donington"), "highway to hell");
    }

    #[test]
    fn the_recording_of_a_song_is_found() {
        let songs = vec![
            song("Aerosmith", "Dream On (Remastered)", "a.mp3"),
            song("Metallica & Michael Kamen", "Nothing Else Matters", "b.flac"),
            song("Metallica", "Nothing Else Matters", "c.ogg"),
            song("", "Seven Nation Army", "d.mp3"),
        ];
        assert_eq!(find(&songs, "Aerosmith", "Dream On").map(|s| s.path.as_path()), Some(Path::new("a.mp3")));
        assert_eq!(find(&songs, "Metallica", "Nothing else matters").map(|s| s.path.as_path()), Some(Path::new("c.ogg")), "the same artist first");
        assert_eq!(find(&songs, "The White Stripes", "Seven Nation Army").map(|s| s.path.as_path()), Some(Path::new("d.mp3")), "no artist in the file");
        assert_eq!(find(&songs, "Aerosmith", "Walk This Way"), None);
        assert_eq!(find(&songs, "Queen", "Dream On"), None, "another artist");
    }

    #[test]
    fn a_folder_is_read_with_its_subfolders_and_the_names_of_its_files() {
        let dir = std::env::temp_dir().join(format!("opengaragemusic-library-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Rock").join("70s")).unwrap();
        let samples = vec![0.0f32; 4410];
        og_engine::wav::write_wav(&dir.join("Rock").join("70s").join("Aerosmith - Dream On.wav"), &samples, 44_100, 1, 16).unwrap();
        og_engine::wav::write_wav(&dir.join("03 - Queen - Bohemian Rhapsody.wav"), &samples, 44_100, 1, 16).unwrap();
        std::fs::write(dir.join("notes.txt"), "not audio").unwrap();
        let songs = scan(&dir, &[]);
        let names: Vec<(&str, &str)> = songs.iter().map(|s| (s.artist.as_str(), s.title.as_str())).collect();
        assert_eq!(names, [("Queen", "Bohemian Rhapsody"), ("Aerosmith", "Dream On")]);
        assert!(find(&songs, "Aerosmith", "Dream On").is_some());
        // Read again, the files that did not change are taken from the index.
        let mut known = songs.clone();
        known[0].title = "From the index".to_owned();
        assert_eq!(scan(&dir, &known)[0].title, "From the index");
        let _ = std::fs::remove_dir_all(dir);
    }
}
