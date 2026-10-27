//! WAV reading and writing (PCM and float), written from the public format description.

use std::io;
use std::path::Path;

use crate::engine::ClipData;

fn header(out: &mut Vec<u8>, data_len: u32, sample_rate: u32, channels: u16, bits: u16) {
    let block_align = channels * (bits / 8);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(block_align)).to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
}

/// Encodes interleaved samples in `[-1, 1]` as a PCM WAV file of 16 or 24 bits.
pub fn encode_wav(samples: &[f32], sample_rate: u32, channels: u16, bits: u16) -> io::Result<Vec<u8>> {
    let bytes = if bits == 24 { 3 } else { 2 };
    let data_len = u32::try_from(samples.len() * bytes)
        .ok()
        .filter(|len| *len <= u32::MAX - 36)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "audio too long for a WAV file"))?;
    let mut out = Vec::with_capacity(44 + samples.len() * bytes);
    header(&mut out, data_len, sample_rate, channels, if bits == 24 { 24 } else { 16 });
    for sample in samples {
        let s = sample.clamp(-1.0, 1.0);
        if bits == 24 {
            let value = (f64::from(s) * 8_388_607.0).round() as i32;
            out.extend_from_slice(&value.to_le_bytes()[..3]);
        } else {
            out.extend_from_slice(&((s * 32767.0).round() as i16).to_le_bytes());
        }
    }
    Ok(out)
}

pub fn write_wav(path: &Path, samples: &[f32], sample_rate: u32, channels: u16, bits: u16) -> io::Result<()> {
    std::fs::write(path, encode_wav(samples, sample_rate, channels, bits)?)
}

/// Encodes interleaved samples in `[-1, 1]` as an AIFF file of 16 or 24 bits:
/// the same sound as a WAV file, in the layout of the other family of
/// computers (most significant byte first).
pub fn encode_aiff(samples: &[f32], sample_rate: u32, channels: u16, bits: u16) -> io::Result<Vec<u8>> {
    let bits: u16 = if bits == 24 { 24 } else { 16 };
    let bytes = usize::from(bits / 8);
    let channels = channels.max(1);
    let frames = samples.len() / usize::from(channels);
    let data_len = u32::try_from(frames * usize::from(channels) * bytes)
        .ok()
        .filter(|len| *len <= u32::MAX - 64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "audio too long for an AIFF file"))?;
    let mut out = Vec::with_capacity(54 + data_len as usize);
    out.extend_from_slice(b"FORM");
    out.extend_from_slice(&(46 + data_len).to_be_bytes());
    out.extend_from_slice(b"AIFFCOMM");
    out.extend_from_slice(&18u32.to_be_bytes());
    out.extend_from_slice(&channels.to_be_bytes());
    out.extend_from_slice(&(frames as u32).to_be_bytes());
    out.extend_from_slice(&bits.to_be_bytes());
    // The sample rate as an 80-bit number: an exponent, then the digits.
    let rate = u64::from(sample_rate.max(1));
    let top = 63 - rate.leading_zeros();
    out.extend_from_slice(&(16_383 + top as u16).to_be_bytes());
    out.extend_from_slice(&(rate << (63 - top)).to_be_bytes());
    out.extend_from_slice(b"SSND");
    out.extend_from_slice(&(8 + data_len).to_be_bytes());
    out.extend_from_slice(&[0; 8]);
    for sample in &samples[..frames * usize::from(channels)] {
        let s = sample.clamp(-1.0, 1.0);
        if bits == 24 {
            let value = (f64::from(s) * 8_388_607.0).round() as i32;
            out.extend_from_slice(&value.to_be_bytes()[1..]);
        } else {
            out.extend_from_slice(&((s * 32767.0).round() as i16).to_be_bytes());
        }
    }
    Ok(out)
}

pub fn write_aiff(path: &Path, samples: &[f32], sample_rate: u32, channels: u16, bits: u16) -> io::Result<()> {
    std::fs::write(path, encode_aiff(samples, sample_rate, channels, bits)?)
}

