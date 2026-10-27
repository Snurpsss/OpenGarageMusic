//! The window of the tablature sites, driven headlessly: the site chosen
//! and agreed to, the search, the tracks the site lists or the parts of the
//! file read, and the project made of the ticked ones. Nothing here waits
//! on the network: the threads' answers are sent through the channels the
//! window polls.

use std::collections::BTreeMap;

use og_engine::guitar_pro::Score;
use og_engine::sites::{Found, TrackEntry};
use og_engine::songsterr::{Part, DRUMS};
use og_engine::Note;

use crate::app::Dialog;
use crate::sites::Opened;
use crate::tests::Harness;

const SONGSTERR: usize = 0;
const GUITARIFF: usize = 1;
const TABLATURE_HUB: usize = 2;
const TONE3000: usize = 4;

/// A song found: on a site that lists its tracks when they are given.
/// Its address leads nowhere, so that a reading started by a test fails at once.
fn found(site: &str, title: &str, badge: &str, tracks: &[(&str, bool)]) -> Found {
    let vars = BTreeMap::from([
        ("artist".to_owned(), "Abc".to_owned()),
        ("title".to_owned(), title.to_owned()),
        ("id".to_owned(), "1".to_owned()),
        ("url".to_owned(), "https://127.0.0.1:9/nothing".to_owned()),
    ]);
    let tracks = tracks.iter().map(|(name, drums)| TrackEntry { name: (*name).to_owned(), detail: "Guitar".to_owned(), drums: *drums, vocal: name.contains("Chant") }).collect();
    Found { site: site.to_owned(), label: format!("Abc – {title}"), badge: badge.to_owned(), vars, tracks }
}

fn part(name: &str, instrument_id: u32, tuning: &[u8]) -> Part {
    Part {
        name: name.to_owned(),
        instrument_id,
        notes: vec![Note { start: 0.0, length: 1.0, pitch: 64, velocity: 96, string: None, ..Default::default() }, Note { start: 4.0, length: 1.0, pitch: 67, velocity: 96, string: None, ..Default::default() }],
        length: 8.0,
        tempo: vec![og_engine::Point { beat: 0.0, value: 120.0 }],
        signature: (4, 4),
        key: None,
        markers: Vec::new(),
        tuning: tuning.to_vec(),
        chords: Vec::new(),
        swing: Vec::new(),
        bar_starts: vec![0.0, 4.0],
        lyrics: Vec::new(),
    }
}

fn score() -> Score {
    Score {
        title: "Song".to_owned(),
        artist: "Abc".to_owned(),
        parts: vec![part("Guitare", 30, &[64, 59, 55, 50, 45, 40]), part("Basse", 33, &[43, 38, 33, 28]), part("", DRUMS, &[])],
    }
}

/// A harness with the window open on a site the user agreed to.
fn window_on(site: usize) -> Harness {
    let mut h = Harness::new();
    h.app.open_sites();
    h.app.sites_choose(site);
    h.app.confirm_site();
    h.settle();
    h
}

#[test]
fn the_window_opens_on_the_site_asked() {
    let mut h = Harness::new();
    let names: Vec<&str> = h.app.known_sites.iter().map(|k| k.site.name.as_str()).collect();
    assert_eq!(names, ["Songsterr", "Guitariff", "Tablature Hub", "Ultimate Guitar", "TONE3000", "TONE3000 IR", "GuitarML"]);
    assert!(h.app.known_sites[3].site.open_only, "Ultimate Guitar is only opened");
    // The Songsterr entry opens the one window, on Songsterr.
    h.app.open_site_named("songsterr");
    h.settle();
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)));
    assert_eq!(h.app.sites.site, SONGSTERR);
    // The sites entry opens it on the site last searched.
    h.app.dialog = None;
    h.app.sites.found = Some(Ok(vec![found("Guitariff", "Song", "GP4", &[])]));
    h.app.sites_choose(GUITARIFF);
    assert!(h.app.sites.found.is_none(), "another site: the results of the first are forgotten");
    h.app.sites.found = Some(Ok(vec![found("Guitariff", "Song", "GP4", &[])]));
    h.app.sites_choose(GUITARIFF);
    assert!(h.app.sites.found.is_some(), "the same site: nothing changes");
    h.app.sites_choose(99);
    assert_eq!(h.app.sites.site, GUITARIFF, "a site that is not there changes nothing");
    h.app.open_sites();
    h.settle();
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)));
    assert_eq!(h.app.sites.site, GUITARIFF);
    for site in [SONGSTERR, GUITARIFF, TABLATURE_HUB] {
        h.app.sites_choose(site);
        h.settle();
        assert!(matches!(h.app.dialog, Some(Dialog::Sites)), "the window draws for site {site}");
    }
}

