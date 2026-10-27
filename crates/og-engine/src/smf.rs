//! Reader and writer of Standard MIDI Files (formats 0 and 1), written from the public
//! specification. Only what a sequencer needs is kept: notes, tempo, names.

use crate::model::Note;

#[derive(Debug, PartialEq)]
pub struct MidiTrack {
    pub name: String,
    /// True when the notes are on the percussion channel (channel 10).
    pub drums: bool,
    pub notes: Vec<Note>,
}

#[derive(Debug, PartialEq)]
pub struct MidiFile {
    /// First tempo found, in beats per minute.
    pub tempo: Option<f64>,
    pub tracks: Vec<MidiTrack>,
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.pos.checked_add(n).filter(|end| *end <= self.data.len()).ok_or("truncated MIDI file")?;
        let slice = &self.data[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Variable-length quantity: seven bits per byte, high bit set on all but the last.
    fn vlq(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self.u8()?;
            value = (value << 7) | u32::from(byte & 0x7F);
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err("invalid number in the MIDI file".to_owned())
    }
}

pub fn parse(data: &[u8]) -> Result<MidiFile, String> {
    let mut r = Reader { data, pos: 0 };
    if r.take(4)? != b"MThd" {
        return Err("this is not a MIDI file".to_owned());
    }
    let header_len = r.u32()? as usize;
    let header_end = r.pos.checked_add(header_len).ok_or("invalid MIDI header")?;
    let _format = r.u16()?;
    let track_count = r.u16()?;
    let division = r.u16()?;
    if division & 0x8000 != 0 || division == 0 {
        return Err("MIDI files in SMPTE time are not supported".to_owned());
    }
    let ticks_per_beat = f64::from(division);
    r.pos = header_end.min(data.len());

    let mut file = MidiFile { tempo: None, tracks: Vec::new() };
    for _ in 0..track_count {
        if r.pos + 8 > data.len() {
            break;
        }
        let id = r.take(4)?;
        let len = r.u32()? as usize;
        let body = r.take(len.min(data.len() - r.pos))?;
        if id != b"MTrk" {
            continue;
        }
        let mut t = Reader { data: body, pos: 0 };
        let mut track = MidiTrack { name: String::new(), drums: false, notes: Vec::new() };
        // Notes waiting for their note-off: (channel, pitch, start tick, velocity).
        let mut open: Vec<(u8, u8, u64, u8)> = Vec::new();
        let mut tick = 0u64;
        let mut status = 0u8;
        while t.pos < body.len() {
            tick += u64::from(t.vlq()?);
            let mut byte = t.u8()?;
            if byte < 0x80 {
                // Running status: the status byte is omitted.
                if status < 0x80 {
                    return Err("invalid MIDI file".to_owned());
                }
                t.pos -= 1;
                byte = status;
            } else if byte < 0xF0 {
                status = byte;
            }
            match byte {
                0xFF => {
                    let kind = t.u8()?;
                    let len = t.vlq()? as usize;
                    let payload = t.take(len)?;
                    match kind {
                        0x03 if track.name.is_empty() => track.name = String::from_utf8_lossy(payload).trim().to_owned(),
                        0x51 if payload.len() == 3 && file.tempo.is_none() => {
                            let micros = u32::from_be_bytes([0, payload[0], payload[1], payload[2]]);
                            if micros > 0 {
                                file.tempo = Some(60_000_000.0 / f64::from(micros));
                            }
                        }
                        0x2F => break,
                        _ => {}
                    }
                }
                0xF0 | 0xF7 => {
                    let len = t.vlq()? as usize;
                    t.take(len)?;
                }
                _ => {
                    let channel = byte & 0x0F;
                    let kind = byte & 0xF0;
                    let a = t.u8()? & 0x7F;
                    let b = if kind == 0xC0 || kind == 0xD0 { 0 } else { t.u8()? & 0x7F };
                    let note_on = kind == 0x90 && b > 0;
                    let note_off = kind == 0x80 || (kind == 0x90 && b == 0);
                    if note_on || note_off {
                        if let Some(i) = open.iter().position(|o| o.0 == channel && o.1 == a) {
                            let (_, pitch, start, velocity) = open.remove(i);
                            track.notes.push(Note {
                                start: start as f64 / ticks_per_beat,
                                length: ((tick - start) as f64 / ticks_per_beat).max(0.03),
                                pitch,
                                velocity,
                                string: None, ..Default::default()
                            });
                        }
                    }
                    if note_on {
                        open.push((channel, a, tick, b));
                        track.drums |= channel == 9;
                    }
                }
            }
        }
        for (_, pitch, start, velocity) in open {
            track.notes.push(Note {
                start: start as f64 / ticks_per_beat,
                length: ((tick - start) as f64 / ticks_per_beat).max(0.25),
                pitch,
                velocity,
                string: None, ..Default::default()
            });
        }
        if !track.notes.is_empty() {
            track.notes.sort_by(|a, b| a.start.total_cmp(&b.start));
            file.tracks.push(track);
        }
    }
    Ok(file)
}

/// A track of a file to write: its name, whether it plays drums, its General
/// MIDI program, and its notes as (start, end, pitch, velocity), in beats.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WriteTrack {
    pub name: String,
    pub drums: bool,
    pub program: u8,
    pub notes: Vec<(f64, f64, u8, u8)>,
}