/// Decodes a WAV file: PCM of 8, 16, 24 or 32 bits, or 32-bit float.
pub fn decode_wav(data: &[u8]) -> Result<ClipData, String> {
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("this is not a WAV file".to_owned());
    }
    let u16_at = |at: usize| u16::from_le_bytes([data[at], data[at + 1]]);
    let u32_at = |at: usize| u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]]);
    let mut format: Option<(u16, u16, u32, u16)> = None;
    let mut pos = 12;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let len = u32_at(pos + 4) as usize;
        let body = pos + 8;
        let end = body.saturating_add(len).min(data.len());
        if id == b"fmt " && end - body >= 16 {
            let mut tag = u16_at(body);
            // "Extensible" files keep the real format in a sub-field.
            if tag == 0xFFFE && end - body >= 26 {
                tag = u16_at(body + 24);
            }
            format = Some((tag, u16_at(body + 2), u32_at(body + 4), u16_at(body + 14)));
        } else if id == b"data" {
            let (tag, channels, sample_rate, bits) = format.ok_or("WAV file without a format description")?;
            if channels == 0 || channels > 8 || sample_rate == 0 {
                return Err("invalid WAV file".to_owned());
            }
            let raw = &data[body..end];
            let samples: Vec<f32> = match (tag, bits) {
                (1, 8) => raw.iter().map(|b| (f32::from(*b) - 128.0) / 128.0).collect(),
                (1, 16) => raw.as_chunks::<2>().0.iter().map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0).collect(),
                (1, 24) => raw
                    .as_chunks::<3>().0.iter()
                    .map(|b| (i32::from_le_bytes([0, b[0], b[1], b[2]]) >> 8) as f32 / 8_388_608.0)
                    .collect(),
                (1, 32) => raw
                    .as_chunks::<4>().0.iter()
                    .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f32 / 2_147_483_648.0)
                    .collect(),
                (3, 32) => raw
                    .as_chunks::<4>().0.iter()
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .map(|s| if s.is_finite() { s.clamp(-4.0, 4.0) } else { 0.0 })
                    .collect(),
                _ => return Err(format!("unsupported WAV format (type {tag}, {bits} bits)")),
            };
            // Keep at most two channels.
            let channels = usize::from(channels);
            let kept = channels.min(2);
            let samples = if channels > 2 {
                samples.chunks_exact(channels).flat_map(|f| [f[0], f[1]]).collect()
            } else {
                let whole = samples.len() - samples.len() % channels;
                samples[..whole].to_vec()
            };
            return Ok(ClipData { sample_rate, channels: kept as u16, samples });
        }
        // Chunks are padded to an even size.
        pos = end.saturating_add(len & 1);
    }
    Err("WAV file without audio data".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_and_samples() {
        let bytes = encode_wav(&[0.0, 1.0, -1.0, 2.0], 44_100, 2, 16).unwrap();
        assert_eq!(bytes.len(), 44 + 8);
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(bytes[4..8].try_into().unwrap()), 36 + 8);
        assert_eq!(u16::from_le_bytes(bytes[22..24].try_into().unwrap()), 2);
        assert_eq!(u32::from_le_bytes(bytes[24..28].try_into().unwrap()), 44_100);
        assert_eq!(u32::from_le_bytes(bytes[28..32].try_into().unwrap()), 176_400);
        assert_eq!(i16::from_le_bytes(bytes[46..48].try_into().unwrap()), 32767);
        assert_eq!(i16::from_le_bytes(bytes[48..50].try_into().unwrap()), -32767);
        // Out-of-range input is clamped.
        assert_eq!(i16::from_le_bytes(bytes[50..52].try_into().unwrap()), 32767);
    }

    #[test]
    fn round_trip_16_and_24_bits() {
        let samples = [0.0f32, 0.5, -0.5, 0.25, -1.0, 1.0];
        for bits in [16u16, 24] {
            let clip = decode_wav(&encode_wav(&samples, 48_000, 2, bits).unwrap()).unwrap();
            assert_eq!((clip.sample_rate, clip.channels), (48_000, 2));
            assert_eq!(clip.samples.len(), samples.len());
            for (a, b) in clip.samples.iter().zip(&samples) {
                assert!((a - b).abs() < 1e-3, "{bits} bits: {a} {b}");
            }
        }
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        assert!(decode_wav(b"").is_err());
        assert!(decode_wav(b"RIFFxxxxWAVE").is_err());
        let good = encode_wav(&[0.1; 64], 44_100, 1, 16).unwrap();
        for cut in 0..good.len() {
            let _ = decode_wav(&good[..cut]);
        }
        let mut odd = good.clone();
        odd[22] = 0; // zero channels
        assert!(decode_wav(&odd).is_err());
    }
}
