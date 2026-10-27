//! The tablature sites, as their descriptions say to read them (see
//! `og_engine::sites`): the descriptions shipped with the program, and the
//! ones in the `sites` folder next to it, which add to them or replace them.
//! Everything here that asks a site blocks on the network and is run in a
//! thread by `start_search` and `start_open`. A site that asks for an
//! account is read with the user's token (see `crate::login`), given to
//! every request of it.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc;

use std::path::Path;

use og_engine::guitar_pro::Score;
use og_engine::sites::{self, Description, Found, Record, MAX_PAGES};

use crate::downloads::{agent, encode_query};
use crate::i18n::tr;

/// What a browser asks for, so that the sites answer as they do to one.
const ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,application/json;q=0.8,*/*;q=0.7";
/// The largest answer taken from a site: a page, or a file of a few hundred kilobytes.
const MAX_BYTES: u64 = 20_000_000;

/// Where a description comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    /// Shipped with the program.
    BuiltIn,
    /// A file of the `sites` folder.
    File(PathBuf),
}

/// A site the program knows, and where its description comes from.
#[derive(Clone, Debug)]
pub struct Known {
    pub site: Description,
    pub source: Source,
}

/// The folder of the descriptions: `sites`, next to the program.
pub fn folder() -> Option<PathBuf> {
    crate::app::beside_program("sites")
}

/// The sites known: those shipped, in their order, then the ones of the
/// folder; a description of the folder that bears the name of one shipped
/// takes its place. Also what was wrong with the files that were not read.
pub fn load(folder: Option<&std::path::Path>) -> (Vec<Known>, Vec<String>) {
    let mut known: Vec<Known> = sites::BUILT_IN.iter().filter_map(|(_, text)| Description::parse(text).ok()).map(|site| Known { site, source: Source::BuiltIn }).collect();
    let mut problems = Vec::new();
    let mut files: Vec<PathBuf> = folder
        .and_then(|dir| std::fs::read_dir(dir).ok())
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
        .collect();
    files.sort();
    for path in files.into_iter().take(64) {
        let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let read = std::fs::read(&path).map_err(|e| e.to_string()).and_then(|bytes| {
            let text = String::from_utf8_lossy(&bytes);
            // Written for a newer program: said plainly, it is not a mistake in the file.
            match sites::format_of(&text) {
                Some(version) if version > sites::FORMAT_VERSION => {
                    Err(tf!("written for a newer version of OpenGarageMusic (description format {version}, this version reads up to {}); update the program to use it" [version], sites::FORMAT_VERSION))
                }
                _ => Description::parse(&text),
            }
        });
        match read {
            Ok(site) => match known.iter_mut().find(|k| k.site.name.eq_ignore_ascii_case(&site.name)) {
                Some(same) => *same = Known { site, source: Source::File(path) },
                None => known.push(Known { site, source: Source::File(path) }),
            },
            Err(e) => problems.push(tf!("“{name}”: {e}" [name, e])),
        }
    }
    (known, problems)
}

/// The bytes of an answer, or why there are none. Only secure addresses are
/// asked; `token` is the user's at a site that asks for an account.
fn bytes(site: &Description, url: &str, patience: u64, token: Option<&str>) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err(tf!("the description asks for “{url}”, which is not a secure address" [url]));
    }
    let mut request = agent(patience)?.get(url).set("Accept", ACCEPT).set("Accept-Language", "fr,en;q=0.8");
    for (name, value) in &site.headers {
        request = request.set(name, value);
    }
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    match request.call() {
        Ok(response) => {
            let mut out = Vec::new();
            response.into_reader().take(MAX_BYTES).read_to_end(&mut out).map_err(|e| tf!("unreadable answer ({e})" [e]))?;
            Ok(out)
        }
        Err(ureq::Error::Status(status, response)) => Err(crate::downloads::site_error(status, &response.into_string().unwrap_or_default())),
        Err(e) => Err(tf!("the site did not answer ({e})" [e])),
    }
}

/// The songs whose artist or title match `query`, in the language of `language`.
pub fn search(site: &Description, query: &str, language: &str, token: Option<&str>) -> Result<Vec<Found>, String> {
    let start = sites::query_vars(site, query, &encode_query(query.trim()));
    let mut lists = Vec::new();
    let mut first_error = None;
    for pipeline in &site.search {
        let mut records = vec![Record { vars: start.clone(), tracks: Vec::new() }];
        let mut failed = false;
        for step in &pipeline.steps {
            let mut next = Vec::new();
            let mut answered = false;
            for record in records.iter().take(MAX_PAGES) {
                match bytes(site, &sites::fill(&step.get, &record.vars), 30, token) {
                    Ok(page) => {
                        answered = true;
                        next.extend(sites::read_page(step, &site.text(&page), &record.vars));
                    }
                    Err(e) => {
                        first_error.get_or_insert(e);
                    }
                }
            }
            // A step that asked and got no answer at all ends the pipeline.
            if !answered && !records.is_empty() {
                failed = true;
                break;
            }
            records = next;
        }
        if !failed {
            lists.push(records);
        }
    }
    if lists.is_empty() {
        return Err(first_error.unwrap_or_else(|| tr("the site did not answer").to_owned()));
    }
    Ok(sites::merge(lists).into_iter().map(|record| site.found(record, language)).collect())
}

/// The files a site gave, each with the track it is for, when it is for one.
type Files = Vec<(Option<usize>, Result<Vec<u8>, String>)>;

/// A song read from a site: its score, and what is worth telling (tracks
/// that were not read).
#[derive(Debug)]
pub struct Opened {
    pub score: Score,
    pub messages: Vec<String>,
    /// The videos of the song the site gave, when they were asked for.
    pub videos: Vec<VideoOffer>,
    /// The words of the song, from its singing part.
    pub lyrics: og_engine::lyrics::Lyrics,
    /// The files of a site that gives captures or impulses, each with the
    /// name it is kept under; the score is empty then.
    pub files: Vec<(String, Vec<u8>)>,
}

/// A video of the song on YouTube, as a site gives it, with the second of the
/// video each bar starts at, in the order the bars are played.
#[derive(Clone, Debug, PartialEq)]
pub struct VideoOffer {
    pub id: String,
    pub label: String,
    pub points: Vec<f64>,
}

/// The videos a site lists for a song it opened; none when it says nothing
/// of them or cannot be asked.
fn videos(site: &Description, vars: &BTreeMap<String, String>) -> Vec<VideoOffer> {
    let Some(step) = &site.videos else { return Vec::new() };
    let Ok(page) = bytes(site, &sites::fill(&step.get, vars), 30, None) else { return Vec::new() };
    sites::read_page(step, &site.text(&page), vars)
        .into_iter()
        .filter_map(|record| {
            let id = record.vars.get("video")?.trim().to_owned();
            let points: Vec<f64> = serde_json::from_str(record.vars.get("points")?).ok()?;
            let valid = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') && points.len() >= 2;
            valid.then(|| VideoOffer { id, label: record.vars.get("label").cloned().unwrap_or_default(), points })
        })
        .take(16)
        .collect()
}

/// The file a site gave, taken out of its archive when it is one that holds
/// it: a Guitar Pro or MusicXML archive is the file itself.
fn file_in_archive(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    if !bytes.starts_with(b"PK\x03\x04") {
        return Ok(bytes);
    }
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).map_err(|e| tf!("unreadable archive ({e})" [e]))?;
    if zip.index_for_name("Content/score.gpif").is_some() || zip.index_for_name("META-INF/container.xml").is_some() {
        return Ok(bytes);
    }
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        if entry.is_dir() || !og_engine::guitar_pro::is_guitar_pro(std::path::Path::new(entry.name())) {
            continue;
        }
        let mut out = Vec::new();
        (&mut entry).take(MAX_BYTES).read_to_end(&mut out).map_err(|e| tf!("unreadable archive ({e})" [e]))?;
        return Ok(out);
    }
    Err(tr("the archive holds no Guitar Pro or MusicXML file").to_owned())
}

/// The name a file a site gives is kept under: the name the site gives it,
/// or the last part of its address, with the extension of the address when
/// the name has none, or the usual one of what the site gives.
fn file_name(vars: &BTreeMap<String, String>, url: &str, gives: &str) -> String {
    let segment = og_engine::tabsites::url_decode(url.split(['?', '#']).next().unwrap_or("").rsplit('/').next().unwrap_or(""));
    let base = vars.get("name").map(|n| n.trim()).filter(|n| !n.is_empty()).map_or_else(|| segment.clone(), str::to_owned);
    let base = if base.is_empty() { "file".to_owned() } else { base };
    let known = ["nam", "json", "wav", "flac"];
    let extension = |name: &str| Path::new(name).extension().map(|e| e.to_string_lossy().to_lowercase()).filter(|e| known.contains(&e.as_str()));
    if extension(&base).is_some() {
        return base;
    }
    let usual = if gives == "impulse" { "wav" } else { "nam" };
    format!("{base}.{}", extension(&segment).unwrap_or_else(|| usual.to_owned()))
}

/// Reads the song a search found: follows the description to its file and
/// reads it. With the tracks the site listed, only the ticked ones are
/// fetched, by their place in the list. On a site that gives files, the
/// files of the one chosen are fetched instead.
pub fn open(site: &Description, found: &Found, tracks: &[usize], with_videos: bool, words_from: Option<usize>, token: Option<&str>) -> Result<Opened, String> {
    // The singing part is fetched for its words, ticked or not.
    let words_only = words_from.filter(|t| !tracks.contains(t));
    let fetched: Vec<usize> = tracks.iter().copied().chain(words_only).collect();
    let mut vars: BTreeMap<String, String> = found.vars.clone();
    vars.insert("home".to_owned(), site.home.clone());
    let mut files: Files = Vec::new();
    // The names of the files, on a site that gives them.
    let mut names: Vec<String> = Vec::new();
    for step in &site.open {
        if let Some(get) = &step.get {
            let page = bytes(site, &sites::fill(get, &vars), 30, token)?;
            if step.items.is_some() {
                // The page lists the files: each is fetched.
                let Some(download) = &step.download else { continue };
                for item in sites::read_open_items(step, &site.text(&page), &vars).into_iter().take(64) {
                    let url = sites::fill(download, &item);
                    names.push(file_name(&item, &url, &site.gives));
                    files.push((None, bytes(site, &url, 60, token)));
                }
                continue;
            }
            vars = sites::read_open_page(step, &site.text(&page), &vars, sites::open_step_reads_json(step));
        }
        let Some(download) = &step.download else { continue };
        let wanted: Vec<Option<usize>> = if step.each_track { fetched.iter().map(|t| Some(*t)).collect() } else { vec![None] };
        for track in wanted.into_iter().take(64) {
            if let Some(track) = track {
                vars.insert("track".to_owned(), track.to_string());
            }
            // The other values of a variable are tried in turn when one fails.
            let tries: Vec<BTreeMap<String, String>> = match step.alternatives.iter().next() {
                Some((name, values)) => values.iter().take(8).map(|value| {
                    let mut tried = vars.clone();
                    tried.insert(name.clone(), value.clone());
                    tried
                }).collect(),
                None => vec![vars.clone()],
            };
            let mut result = Err(tr("the site did not give the file").to_owned());
            for tried in tries {
                let url = sites::fill(download, &tried);
                if url.trim_end_matches('/') == site.home || url.ends_with("://") {
                    result = Err(tr("the page offers no file").to_owned());
                    break;
                }
                result = bytes(site, &url, 60, token);
                if result.is_ok() {
                    names.push(file_name(&tried, &url, &site.gives));
                    break;
                }
            }
            files.push((track, result));
        }
    }
    let title = vars.get("title").cloned().unwrap_or_default();
    let artist = vars.get("artist").cloned().unwrap_or_default();
    match site.reader.as_str() {
        "capture" | "impulse" => {
            let (mut kept, mut messages) = (Vec::new(), Vec::new());
            for (name, (_, file)) in names.into_iter().zip(files) {
                match file {
                    Ok(bytes) if !bytes.is_empty() => kept.push((name, bytes)),
                    Ok(_) => {
                        let e = tr("the file is empty");
                        messages.push(tf!("“{name}” was not read: {e}" [name, e]));
                    }
                    Err(e) => messages.push(tf!("“{name}” was not read: {e}" [name, e])),
                }
            }
            if kept.is_empty() {
                return Err(messages.pop().unwrap_or_else(|| tr("the site did not give the file").to_owned()));
            }
            let score = Score { title: String::new(), artist: String::new(), parts: Vec::new() };
            Ok(Opened { score, messages, videos: Vec::new(), lyrics: Default::default(), files: kept })
        }
        "songsterr" => {
            let (mut parts, mut messages, mut words) = (Vec::new(), Vec::new(), Vec::new());
            for (track, file) in files {
                let name = track.and_then(|t| found.tracks.get(t)).map_or_else(String::new, |t| t.name.clone());
                match file.and_then(|bytes| og_engine::songsterr::parse_part(&String::from_utf8_lossy(&bytes))) {
                    // Fetched for its words only.
                    Ok(part) if track.is_some() && track == words_only => words = part.lyrics,
                    Ok(part) => parts.push(part),
                    Err(_) if track.is_some() && track == words_only => {}
                    Err(e) => messages.push(tf!("“{name}” was not read: {e}" [name, e])),
                }
            }
            if parts.is_empty() {
                return Err(messages.pop().unwrap_or_else(|| tr("no part was chosen").to_owned()));
            }
            let videos = if with_videos { videos(site, &vars) } else { Vec::new() };
            let lyrics = match words.is_empty() {
                true => og_engine::lyrics::from_parts(&parts),
                false => og_engine::lyrics::Lyrics { sung: words, source: "tablature".to_owned(), ..Default::default() },
            };
            Ok(Opened { score: Score { title, artist, parts }, messages, videos, lyrics, files: Vec::new() })
        }
        _ => {
            let (_, file) = files.into_iter().next().ok_or_else(|| tr("the site did not give the file").to_owned())?;
            let file = file_in_archive(file?)?;
            if file.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<') && !og_engine::guitar_pro::is_score_text(&file) {
                return Err(tr("the site did not give the file").to_owned());
            }
            let mut score = og_engine::guitar_pro::parse(&file)?;
            if score.title.trim().is_empty() {
                score.title = title;
            }
            if score.artist.trim().is_empty() {
                score.artist = artist;
            }
            let videos = if with_videos { videos(site, &vars) } else { Vec::new() };
            let lyrics = og_engine::lyrics::from_parts(&score.parts);
            Ok(Opened { score, messages: Vec::new(), videos, lyrics, files: Vec::new() })
        }
    }
}

pub fn start_search(site: Description, query: String, language: String, token: Option<String>) -> mpsc::Receiver<Result<Vec<Found>, String>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(search(&site, &query, &language, token.as_deref()));
    });
    receive
}

pub fn start_open(site: Description, found: Found, tracks: Vec<usize>, with_videos: bool, words_from: Option<usize>, token: Option<String>) -> mpsc::Receiver<Result<Opened, String>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(open(&site, &found, &tracks, with_videos, words_from, token.as_deref()));
    });
    receive
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sites_shipped_come_first_and_a_folder_adds_or_replaces() {
        let dir = std::env::temp_dir().join(format!("opengaragemusic-sites-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (known, problems) = load(Some(&dir));
        let names: Vec<&str> = known.iter().map(|k| k.site.name.as_str()).collect();
        assert_eq!(names, ["Songsterr", "Guitariff", "Tablature Hub", "Ultimate Guitar", "TONE3000", "TONE3000 IR", "GuitarML"]);
        assert!(problems.is_empty() && known.iter().all(|k| k.source == Source::BuiltIn));
        let mine = r#"{"name": "Mon site", "home": "https://tabs.example.org", "search": [{"steps": [{"get": "{home}/?q={query}", "items": {"regex": "<a href=\"([^\"]+)\">(.*?)</a>"}, "fields": {"url": {"regex": "href=\"([^\"]+)\"", "absolute": true}, "title": {"regex": ">(.*?)<"}}}]}], "open": [{"download": "{url}"}]}"#;
        std::fs::write(dir.join("mon-site.json"), mine).unwrap();
        std::fs::write(dir.join("guitariff.json"), mine.replace("Mon site", "guitariff")).unwrap();
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();
        std::fs::write(dir.join("notes.txt"), "not a description").unwrap();
        let (known, problems) = load(Some(&dir));
        let names: Vec<&str> = known.iter().map(|k| k.site.name.as_str()).collect();
        assert_eq!(names, ["Songsterr", "guitariff", "Tablature Hub", "Ultimate Guitar", "TONE3000", "TONE3000 IR", "GuitarML", "Mon site"], "the folder's Guitariff replaces the one shipped");
        assert!(matches!(&known[1].source, Source::File(path) if path.ends_with("guitariff.json")));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("broken.json"));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(load(None).0.len(), 7);
    }

    #[test]
    fn files_come_out_of_their_archives_unless_they_are_one() {
        use std::io::Write;
        let options = zip::write::SimpleFileOptions::default();
        let zipped = |entries: &[(&str, &[u8])]| {
            let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            for (name, bytes) in entries {
                archive.start_file(*name, options).unwrap();
                archive.write_all(bytes).unwrap();
            }
            archive.finish().unwrap().into_inner()
        };
        let gp4 = b"\x18FICHIER GUITAR PRO v4.06".to_vec();
        assert_eq!(file_in_archive(zipped(&[("lisez-moi.txt", b"un texte"), ("Abc - Song.GP4", &gp4)])).unwrap(), gp4);
        let gp7 = zipped(&[("Content/score.gpif", b"<GPIF/>")]);
        assert_eq!(file_in_archive(gp7.clone()).unwrap(), gp7, "a GP7 file is an archive itself");
        let mxl = zipped(&[("META-INF/container.xml", b"<container/>")]);
        assert_eq!(file_in_archive(mxl.clone()).unwrap(), mxl);
        assert!(file_in_archive(zipped(&[("notes.txt", b"x")])).unwrap_err().contains("Guitar Pro"));
        assert_eq!(file_in_archive(b"plain".to_vec()).unwrap(), b"plain");
        assert!(file_in_archive(b"PK\x03\x04broken".to_vec()).is_err());
    }

    #[test]
    fn only_secure_addresses_are_asked() {
        let site = Description { name: "A".into(), home: "https://a.example".into(), ..Description::default() };
        assert!(bytes(&site, "http://a.example/x", 5, None).unwrap_err().contains("http://a.example/x"));
        assert!(bytes(&site, "file:///c:/x", 5, Some("t")).unwrap_err().contains("file:///c:/x"));
        // The names the files of a site are kept under.
        let vars = |name: &str| BTreeMap::from([("name".to_owned(), name.to_owned())]);
        assert_eq!(file_name(&vars("Standard"), "https://cdn.example/a/b/model-1.nam?x=1", "capture"), "Standard.nam");
        assert_eq!(file_name(&vars("Lite.nam"), "https://cdn.example/x", "capture"), "Lite.nam");
        assert_eq!(file_name(&vars(""), "https://cdn.example/Barber%20GC.json", "capture"), "Barber GC.json");
        assert_eq!(file_name(&vars(" "), "https://cdn.example/", "impulse"), "file.wav");
        assert_eq!(file_name(&vars("Room"), "https://cdn.example/ir/room.WAV", "impulse"), "Room.wav");
    }

    /// Talks to the real GuitarML site, which needs no account. Run on demand:
    /// `dev test -p opengaragemusic -- --ignored live_capture_site --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn live_capture_site_lists_its_captures_and_gives_one() {
        let (known, _) = load(None);
        let site = &known.iter().find(|k| k.site.name == "GuitarML").unwrap().site;
        let found = search(site, "", "fr", None).unwrap();
        println!("GuitarML: {} captures, the first {:?}", found.len(), found.first().map(|f| &f.label));
        assert!(found.len() > 10);
        let sifted = search(site, "barber", "fr", None).unwrap();
        assert!(!sifted.is_empty() && sifted.len() < found.len());
        let opened = open(site, &sifted[0], &[], false, None, None).unwrap();
        assert_eq!(opened.files.len(), 1);
        let (name, bytes) = &opened.files[0];
        println!("  {name}: {} bytes", bytes.len());
        assert!(name.ends_with(".json"));
        let capture = og_engine::capture::Capture::parse(&String::from_utf8_lossy(bytes), name).unwrap();
        assert_eq!(capture.about.format, "Proteus");
    }

    /// Talks to the real sites, with the descriptions shipped. Run on demand:
    /// `dev test -p opengaragemusic -- --ignored live_sites_described --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn live_sites_described_answer_a_search_and_give_a_song() {
        let (known, _) = load(None);
        for known in known.iter().filter(|k| !k.site.open_only && k.site.is_tablature()) {
            let site = &known.site;
            let query = if site.name == "Songsterr" { "nothing else matters" } else { "hallelujah" };
            let found = search(site, query, "fr", None).unwrap();
            println!("{}: {} songs, the first {:?} ({})", site.name, found.len(), found.first().map(|f| &f.label), found.first().map_or("", |f| f.badge.as_str()));
            assert!(!found.is_empty(), "{}", site.name);
            let song = found.iter().find(|f| f.tracks.len() > 1).unwrap_or(&found[0]);
            let opened = open(site, song, &[0, 1], true, song.tracks.iter().position(|t| t.vocal), None).unwrap();
            println!("  words: {} syllables", opened.lyrics.sung.len());
            println!("  {}: {} parts, {} notes; {:?}", song.label, opened.score.parts.len(), opened.score.parts.iter().map(|p| p.notes.len()).sum::<usize>(), opened.messages);
            let bars = opened.score.parts.first().map_or(0, |p| p.bar_starts.len());
            for video in &opened.videos {
                println!("  video {} ({}): {} points for {bars} bars", video.id, video.label, video.points.len());
            }
            if site.videos.is_some() {
                assert!(opened.videos.iter().all(|v| v.points.len().abs_diff(bars) <= 1), "the points of the videos are those of the bars played");
            }
            assert!(!opened.score.parts.is_empty());
        }
    }
}
