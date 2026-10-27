//! Drives the real UI headlessly with simulated mouse and keyboard events.

use eframe::egui::{self, pos2, vec2, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect};
use og_engine::{templates, AudioRef, ClipData, Content, Note, Region, TrackKind};

use crate::app::{App, AutoParam, BottomTab, Dialog, NoteView, RightPanel, SNAP};
use crate::audio;

/// A folder of the test's own, empty, in the temporary files.
pub(crate) fn temp_folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub(crate) struct Harness {
    pub(crate) ctx: egui::Context,
    pub(crate) app: App,
    pub(crate) time: f64,
}

impl Harness {
    pub(crate) fn new() -> Self {
        let app = App::with_audio(audio::silent(templates::demo()), None);
        let mut harness = Self { ctx: egui::Context::default(), app, time: 0.0 };
        harness.settle();
        harness
    }

    /// Lets the panels reach their final sizes.
    pub(crate) fn settle(&mut self) {
        for _ in 0..3 {
            self.frame(Vec::new());
        }
    }

    pub(crate) fn frame(&mut self, events: Vec<Event>) {
        self.frame_with(events, Modifiers::NONE);
    }

    fn frame_with(&mut self, events: Vec<Event>, modifiers: Modifiers) {
        self.time += 0.02;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 860.0))),
            time: Some(self.time),
            modifiers,
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let _ = self.ctx.run(input, |ctx| app.ui(ctx));
    }

    fn button(&mut self, pos: Pos2, button: PointerButton) {
        self.frame(vec![Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE }]);
        }
        self.frame(Vec::new());
    }

    pub(crate) fn click(&mut self, pos: Pos2) {
        self.button(pos, PointerButton::Primary);
    }

    fn drag(&mut self, from: Pos2, to: Pos2) {
        self.drag_with(from, to, Modifiers::NONE);
    }

    /// A drag with keys held (Alt, Ctrl…) all along.
    fn drag_with(&mut self, from: Pos2, to: Pos2, modifiers: Modifiers) {
        let button = PointerButton::Primary;
        self.frame_with(vec![Event::PointerMoved(from)], modifiers);
        self.frame_with(vec![Event::PointerButton { pos: from, button, pressed: true, modifiers }], modifiers);
        for step in 1..=8 {
            self.frame_with(vec![Event::PointerMoved(from + (to - from) * (step as f32 / 8.0))], modifiers);
        }
        self.frame_with(vec![Event::PointerButton { pos: to, button, pressed: false, modifiers }], modifiers);
        self.frame(Vec::new());
    }

    /// Files dragged from the file explorer and dropped at `at`.
    fn drop_files(&mut self, at: Pos2, paths: &[std::path::PathBuf]) {
        let files: Vec<egui::DroppedFile> = paths.iter().map(|p| egui::DroppedFile { path: Some(p.clone()), ..Default::default() }).collect();
        let hovered: Vec<egui::HoveredFile> = paths.iter().map(|p| egui::HoveredFile { path: Some(p.clone()), ..Default::default() }).collect();
        self.frame(vec![Event::PointerMoved(at)]);
        for (hovered, dropped) in [(hovered, Vec::new()), (Vec::new(), files)] {
            self.time += 0.02;
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 860.0))),
                time: Some(self.time),
                events: vec![Event::PointerMoved(at)],
                hovered_files: hovered,
                dropped_files: dropped,
                ..Default::default()
            };
            let app = &mut self.app;
            let _ = self.ctx.run(input, |ctx| app.ui(ctx));
        }
        self.frame(Vec::new());
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) {
        for pressed in [true, false] {
            let event = Event::Key { key, physical_key: None, pressed, repeat: false, modifiers };
            self.frame_with(vec![event], modifiers);
        }
    }

    /// Lets enough time pass for the next click not to count as a double click,
    /// and for the next edit to be its own undo step.
    fn pause(&mut self) {
        self.time += 1.0;
        self.frame(Vec::new());
    }

    /// Screen position of `beat` in the lane of track `ti`; `up` is 0 at the top
    /// of the lane and 1 at the bottom.
    fn lane(&self, ti: usize, beat: f64, up: f32) -> Pos2 {
        let rect = self.app.lane_rects[ti];
        pos2(rect.left() + ((beat - self.app.scroll) * f64::from(self.app.ppb)) as f32, rect.top() + rect.height() * up)
    }

    /// Runs the engine as the sound card would, for `seconds`.
    fn run_audio(&mut self, seconds: f64) {
        let mut block = vec![0.0f32; 960];
        for _ in 0..(seconds * 100.0) as usize {
            self.app.engine().process(&mut block, 2);
        }
    }
}

const CTRL: Modifiers = Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };
const ALT: Modifiers = Modifiers { alt: true, ctrl: false, shift: false, mac_cmd: false, command: false };

#[test]
fn clicking_a_region_selects_it() {
    let mut h = Harness::new();
    assert_eq!(h.app.selected_region, None);
    h.click(h.lane(2, 6.0, 0.5));
    assert_eq!(h.app.selected_region, Some((2, 0)));
    assert_eq!(h.app.selected_track, 2);
    // Clicking empty space in another lane clears the region selection.
    h.pause();
    h.click(h.lane(0, 24.0, 0.5));
    assert_eq!(h.app.selected_region, None);
    assert_eq!(h.app.selected_track, 0);
}

#[test]
fn space_starts_and_stops_playback() {
    let mut h = Harness::new();
    h.key(Key::Space, Modifiers::NONE);
    assert!(h.app.engine().is_playing());
    h.key(Key::Space, Modifiers::NONE);
    assert!(!h.app.engine().is_playing());
}

#[test]
fn double_click_creates_a_region_then_notes_can_be_added_and_undone() {
    let mut h = Harness::new();
    // Beat 20 is in bar 6, past the demo regions.
    let at = h.lane(0, 20.0, 0.5);
    h.click(at);
    h.click(at);
    assert_eq!(h.app.project.tracks[0].regions.len(), 2);
    assert_eq!(h.app.selected_region, Some((0, 1)));
    let region = &h.app.project.tracks[0].regions[1];
    assert_eq!((region.start, region.length), (20.0, 16.0));

    // Click in the piano roll to add a note, right-click to remove it.
    h.pause();
    let spot = h.app.roll_rect.center() + vec2(100.0, 0.0);
    h.click(spot);
    let notes = h.app.project.tracks[0].regions[1].notes();
    assert_eq!(notes.len(), 1, "a click in the editor adds a note");
    assert_eq!(notes[0].length, 0.5);
    assert_eq!(notes[0].start % SNAP, 0.0);
    assert_eq!(h.app.selected_notes, vec![0]);
    h.pause();
    h.button(spot, PointerButton::Secondary);
    assert!(h.app.project.tracks[0].regions[1].notes().is_empty());

    // Undo brings the note back, then removes it, then removes the region.
    h.key(Key::Z, CTRL);
    assert_eq!(h.app.project.tracks[0].regions[1].notes().len(), 1);
    h.key(Key::Z, CTRL);
    h.key(Key::Z, CTRL);
    assert_eq!(h.app.project.tracks[0].regions.len(), 1);
    assert_eq!(h.app.selected_region, None);
    h.key(Key::Y, CTRL);
    assert_eq!(h.app.project.tracks[0].regions.len(), 2);
}

#[test]
fn notes_can_be_selected_moved_and_deleted_together() {
    let mut h = Harness::new();
    h.click(h.lane(2, 6.0, 0.5));
    h.pause();
    h.key(Key::A, CTRL);
    let count = h.app.project.tracks[2].regions[0].notes().len();
    assert_eq!(h.app.selected_notes.len(), count);
    h.key(Key::Delete, Modifiers::NONE);
    assert!(h.app.project.tracks[2].regions[0].notes().is_empty());
    assert_eq!(h.app.project.tracks[2].regions.len(), 1, "the region itself stays");
    h.key(Key::Z, CTRL);
    assert_eq!(h.app.project.tracks[2].regions[0].notes().len(), count);
}

#[test]
fn dragging_moves_resizes_and_loops_a_region() {
    let mut h = Harness::new();
    let beat = |h: &Harness, b: f64, up: f32| h.lane(1, b, up);
    // Drag the bass region two bars to the right.
    h.drag(beat(&h, 3.0, 0.5), beat(&h, 11.0, 0.5));
    assert_eq!(h.app.project.tracks[1].regions[0].start, 8.0);
    assert_eq!(h.app.project.tracks[1].regions[0].length, 16.0);
    // Drag the lower part of its right edge one bar to the left.
    h.pause();
    let edge = beat(&h, 24.0, 0.8) - vec2(3.0, 0.0);
    h.drag(edge, edge - vec2(4.0 * h.app.ppb, 0.0));
    assert_eq!(h.app.project.tracks[1].regions[0].length, 12.0);
    assert_eq!(h.app.project.tracks[1].regions[0].loop_len, None);
    // Drag the upper part of its right edge two bars to the right: it loops.
    h.pause();
    let edge = beat(&h, 20.0, 0.2) - vec2(3.0, 0.0);
    h.drag(edge, edge + vec2(8.0 * h.app.ppb, 0.0));
    let region = &h.app.project.tracks[1].regions[0];
    assert_eq!((region.length, region.loop_len), (20.0, Some(12.0)));
}

#[test]
fn the_tablature_writes_how_the_notes_are_played() {
    use og_engine::{Articulation, Slide, Vibrato};
    let mut h = Harness::new();
    h.app.select_region(2, 0);
    let guitar = h.app.patches.iter().find(|p| p.name == "Steel guitar").unwrap().clone();
    h.app.apply_patch(&guitar);
    let on = |start: f64, pitch: u8, string: u8, articulation: Articulation| Note { start, length: 1.0, pitch, velocity: 100, string: Some(string), articulation };
    let notes = vec![
        on(0.0, 45, 4, Articulation { slide: Some(Slide::Legato), ..Articulation::default() }),
        on(1.0, 47, 4, Articulation { hammer: true, bend: vec![(0.0, 0.0), (1.0, 2.0)], ..Articulation::default() }),
        on(2.0, 48, 4, Articulation { vibrato: Some(Vibrato::Wide), staccato: true, accent: 2, let_ring: true, ..Articulation::default() }),
        on(3.0, 40, 5, Articulation { dead: true, palm_mute: true, ..Articulation::default() }),
        on(3.5, 40, 5, Articulation { ghost: true, palm_mute: true, slide: Some(Slide::OutDown), ..Articulation::default() }),
        on(4.0, 52, 5, Articulation { harmonic: Some(12), bend: vec![(0.0, 0.0), (1.0, -7.0)], slide: Some(Slide::FromBelow), ..Articulation::default() }),
    ];
    h.app.project.tracks[2].regions[0].content = Content::Midi(notes.clone());
    h.app.project.tracks[2].regions[0].chords.clear();
    h.app.note_view = NoteView::Tab;
    h.settle();
    assert_eq!(h.app.note_view, NoteView::Tab);
    // The project keeps the articulations, and a plain note writes nothing new.
    let text = serde_json::to_string(&h.app.project).unwrap();
    assert!(text.contains("\"harmonic\":12") && text.contains("\"Legato\""));
    let plain = serde_json::to_string(&Note { start: 0.0, length: 1.0, pitch: 60, velocity: 100, string: None, ..Default::default() }).unwrap();
    assert!(!plain.contains("articulation"), "{plain}");
    let back: Note = serde_json::from_str(&plain).unwrap();
    assert!(back.articulation.is_plain());
    // Playing the region raises no error: the bends and slides reach the sound.
    h.app.position = 0.0;
    h.app.toggle_play();
    h.run_audio(0.5);
    h.app.toggle_play();
    h.settle();
}

#[test]
fn chord_grids_hang_over_the_tablature() {
    use crate::ui_bottom::{chord_grid_shape, tab_positions};
    const GUITAR: [u8; 6] = [64, 59, 55, 50, 45, 40];
    let x = None;
    let on = |start: f64, pitch: u8, string: u8| Note { start, length: 1.0, pitch, velocity: 100, string: Some(string), ..Default::default() };
    // A chord strummed as written on the tablature: the grid shows those very frets.
    let strummed = [on(0.0, 45, 4), on(0.0, 52, 3), on(0.0, 57, 2), on(0.0, 60, 1), on(0.0, 64, 0)];
    let places = tab_positions(&strummed, &GUITAR);
    let grid = chord_grid_shape("Am", 0.0, 4.0, &strummed, &places, &GUITAR).unwrap();
    assert_eq!((grid.base, grid.frets), (1, vec![Some(0), Some(1), Some(2), Some(2), Some(0), x]));
    // Played high on the neck, the grid starts at the first fret held.
    let high = [on(0.0, 48, 5), on(0.0, 55, 4), on(0.0, 60, 3), on(0.0, 64, 2)];
    let grid = chord_grid_shape("C", 0.0, 4.0, &high, &tab_positions(&high, &GUITAR), &GUITAR).unwrap();
    assert_eq!((grid.base, grid.frets), (8, vec![x, x, Some(9), Some(10), Some(10), Some(8)]));
    // An arpeggio: the first note on each string, as long as it belongs to the chord.
    let arpeggio = [on(0.0, 40, 5), on(1.0, 47, 4), on(2.0, 52, 3), on(2.5, 55, 2), on(3.0, 59, 1), on(3.5, 62, 0), on(4.0, 45, 4)];
    let places = tab_positions(&arpeggio, &GUITAR);
    let grid = chord_grid_shape("Em", 0.0, 4.0, &arpeggio, &places, &GUITAR).unwrap();
    assert_eq!(grid.frets, vec![x, Some(0), Some(0), Some(2), Some(2), Some(0)], "the D on the high string is not in E minor");
    // Notes that do not show the chord (a bass line): a usual way to play it instead.
    let line = [on(0.0, 40, 5), on(1.0, 47, 4)];
    let grid = chord_grid_shape("G", 0.0, 4.0, &line, &tab_positions(&line, &GUITAR), &GUITAR).unwrap();
    assert_eq!(grid.frets, vec![Some(3), Some(0), Some(0), Some(0), Some(2), Some(3)]);
    assert_eq!(chord_grid_shape("N.C.", 0.0, 4.0, &line, &tab_positions(&line, &GUITAR), &GUITAR), None, "not a chord");
    assert_eq!(chord_grid_shape("Cm7b5", 0.0, 4.0, &[], &[], &[43, 38, 33]), None, "four notes on three strings");

    // The tablature with chords written over it draws them, and leaves room above the strings.
    let mut h = Harness::new();
    h.app.select_region(2, 0);
    let guitar = h.app.patches.iter().find(|p| p.name == "Steel guitar").unwrap().clone();
    h.app.apply_patch(&guitar);
    h.app.note_view = NoteView::Tab;
    h.app.project.tracks[2].regions[0].chords = vec![
        og_engine::ChordMark { beat: 0.0, name: "B".to_owned() },
        og_engine::ChordMark { beat: 2.0, name: "F#".to_owned() },
        og_engine::ChordMark { beat: 4.0, name: "G#5".to_owned() },
        og_engine::ChordMark { beat: 6.0, name: "N.C.".to_owned() },
    ];
    h.settle();
    assert_eq!(h.app.note_view, NoteView::Tab);

    // A chord is drawn at its start. Once that has scrolled past the left edge
    // of the view, it waits at the edge while it lasts: the chord being played
    // always shows. The next chord then pushes it out.
    use crate::ui_bottom::chord_place;
    assert_eq!(chord_place(120.0, 400.0, 40.0, 50.0), 120.0);
    assert_eq!(chord_place(-300.0, 400.0, 40.0, 50.0), 50.0, "held at the edge");
    assert_eq!(chord_place(-300.0, 70.0, 40.0, 50.0), 30.0, "pushed out by the next chord");

    // The tablature is drawn at the text size the user chose, within limits;
    // taller than its panel, it still draws.
    assert_eq!(h.app.tab_text_size(), crate::ui_bottom::TAB_TEXT, "the usual size");
    h.app.settings.tab_text_size = 22.0;
    h.settle();
    assert_eq!(h.app.tab_text_size(), 22.0);
    h.app.settings.tab_text_size = 500.0;
    h.settle();
    assert_eq!(h.app.tab_text_size(), *crate::ui_bottom::TAB_TEXT_RANGE.end());
    h.app.settings.tab_text_size = 0.0;
}

#[test]
fn a_guitar_track_has_a_tablature_and_the_editors_zoom() {
    use crate::ui_bottom::tab_positions;
    const GUITAR: [u8; 6] = [64, 59, 55, 50, 45, 40];
    let mut h = Harness::new();
    // The melody plays a synth: no tablature. Given a guitar sound, it has one.
    h.app.select_region(2, 0);
    h.settle();
    assert!(!h.app.is_guitar_track(2));
    h.app.note_view = NoteView::Tab;
    h.settle();
    assert_eq!(h.app.note_view, NoteView::Roll, "no tablature for a synth");
    let guitar = h.app.patches.iter().find(|p| p.name == "Steel guitar").unwrap().clone();
    h.app.apply_patch(&guitar);
    assert!(h.app.is_guitar_track(2) && !h.app.is_bass_track(2));
    h.app.note_view = NoteView::Tab;
    h.settle();
    assert_eq!(h.app.note_view, NoteView::Tab);
    // Every note of the melody sits on a string, at the lowest fret that gives its pitch.
    let notes = h.app.project.tracks[2].regions[0].notes().to_vec();
    let places = tab_positions(&notes, &GUITAR);
    for (note, place) in notes.iter().zip(&places) {
        let (string, fret) = place.expect("within reach");
        assert_eq!(GUITAR[string] + fret, note.pitch);
        assert!(fret <= 24);
    }
    // A chord spreads over the strings; what is out of reach is left out.
    let at = |pitch: u8| Note { start: 0.0, length: 1.0, pitch, velocity: 100, string: None, ..Default::default() };
    let chord = tab_positions(&[at(40), at(45), at(20), at(70), at(100)], &GUITAR);
    assert_eq!(chord, [Some((5, 0)), Some((4, 0)), None, Some((0, 6)), None]);
    // Two notes of one pitch at once: the second takes another string.
    assert_eq!(tab_positions(&[at(45), at(45)], &GUITAR), [Some((4, 0)), Some((5, 5))]);
    // A note that knows its string (from a tablature) stays on it, whatever the fret.
    let on = |pitch: u8, string: u8| Note { start: 0.0, length: 1.0, pitch, velocity: 100, string: Some(string), ..Default::default() };
    assert_eq!(tab_positions(&[on(52, 5), at(52)], &GUITAR), [Some((5, 12)), Some((3, 2))]);
    assert_eq!(tab_positions(&[on(30, 5)], &GUITAR), [None], "below the open string: nowhere");
    assert_eq!(tab_positions(&[on(52, 9)], &GUITAR), [Some((3, 2))], "a string the instrument lacks: placed anew");
    // The track's own tuning: a seven-string guitar.
    h.app.project.tracks[2].tuning = vec![64, 59, 55, 50, 45, 40, 35];
    h.settle();
    assert_eq!(tab_positions(&[on(40, 6)], &h.app.project.tracks[2].tuning), [Some((6, 5))]);
    h.app.project.tracks[2].tuning.clear();
    // Chord names: the ones written over the region, else the ones the notes make.
    use crate::ui_bottom::chord_label;
    assert_eq!(chord_label("Em"), "Mim");
    assert_eq!(chord_label("F#m7"), "Fa#m7");
    assert_eq!(chord_label("C/E"), "Do/Mi");
    assert_eq!(chord_label("N.C."), "N.C.");
    h.app.project.tracks[2].regions[0].chords = vec![og_engine::ChordMark { beat: 0.0, name: "Am".to_owned() }];
    h.settle();
    h.app.project.tracks[2].regions[0].chords.clear();
    // Clicking a fret number selects its note.
    let first = notes.iter().enumerate().min_by(|a, b| a.1.start.total_cmp(&b.1.start)).map(|(i, _)| i).unwrap();
    let (string, _) = places[first].unwrap();
    let rect = h.app.roll_rect;
    let left = rect.left() + 46.0 + crate::ui_bottom::TAB_INSET;
    let cycle = h.app.project.tracks[2].regions[0].cycle_len();
    let ppb = f64::from(rect.right() - left - 16.0) / cycle;
    let spot = pos2(left + (notes[first].start * ppb) as f32, rect.top() + 52.0 + 36.0 + string as f32 * 18.0);
    h.click(spot);
    assert_eq!(h.app.selected_notes, vec![first]);
    h.pause();
    h.click(pos2(rect.right() - 300.0, rect.top() + 8.0));
    assert!(h.app.selected_notes.is_empty(), "a click on nothing clears the selection");
    assert!(h.app.position > 0.0, "and the playhead goes there");
    h.app.go_to(0.0);
    // A bass: four strings, and the view still draws.
    let bass = h.app.patches.iter().find(|p| p.name == "Electric bass").unwrap().clone();
    h.app.apply_patch(&bass);
    assert!(h.app.is_bass_track(2) && h.app.is_guitar_track(2));
    h.settle();

    // Ctrl and the wheel over each editor zoom it in; the scroll stays within the region.
    for view in [NoteView::Tab, NoteView::Roll, NoteView::Score] {
        h.app.note_view = view;
        h.app.editor_zoom = Some(10.0);
        h.settle();
        let at = h.app.roll_rect.center();
        h.frame(vec![Event::PointerMoved(at)]);
        h.frame_with(vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: vec2(0.0, 3.0), modifiers: CTRL }], CTRL);
        h.settle();
        assert!(h.app.editor_zoom.is_some_and(|z| z > 10.0), "{view:?}: {:?}", h.app.editor_zoom);
        assert!(h.app.editor_scroll >= 0.0 && h.app.editor_scroll < cycle, "{view:?}: {}", h.app.editor_scroll);
        // Without Ctrl, the wheel does not zoom (once the smoothed zoom has settled).
        for _ in 0..40 {
            h.frame(Vec::new());
        }
        let zoom = h.app.editor_zoom;
        h.frame(vec![Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: vec2(0.0, 3.0), modifiers: Modifiers::NONE }]);
        for _ in 0..40 {
            h.frame(Vec::new());
        }
        assert_eq!(h.app.editor_zoom, zoom);
    }
    // Four bars hold the whole region: nothing to scroll.
    h.app.editor_zoom = None;
    h.app.editor_scroll = 1e9;
    h.settle();
    assert_eq!(h.app.editor_scroll, 0.0, "the whole region shows");

    // While the song plays, the playhead stays a fifth of the way across and the
    // notes scroll under it: the first beat shown is a fifth of a view before it.
    assert!(!h.app.settings.editor_playhead_moves, "the usual way");
    h.app.note_view = NoteView::Tab;
    // Played in a region twice as long as four bars: it scrolls.
    h.app.project.tracks[2].regions[0].length = 32.0;
    h.app.project.tracks[2].regions[0].loop_len = None;
    h.settle();
    h.app.toggle_play();
    // Six beats at 100 bpm, into the sixteen-beat region seen a quarter at a time.
    h.run_audio(3.8);
    h.frame(Vec::new());
    assert!(h.app.position > 4.0 && h.app.position < 8.0, "{}", h.app.position);
    assert!((h.app.editor_scroll - (h.app.position - crate::ui_bottom::TAB_CLOSEST * crate::ui_bottom::PLAYHEAD_AT)).abs() < 0.2, "a fifth of the four beats shown: {} at {}", h.app.editor_scroll, h.app.position);
    h.app.toggle_play();
    h.settle();
    assert!(h.app.editor_scroll >= 0.0, "stopped: back within the region");
    // Otherwise the playhead moves and the editor pages when it leaves the view.
    h.app.settings.editor_playhead_moves = true;
    h.app.editor_scroll = 0.0;
    h.app.engine().set_position(0.0);
    h.frame(Vec::new());
    h.app.toggle_play();
    h.run_audio(10.5);
    h.frame(Vec::new());
    assert!(h.app.position > 16.0, "{}", h.app.position);
    assert!((h.app.editor_scroll - 16.0).abs() < 1e-6, "paged to the next four bars: {}", h.app.editor_scroll);
    h.app.toggle_play();
    h.settle();
    h.app.settings.editor_playhead_moves = false;
}

#[test]
fn dragging_with_alt_or_ctrl_held_copies_a_region_or_notes() {
    let mut h = Harness::new();
    let before = h.app.project.tracks[1].regions[0].clone();
    // Alt-drag the bass region four bars to the right: a copy lands there.
    h.drag_with(h.lane(1, 3.0, 0.5), h.lane(1, 19.0, 0.5), ALT);
    let regions = &h.app.project.tracks[1].regions;
    assert_eq!(regions.len(), 2, "{regions:?}");
    assert_eq!(regions[0], before, "the original stays");
    assert_eq!((regions[1].start, regions[1].length, regions[1].content == before.content), (16.0, 16.0, true));
    assert_eq!(h.app.selected_region, Some((1, 1)));
    // One undo step takes the copy away.
    h.key(Key::Z, CTRL);
    assert_eq!(h.app.project.tracks[1].regions.len(), 1);
    // Ctrl works too, and onto another track of the right kind.
    h.pause();
    h.drag_with(h.lane(1, 3.0, 0.5), h.lane(2, 3.0, 0.5), CTRL);
    assert_eq!(h.app.project.tracks[1].regions.len(), 1);
    assert_eq!(h.app.project.tracks[2].regions.len(), 2);
    // The right edge still resizes, copy key or not.
    h.pause();
    let edge = h.lane(1, 16.0, 0.8) - vec2(3.0, 0.0);
    h.drag_with(edge, edge - vec2(4.0 * h.app.ppb, 0.0), ALT);
    assert_eq!(h.app.project.tracks[1].regions.len(), 1);
    assert_eq!(h.app.project.tracks[1].regions[0].length, 12.0);

    // Notes: a region of notes with one note in it, Alt-dragged to the right.
    h.pause();
    let at = h.lane(0, 20.0, 0.5);
    h.click(at);
    h.click(at);
    h.pause();
    let spot = h.app.roll_rect.center() + vec2(100.0, 0.0);
    h.click(spot);
    let notes = h.app.project.tracks[0].regions[1].notes().to_vec();
    assert_eq!(notes.len(), 1);
    h.pause();
    h.drag_with(spot, spot + vec2(60.0, 0.0), ALT);
    let now = h.app.project.tracks[0].regions[1].notes().to_vec();
    assert_eq!(now.len(), 2, "{now:?}");
    assert_eq!(now[0], notes[0], "the original stays");
    assert!(now[1].start > notes[0].start && now[1].pitch == notes[0].pitch, "{now:?}");
    assert_eq!(h.app.selected_notes, vec![1]);
    // Without the key: a move.
    h.pause();
    h.drag(spot, spot + vec2(60.0, 0.0));
    assert_eq!(h.app.project.tracks[0].regions[1].notes().len(), 2);
}