#[test]
fn a_site_is_read_only_once_the_user_agreed_to_it() {
    let mut h = Harness::new();
    h.app.open_sites();
    h.app.sites_choose(TABLATURE_HUB);
    h.settle();
    assert!(!h.app.site_confirmed());
    h.app.sites.search = "hallelujah".to_owned();
    h.app.sites_search();
    assert!(h.app.sites.finding.is_none(), "nothing is asked before the user agrees");
    h.app.confirm_site();
    assert!(h.app.site_confirmed());
    assert_eq!(h.app.settings.sites_confirmed, ["Tablature Hub"]);
    h.app.confirm_site();
    assert_eq!(h.app.settings.sites_confirmed.len(), 1, "agreed once");
    // Agreeing to one site is not agreeing to the others.
    h.app.sites_choose(GUITARIFF);
    assert!(!h.app.site_confirmed());
    h.settle();
    // A search on a site agreed to starts; one of nothing does not.
    h.app.sites_choose(TABLATURE_HUB);
    h.app.sites.search = "  ".to_owned();
    h.app.sites_search();
    assert!(h.app.sites.finding.is_none());
}

#[test]
fn a_guitar_pro_site_search_draws_in_every_state() {
    let mut h = window_on(TABLATURE_HUB);
    // A search in progress; then a failure, then nothing, then results.
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.search = "hallelujah".to_owned();
    h.app.sites.finding = Some(receive);
    h.settle();
    assert!(h.app.sites.busy());
    send.send(Err("pas de réseau".to_owned())).unwrap();
    h.settle();
    assert!(matches!(&h.app.sites.found, Some(Err(e)) if e == "pas de réseau"));
    assert!(h.app.sites.finding.is_none() && !h.app.sites.busy());
    h.app.sites.found = Some(Ok(Vec::new()));
    h.settle();
    h.app.sites.found = Some(Ok(vec![found("Tablature Hub", "Hallelujah (3)", "GP3", &[]), found("Tablature Hub", "Hallelujah", "", &[])]));
    h.app.sites.selected = Some(1);
    h.settle();
    // A search that stops without answering is an error, not a spinner for ever.
    let (send, receive) = std::sync::mpsc::channel::<Result<Vec<Found>, String>>();
    h.app.sites.finding = Some(receive);
    drop(send);
    h.settle();
    assert!(matches!(&h.app.sites.found, Some(Err(_))));
    assert!(h.app.sites.selected.is_none() && h.app.sites.chosen.is_empty());
    // The file of the chosen tablature is being read; then it was not: the
    // window says so and the list stays.
    h.app.sites.found = Some(Ok(vec![found("Tablature Hub", "Hallelujah (3)", "GP3", &[])]));
    h.app.sites_select(0);
    assert!(h.app.sites.opening.is_some(), "the file of a song without tracks listed is read at once");
    let (send, receive) = std::sync::mpsc::channel::<Result<Opened, String>>();
    h.app.sites.opening = Some(receive);
    h.settle();
    assert!(h.app.sites.busy());
    send.send(Err("le site n'a pas donné le fichier".to_owned())).unwrap();
    h.settle();
    assert!(h.app.sites.error && h.app.sites.message.contains("le site n'a pas donné le fichier"), "{}", h.app.sites.message);
    assert!(h.app.sites.opening.is_none() && h.app.sites.score.is_none());
    assert!(matches!(&h.app.sites.found, Some(Ok(list)) if list.len() == 1), "the list stays");
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)));
    // The right column shows the error under the song, with a Retry button.
    h.settle();
    assert!(h.app.sites.error, "the error stays until a new reading");
    // A reading thread that dies is an error too.
    let (send, receive) = std::sync::mpsc::channel::<Result<Opened, String>>();
    h.app.sites.opening = Some(receive);
    drop(send);
    h.settle();
    assert!(h.app.sites.error && h.app.sites.opening.is_none());
    // Choosing a site again clears all of it but the text searched.
    h.app.sites_choose(GUITARIFF);
    assert!(h.app.sites.found.is_none() && h.app.sites.message.is_empty() && !h.app.sites.error);
    assert_eq!(h.app.sites.search, "hallelujah");
    h.settle();
}

