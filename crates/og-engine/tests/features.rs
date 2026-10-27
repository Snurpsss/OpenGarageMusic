//! The equaliser's curve, the drummer that follows a track, AIFF files.

use og_engine::fx::eq_response_db;
use og_engine::{templates, Content, DrummerParams, Eq, Note, Project, Region, TrackKind};

const KICK: u8 = 36;

#[test]
fn the_curve_of_the_equaliser_follows_its_three_points() {
    let rate = 48_000.0;
    let flat = Eq::default();
    for hz in [30.0, 200.0, 1000.0, 6000.0, 15_000.0] {
        assert!(eq_response_db(&flat, rate, hz).abs() < 0.01, "flat at {hz} Hz");
    }
    // Lows raised: the bottom goes up, the middle stays.
    let lows = Eq { on: true, low: 12.0, ..Eq::default() };
    assert!(eq_response_db(&lows, rate, 30.0) > 10.0);
    assert!(eq_response_db(&lows, rate, 2000.0).abs() < 1.0);
    // The point moved to the right takes more of the spectrum with it.
    let wider = Eq { low_freq: 500.0, ..lows };
    assert!(eq_response_db(&wider, rate, 300.0) > eq_response_db(&lows, rate, 300.0) + 3.0);
    // The middle point is a bump where it is put.
    let mids = Eq { on: true, mid: -9.0, mid_freq: 800.0, ..Eq::default() };
    assert!((eq_response_db(&mids, rate, 800.0) + 9.0).abs() < 0.5);
    assert!(eq_response_db(&mids, rate, 60.0).abs() < 1.0 && eq_response_db(&mids, rate, 12_000.0).abs() < 1.0);
    // Highs lowered, from the frequency chosen.
    let highs = Eq { on: true, high: -12.0, high_freq: 3000.0, ..Eq::default() };
    assert!(eq_response_db(&highs, rate, 15_000.0) < -10.0);
    assert!(eq_response_db(&highs, rate, 200.0).abs() < 1.0);
    // A project written before the points could move sideways reads as before.
    let old: Eq = serde_json::from_str(r#"{ "on": true, "low": 3.0, "mid": 0.0, "mid_freq": 1000.0, "high": 0.0 }"#).unwrap();
    assert_eq!((old.low_freq, old.high_freq), (120.0, 5000.0));
}

fn kicks(notes: &[Note]) -> Vec<f64> {
    let mut starts: Vec<f64> = notes.iter().filter(|n| n.pitch == KICK).map(|n| n.start).collect();
    starts.sort_by(f64::total_cmp);
    starts
}

#[test]
fn a_drummer_that_follows_a_track_puts_its_kick_on_that_tracks_notes() {
    let mut project = Project::empty();
    let mut bass = templates::track("Bass", TrackKind::Instrument, "Electric piano", 0);
    // A two-beat pattern, repeated over two bars, that starts on the second bar.
    let mut region = Region::new("Line", 4.0, 8.0);
    region.content = Content::Midi(vec![
        Note { start: 0.0, length: 0.5, pitch: 40, velocity: 100, string: None, ..Default::default() },
        Note { start: 0.74, length: 0.25, pitch: 40, velocity: 100, string: None, ..Default::default() },
        Note { start: 1.5, length: 0.5, pitch: 43, velocity: 100, string: None, ..Default::default() },
    ]);
    region.loop_len = Some(2.0);
    bass.regions.push(region);
    project.tracks.push(bass);

    let own = DrummerParams { fills: 0.0, swing: 0.0, ..DrummerParams::default() };
    let following = DrummerParams { follow: 1, ..own };
    let plain = project.drummer_notes(&own, 0.0, 16.0);
    assert_eq!(plain, og_engine::drummer::generate(&own, 16.0, 4.0), "without following: the style's own pattern");
    let followed = project.drummer_notes(&following, 0.0, 16.0);
    // Nothing in the first bar and after the region; in between, the pattern
    // of the bass, each note moved to the nearest sixteenth.
    let expected: Vec<f64> = (0..4).flat_map(|k| [0.0, 0.75, 1.5].map(|at| 4.0 + 2.0 * f64::from(k) + at)).collect();
    let found = kicks(&followed);
    assert_eq!(found.len(), expected.len(), "{found:?}");
    for (kick, wanted) in found.iter().zip(&expected) {
        assert!((kick - wanted).abs() < 0.09, "kick at {kick}, bass at {wanted}");
    }
    assert_ne!(kicks(&plain), found);
    // The rest of the kit still plays.
    assert!(followed.iter().any(|n| n.pitch != KICK));
    // A drummer region that starts later counts from its own start.
    let later = kicks(&project.drummer_notes(&following, 8.0, 4.0));
    assert_eq!(later.len(), 6);
    assert!(later[0].abs() < 0.09 && (later[1] - 0.75).abs() < 0.09);
    // A track that is not there, or that holds no notes: the style's own pattern.
    assert_eq!(project.drummer_notes(&DrummerParams { follow: 7, ..own }, 0.0, 16.0), plain);
    project.tracks.push(templates::track("Voice", TrackKind::Audio, "No effect", 1));
    assert_eq!(project.drummer_notes(&DrummerParams { follow: 2, ..own }, 0.0, 16.0), plain);
}

#[test]
fn an_aiff_file_holds_the_sound_most_significant_byte_first() {
    let samples = [0.0f32, 0.5, -0.5, 1.0, 0.25, -1.0];
    let file = og_engine::wav::encode_aiff(&samples, 44_100, 2, 16).unwrap();
    assert_eq!((&file[0..4], &file[8..12], &file[12..16]), (&b"FORM"[..], &b"AIFF"[..], &b"COMM"[..]));
    assert_eq!(u32::from_be_bytes([file[4], file[5], file[6], file[7]]) as usize, file.len() - 8);
    assert_eq!(file.len(), 54 + samples.len() * 2);
    // Two channels, three frames, sixteen bits.
    assert_eq!(&file[20..28], &[0, 2, 0, 0, 0, 3, 0, 16]);
    // 44 100 as an 80-bit number.
    assert_eq!(&file[28..38], &[0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]);
    assert_eq!(&file[38..42], b"SSND");
    let sound: Vec<i16> = file[54..].as_chunks::<2>().0.iter().map(|b| i16::from_be_bytes([b[0], b[1]])).collect();
    assert_eq!(sound, [0, 16_384, -16_384, 32_767, 8192, -32_767]);
    // 24 bits: three bytes a sample.
    let deep = og_engine::wav::encode_aiff(&samples, 48_000, 1, 24).unwrap();
    assert_eq!(deep.len(), 54 + samples.len() * 3);
    assert_eq!(&deep[26..28], &[0, 24]);
    assert_eq!(&deep[57..60], &[0x40, 0x00, 0x00], "half of full scale");
}
