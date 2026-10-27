//! Compressed export (MP3, AAC) through the encoders that ship with Windows
//! (Media Foundation). No codec is bundled with the application.

use crate::i18n::tr;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Compressed {
    Mp3,
    /// AAC in an MP4 container (.m4a).
    Aac,
}

impl Compressed {
    pub fn extension(self) -> &'static str {
        match self {
            Compressed::Mp3 => "mp3",
            Compressed::Aac => "m4a",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Compressed::Mp3 => "MP3",
            Compressed::Aac => "AAC",
        }
    }
}

/// Writes interleaved stereo `samples` at 44.1 or 48 kHz as a compressed file.
pub fn encode(path: &Path, samples: &[f32], sample_rate: u32, format: Compressed, kbps: u32) -> Result<(), String> {
    if !matches!(sample_rate, 44_100 | 48_000) {
        return Err(tr("unsupported sample rate").to_owned());
    }
    if samples.is_empty() {
        return Err(tr("there is nothing to export").to_owned());
    }
    imp::encode(path, samples, sample_rate, format, kbps)
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn encode(_: &Path, _: &[f32], _: u32, format: Compressed, _: u32) -> Result<(), String> {
        Err(tf!("{} export is only available on Windows for now", format.label()))
    }
}

#[cfg(windows)]
mod imp {
    use windows::core::HSTRING;
    use windows::Win32::Media::MediaFoundation::{
        IMFMediaType, MFAudioFormat_AAC, MFAudioFormat_MP3, MFAudioFormat_PCM, MFCreateMediaType,
        MFCreateMemoryBuffer, MFCreateSample, MFCreateSinkWriterFromURL, MFMediaType_Audio,
        MFShutdown, MFStartup, MFSTARTUP_FULL, MF_MT_ALL_SAMPLES_INDEPENDENT,
        MF_MT_AUDIO_AVG_BYTES_PER_SECOND, MF_MT_AUDIO_BITS_PER_SAMPLE, MF_MT_AUDIO_BLOCK_ALIGNMENT,
        MF_MT_AUDIO_NUM_CHANNELS, MF_MT_AUDIO_SAMPLES_PER_SECOND, MF_MT_MAJOR_TYPE, MF_MT_SUBTYPE,
    };
    use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

    use super::*;

    /// MF_SDK_VERSION 2, MF_API_VERSION 0x70.
    const MF_VERSION: u32 = 0x0002_0070;

    pub fn encode(path: &Path, samples: &[f32], sample_rate: u32, format: Compressed, kbps: u32) -> Result<(), String> {
        // Media Foundation wants its own COM apartment; a worker thread keeps
        // it away from the one the window uses.
        let path = path.to_path_buf();
        let pcm: Vec<i16> = samples.iter().map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i16).collect();
        std::thread::spawn(move || {
            // SAFETY: plain COM calls on objects created and released in this thread.
            unsafe {
                CoInitializeEx(None, COINIT_MULTITHREADED).ok().map_err(|e| e.message())?;
                let result = MFStartup(MF_VERSION, MFSTARTUP_FULL)
                    .map_err(|e| e.message())
                    .and_then(|()| {
                        let written = write(&path, &pcm, sample_rate, format, kbps).map_err(|e| e.message());
                        let _ = MFShutdown();
                        written
                    });
                CoUninitialize();
                result
            }
        })
        .join()
        .map_err(|_| tr("the encoder stopped unexpectedly").to_owned())?
        .map_err(|e| tf!("{} encoding failed: {e}" [e], format.label()))
    }

    unsafe fn audio_type(subtype: &windows::core::GUID, sample_rate: u32, bytes_per_second: u32, bits: bool) -> windows::core::Result<IMFMediaType> {
        let kind = MFCreateMediaType()?;
        kind.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
        kind.SetGUID(&MF_MT_SUBTYPE, subtype)?;
        // The MP3 encoder rejects a type that states a sample size.
        if bits {
            kind.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
        }
        kind.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, sample_rate)?;
        kind.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
        kind.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, bytes_per_second)?;
        Ok(kind)
    }

    unsafe fn write(path: &Path, pcm: &[i16], sample_rate: u32, format: Compressed, kbps: u32) -> windows::core::Result<()> {
        let writer = MFCreateSinkWriterFromURL(&HSTRING::from(path.as_os_str()), None, None)?;
        // The encoders only accept a few bit rates: pick the nearest.
        let (subtype, rates): (_, &[u32]) = match format {
            Compressed::Mp3 => (&MFAudioFormat_MP3, &[128, 160, 192, 224, 256, 320]),
            Compressed::Aac => (&MFAudioFormat_AAC, &[96, 128, 160, 192]),
        };
        let kbps = rates.iter().copied().min_by_key(|r| r.abs_diff(kbps)).unwrap_or(192);
        let output = audio_type(subtype, sample_rate, kbps * 1000 / 8, format == Compressed::Aac)?;
        let stream = writer.AddStream(&output).map_err(|e| windows::core::Error::new(e.code(), tf!("output stream: {}", e.message())))?;
        let input = audio_type(&MFAudioFormat_PCM, sample_rate, sample_rate * 4, true)?;
        input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
        input.SetUINT32(&MF_MT_ALL_SAMPLES_INDEPENDENT, 1)?;
        writer.SetInputMediaType(stream, &input, None).map_err(|e| windows::core::Error::new(e.code(), tf!("input format: {}", e.message())))?;
        writer.BeginWriting()?;

        // Times are in units of 100 nanoseconds.
        let ticks = |frames: usize| (frames as i64) * 10_000_000 / i64::from(sample_rate);
        let mut frames_done = 0usize;
        for block in pcm.chunks(sample_rate as usize * 2) {
            let bytes = (block.len() * 2) as u32;
            let buffer = MFCreateMemoryBuffer(bytes)?;
            let mut data = std::ptr::null_mut();
            buffer.Lock(&mut data, None, None)?;
            std::ptr::copy_nonoverlapping(block.as_ptr().cast::<u8>(), data, bytes as usize);
            buffer.Unlock()?;
            buffer.SetCurrentLength(bytes)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buffer)?;
            let frames = block.len() / 2;
            sample.SetSampleTime(ticks(frames_done))?;
            sample.SetSampleDuration(ticks(frames))?;
            writer.WriteSample(stream, &sample)?;
            frames_done += frames;
        }
        writer.Finalize()
    }
}
