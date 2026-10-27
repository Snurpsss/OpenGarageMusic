//! Reading compressed audio (MP3, AAC in MP4/M4A or ADTS, FLAC, Ogg Vorbis)
//! through the Symphonia decoders, and WAV through our own reader.

use std::io::Cursor;
use std::path::Path;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::engine::ClipData;
use crate::wav;

/// File extensions `decode_file` understands.
pub const AUDIO_EXTENSIONS: [&str; 7] = ["wav", "mp3", "m4a", "aac", "mp4", "flac", "ogg"];

/// The extension of a file, in lower case; empty when it has none.
pub fn extension_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Decodes an audio file, choosing the reader from its extension.
pub fn decode_file(path: &Path) -> Result<ClipData, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let extension = extension_of(path);
    if extension == "wav" {
        wav::decode_wav(&bytes)
    } else {
        decode_compressed(bytes, &extension)
    }
}

/// Decodes MP3 or AAC data. `extension` is a hint for telling formats apart.
pub fn decode_compressed(bytes: Vec<u8>, extension: &str) -> Result<ClipData, String> {
    let unsupported = |e: Error| format!("unsupported audio format ({e})");
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut hint = Hint::new();
    if !extension.is_empty() {
        hint.with_extension(extension);
    }
    let probed = symphonia::default::get_probe()
        .format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default())
        .map_err(unsupported)?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("this file holds no audio track")?;
    let track_id = track.id;
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(unsupported)?;

    let mut samples: Vec<f32> = Vec::new();
    let mut sample_rate = 0u32;
    let mut channels = 0usize;
    // A damaged packet is skipped; a damaged file ends the decoding early.
    while let Ok(packet) = format.next_packet() {
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                if channels == 0 {
                    sample_rate = spec.rate;
                    channels = spec.channels.count();
                }
                if spec.channels.count() != channels {
                    continue;
                }
                let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                buffer.copy_interleaved_ref(decoded);
                samples.extend_from_slice(buffer.samples());
                // Four hours of stereo at 48 kHz: refuse to grow without limit.
                if samples.len() > 48_000 * 2 * 3600 * 4 {
                    break;
                }
            }
            Err(Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }
    if channels == 0 || sample_rate == 0 || samples.is_empty() {
        return Err("no sound could be read from this file".to_owned());
    }
    // Keep at most two channels.
    if channels > 2 {
        samples = samples.chunks_exact(channels).flat_map(|f| [f[0], f[1]]).collect();
        channels = 2;
    }
    for sample in &mut samples {
        *sample = if sample.is_finite() { sample.clamp(-4.0, 4.0) } else { 0.0 };
    }
    Ok(ClipData { sample_rate, channels: channels as u16, samples })
}

/// The artist and the title an audio file says it holds, as its tags (ID3,
/// Vorbis comments, MP4 atoms) give them; empty when it says nothing. Only
/// the head of the file is read.
pub fn tags(path: &Path) -> (String, String) {
    use symphonia::core::meta::StandardTagKey;
    let Ok(file) = std::fs::File::open(path) else { return Default::default() };
    let stream = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(extension) = path.extension() {
        hint.with_extension(&extension.to_string_lossy());
    }
    let Ok(mut probed) = symphonia::default::get_probe().format(&hint, stream, &FormatOptions::default(), &MetadataOptions::default()) else {
        return Default::default();
    };
    let (mut artist, mut title) = (String::new(), String::new());
    let mut read = |revision: &symphonia::core::meta::MetadataRevision| {
        for tag in revision.tags() {
            match tag.std_key {
                Some(StandardTagKey::Artist) if artist.is_empty() => artist = tag.value.to_string(),
                Some(StandardTagKey::AlbumArtist) if artist.is_empty() => artist = tag.value.to_string(),
                Some(StandardTagKey::TrackTitle) if title.is_empty() => title = tag.value.to_string(),
                _ => {}
            }
        }
    };
    // Tags before the audio (ID3v2), then those in the stream itself.
    if let Some(metadata) = probed.metadata.get() {
        if let Some(revision) = metadata.current() {
            read(revision);
        }
    }
    if let Some(revision) = probed.format.metadata().current() {
        read(revision);
    }
    (artist.trim().to_owned(), title.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(decode_compressed(Vec::new(), "mp3").is_err());
        assert!(decode_compressed(b"this is not audio at all".to_vec(), "mp3").is_err());
        let noise: Vec<u8> = (0..20_000u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        for extension in ["mp3", "m4a", "aac", ""] {
            assert!(decode_compressed(noise.clone(), extension).is_err(), "{extension}");
        }
        assert!(decode_file(Path::new("does-not-exist.mp3")).is_err());
    }
}
