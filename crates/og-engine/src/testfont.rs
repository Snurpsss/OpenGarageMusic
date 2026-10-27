//! Builds a minimal SoundFont (SF2) in memory, from the public SoundFont 2
//! specification. It holds one looped sine-wave sample and two presets, and
//! lets the tests exercise SoundFont support without shipping a sound bank.

use crate::sf3::chunk;

fn list(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let mut body = kind.to_vec();
    for c in chunks {
        body.extend_from_slice(c);
    }
    chunk(b"LIST", &body)
}

fn name(text: &str) -> [u8; 20] {
    let mut out = [0u8; 20];
    out[..text.len().min(19)].copy_from_slice(&text.as_bytes()[..text.len().min(19)]);
    out
}

/// A valid SF2 file: "Sinus" at bank 0 preset 0 and "Kit" at bank 128 preset 0.
pub fn tiny_soundfont() -> Vec<u8> {
    // One second of A4 at 22.05 kHz; 441 periods fit exactly, so it loops cleanly.
    const RATE: u32 = 22_050;
    let frames = RATE as usize;
    let mut smpl = Vec::with_capacity((frames + 46) * 2);
    for i in 0..frames {
        let s = (std::f32::consts::TAU * 440.0 * i as f32 / RATE as f32).sin();
        smpl.extend_from_slice(&((s * 20_000.0) as i16).to_le_bytes());
    }
    smpl.extend_from_slice(&[0u8; 92]); // the 46 zero samples the format asks for

    let mut phdr = Vec::new();
    for (label, preset, bank, bag) in [("Sinus", 0u16, 0u16, 0u16), ("Kit", 0, 128, 1), ("EOP", 0, 0, 2)] {
        phdr.extend_from_slice(&name(label));
        phdr.extend_from_slice(&preset.to_le_bytes());
        phdr.extend_from_slice(&bank.to_le_bytes());
        phdr.extend_from_slice(&bag.to_le_bytes());
        phdr.extend_from_slice(&[0u8; 12]);
    }
    let words = |values: &[u16]| values.iter().flat_map(|v| v.to_le_bytes()).collect::<Vec<u8>>();
    // Each preset zone has one generator: "instrument" (41) = 0.
    let pbag = words(&[0, 0, 1, 0, 2, 0]);
    let pgen = words(&[41, 0, 41, 0, 0, 0]);
    let mods = vec![0u8; 10];
    let mut inst = Vec::new();
    for (label, bag) in [("Sinus", 0u16), ("EOI", 1)] {
        inst.extend_from_slice(&name(label));
        inst.extend_from_slice(&bag.to_le_bytes());
    }
    // The instrument zone: loop the sample (54 = 1), then "sampleID" (53) = 0.
    let ibag = words(&[0, 0, 2, 0]);
    let igen = words(&[54, 1, 53, 0, 0, 0]);
    let mut shdr = Vec::new();
    shdr.extend_from_slice(&name("Sinus"));
    for value in [0u32, frames as u32, 0, frames as u32, RATE] {
        shdr.extend_from_slice(&value.to_le_bytes());
    }
    shdr.extend_from_slice(&[69, 0]); // original pitch A4, no correction
    shdr.extend_from_slice(&words(&[0, 1])); // no link, mono
    shdr.extend_from_slice(&name("EOS"));
    shdr.extend_from_slice(&[0u8; 26]);

    let info = list(
        b"INFO",
        &[chunk(b"ifil", &words(&[2, 1])), chunk(b"isng", b"EMU8000\0"), chunk(b"INAM", b"Banque d'essai\0\0")],
    );
    let sdta = list(b"sdta", &[chunk(b"smpl", &smpl)]);
    let pdta = list(
        b"pdta",
        &[
            chunk(b"phdr", &phdr),
            chunk(b"pbag", &pbag),
            chunk(b"pmod", &mods),
            chunk(b"pgen", &pgen),
            chunk(b"inst", &inst),
            chunk(b"ibag", &ibag),
            chunk(b"imod", &mods),
            chunk(b"igen", &igen),
            chunk(b"shdr", &shdr),
        ],
    );
    let mut body = b"sfbk".to_vec();
    body.extend_from_slice(&info);
    body.extend_from_slice(&sdta);
    body.extend_from_slice(&pdta);
    chunk(b"RIFF", &body)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rustysynth::SoundFont;

    use super::*;
    use crate::analysis::detect_pitch;
    use crate::model::{SamplerParams, Sound};
    use crate::synth::{font_presets, Bank, Fonts, SoundRt};

    #[test]
    fn a_soundfont_preset_plays_in_tune_and_stops() {
        let font = Arc::new(SoundFont::new(&mut tiny_soundfont().as_slice()).unwrap());
        assert_eq!(font_presets(&font), vec![("Sinus".to_owned(), 0, 0), ("Kit".to_owned(), 128, 0)]);
        let mut fonts = Fonts::new();
        fonts.insert(7, Bank::Sf2(font));

        let sound = Sound::Sampler(SamplerParams { font: 7, bank: 0, preset: 0, gain: 1.0 });
        let mut rt = SoundRt::new(&sound, 48_000.0, &fonts);
        assert!(matches!(rt, SoundRt::Sampler(_)));
        // An octave below the sample's own pitch.
        rt.note_on(57, 100);
        let mut buf = vec![0.0f32; 24_000];
        rt.render(&mut buf);
        let found = detect_pitch(&buf[8000..12_096], 48_000.0).unwrap();
        assert!((found - 220.0).abs() < 2.0, "{found}");
        rt.note_off(57);
        let mut tail = vec![0.0f32; 96_000];
        rt.render(&mut tail);
        assert!(rt.is_silent());

        // A preset whose file is not loaded is silent, and comes alive once it is.
        let mut missing = SoundRt::new(&sound, 48_000.0, &Fonts::new());
        assert!(matches!(missing, SoundRt::Missing(_)));
        missing.note_on(60, 100);
        missing.render(&mut buf);
        missing.update(&sound, 48_000.0, &fonts);
        assert!(matches!(missing, SoundRt::Sampler(_)));
        assert!(!sound.is_drums());
        assert!(Sound::Sampler(SamplerParams { font: 7, bank: 128, preset: 0, gain: 1.0 }).is_drums());
        assert!(SoundFont::new(&mut &b"not a soundfont"[..]).is_err());
    }
}