#[test]
fn files_dropped_from_the_explorer_are_taken_in_by_kind() {
    let dir = features_dir("depot");
    let mut h = Harness::new();
    let tracks = h.app.project.tracks.len();
    // An audio file under the tracks: a new audio track, at the bar under the pointer.
    let wav = dir.join("son.wav");
    og_engine::wav::write_wav(&wav, &sine_clip(440.0, 1.0).samples, 48_000, 1, 16).unwrap();
    let last = *h.app.lane_rects.last().unwrap();
    let under = pos2(h.lane(0, 21.0, 0.5).x, last.bottom() + 40.0);
    h.drop_files(under, std::slice::from_ref(&wav));
    assert_eq!(h.app.project.tracks.len(), tracks + 1);
    let audio = &h.app.project.tracks[tracks];
    assert!(audio.kind.is_audio());
    assert_eq!(audio.regions.len(), 1);
    assert_eq!(audio.regions[0].start, 20.0, "at the bar under the pointer");
    assert!(matches!(audio.regions[0].content, Content::Audio(_)));
    // Onto that audio track: a second region there, at another bar.
    h.settle();
    h.drop_files(h.lane(tracks, 29.0, 0.5), std::slice::from_ref(&wav));
    assert_eq!(h.app.project.tracks.len(), tracks + 1, "no new track");
    assert_eq!(h.app.project.tracks[tracks].regions.len(), 2);
    assert_eq!(h.app.project.tracks[tracks].regions[1].start, 28.0);
    // Onto a track of notes, which cannot take audio: the selected audio track, else a new one.
    h.app.selected_track = 0;
    h.drop_files(h.lane(0, 29.0, 0.5), std::slice::from_ref(&wav));
    assert_eq!(h.app.project.tracks.len(), tracks + 2);
    // A MIDI file: its tracks are added wherever it is dropped.
    let midi = dir.join("notes.mid");
    // One note of a beat, in a format 0 file with 96 ticks a beat.
    let mut bytes = b"MThd".to_vec();
    bytes.extend_from_slice(&[0, 0, 0, 6, 0, 0, 0, 1, 0, 96]);
    bytes.extend_from_slice(b"MTrk");
    bytes.extend_from_slice(&[0, 0, 0, 12, 0, 0x90, 60, 100, 96, 0x80, 60, 0, 0, 0xFF, 0x2F, 0]);
    std::fs::write(&midi, bytes).unwrap();
    let count = h.app.project.tracks.len();
    h.drop_files(pos2(700.0, 400.0), &[midi]);
    assert_eq!(h.app.project.tracks.len(), count + 1);
    assert!(h.app.status.contains("import"), "{}", h.app.status);
    // A sound bank: into the library.
    let bank = dir.join("Essai.sf2");
    std::fs::write(&bank, og_engine::testfont::tiny_soundfont()).unwrap();
    h.drop_files(pos2(100.0, 300.0), &[bank]);
    assert!(h.app.patches.iter().any(|p| p.name == "Sinus"));
    assert!(h.app.show_library);
    // A project file: opened. Something else: refused in a sentence.
    let file = dir.join("autre.ogproj");
    h.app.project.name = "Déposé".to_owned();
    h.app.save_to(&file).unwrap();
    h.app.replace_project(og_engine::Project::empty(), None);
    h.drop_files(pos2(700.0, 400.0), &[file]);
    assert_eq!(h.app.project.name, "Déposé");
    h.drop_files(pos2(700.0, 400.0), &[dir.join("lisez.txt")]);
    assert!(h.app.status.contains("lisez.txt"), "{}", h.app.status);
    // Hovering draws a hint and changes nothing.
    let hovered = vec![egui::HoveredFile { path: Some(wav), ..Default::default() }];
    let app = &mut h.app;
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1400.0, 860.0))), hovered_files: hovered, ..Default::default() };
    let _ = h.ctx.run(input, |ctx| app.ui(ctx));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_effects_of_a_track_are_automated_like_its_volume() {
    use og_engine::FxParam;
    let mut h = Harness::new();
    let all = AutoParam::all();
    assert!(all.len() > 8 && all.iter().all(|p| !p.label().is_empty()));
    assert_eq!(AutoParam::Fx(FxParam::Pedal(1)).label(), "Pédale 2");
    // Every lane draws.
    for param in &all {
        h.app.automation = Some(*param);
        h.settle();
    }
    // A click on the reverb lane of the bass track puts a point there.
    h.app.automation = Some(AutoParam::Fx(FxParam::Reverb));
    h.settle();
    h.click(h.lane(1, 8.0, 0.1));
    let curve = h.app.curve(crate::app::CurveId::Fx(1, FxParam::Reverb)).to_vec();
    assert_eq!(curve.len(), 1, "{curve:?}");
    assert_eq!(curve[0].beat, 8.0);
    assert!(curve[0].value > 0.8, "near the top of the lane: {}", curve[0].value);
    assert!(h.app.project.tracks[1].fx_automated());
    h.frame(Vec::new());
    assert_eq!(h.app.synced.tracks[1].fx_auto, h.app.project.tracks[1].fx_auto, "reaches the engine");
    // Undo takes the point away; the other lanes have none.
    h.key(Key::Z, CTRL);
    assert!(h.app.curve(crate::app::CurveId::Fx(1, FxParam::Reverb)).is_empty());
    assert!(h.app.curve(crate::app::CurveId::Fx(1, FxParam::EqLow)).is_empty());
    // Pedals can be chosen one by one on a track of notes too.
    h.app.selected_track = 2;
    assert!(h.app.visible_panels(2).contains(&crate::app::Panel::Pedals));
}

#[test]
fn a_region_can_be_dragged_to_another_track_of_the_same_kind() {
    let mut h = Harness::new();
    // From the bass track to the melody track, same time.
    h.drag(h.lane(1, 3.0, 0.5), h.lane(2, 3.0, 0.5));
    assert!(h.app.project.tracks[1].regions.is_empty());
    assert_eq!(h.app.project.tracks[2].regions.len(), 2);
    assert_eq!(h.app.selected_region, Some((2, 1)));
    // The drummer track only takes drummer regions.
    h.pause();
    h.drag(h.lane(2, 3.0, 0.5), h.lane(3, 3.0, 0.5));
    assert_eq!(h.app.project.tracks[3].regions.len(), 1);
    assert_eq!(h.app.project.tracks[2].regions.len(), 2);
}

#[test]
fn slider_style_edits_reach_the_engine_and_can_be_undone() {
    let mut h = Harness::new();
    h.app.project.tempo = 140.0;
    h.frame(Vec::new());
    assert_eq!(h.app.synced.tempo, 140.0);
    // Changes in consecutive frames are one undo step.
    h.app.project.tempo = 141.0;
    h.frame(Vec::new());
    h.key(Key::Z, CTRL);
    assert_eq!(h.app.project.tempo, 100.0);
    h.key(Key::Y, CTRL);
    assert_eq!(h.app.project.tempo, 141.0);
}

#[test]
fn split_join_copy_paste_and_loop() {
    let mut h = Harness::new();
    h.app.select_region(2, 0);
    let before = h.app.project.tracks[2].regions[0].notes().len();
    h.app.go_to(8.0);
    h.app.split_region();
    {
        let regions = &h.app.project.tracks[2].regions;
        assert_eq!(regions.len(), 2);
        assert_eq!((regions[0].length, regions[1].start, regions[1].length), (8.0, 8.0, 8.0));
        assert_eq!(regions[0].notes().len() + regions[1].notes().len(), before);
        assert!(regions[1].notes().iter().all(|n| n.start >= 0.0 && n.start < 8.0));
    }
    h.app.join_regions();
    {
        let regions = &h.app.project.tracks[2].regions;
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].length, 16.0);
        assert_eq!(regions[0].notes().len(), before);
    }

    h.app.toggle_loop();
    assert_eq!(h.app.project.tracks[2].regions[0].loop_len, Some(16.0));
    assert_eq!(h.app.project.tracks[2].regions[0].length, 32.0);
    // Splitting a looped region writes the repeats out.
    h.app.go_to(20.0);
    h.app.split_region();
    assert_eq!(h.app.project.tracks[2].regions.len(), 2);
    assert_eq!(h.app.project.tracks[2].regions[0].loop_len, None);
    h.app.undo();
    h.app.toggle_loop();
    assert_eq!(h.app.project.tracks[2].regions[0].length, 16.0);

    h.app.copy_region();
    h.app.go_to(32.0);
    h.app.paste_region();
    assert_eq!(h.app.project.tracks[2].regions.len(), 2);
    assert_eq!(h.app.project.tracks[2].regions[1].start, 32.0);
    // A note region cannot be pasted on the drummer track.
    h.app.selected_track = 3;
    h.app.paste_region();
    assert_eq!(h.app.project.tracks[3].regions.len(), 1);
    h.app.select_region(2, 1);
    h.app.cut_region();
    assert_eq!(h.app.project.tracks[2].regions.len(), 1);
}

#[test]
fn recording_what_is_played_creates_a_region() {
    let mut h = Harness::new();
    h.app.project.count_in = false;
    h.app.selected_track = 2;
    h.app.go_to(16.0);
    h.key(Key::R, Modifiers::NONE);
    assert!(h.app.recording && h.app.engine().is_recording());
    h.run_audio(0.3);
    h.app.engine().live_note_on(2, 64, 100);
    h.run_audio(0.6);
    h.app.engine().live_note_off(2, 64);
    h.run_audio(0.3);
    h.key(Key::R, Modifiers::NONE);
    h.frame(Vec::new());
    assert!(!h.app.recording);
    let regions = &h.app.project.tracks[2].regions;
    assert_eq!(regions.len(), 2);
    assert_eq!(regions[1].start, 16.0);
    assert_eq!(regions[1].notes().len(), 1);
    assert_eq!(regions[1].notes()[0].pitch, 64);
    // 0.6 s at 100 bpm is one beat.
    assert!((regions[1].notes()[0].length - 1.0).abs() < 0.05);
    // The drummer track cannot be recorded on.
    h.app.selected_track = 3;
    h.app.toggle_record();
    assert!(!h.app.recording);
}

#[test]
fn loops_are_added_and_auditioned() {
    let mut h = Harness::new();
    let tracks = h.app.project.tracks.len();
    let index = h.app.loops.iter().position(|l| l.name == "Funk slap bass").unwrap();
    h.app.go_to(16.0);
    h.app.add_loop(index);
    assert_eq!(h.app.project.tracks.len(), tracks + 1);
    let track = h.app.project.tracks.last().unwrap();
    // The sound keeps the library's name; the track and the region are named
    // in the language of the interface.
    assert_eq!(track.patch, "Slap bass");
    assert_eq!((track.name.as_str(), track.regions[0].name.as_str()), ("Basse funk slap", "Basse funk slap"));
    assert_eq!((track.regions[0].start, track.regions[0].length), (16.0, 8.0));
    assert!(!track.regions[0].notes().is_empty());
    // The same loop again goes after the first one, on the same track.
    h.app.add_loop(index);
    assert_eq!(h.app.project.tracks.len(), tracks + 1);
    assert_eq!(h.app.project.tracks.last().unwrap().regions.len(), 2);

    h.app.toggle_audition(index);
    assert!(h.app.engine().is_auditioning());
    h.app.toggle_audition(index);
    assert!(!h.app.engine().is_auditioning());
}

#[test]
fn automation_points_are_added_by_clicking_the_lane() {
    let mut h = Harness::new();
    h.app.automation = Some(AutoParam::Volume);
    h.settle();
    h.click(h.lane(0, 4.0, 0.5));
    let points = &h.app.project.tracks[0].vol_auto;
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].beat, 4.0);
    assert!((points[0].value - 0.75).abs() < 0.1);
    assert_eq!(h.app.project.tracks[0].regions.len(), 1, "regions are not touched in automation view");
    // Drag the point down to silence.
    h.pause();
    h.drag(h.lane(0, 4.0, 0.5), h.lane(0, 6.0, 0.98));
    let points = &h.app.project.tracks[0].vol_auto;
    assert_eq!(points[0].beat, 6.0);
    assert!(points[0].value < 0.05);
    h.pause();
    h.button(h.lane(0, 6.0, 0.92), PointerButton::Secondary);
    assert!(h.app.project.tracks[0].vol_auto.is_empty());
}

#[test]
fn tracks_can_be_added_moved_duplicated_and_deleted() {
    let mut h = Harness::new();
    for kind in [TrackKind::Audio, TrackKind::Guitar, TrackKind::Drummer, TrackKind::Instrument] {
        h.app.add_track(kind);
        assert_eq!(h.app.project.tracks.last().unwrap().kind, kind);
    }
    h.frame(Vec::new());
    assert_eq!(h.app.project.tracks.len(), 8);
    assert!(h.app.project.tracks[5].fx.amp.on, "guitar tracks start with an amp");
    assert_eq!(h.app.project.tracks[6].regions.len(), 1, "drummer tracks start with a region");

    h.app.move_track(7, -1);
    assert_eq!(h.app.project.tracks[6].kind, TrackKind::Instrument);
    h.app.duplicate_track(0);
    assert_eq!(h.app.project.tracks[1].name, "Accords (copie)");
    h.app.delete_track(1);
    assert_eq!(h.app.project.tracks.len(), 8);
    // Zoom out so that bar 11 is on screen, and make room for all eight tracks.
    h.app.ppb = 12.0;
    h.app.show_bottom = false;
    h.settle();

    // Double-click on the drummer lane adds a drummer region.
    let ti = h.app.project.tracks.iter().position(|t| t.kind == TrackKind::Drummer && t.name == "Batteur" && t.regions[0].length == 32.0).unwrap();
    let at = h.lane(ti, 40.0, 0.5);
    h.click(at);
    h.click(at);
    assert_eq!(h.app.project.tracks[ti].regions.len(), 2);
    assert!(matches!(h.app.project.tracks[ti].regions[1].content, Content::Drummer(_)));
}

#[test]
fn takes_no_region_plays_are_forgotten_at_save_and_a_take_can_be_forgotten() {
    let mut h = Harness::new();
    h.app.add_track(TrackKind::Audio);
    let ti = h.app.project.tracks.len() - 1;
    for id in [1, 2, 3] {
        h.app.add_clip(id, &format!("Prise {id}"), std::path::Path::new("nowhere.wav"), ClipData { sample_rate: 48_000, channels: 1, samples: vec![0.1; 480] });
    }
    let mut region = Region::new("Prise", 0.0, 1.0);
    region.content = Content::Audio(AudioRef::new(1));
    region.takes = vec![Content::Audio(AudioRef::new(2))];
    h.app.project.tracks[ti].regions.push(region);
    let ids = |h: &Harness| h.app.project.clips.iter().map(|c| c.id).collect::<Vec<_>>();
    // The clip no region plays, not even as another take, goes at save.
    h.app.prune_clips();
    assert_eq!(ids(&h), [1, 2]);
    // A take forgotten by the user goes the same way; undo brings it back.
    h.app.select_region(ti, 0);
    h.app.remove_take(0);
    assert!(h.app.project.tracks[ti].regions[0].takes.is_empty());
    h.app.prune_clips();
    assert_eq!(ids(&h), [1]);
    h.app.undo();
    assert_eq!(h.app.project.tracks[ti].regions[0].takes.len(), 1);
    assert_eq!(ids(&h), [1, 2]);
}

#[test]
fn projects_with_audio_save_and_reopen() {
    let mut h = Harness::new();
    let dir = std::env::temp_dir().join(format!("opengarage-test-{}", std::process::id()));
    let takes = std::env::temp_dir().join("opengarage");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::create_dir_all(&takes).unwrap();

    // A take as the recorder leaves it: a WAV in the temporary folder.
    let take = takes.join(format!("prise-test-{}.wav", std::process::id()));
    let samples: Vec<f32> = (0..4800).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    og_engine::wav::write_wav(&take, &samples, 48_000, 1, 24).unwrap();
    h.app.add_track(TrackKind::Audio);
    let ti = h.app.project.tracks.len() - 1;
    h.app.add_clip(1, "Prise 1", &take, ClipData { sample_rate: 48_000, channels: 1, samples });
    let mut region = Region::new("Prise 1", 0.0, 1.0);
    region.content = Content::Audio(AudioRef::new(1));
    h.app.project.tracks[ti].regions.push(region);
    h.app.project.notes = "à ne pas perdre".to_owned();
    h.frame(Vec::new());

    let file = dir.join("essai.ogproj");
    h.app.save_to(&file).unwrap();
    // The project is one file, an archive of the project and its takes; the
    // take moved from the temporary folder to the session's, where the
    // project keeps working from.
    let working = std::path::PathBuf::from(&h.app.project.clips[0].path);
    assert!(working.starts_with(h.app.sessions_dir()) && working.is_file(), "{}", working.display());
    let archive = zip::ZipArchive::new(std::fs::File::open(&file).unwrap()).unwrap();
    let names: Vec<&str> = archive.file_names().collect();
    let inside = format!("media/{}", take.file_name().unwrap().to_string_lossy());
    assert_eq!(names, ["project.json", inside.as_str()]);
    drop(archive);

    let saved = h.app.project.clone();
    h.app.replace_project(templates::empty(), None);
    assert!(h.app.clips.is_empty());
    h.app.open_path(&file).unwrap();
    assert_eq!(h.app.project, saved);
    assert_eq!(h.app.clips[&1].samples.len(), 4800);
    assert!(h.app.waveforms.contains_key(&1));
    h.settle();

    // A project whose audio is gone still opens, and says so: an archive
    // that holds the project alone.
    let alone = dir.join("seul.ogproj");
    {
        use std::io::{Read, Write};
        let mut from = zip::ZipArchive::new(std::fs::File::open(&file).unwrap()).unwrap();
        let mut text = String::new();
        from.by_name("project.json").unwrap().read_to_string(&mut text).unwrap();
        let mut to = zip::ZipWriter::new(std::fs::File::create(&alone).unwrap());
        to.start_file("project.json", zip::write::SimpleFileOptions::default()).unwrap();
        to.write_all(text.as_bytes()).unwrap();
        to.finish().unwrap();
    }
    h.app.open_path(&alone).unwrap();
    assert!(h.app.status.contains("introuvable"), "{}", h.app.status);
    h.settle();
    assert!(h.app.open_path(&dir.join("absent.ogproj")).is_err());
    std::fs::write(dir.join("faux.ogproj"), "pas du json").unwrap();
    assert!(h.app.open_path(&dir.join("faux.ogproj")).is_err());

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&take);
}

#[test]
fn every_panel_draws_for_every_kind_of_track() {
    let mut h = Harness::new();
    for kind in [TrackKind::Audio, TrackKind::Guitar] {
        h.app.add_track(kind);
    }
    // An audio region whose clip is missing, and one that is present.
    let samples = vec![0.3f32; 2000];
    h.app.add_clip(9, "son", std::path::Path::new("son.wav"), ClipData { sample_rate: 8000, channels: 1, samples });
    for (clip, start) in [(9u64, 0.0), (404, 8.0)] {
        let mut region = Region::new("audio", start, 4.0);
        region.content = Content::Audio(AudioRef::new(clip));
        h.app.project.tracks[4].regions.push(region);
    }
    h.app.project.tracks[0].regions[0].loop_len = Some(4.0);
    h.app.project.tempo_points.push(og_engine::Point { beat: 4.0, value: 90.0 });
    h.app.project.transpose_points.push(og_engine::Point { beat: 8.0, value: 2.0 });
    h.app.project.markers.push(og_engine::Marker { name: "Intro".to_owned(), start: 0.0, length: 16.0 });
    h.app.show_global = true;
    h.app.show_master = true;
    h.app.typing = true;

    for right in [RightPanel::None, RightPanel::Loops, RightPanel::Notes] {
        h.app.right_panel = right;
        for tab in [BottomTab::Controls, BottomTab::Editor] {
            h.app.bottom_tab = tab;
            for ti in 0..h.app.project.tracks.len() {
                h.app.selected_track = ti;
                h.app.selected_region = None;
                h.settle();
                for ri in 0..h.app.project.tracks[ti].regions.len() {
                    h.app.select_region(ti, ri);
                    for view in [NoteView::Roll, NoteView::Score, NoteView::Tab] {
                        h.app.note_view = view;
                        h.settle();
                    }
                }
            }
        }
    }
    for automation in [Some(AutoParam::Volume), Some(AutoParam::Pan), None] {
        h.app.automation = automation;
        h.settle();
    }
    for dialog in [Dialog::NewProject, Dialog::NewTrack, Dialog::Tuner, Dialog::SavePatch("essai".to_owned())] {
        h.app.dialog = Some(dialog);
        h.settle();
        assert!(h.app.dialog.is_some());
    }
    h.app.dialog = None;
    // Playing through it all, with the view following the playhead.
    h.app.toggle_play();
    for _ in 0..20 {
        h.run_audio(0.5);
        h.frame(Vec::new());
    }
    // An empty project, and a tiny window.
    h.app.replace_project(og_engine::Project::empty(), None);
    h.settle();
    let input = RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 200.0))), ..Default::default() };
    let app = &mut h.app;
    let _ = h.ctx.run(input, |ctx| app.ui(ctx));
}

#[test]
fn locked_tracks_refuse_edits() {
    let mut h = Harness::new();
    h.app.project.tracks[1].locked = true;
    h.settle();
    let before = h.app.project.tracks[1].clone();
    h.drag(h.lane(1, 3.0, 0.5), h.lane(1, 11.0, 0.5));
    h.pause();
    let at = h.lane(1, 24.0, 0.5);
    h.click(at);
    h.click(at);
    h.app.select_region(1, 0);
    h.app.delete_selected_region();
    h.app.split_region();
    h.app.toggle_loop();
    h.app.selected_track = 1;
    h.app.toggle_record();
    assert_eq!(h.app.project.tracks[1], before);
    assert!(!h.app.recording);
}

#[test]
fn the_computer_keyboard_plays_and_records_when_enabled() {
    let mut h = Harness::new();
    let note = |h: &mut Harness, pressed: bool| {
        let event = Event::Key { key: Key::A, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE };
        h.frame(vec![event]);
    };
    // Off by default: letters are shortcuts ("A" toggles automation).
    note(&mut h, true);
    note(&mut h, false);
    assert!(h.app.automation.is_some());
    h.app.automation = None;

    h.key(Key::K, CTRL);
    assert!(h.app.typing);
    h.app.project.count_in = false;
    h.app.selected_track = 0;
    h.app.go_to(16.0);
    h.app.toggle_record();
    h.run_audio(0.2);
    note(&mut h, true);
    h.run_audio(0.6);
    note(&mut h, false);
    assert!(h.app.automation.is_none());
    h.app.toggle_record();
    h.frame(Vec::new());
    let recorded: Vec<Note> = h.app.project.tracks[0].regions[1].notes().to_vec();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].pitch, 60);
}

#[test]
fn an_audio_take_becomes_a_clip_and_a_region() {
    let mut h = Harness::new();
    h.app.add_track(TrackKind::Audio);
    let ti = h.app.project.tracks.len() - 1;
    h.app.go_to(8.0);
    h.app.begin_audio_take_for_test(ti);
    h.run_audio(1.0);
    // One second from a 48 kHz input.
    h.app.audio.inject_capture((0..48_000).map(|i| (i as f32 * 0.03).sin() * 0.4).collect());
    h.app.toggle_record();
    h.frame(Vec::new());
    assert!(!h.app.recording);

    let region = &h.app.project.tracks[ti].regions[0];
    assert_eq!(region.start, 8.0);
    // One second at 100 bpm.
    assert!((region.length - 100.0 / 60.0).abs() < 1e-6, "{}", region.length);
    let Content::Audio(audio) = &region.content else { panic!("not an audio region") };
    assert_eq!(h.app.clips[&audio.clip].samples.len(), 48_000);
    let clip = h.app.project.clips.iter().find(|c| c.id == audio.clip).unwrap();
    let file = std::path::PathBuf::from(&clip.path);
    assert!(file.exists(), "the take is kept on disk");
    // The take is heard on playback.
    let out = og_engine::render_offline(&h.app.project, &h.app.clips, &h.app.fonts, 24_000);
    let from = (8.2 * 0.6 * 24_000.0) as usize * 2;
    assert!(out[from..from + 4800].iter().any(|s| s.abs() > 0.05));
    h.settle();
    // Undo removes the region; an empty take leaves nothing behind.
    h.app.undo();
    assert!(h.app.project.tracks[ti].regions.is_empty());
    h.app.begin_audio_take_for_test(ti);
    h.app.audio.inject_capture(Vec::new());
    h.app.toggle_record();
    h.frame(Vec::new());
    assert!(h.app.project.tracks[ti].regions.is_empty());
    let _ = std::fs::remove_file(file);
}

