//! SF3 sound banks: SoundFont 2 files whose samples are stored as Ogg Vorbis
//! (the format introduced by MuseScore). An SF3 file is turned back into a
//! plain SF2 file in memory, which the SoundFont player then reads.

use crate::codec;

/// Bit of `sampleType` that marks a sample as Ogg Vorbis.
const COMPRESSED: u16 = 0x10;
const SHDR: usize = 46;

/// The little-endian u32 at `at`, if the data reaches that far.
pub(crate) fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

/// The chunks found in `data`: (id, body).
fn chunks(data: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos + 8 <= data.len() {
        let id = [data[pos], data[pos + 1], data[pos + 2], data[pos + 3]];
        let len = u32_at(data, pos + 4).unwrap_or(0) as usize;
        let end = (pos + 8).saturating_add(len).min(data.len());
        out.push((id, &data[pos + 8..end]));
        pos = end + (len & 1);
    }
    out
}

/// A RIFF chunk: its id, its length, its body, padded to an even length.
pub(crate) fn chunk(id: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(body.len() + 9);
    out.extend_from_slice(id);
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    if body.len() % 2 == 1 {
        out.push(0);
    }
    out
}

struct Parts<'a> {
    info: &'a [u8],
    smpl: &'a [u8],
    /// The chunks of the "pdta" list, in order.
    pdta: Vec<([u8; 4], &'a [u8])>,
}

fn parts(file: &[u8]) -> Result<Parts<'_>, String> {
    let bad = || "this file is not a SoundFont bank".to_owned();
    if file.len() < 12 || &file[0..4] != b"RIFF" || &file[8..12] != b"sfbk" {
        return Err(bad());
    }
    let (mut info, mut smpl, mut pdta) = (None, None, None);
    for (id, body) in chunks(&file[12..]) {
        if &id != b"LIST" || body.len() < 4 {
            continue;
        }
        match &body[0..4] {
            b"INFO" => info = Some(&body[4..]),
            b"sdta" => smpl = chunks(&body[4..]).into_iter().find(|c| &c.0 == b"smpl").map(|c| c.1),
            b"pdta" => pdta = Some(chunks(&body[4..])),
            _ => {}
        }
    }
    Ok(Parts { info: info.ok_or_else(bad)?, smpl: smpl.ok_or_else(bad)?, pdta: pdta.ok_or_else(bad)? })
}

/// Whether a SoundFont file holds compressed (SF3) samples.
pub fn is_sf3(file: &[u8]) -> bool {
    let Ok(parts) = parts(file) else { return false };
    let Some(shdr) = parts.pdta.iter().find(|c| &c.0 == b"shdr").map(|c| c.1) else { return false };
    shdr.as_chunks::<SHDR>().0.iter().any(|record| u16::from_le_bytes([record[44], record[45]]) & COMPRESSED != 0)
}