#[test]
fn the_parts_of_the_file_read_make_the_project_of_the_ticked_ones() {
    let mut h = window_on(GUITARIFF);
    h.app.sites.found = Some(Ok(vec![found("Guitariff", "Song", "GP4", &[])]));
    h.app.sites_select(0);
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.opening = Some(receive);
    h.settle();
    // Nothing ticked yet: no project is made.
    let before = h.app.project.clone();
    h.app.sites_create();
    assert_eq!(h.app.project, before);
    send.send(Ok(Opened { score: score(), messages: Vec::new(), videos: Vec::new(), lyrics: Default::default(), files: Vec::new() })).unwrap();
    h.settle();
    assert_eq!(h.app.sites.chosen, [true, true, true], "every part ticked once the file is read");
    assert_eq!(h.app.sites.score.as_ref().map(|s| s.parts.len()), Some(3));
    assert!(h.app.sites.message.is_empty() && !h.app.sites.error);
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)), "the window waits for the choice of the parts");
    // The bass left out: the project holds the guitar and the drums, with
    // their names and tuning, and the window closes.
    h.app.sites.chosen[1] = false;
    h.settle();
    h.app.sites_create();
    assert!(h.app.dialog.is_none());
    assert_ne!(h.app.project, before);
    assert_eq!(h.app.project.name, "Abc – Song");
    assert_eq!(h.app.project.tempo, 120.0);
    assert_eq!(h.app.project.tracks.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["Guitare", "Part 2"], "the parts kept are numbered in the project");
    assert_eq!(h.app.project.tracks[0].tuning, [64, 59, 55, 50, 45, 40]);
    assert!(h.app.project.tracks[1].tuning.is_empty());
    assert_eq!(h.app.project.tracks[0].patch, "Classic lead");
    assert_eq!(h.app.project.tracks.iter().map(|t| t.icon).collect::<Vec<_>>(), [3, 1]);
    assert!(h.app.status.contains("Abc – Song") && h.app.status.contains("2 ") && h.app.status.contains("Guitariff"), "{}", h.app.status);
    assert!(!h.app.is_modified(), "a project just made has nothing to lose");
    assert!(h.app.path.is_none());
    // Nothing ticked: the button does nothing.
    h.app.sites.found = Some(Ok(vec![found("Guitariff", "Song", "GP4", &[])]));
    h.app.sites.selected = Some(0);
    h.app.sites.score = Some(score());
    h.app.sites.chosen = vec![false; 3];
    let made = h.app.project.clone();
    h.app.sites_create();
    assert_eq!(h.app.project, made);
}

#[test]
fn the_tracks_a_site_lists_are_read_once_ticked() {
    let mut h = window_on(SONGSTERR);
    // Its description, pointed nowhere: the readings the test starts fail at once.
    h.app.known_sites[SONGSTERR].site.home = "https://127.0.0.1:9".to_owned();
    let songs = vec![found("Songsterr", "Nothing Else Matters", "3 pistes", &[("Lead Guitar", false), ("Bass", false), ("", true)]), found("Songsterr", "One", "1 pistes", &[("Guitar", false)])];
    h.app.sites.found = Some(Ok(songs));
    h.settle();
    // Choosing a song ticks its tracks at once, and reads nothing yet.
    h.app.sites_select(0);
    assert_eq!(h.app.sites.chosen, [true, true, true]);
    assert!(h.app.sites.opening.is_none());
    h.app.sites.chosen[1] = false;
    h.settle();
    h.app.sites_select(5);
    assert_eq!(h.app.sites.selected, Some(0), "a song that is not there changes nothing");
    // Creating the project reads the ticked tracks; a failure says so and the window stays.
    h.app.sites_create();
    assert!(h.app.sites.opening.is_some() && h.app.sites.create_when_read);
    let (send, receive) = std::sync::mpsc::channel::<Result<Opened, String>>();
    h.app.sites.opening = Some(receive);
    h.settle();
    send.send(Err("rien".to_owned())).unwrap();
    h.settle();
    assert!(h.app.sites.error && h.app.sites.message.contains("rien"), "{}", h.app.sites.message);
    assert!(!h.app.sites.create_when_read && matches!(h.app.dialog, Some(Dialog::Sites)));
    // Read: the project is made at once, and the status tells what was left out.
    let before = h.app.project.clone();
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.opening = Some(receive);
    h.app.sites.create_when_read = true;
    let one = Score { title: "One".to_owned(), artist: "Metallica".to_owned(), parts: vec![Part { tempo: vec![og_engine::Point { beat: 0.0, value: 100.0 }], ..part("Guitar", 30, &[]) }] };
    send.send(Ok(Opened { score: one, messages: vec!["Une partie manque.".to_owned()], videos: Vec::new(), lyrics: Default::default(), files: Vec::new() })).unwrap();
    h.settle();
    assert!(h.app.dialog.is_none());
    assert_ne!(h.app.project, before);
    assert_eq!((h.app.project.name.as_str(), h.app.project.tempo, h.app.project.tracks.len()), ("Metallica – One", 100.0, 1));
    assert_eq!(h.app.project.tracks[0].patch, "Classic lead");
    assert!(h.app.status.contains("1 pistes") && h.app.status.contains("Songsterr") && h.app.status.contains("Une partie manque."), "{}", h.app.status);
    assert!(h.app.status.contains("banque"), "built-in sounds stand in: {}", h.app.status);
    assert!(!h.app.is_modified(), "a project just made has nothing to lose");
    // On a site without results, choosing does nothing.
    h.app.sites_choose(TABLATURE_HUB);
    h.app.sites_select(0);
    assert!(h.app.sites.selected.is_none() && h.app.sites.opening.is_none(), "no result: nothing to read");
    h.settle();
}