#[cfg(windows)]
#[test]
fn compressed_export_can_be_read_back() {
    use crate::export::{encode, Compressed};
    // Two seconds of A4 on the left, silence on the right.
    let samples: Vec<f32> = (0..88_200).flat_map(|i| [(i as f32 * 440.0 * std::f32::consts::TAU / 44_100.0).sin() * 0.5, 0.0]).collect();
    let dir = std::env::temp_dir().join(format!("opengarage-export-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for format in [Compressed::Mp3, Compressed::Aac] {
        let file = dir.join(format!("essai.{}", format.extension()));
        encode(&file, &samples, 44_100, format, 192).unwrap();
        let size = std::fs::metadata(&file).unwrap().len();
        // 192 kbit/s for two seconds is 48 kB; uncompressed it would be 353 kB.
        assert!((20_000..120_000).contains(&size), "{format:?}: {size} bytes");

        let clip = og_engine::codec::decode_file(&file).unwrap();
        assert_eq!((clip.sample_rate, clip.channels), (44_100, 2), "{format:?}");
        let seconds = clip.seconds();
        assert!((1.9..2.3).contains(&seconds), "{format:?}: {seconds} s");
        let left: Vec<f32> = clip.samples.chunks(2).map(|f| f[0]).collect();
        let right: Vec<f32> = clip.samples.chunks(2).map(|f| f[1]).collect();
        let found = og_engine::analysis::detect_pitch(&left[40_000..44_096], 44_100.0).unwrap();
        assert!((found - 440.0).abs() < 2.0, "{format:?}: {found} Hz");
        assert!(right[20_000..60_000].iter().all(|s| s.abs() < 0.02), "{format:?}: right channel should stay silent");
    }
    assert!(encode(&dir.join("vide.mp3"), &[], 44_100, Compressed::Mp3, 192).is_err());
    assert!(encode(&dir.join("x.mp3"), &samples, 22_050, Compressed::Mp3, 192).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

fn sine_clip(hz: f32, seconds: f32) -> ClipData {
    let samples = (0..(48_000.0 * seconds) as usize).map(|i| (i as f32 * hz * std::f32::consts::TAU / 48_000.0).sin() * 0.5).collect();
    ClipData { sample_rate: 48_000, channels: 1, samples }
}

/// Adds an audio track holding one region that plays `clip`.
fn audio_track(h: &mut Harness, id: u64, clip: ClipData, beats: f64) -> usize {
    h.app.add_track(TrackKind::Audio);
    let ti = h.app.project.tracks.len() - 1;
    h.app.add_clip(id, "son", std::path::Path::new("son.wav"), clip);
    let mut region = Region::new("son", 0.0, beats);
    region.content = Content::Audio(AudioRef::new(id));
    h.app.project.tracks[ti].regions.push(region);
    h.app.select_region(ti, 0);
    ti
}

#[test]
fn a_sound_bank_adds_sampled_instruments_to_the_library() {
    let mut h = Harness::new();
    let dir = std::env::temp_dir().join(format!("opengarage-font-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Essai.sf2");
    std::fs::write(&file, og_engine::testfont::tiny_soundfont()).unwrap();

    let id = h.app.load_font(&file).unwrap();
    assert_eq!(h.app.load_font(&file).unwrap(), id, "loading twice is harmless");
    assert_eq!(h.app.font_names(), vec!["Essai".to_owned()]);
    let patch = h.app.patches.iter().find(|p| p.category == "Essai" && p.name == "Sinus").cloned().unwrap();
    h.app.selected_track = 2;
    h.app.apply_patch(&patch);
    h.frame(Vec::new());
    assert!(matches!(h.app.project.tracks[2].sound, og_engine::Sound::Sampler(_)));
    assert_eq!(h.app.project.fonts.len(), 1);

    // The melody is now played by the sampled instrument, on its own and in the export.
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    let out = og_engine::render_offline(&h.app.project, &h.app.clips, &h.app.fonts, 24_000);
    assert!(out.iter().any(|s| s.abs() > 0.05));
    let silent = og_engine::render_offline(&h.app.project, &h.app.clips, &og_engine::Fonts::new(), 24_000);
    assert!(silent.iter().all(|s| s.abs() < 1e-4), "without the bank the track is silent");

    // The project remembers the bank and reloads it.
    let project = dir.join("avec-banque.ogproj");
    h.app.save_to(&project).unwrap();
    let mut fresh = Harness::new();
    assert!(fresh.app.fonts.is_empty());
    fresh.app.open_path(&project).unwrap();
    assert_eq!(fresh.app.fonts.len(), 1);
    fresh.settle();
    // A file that is not a sound bank is refused.
    std::fs::write(dir.join("faux.sf2"), b"rien").unwrap();
    assert!(h.app.load_font(&dir.join("faux.sf2")).is_err());
    h.settle();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn transposing_audio_keeps_its_length_and_changes_its_pitch() {
    let mut h = Harness::new();
    let ti = audio_track(&mut h, 5, sine_clip(220.0, 1.0), 2.0);
    h.frame(Vec::new());
    assert_eq!(h.app.clips.len(), 1);
    let Content::Audio(audio) = &mut h.app.project.tracks[ti].regions[0].content else { panic!() };
    audio.transpose = 12;
    let key = audio.key();
    h.frame(Vec::new());
    let processed = h.app.clips.get(&key).expect("the transposed copy is made");
    assert_eq!(processed.samples.len(), 48_000);
    let found = og_engine::analysis::detect_pitch(&processed.samples[20_000..24_096], 48_000.0).unwrap();
    assert!((found - 440.0).abs() < 9.0, "{found}");
    // Back to no transposition: the copy is dropped.
    let Content::Audio(audio) = &mut h.app.project.tracks[ti].regions[0].content else { panic!() };
    audio.transpose = 0;
    h.frame(Vec::new());
    assert_eq!(h.app.clips.len(), 1);
}

#[test]
fn audio_timing_correction_makes_a_new_clip_and_can_be_undone() {
    let mut h = Harness::new();
    // Hits around the beats of a 100 bpm project (0.6 s per beat).
    let mut samples = vec![0.0f32; 96_000];
    for time in [0.03f32, 0.66, 1.17] {
        let at = (time * 48_000.0) as usize;
        for i in 0..2400 {
            samples[at + i] = (i as f32 * 0.1).sin() * (1.0 - i as f32 / 2400.0) * 0.8;
        }
    }
    let ti = audio_track(&mut h, 5, ClipData { sample_rate: 48_000, channels: 1, samples }, 4.0);
    h.app.timing_grid = 1.0;
    h.app.quantize_audio_region();
    let Content::Audio(audio) = h.app.project.tracks[ti].regions[0].content else { panic!() };
    assert_ne!(audio.clip, 5);
    let fixed = &h.app.clips[&audio.clip];
    let onsets = og_engine::stretch::detect_onsets(&fixed.samples, 48_000.0);
    assert_eq!(onsets.len(), 3);
    for (onset, beat) in onsets.iter().zip([0.0f32, 0.6, 1.2]) {
        // The first hit cannot move before the start of the file.
        assert!((*onset as f32 / 48_000.0 - beat).abs() < 0.05, "{onset}");
    }
    let file = std::path::PathBuf::from(&h.app.project.clips.iter().find(|c| c.id == audio.clip).unwrap().path);
    h.app.undo();
    let Content::Audio(audio) = h.app.project.tracks[ti].regions[0].content else { panic!() };
    assert_eq!(audio.clip, 5);
    let _ = std::fs::remove_file(file);
}

#[test]
fn cycle_recording_keeps_every_pass_as_a_take() {
    let mut h = Harness::new();
    h.app.add_track(TrackKind::Audio);
    let ti = h.app.project.tracks.len() - 1;
    // A two-beat cycle at 100 bpm lasts 1.2 s: 57 600 samples at 48 kHz.
    h.app.project.cycle_enabled = true;
    h.app.project.cycle_start = 8.0;
    h.app.project.cycle_end = 10.0;
    h.app.go_to(8.0);
    h.app.begin_audio_take_for_test(ti);
    h.run_audio(0.2);
    // Three passes, each at its own level, then a short unfinished one.
    let mut capture = Vec::new();
    for level in [0.1f32, 0.2, 0.3] {
        capture.extend(std::iter::repeat_n(level, 57_600));
    }
    capture.extend(std::iter::repeat_n(0.9, 5000));
    h.app.audio.inject_capture(capture);
    h.app.toggle_record();
    h.frame(Vec::new());

    let region = h.app.project.tracks[ti].regions[0].clone();
    assert_eq!((region.start, region.length, region.takes.len()), (8.0, 2.0, 2));
    let level_of = |h: &Harness, content: &Content| {
        let Content::Audio(audio) = content else { panic!() };
        h.app.clips[&audio.clip].samples[100]
    };
    assert_eq!(level_of(&h, &region.content), 0.3, "the last full pass is the one heard");
    h.settle();
    h.app.select_take(0);
    let region = &h.app.project.tracks[ti].regions[0];
    assert_eq!(level_of(&h, &region.content), 0.1);
    assert_eq!(region.takes.len(), 2);
    h.app.select_take(9);
    for clip in &h.app.project.clips {
        let _ = std::fs::remove_file(&clip.path);
    }
}

#[test]
fn sections_carry_their_regions() {
    let mut h = Harness::new();
    h.app.project.markers.push(og_engine::Marker { name: "Couplet".to_owned(), start: 0.0, length: 16.0 });
    h.app.duplicate_section(0);
    assert_eq!(h.app.project.markers.len(), 2);
    assert_eq!(h.app.project.markers[1].start, 16.0);
    assert!(h.app.project.tracks.iter().all(|t| t.regions.len() == 2 && t.regions[1].start == 16.0));

    h.app.move_section(1, 32.0);
    assert_eq!(h.app.project.markers[1].start, 32.0);
    assert!(h.app.project.tracks.iter().all(|t| t.regions[0].start == 0.0 && t.regions[1].start == 32.0));
    // A locked track stays where it is.
    h.app.project.tracks[0].locked = true;
    h.app.move_section(1, 48.0);
    assert_eq!(h.app.project.tracks[0].regions[1].start, 32.0);
    assert_eq!(h.app.project.tracks[1].regions[1].start, 48.0);
    h.app.project.tracks[0].locked = false;

    h.app.delete_section(0, true);
    assert_eq!(h.app.project.markers.len(), 1);
    assert!(h.app.project.tracks.iter().all(|t| t.regions.len() == 1));
    h.app.delete_section(0, false);
    assert!(h.app.project.markers.is_empty());
    assert!(h.app.project.tracks.iter().all(|t| t.regions.len() == 1));
    h.app.delete_section(5, true);
    h.settle();
}

#[test]
fn compressed_audio_can_be_brought_into_a_project() {
    // Encode a short tone with the system encoder, then load it as a clip the
    // way a reopened project does.
    let mut h = Harness::new();
    let dir = std::env::temp_dir().join(format!("opengarage-import-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let samples: Vec<f32> = sine_clip(330.0, 1.0).samples.iter().flat_map(|s| [*s, *s]).collect();
    let file = dir.join("son.mp3");
    if crate::export::encode(&file, &samples, 48_000, crate::export::Compressed::Mp3, 192).is_ok() {
        h.app.project.clips.push(og_engine::Clip { id: 3, name: "son".to_owned(), path: file.to_string_lossy().into_owned() });
        h.app.add_track(TrackKind::Audio);
        let mut region = Region::new("son", 0.0, 1.0);
        region.content = Content::Audio(AudioRef::new(3));
        let last = h.app.project.tracks.len() - 1;
        h.app.project.tracks[last].regions.push(region);
        let project = dir.join("p.ogproj");
        h.app.save_to(&project).unwrap();
        h.app.open_path(&project).unwrap();
        assert!(h.app.status.starts_with("Projet ouvert :"), "{}", h.app.status);
        assert!((h.app.clips[&3].seconds() - 1.0).abs() < 0.1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn listening_to_the_input_is_switched_with_the_track() {
    let mut h = Harness::new();
    h.app.add_track(TrackKind::Audio);
    let ti = h.app.project.tracks.len() - 1;
    h.app.project.tracks[ti].monitor = true;
    h.frame(Vec::new());
    assert!(h.app.audio.monitoring.load(std::sync::atomic::Ordering::Relaxed));
    h.app.audio.feed.push(std::iter::repeat_n(0.5, 4800));
    let mut block = vec![0.0f32; 4800];
    h.app.engine().process(&mut block, 2);
    assert!(block[2000..4000].iter().any(|s| s.abs() > 0.1), "the input is heard");
    h.app.project.tracks[ti].monitor = false;
    h.frame(Vec::new());
    assert!(!h.app.audio.monitoring.load(std::sync::atomic::Ordering::Relaxed));
}

/// The plug-in file built from `tools/og-clap-test`, next to the test binary.
fn test_plugin_file() -> std::path::PathBuf {
    let dir = std::env::current_exe().unwrap().parent().unwrap().to_path_buf();
    let name = if cfg!(windows) { "og_clap_test.dll" } else if cfg!(target_os = "macos") { "libog_clap_test.dylib" } else { "libog_clap_test.so" };
    let file = dir.join(name);
    assert!(file.exists(), "{} should have been built with the tests", file.display());
    file
}

#[test]
fn a_clap_plugin_file_is_loaded_and_its_plugins_run() {
    use crate::clap_host::{ClapInstance, ClapLibrary};
    use og_engine::ExternalUnit;
    let library = ClapLibrary::load(&test_plugin_file()).unwrap();
    let names: Vec<(&str, bool)> = library.plugins.iter().map(|p| (p.id.as_str(), p.instrument)).collect();
    assert_eq!(names, vec![("org.opengarage.test.gain", false), ("org.opengarage.test.sine", true)]);

    // The effect: one parameter, applied to what goes through.
    let (mut gain, _gain_handle, params) = ClapInstance::new(&library, "org.opengarage.test.gain", 48_000.0).unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!((params[0].name.as_str(), params[0].min, params[0].max, params[0].default), ("Gain", 0.0, 2.0, 0.5));
    let mut buf = vec![1.0f32; 10_000];
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.5).abs() < 1e-6));
    gain.set_param(params[0].id, 2.0);
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 1.0).abs() < 1e-6));

    // The instrument: sound while a note is held, at the right pitch.
    let (mut sine, _sine_handle, params) = ClapInstance::new(&library, "org.opengarage.test.sine", 48_000.0).unwrap();
    assert!(params.is_empty());
    let mut buf = vec![0.0f32; 9600];
    sine.process(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0));
    sine.note_on(69, 100);
    sine.process(&mut buf);
    let found = og_engine::analysis::detect_pitch(&buf[2000..6096], 48_000.0).unwrap();
    assert!((found - 440.0).abs() < 1.0, "{found}");
    sine.all_notes_off();
    // An instrument adds to what is already in the buffer.
    buf.fill(0.0);
    sine.process(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0));

    assert!(ClapInstance::new(&library, "no.such.plugin", 48_000.0).is_err());
    let not_a_plugin = std::env::temp_dir().join(format!("opengarage-faux-{}.clap", std::process::id()));
    std::fs::write(&not_a_plugin, b"pas un greffon").unwrap();
    assert!(ClapLibrary::load(&not_a_plugin).is_err());
    let _ = std::fs::remove_file(not_a_plugin);
}

#[test]
fn plugins_on_a_track_follow_the_project() {
    use crate::clap_host::ClapLibrary;
    let mut h = Harness::new();
    let file = test_plugin_file();
    let library = ClapLibrary::load(&file).unwrap();
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    h.app.selected_track = 2;
    h.app.add_plugin(&file, &library.plugins[0]);
    h.frame(Vec::new());
    let slot = h.app.project.tracks[2].plugins[0].id;
    assert!(h.app.engine().has_unit(slot));
    assert_eq!(h.app.plugin_params[&slot].len(), 1);

    // Gain at 0 silences the track, in the export too (which uses its own instance).
    h.app.set_plugin_param(slot, 1, 0.0);
    assert_eq!(h.app.project.tracks[2].plugins[0].params, vec![(1, 0.0)]);
    let silent = h.app.render_song_for_test();
    assert!(silent.iter().all(|s| s.abs() < 1e-5));
    h.app.set_plugin_param(slot, 1, 1.0);
    let heard = h.app.render_song_for_test();
    assert!(heard.iter().any(|s| s.abs() > 0.02));

    // The external instrument takes over the notes of the track.
    h.pause();
    h.app.add_plugin(&file, &library.plugins[1]);
    h.frame(Vec::new());
    let instrument = h.app.project.tracks[2].instrument.clone().unwrap();
    assert!(h.app.engine().has_unit(instrument.id));
    let out = h.app.render_song_for_test();
    // The melody starts on E5 (659 Hz): a pure sine now, where the synth was a saw.
    let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
    let found = og_engine::analysis::detect_pitch(&left[4000..8096], 44_100.0).unwrap();
    assert!((found - 659.26).abs() < 4.0, "{found}");
    // An instrument plug-in is refused on a track that is not an instrument track.
    h.app.selected_track = 3;
    h.app.add_plugin(&file, &library.plugins[1]);
    assert!(h.app.project.tracks[3].instrument.is_none());
    h.settle();

    // Undo removes the instrument, and its instance is shut down; redo brings it back.
    h.key(Key::Z, CTRL);
    assert!(h.app.project.tracks[2].instrument.is_none());
    assert!(!h.app.engine().has_unit(instrument.id));
    h.key(Key::Y, CTRL);
    h.frame(Vec::new());
    assert!(h.app.engine().has_unit(instrument.id));
    h.app.remove_plugin(slot);
    h.frame(Vec::new());
    assert!(!h.app.engine().has_unit(slot));

    // A project naming a plug-in file that is gone still opens and plays.
    h.app.project.tracks[2].plugins.push(og_engine::PluginSlot { id: 77, file: "absent.clap".to_owned(), plugin: "x".to_owned(), name: "Absent".to_owned(), on: true, params: Vec::new(), state: None });
    h.settle();
    assert!(h.app.status.contains("indisponible"), "{}", h.app.status);
    assert!(!h.app.engine().has_unit(77));
}

#[test]
fn an_sfz_instrument_is_imported_like_a_sound_bank() {
    let mut h = Harness::new();
    let dir = std::env::temp_dir().join(format!("opengarage-sfz-app-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    og_engine::wav::write_wav(&dir.join("la.wav"), &sine_clip(440.0, 1.0).samples, 48_000, 1, 16).unwrap();
    let file = dir.join("Mon orgue.sfz");
    std::fs::write(&file, "<region> sample=la.wav pitch_keycenter=69 loop_mode=loop_continuous loop_start=0 loop_end=47000").unwrap();

    h.app.load_font(&file).unwrap();
    assert_eq!(h.app.font_names(), vec![og_engine::patches::CAT_SFZ.to_owned()]);
    let patch = h.app.patches.iter().find(|p| p.category == og_engine::patches::CAT_SFZ).cloned().unwrap();
    assert_eq!(patch.name, "Mon orgue");
    h.app.selected_track = 2;
    h.app.apply_patch(&patch);
    h.frame(Vec::new());
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    // The melody's first note is E5: the sampled A4 played a fifth higher.
    let out = h.app.render_song_for_test();
    let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
    let found = og_engine::analysis::detect_pitch(&left[4000..8096], 44_100.0).unwrap();
    assert!((found - 659.26).abs() < 4.0, "{found}");
    assert!(h.app.load_font(&dir.join("absent.sfz")).is_err());
    h.settle();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn deleting_a_track_from_its_own_menu_does_not_crash() {
    // The last track is the dangerous one: its row is still being drawn when
    // the menu asks for it to go.
    let mut h = Harness::new();
    let last = h.app.project.tracks.len() - 1;
    let lane = h.app.lane_rects[last];
    let header = pos2(lane.left() - 60.0, lane.top() + 4.0);
    // The menu opens under the pointer; "Supprimer la piste" is its last entry.
    // A click beside the menu closes it, so it is reopened for every try.
    let mut deleted = false;
    for down in (40..=130).rev().step_by(6).chain((40..=130).step_by(6)) {
        h.pause();
        h.button(header, PointerButton::Secondary);
        h.click(header + vec2(40.0, down as f32));
        if h.app.project.tracks.len() == last {
            deleted = true;
            break;
        }
        if h.app.project.tracks.len() != last + 1 {
            panic!("another menu entry was hit: {} tracks", h.app.project.tracks.len());
        }
    }
    assert!(deleted, "the menu entry was not found");
    assert!(h.app.project.tracks.iter().all(|t| t.name != "Batteur"));
    h.settle();
    // Down to no track at all, then back.
    for _ in 0..last {
        h.app.pending_track = Some((0, crate::app::TrackAction::Delete));
        h.settle();
    }
    assert!(h.app.project.tracks.is_empty());
    h.app.pending_track = Some((5, crate::app::TrackAction::Delete));
    h.settle();
    h.key(Key::Z, CTRL);
    h.settle();
    assert_eq!(h.app.project.tracks.len(), 1);
}

mod downloading {
    use std::io::{Read, Write};
    use std::sync::atomic::AtomicBool;
    use std::sync::Mutex;

    use super::*;
    use crate::app::{gm_category, Section};
    use crate::downloads::{self, Item, Progress, Provider};
    use og_engine::patches;

    /// Serves `body` once per expected request on a local port; returns the URL.
    fn serve(body: Vec<u8>, requests: usize, honest_length: bool) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().take(requests) {
                let Ok(mut stream) = stream else { continue };
                let mut request = [0u8; 2048];
                let _ = stream.read(&mut request);
                let length = if honest_length { body.len() } else { body.len() + 1000 };
                let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
                let _ = stream.write_all(&body);
            }
        });
        format!("http://127.0.0.1:{port}/fichier")
    }

    fn item(url: String, file_name: &str) -> Item {
        Item {
            provider: Provider::MuseScore,
            name: "Banque d'essai".to_owned(),
            author: "Quelqu'un".to_owned(),
            license: "MIT".to_owned(),
            url,
            file_name: file_name.to_owned(),
            size: None,
            sha256: None,
            page: "https://example.org/page".to_owned(),
            note: String::new(),
        }
    }

    fn temp(name: &str) -> std::path::PathBuf {
        super::temp_folder(&format!("dl-{name}"))
    }

    fn run(item: &Item, dir: &std::path::Path) -> Result<Vec<std::path::PathBuf>, String> {
        downloads::download(item, dir, &Mutex::new(Progress::default()), &AtomicBool::new(false))
    }

    #[test]
    fn only_free_licences_are_listed() {
        for free in ["public", "by", "by-sa", "by-3", "wtfpl"] {
            assert!(downloads::artifact_license_is_free(free), "{free}");
        }
        for closed in ["copyright", "gray", "by-nc", "by-nc-nd", "by-nd", "by-nd-3", "by-nc-sa", ""] {
            assert!(!downloads::artifact_license_is_free(closed), "{closed}");
        }
        assert!(downloads::spdx_is_free("CC0-1.0") && downloads::spdx_is_free("CC-BY-4.0"));
        assert!(!downloads::spdx_is_free("NOASSERTION") && !downloads::spdx_is_free("CC-BY-NC-4.0"));

        let artifacts = r#"[
            {"name":"Libre","author":"A","license":"public","file":"https://musical-artifacts.com/artifacts/1/Libre Banque.sf2","file_hash":"a28a2d3d965f76c1d69b65f7afcd90b3a143e7963e9c0b3e15e3b3dab0470442","url":"https://musical-artifacts.com/artifacts/1.json"},
            {"name":"Pas libre","author":"B","license":"by-nc","file":"https://musical-artifacts.com/artifacts/2/x.sf2","file_hash":"","url":""},
            {"name":"Sans fichier","author":"C","license":"public","file":null,"url":""},
            {"name":"Autre format","author":"D","license":"public","file":"https://musical-artifacts.com/artifacts/4/x.exe","url":""},
            {"name":"Pas https","author":"E","license":"public","file":"http://ailleurs.example/x.sf2","url":""}
        ]"#;
        let items = downloads::parse_musical_artifacts(artifacts);
        assert_eq!(items.len(), 1);
        assert_eq!((items[0].name.as_str(), items[0].file_name.as_str()), ("Libre", "Libre_Banque.sf2"));
        assert_eq!(items[0].page, "https://musical-artifacts.com/artifacts/1");
        assert!(items[0].sha256.is_some());
        assert!(downloads::parse_musical_artifacts("pas du json").is_empty());

        let repos = r#"[
            {"name":"karoryfer.cowsynth","license":{"spdx_id":"CC0-1.0"},"default_branch":"master","html_url":"https://github.com/sfzinstruments/karoryfer.cowsynth","size":13000},
            {"name":"SansLicence","license":null,"default_branch":"master","html_url":"https://github.com/sfzinstruments/SansLicence","size":1},
            {"name":"jlearman.jRhodes3c","license":{"spdx_id":"NOASSERTION"},"default_branch":"master","html_url":"https://github.com/sfzinstruments/jlearman.jRhodes3c","size":1},
            {"name":"sfztemplates","license":{"spdx_id":"CC0-1.0"},"default_branch":"main","html_url":"https://github.com/sfzinstruments/sfztemplates","size":1}
        ]"#;
        let items = downloads::parse_github_repos(repos);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].url, "https://github.com/sfzinstruments/karoryfer.cowsynth/archive/refs/heads/master.zip");
        assert_eq!((items[0].license.as_str(), items[0].author.as_str(), items[0].size), ("CC0-1.0", "karoryfer", Some(13_312_000)));
        assert!(downloads::musescore_items().iter().all(|i| i.license == "MIT" && i.url.starts_with("https://ftp.osuosl.org/")));
    }

    #[test]
    fn a_bank_is_downloaded_checked_and_documented() {
        let dir = temp("simple");
        let bank = og_engine::testfont::tiny_soundfont();
        let mut wanted = item(serve(bank.clone(), 1, true), "Mon essai (v2).sf2");
        let files = run(&wanted, &dir).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file_name().unwrap().to_string_lossy(), "Mon_essai__v2_.sf2");
        assert_eq!(std::fs::read(&files[0]).unwrap(), bank);
        let folder = files[0].parent().unwrap();
        assert!(folder.starts_with(dir.join("Banques").join("MuseScore")));
        let source = std::fs::read_to_string(folder.join("SOURCE.txt")).unwrap();
        assert!(source.contains("Licence : MIT") && source.contains("Auteur : Quelqu'un") && source.contains("127.0.0.1"), "{source}");

        // A file that does not match the announced checksum is thrown away.
        wanted.url = serve(bank.clone(), 1, true);
        wanted.name = "Autre".to_owned();
        wanted.sha256 = Some("0".repeat(64));
        assert!(run(&wanted, &dir).unwrap_err().contains("empreinte"));
        assert!(downloads::find_banks(&downloads::item_folder(&dir, &wanted)).is_empty());
        // So is one cut short, and nothing is left behind.
        wanted.sha256 = None;
        wanted.url = serve(bank.clone(), 1, false);
        assert!(run(&wanted, &dir).is_err());
        let leftovers: Vec<_> = std::fs::read_dir(downloads::item_folder(&dir, &wanted)).unwrap().flatten().collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
        // A cancelled download stops with an error.
        wanted.url = serve(bank, 1, true);
        let cancelled = downloads::download(&wanted, &dir, &Mutex::new(Progress::default()), &AtomicBool::new(true));
        assert!(cancelled.unwrap_err().contains("annulé"));
        // A file that is not a bank is reported as such; an unreachable site too.
        wanted.url = serve(b"bonjour".to_vec(), 1, true);
        wanted.file_name = "lisezmoi.txt".to_owned();
        assert!(run(&wanted, &dir).unwrap_err().contains("aucune banque"));
        wanted.url = "http://127.0.0.1:1/absent".to_owned();
        assert!(run(&wanted, &dir).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn archives_are_unpacked_without_leaving_their_folder() {
        let dir = temp("zip");
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("pack-master/Programs/orgue.sfz", options).unwrap();
        archive.write_all(b"<region> sample=../Samples/la.wav pitch_keycenter=69").unwrap();
        archive.start_file("pack-master/Samples/la.wav", options).unwrap();
        archive.write_all(&og_engine::wav::encode_wav(&sine_clip(440.0, 0.5).samples, 48_000, 1, 16).unwrap()).unwrap();
        archive.start_file("pack-master/LICENSE", options).unwrap();
        archive.write_all(b"CC0").unwrap();
        // Entries that try to climb out of the folder.
        archive.start_file("../../evade.sfz", options).unwrap();
        archive.write_all(b"<region> sample=x.wav").unwrap();
        let bytes = archive.finish().unwrap().into_inner();

        let wanted = Item { provider: Provider::SfzInstruments, ..item(serve(bytes, 1, true), "pack.zip") };
        let files = run(&wanted, &dir).unwrap();
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(files[0].ends_with("pack-master/Programs/orgue.sfz"));
        let folder = downloads::item_folder(&dir, &wanted);
        assert!(folder.join("pack-master/LICENSE").exists() && folder.join("SOURCE.txt").exists());
        assert!(!folder.join("pack.zip").exists(), "the archive is removed once unpacked");
        assert!(!dir.join("evade.sfz").exists() && !dir.parent().unwrap().join("evade.sfz").exists());

        // The unpacked instrument loads and lands where asked.
        let mut h = Harness::new();
        assert_eq!(h.app.install_bank(&files[0], &Section::Category(patches::CAT_KEYS.to_owned())).unwrap(), 1);
        let patch = h.app.patches.iter().find(|p| p.name == "orgue").unwrap();
        assert_eq!(patch.category, patches::CAT_KEYS);
        assert!(h.app.font_names().is_empty(), "no extra section was needed");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn instruments_are_filed_by_family() {
        assert_eq!(gm_category(0, 0), patches::CAT_KEYS);
        assert_eq!(gm_category(0, 19), patches::CAT_KEYS);
        assert_eq!(gm_category(0, 12), patches::CAT_MALLET);
        assert_eq!(gm_category(0, 25), patches::CAT_GUITAR);
        assert_eq!(gm_category(0, 33), patches::CAT_BASS);
        assert_eq!(gm_category(0, 48), patches::CAT_ORCH);
        assert_eq!(gm_category(0, 73), patches::CAT_ORCH);
        assert_eq!(gm_category(0, 81), patches::CAT_SYNTH);
        assert_eq!(gm_category(128, 0), patches::CAT_DRUMS);

        let dir = temp("familles");
        let file = dir.join("Essai.sf2");
        std::fs::write(&file, og_engine::testfont::tiny_soundfont()).unwrap();
        let mut h = Harness::new();
        assert_eq!(h.app.install_bank(&file, &Section::Auto).unwrap(), 2);
        let category = |h: &Harness, name: &str| h.app.patches.iter().find(|p| p.name == name).unwrap().category.clone();
        assert_eq!(category(&h, "Sinus"), patches::CAT_KEYS);
        assert_eq!(category(&h, "Kit"), patches::CAT_DRUMS);
        assert_eq!(h.app.settings.banks.len(), 1);
        assert_eq!(h.app.settings.banks[0].section, Section::Auto);
        // Installing again, elsewhere: the instruments move there and the setting follows.
        assert_eq!(h.app.install_bank(&file, &Section::Own).unwrap(), 2);
        assert_eq!(h.app.settings.banks.len(), 1);
        assert_eq!(h.app.settings.banks[0].section, Section::Own);
        assert_eq!((category(&h, "Sinus").as_str(), category(&h, "Kit").as_str()), ("Essai", "Essai"));
        assert_eq!(h.app.patches.iter().filter(|p| p.name == "Sinus").count(), 1, "listed once");
        assert_eq!(h.app.font_files.len(), 1);
        // And back by family, as a download done again with that choice.
        h.app.install_bank(&file, &Section::Auto).unwrap();
        assert_eq!(category(&h, "Kit"), patches::CAT_DRUMS);
        assert_eq!(h.app.settings.banks[0].section, Section::Auto);
        // A bank the project referred to was loaded in its own section before
        // the download asked for families: the download wins.
        let mut again = Harness::new();
        again.app.load_font(&file).unwrap();
        assert_eq!(again.app.patches.iter().find(|p| p.name == "Kit").unwrap().category, "Essai");
        again.app.install_bank(&file, &Section::Auto).unwrap();
        assert_eq!(again.app.patches.iter().find(|p| p.name == "Kit").unwrap().category, patches::CAT_DRUMS);
        // The tracks that use the bank still do: the sound is the same preset.
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn the_download_window_draws_in_every_state() {
        let mut h = Harness::new();
        h.app.dialog = Some(Dialog::Downloads);
        for provider in Provider::BANKS.into_iter().chain(Provider::EFFECTS) {
            h.app.downloads.provider = provider;
            for listing in [None, Some(Err("pas de réseau".to_owned())), Some(Ok(Vec::new())), Some(Ok(downloads::musescore_items()))] {
                h.app.downloads.listing = listing;
                h.app.downloads.selected = Some(0);
                for section in [Section::Auto, Section::Own, Section::Category(patches::CAT_BASS.to_owned())] {
                    h.app.downloads.section = section;
                    h.settle();
                }
            }
        }
        assert!(matches!(h.app.dialog, Some(Dialog::Downloads)));
        // A download in progress, then finished: its bank joins the library.
        let dir = temp("fenetre");
        let wanted = item(serve(og_engine::testfont::tiny_soundfont(), 1, true), "essai.sf2");
        h.app.downloads.section = Section::Auto;
        h.app.downloads.job = Some(downloads::start(wanted, dir.clone()));
        for _ in 0..200 {
            h.frame(Vec::new());
            if h.app.downloads.job.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(h.app.downloads.job.is_none(), "the download should have finished");
        assert!(h.app.downloads.message.contains("2 instruments"), "{}", h.app.downloads.message);
        assert!(h.app.patches.iter().any(|p| p.name == "Sinus" && p.category == patches::CAT_KEYS));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn plugin_releases_are_read_and_plugins_installed_apart_from_sounds() {
        // The 64-bit Windows zip is picked among a release's files.
        let names = |list: &[&str]| list.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        let pick = |list: &[&'static str]| -> Option<&'static str> { downloads::pick_windows_asset(&names(list)).map(|i| list[i]) };
        assert_eq!(pick(&["x-3.2-linux.tar.xz", "x-3.2-win32.zip", "x-3.2-win64.zip", "x-3.2-macos.dmg"]), Some("x-3.2-win64.zip"));
        assert_eq!(pick(&["s-win64-arm64-beta-pluginsonly.zip", "s-win64-NIGHTLY-pluginsonly.zip", "s-win64-NIGHTLY-portable-install.zip", "s-win64-setup.exe"]), Some("s-win64-NIGHTLY-pluginsonly.zip"));
        assert_eq!(pick(&["p-Windows-2026.zip", "p-macOS.dmg"]), Some("p-Windows-2026.zip"));
        assert_eq!(pick(&["p-setup.exe", "p-linux.zip", "p.tar.gz"]), None);

        let project = downloads::CLAP_PROJECTS[1];
        let release = r#"{"tag_name":"v1.0.2","assets":[
            {"name":"wolf-shaper-v1.0.2-linux-x64.zip","size":5,"browser_download_url":"https://github.com/wolf-plugins/wolf-shaper/releases/download/v1.0.2/wolf-shaper-v1.0.2-linux-x64.zip"},
            {"name":"wolf-shaper-v1.0.2-windows-x64.zip","size":5000000,"browser_download_url":"https://github.com/wolf-plugins/wolf-shaper/releases/download/v1.0.2/wolf-shaper-v1.0.2-windows-x64.zip"}
        ]}"#;
        let found = downloads::parse_release(release, &project).unwrap();
        assert_eq!((found.name.as_str(), found.license.as_str(), found.size), ("Wolf Shaper", "GPL-3.0", Some(5_000_000)));
        assert!(found.url.ends_with("windows-x64.zip") && found.note.contains("v1.0.2"));
        assert_eq!(found.provider.kind(), downloads::Kind::Effects);
        // A download address outside the project's own releases is refused.
        let elsewhere = release.replace("https://github.com/wolf-plugins/wolf-shaper/releases/download/v1.0.2/wolf-shaper-v1.0.2-windows-x64.zip", "https://ailleurs.example/w-windows-x64.zip");
        assert!(downloads::parse_release(&elsewhere, &project).is_none());
        assert!(downloads::parse_release("{}", &project).is_none());
        assert!(downloads::CLAP_PROJECTS.iter().all(|p| downloads::spdx_is_free(p.3)));

        // An archive with a plug-in goes under "Effets", away from the sound banks.
        let dir = temp("effets");
        let plugin = std::fs::read(test_plugin_file()).unwrap();
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default();
        archive.start_file("Essai/CLAP/essai.clap", options).unwrap();
        archive.write_all(&plugin).unwrap();
        archive.start_file("Essai/VST2/essai.dll", options).unwrap();
        archive.write_all(b"autre format").unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        let wanted = Item { provider: Provider::ClapGithub, ..item(serve(bytes, 1, true), "essai-win64.zip") };
        let mut h = Harness::new();
        h.app.set_data_dir_for_test(&dir);
        h.app.downloads.provider = Provider::ClapGithub;
        h.app.downloads.job = Some(downloads::start(wanted, dir.clone()));
        for _ in 0..300 {
            h.frame(Vec::new());
            if h.app.downloads.job.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(h.app.downloads.message.contains("1 greffon"), "{}", h.app.downloads.message);
        let files = h.app.known_plugin_files();
        assert_eq!(files.len(), 1, "{files:?}");
        assert!(files[0].starts_with(dir.join("Effets").join("GitHub")), "{}", files[0].display());
        assert!(!dir.join("Banques").exists());
        assert!(h.app.patches.iter().all(|p| p.category != "essai"), "a plug-in is not a sound bank");

        // The installed plug-in is offered in the picker and can be put on a track.
        h.app.selected_track = 2;
        h.app.pick_plugin();
        h.settle();
        assert!(matches!(h.app.dialog, Some(Dialog::PluginPicker)));
        h.app.open_plugin_file(&files[0]);
        h.settle();
        // The test file holds two plug-ins: the chooser asks which one.
        assert!(matches!(h.app.dialog, Some(Dialog::Plugins { .. })));
        // A zip without any CLAP plug-in is reported as such.
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive.start_file("Essai/VST2/essai.dll", options).unwrap();
        archive.write_all(b"autre format").unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        let other = Item { provider: Provider::ClapGithub, name: "Sans CLAP".to_owned(), ..item(serve(bytes, 1, true), "x-win64.zip") };
        assert!(run(&other, &dir).unwrap_err().contains("aucun greffon CLAP"));
        // Plug-in files imported by hand are remembered too.
        h.app.register_plugin_file(&test_plugin_file());
        h.app.register_plugin_file(&test_plugin_file());
        assert_eq!(h.app.settings.plugins.len(), 1);
        assert_eq!(h.app.known_plugin_files().len(), 2);
        drop(h);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Asks GitHub for the real releases and checks that each archive holds a
    /// CLAP plug-in, without loading any of them. Run on demand:
    /// `dev test -p opengaragemusic -- --ignored live_plugin --nocapture`.
    #[test]
    #[ignore = "needs the network and downloads about 100 MB"]
    fn live_plugin_projects_publish_clap_builds() {
        let items = downloads::list(Provider::ClapGithub, "").unwrap();
        eprintln!("{} projets avec une archive Windows", items.len());
        let dir = temp("greffons-reels");
        for item in &items {
            let result = run(item, &dir);
            let summary = match &result {
                Ok(files) => format!("{} fichier(s) .clap : {:?}", files.len(), files.iter().filter_map(|f| f.file_name()).collect::<Vec<_>>()),
                Err(e) => format!("ECHEC : {e}"),
            };
            eprintln!("{} ({}, {}) -> {summary}", item.name, item.license, item.file_name);
        }
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Talks to the real sites and downloads a real SF3 bank. Run on demand:
    /// `dev test -p opengaragemusic -- --ignored live_sites --nocapture`.
    #[test]
    #[ignore = "needs the network and downloads about 40 MB"]
    fn live_sites_answer_and_an_sf3_bank_plays() {
        for provider in [Provider::MuseScore, Provider::MusicalArtifacts, Provider::SfzInstruments] {
            let items = downloads::list(provider, "").unwrap();
            eprintln!("{} : {} éléments, par exemple {:?}", provider.label(), items.len(), items.first().map(|i| (&i.name, &i.license)));
            assert!(!items.is_empty(), "{}", provider.label());
        }
        let dir = temp("reel");
        let sf3 = downloads::musescore_items().into_iter().find(|i| i.file_name.ends_with(".sf3")).unwrap();
        let started = std::time::Instant::now();
        let files = run(&sf3, &dir).unwrap();
        eprintln!("téléchargé en {:.0} s : {:?}", started.elapsed().as_secs_f32(), files);
        let mut h = Harness::new();
        let started = std::time::Instant::now();
        let count = h.app.install_bank(&files[0], &Section::Auto).unwrap();
        eprintln!("{count} instruments, banque SF3 décompressée et chargée en {:.1} s", started.elapsed().as_secs_f32());
        assert!(count > 150);
        // Strings play at a sensible level and in tune.
        let strings = h.app.patches.iter().find(|p| matches!(p.sound, og_engine::Sound::Sampler(s) if s.bank == 0 && s.preset == 48)).cloned().unwrap();
        assert_eq!(strings.category, patches::CAT_ORCH);
        let mut rt = og_engine::synth::SoundRt::new(&strings.sound, 48_000.0, &h.app.fonts);
        rt.note_on(69, 100);
        let mut buf = vec![0.0f32; 48_000];
        rt.render(&mut buf);
        let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let pitch = og_engine::analysis::detect_pitch(&buf[30_000..34_096], 48_000.0);
        eprintln!("cordes : crête {peak:.3}, hauteur {pitch:?}");
        assert!(peak > 0.05);
        assert!(pitch.is_some_and(|hz| (hz - 440.0).abs() < 6.0), "{pitch:?}");
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[cfg(windows)]
#[test]
fn a_plugin_shows_its_own_window_and_its_state_is_kept() {
    use crate::clap_host::{ClapInstance, ClapLibrary};
    use og_engine::ExternalUnit;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::{GetWindow, IsWindow, SendMessageW, GW_CHILD, WM_CLOSE};

    let library = ClapLibrary::load(&test_plugin_file()).unwrap();
    let (mut gain, mut handle, _) = ClapInstance::new(&library, "org.opengarage.test.gain", 48_000.0).unwrap();
    let (_sine, mut sine_handle, _) = ClapInstance::new(&library, "org.opengarage.test.sine", 48_000.0).unwrap();
    assert!(handle.has_window());
    assert!(!sine_handle.has_window(), "the test instrument has no window");
    assert!(sine_handle.open_window("x").is_err());

    // The plug-in draws in a child of a window the host made for it.
    assert!(!handle.window_open());
    handle.open_window("Gain d'essai").unwrap();
    handle.open_window("Gain d'essai").unwrap();
    assert!(handle.window_open());
    let host = HWND(handle.window_handle().unwrap() as isize);
    // SAFETY: plain queries on windows of this process.
    let child = unsafe { GetWindow(host, GW_CHILD) };
    assert!(unsafe { IsWindow(host).as_bool() } && child.0 != 0, "the plug-in's window is inside ours");
    assert!(!handle.idle());
    // Audio keeps running while the window is open.
    let mut buf = vec![1.0f32; 512];
    gain.process(&mut buf);
    assert!((buf[0] - 0.5).abs() < 1e-6);
    // The user closes the window with its close box.
    unsafe { SendMessageW(host, WM_CLOSE, WPARAM(0), LPARAM(0)) };
    assert!(handle.idle(), "the close is noticed");
    assert!(!handle.window_open());
    assert!(unsafe { !IsWindow(host).as_bool() && !IsWindow(child).as_bool() });
    // It can be opened and closed again, and dropped while open.
    handle.open_window("Gain d'essai").unwrap();
    handle.close_window();
    handle.open_window("Gain d'essai").unwrap();

    // State: what the plug-in saves comes back in a new instance.
    gain.set_param(1, 1.7);
    gain.process(&mut buf);
    assert_eq!(handle.param_value(1), Some(1.7));
    let saved = handle.save_state().unwrap();
    assert_eq!(saved, 1.7f64.to_le_bytes());
    drop(handle);
    let (mut fresh, fresh_handle, _) = ClapInstance::new(&library, "org.opengarage.test.gain", 48_000.0).unwrap();
    assert!(fresh_handle.load_state(&saved));
    assert!(!fresh_handle.load_state(&saved[..3]), "a truncated state is refused");
    let mut buf = vec![1.0f32; 512];
    fresh.process(&mut buf);
    assert!((buf[0] - 1.7).abs() < 1e-6);
    // The plug-in outlives its handle for as long as the audio side uses it.
    drop(fresh_handle);
    fresh.process(&mut buf);
}

#[test]
fn what_is_set_in_a_plugin_window_is_saved_with_the_project() {
    use crate::clap_host::ClapLibrary;
    let mut h = Harness::new();
    let file = test_plugin_file();
    let library = ClapLibrary::load(&file).unwrap();
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    h.app.selected_track = 2;
    h.app.add_plugin(&file, &library.plugins[0]);
    h.settle();
    let slot = h.app.project.tracks[2].plugins[0].id;
    // The controls panel draws the plug-in's sliders and window button.
    h.app.bottom_tab = BottomTab::Controls;
    h.settle();
    if cfg!(windows) {
        h.app.toggle_plugin_window(slot, "Gain d'essai");
        assert!(h.app.plugin_handles[&slot].window_open());
        h.settle();
    }
    // Stand-in for the user turning the gain to zero in the plug-in's window.
    assert!(h.app.plugin_handles[&slot].load_state(&0f64.to_le_bytes()));
    if cfg!(windows) {
        h.app.toggle_plugin_window(slot, "Gain d'essai");
        assert!(!h.app.plugin_handles[&slot].window_open());
        assert_eq!(h.app.project.tracks[2].plugins[0].state.as_deref(), Some("0000000000000000"), "closing the window keeps the state");
    }
    // The export, which runs its own instance, hears the same setting.
    let out = h.app.render_song_for_test();
    assert!(out.iter().all(|s| s.abs() < 1e-5), "gain 0 from the plug-in's state");

    // And so does the project once saved and reopened.
    let dir = std::env::temp_dir().join(format!("opengaragemusic-etat-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let project = dir.join("etat.ogproj");
    h.app.save_to(&project).unwrap();
    let mut fresh = Harness::new();
    fresh.app.open_path(&project).unwrap();
    fresh.settle();
    let reopened = &fresh.app.project.tracks[2].plugins[0];
    assert_eq!(reopened.state.as_deref(), Some("0000000000000000"));
    assert_eq!(fresh.app.plugin_handles[&reopened.id].param_value(1), Some(0.0));
    let out = fresh.app.render_song_for_test();
    assert!(out.iter().all(|s| s.abs() < 1e-5));
    // A slider change after that is applied on top of the saved state.
    fresh.app.set_plugin_param(slot, 1, 1.0);
    let out = fresh.app.render_song_for_test();
    assert!(out.iter().any(|s| s.abs() > 0.02));
    // Removing the plug-in closes its window and lets go of it.
    fresh.app.remove_plugin(slot);
    fresh.settle();
    assert!(fresh.app.plugin_handles.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

/// Writes an LV2 folder around the test plug-in library and returns its path.
fn test_lv2_bundle(name: &str) -> std::path::PathBuf {
    let bundle = std::env::temp_dir().join(format!("opengaragemusic-lv2-{}-{name}", std::process::id())).join("essai.lv2");
    let _ = std::fs::remove_dir_all(&bundle);
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::copy(test_plugin_file(), bundle.join("essai.dll")).unwrap();
    std::fs::write(
        bundle.join("manifest.ttl"),
        "@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n\
         @prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n\
         <http://opengaragemusic.example/lv2/gain> a lv2:Plugin ; lv2:binary <essai.dll> ; rdfs:seeAlso <essai.ttl> .\n\
         <http://opengaragemusic.example/lv2/sine> a lv2:Plugin ; lv2:binary <essai.dll> ; rdfs:seeAlso <essai.ttl> .\n",
    )
    .unwrap();
    std::fs::write(
        bundle.join("essai.ttl"),
        r#"@prefix lv2:  <http://lv2plug.in/ns/lv2core#> .
@prefix doap: <http://usefulinc.com/ns/doap#> .
@prefix atom: <http://lv2plug.in/ns/ext/atom#> .
@prefix midi: <http://lv2plug.in/ns/ext/midi#> .
@prefix urid: <http://lv2plug.in/ns/ext/urid#> .
@prefix ui:   <http://lv2plug.in/ns/extensions/ui#> .

<http://opengaragemusic.example/lv2/gain#ui>
    a ui:WindowsUI ;
    ui:binary <essai.dll> ;
    lv2:requiredFeature ui:parent ;
    lv2:optionalFeature ui:resize , ui:idleInterface .

<http://opengaragemusic.example/lv2/gain>
    a lv2:Plugin , lv2:AmplifierPlugin ;
    doap:name "Gain LV2" ;
    ui:ui <http://opengaragemusic.example/lv2/gain#ui> ;
    lv2:requiredFeature urid:map ;
    lv2:optionalFeature lv2:hardRTCapable ;
    lv2:port [
        a lv2:InputPort , lv2:AudioPort ; lv2:index 0 ; lv2:symbol "in" ; lv2:name "In"
    ] , [
        a lv2:OutputPort , lv2:AudioPort ; lv2:index 1 ; lv2:symbol "out" ; lv2:name "Out"
    ] , [
        a lv2:InputPort , lv2:ControlPort ; lv2:index 2 ; lv2:symbol "gain" ; lv2:name "Gain" ;
        lv2:default 0.5 ; lv2:minimum 0.0 ; lv2:maximum 2.0
    ] .

<http://opengaragemusic.example/lv2/sine>
    a lv2:Plugin , lv2:InstrumentPlugin ;
    doap:name "Sinus LV2" ;
    lv2:requiredFeature urid:map ;
    lv2:port [
        a lv2:InputPort , atom:AtomPort ; atom:bufferType atom:Sequence ; atom:supports midi:MidiEvent ;
        lv2:index 0 ; lv2:symbol "events" ; lv2:name "Events"
    ] , [
        a lv2:OutputPort , lv2:AudioPort ; lv2:index 1 ; lv2:symbol "out" ; lv2:name "Out"
    ] .
"#,
    )
    .unwrap();
    bundle
}

#[test]
fn an_lv2_folder_is_described_loaded_and_run() {
    use crate::lv2_host::{describe, Lv2Instance};
    use og_engine::ExternalUnit;
    let bundle = test_lv2_bundle("hote");
    let plugins = describe(&bundle).unwrap();
    let summary: Vec<(&str, bool)> = plugins.iter().map(|p| (p.name.as_str(), p.instrument)).collect();
    assert_eq!(summary, vec![("Gain LV2", false), ("Sinus LV2", true)]);

    // The effect: its control port is a parameter.
    let (mut gain, params) = Lv2Instance::new(&bundle, &plugins[0].id, 48_000.0).unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!((params[0].name.as_str(), params[0].id, params[0].min, params[0].max, params[0].default), ("Gain", 2, 0.0, 2.0, 0.5));
    let mut buf = vec![1.0f32; 10_000];
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.5).abs() < 1e-6));
    gain.set_param(2, 2.0);
    gain.set_param(99, 7.0);
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 1.0).abs() < 1e-6));

    // The instrument: notes reach it as MIDI events.
    let (mut sine, params) = Lv2Instance::new(&bundle, &plugins[1].id, 48_000.0).unwrap();
    assert!(params.is_empty());
    let mut buf = vec![0.0f32; 9600];
    sine.process(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0));
    sine.note_on(69, 100);
    sine.process(&mut buf);
    let found = og_engine::analysis::detect_pitch(&buf[2000..6096], 48_000.0).unwrap();
    assert!((found - 440.0).abs() < 1.0, "{found}");
    sine.all_notes_off();
    buf.fill(0.0);
    sine.process(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0));

    // What the plug-in keeps outside its ports is saved and comes back in a new
    // instance. The test plug-in keeps one factor there, 1 to begin with.
    {
        use crate::lv2_host::Lv2Handle;
        use crate::plugin::PluginHandle;
        let handle = Lv2Handle::new(&bundle, &plugins[0].id, gain.shared());
        let saved = handle.save_state().unwrap();
        assert!(String::from_utf8_lossy(&saved).contains("gain#trim"), "values are kept under their names");
        let one = 1.0f32.to_le_bytes();
        let at = saved.windows(4).rposition(|w| w == one).unwrap();
        let mut halved = saved.clone();
        halved[at..at + 4].copy_from_slice(&0.5f32.to_le_bytes());
        let (mut fresh, _) = Lv2Instance::new(&bundle, &plugins[0].id, 48_000.0).unwrap();
        let fresh_handle = Lv2Handle::new(&bundle, &plugins[0].id, fresh.shared());
        assert!(fresh_handle.load_state(&halved));
        assert!(!fresh_handle.load_state(&halved[..7]) && !fresh_handle.load_state(&[]), "a damaged state is refused");
        let mut buf = vec![1.0f32; 2000];
        fresh.process(&mut buf);
        assert!(buf.iter().all(|s| (*s - 0.25).abs() < 1e-6), "the default gain of a half, halved again");
        assert_eq!(fresh_handle.save_state().unwrap(), halved);
        // Once the plug-in is gone, its handle answers that there is nothing.
        drop(fresh);
        assert!(fresh_handle.save_state().is_none() && !fresh_handle.load_state(&halved));
        // The instrument has no state of its own.
        let (sine, _) = Lv2Instance::new(&bundle, &plugins[1].id, 48_000.0).unwrap();
        assert!(Lv2Handle::new(&bundle, &plugins[1].id, sine.shared()).save_state().is_some());
    }

    // Folders that are not usable plug-ins are refused with a reason.
    assert!(Lv2Instance::new(&bundle, "http://example.org/absent", 48_000.0).is_err());
    let odd = bundle.parent().unwrap().join("autre.lv2");
    std::fs::create_dir_all(&odd).unwrap();
    assert!(describe(&odd).is_err(), "no manifest");
    std::fs::write(odd.join("manifest.ttl"), "@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n<http://x/y> a lv2:Plugin ; lv2:binary <../ailleurs.dll> ;\n lv2:requiredFeature <http://example.org/inconnue> ;\n lv2:port [ a lv2:InputPort , lv2:AudioPort ; lv2:index 0 ] .").unwrap();
    assert!(Lv2Instance::new(&odd, "http://x/y", 48_000.0).err().unwrap().contains("exige"));
    std::fs::write(odd.join("manifest.ttl"), "@prefix lv2: <http://lv2plug.in/ns/lv2core#> .\n<http://x/y> a lv2:Plugin ; lv2:binary <../ailleurs.dll> ;\n lv2:port [ a lv2:OutputPort , lv2:AudioPort ; lv2:index 0 ] .").unwrap();
    assert!(Lv2Instance::new(&odd, "http://x/y", 48_000.0).err().unwrap().contains("dossier"));
    drop((gain, sine));
    let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
}

#[test]
fn an_lv2_plugin_goes_on_a_track_like_any_other() {
    let mut h = Harness::new();
    let bundle = test_lv2_bundle("piste");
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    h.app.selected_track = 2;
    h.app.register_plugin_file(&bundle);
    assert_eq!(h.app.known_plugin_files(), vec![bundle.clone()]);
    // Two plug-ins in the folder: the chooser asks which, without running anything.
    h.app.open_plugin_file(&bundle);
    let Some(Dialog::Plugins { options, .. }) = &h.app.dialog else { panic!("the chooser should be open") };
    let (gain, sine) = (options[0].clone(), options[1].clone());
    h.settle();
    h.app.add_plugin(&bundle, &gain);
    h.settle();
    let slot = h.app.project.tracks[2].plugins[0].id;
    assert!(h.app.engine().has_unit(slot));
    assert_eq!(h.app.plugin_handles[&slot].has_window(), cfg!(windows), "the window is known, not opened");
    assert!(h.app.plugin_handles.values().all(|handle| !handle.window_open()));
    h.app.bottom_tab = BottomTab::Controls;
    h.settle();
    h.app.set_plugin_param(slot, 2, 0.0);
    assert!(h.app.render_song_for_test().iter().all(|s| s.abs() < 1e-5));
    h.app.set_plugin_param(slot, 2, 1.0);
    h.pause();
    h.app.add_plugin(&bundle, &sine);
    h.settle();
    let out = h.app.render_song_for_test();
    let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
    let found = og_engine::analysis::detect_pitch(&left[4000..8096], 44_100.0).unwrap();
    assert!((found - 659.26).abs() < 4.0, "{found}");
    drop(h);
    let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
}

#[test]
fn an_lv2_plugin_shows_its_own_window_on_request() {
    let mut h = Harness::new();
    let bundle = test_lv2_bundle("fenetre");
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    h.app.selected_track = 2;
    let plugins = crate::lv2_host::describe(&bundle).unwrap();
    h.app.add_plugin(&bundle, &plugins[0]);
    h.app.bottom_tab = BottomTab::Controls;
    h.settle();
    let slot = h.app.project.tracks[2].plugins[0].id;
    // Sliders to begin with: the window's code is not even loaded.
    let handle = &h.app.plugin_handles[&slot];
    if !handle.has_window() {
        if cfg!(windows) {
            panic!("on Windows the test plug-in's window is found");
        }
        return;
    }
    assert!(!handle.window_open());
    assert_eq!(handle.param_value(2), Some(0.5));

    // Right-click on the plug-in's box, first line of the menu: its own window.
    let at = h.app.slot_rects.iter().find(|s| s.0 == slot).unwrap().1.right_top() + vec2(-4.0, 6.0);
    h.button(at, PointerButton::Secondary);
    h.pause();
    h.click(at + vec2(30.0, 16.0));
    h.settle();
    assert!(h.app.plugin_handles[&slot].window_open(), "{} / hidden {:?} / slot {:?} at {at:?}", h.app.status, h.app.settings.panels_hidden, h.app.slot_rects);
    // What the host's slider sets reaches the window, and what is turned in
    // the window (the test window answers 0.25 with 1.5) comes back as a setting.
    h.app.set_plugin_param(slot, 2, 0.25);
    h.settle();
    assert_eq!(h.app.project.tracks[2].plugins[0].params, vec![(2, 1.5)]);
    assert_eq!(h.app.plugin_handles[&slot].param_value(2), Some(1.5));
    let out = h.app.render_song_for_test();
    let reference = {
        h.app.set_plugin_param(slot, 2, 1.0);
        h.app.render_song_for_test()
    };
    let (loud, plain) = (out.iter().fold(0.0f32, |m, s| m.max(s.abs())), reference.iter().fold(0.0f32, |m, s| m.max(s.abs())));
    assert!(loud > plain * 1.2, "the export hears what the window set: {loud} against {plain}");

    // Closing the window, from the menu or by its close box, goes back to the sliders.
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{GetWindow, IsWindow, SendMessageW, GW_CHILD, WM_CLOSE};
        let host = HWND(h.app.plugin_handles[&slot].window_handle().unwrap() as isize);
        // SAFETY: plain queries and a close request on a window of this process.
        unsafe {
            assert!(GetWindow(host, GW_CHILD).0 != 0, "the plug-in's window is inside ours");
            SendMessageW(host, WM_CLOSE, WPARAM(0), LPARAM(0));
        }
        h.settle();
        assert!(!h.app.plugin_handles[&slot].window_open());
        assert!(unsafe { !IsWindow(host).as_bool() });
    }
    h.app.toggle_plugin_window(slot, "Gain LV2");
    assert!(h.app.plugin_handles[&slot].window_open());
    h.app.toggle_plugin_window(slot, "Gain LV2");
    assert!(!h.app.plugin_handles[&slot].window_open());
    // A plug-in without a described window has none; one that is removed loses it.
    h.pause();
    h.app.add_plugin(&bundle, &plugins[1]);
    h.settle();
    let sine = h.app.project.tracks[2].instrument.as_ref().unwrap().id;
    assert!(!h.app.plugin_handles[&sine].has_window());
    h.app.toggle_plugin_window(slot, "Gain LV2");
    h.app.remove_plugin(slot);
    h.settle();
    assert!(!h.app.plugin_handles.contains_key(&slot));
    drop(h);
    let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
}

/// Puts the test plug-in library where a VST3 host expects one: as a single
/// file, and as a folder of the same name. Returns (folder of both, file, bundle).
fn test_vst3_files(name: &str) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-vst3-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let bundle = dir.join("Dossier.vst3");
    let inside = bundle.join("Contents").join("x86_64-win");
    std::fs::create_dir_all(&inside).unwrap();
    let file = dir.join("essai.vst3");
    std::fs::copy(test_plugin_file(), &file).unwrap();
    std::fs::copy(test_plugin_file(), inside.join("Dossier.vst3")).unwrap();
    (dir, file, bundle)
}

#[test]
fn a_vst3_file_is_loaded_and_its_plugins_run() {
    use crate::plugin::PluginHandle;
    use crate::vst3_host::{is_vst3, Vst3Instance, Vst3Library};
    use og_engine::ExternalUnit;
    let (dir, file, bundle) = test_vst3_files("hote");
    assert!(is_vst3(&file) && is_vst3(&bundle) && !is_vst3(&dir));
    assert!(!crate::plugin::is_lv2(&bundle), "a VST3 folder is not taken for an LV2 one");
    assert_eq!((crate::plugin::plugin_format(&bundle), crate::plugin::plugin_format(&dir.join("x.clap"))), ("VST3", "CLAP"));

    let library = Vst3Library::load(&file).unwrap();
    let summary: Vec<(&str, bool)> = library.plugins.iter().map(|p| (p.name.as_str(), p.instrument)).collect();
    assert_eq!(summary, vec![("Gain VST3", false), ("Sinus VST3", true), ("Gain 64 VST3", false)], "the controller class is not a plug-in");

    // The effect: one object for sound, parameters and window.
    let (mut gain, mut handle, params) = Vst3Instance::new(&library, &library.plugins[0].id, 48_000.0).unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!((params[0].name.as_str(), params[0].id, params[0].min, params[0].max, params[0].default), ("Gain", 7, 0.0, 1.0, 0.25));
    let mut buf = vec![1.0f32; 10_000];
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.5).abs() < 1e-6), "a quarter of the way is a gain of a half");
    gain.set_param(7, 1.0);
    gain.set_param(99, 0.3);
    buf.fill(1.0);
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 2.0).abs() < 1e-6));
    assert_eq!(handle.param_value(7), Some(1.0), "both halves are the same object");
    // What the plug-in reports while computing (its level, a read-only
    // parameter that gets no slider) reaches the half that draws.
    buf.fill(0.25);
    gain.process(&mut buf);
    assert_eq!(handle.param_value(8), Some(0.0));
    assert!(handle.idle().changes.is_empty(), "a report is not an edit");
    assert_eq!(handle.param_value(8), Some(0.5));

    // What the sliders set reaches the half that draws the window; what is
    // turned there (the test plug-in answers 0.125 with 0.75) comes back.
    assert!(handle.idle().changes.is_empty());
    handle.param_changed(7, 0.125);
    assert_eq!(handle.idle().changes, vec![(7, 0.75)]);
    assert_eq!(handle.param_value(7), Some(0.75));

    // State: what the plug-in saves comes back in a new instance.
    let saved = handle.save_state().unwrap();
    let (mut fresh, fresh_handle, _) = Vst3Instance::new(&library, &library.plugins[0].id, 48_000.0).unwrap();
    assert!(fresh_handle.load_state(&saved));
    assert!(!fresh_handle.load_state(&saved[..3]) && !fresh_handle.load_state(&[200, 0, 0, 0, 1]), "a damaged state is refused");
    buf.fill(1.0);
    fresh.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 1.5).abs() < 1e-6));
    drop(fresh_handle);
    fresh.process(&mut buf);

    // The window: the plug-in draws in a child of a window the host made for it.
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
        use windows::Win32::UI::WindowsAndMessaging::{GetWindow, IsWindow, SendMessageW, GW_CHILD, WM_CLOSE};
        assert!(handle.has_window() && !handle.window_open());
        handle.open_window("Gain VST3").unwrap();
        handle.open_window("Gain VST3").unwrap();
        let host = HWND(handle.window_handle().unwrap() as isize);
        // SAFETY: plain queries and a close request on windows of this process.
        let child = unsafe { GetWindow(host, GW_CHILD) };
        assert!(child.0 != 0, "the plug-in's window is inside ours");
        gain.process(&mut buf);
        unsafe { SendMessageW(host, WM_CLOSE, WPARAM(0), LPARAM(0)) };
        assert!(handle.idle().closed, "the close is noticed");
        assert!(!handle.window_open());
        assert!(unsafe { !IsWindow(host).as_bool() && !IsWindow(child).as_bool() });
        // It can be opened and closed again, and dropped while open.
        handle.open_window("Gain VST3").unwrap();
        handle.close_window();
        handle.open_window("Gain VST3").unwrap();
    }
    drop(handle);
    gain.process(&mut buf);

    // The instrument: notes reach it as events; its controller is an object of
    // its own, which has no window to offer.
    let (mut sine, mut sine_handle, params) = Vst3Instance::new(&library, &library.plugins[1].id, 48_000.0).unwrap();
    assert!(params.is_empty());
    assert!(sine_handle.open_window("x").is_err());
    let mut buf = vec![0.0f32; 9600];
    sine.process(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0));
    sine.note_on(69, 100);
    sine.process(&mut buf);
    let found = og_engine::analysis::detect_pitch(&buf[2000..6096], 48_000.0).unwrap();
    assert!((found - 440.0).abs() < 1.0, "{found}");
    // It splits its sound between two outputs: both are heard.
    let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!((peak - 0.3).abs() < 0.01, "{peak}");
    sine.all_notes_off();
    buf.fill(0.0);
    sine.process(&mut buf);
    assert!(buf.iter().all(|s| *s == 0.0));

    // A plug-in that only computes in 64 bits is given its sound in that size.
    let (mut wide, _wide_handle, _) = Vst3Instance::new(&library, &library.plugins[2].id, 48_000.0).unwrap();
    let mut buf = vec![0.5f32; 5000];
    wide.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.25).abs() < 1e-6));
    drop(wide);

    // The folder form loads the same; what is not a plug-in is refused with a reason.
    assert_eq!(Vst3Library::load(&bundle).unwrap().plugins.len(), 3);
    assert!(Vst3Instance::new(&library, "absent", 48_000.0).is_err());
    std::fs::write(dir.join("faux.vst3"), b"rien").unwrap();
    assert!(Vst3Library::load(&dir.join("faux.vst3")).is_err());
    drop((gain, sine, sine_handle, fresh, library));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_vst3_plugin_goes_on_a_track_and_keeps_its_state() {
    let mut h = Harness::new();
    let (dir, file, bundle) = test_vst3_files("piste");
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    h.app.selected_track = 2;
    // Downloaded plug-ins are found as files or as folders; a folder is one plug-in.
    assert_eq!(crate::downloads::find_vst3(&dir), vec![bundle.clone(), file.clone()]);
    assert_eq!(crate::downloads::usable(&dir, crate::downloads::Kind::Effects).len(), 2);
    h.app.register_plugin_file(&file);
    assert_eq!(h.app.known_plugin_files(), vec![file.clone()]);
    // Two plug-ins in the file: the chooser asks which.
    h.app.open_plugin_file(&file);
    let Some(Dialog::Plugins { options, .. }) = &h.app.dialog else { panic!("the chooser should be open: {}", h.app.status) };
    let (gain, sine) = (options[0].clone(), options[1].clone());
    h.settle();
    h.app.add_plugin(&file, &gain);
    h.app.bottom_tab = BottomTab::Controls;
    h.settle();
    let slot = h.app.project.tracks[2].plugins[0].id;
    assert!(h.app.engine().has_unit(slot), "{}", h.app.status);
    assert!(h.app.plugin_handles.contains_key(&slot));
    h.app.set_plugin_param(slot, 7, 0.0);
    assert!(h.app.render_song_for_test().iter().all(|s| s.abs() < 1e-5), "a gain of zero");
    // The plug-in's own edit (it answers 0.125 with 0.75) becomes the setting.
    h.app.set_plugin_param(slot, 7, 0.125);
    h.settle();
    assert_eq!(h.app.project.tracks[2].plugins[0].params, vec![(7, 0.75)]);
    if cfg!(windows) {
        h.app.toggle_plugin_window(slot, "Gain VST3");
        assert!(h.app.plugin_handles[&slot].window_open(), "{}", h.app.status);
        h.settle();
        h.app.toggle_plugin_window(slot, "Gain VST3");
        assert!(!h.app.plugin_handles[&slot].window_open());
    }
    let loud = h.app.render_song_for_test().iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!(loud > 0.05);

    // Saved and reopened, the project sounds the same.
    let project = dir.join("etat.ogproj");
    h.app.save_to(&project).unwrap();
    let mut fresh = Harness::new();
    fresh.app.open_path(&project).unwrap();
    fresh.settle();
    assert!(fresh.app.project.tracks[2].plugins[0].state.is_some());
    let again = fresh.app.render_song_for_test().iter().fold(0.0f32, |m, s| m.max(s.abs()));
    assert!((again - loud).abs() < 1e-4, "{again} against {loud}");

    // The instrument replaces the track's sound.
    h.pause();
    h.app.set_plugin_param(slot, 7, 0.5);
    h.app.add_plugin(&file, &sine);
    h.settle();
    let out = h.app.render_song_for_test();
    let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
    let found = og_engine::analysis::detect_pitch(&left[4000..8096], 44_100.0).unwrap();
    assert!((found - 659.26).abs() < 4.0, "{found}");
    drop((h, fresh));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_plugin_in_its_own_process_plays_and_its_crash_is_survived() {
    use crate::plugin::PluginHandle;
    use og_engine::ExternalUnit;
    let file = test_plugin_file();
    let (mut gain, mut handle, params) = crate::isolate::start(&file, "org.opengarage.test.gain", 48_000.0).unwrap();
    assert_eq!(params.len(), 1);
    assert_eq!(params[0].id, 1);
    // Sound goes to the other process and comes back processed, in blocks of any size.
    let mut buf = vec![1.0f32; 10_000];
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.5).abs() < 1e-6));
    gain.set_param(1, 1.5);
    buf.fill(1.0);
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 1.5).abs() < 1e-6));

    // The interface's requests: values, state, window.
    assert_eq!(handle.param_value(1), None, "asked for now, known at the next idle call");
    let idle = handle.idle();
    assert!(!idle.closed && idle.changes.is_empty() && idle.failed.is_none());
    assert_eq!(handle.param_value(1), Some(1.5));
    let saved = handle.save_state().unwrap();
    assert_eq!(saved, 1.5f64.to_le_bytes());
    assert!(handle.load_state(&0.25f64.to_le_bytes()) && !handle.load_state(&[1, 2, 3]));
    buf.fill(1.0);
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.25).abs() < 1e-6));
    if cfg!(windows) {
        assert!(handle.has_window() && !handle.window_open());
        handle.open_window("Gain d'essai").unwrap();
        assert!(handle.window_open());
        gain.process(&mut buf);
        handle.close_window();
        assert!(!handle.idle().closed && !handle.window_open());
    }

    // The plug-in brings its process down (the test plug-in does at this
    // value). The application is still here: the sound passes through
    // untouched, and the loss is told once.
    gain.set_param(1, -1313.0);
    buf.fill(0.7);
    gain.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.7).abs() < 1e-6), "an effect that is gone is bypassed");
    gain.note_on(60, 100);
    gain.all_notes_off();
    gain.process(&mut buf);
    let idle = handle.idle();
    assert!(idle.failed.is_some_and(|reason| reason.contains("processus")));
    assert!(handle.idle().failed.is_none(), "told once");
    assert!(!handle.has_window() && handle.save_state().is_none() && handle.open_window("x").is_err());
    drop((gain, handle));

    // The instrument, a VST3 and an LV2 plug-in run the same way.
    let (mut sine, sine_handle, _) = crate::isolate::start(&file, "org.opengarage.test.sine", 48_000.0).unwrap();
    let mut buf = vec![0.0f32; 9600];
    sine.note_on(69, 100);
    sine.process(&mut buf);
    let found = og_engine::analysis::detect_pitch(&buf[2000..6096], 48_000.0).unwrap();
    assert!((found - 440.0).abs() < 1.0, "{found}");
    drop((sine, sine_handle));
    let (dir, vst3, _) = test_vst3_files("isole");
    let library = crate::vst3_host::Vst3Library::load(&vst3).unwrap();
    let (mut other, other_handle, params) = crate::isolate::start(&vst3, &library.plugins[0].id, 48_000.0).unwrap();
    assert_eq!(params[0].id, 7);
    let mut buf = vec![1.0f32; 500];
    other.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.5).abs() < 1e-6));
    drop((other, other_handle, library));
    let bundle = test_lv2_bundle("isole");
    let (mut third, third_handle, _) = crate::isolate::start(&bundle, "http://opengaragemusic.example/lv2/gain", 48_000.0).unwrap();
    buf.fill(1.0);
    third.process(&mut buf);
    assert!(buf.iter().all(|s| (*s - 0.5).abs() < 1e-6));
    assert!(third_handle.save_state().is_some());
    drop((third, third_handle));

    // A file that is not a plug-in is refused with the reason the other process gives.
    let Err(reason) = crate::isolate::start(&dir.join("absent.clap"), "x", 48_000.0) else { panic!("nothing to start") };
    assert!(!reason.is_empty());
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_dir_all(bundle.parent().unwrap());
}