/// Decodes the samples of an SF3 file and returns the equivalent SF2 file.
pub fn to_sf2(file: &[u8]) -> Result<Vec<u8>, String> {
    let parts = parts(file)?;
    let shdr = parts.pdta.iter().find(|c| &c.0 == b"shdr").map(|c| c.1).ok_or("SoundFont bank without samples")?;
    let count = shdr.len() / SHDR;
    let mut samples: Vec<i16> = Vec::new();
    let mut headers = Vec::with_capacity(shdr.len());

    for (index, record) in shdr.as_chunks::<SHDR>().0.iter().enumerate() {
        let mut record = record.to_vec();
        // The last record only marks the end of the list.
        if index + 1 == count {
            headers.extend_from_slice(&record);
            break;
        }
        let field = |at: usize| u32::from_le_bytes([record[at], record[at + 1], record[at + 2], record[at + 3]]) as usize;
        let (start, end, loop_start, loop_end) = (field(20), field(24), field(28), field(32));
        let kind = u16::from_le_bytes([record[44], record[45]]);
        let new_start = samples.len();
        let (loop_from, loop_to);
        if kind & COMPRESSED != 0 {
            // `start` and `end` are byte offsets of an Ogg Vorbis stream; the loop
            // points count samples from the start of the decoded sound.
            let ogg = parts.smpl.get(start..end).ok_or("damaged SF3 bank (sample out of bounds)")?;
            let clip = codec::decode_compressed(ogg.to_vec(), "ogg").map_err(|e| format!("unreadable SF3 sample: {e}"))?;
            let step = usize::from(clip.channels.max(1));
            samples.extend(clip.samples.iter().step_by(step).map(|s| (s.clamp(-1.0, 1.0) * 32767.0) as i16));
            (loop_from, loop_to) = (new_start + loop_start, new_start + loop_end);
        } else {
            // Uncompressed samples may be mixed in: positions count 16-bit samples.
            let raw = parts.smpl.get(start * 2..end * 2).unwrap_or(&[]);
            samples.extend(raw.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes([b[0], b[1]])));
            (loop_from, loop_to) = (new_start + loop_start.saturating_sub(start), new_start + loop_end.saturating_sub(start));
        }
        let new_end = samples.len();
        // The format asks for 46 silent samples after each sound.
        samples.extend(std::iter::repeat_n(0, 46));
        if samples.len() > 1_500_000_000 {
            return Err("the SF3 bank is too large once unpacked".to_owned());
        }
        for (at, value) in [(20, new_start), (24, new_end), (28, loop_from.min(new_end)), (32, loop_to.min(new_end))] {
            record[at..at + 4].copy_from_slice(&(value as u32).to_le_bytes());
        }
        record[44..46].copy_from_slice(&(kind & !COMPRESSED).to_le_bytes());
        headers.extend_from_slice(&record);
    }

    // Rebuild the file: same description and presets, new samples and sample headers.
    let mut info = b"INFO".to_vec();
    for (id, body) in chunks(parts.info) {
        // SF3 files announce version 3; the content is now plain version 2.
        let body = if &id == b"ifil" { vec![2, 0, 1, 0] } else { body.to_vec() };
        info.extend(chunk(&id, &body));
    }
    let mut smpl = Vec::with_capacity(samples.len() * 2);
    for sample in &samples {
        smpl.extend_from_slice(&sample.to_le_bytes());
    }
    let mut sdta = b"sdta".to_vec();
    sdta.extend(chunk(b"smpl", &smpl));
    let mut pdta = b"pdta".to_vec();
    for (id, body) in &parts.pdta {
        pdta.extend(chunk(id, if id == b"shdr" { &headers } else { body }));
    }
    let mut body = b"sfbk".to_vec();
    body.extend(chunk(b"LIST", &info));
    body.extend(chunk(b"LIST", &sdta));
    body.extend(chunk(b"LIST", &pdta));
    Ok(chunk(b"RIFF", &body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testfont::tiny_soundfont;

    #[test]
    fn plain_soundfonts_pass_through_unchanged_in_content() {
        let sf2 = tiny_soundfont();
        assert!(!is_sf3(&sf2));
        // Rebuilding a file with no compressed sample gives an equivalent bank.
        let rebuilt = to_sf2(&sf2).unwrap();
        let font = rustysynth::SoundFont::new(&mut rebuilt.as_slice()).unwrap();
        assert_eq!(crate::synth::font_presets(&font).len(), 2);
        assert_eq!(font.get_sample_headers().len(), 1);
        assert_eq!(font.get_sample_headers()[0].get_end() - font.get_sample_headers()[0].get_start(), 22_050);
    }

    #[test]
    fn compressed_flag_is_detected_and_bad_data_is_refused() {
        let mut sf3 = tiny_soundfont();
        // Mark the one sample as compressed: its "Ogg" data is then garbage.
        let shdr = sf3.windows(4).rposition(|w| w == b"shdr").unwrap();
        sf3[shdr + 8 + 44] |= COMPRESSED as u8;
        assert!(is_sf3(&sf3));
        assert!(to_sf2(&sf3).unwrap_err().contains("unreadable"));
        assert!(to_sf2(b"RIFFxxxxsfbk").is_err());
        assert!(!is_sf3(b""));
        for cut in (0..sf3.len()).step_by(97) {
            let _ = to_sf2(&sf3[..cut]);
            let _ = is_sf3(&sf3[..cut]);
        }
    }
}