#[test]
fn a_description_written_for_a_newer_program_is_named_and_not_read() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-sites-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let site = |version: u32, name: &str| format!(r#"{{"format_version": {version}, "name": "{name}", "home": "https://a.b", "search": [{{"steps": [{{"get": "{{home}}"}}]}}], "open": [{{"download": "{{url}}"}}]}}"#);
    std::fs::write(dir.join("ancien.json"), site(og_engine::sites::FORMAT_VERSION, "Ancien")).unwrap();
    std::fs::write(dir.join("futur.json"), site(og_engine::sites::FORMAT_VERSION + 1, "Futur")).unwrap();
    let (known, problems) = crate::sites::load(Some(&dir));
    assert!(known.iter().any(|k| k.site.name == "Ancien") && !known.iter().any(|k| k.site.name == "Futur"));
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("futur.json") && problems[0].contains("OpenGarageMusic") && problems[0].contains(&(og_engine::sites::FORMAT_VERSION + 1).to_string()), "{}", problems[0]);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_song_of_songsterr_comes_with_its_video_in_step() {
    let mut h = window_on(SONGSTERR);
    h.app.sites.found = Some(Ok(vec![found("Songsterr", "Song", "3 tracks", &[("Guitare", false), ("Basse", false), ("Drums", true)])]));
    h.app.sites_select(0);
    h.settle();
    assert!(!h.app.sites.want_video, "asked for only when ticked");
    h.app.sites.want_video = true;
    h.settle();
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.opening = Some(receive);
    h.app.sites.create_when_read = true;
    let offers = vec![
        crate::sites::VideoOffer { id: "alternatif1".to_owned(), label: "alternative".to_owned(), points: vec![0.3, 2.3, 4.3] },
        crate::sites::VideoOffer { id: "original01".to_owned(), label: String::new(), points: vec![1.0, 3.0, 5.0] },
    ];
    send.send(Ok(Opened { score: score(), messages: Vec::new(), videos: offers, lyrics: Default::default(), files: Vec::new() })).unwrap();
    h.settle();
    assert!(h.app.dialog.is_none());
    // The original video, as a track of its own, in step bar by bar.
    let video = h.app.project.tracks.last().unwrap();
    assert_eq!((video.kind, video.video.as_str(), video.lead), (og_engine::TrackKind::Video, "original01", true));
    assert_eq!(video.timing, [og_engine::Timing { beat: 0.0, sec: 1.0 }, og_engine::Timing { beat: 4.0, sec: 3.0 }]);
    assert!(h.app.has_recording() && h.app.project.follow, "played with the video at once");
    // With the original sound: at the pace of the video, the tracks silent.
    let played = og_engine::recording::as_played(&h.app.project);
    assert_eq!(played.tempo, 120.0);
    assert!(played.tracks.iter().all(|t| t.mute));
    h.settle();
    // The other video is kept, and can be played instead; the one left keeps
    // its shift for coming back to it.
    let ti = h.app.project.tracks.len() - 1;
    assert_eq!(h.app.project.tracks[ti].videos.len(), 2);
    h.app.shift_recording(ti, 0.5);
    h.app.choose_video(ti, 0);
    let video = &h.app.project.tracks[ti];
    assert_eq!(video.video, "alternatif1");
    assert_eq!(video.timing, [og_engine::Timing { beat: 0.0, sec: 0.3 }, og_engine::Timing { beat: 4.0, sec: 2.3 }]);
    h.app.choose_video(ti, 1);
    let video = &h.app.project.tracks[ti];
    assert_eq!((video.video.as_str(), video.timing[0].sec), ("original01", 1.5));
    // Saved with the project.
    let saved: og_engine::Project = serde_json::from_str(&serde_json::to_string(&h.app.project).unwrap()).unwrap();
    assert_eq!(saved.tracks[ti].videos, h.app.project.tracks[ti].videos);
    h.settle();
}