#[test]
fn a_project_survives_a_plugin_that_crashes_and_can_start_it_again() {
    let mut h = Harness::new();
    h.app.isolate_for_test = true;
    let file = test_plugin_file();
    let library = crate::clap_host::ClapLibrary::load(&file).unwrap();
    for other in [0, 1, 3] {
        h.app.project.tracks[other].mute = true;
    }
    h.app.selected_track = 2;
    h.app.add_plugin(&file, &library.plugins[0]);
    h.app.bottom_tab = BottomTab::Controls;
    h.settle();
    let slot = h.app.project.tracks[2].plugins[0].id;
    assert!(h.app.engine().has_unit(slot), "{}", h.app.status);
    h.app.set_plugin_param(slot, 1, 0.0);
    assert!(h.app.render_song_for_test().iter().all(|s| s.abs() < 1e-5), "the export uses processes of its own too");
    h.run_audio(0.1);
    h.settle();
    assert!(h.app.plugin_failed.is_empty());

    // The plug-in brings its process down while the song plays.
    h.app.set_plugin_param(slot, 1, -1313.0);
    h.run_audio(0.2);
    h.settle();
    assert_eq!(h.app.plugin_failed, vec![slot]);
    assert!(h.app.status.contains("Gain d'essai") && h.app.status.contains("processus"), "{}", h.app.status);
    assert!(!h.app.plugin_handles.contains_key(&slot));
    // The project is intact and keeps playing, the track without its effect.
    assert_eq!(h.app.project.tracks[2].plugins.len(), 1);
    h.run_audio(0.1);
    h.settle();

    // Started again with a setting that does not hurt, it works as before.
    h.app.set_plugin_param(slot, 1, 0.0);
    h.app.restart_plugin(slot);
    h.pause();
    h.settle();
    assert!(h.app.plugin_failed.is_empty() && h.app.plugin_handles.contains_key(&slot), "{}", h.app.status);
    assert!(h.app.render_song_for_test().iter().all(|s| s.abs() < 1e-5));
}