/// Ticks of a quarter note in the files written.
const TICKS: f64 = 480.0;

fn vlq(out: &mut Vec<u8>, mut value: u32) {
    let mut bytes = vec![(value & 0x7F) as u8];
    value >>= 7;
    while value > 0 {
        bytes.push((value & 0x7F) as u8 | 0x80);
        value >>= 7;
    }
    out.extend(bytes.iter().rev());
}

/// A track chunk of events (tick, bytes), sorted by tick, with its end.
fn chunk(mut events: Vec<(u64, Vec<u8>)>) -> Vec<u8> {
    events.sort_by_key(|(tick, _)| *tick);
    let mut body = Vec::new();
    let mut last = 0u64;
    for (tick, bytes) in events {
        vlq(&mut body, (tick - last).min(u64::from(u32::MAX >> 4)) as u32);
        body.extend(bytes);
        last = tick;
    }
    body.extend([0x00, 0xFF, 0x2F, 0x00]);
    let mut out = b"MTrk".to_vec();
    out.extend((body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

fn ticks(beat: f64) -> u64 {
    (beat.max(0.0) * TICKS).round() as u64
}

/// A Standard MIDI File (format 1): a first track with the tempo changes, as
/// (beat, quarter notes per minute), and the time signature, then one track
/// per track given. Drum tracks play on channel 10, the others each on a
/// channel of their own (shared again past fifteen tracks).
pub fn write(tempo: &[(f64, f64)], signature: (u32, u32), tracks: &[WriteTrack]) -> Vec<u8> {
    let mut out = b"MThd".to_vec();
    out.extend(6u32.to_be_bytes());
    out.extend(1u16.to_be_bytes());
    out.extend(((tracks.len() + 1).min(u16::MAX as usize) as u16).to_be_bytes());
    out.extend((TICKS as u16).to_be_bytes());

    let mut meta: Vec<(u64, Vec<u8>)> = Vec::new();
    let unit = signature.1.clamp(1, 32).next_power_of_two().trailing_zeros() as u8;
    meta.push((0, vec![0xFF, 0x58, 0x04, signature.0.clamp(1, 64) as u8, unit, 24, 8]));
    for &(beat, bpm) in tempo.iter().filter(|(_, bpm)| bpm.is_finite() && *bpm > 0.0) {
        let micros = (60_000_000.0 / bpm).round().clamp(1.0, f64::from(0x00FF_FFFF)) as u32;
        let [_, a, b, c] = micros.to_be_bytes();
        meta.push((ticks(beat), vec![0xFF, 0x51, 0x03, a, b, c]));
    }
    out.extend(chunk(meta));

    let mut next_channel = 0u8;
    for track in tracks {
        let channel = if track.drums {
            9
        } else {
            let channel = next_channel;
            next_channel = (next_channel + 1) % 15;
            if channel >= 9 { channel + 1 } else { channel }
        };
        let mut events: Vec<(u64, Vec<u8>)> = Vec::new();
        let name = track.name.as_bytes();
        let mut text = vec![0xFF, 0x03];
        vlq(&mut text, name.len().min(127) as u32);
        text.extend(&name[..name.len().min(127)]);
        events.push((0, text));
        if !track.drums {
            events.push((0, vec![0xC0 | channel, track.program.min(127)]));
        }
        for &(start, end, pitch, velocity) in &track.notes {
            let (on, off) = (ticks(start), ticks(end).max(ticks(start) + 1));
            // At the same tick, a note-off sorts before a note-on.
            events.push((off, vec![0x80 | channel, pitch.min(127), 0]));
            events.push((on, vec![0x90 | channel, pitch.min(127), velocity.clamp(1, 127)]));
        }
        events.sort_by_key(|(tick, bytes)| (*tick, bytes[0] & 0xF0 == 0x90));
        out.extend(chunk(events));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_file() -> Vec<u8> {
        let mut f = Vec::new();
        f.extend_from_slice(b"MThd");
        f.extend_from_slice(&6u32.to_be_bytes());
        f.extend_from_slice(&[0, 1, 0, 2, 0, 96]); // format 1, two tracks, 96 ticks per beat
        // Tempo track: 100 bpm = 600 000 microseconds per beat.
        let tempo = [0x00, 0xFF, 0x51, 0x03, 0x09, 0x27, 0xC0, 0x00, 0xFF, 0x2F, 0x00];
        f.extend_from_slice(b"MTrk");
        f.extend_from_slice(&(tempo.len() as u32).to_be_bytes());
        f.extend_from_slice(&tempo);
        let notes = [
            0x00, 0xFF, 0x03, 0x04, b'L', b'e', b'a', b'd', // name
            0x00, 0x90, 60, 100, // C on
            0x60, 0x80, 60, 0, // off after one beat
            0x00, 0x90, 64, 90, // E on, then running status with velocity 0 as note-off
            0x81, 0x40, 64, 0, // 192 ticks later
            0x00, 0xFF, 0x2F, 0x00,
        ];
        f.extend_from_slice(b"MTrk");
        f.extend_from_slice(&(notes.len() as u32).to_be_bytes());
        f.extend_from_slice(&notes);
        f
    }

    #[test]
    fn reads_notes_tempo_and_names() {
        let file = parse(&sample_file()).unwrap();
        assert_eq!(file.tempo, Some(100.0));
        assert_eq!(file.tracks.len(), 1);
        let track = &file.tracks[0];
        assert_eq!(track.name, "Lead");
        assert!(!track.drums);
        assert_eq!(
            track.notes,
            vec![
                Note { start: 0.0, length: 1.0, pitch: 60, velocity: 100, string: None, ..Default::default() },
                Note { start: 1.0, length: 2.0, pitch: 64, velocity: 90, string: None, ..Default::default() },
            ]
        );
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert!(parse(b"hello").is_err());
        assert!(parse(b"").is_err());
        let good = sample_file();
        for cut in 0..good.len() {
            let _ = parse(&good[..cut]);
        }
        let mut noise = good.clone();
        for (i, byte) in noise.iter_mut().enumerate().skip(14) {
            *byte = byte.wrapping_mul(31).wrapping_add(i as u8);
        }
        let _ = parse(&noise);
    }

    #[test]
    fn a_file_written_is_read_back() {
        let guitar = WriteTrack { name: "Guitare".into(), drums: false, program: 25, notes: vec![(0.0, 1.0, 64, 100), (1.0, 1.5, 67, 90), (1.0, 2.0, 60, 80)] };
        let kit = WriteTrack { name: "Kit".into(), drums: true, program: 0, notes: vec![(0.0, 0.25, 36, 110), (0.5, 0.75, 38, 100)] };
        let bytes = write(&[(0.0, 96.0), (4.0, 120.0)], (3, 4), &[guitar, kit]);
        let file = parse(&bytes).unwrap();
        assert_eq!(file.tempo, Some(96.0));
        let tracks: Vec<&MidiTrack> = file.tracks.iter().filter(|t| !t.notes.is_empty()).collect();
        assert_eq!(tracks.len(), 2);
        assert_eq!((tracks[0].name.as_str(), tracks[0].drums, tracks[1].name.as_str(), tracks[1].drums), ("Guitare", false, "Kit", true));
        let notes: Vec<(f64, f64, u8, u8)> = tracks[0].notes.iter().map(|n| (n.start, n.length, n.pitch, n.velocity)).collect();
        assert!(notes.contains(&(0.0, 1.0, 64, 100)) && notes.contains(&(1.0, 0.5, 67, 90)) && notes.contains(&(1.0, 1.0, 60, 80)), "{notes:?}");
        assert_eq!(tracks[1].notes.len(), 2);
        // A long value and a note that ends where it starts are written all the same.
        let long = WriteTrack { notes: vec![(1000.0, 1000.0, 60, 0)], ..WriteTrack::default() };
        assert_eq!(parse(&write(&[], (4, 4), &[long])).unwrap().tracks.iter().map(|t| t.notes.len()).sum::<usize>(), 1);
    }
}