#[test]
fn the_recording_of_a_song_is_found_in_the_music_library() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-music-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("Abc")).unwrap();
    og_engine::wav::write_wav(&dir.join("Abc").join("Abc - Song (Remastered).wav"), &[0.0f32; 4410], 44_100, 1, 16).unwrap();
    let mut h = window_on(GUITARIFF);
    h.app.settings.music_folder = dir.to_string_lossy().into_owned();
    h.app.scan_library();
    while h.app.library_scan.is_some() {
        std::thread::sleep(std::time::Duration::from_millis(10));
        h.app.poll_library();
    }
    assert_eq!(h.app.library.len(), 1);
    h.app.sites.found = Some(Ok(vec![found("Guitariff", "Song", "GP4", &[]), found("Guitariff", "Other", "GP4", &[])]));
    h.app.sites_select(0);
    h.settle();
    assert!(h.app.sites.use_recording && h.app.sites.recording.as_ref().is_some_and(|p| p.ends_with("Abc - Song (Remastered).wav")));
    h.app.sites_select(1);
    assert!(h.app.sites.recording.is_none() && !h.app.sites.use_recording, "not for another song");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_words_come_with_the_song_or_are_looked_for() {
    use og_engine::lyrics::{Lyrics, Syllable};
    // A song whose singing part is not ticked: its words come all the same.
    let mut h = window_on(SONGSTERR);
    h.app.sites.found = Some(Ok(vec![found("Songsterr", "Song", "2 tracks", &[("Guitare", false), ("Chant", false)])]));
    h.app.sites_select(0);
    assert!(h.app.sites.want_words, "asked for at first");
    h.app.sites.chosen = vec![true, false];
    h.settle();
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.opening = Some(receive);
    h.app.sites.create_when_read = true;
    let words = Lyrics { sung: vec![Syllable { beat: 0.0, text: "La".into(), line_start: true }, Syllable { beat: 1.0, text: "la".into(), line_start: false }], source: "tablature".into(), ..Lyrics::default() };
    send.send(Ok(Opened { score: score(), messages: Vec::new(), videos: Vec::new(), lyrics: words.clone(), files: Vec::new() })).unwrap();
    h.settle();
    assert_eq!(h.app.project.lyrics, words);
    assert!(h.app.words_search.is_none());

    // A song without words: LRCLIB is asked.
    let mut h = window_on(GUITARIFF);
    h.app.sites.found = Some(Ok(vec![found("Guitariff", "Song", "GP4", &[])]));
    h.app.sites_select(0);
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.opening = Some(receive);
    send.send(Ok(Opened { score: score(), messages: Vec::new(), videos: Vec::new(), lyrics: Lyrics::default(), files: Vec::new() })).unwrap();
    h.settle();
    h.app.sites_create();
    h.settle();
    assert!(h.app.project.lyrics.is_empty() && h.app.words_search.is_some(), "looked for on LRCLIB");
}