#[test]
fn a_capture_and_impulses_go_with_the_project_and_are_heard() {
    let dir = temp_folder("captures");
    // A Proteus capture that passes the sound as it is, and impulses: a
    // click for the cabinet, two clicks for the room.
    let row = |n: usize| format!("[{}]", vec!["0"; n].join(","));
    let rows = |count: usize, n: usize| format!("[{}]", vec![row(n); count].join(","));
    let capture = dir.join("pass.json");
    std::fs::write(&capture, format!(r#"{{"model_data": {{"model": "SimpleRNN", "input_size": 1, "skip": 1, "output_size": 1, "unit_type": "LSTM", "num_layers": 1, "hidden_size": 2, "bias_fl": true}},
        "state_dict": {{"rec.weight_ih_l0": {}, "rec.weight_hh_l0": {}, "rec.bias_ih_l0": {}, "rec.bias_hh_l0": {}, "lin.weight": {}, "lin.bias": [0]}}}}"#, rows(8, 1), rows(8, 2), row(8), row(8), rows(1, 2))).unwrap();
    let cabinet = dir.join("cab.wav");
    let mut click = vec![0.0f32; 512];
    click[0] = 1.0;
    og_engine::wav::write_wav(&cabinet, &click, 48_000, 1, 16).unwrap();
    let room = dir.join("room.wav");
    let mut clicks = vec![0.0f32; 4800];
    clicks[0] = 0.7;
    clicks[2400] = 0.7;
    og_engine::wav::write_wav(&room, &clicks, 48_000, 1, 16).unwrap();

    let mut h = Harness::new();
    for other in 1..h.app.project.tracks.len() {
        h.app.project.tracks[other].mute = true;
    }
    h.app.set_capture(0, Some(&capture));
    h.app.set_cabinet(0, Some(&cabinet));
    h.app.set_reverb_impulse(Some(&room));
    h.settle();
    let fx = &h.app.project.tracks[0].fx;
    assert!(fx.amp.on && fx.capture == capture.to_string_lossy() && fx.cabinet == cabinet.to_string_lossy());
    assert_eq!(h.app.project.reverb_impulse, room.to_string_lossy());
    assert_eq!(h.app.captures.gear[&fx.capture].about.format, "Proteus");
    assert!((h.app.captures.impulses[&h.app.project.reverb_impulse].seconds() - 0.1).abs() < 1e-6);
    let heard = h.app.render_song_for_test();
    assert!(heard.iter().any(|s| s.abs() > 0.01), "the track is heard through the capture and the cabinet");
    // A file that is not a capture is refused, the amp left as it was.
    h.app.set_capture(0, Some(&room));
    assert!(h.app.status.contains(crate::i18n::tr("This file cannot be used: {e}").split(':').next().unwrap()));
    assert_eq!(h.app.project.tracks[0].fx.capture, capture.to_string_lossy());

    // Saved beside the files, the project names them from there; opened
    // again, it finds them.
    let file = dir.join("song.ogproj");
    h.app.save_to(&file).unwrap();
    let fx = &h.app.project.tracks[0].fx;
    assert_eq!((fx.capture.as_str(), fx.cabinet.as_str()), (capture.to_string_lossy().as_ref(), cabinet.to_string_lossy().as_ref()), "the user's files stay by their path");
    assert!(h.app.captures.gear.contains_key(fx.capture.as_str()) && h.app.captures.impulses.contains_key(h.app.project.reverb_impulse.as_str()));
    let mut again = Harness::new();
    again.app.open_path(&file).unwrap();
    assert!(again.app.status.starts_with(&tf!("Project opened: {}", file.display())), "{}", again.app.status);
    assert_eq!(again.app.captures.gear.len(), 1);
    assert_eq!(again.app.captures.impulses.len(), 2);
    // Taken away, the built-in amp and reverb are back.
    again.app.set_capture(0, None);
    again.app.set_cabinet(0, None);
    again.app.set_reverb_impulse(None);
    assert!(again.app.project.tracks[0].fx.capture.is_empty() && again.app.project.reverb_impulse.is_empty());
    // A project whose files are gone says so.
    std::fs::remove_file(&capture).unwrap();
    let mut third = Harness::new();
    third.app.open_path(&file).unwrap();
    assert!(third.app.status.contains("pass.json"), "{}", third.app.status);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_program_comes_back_as_it_was_left() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-session-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let data = dir.join("donnees");
    let sessions = data.join("Sessions");
    // A take that only exists in the temporary folder, as after a recording.
    let takes = std::env::temp_dir().join("opengarage");
    std::fs::create_dir_all(&takes).unwrap();
    let take = takes.join(format!("prise-session-{}.wav", std::process::id()));
    let sound = sine_clip(330.0, 0.25);
    og_engine::wav::write_wav(&take, &sound.samples, sound.sample_rate, sound.channels, 16).unwrap();

    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&data);
    assert_eq!(h.app.sessions_dir(), sessions, "in the data folder, for the tests");
    assert!(!h.app.is_modified(), "the demonstration project is as it was made");
    assert!(!h.app.restore_session(), "nothing was left yet");
    // The demonstration, untouched and in no file, is not worth a session.
    assert!(h.app.closing());
    assert!(crate::sessions::list(&sessions).is_empty());
    h.app.quitting = false;
    // The project is changed and never saved; the window is arranged.
    h.app.project.tempo = 93.0;
    h.app.project.tracks[1].name = "Ma basse".to_owned();
    h.app.add_clip(4242, "Prise", &take, sound);
    // Played by a region: a take no region plays would go at the next save.
    h.app.add_track(TrackKind::Audio);
    let mut region = Region::new("Prise", 0.0, 1.0);
    region.content = Content::Audio(AudioRef::new(4242));
    let last = h.app.project.tracks.len() - 1;
    h.app.project.tracks[last].regions.push(region);
    h.app.selected_track = 2;
    h.app.bottom_tab = BottomTab::Controls;
    h.app.right_panel = RightPanel::Loops;
    h.app.show_global = true;
    h.app.ppb = 41.0;
    h.app.go_to(6.0);
    h.settle();
    assert!(h.app.is_modified());
    // Closing the window: nothing is asked, the project is kept as a session.
    assert!(h.app.closing() && h.app.dialog.is_none());
    let kept = crate::sessions::list(&sessions);
    assert_eq!(kept.len(), 1);
    assert!(kept[0].dir.join("session.json").is_file() && kept[0].dir.join("about.json").is_file());
    assert_eq!((kept[0].about.tracks, kept[0].about.path.is_none()), (h.app.project.tracks.len(), true));
    let project_left = h.app.project.clone();
    drop(h);
    std::fs::remove_file(&take).unwrap();

    // The next start finds everything, the unsaved changes and the take included.
    let mut again = Harness::new();
    again.app.set_data_dir_for_test(&data);
    assert!(again.app.restore_session());
    again.settle();
    assert_eq!((again.app.project.tempo, again.app.project.tracks[1].name.as_str()), (93.0, "Ma basse"));
    assert_eq!(again.app.project.tracks, project_left.tracks);
    assert_eq!(again.app.project.id, project_left.id, "the same session");
    assert!(again.app.is_modified(), "what was not in a file still is not");
    assert_eq!((again.app.selected_track, again.app.ppb, again.app.position), (2, 41.0, 6.0));
    assert!(again.app.bottom_tab == BottomTab::Controls && again.app.right_panel == RightPanel::Loops && again.app.show_global);
    assert!(again.app.clips.contains_key(&4242), "{}", again.app.status);
    let kept = std::path::PathBuf::from(&again.app.project.clips[0].path);
    assert!(kept.starts_with(&sessions) && kept.is_file());

    // Saved for good, the project takes its audio along and is no longer "modified".
    let file = dir.join("Morceau.ogproj");
    again.app.save_to(&file).unwrap();
    assert!(!again.app.is_modified());
    assert_eq!(again.app.project.clips[0].path, kept.to_string_lossy(), "the project keeps working from the session's take, which the archive holds");
    again.app.path = Some(file.clone());
    assert!(again.app.closing());
    let mut third = Harness::new();
    third.app.set_data_dir_for_test(&data);
    assert!(third.app.restore_session());
    assert!(!third.app.is_modified() && third.app.path.as_deref() == Some(file.as_path()));
    assert!(third.app.clips.contains_key(&4242));
    assert!(kept.exists(), "the session's take is the project's working file");
    assert_eq!(crate::sessions::list(&sessions).len(), 1, "one project, one session");
    // Saved again, its audio is still named from the project's folder.
    third.app.save_to(&file).unwrap();
    assert_eq!(third.app.project.clips[0].path, kept.to_string_lossy());

    // A damaged session is not an error: the program starts as if there were none.
    let only = crate::sessions::list(&sessions).remove(0).dir;
    std::fs::write(only.join("session.json"), "{").unwrap();
    assert!(!third.app.restore_session());
    drop((again, third));

    // The one session of an earlier version, in the data folder, comes back too.
    let old = dir.join("ancien");
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&old);
    h.app.project.tempo = 77.0;
    h.app.write_session().unwrap();
    let written = crate::sessions::list(&old.join("Sessions")).remove(0).dir;
    std::fs::create_dir_all(old.join("Session")).unwrap();
    std::fs::rename(written.join("session.json"), old.join("Session").join("session.json")).unwrap();
    std::fs::remove_dir_all(old.join("Sessions")).unwrap();
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&old);
    assert!(h.app.restore_session());
    assert_eq!(h.app.project.tempo, 77.0);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn without_sessions_quitting_offers_to_save_what_would_be_lost() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-quitter-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    // Not coming back at the next start is a setting only: sessions are still kept.
    h.app.set_keep_session(false);
    assert!(h.app.settings.forget_session);
    h.app.project.tracks[0].name = "Autre nom".to_owned();
    assert!(h.app.closing() && h.app.dialog.is_none());
    assert_eq!(crate::sessions::list(&dir.join("Sessions")).len(), 1);
    h.app.set_keep_session(true);
    assert!(!h.app.settings.forget_session);
    // Where no session can be kept, a change is not lost without a word.
    h.app.quitting = false;
    h.app.keep_sessions = false;
    h.app.project.tracks[0].name = "Encore un autre".to_owned();
    assert!(!h.app.closing());
    assert!(matches!(h.app.dialog, Some(Dialog::Quit)));
    h.settle();
    assert!(matches!(h.app.dialog, Some(Dialog::Quit)), "the question stays until it is answered");
    // Once saved, quitting asks nothing more.
    h.app.dialog = None;
    h.app.save_to(&dir.join("x.ogproj")).unwrap();
    assert!(h.app.closing() && h.app.dialog.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn previous_sessions_keep_the_last_projects_and_reopen_them() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-sessions-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    h.app.settings.sessions_kept = 5;
    // Seven projects worked on one after the other: each new one keeps the one before.
    for k in 0..7 {
        let mut project = og_engine::templates::empty();
        project.artist = "Groupe".to_owned();
        project.title = format!("Chanson {k}");
        h.app.replace_project(project, None);
        h.app.project.tempo = 100.0 + f64::from(k);
        // The list goes by the second each was kept.
        std::thread::sleep(std::time::Duration::from_millis(1050));
    }
    h.app.open_sessions();
    assert!(matches!(h.app.dialog, Some(Dialog::Sessions)));
    let songs: Vec<String> = h.app.session_list.iter().map(|e| e.about.song()).collect();
    assert_eq!(songs, ["Groupe – Chanson 6", "Groupe – Chanson 5", "Groupe – Chanson 4", "Groupe – Chanson 3", "Groupe – Chanson 2"], "the last five, the latest first");
    h.settle();
    assert!(matches!(h.app.dialog, Some(Dialog::Sessions)), "the window stays open");
    // Reopening one: the project open now is kept, and the other comes back.
    let fourth = h.app.session_list[2].dir.clone();
    h.app.reopen_session(&fourth);
    assert_eq!((h.app.project.title.as_str(), h.app.project.tempo), ("Chanson 4", 104.0));
    assert!(h.app.dialog.is_none() && h.app.session_message.is_empty());
    // Deleting one.
    h.app.open_sessions();
    let last = h.app.session_list.last().unwrap().dir.clone();
    h.app.delete_session(&last);
    assert!(!last.exists());
    assert_eq!(h.app.session_list.len(), 4);
    // Moving them to another folder.
    let elsewhere = dir.join("ailleurs");
    h.app.move_sessions(&elsewhere);
    assert_eq!(h.app.sessions_dir(), elsewhere);
    assert_eq!(crate::sessions::list(&elsewhere).len(), 4);
    assert!(crate::sessions::list(&dir.join("Sessions")).is_empty());
    h.settle();
    // The number kept stays within its bounds.
    h.app.settings.sessions_kept = 500;
    assert_eq!(h.app.sessions_kept(), 100);
    h.app.settings.sessions_kept = 0;
    assert_eq!(h.app.sessions_kept(), 20);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_song_is_exported_as_midi_and_the_windows_of_import_and_export_stay_until_used() {
    let mut h = Harness::new();
    let file = og_engine::smf::parse(&h.app.midi_bytes()).unwrap();
    assert_eq!(file.tempo, Some(h.app.project.tempo));
    assert!(file.tracks.iter().filter(|t| !t.notes.is_empty()).count() >= 2);
    assert!(file.tracks.iter().any(|t| t.drums && !t.notes.is_empty()), "the drummer plays on the drums channel");
    for dialog in [Dialog::Export, Dialog::Import] {
        h.app.dialog = Some(dialog);
        h.settle();
        assert!(h.app.dialog.is_some(), "the window stays until something is chosen");
    }
    h.app.export_format = crate::ui_transfer::ExportFormat::Mp3;
    h.app.dialog = Some(Dialog::Export);
    h.settle();
    assert!(h.app.dialog.is_some());
}

#[test]
fn the_bottom_panel_keeps_its_height_or_takes_the_window_on_request() {
    let mut h = Harness::new();
    h.app.selected_track = 2;
    for tab in [BottomTab::Controls, BottomTab::Editor] {
        h.app.bottom_tab = tab;
        h.settle();
        // Under the tracks, at its height, whatever it shows.
        assert_eq!(h.app.bottom_height, 290.0, "nothing but the user changes the height");
        let lanes = h.app.lane_rects.clone();
        assert!(lanes[0].top() < 200.0 && lanes[3].bottom() < 860.0 - 290.0, "the tracks are above the panel");
        // Switched off and on again, it comes back the same.
        h.key(Key::B, Modifiers::NONE);
        h.key(Key::B, Modifiers::NONE);
        h.app.bottom_tab = tab;
        h.app.show_bottom = true;
        h.settle();
        assert_eq!(h.app.bottom_height, 290.0);
    }
    // Many rows of sound controls scroll inside the panel; they do not make it grow.
    h.app.bottom_tab = BottomTab::Controls;
    for (panel, target) in [(crate::app::Panel::Plugins, crate::app::Panel::Sound), (crate::app::Panel::Eq, crate::app::Panel::Plugins), (crate::app::Panel::Master, crate::app::Panel::Eq)] {
        h.app.place_panel(panel, target, crate::app::Side::Below);
    }
    h.settle();
    assert!(h.app.visible_rows(2).len() >= 4);
    assert_eq!(h.app.bottom_height, 290.0);
    assert!(h.app.controls_rect.height() < 290.0);

    // Dragging the top edge of the panel changes its height.
    let edge = pos2(700.0, h.app.controls_rect.top() - 33.0);
    let before = h.app.bottom_height;
    h.drag(pos2(700.0, 860.0 - 22.0 - before + 2.0), pos2(700.0, 860.0 - 22.0 - before - 98.0));
    h.settle();
    assert!(h.app.bottom_height > before + 60.0, "{} from {before} (edge near {edge:?})", h.app.bottom_height);

    // The button on the right of its title bar: the panel alone, then back.
    let small = h.app.controls_rect.height();
    h.pause();
    h.click(h.app.bottom_toggle_rect.center());
    h.settle();
    assert!(h.app.bottom_full);
    assert!(h.app.controls_rect.height() > small + 200.0, "the panel has the window to itself");
    assert!(h.app.controls_rect.width() > 1300.0, "the library gives way too");
    h.pause();
    h.click(h.app.bottom_toggle_rect.center());
    h.settle();
    assert!(!h.app.bottom_full);
    assert!((h.app.controls_rect.height() - small).abs() < 1.0, "back under the tracks, at the height it had");
}

#[test]
fn tracks_are_given_outputs_and_inputs_of_their_own() {
    let mut h = Harness::new();
    // Without a choice, everything goes through the program's devices.
    assert_eq!(h.app.input_for(0), "");
    h.app.settings.input_device = "Micro du bureau".to_owned();
    assert_eq!(h.app.input_for(0), "Micro du bureau");
    let audio = audio_track(&mut h, 91, sine_clip(220.0, 0.5), 2.0);
    h.app.set_track_input(audio, "Carte externe");
    assert_eq!((h.app.input_for(audio), h.app.input_for(0)), ("Carte externe".to_owned(), "Micro du bureau".to_owned()));
    h.app.set_track_output(2, "Casque");
    h.app.set_track_output(2, "Casque");
    h.settle();
    assert_eq!(h.app.project.tracks[2].output, "Casque");
    // The choice is part of the project: saved, reopened, undone.
    let dir = std::env::temp_dir().join(format!("opengaragemusic-sorties-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("sorties.ogproj");
    h.app.save_to(&file).unwrap();
    let mut fresh = Harness::new();
    fresh.app.open_path(&file).unwrap();
    assert_eq!((fresh.app.project.tracks[2].output.as_str(), fresh.app.project.tracks[audio].input.as_str()), ("Casque", "Carte externe"));
    h.pause();
    h.key(Key::Z, CTRL);
    assert_eq!(h.app.project.tracks[2].output, "");
    // The windows that offer these choices draw, with or without devices to offer.
    h.app.audio_outputs = vec!["Casque".to_owned(), "Enceintes".to_owned()];
    h.app.audio_inputs = vec!["Carte externe".to_owned()];
    h.app.dialog = Some(Dialog::Audio);
    h.settle();
    assert!(matches!(h.app.dialog, Some(Dialog::Audio)));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn installed_banks_load_in_the_background_at_start_up() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-demarrage-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("Essai.sf2");
    std::fs::write(&file, og_engine::testfont::tiny_soundfont()).unwrap();
    let mut h = Harness::new();
    h.app.settings.banks = vec![
        crate::app::BankEntry { path: file.to_string_lossy().into_owned(), section: crate::app::Section::Auto },
        crate::app::BankEntry { path: dir.join("disparue.sf2").to_string_lossy().into_owned(), section: crate::app::Section::Own },
    ];
    // A path left by an earlier version is taken over.
    h.app.settings.soundfonts = vec![file.to_string_lossy().into_owned()];
    h.app.load_installed_banks();
    assert!(h.app.banks_loading());
    assert!(h.app.fonts.is_empty(), "the window does not wait for the banks");
    for _ in 0..300 {
        h.frame(Vec::new());
        if !h.app.banks_loading() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!h.app.banks_loading());
    assert_eq!(h.app.fonts.len(), 1, "the missing bank is skipped");
    assert!(h.app.patches.iter().any(|p| p.name == "Sinus" && p.category == og_engine::patches::CAT_KEYS));
    assert_eq!(h.app.settings.banks.len(), 2);
    assert!(h.app.settings.soundfonts.is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

mod panels_and_library {
    use super::*;
    use crate::app::{BankEntry, Panel, Section, Side};
    use crate::downloads::{self, Item, Provider};

    fn temp(name: &str) -> std::path::PathBuf {
        super::temp_folder(name)
    }

    fn title(h: &Harness, panel: Panel) -> Pos2 {
        h.app.panel_rects.iter().find(|p| p.0 == panel).map(|p| p.1.center()).unwrap_or_else(|| panic!("{panel:?} is not drawn"))
    }

    #[test]
    fn each_kind_of_track_has_its_own_panels() {
        let mut h = Harness::new();
        h.app.selected_track = 2;
        assert_eq!(h.app.project.tracks[2].kind, TrackKind::Instrument);
        let shown = h.app.visible_panels(2);
        assert_eq!(shown, vec![Panel::Sound, Panel::Arp, Panel::Pedals, Panel::Plugins, Panel::Eq, Panel::Dynamics, Panel::Master, Panel::Save]);
        let audio = audio_track(&mut h, 77, sine_clip(220.0, 0.5), 2.0);
        let shown = h.app.visible_panels(audio);
        assert!(shown.contains(&Panel::Amp) && shown.contains(&Panel::Pedals));
        assert!(!shown.contains(&Panel::Sound) && !shown.contains(&Panel::Arp));
        assert!(h.app.visible_panels(99).is_empty());
    }

    #[test]
    fn panels_are_hidden_shown_and_arranged_in_rows() {
        use Panel::{Amp, Arp, Dynamics, Eq, Master, Pedals, Plugins, Save, Sound};
        let mut h = Harness::new();
        // Two rows to begin with: what makes the sound, what treats it.
        assert_eq!(h.app.visible_rows(2), vec![vec![Sound, Arp, Pedals, Plugins], vec![Eq, Dynamics, Master, Save]]);
        h.app.hide_panel(Arp);
        h.app.hide_panel(Arp);
        h.app.hide_panel(Amp);
        assert_eq!(h.app.settings.panels_hidden, vec![Arp, Amp]);
        assert!(!h.app.visible_panels(2).contains(&Arp));
        // Only what this track could show is offered again.
        assert_eq!(h.app.hidden_panels(2), vec![Arp]);
        h.app.show_panel(Arp);
        assert!(h.app.visible_panels(2).contains(&Arp));
        h.app.show_all_panels();
        assert!(h.app.settings.panels_hidden.is_empty());

        // Beside another panel, in its row.
        h.app.place_panel(Master, Sound, Side::Before);
        assert_eq!(h.app.visible_rows(2), vec![vec![Master, Sound, Arp, Pedals, Plugins], vec![Eq, Dynamics, Save]]);
        h.app.place_panel(Master, Eq, Side::After);
        assert_eq!(h.app.visible_rows(2), vec![vec![Sound, Arp, Pedals, Plugins], vec![Eq, Master, Dynamics, Save]]);
        // In a row of its own, above or below another.
        h.app.place_panel(Plugins, Eq, Side::Above);
        assert_eq!(h.app.visible_rows(2), vec![vec![Sound, Arp, Pedals], vec![Plugins], vec![Eq, Master, Dynamics, Save]]);
        h.app.place_panel(Save, Sound, Side::Below);
        assert_eq!(h.app.visible_rows(2), vec![vec![Sound, Arp, Pedals], vec![Save], vec![Plugins], vec![Eq, Master, Dynamics]]);
        // A row that loses its last panel closes up.
        h.app.place_panel(Save, Plugins, Side::After);
        assert_eq!(h.app.visible_rows(2), vec![vec![Sound, Arp, Pedals], vec![Plugins, Save], vec![Eq, Master, Dynamics]]);
        h.app.place_panel(Sound, Sound, Side::Below);
        assert_eq!(h.app.visible_rows(2).len(), 3, "a panel dropped on itself stays where it is");

        // The menu moves one place at a time, in the row or from row to row.
        h.app.move_panel(Save, -1, 2);
        h.app.move_panel(Save, -1, 2);
        assert_eq!(h.app.visible_rows(2)[1], vec![Save, Plugins]);
        h.app.move_panel_row(Save, 1, 2);
        assert_eq!(h.app.visible_rows(2), vec![vec![Sound, Arp, Pedals], vec![Plugins], vec![Eq, Master, Dynamics, Save]]);
        h.app.move_panel_row(Save, 1, 2);
        h.app.move_panel_row(Save, 1, 2);
        assert_eq!(h.app.visible_rows(2), vec![vec![Sound, Arp, Pedals], vec![Plugins], vec![Eq, Master, Dynamics], vec![Save]]);
        h.app.move_panel_row(Sound, -1, 2);
        assert_eq!(h.app.visible_rows(2)[..2], [vec![Sound], vec![Arp, Pedals]]);
        // Nothing was lost on the way, including the panels this track does not show.
        let mut all = h.app.panel_rows().concat();
        assert_eq!(all.len(), Panel::ALL.len());
        all.dedup();
        assert_eq!(all.len(), Panel::ALL.len());

        // The rows survive being written and read back with the settings.
        let text = serde_json::to_string(&h.app.settings).unwrap();
        let back: crate::app::Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.panel_rows, h.app.settings.panel_rows);
        // The single row of an earlier version becomes the first and only row,
        // and the panels it did not list follow.
        let dir = temp("rangees");
        let file = dir.join("settings.json");
        std::fs::write(&file, r#"{"panel_order":["Eq","Sound"],"library_names":true}"#).unwrap();
        let old = App::with_audio(audio::silent(templates::demo()), Some(file));
        assert_eq!(old.panel_rows().len(), 1);
        assert_eq!(old.panel_rows()[0][..3], [Eq, Sound, Arp]);
        assert_eq!(old.panel_rows()[0].len(), Panel::ALL.len());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn panels_are_dragged_and_hidden_with_the_mouse() {
        use Panel::{Arp, Dynamics, Eq, Master, Pedals, Plugins, Save, Sound};
        let frame = |h: &Harness, panel: Panel| h.app.panel_frames.iter().find(|p| p.0 == panel).map(|p| p.1).unwrap_or_else(|| panic!("{panel:?} is not drawn"));
        let mut h = Harness::new();
        h.app.selected_track = 2;
        h.app.bottom_tab = BottomTab::Controls;
        h.settle();
        assert_eq!(h.app.panel_rects.len(), h.app.visible_panels(2).len());
        assert!(frame(&h, Eq).top() > frame(&h, Sound).bottom(), "the second row is drawn under the first");

        // Dragging the title of the arpeggiator onto the left of the sound panel
        // puts it before that panel.
        let (from, to) = (title(&h, Arp), title(&h, Sound));
        h.drag(from, to);
        h.settle();
        assert_eq!(h.app.visible_rows(2)[0], vec![Arp, Sound, Pedals, Plugins]);
        assert!(title(&h, Arp).x < title(&h, Sound).x, "the panels are drawn in the new order");

        // Right-click on a title, then "Masquer", the first line of the menu.
        h.pause();
        let at = title(&h, Arp);
        h.button(at, PointerButton::Secondary);
        h.pause();
        h.click(at + vec2(30.0, 18.0));
        h.settle();
        assert_eq!(h.app.settings.panels_hidden, vec![Arp]);
        assert!(h.app.panel_rects.iter().all(|p| p.0 != Arp));

        // Right-click on the background, past the end of the first row: the hidden
        // panel is the first button of that menu, below its heading.
        h.pause();
        let end = frame(&h, Plugins);
        let background = pos2(end.right() + 40.0, end.center().y);
        h.button(background, PointerButton::Secondary);
        h.pause();
        h.click(background + vec2(40.0, 40.0));
        h.settle();
        assert!(h.app.settings.panels_hidden.is_empty(), "the panel is back");
        assert!(h.app.panel_rects.iter().any(|p| p.0 == Arp));

        // The controls inside a panel still answer: the frame takes nothing from them.
        let before = h.app.project.tracks[2].arp.on;
        let arp = h.app.panel_rects.iter().find(|p| p.0 == Arp).unwrap().1;
        h.pause();
        h.click(pos2(arp.left() + 8.0, arp.bottom() + 14.0));
        assert_ne!(h.app.project.tracks[2].arp.on, before, "the checkbox under the title");

        // Dropped on the middle of a panel, low: a row of its own under that panel's row.
        h.pause();
        let target = frame(&h, Sound);
        h.drag(title(&h, Eq), pos2(target.center().x, target.bottom() - 6.0));
        h.settle();
        assert_eq!(h.app.visible_rows(2), vec![vec![Arp, Sound, Pedals, Plugins], vec![Eq], vec![Dynamics, Master, Save]]);
        assert!(frame(&h, Eq).top() > frame(&h, Sound).bottom() && frame(&h, Eq).bottom() < frame(&h, Dynamics).top());
        // Dropped on the background, level with a row: at the end of that row.
        h.pause();
        let alone = frame(&h, Eq);
        h.drag(title(&h, Plugins), pos2(alone.right() + 200.0, alone.center().y));
        h.settle();
        assert_eq!(h.app.visible_rows(2), vec![vec![Arp, Sound, Pedals], vec![Eq, Plugins], vec![Dynamics, Master, Save]]);

        // With everything hidden the panel says how to get things back.
        for panel in Panel::ALL {
            h.app.hide_panel(panel);
        }
        h.settle();
        assert!(h.app.panel_rects.is_empty());
    }

    #[test]
    fn what_is_already_downloaded_is_marked() {
        let dir = temp("deja");
        let item = |name: &str| Item {
            provider: Provider::MuseScore,
            name: name.to_owned(),
            author: String::new(),
            license: "MIT".to_owned(),
            url: "https://example.org/b.sf2".to_owned(),
            file_name: "b.sf2".to_owned(),
            size: None,
            sha256: None,
            page: String::new(),
            note: String::new(),
        };
        let (here, absent, broken) = (item("Présente"), item("Absente"), item("Interrompue"));
        let folder = downloads::item_folder(&dir, &here);
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(folder.join("b.sf2"), og_engine::testfont::tiny_soundfont()).unwrap();
        std::fs::write(folder.join("SOURCE.txt"), "Nom : essai").unwrap();
        // A download that stopped half-way left a partial file and no note.
        let partial = downloads::item_folder(&dir, &broken);
        std::fs::create_dir_all(&partial).unwrap();
        std::fs::write(partial.join("b.sf2.part"), b"...").unwrap();
        assert!(downloads::is_downloaded(&dir, &here));
        assert!(!downloads::is_downloaded(&dir, &absent));
        assert!(!downloads::is_downloaded(&dir, &broken));

        let mut h = Harness::new();
        h.app.set_data_dir_for_test(&dir);
        h.app.dialog = Some(Dialog::Downloads);
        h.app.downloads.listing = Some(Ok(vec![here, absent, broken]));
        h.app.mark_downloaded();
        assert_eq!(h.app.downloads.have, vec![true, false, false]);
        for selected in 0..3 {
            h.app.select_download(selected);
            h.settle();
        }
        // Asking the site again forgets the marks until its answer arrives.
        h.app.downloads.provider = Provider::Polyphone;
        h.app.refresh_downloads();
        assert!(h.app.downloads.have.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_list_of_projects_is_read_and_each_project_checked_before_download() {
        let readme = "# OpenAudio\n\n| Plugin | Description | Type | Framework |\n| --- | --- | --- | --- |\n\
| [Aeolus](https://github.com/Archie3d/aeolus_plugin) | Pipe organ emulator | Instrument | JUCE |\n\
| [Floe](https://github.com/floe-audio/Floe) | Sample library platform | CLAP, VST3, AU | Instrument | N/A |\n\
| [Ambience](https://github.com/OTODESK4193/Ambience1.0.1) | Algorithmic reverb | Effect | JUCE |\n\
| [Ailleurs](https://gitlab.com/x/y) | Hosted elsewhere | Effect | JUCE |\n\
| [Louche](https://github.com/x/y/../../z) | Odd address | Effect | JUCE |\n\
| [Aeolus encore](https://github.com/archie3d/aeolus_plugin/) | Listed twice | Instrument | JUCE |\n\
Some text with a [link](https://github.com/a/b) outside the table.\n";
        let items = downloads::parse_openaudio(readme);
        assert_eq!(items.iter().map(|i| i.name.as_str()).collect::<Vec<_>>(), ["Aeolus", "Floe", "Ambience"]);
        assert!(items.iter().all(|i| i.url.is_empty() && i.provider == Provider::OpenAudio && i.provider.kind() == downloads::Kind::Effects));
        assert_eq!(items[0].page, "https://github.com/Archie3d/aeolus_plugin");
        assert!(items[0].note.starts_with("Instrument. Pipe organ") && items[1].note.starts_with("Instrument.") && items[2].note.starts_with("Effet."));

        let release = r#"{"tag_name":"v2","assets":[
            {"name":"aeolus-macos.dmg","size":5,"browser_download_url":"https://github.com/Archie3d/aeolus_plugin/releases/download/v2/aeolus-macos.dmg"},
            {"name":"aeolus-win64-clap.zip","size":700,"browser_download_url":"https://github.com/Archie3d/aeolus_plugin/releases/download/v2/aeolus-win64-clap.zip"}
        ]}"#;
        let found = downloads::resolve_from(&items[0], r#"{"license":{"spdx_id":"GPL-3.0"}}"#, release).unwrap();
        assert_eq!((found.provider, found.license.as_str(), found.size), (Provider::OpenAudio, "GPL-3.0", Some(700)));
        assert!(found.url.ends_with("aeolus-win64-clap.zip") && found.name == "Aeolus");
        // No licence, a licence that is not free, no Windows archive: nothing to download.
        for (repo, release, reason) in [
            (r#"{"license":null}"#, release, "ne déclare pas de licence"),
            (r#"{"license":{"spdx_id":"NOASSERTION"}}"#, release, "ne déclare pas de licence"),
            (r#"{"license":{"spdx_id":"CC-BY-NC-4.0"}}"#, release, "n'est pas dans la liste"),
            (r#"{"license":{"spdx_id":"MIT"}}"#, r#"{"tag_name":"v2","assets":[{"name":"setup.exe","size":5,"browser_download_url":"https://github.com/Archie3d/aeolus_plugin/releases/download/v2/setup.exe"}]}"#, "n'a pas d'archive"),
        ] {
            let error = downloads::resolve_from(&items[0], repo, release).unwrap_err();
            assert!(error.contains(reason), "{error}");
        }

        // In the window: an item without an address cannot be downloaded; the
        // answer of the look-up either completes it or says why not.
        let mut h = Harness::new();
        h.app.dialog = Some(Dialog::Downloads);
        h.app.downloads.provider = Provider::OpenAudio;
        h.app.downloads.listing = Some(Ok(items.clone()));
        h.app.downloads.selected = Some(0);
        h.settle();
        h.app.resolved_download(0, Err("pas de version".to_owned()));
        h.settle();
        assert_eq!(h.app.downloads.blocked, Some((0, "pas de version".to_owned())));
        h.app.resolved_download(0, Ok(found.clone()));
        h.settle();
        assert_eq!(h.app.downloads.listing.as_ref().unwrap().as_ref().unwrap()[0], found);
    }

    #[test]
    fn settings_from_before_the_translation_are_carried_over() {
        let dir = temp("langue");
        let file = dir.join("settings.json");
        std::fs::write(&file, r#"{"loop_favorites":["Basse funk slap","Inconnue"],"user_patches":[]}"#).unwrap();
        let mut app = App::with_audio(audio::silent(templates::demo()), Some(file.clone()));
        // Loops are now known by their English names, whatever the language shown.
        assert_eq!(app.settings.loop_favorites, vec!["Funk slap bass".to_owned(), "Inconnue".to_owned()]);
        assert!(app.settings.library_names && app.settings.language.is_empty());
        // The demonstration project is named in the language in use (French, in the tests).
        assert_eq!(app.project.tracks[3].name, "Batteur");
        app.save_settings();
        let again = App::with_audio(audio::silent(templates::demo()), Some(file));
        assert_eq!(again.settings.loop_favorites[0], "Funk slap bass", "carried over once only");
        // Every language is offered under its own name, English first.
        let names: Vec<String> = crate::i18n::languages().into_iter().map(|l| l.1).collect();
        assert_eq!(names, ["English", "Deutsch", "Español", "Français", "Italiano", "Português"]);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Reads the real list and asks GitHub about one project. Run on demand:
    /// `dev test -p opengaragemusic -- --ignored live_openaudio --nocapture`.
    #[test]
    #[ignore = "needs the network"]
    fn live_openaudio_list_is_read() {
        let items = downloads::list(Provider::OpenAudio, "").unwrap();
        println!("{} projects listed", items.len());
        assert!(items.len() > 200);
        let reverbs = downloads::list(Provider::OpenAudio, "reverb").unwrap();
        assert!(!reverbs.is_empty() && reverbs.len() < items.len());
        let dragonfly = items.iter().find(|i| i.page.to_lowercase().ends_with("michaelwillis/dragonfly-reverb")).expect("a known project is listed");
        match downloads::resolve(dragonfly) {
            Ok(found) => println!("{} : {} ({})", found.name, found.url, found.license),
            Err(reason) => println!("not downloadable now: {reason}"),
        }
    }

    #[test]
    fn banks_that_moved_are_found_again() {
        let dir = temp("deplacees");
        let old = dir.join("ancien").join("Essai.sf2");
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(&old, og_engine::testfont::tiny_soundfont()).unwrap();

        // A project made while the bank was at its first place.
        let mut h = Harness::new();
        let first = h.app.load_font(&old).unwrap();
        let patch = h.app.patches.iter().find(|p| p.category == "Essai" && p.name == "Sinus").cloned().unwrap();
        h.app.selected_track = 2;
        h.app.apply_patch(&patch);
        h.frame(Vec::new());
        let project = dir.join("morceau.ogproj");
        h.app.save_to(&project).unwrap();

        // The bank moves to the folder of downloaded banks.
        let data = dir.join("donnees");
        let new = data.join("Banques").join("MuseScore").join("Essai").join("essai.sf2");
        std::fs::create_dir_all(new.parent().unwrap()).unwrap();
        std::fs::rename(&old, &new).unwrap();

        // At start-up the settings follow it.
        let mut fresh = Harness::new();
        fresh.app.set_data_dir_for_test(&data);
        fresh.app.settings.banks = vec![
            BankEntry { path: old.to_string_lossy().into_owned(), section: Section::Auto },
            BankEntry { path: dir.join("perdue.sf2").to_string_lossy().into_owned(), section: Section::Own },
        ];
        fresh.app.load_installed_banks();
        assert_eq!(std::path::Path::new(&fresh.app.settings.banks[0].path), new);
        assert_eq!(fresh.app.settings.banks.len(), 2, "a bank that is nowhere stays listed");
        for _ in 0..300 {
            fresh.frame(Vec::new());
            if !fresh.app.banks_loading() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(fresh.app.fonts.len(), 1);

        // And the project that names the old place plays with the bank at the new one.
        fresh.app.open_path(&project).unwrap();
        fresh.settle();
        assert!(!fresh.app.status.contains("introuvable"), "{}", fresh.app.status);
        let og_engine::Sound::Sampler(sampler) = &fresh.app.project.tracks[2].sound else { panic!("the track keeps its sampled sound") };
        assert_ne!(sampler.font, first);
        assert!(fresh.app.fonts.contains_key(&sampler.font));
        assert_eq!(fresh.app.project.fonts.len(), 1);
        assert_eq!(std::path::Path::new(&fresh.app.project.fonts[0].path), new);
        for other in [0, 1, 3] {
            fresh.app.project.tracks[other].mute = true;
        }
        let out = og_engine::render_offline(&fresh.app.project, &fresh.app.clips, &fresh.app.fonts, 24_000);
        assert!(out.iter().any(|s| s.abs() > 0.05), "the moved bank is heard");

        // Without the downloads folder, a project is matched to an installed bank.
        let mut other = Harness::new();
        other.app.settings.banks = vec![BankEntry { path: new.to_string_lossy().into_owned(), section: Section::Own }];
        other.app.open_path(&project).unwrap();
        assert_eq!(other.app.fonts.len(), 1);
        drop((h, fresh, other));
        let _ = std::fs::remove_dir_all(dir);
    }
}

// ----- Q, recent projects, time, loops dragged, audio loops, equaliser, help, themes -----

fn features_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("ogm-features-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_q_key_puts_the_notes_of_the_editor_back_on_the_grid() {
    let mut h = Harness::new();
    h.click(h.lane(0, 6.0, 0.5));
    assert_eq!(h.app.selected_region, Some((0, 0)));
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    for note in h.app.project.tracks[0].regions[0].notes_mut().unwrap() {
        note.start += 0.07;
    }
    h.settle();
    let off_grid = |h: &Harness| h.app.project.tracks[0].regions[0].notes().iter().filter(|n| (n.start / 0.25 - (n.start / 0.25).round()).abs() > 1e-6).count();
    assert!(off_grid(&h) > 0);
    // While the computer keyboard plays notes, Q is one of them.
    h.app.typing = true;
    h.key(Key::Q, Modifiers::NONE);
    assert!(off_grid(&h) > 0);
    h.app.release_typing();
    h.app.typing = false;
    h.pause();
    h.key(Key::Q, Modifiers::NONE);
    assert_eq!(off_grid(&h), 0, "Q quantizes");
    // And it is one step to undo.
    h.app.undo();
    assert!(off_grid(&h) > 0);
}

#[test]
fn recent_projects_are_remembered_latest_first_and_the_time_is_shown() {
    let dir = features_dir("recent");
    let mut h = Harness::new();
    assert!(h.app.recent_projects().is_empty());
    let (first, second) = (dir.join("un.ogp"), dir.join("deux.ogp"));
    h.app.save_to(&first).unwrap();
    h.app.save_to(&second).unwrap();
    h.app.open_path(&first).unwrap();
    h.app.open_path(&second).unwrap();
    assert_eq!(h.app.recent_projects(), [second.clone(), first.clone()]);
    h.app.open_path(&first).unwrap();
    assert_eq!(h.app.recent_projects(), [first.clone(), second.clone()], "opened again: back on top, listed once");
    // A file that is gone is no longer offered; one that cannot be read says so.
    std::fs::remove_file(&second).unwrap();
    assert_eq!(h.app.recent_projects(), std::slice::from_ref(&first));
    h.app.open_recent(&second);
    assert!(h.app.status.contains("deux.ogp"), "{}", h.app.status);
    // The start window lists them.
    h.app.dialog = Some(Dialog::NewProject);
    h.settle();

    // Minutes and seconds, next to bars and beats.
    h.app.dialog = None;
    h.app.project.tempo = 120.0;
    h.app.project.tempo_points.clear();
    h.app.position = 0.0;
    assert_eq!(h.app.position_time(), "0:00.0");
    h.app.position = 130.0;
    assert_eq!(h.app.position_time(), "1:05.0");
    h.app.position = 3.0;
    assert_eq!(h.app.position_time(), "0:01.5");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_input_channel_and_the_mp3_quality_are_settings() {
    let mut h = Harness::new();
    assert_eq!(h.app.export_kbps(), 192);
    h.app.settings.export_kbps = 320;
    assert_eq!(h.app.export_kbps(), 320);
    // The channel of the program, unless the track has its own.
    assert_eq!(h.app.channel_for(0), 0, "all the channels, mixed");
    h.app.set_input_channel(2);
    assert_eq!((h.app.settings.input_channel, h.app.channel_for(0), h.app.channel_for(3)), (2, 2, 2));
    h.app.set_track_input_channel(3, 1);
    assert_eq!((h.app.channel_for(3), h.app.channel_for(0)), (1, 2));
    h.app.undo();
    assert_eq!(h.app.channel_for(3), 2);
    h.settle();
}

#[test]
fn a_loop_dragged_onto_the_tracks_lands_where_it_is_dropped() {
    use crate::userloops::LoopPick;
    let mut h = Harness::new();
    h.app.right_panel = RightPanel::Loops;
    h.settle();
    let from = h.app.loop_rects.iter().find(|(pick, _)| matches!(pick, LoopPick::Notes(_))).map(|(_, rect)| rect.center()).expect("loops are listed");
    let Some(&(LoopPick::Notes(index), _)) = h.app.loop_rects.iter().find(|(pick, _)| matches!(pick, LoopPick::Notes(_))) else { unreachable!() };
    let tracks = h.app.project.tracks.len();
    let regions = |h: &Harness| h.app.project.tracks.iter().map(|t| t.regions.len()).sum::<usize>();
    let before = regions(&h);

    // Under the tracks: a track is made for it (or one with its sound takes it).
    let last = *h.app.lane_rects.last().unwrap();
    let under = pos2(h.lane(0, 21.0, 0.5).x, last.bottom() + 40.0);
    assert!(h.app.tracks_rect.contains(under));
    h.drag(from, under);
    assert_eq!(regions(&h), before + 1, "the loop was put down");
    let (ti, region) = h.app.project.tracks.iter().enumerate().find_map(|(ti, t)| t.regions.iter().find(|r| r.start == 20.0).map(|r| (ti, r.clone()))).expect("at the bar under the pointer");
    assert_eq!(region.start % h.app.project.bar_len(), 0.0);
    assert!(!region.notes().is_empty());
    let expected = h.app.loops[index].patch;
    assert_eq!(h.app.project.tracks[ti].patch, expected, "on a track with the loop's sound");
    assert!(h.app.project.tracks.len() >= tracks);

    // Onto a track of notes, in an empty place: that track takes it.
    h.pause();
    let from = h.app.loop_rects.iter().find(|(pick, _)| *pick == LoopPick::Notes(index)).map(|(_, rect)| rect.center()).unwrap();
    let count = h.app.project.tracks[0].regions.len();
    let now = h.app.project.tracks.len();
    h.drag(from, h.lane(0, 17.0, 0.5));
    assert_eq!(h.app.project.tracks.len(), now, "no new track");
    assert_eq!(h.app.project.tracks[0].regions.len(), count + 1);
    assert!(h.app.project.tracks[0].regions.iter().any(|r| r.start == 16.0));
    // One step to undo each.
    h.app.undo();
    assert_eq!(h.app.project.tracks[0].regions.len(), count);

    // Dropped back on the browser: nothing happens.
    h.pause();
    let total = regions(&h);
    h.drag(from, from + vec2(20.0, 60.0));
    assert_eq!(regions(&h), total);
}

#[test]
fn audio_loops_are_the_users_files_stretched_to_the_tempo() {
    use crate::userloops::LoopPick;
    let dir = features_dir("loops");
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir.join("data"));
    h.app.refresh_audio_loops();
    assert!(h.app.audio_loops.is_empty(), "none comes with the program");
    // Two seconds of sound: one bar at 120.
    let rate = 24_000u32;
    let sound: Vec<f32> = (0..rate * 2).map(|i| (i as f32 * 0.05).sin() * 0.5).collect();
    let file = dir.join("Riff 120 bpm.wav");
    og_engine::wav::write_wav(&file, &sound, rate, 1, 16).unwrap();
    std::fs::write(dir.join("notes.txt"), "not a sound").unwrap();
    assert_eq!(h.app.install_audio_loops(&[file, dir.join("notes.txt")]), 2);
    assert_eq!(h.app.audio_loops.len(), 1, "only sound files are listed");
    assert_eq!((h.app.audio_loops[0].name.as_str(), h.app.audio_loops[0].named_tempo), ("Riff 120 bpm", Some(120.0)));
    assert!(h.app.loops_dir().join("Riff 120 bpm.wav").is_file());

    // The browser lists it; put on the tracks, it gets an audio track.
    h.app.right_panel = RightPanel::Loops;
    h.settle();
    assert!(h.app.loop_rects.iter().any(|(pick, _)| *pick == LoopPick::Audio(0)));
    h.app.project.tempo = 100.0;
    h.app.project.tempo_points.clear();
    let tracks = h.app.project.tracks.len();
    h.app.add_loop_at(LoopPick::Audio(0), None, 8.0);
    assert_eq!(h.app.project.tracks.len(), tracks + 1, "{}", h.app.status);
    let track = h.app.project.tracks.last().unwrap();
    assert!(track.kind.is_audio());
    let region = &track.regions[0];
    assert_eq!((region.start, region.length), (8.0, 4.0), "one bar");
    let Content::Audio(audio) = &region.content else { panic!("a region of sound") };
    let clip = &h.app.clips[&audio.clip];
    // One bar at 100 lasts 2.4 seconds: the two seconds were stretched.
    assert!((clip.seconds() - 2.4).abs() < 0.08, "{} s", clip.seconds());
    assert!(clip.samples.iter().any(|s| s.abs() > 0.1));
    // A second one on the same track, further on.
    let ti = h.app.project.tracks.len() - 1;
    h.app.add_loop_at(LoopPick::Audio(0), Some(ti), 16.0);
    assert_eq!((h.app.project.tracks.len(), h.app.project.tracks[ti].regions.len()), (tracks + 1, 2));
    // Dragged from the browser onto that track.
    h.settle();
    let from = h.app.loop_rects.iter().find(|(pick, _)| *pick == LoopPick::Audio(0)).map(|(_, rect)| rect.center()).unwrap();
    h.drag(from, h.lane(ti, 21.0, 0.5));
    assert_eq!(h.app.project.tracks[ti].regions.len(), 3);
    assert!(h.app.project.tracks[ti].regions.iter().any(|r| r.start == 20.0));
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_points_of_the_equaliser_are_dragged_on_its_curve() {
    let mut h = Harness::new();
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Controls;
    h.app.bottom_full = true;
    h.settle();
    assert_eq!(h.app.eq_handles.len(), 3, "low, middle, high");
    let ti = h.app.selected_track;
    let before = h.app.project.tracks[ti].fx.eq;
    assert!(!before.on && before.mid == 0.0);
    // The middle point, up and to the right: more gain, higher frequency.
    let middle = h.app.eq_handles[1].center();
    h.drag(middle, middle + vec2(30.0, -25.0));
    let eq = h.app.project.tracks[ti].fx.eq;
    assert!(eq.on, "moving a point turns the equaliser on");
    assert!(eq.mid > 4.0 && eq.mid < 14.0, "{} dB", eq.mid);
    assert!(eq.mid_freq > before.mid_freq * 1.5, "{} Hz", eq.mid_freq);
    assert_eq!((eq.low, eq.high), (0.0, 0.0));
    // The low point, down: the lows are lowered; it does not go past its range.
    h.pause();
    let low = h.app.eq_handles[0].center();
    h.drag(low, low + vec2(-200.0, 30.0));
    let eq = h.app.project.tracks[ti].fx.eq;
    assert!(eq.low < -5.0 && eq.low_freq == 30.0, "{} dB at {} Hz", eq.low, eq.low_freq);
    // The curve is the one that is heard.
    assert!(og_engine::fx::eq_response_db(&eq, 48_000.0, 30.0) < -2.0);
    assert!(og_engine::fx::eq_response_db(&eq, 48_000.0, eq.mid_freq) > 3.0);
    // The drag is one step to undo.
    h.app.undo();
    assert_eq!(h.app.project.tracks[ti].fx.eq.low, 0.0);
    // A double click puts a point back on the middle line.
    h.pause();
    let middle = h.app.eq_handles[1].center();
    h.click(middle);
    h.click(middle);
    let eq = h.app.project.tracks[ti].fx.eq;
    assert_eq!(eq.mid, 0.0);
    assert!(eq.mid_freq > before.mid_freq * 1.5, "the frequency stays");
}

#[test]
fn a_drummer_can_be_told_to_follow_another_track() {
    let mut h = Harness::new();
    let di = h.app.project.tracks.iter().position(|t| t.kind == TrackKind::Drummer).expect("the demonstration has a drummer");
    let bass = h.app.project.tracks.iter().position(|t| t.kind == TrackKind::Instrument && t.regions.iter().any(|r| !r.notes().is_empty())).unwrap();
    let kicks = |h: &Harness| {
        let region = &h.app.project.tracks[di].regions[0];
        let Content::Drummer(params) = &region.content else { panic!("a drummer region") };
        let mut starts: Vec<f64> = h.app.project.drummer_notes(params, region.start, region.cycle_len()).iter().filter(|n| n.pitch == 36).map(|n| n.start).collect();
        starts.sort_by(f64::total_cmp);
        starts
    };
    let own = kicks(&h);
    let Content::Drummer(params) = &mut h.app.project.tracks[di].regions[0].content else { panic!() };
    params.follow = bass as u16 + 1;
    params.fills = 0.0;
    let followed = kicks(&h);
    assert_ne!(own, followed);
    assert!(!followed.is_empty());
    // Every kick is on a note of the followed track, repeats included.
    let onsets: Vec<f64> = h.app.project.tracks[bass]
        .regions
        .iter()
        .flat_map(|r| {
            let cycle = r.cycle_len();
            let repeats = (r.length / cycle).ceil() as usize;
            (0..repeats).flat_map(|k| r.notes().iter().map(|n| r.start + k as f64 * cycle + n.start).collect::<Vec<_>>()).collect::<Vec<_>>()
        })
        .collect();
    let start = h.app.project.tracks[di].regions[0].start;
    for kick in &followed {
        assert!(onsets.iter().any(|on| (on - (start + kick)).abs() < 0.2), "kick at {kick}");
    }
    // The editor of the drummer shows the choice, and what is heard changes
    // when the followed track does.
    h.app.selected_track = di;
    h.app.selected_region = Some((di, 0));
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.settle();
    for other in 0..h.app.project.tracks.len() {
        h.app.project.tracks[other].mute = other != di;
    }
    h.settle();
    let heard = og_engine::render_offline(&h.app.project, &h.app.clips, &h.app.fonts, 24_000);
    for region in &mut h.app.project.tracks[bass].regions {
        if let Some(notes) = region.notes_mut() {
            notes.retain(|n| n.start < 1.0);
        }
    }
    h.settle();
    let after = og_engine::render_offline(&h.app.project, &h.app.clips, &h.app.fonts, 24_000);
    assert!(heard.iter().any(|s| s.abs() > 0.02));
    assert_ne!(heard, after, "fewer notes to follow: fewer kicks");
    assert!(kicks(&h).len() < followed.len());
}

#[test]
fn quick_help_tells_what_the_pointer_is_over() {
    let mut h = Harness::new();
    assert!(!h.app.help && h.app.help_text.is_empty());
    let usual = h.ctx.style().interaction.tooltip_delay;
    h.frame(vec![Event::PointerMoved(h.lane(1, 8.0, 0.5))]);
    h.frame(Vec::new());
    assert!(h.app.help_text.is_empty(), "nothing is told unless asked");
    h.click(h.app.help_rect.center());
    assert!(h.app.help, "the ? button turns it on");
    let mut told = Vec::new();
    let lane = h.lane(1, 8.0, 0.5);
    let header = pos2(h.app.lane_left - 40.0, lane.y);
    let ruler = pos2(lane.x, h.app.lane_rects[0].top() - 8.0);
    let top = h.app.help_rect.center() - vec2(300.0, 0.0);
    for at in [lane, header, ruler, top] {
        h.frame(vec![Event::PointerMoved(at)]);
        h.frame(Vec::new());
        assert!(!h.app.help_text.is_empty());
        told.push(h.app.help_text.clone());
    }
    assert!(told[0].starts_with("Pistes"), "{}", told[0]);
    assert!(told[1].starts_with("En-tête"), "{}", told[1]);
    assert!(told[2].starts_with("Règle"), "{}", told[2]);
    assert!(told[3].starts_with("Menus"), "{}", told[3]);
    // The panels have their own words.
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.app.right_panel = RightPanel::Loops;
    h.settle();
    h.frame(vec![Event::PointerMoved(pos2(600.0, 860.0 - 90.0))]);
    h.frame(Vec::new());
    assert!(h.app.help_text.starts_with("Éditeur"), "{}", h.app.help_text);
    let in_loops = h.app.loop_rects[0].1.center();
    h.frame(vec![Event::PointerMoved(in_loops)]);
    h.frame(Vec::new());
    assert!(h.app.help_text.starts_with("Navigateur"), "{}", h.app.help_text);
    // The tips of the buttons do not wait.
    assert_eq!(h.ctx.style().interaction.tooltip_delay, 0.0);
    h.pause();
    h.click(h.app.help_rect.center());
    assert!(!h.app.help && h.app.help_text.is_empty());
    assert_eq!(h.ctx.style().interaction.tooltip_delay, usual);
}

#[test]
fn themes_are_chosen_and_the_users_own_are_imported() {
    use crate::theme::Theme;
    let dir = features_dir("themes");
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir.join("data"));
    h.app.refresh_themes();
    assert_eq!(h.app.theme_id(), "studio");
    assert_eq!(h.app.themes.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["studio", "dark", "light"]);
    h.settle();
    assert!(h.ctx.style().visuals.dark_mode);
    let studio = h.ctx.style().visuals.panel_fill;

    h.app.set_theme("light");
    h.settle();
    assert_eq!((h.app.theme_id(), h.app.settings.theme.as_str()), ("light", "light"));
    assert!(!h.ctx.style().visuals.dark_mode);
    assert_eq!(h.ctx.style().visuals.panel_fill, h.app.themes[2].visuals().panel_fill);
    h.app.set_theme("dark");
    h.settle();
    assert!(h.ctx.style().visuals.dark_mode);
    assert_ne!(h.ctx.style().visuals.panel_fill, studio);
    // Every panel draws with each of them.
    h.app.show_bottom = true;
    h.app.right_panel = RightPanel::Loops;
    h.settle();

    // The user's own theme: a file that gives only what it changes.
    let file = dir.join("Sable.json");
    std::fs::write(&file, r##"{ "name": "Sable du soir", "base": "light", "colors": { "panel": "#e8dcc0", "record": "#aa2200" }, "tracks": ["#336699"] }"##).unwrap();
    assert_eq!(h.app.install_theme(&file).unwrap(), "Sable du soir");
    assert_eq!(h.app.theme_id(), "Sable");
    assert_eq!(h.app.themes.len(), 4);
    assert!(h.app.themes_dir().join("Sable.json").is_file());
    h.settle();
    assert_eq!(h.ctx.style().visuals.panel_fill, egui::Color32::from_rgb(0xe8, 0xdc, 0xc0));
    assert_eq!(h.app.theme.record(), egui::Color32::from_rgb(0xaa, 0x22, 0x00));
    assert_eq!(h.app.theme.tracks[0], egui::Color32::from_rgb(0x33, 0x66, 0x99));
    // The model written from it reads back the same.
    let model = Theme::parse("Sable", &h.app.theme.to_json()).unwrap();
    assert_eq!(model.visuals().panel_fill, h.app.theme.visuals().panel_fill);
    assert_eq!(model.to_json(), h.app.theme.to_json());

    // A file that is not a theme is refused, and the theme in use stays.
    let bad = dir.join("Faux.json");
    std::fs::write(&bad, r##"{ "colors": { "panel": "sable" } }"##).unwrap();
    assert!(h.app.install_theme(&bad).unwrap_err().contains("panel"));
    assert_eq!((h.app.theme_id(), h.app.themes.len()), ("Sable", 4));
    // A chosen theme whose file is gone: the program's own.
    std::fs::remove_file(h.app.themes_dir().join("Sable.json")).unwrap();
    h.app.refresh_themes();
    h.settle();
    assert_eq!(h.app.theme.id, "studio");
    assert_eq!(h.ctx.style().visuals.panel_fill, studio);
    h.app.set_theme("studio");
    assert!(h.app.settings.theme.is_empty(), "the program's own theme is the default");
    drop(h);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_score_and_the_tablature_run_over_several_lines() {
    let mut h = Harness::new();
    h.app.select_region(2, 0);
    let guitar = h.app.patches.iter().find(|p| p.name == "Steel guitar").unwrap().clone();
    h.app.apply_patch(&guitar);
    // Eight bars, so that four bars a line make two lines.
    h.app.project.tracks[2].regions[0].length = 32.0;
    h.app.project.tracks[2].regions[0].loop_len = None;
    h.app.note_view = NoteView::Score;
    h.settle();
    // The usual way: one line, from the start of the region.
    let lines = h.app.editor_lines(1000.0, 32.0, 0.0, 32.0, 16.0, 210.0, 400.0);
    assert!(!lines.paged);
    assert_eq!(lines.systems.len(), 1);
    assert_eq!(lines.systems[0].from, 0.0);
    assert_eq!(lines.height, 400.0, "as tall as the view");

    // Two lines of whole bars: the region over them both, the second where
    // the first ends.
    h.app.settings.two_lines = true;
    let lines = h.app.editor_lines(1000.0, 32.0, 0.0, 32.0, 16.0, 210.0, 400.0);
    assert!(lines.paged);
    assert_eq!(lines.systems.len(), 2);
    assert_eq!((lines.systems[0].from, lines.systems[0].to), (0.0, 16.0));
    assert_eq!((lines.systems[1].from, lines.systems[1].to), (16.0, 32.0));
    assert_eq!(lines.systems[1].top, 210.0);
    assert_eq!(lines.ppb, 62.5, "four bars across");
    // Zoomed out to 20 pixels a beat, all eight bars fit one line.
    h.app.editor_zoom = Some(20.0);
    let lines = h.app.editor_lines(1000.0, 32.0, 0.0, 32.0, 16.0, 210.0, 400.0);
    assert_eq!(lines.systems.len(), 1);
    assert_eq!(lines.ppb, 20.0);
    // On a longer region, the pages follow the song.
    h.app.editor_zoom = None;
    h.app.playing = true;
    h.app.position = 40.0;
    let lines = h.app.editor_lines(1000.0, 128.0, 0.0, 128.0, 16.0, 210.0, 400.0);
    assert_eq!((lines.systems[0].from, lines.systems[1].from), (32.0, 48.0), "the pair of lines being played");
    assert_eq!(lines.current, Some(0));
    h.app.playing = false;

    // The score draws over two lines, and a click on the second one adds a
    // note there.
    h.app.bottom_height = 600.0;
    h.settle();
    let rect = h.app.roll_rect;
    assert!(rect.height() >= 420.0, "{}", rect.height());
    let before = h.app.project.tracks[2].regions[0].notes().len();
    h.click(pos2(rect.left() + 56.0 + 300.0, rect.top() + 210.0 + 105.0 - 20.0));
    h.settle();
    let notes = h.app.project.tracks[2].regions[0].notes();
    assert_eq!(notes.len(), before + 1);
    let added = notes.last().unwrap();
    assert!(added.start >= 16.0 && added.start < 32.0, "on the second line: {}", added.start);
    assert_eq!(added.pitch, 67, "four steps over the middle line of the treble staff: G");

    // The tablature too; and the full window lays the whole region out, with
    // as many lines as it takes, from the first.
    h.app.note_view = NoteView::Tab;
    h.settle();
    assert_eq!(h.app.note_view, NoteView::Tab);
    h.app.bottom_full = true;
    h.settle();
    let lines = h.app.editor_lines(1000.0, 64.0, 0.0, 64.0, 16.0, 150.0, 300.0);
    assert_eq!(lines.systems.len(), 4);
    assert_eq!(lines.systems[3].from, 48.0);
    assert_eq!(lines.height, 600.0);
    assert!(h.app.roll_rect.height() > 500.0, "{}", h.app.roll_rect.height());
    // Playing through the sixth line (a tablature shows one bar a line at the
    // most zoom): the view follows it.
    h.app.go_to(20.0);
    h.app.toggle_play();
    h.settle();
    assert_eq!(h.app.editor_line, Some(5));
    h.app.toggle_play();
    h.settle();
    assert_eq!(h.app.editor_line, None);
    h.app.bottom_full = false;
    h.app.settings.two_lines = false;
}

#[test]
fn the_zoom_of_the_editors_is_pixels_a_beat_without_end() {
    let mut h = Harness::new();
    h.app.select_region(2, 0);
    for view in [NoteView::Tab, NoteView::Score, NoteView::Roll] {
        h.app.note_view = view;
        // Until the user zooms: four bars across, whatever the editor.
        h.app.editor_zoom = None;
        let lines = h.app.editor_lines(1000.0, 64.0, 0.0, 64.0, 16.0, 210.0, 400.0);
        assert_eq!(lines.systems[0].to - lines.systems[0].from, 16.0, "{view:?}: four bars");
        // The zoom is the pixels a beat, the same on every track and song: far
        // out, the whole region and more; far in, a couple of beats.
        h.app.editor_zoom = Some(4.0);
        let lines = h.app.editor_lines(1000.0, 64.0, 0.0, 64.0, 16.0, 210.0, 400.0);
        assert_eq!((lines.ppb, lines.systems[0].to - lines.systems[0].from), (4.0, 250.0), "{view:?}");
        h.app.editor_zoom = Some(500.0);
        let lines = h.app.editor_lines(1000.0, 64.0, 0.0, 64.0, 16.0, 210.0, 400.0);
        assert_eq!(lines.systems[0].to - lines.systems[0].from, 2.0, "{view:?}");
    }
    // Bars of three beats: twelve beats until the user zooms.
    h.app.project.beats_per_bar = 3;
    h.app.editor_zoom = None;
    let lines = h.app.editor_lines(1200.0, 64.0, 0.0, 64.0, 12.0, 210.0, 400.0);
    assert_eq!(lines.systems[0].to - lines.systems[0].from, 12.0);
    h.app.project.beats_per_bar = 4;
    // A tablature starts at four beats.
    let lines = h.app.editor_lines(1000.0, 64.0, 0.0, 64.0, crate::ui_bottom::TAB_CLOSEST, 210.0, 400.0);
    assert_eq!(lines.systems[0].to - lines.systems[0].from, 4.0, "four beats");
    // The zoom set is kept in the settings, within its range; none is 0.
    h.app.set_editor_zoom(Some(80.0));
    assert_eq!((h.app.editor_zoom, h.app.settings.editor_zoom), (Some(80.0), 80.0));
    h.app.set_editor_zoom(Some(1e9));
    assert_eq!(h.app.editor_zoom, Some(*crate::ui_bottom::ZOOM_RANGE.end()));
    h.app.set_editor_zoom(None);
    assert_eq!((h.app.editor_zoom, h.app.settings.editor_zoom), (None, 0.0));
    // The "-" and "+" at the bottom right of the editor, and the value between
    // them, which a click puts back to four bars.
    h.app.note_view = NoteView::Tab;
    h.app.set_editor_zoom(Some(40.0));
    h.settle();
    let rect = h.app.roll_rect;
    let y = rect.bottom() - 14.0;
    let strip_left = rect.right() - 150.0;
    let at = |x: f32| pos2(strip_left + x, y);
    // "Zoom" label, then "-", then the value, then "+": found by trying along the strip.
    let mut found = Vec::new();
    for x in (0..140).step_by(4) {
        let before = h.app.editor_zoom;
        h.click(at(x as f32));
        h.settle();
        if h.app.editor_zoom != before {
            found.push((x, h.app.editor_zoom));
            h.app.set_editor_zoom(Some(40.0));
            h.settle();
        }
    }
    let zooms: Vec<Option<f64>> = found.iter().map(|(_, z)| *z).collect();
    assert!(zooms.contains(&Some(32.0)), "a press of - zooms out by a step: {found:?}");
    assert!(zooms.contains(&None), "a click on the value puts the zoom back: {found:?}");
    assert!(zooms.contains(&Some(50.0)), "a press of + zooms in by a step: {found:?}");
}

#[test]
fn the_score_writes_the_key_the_values_and_the_rests() {
    use crate::ui_bottom::{rests_of, Accidental, Key, Value};
    // G major has one sharp: F is written plain, F natural with a natural sign.
    let g = Key::of(7, false);
    assert_eq!(g, Key(1));
    assert_eq!(g.spell(66), (38, Accidental::None), "F sharp, the key says so");
    assert_eq!(g.spell(65), (38, Accidental::Natural));
    assert_eq!(g.spell(61), (35, Accidental::Sharp), "C sharp");
    assert_eq!(g.pitch_of(38), 66, "the F step is F sharp");
    // F major has one flat and writes the notes between the letters as flats.
    let f = Key::of(5, false);
    assert_eq!(f, Key(-1));
    assert_eq!(f.spell(70), (41, Accidental::None), "B flat");
    assert_eq!(f.spell(71), (41, Accidental::Natural));
    assert_eq!(f.spell(63), (37, Accidental::Flat), "E flat");
    // A minor shares the signature of C major; E flat major has three flats.
    assert_eq!(Key::of(9, true), Key(0));
    assert_eq!(Key::of(3, false), Key(-3));
    assert_eq!(Key::of(6, false), Key(6), "F sharp major, six sharps");

    // Values: plain, dotted, triplets, and the longest that fits otherwise.
    assert_eq!(Value::of(1.0), Value { base: 1.0, dotted: false, triplet: false });
    assert_eq!(Value::of(0.75), Value { base: 0.5, dotted: true, triplet: false });
    assert_eq!(Value::of(1.0 / 3.0), Value { base: 0.5, dotted: false, triplet: true });
    assert_eq!(Value::of(0.7), Value { base: 0.5, dotted: false, triplet: false });
    assert_eq!((Value::of(4.0).flags(), Value::of(0.5).flags(), Value::of(0.25).flags(), Value::of(0.125).flags()), (0, 1, 2, 3));

    // Rests fill what nothing sounds in, from the longest value that starts
    // on its own multiple.
    assert_eq!(rests_of(&[], 0.0, 4.0), vec![(0.0, 4.0)], "an empty bar");
    assert_eq!(rests_of(&[(0.0, 1.0)], 0.0, 4.0), vec![(1.0, 1.0), (2.0, 2.0)]);
    assert_eq!(rests_of(&[(0.0, 0.5), (3.0, 4.0)], 0.0, 4.0), vec![(0.5, 0.5), (1.0, 1.0), (2.0, 1.0)]);
    assert_eq!(rests_of(&[(4.0, 5.0), (5.0, 8.0)], 4.0, 8.0), vec![], "a full bar");
    assert_eq!(rests_of(&[(0.0, 0.1)], 0.0, 1.0), vec![(0.25, 0.25), (0.5, 0.5)], "a very short note, then the grid of sixteenths");

    // Notes are cut at the bar lines and written as values tied together.
    use crate::ui_bottom::score_pieces;
    let note = |start: f64, length: f64| Note { start, length, pitch: 60, velocity: 100, string: None, ..Default::default() };
    let cut = |notes: &[Note]| {
        let (pieces, owner, tied) = score_pieces(notes, 4.0, 0.0);
        pieces.iter().zip(owner).zip(tied).map(|((p, o), t)| (p.start, p.length, o, t)).collect::<Vec<_>>()
    };
    assert_eq!(cut(&[note(0.0, 2.0)]), vec![(0.0, 2.0, 0, None)], "a half note stays whole");
    assert_eq!(cut(&[note(3.0, 2.0)]), vec![(3.0, 1.0, 0, None), (4.0, 1.0, 0, Some(0))], "across the bar line: two quarters tied");
    assert_eq!(cut(&[note(0.0, 2.5)]), vec![(0.0, 2.0, 0, None), (2.0, 0.5, 0, Some(0))], "no single value: a half and an eighth");
    assert_eq!(cut(&[note(0.0, 1.5), note(1.5, 1.0 / 3.0)]), vec![(0.0, 1.5, 0, None), (1.5, 1.0 / 3.0, 1, None)], "dotted and triplet values are written as they are");
    assert_eq!(cut(&[note(2.0, 9.0)]).len(), 3, "half, whole, then a quarter: {:?}", cut(&[note(2.0, 9.0)]));
    assert_eq!(cut(&[note(0.0, 1.03)]), vec![(0.0, 1.03, 0, None)], "a crumb stays with its note");
    // Bars that start off the region's start (a region that starts mid-bar).
    let (pieces, _, _) = score_pieces(&[note(1.0, 2.0)], 4.0, 2.0);
    assert_eq!(pieces.iter().map(|p| (p.start, p.length)).collect::<Vec<_>>(), vec![(1.0, 1.0), (2.0, 1.0)]);
}

#[test]
fn the_sites_window_opens_the_way_in_for_guitar_pro_files() {
    let mut h = Harness::new();
    assert_eq!(h.app.known_sites.len(), 7);
    assert!(h.app.known_sites[..3].iter().all(|k| !k.site.open_only) && h.app.known_sites[3].site.open_only, "Ultimate Guitar is only opened");
    h.app.open_sites();
    h.settle();
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)));
    // A file that is not a tablature is refused with a message, and the
    // project stays.
    let dir = std::env::temp_dir().join(format!("og-gp-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bad = dir.join("song.gp5");
    std::fs::write(&bad, b"not a tablature at all").unwrap();
    let tracks = h.app.project.tracks.len();
    h.app.drop_file(&bad, None, None);
    h.settle();
    assert!(h.app.import_message.contains("song.gp5"), "{}", h.app.import_message);
    assert_eq!(h.app.project.tracks.len(), tracks);
    assert!(matches!(h.app.dialog, Some(Dialog::Sites)), "the window stays to say so");
    assert!(og_engine::guitar_pro::is_guitar_pro(&bad) && !og_engine::guitar_pro::is_guitar_pro(std::path::Path::new("a.mid")));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn several_tracks_share_the_editor_and_auto_switch_follows_the_one_that_plays() {
    let mut h = Harness::new();
    h.click(h.lane(0, 6.0, 0.5));
    assert_eq!(h.app.selected_region, Some((0, 0)));
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.settle();
    assert_eq!(h.app.editor_shown(), (vec![(0, 0)], false));

    // Ctrl and a click on a header adds the track to the editor; the tracks
    // then show one under the other, in the order they were chosen.
    h.pause();
    let header = pos2(h.app.lane_left - 45.0, h.app.lane_rects[2].top() + 13.0);
    h.frame_with(vec![Event::PointerMoved(header)], CTRL);
    for pressed in [true, false] {
        h.frame_with(vec![Event::PointerButton { pos: header, button: PointerButton::Primary, pressed, modifiers: CTRL }], CTRL);
    }
    h.settle();
    assert_eq!(h.app.editor_tracks, vec![0, 2]);
    assert_eq!(h.app.selected_track, 0, "the selected track stays");
    h.app.toggle_editor_track(1);
    assert_eq!(h.app.editor_tracks, vec![0, 2, 1]);
    assert_eq!(h.app.editor_shown(), (vec![(0, 0), (2, 0), (1, 0)], true));
    h.settle();
    // Taking the selected one away gives the lead to the first of the others.
    h.app.toggle_editor_track(0);
    assert_eq!(h.app.editor_tracks, vec![2, 1]);
    assert_eq!(h.app.selected_track, 2);
    h.app.toggle_editor_track(0);
    assert_eq!(h.app.editor_tracks, vec![2, 1, 0]);
    // A plain click on a header is a single choice again.
    h.pause();
    h.click(header);
    assert_eq!(h.app.editor_track_list(), vec![2]);
    assert!(!h.app.editor_shown().1);

    // Auto-switch: the first chosen track whenever it plays; else the one
    // followed, until nothing more comes on it within the chosen bars: then,
    // as its last note is struck, the next track that plays.
    let only = |h: &mut Harness, ti: usize, from: f64, to: f64| {
        let notes: Vec<Note> = (0..((to - from) * 2.0) as usize).map(|k| Note { start: from + k as f64 * 0.5, length: 0.5, pitch: 60, velocity: 100, string: None, ..Default::default() }).collect();
        h.app.project.tracks[ti].regions[0].content = Content::Midi(notes);
    };
    only(&mut h, 0, 0.0, 4.0);
    only(&mut h, 1, 0.0, 16.0);
    only(&mut h, 2, 8.0, 16.0);
    assert!(h.app.track_plays_at(0, 2.0) && !h.app.track_plays_at(0, 5.0) && h.app.track_plays_at(2, 9.0));
    assert!(h.app.track_plays_within(0, 3.0, 8.0), "a note at 3.5 is to come");
    assert!(!h.app.track_plays_within(0, 3.5, 8.0), "the note at 3.5 is the last");
    assert!(h.app.track_plays_within(2, 2.0, 8.0) && !h.app.track_plays_within(2, 2.0, 5.0));
    h.app.editor_tracks = vec![0, 2, 1];
    h.app.select_region(0, 0);
    h.app.auto_switch = true;
    h.app.playing = true;
    h.app.position = 2.0;
    assert_eq!(h.app.editor_shown().0, vec![(0, 0)], "the first plays");
    h.app.position = 3.0;
    assert_eq!(h.app.editor_shown().0, vec![(0, 0)], "a note of it is still to come");
    h.app.position = 3.5;
    assert_eq!(h.app.editor_shown().0, vec![(1, 0)], "its last note: the next that sounds now, in the order chosen");
    assert_eq!(h.app.followed, Some(1));
    h.app.position = 9.0;
    assert_eq!(h.app.editor_shown().0, vec![(1, 0)], "kept while it plays");
    h.app.settings.auto_switch_bars = 1;
    h.app.position = 15.5;
    assert_eq!(h.app.editor_shown().0, vec![(2, 0)], "nothing more within a bar: the next that sounds");
    only(&mut h, 0, 14.0, 16.0);
    h.app.position = 15.0;
    assert_eq!(h.app.editor_shown().0, vec![(0, 0)], "the first chosen has priority");
    h.app.playing = false;
    assert_eq!(h.app.editor_shown().0, vec![(0, 0)], "stays once stopped");
    h.app.auto_switch = false;
    assert_eq!(h.app.editor_shown().0.len(), 3);
    h.app.settings.auto_switch_bars = 0;
    assert_eq!(h.app.auto_switch_bars(), 2);
    h.settle();
}

#[test]
fn deleting_or_moving_a_track_keeps_the_tracks_of_the_editor() {
    let mut h = Harness::new();
    h.app.select_track(1);
    h.app.toggle_editor_track(3);
    h.app.toggle_editor_track(0);
    assert_eq!(h.app.editor_track_list(), vec![1, 3, 0]);
    let names = |h: &mut Harness| h.app.editor_track_list().iter().map(|t| h.app.project.tracks[*t].name.clone()).collect::<Vec<_>>();
    let before = names(&mut h);
    // Moving a track changes nothing of what the editor shows.
    h.app.move_track(3, -1);
    assert_eq!(names(&mut h), before);
    h.app.move_track(1, 1);
    assert_eq!(names(&mut h), before);
    assert_eq!(h.app.project.tracks[h.app.selected_track].name, before[0], "the selected track moved with it");
    // Deleting one that is not selected: it leaves, none takes its place.
    let selected = h.app.project.tracks[h.app.selected_track].name.clone();
    let gone = h.app.editor_track_list()[2];
    h.app.delete_track(gone);
    assert_eq!(names(&mut h), vec![before[0].clone(), before[1].clone()]);
    assert_eq!(h.app.project.tracks[h.app.selected_track].name, selected);
    // Deleting the selected one: the first of the others is selected.
    let first = h.app.editor_track_list()[0];
    h.app.delete_track(first);
    assert_eq!(names(&mut h), vec![before[1].clone()]);
    assert_eq!(h.app.project.tracks[h.app.selected_track].name, before[1]);
    h.settle();
}

#[test]
fn the_tracks_can_slide_under_a_playhead_kept_in_the_middle() {
    let mut h = Harness::new();
    h.app.project.cycle_enabled = false;
    // A song long enough to play past the middle of the view.
    h.app.project.tracks[0].regions[0].length = 200.0;
    h.settle();
    let visible = f64::from(h.app.lane_w / h.app.ppb);
    assert!(visible > 4.0);
    // Turned off: the playhead moves and the view stays where it is.
    assert!(!h.app.settings.tracks_playhead_moves, "kept in the middle unless asked otherwise");
    h.app.settings.tracks_playhead_moves = true;
    h.key(Key::Space, Modifiers::NONE);
    h.run_audio(1.0);
    h.frame(Vec::new());
    assert!(h.app.position > 0.5 && h.app.position < visible);
    assert_eq!(h.app.scroll, 0.0);
    // The usual way: nothing moves until the song reaches the middle...
    h.app.settings.tracks_playhead_moves = false;
    h.frame(Vec::new());
    assert!(h.app.position < visible / 2.0);
    assert_eq!(h.app.scroll, 0.0);
    // ...then the tracks slide, and the playhead is drawn in the middle of the lanes.
    while h.app.position < visible {
        h.run_audio(1.0);
        h.frame(Vec::new());
        assert!(h.app.playing, "the song is long enough");
    }
    h.frame(Vec::new());
    assert!((h.app.scroll - (h.app.position - visible / 2.0)).abs() < 1e-6, "{} at {}", h.app.scroll, h.app.position);
    let lane = h.app.lane_rects[0];
    let x = h.lane(0, h.app.position, 0.5).x;
    assert!((x - (lane.left() + h.app.lane_w / 2.0)).abs() < 1.5, "{x} in {lane:?}");
    let before = h.app.scroll;
    h.run_audio(0.5);
    h.frame(Vec::new());
    assert!(h.app.scroll > before, "it goes on sliding");
    // Stopped, the view stays where it is and can be scrolled by hand again.
    h.key(Key::Space, Modifiers::NONE);
    let stopped = h.app.scroll;
    h.settle();
    assert_eq!(h.app.scroll, stopped);
    // Turned off again while playing: the view pages as before.
    h.app.settings.tracks_playhead_moves = true;
    h.key(Key::Space, Modifiers::NONE);
    h.run_audio(0.3);
    h.frame(Vec::new());
    assert_eq!(h.app.scroll, stopped, "the playhead is still in view: nothing moves");
}

#[test]
fn selecting_a_track_by_its_header_brings_its_region_to_the_editor() {
    let mut h = Harness::new();
    // A region of the first track is being edited; the third is a guitar.
    h.click(h.lane(0, 6.0, 0.5));
    assert_eq!(h.app.selected_region, Some((0, 0)));
    h.app.project.tracks[2].icon = 3;
    assert!(h.app.is_guitar_track(2) && !h.app.is_guitar_track(0));
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.app.selected_notes = vec![0];
    h.settle();
    // A click on the header of the guitar track, beside its name and buttons.
    h.pause();
    let lane = h.app.lane_rects[2];
    // On the word that says what kind of track it is: the click still selects the track.
    h.click(pos2(h.app.lane_left - 45.0, lane.top() + 13.0));
    assert_eq!(h.app.selected_track, 2);
    assert_eq!(h.app.selected_region, Some((2, 0)), "the editor shows the region of the track that was chosen");
    assert!(h.app.selected_notes.is_empty());
    // So the tablature is offered, and stays chosen.
    h.app.note_view = crate::app::NoteView::Tab;
    h.settle();
    assert_eq!(h.app.note_view, crate::app::NoteView::Tab);

    // A second region further on: the one under the playhead is taken.
    let mut later = h.app.project.tracks[2].regions[0].clone();
    later.start = 32.0;
    h.app.project.tracks[2].regions.push(later);
    h.app.select_track(0);
    assert_eq!(h.app.selected_region, Some((0, 0)));
    h.app.position = 34.0;
    h.app.select_track(2);
    assert_eq!(h.app.selected_region, Some((2, 1)));
    // A region of that track already selected is kept.
    h.app.position = 2.0;
    h.app.select_track(2);
    assert_eq!(h.app.selected_region, Some((2, 1)));
    // A track without regions: nothing of another track stays in the editor.
    h.app.project.tracks[1].regions.clear();
    h.app.select_track(1);
    assert_eq!((h.app.selected_track, h.app.selected_region), (1, None));
    h.settle();
}

// ----- tablature sites -----------------------------------------------------------------

mod sites_import {
    use super::*;
    use crate::downloads::site_error;
    use crate::sites::Opened;
    use og_engine::patches;
    use og_engine::songsterr::Part;


    fn temp(name: &str) -> std::path::PathBuf {
        super::temp_folder(&format!("sites-{name}"))
    }

    #[test]
    fn imported_tracks_take_the_instruments_of_an_installed_bank() {
        let dir = temp("banque");
        let file = dir.join("Essai.sf2");
        std::fs::write(&file, og_engine::testfont::tiny_soundfont()).unwrap();
        let mut h = Harness::new();
        h.app.install_bank(&file, &crate::app::Section::Auto).unwrap();
        // The test bank has a piano (program 0) and a drum kit.
        assert_eq!(h.app.gm_patch(0).map(|p| p.name.as_str()), Some("Sinus"));
        assert_eq!(h.app.gm_patch(og_engine::songsterr::DRUMS).map(|p| p.name.as_str()), Some("Kit"));
        assert!(h.app.gm_patch(30).is_none(), "no distortion guitar in it");
        let part = |instrument_id: u32| Part {
            name: format!("Part {instrument_id}"),
            instrument_id,
            notes: vec![Note { start: 0.0, length: 1.0, pitch: 60, velocity: 96, string: None, ..Default::default() }],
            length: 4.0,
            tempo: Vec::new(),
            signature: (4, 4),
            key: None,
            markers: Vec::new(),
            tuning: Vec::new(),
            chords: Vec::new(),
            swing: Vec::new(),
            bar_starts: Vec::new(),
            lyrics: Vec::new(),
        };
        let score = og_engine::guitar_pro::Score { title: "One".to_owned(), artist: "Metallica".to_owned(), parts: vec![part(0), part(30), part(og_engine::songsterr::DRUMS)] };
        let (send, receive) = std::sync::mpsc::channel();
        h.app.sites.opening = Some(receive);
        h.app.sites.create_when_read = true;
        send.send(Ok(Opened { score, messages: Vec::new(), videos: Vec::new(), lyrics: Default::default(), files: Vec::new() })).unwrap();
        h.settle();
        let patches: Vec<&str> = h.app.project.tracks.iter().map(|t| t.patch.as_str()).collect();
        assert_eq!(patches, ["Sinus", "Classic lead", "Kit"]);
        assert!(matches!(h.app.project.tracks[2].sound, og_engine::Sound::Sampler(s) if s.bank == 128));
        assert_eq!(h.app.project.fonts.len(), 1, "the project remembers the bank file");
        assert_eq!(h.app.project.tracks.iter().map(|t| t.icon).collect::<Vec<_>>(), [0, 3, 1]);
        assert!(h.app.status.contains("2 ") && h.app.status.contains("3 "), "{}", h.app.status);
        assert!(!h.app.is_modified());
        // The sound button of a track shows the library on that sound, in its section.
        h.app.show_library = false;
        h.app.lib_search = "x".to_owned();
        h.app.lib_category = patches::CAT_SYNTH.to_owned();
        h.app.reveal_patch(2);
        assert!(h.app.show_library && h.app.lib_search.is_empty());
        assert_eq!((h.app.selected_track, h.app.lib_category.as_str()), (2, patches::CAT_DRUMS));
        assert_eq!(h.app.lib_scroll_to.as_deref(), Some("Kit"));
        h.settle();
        assert!(h.app.lib_scroll_to.is_none(), "scrolled once");
        h.app.reveal_patch(1);
        assert_eq!(h.app.lib_category, patches::CAT_SYNTH);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_account_kept_by_an_earlier_version_is_dropped_from_the_settings() {
        let dir = temp("compte");
        let file = dir.join("settings.json");
        let old = serde_json::json!({ "songsterr_email": "someone@example.org", "songsterr_password": "dpapi:0102", "export_kbps": 256, "library_names": true }).to_string();
        std::fs::write(&file, old).unwrap();
        let mut app = App::with_audio(audio::silent(templates::demo()), Some(file.clone()));
        assert_eq!(app.export_kbps(), 256, "the rest of the settings is read");
        app.save_settings();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(!text.contains("songsterr") && !text.contains("someone@example.org") && !text.contains("dpapi"), "{text}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn an_installed_bank_keeps_its_filing_when_a_project_loads_it_first() {
        let dir = temp("rangement");
        let file = dir.join("Essai.sf2");
        std::fs::write(&file, og_engine::testfont::tiny_soundfont()).unwrap();
        let category = |app: &App, name: &str| app.patches.iter().find(|p| p.name == name).map(|p| p.category.clone()).unwrap_or_default();
        // The settings say "by family"; a project that uses the bank loads it as such.
        let settings = dir.join("settings.json");
        let text = serde_json::json!({ "banks": [{ "path": file.to_string_lossy().to_uppercase(), "section": "Auto" }], "library_names": true }).to_string();
        std::fs::write(&settings, text).unwrap();
        let mut app = App::with_audio(audio::silent(templates::demo()), Some(settings));
        assert_eq!(app.installed_section(&file), crate::app::Section::Auto, "found whatever the case of the path");
        assert_eq!(app.installed_section(&dir.join("Autre.sf2")), crate::app::Section::Own);
        app.load_font(&file).unwrap();
        assert_eq!(category(&app, "Kit"), patches::CAT_DRUMS, "{:?}", app.patches.iter().map(|p| (&p.name, &p.category)).collect::<Vec<_>>());
        // Loaded in its own section before the installed banks arrive: they win.
        let mut h = Harness::new();
        h.app.load_font(&file).unwrap();
        assert_eq!(category(&h.app, "Kit"), "Essai");
        h.app.settings.banks.push(crate::app::BankEntry { path: file.to_string_lossy().into_owned(), section: crate::app::Section::Auto });
        h.app.load_installed_banks();
        for _ in 0..500 {
            h.frame(Vec::new());
            if !h.app.banks_loading() {
                break;
            }
        }
        assert!(!h.app.banks_loading());
        assert_eq!(category(&h.app, "Kit"), patches::CAT_DRUMS);
        assert_eq!(category(&h.app, "Sinus"), patches::CAT_KEYS);
        assert_eq!(h.app.patches.iter().filter(|p| p.name == "Kit").count(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn what_the_site_refuses_is_said_in_a_sentence() {
        let missing = site_error(404, r#"{"code":"ERR_ROUTE_NOT_FOUND","error":"Not found"}"#);
        assert!(missing.contains("404") && missing.contains("Not found"), "{missing}");
        assert!(site_error(500, "<html>").contains("500"));
        assert!(site_error(401, r#"{"message":"Unauthorized"}"#).contains("Unauthorized"));
    }
}

#[test]
fn beats_chosen_with_shift_in_the_editor_are_played_in_a_loop() {
    let mut h = Harness::new();
    h.app.select_region(0, 0);
    let region = h.app.project.tracks[0].regions[0].clone();
    let cycle = region.cycle_len().min(region.length);
    assert!(cycle >= 4.0, "the demo region holds a few beats");
    assert_eq!(h.app.beats_selected(0, 0), None);
    // Shift and a click: the first beat alone, then from it to another.
    assert_eq!(h.app.beats_to_choose(0, 0, 2.6), (2.0, 3.0));
    h.app.choose_beat(0, 0, 2.6);
    assert_eq!(h.app.beats_selected(0, 0), Some((2.0, 3.0)));
    assert_eq!(h.app.beats_to_choose(0, 0, 0.2), (0.0, 3.0), "the second click goes back from the first");
    h.app.choose_beat(0, 0, 0.2);
    assert_eq!(h.app.beats_selected(0, 0), Some((0.0, 3.0)));
    assert_eq!(h.app.beats_to_choose(0, 0, 1.0), (1.0, 2.0), "a third click starts again");
    assert_eq!(h.app.beats_selected(1, 0), None, "only in the editor of that region");
    h.app.choose_beat(0, 0, -1.0);
    assert_eq!(h.app.beats_selected(0, 0), Some((0.0, 3.0)), "a beat out of the region changes nothing");

    // The button loops the song over them, from their first, and plays.
    h.app.toggle_loop_beats(0, 0);
    assert!(h.app.looping_beats(0, 0) && h.app.playing);
    assert_eq!((h.app.project.cycle_start, h.app.project.cycle_end), (region.start, region.start + 3.0));
    assert_eq!(h.app.position, region.start);
    h.settle();
    h.run_audio(4.0);
    h.settle();
    assert!(h.app.playing, "the loop keeps playing");
    assert!(h.app.position >= region.start && h.app.position < region.start + 3.0, "the playhead stays within the beats: {}", h.app.position);
    // Again: the loop stops, the song plays on.
    h.app.toggle_loop_beats(0, 0);
    assert!(!h.app.project.cycle_enabled && !h.app.looping_beats(0, 0));
    // Escape lets the beats go.
    h.app.choose_beat(0, 0, 1.0);
    h.frame_with(vec![Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], Modifiers::NONE);
    assert_eq!(h.app.beat_selection, None);
    // Shown in the score and the tablature, in the panel and the full window.
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.app.choose_beat(0, 0, 1.0);
    h.app.choose_beat(0, 0, 2.0);
    for (view, full) in [(NoteView::Score, false), (NoteView::Tab, false), (NoteView::Score, true)] {
        h.app.note_view = view;
        h.app.bottom_full = full;
        h.frame_with(vec![Event::PointerMoved(h.app.roll_rect.center())], Modifiers::SHIFT);
        h.settle();
    }
    assert_eq!(h.app.beats_selected(0, 0), Some((1.0, 3.0)));
    h.pause();
}

#[test]
fn the_icon_of_a_track_says_what_it_plays_and_a_click_on_it_chooses_its_colour() {
    use crate::instruments::Instrument;
    let mut h = Harness::new();
    let kinds: Vec<TrackKind> = h.app.project.tracks.iter().map(|t| t.kind).collect();
    for (ti, kind) in kinds.iter().enumerate() {
        let instrument = h.app.track_instrument(ti);
        match kind {
            TrackKind::Drummer => assert_eq!(instrument, Instrument::Drums),
            TrackKind::Guitar => assert_eq!(instrument, Instrument::Guitar),
            TrackKind::Audio => assert_eq!(instrument, Instrument::Voice),
            TrackKind::Video => assert_eq!(instrument, Instrument::Video),
            TrackKind::Instrument => {}
        }
    }
    // An instrument track is what its sound is: a sound of the library, or a
    // General MIDI program.
    let ti = kinds.iter().position(|k| *k == TrackKind::Instrument).unwrap();
    for (patch, instrument) in [("Felt piano", Instrument::Piano), ("Warm pad", Instrument::Synth), ("Brass", Instrument::Brass), ("Flute", Instrument::Winds), ("String ensemble", Instrument::Strings), ("Marimba", Instrument::Mallets)] {
        let sound = h.app.patches.iter().find(|p| p.name == patch).unwrap().sound;
        h.app.project.tracks[ti].patch = patch.to_owned();
        h.app.project.tracks[ti].sound = sound;
        assert_eq!(h.app.track_instrument(ti), instrument, "{patch}");
    }
    assert_eq!((Instrument::of_program(0), Instrument::of_program(25), Instrument::of_program(33), Instrument::of_program(40), Instrument::of_program(56), Instrument::of_program(65)), (Instrument::Piano, Instrument::Guitar, Instrument::Bass, Instrument::Strings, Instrument::Brass, Instrument::Winds));

    // A click on the icon opens the colours, a click on one gives it to the track.
    let (row, left) = (h.app.lane_rects[ti].top() + 16.0, h.app.lane_left - 300.0);
    let icon = (0..80).map(|k| pos2(left + k as f32 * 4.0, row)).find(|p| {
        h.pause();
        h.click(*p);
        h.ctx.memory(|m| m.any_popup_open())
    });
    let icon = icon.expect("the icon opens the colours");
    let before = h.app.project.tracks[ti].color;
    let name = h.app.project.tracks[ti].name.clone();
    let mut chosen = false;
    for k in 0..40 {
        let p = icon + vec2(-10.0 + k as f32 * 6.0, 22.0);
        h.pause();
        if !h.ctx.memory(|m| m.any_popup_open()) {
            h.click(icon);
        }
        h.click(p);
        if h.app.project.tracks[ti].color != before {
            chosen = true;
            break;
        }
    }
    assert!(chosen, "a colour was chosen");
    assert_eq!(h.app.project.tracks[ti].name, name);
    assert_eq!(h.app.track_instrument(ti), Instrument::Mallets, "the icon stays that of the instrument");
}

#[test]
fn a_project_opened_is_kept_even_untouched_but_not_a_blank_one() {
    let dir = std::env::temp_dir().join(format!("opengaragemusic-untouched-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut h = Harness::new();
    h.app.set_data_dir_for_test(&dir);
    let sessions = dir.join("Sessions");
    // As a song read from a tablature site: replaced, never changed.
    let mut song = og_engine::templates::empty();
    (song.artist, song.title) = ("Groupe".to_owned(), "Première".to_owned());
    h.app.replace_project(song, None);
    h.settle();
    assert_eq!(crate::sessions::list(&sessions).len(), 1, "kept at once, without a change");
    // A new blank project, untouched, is not.
    h.app.replace_project(og_engine::templates::empty(), None);
    h.app.blank = true;
    h.settle();
    h.app.replace_project(og_engine::templates::empty(), None);
    h.settle();
    let songs: Vec<String> = crate::sessions::list(&sessions).iter().map(|e| e.about.song()).collect();
    assert_eq!(songs.len(), 2, "{songs:?}");
    assert!(songs.contains(&"Groupe – Première".to_owned()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn chords_written_with_flats_and_sharps_are_named_and_drawn() {
    use crate::ui_bottom::{chord_grid_shape, chord_label};
    // The font has no glyph for the signs: they are written as letters.
    for name in ["B♭7sus4", "B♭7", "F♯m"] {
        let label = chord_label(name);
        assert!(!label.contains('♭') && !label.contains('♯'), "{label}");
    }
    // A grid for each, from the usual ways of playing them, without notes.
    let guitar = [64, 59, 55, 50, 45, 40];
    for name in ["B♭7sus4", "B♭7", "G7", "C7", "F♯m", "Am7(b5)", "CM7"] {
        assert!(chord_grid_shape(name, 0.0, 4.0, &[], &[], &guitar).is_some(), "{name}");
    }
}

#[test]
fn the_full_window_shows_the_line_being_played_at_once() {
    let mut h = Harness::new();
    h.app.select_region(2, 0);
    let guitar = h.app.patches.iter().find(|p| p.name == "Steel guitar").unwrap().clone();
    h.app.apply_patch(&guitar);
    h.app.project.tracks[2].regions[0].length = 64.0;
    h.app.project.tracks[2].regions[0].loop_len = None;
    for view in [NoteView::Tab, NoteView::Score] {
        h.app.note_view = view;
        h.app.bottom_full = false;
        h.settle();
        // Playing far into the region, then the editor takes the whole window.
        h.app.go_to(50.0);
        h.app.toggle_play();
        h.frame(Vec::new());
        // With its button, as the user does: the editor is drawn once more
        // in the panel before it takes the window.
        h.click(h.app.bottom_toggle_rect.center());
        assert!(h.app.bottom_full);
        // The view slides there within half a second.
        for _ in 0..25 {
            h.frame(Vec::new());
        }
        let lines = h.app.editor_line.expect("a line is followed");
        assert!(lines > 0, "{view:?}");
        // The playhead's line is in the window: the tablature has scrolled to it.
        let window = h.ctx.screen_rect();
        let top = h.app.roll_rect.top() + h.app.editor_line_top;
        assert!(top >= window.top() - 1.0 && top < window.bottom(), "{view:?}: line at {top} in {window:?}");
        h.app.toggle_play();
        h.app.bottom_full = false;
        h.settle();
    }
}

#[test]
fn a_recording_of_the_song_is_found_in_step_and_the_song_plays_either_way() {
    use og_engine::TrackKind;
    let dir = std::env::temp_dir().join(format!("opengaragemusic-recording-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let mut h = Harness::new();
    // The "recording": the demonstration played faster, after a second of silence.
    let mut faster = h.app.project.clone();
    faster.tempo = h.app.project.tempo * 1.25;
    let rate = 22_050;
    let mut sound = vec![0.0f32; rate as usize * 2];
    sound.extend(og_engine::render_offline(&faster, &og_engine::Clips::new(), &og_engine::Fonts::new(), rate));
    let file = dir.join("Démo enregistrée.wav");
    og_engine::wav::write_wav(&file, &sound, rate, 2, 16).unwrap();

    assert!(!h.app.has_recording());
    h.app.link_recording(file.clone());
    while h.app.aligning.is_some() {
        std::thread::sleep(std::time::Duration::from_millis(20));
        h.app.poll_recording();
    }
    h.settle();
    let index = h.app.project.tracks.len() - 1;
    let track = &h.app.project.tracks[index];
    assert_eq!((track.kind, track.lead, track.name.as_str()), (TrackKind::Audio, true, "Démo enregistrée"));
    assert!(h.app.project.follow, "the song plays with it at once: {}", h.app.status);
    // Bar two of the demonstration, where the recording has it.
    let bar = h.app.project.bar_len();
    let expected = 1.0 + bar * 60.0 / faster.tempo;
    let found = og_engine::recording::sec_at(&track.timing, bar).unwrap();
    assert!((found - expected).abs() < 0.2, "{found} for {expected}");
    // Played with it: at its pace, it alone sounds, unless a track is turned on.
    let played = og_engine::recording::as_played(&h.app.project);
    assert!((played.tempo - faster.tempo).abs() < 8.0, "{}", played.tempo);
    assert!(played.tracks[..index].iter().all(|t| t.mute) && !played.tracks[index].mute);
    h.app.project.tracks[0].with_recording = true;
    assert!(!og_engine::recording::as_played(&h.app.project).tracks[0].mute);
    // As written: the recording is silent, the tempo is the score's.
    h.app.set_follow(false);
    let played = og_engine::recording::as_played(&h.app.project);
    assert!(played.tracks[index].mute && !played.tracks[0].mute);
    assert_eq!(played.tempo, h.app.project.tempo);
    // Its editor offers to shift it.
    h.app.select_region(index, 0);
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.settle();
    let before = h.app.project.tracks[index].timing[0].sec;
    h.app.shift_recording(index, 0.1);
    assert!((h.app.project.tracks[index].timing[0].sec - before - 0.1).abs() < 1e-9);
    // Kept with the project.
    let again = og_engine::Project::from_json(&h.app.project.to_json().unwrap()).unwrap();
    let kept = &again.tracks[index].timing;
    assert_eq!(kept.len(), h.app.project.tracks[index].timing.len());
    assert!(kept.iter().zip(&h.app.project.tracks[index].timing).all(|(a, b)| a.beat == b.beat && (a.sec - b.sec).abs() < 1e-9));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_video_track_shows_in_the_editor_and_its_header_says_what_it_is() {
    let mut h = Harness::new();
    let offer = crate::sites::VideoOffer { id: "abcdefghijk".to_owned(), label: String::new(), points: vec![0.5, 2.9, 5.3, 7.7] };
    h.app.add_video_track(&[offer], &[0.0, 4.0, 8.0, 12.0]);
    let index = h.app.project.tracks.len() - 1;
    h.app.select_region(index, 0);
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.settle();
    // The tests have no window for the player: it is not started, nothing breaks.
    assert!(h.app.video.is_none());
    assert_eq!(h.app.track_instrument(index), crate::instruments::Instrument::Video);
    // Original sound: the M of the other tracks says whether they play along.
    h.app.set_follow(true);
    assert!(h.app.project.follow);
    h.settle();
    h.app.set_follow(false);
    h.settle();
}

#[test]
fn the_line_between_the_tracks_of_the_editor_is_dragged() {
    let mut h = Harness::new();
    h.app.select_region(0, 0);
    h.app.editor_tracks = vec![0, 1];
    h.app.show_bottom = true;
    h.app.bottom_full = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.settle();
    assert_eq!(h.app.editor_shares, [0.5, 0.5], "half the height each at first");
    let line = h.app.editor_splits[0];
    h.drag(line.center(), line.center() + vec2(0.0, 120.0));
    h.settle();
    let shares = h.app.editor_shares.clone();
    assert!(shares[0] > 0.6 && (shares[0] + shares[1] - 1.0).abs() < 1e-4, "{shares:?}");
    // Not past the room each keeps.
    let line = h.app.editor_splits[0];
    h.drag(line.center(), line.center() + vec2(0.0, 2000.0));
    h.settle();
    assert!(h.app.editor_shares[1] > 0.05, "{:?}", h.app.editor_shares);
    h.app.bottom_full = false;
}

#[test]
fn a_click_on_nothing_in_the_tablature_moves_the_playhead() {
    let mut h = Harness::new();
    h.app.select_region(0, 0);
    h.app.project.tracks[0].icon = 3;
    let guitar = h.app.patches.iter().find(|p| p.name == "Steel guitar").unwrap().clone();
    h.app.apply_patch(&guitar);
    h.app.note_view = NoteView::Tab;
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.app.editor_zoom = Some(40.0);
    h.settle();
    assert_eq!(h.app.position, 0.0);
    h.app.playhead_moved = false;
    // Over the strings, where no fret is written, in the second bar.
    let rect = h.app.roll_rect;
    h.click(pos2(rect.left() + rect.width() * 0.25, rect.top() + 30.0));
    h.settle();
    assert!(h.app.position > 1.0, "{}", h.app.position);
    assert!(h.app.playhead_moved || h.app.video.is_none(), "the video would follow");
}

#[test]
fn the_words_show_under_the_editor_as_the_song_plays() {
    use og_engine::lyrics::{mark_lines, Lyrics, Syllable};
    let mut h = Harness::new();
    let mut sung: Vec<Syllable> = ["Eve-", "ry", "time", "I", "look", "All", "these", "lines"].iter().enumerate().map(|(k, t)| Syllable { beat: k as f64 * 2.0, text: (*t).to_owned(), line_start: false }).collect();
    mark_lines(&mut sung, 4.0);
    h.app.project.lyrics = Lyrics { sung, source: "tablature".to_owned(), ..Lyrics::default() };
    h.app.select_region(0, 0);
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    assert!(!h.app.settings.hide_lyrics, "shown at first");
    h.settle();
    let (lines, now) = h.app.words_lines();
    assert_eq!(lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["Every time I look", "All these lines"]);
    assert_eq!(now, Some(0.0));
    // The whole text: a click on the second line takes the song there.
    h.app.settings.lyrics_whole = true;
    h.app.go_to(12.0);
    h.settle();
    assert_eq!(h.app.words_line, Some(1));
    // Timed words follow the seconds of the song.
    h.app.project.lyrics = crate::words::typed("[00:00.00]Un\n[00:03.00]Deux");
    h.app.go_to(6.0);
    let (lines, now) = h.app.words_lines();
    assert_eq!(og_engine::lyrics::current(&lines, now.unwrap()), Some(1), "beat 6 at 100 bpm is 3.6 s");
    // Hidden with its button, the editor takes the room again.
    h.app.settings.hide_lyrics = true;
    h.settle();
}

#[test]
fn the_band_of_the_words_is_made_taller_or_lower() {
    let mut h = Harness::new();
    h.app.project.lyrics = crate::words::typed("Une ligne\nUne autre");
    h.app.select_region(0, 0);
    h.app.show_bottom = true;
    h.app.bottom_tab = BottomTab::Editor;
    h.settle();
    let usual = h.app.words_band_height();
    // Under the tracks: dragged up, the band grows, and the panel with it.
    let line = h.app.words_split;
    h.drag(line.center(), line.center() - vec2(0.0, 60.0));
    h.settle();
    assert!((h.app.words_band_height() - usual - 60.0).abs() < 2.0, "{} from {usual}", h.app.words_band_height());
    // In the full window too, the editor keeping some room.
    h.app.bottom_full = true;
    h.settle();
    let before = h.app.words_band_height();
    let line = h.app.words_split;
    h.drag(line.center(), line.center() + vec2(0.0, 40.0));
    h.settle();
    assert!((before - h.app.words_band_height() - 40.0).abs() < 2.0);
    // Never lower than a line or two.
    let line = h.app.words_split;
    h.drag(line.center(), line.center() + vec2(0.0, 2000.0));
    h.settle();
    assert_eq!(h.app.words_band_height(), crate::ui_bottom::WORDS_LEAST);
    h.app.bottom_full = false;
}