#[test]
fn a_song_of_the_set_list_gets_its_project_from_the_site() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-setlist-site-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut h = window_on(SONGSTERR);
    h.app.set_data_dir_for_test(&dir);
    h.app.dialog = None;
    h.app.settings.set_list.songs = vec![og_engine::setlist::SetSong { number: 3, artist: "Abc".to_owned(), title: "Song".to_owned(), ..Default::default() }];
    h.app.open_set_list();
    assert!(!h.app.find_set_project(0), "the user chooses the tablature");
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)));
    assert_eq!(h.app.sites.search, "Abc Song");
    assert!(h.app.sites.finding.is_some(), "searched at once on a site agreed to");
    // The site answers, the user chooses the song and makes the project.
    h.app.sites.finding = None;
    h.app.sites.found = Some(Ok(vec![found("Songsterr", "Song", "3 tracks", &[("Guitare", false), ("Basse", false)])]));
    h.app.sites_select(0);
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.opening = Some(receive);
    h.app.sites.create_when_read = true;
    send.send(Ok(Opened { score: score(), messages: Vec::new(), videos: Vec::new(), lyrics: Default::default(), files: Vec::new() })).unwrap();
    h.settle();
    // Saved once its words have come (the tests ask nothing of LRCLIB).
    assert_eq!(h.app.setlist.saving, Some(0));
    h.app.words_search = None;
    h.settle();
    let linked = std::path::PathBuf::from(&h.app.settings.set_list.songs[0].project);
    assert_eq!(linked, h.app.set_list_folder().join("Abc - Song.ogproj"));
    assert!(linked.is_file());
    assert_eq!(h.app.path.as_deref(), Some(linked.as_path()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_site_that_gives_captures_lists_files_and_downloads_them_to_the_library() {
    let dir = crate::tests::temp_folder("sites-captures");
    let mut h = window_on(TONE3000);
    h.app.set_data_dir_for_test(&dir);
    // Nothing is asked of the site: the key of the program first, then the sign-in.
    let site = h.app.site().unwrap().clone();
    assert!((site.gives.as_str(), site.lists_only(), site.login.is_some()) == ("capture", true, true));
    assert!(h.app.site_key().is_empty() && !h.app.site_signed_in());
    h.app.sites.search.clear();
    h.app.sites_search();
    assert!(h.app.sites.finding.is_none(), "not without the user's account");
    h.app.sites.key_text = " t3k_pk_x ".to_owned();
    h.app.sites_keep_key();
    assert_eq!(h.app.site_key(), "t3k_pk_x");
    assert_eq!(h.app.settings.site_keys.get("TONE3000").map(String::as_str), Some("t3k_pk_x"));
    let token = crate::login::SiteToken { access: "a".to_owned(), refresh: "r".to_owned(), expires: crate::sessions::now() + 3600 };
    h.app.settings.site_logins.insert("TONE3000".to_owned(), token);
    assert!(h.app.site_signed_in() && h.app.site_token() == Some("a".to_owned()));
    h.settle();
    // The list answers; the chosen tone's files are listed, ticked, and the
    // ticked ones written to the library, with where they came from.
    h.app.known_sites[TONE3000].site.home = "https://127.0.0.1:9".to_owned();
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.finding = Some(receive);
    let mut tone = found("TONE3000", "Twin", "nam · t3k", &[]);
    tone.vars.insert("user".to_owned(), "ann".to_owned());
    tone.vars.insert("license".to_owned(), "t3k".to_owned());
    send.send(Ok(vec![tone])).unwrap();
    h.settle();
    h.app.sites_select(0);
    let (send, receive) = std::sync::mpsc::channel();
    h.app.sites.opening = Some(receive);
    let empty = Score { title: String::new(), artist: String::new(), parts: Vec::new() };
    let files = vec![("Standard.nam".to_owned(), b"{}".to_vec()), ("Lite.nam".to_owned(), b"{}".to_vec())];
    send.send(Ok(Opened { score: empty, messages: Vec::new(), videos: Vec::new(), lyrics: Default::default(), files })).unwrap();
    h.settle();
    assert_eq!(h.app.sites.files.as_ref().map(Vec::len), Some(2));
    assert_eq!(h.app.sites.chosen, vec![true, true]);
    h.app.sites.chosen[1] = false;
    h.app.sites_save_files();
    let folder = h.app.sites.saved.clone().expect("written");
    assert!(folder.starts_with(dir.join("Captures").join("TONE3000")), "{}", folder.display());
    assert!(folder.join("Standard.nam").is_file() && !folder.join("Lite.nam").exists());
    let source = std::fs::read_to_string(folder.join("SOURCE.txt")).unwrap();
    assert!(source.contains("By: ann") && source.contains("Licence: t3k"), "{source}");
    assert!(h.app.sites.message.contains("1 "), "{}", h.app.sites.message);
    // Signing out forgets the token; the key stays.
    h.app.sites_sign_out();
    assert!(!h.app.site_signed_in() && h.app.site_key() == "t3k_pk_x");
    h.settle();
    let _ = std::fs::remove_dir_all(dir);
}
