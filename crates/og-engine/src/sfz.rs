//! SFZ instruments: a text file that maps audio samples to keys and velocities.
//! Written from the public description of the format. The common opcodes of
//! SFZ 1 are understood; anything else is ignored, so most instruments load
//! and play even when some refinement is lost.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::codec;
use crate::engine::ClipData;
use crate::synth::Rng;

/// Total sample frames an instrument may load (about 1.2 GB in memory).
const MAX_FRAMES: usize = 300_000_000;
const MAX_VOICES: usize = 48;

#[derive(Clone, Debug)]
pub struct SfzRegion {
    pub sample: Arc<ClipData>,
    pub lokey: u8,
    pub hikey: u8,
    pub lovel: u8,
    pub hivel: u8,
    /// Key at which the sample plays at its recorded pitch.
    pub root: i32,
    /// Cents per key; 0 for sounds that do not follow the keyboard.
    pub keytrack: f32,
    /// Fixed pitch offset in cents.
    pub tune: f32,
    pub volume_db: f32,
    pub offset: usize,
    pub end: Option<usize>,
    pub loop_points: Option<(usize, usize)>,
    /// Plays to its end whatever the key does (drum hits).
    pub one_shot: bool,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    pub group: u32,
    pub off_by: u32,
    /// Random range in which the region is chosen (round-robin variations).
    pub lorand: f32,
    pub hirand: f32,
}

#[derive(Debug)]
pub struct SfzInstrument {
    pub name: String,
    pub regions: Vec<SfzRegion>,
}

/// "c4", "a#3", "db2" or a plain number, as a MIDI key.
fn parse_key(text: &str) -> Option<i32> {
    let text = text.trim().to_lowercase();
    if let Ok(number) = text.parse::<i32>() {
        return Some(number);
    }
    let mut chars = text.chars();
    let base = match chars.next()? {
        'c' => 0,
        'd' => 2,
        'e' => 4,
        'f' => 5,
        'g' => 7,
        'a' => 9,
        'b' => 11,
        _ => return None,
    };
    let rest: String = chars.collect();
    let (accidental, octave) = match rest.chars().next()? {
        '#' => (1, &rest[1..]),
        'b' => (-1, &rest[1..]),
        _ => (0, rest.as_str()),
    };
    Some(base + accidental + (octave.parse::<i32>().ok()? + 1) * 12)
}

/// Inlines `#include` files and applies `#define` variables; drops comments.
fn preprocess(path: &Path, depth: u32, defines: &mut Vec<(String, String)>, out: &mut String) -> Result<(), String> {
    if depth > 8 {
        return Err("SFZ files included in one another too deeply".to_owned());
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{} : {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let dir = path.parent().unwrap_or(Path::new("."));
    for line in text.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        if let Some(rest) = line.strip_prefix("#include") {
            let file = rest.trim().trim_matches('"').replace('\\', "/");
            // A missing include loses part of the instrument, not all of it.
            let _ = preprocess(&dir.join(file), depth + 1, defines, out);
        } else if let Some(rest) = line.strip_prefix("#define") {
            let mut parts = rest.split_whitespace();
            if let (Some(name), Some(value)) = (parts.next(), parts.next()) {
                defines.retain(|d| d.0 != name);
                defines.push((name.to_owned(), value.to_owned()));
                // Longest names first, so that $KEY2 is not read as $KEY followed by 2.
                defines.sort_by_key(|d| std::cmp::Reverse(d.0.len()));
            }
        } else if !line.is_empty() {
            let mut line = line.to_owned();
            for (name, value) in defines.iter() {
                line = line.replace(name, value);
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    Ok(())
}

/// Splits SFZ text into headers (`<region>`) and opcodes (`key=value`). A value
/// runs up to the next opcode or header, so file names may contain spaces.
fn tokens(text: &str) -> Vec<(String, String)> {
    let spaced = text.replace('<', " <").replace('>', "> ");
    let mut out: Vec<(String, String)> = Vec::new();
    for word in spaced.split_whitespace() {
        if word.starts_with('<') && word.ends_with('>') {
            out.push((word.to_lowercase(), String::new()));
        } else if let Some((key, value)) = word.split_once('=').filter(|(k, _)| !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')) {
            out.push((key.to_lowercase(), value.to_owned()));
        } else if let Some(last) = out.last_mut().filter(|l| !l.0.starts_with('<')) {
            last.1.push(' ');
            last.1.push_str(word);
        }
    }
    out
}

type Opcodes = HashMap<String, String>;

fn build_region(opcodes: &Opcodes, sample: Arc<ClipData>) -> Option<SfzRegion> {
    let text = |name: &str| opcodes.get(name).map(String::as_str);
    let number = |name: &str| text(name).and_then(|v| v.trim().parse::<f32>().ok());
    let key = |name: &str| text(name).and_then(parse_key);
    // Regions that only sound on key release or in legato are left out.
    if matches!(text("trigger"), Some("release" | "release_key" | "legato")) {
        return None;
    }
    // Of round-robin sequences, keep the first variation.
    if number("seq_position").is_some_and(|p| p > 1.0) {
        return None;
    }
    let single = key("key");
    let lokey = key("lokey").or(single).unwrap_or(0).clamp(0, 127);
    let hikey = key("hikey").or(single).unwrap_or(127).clamp(0, 127);
    let frames = sample.frames();
    let mode = text("loop_mode").or(text("loopmode")).unwrap_or("");
    let loop_start = number("loop_start").or(number("loopstart")).map(|v| v.max(0.0) as usize);
    let loop_end = number("loop_end").or(number("loopend")).map(|v| v.max(0.0) as usize);
    let looping = matches!(mode, "loop_continuous" | "loop_sustain") || (mode.is_empty() && loop_start.is_some() && loop_end.is_some());
    let loop_points = looping
        .then(|| (loop_start.unwrap_or(0), loop_end.unwrap_or(frames.saturating_sub(1)).min(frames.saturating_sub(1))))
        .filter(|(start, end)| end > start && end - start >= 2);
    Some(SfzRegion {
        lokey: lokey as u8,
        hikey: hikey as u8,
        lovel: number("lovel").unwrap_or(0.0).clamp(0.0, 127.0) as u8,
        hivel: number("hivel").unwrap_or(127.0).clamp(0.0, 127.0) as u8,
        root: key("pitch_keycenter").or(single).unwrap_or(60),
        keytrack: number("pitch_keytrack").unwrap_or(100.0),
        tune: number("tune").unwrap_or(0.0) + 100.0 * number("transpose").unwrap_or(0.0),
        volume_db: number("volume").unwrap_or(0.0).clamp(-96.0, 24.0),
        offset: number("offset").unwrap_or(0.0).max(0.0) as usize,
        end: number("end").filter(|v| *v > 0.0).map(|v| v as usize),
        loop_points,
        one_shot: mode == "one_shot",
        attack: number("ampeg_attack").unwrap_or(0.001).clamp(0.0, 30.0),
        decay: number("ampeg_decay").unwrap_or(0.0).clamp(0.0, 60.0),
        sustain: (number("ampeg_sustain").unwrap_or(100.0) / 100.0).clamp(0.0, 1.0),
        release: number("ampeg_release").unwrap_or(0.05).clamp(0.005, 60.0),
        group: number("group").unwrap_or(0.0) as u32,
        off_by: number("off_by").unwrap_or(0.0) as u32,
        lorand: number("lorand").unwrap_or(0.0),
        hirand: number("hirand").unwrap_or(1.0),
        sample,
    })
}

impl SfzInstrument {
    /// Reads an `.sfz` file and the samples it refers to.
    pub fn load(path: &Path) -> Result<Self, String> {
        let mut text = String::new();
        preprocess(path, 0, &mut Vec::new(), &mut text)?;
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let name = path.file_stem().map_or_else(|| "SFZ".to_owned(), |s| s.to_string_lossy().into_owned());

        // Opcodes are inherited: global, then master, then group, then region.
        let (mut control, mut global, mut master, mut group, mut region) =
            (Opcodes::new(), Opcodes::new(), Opcodes::new(), Opcodes::new(), Opcodes::new());
        let mut level = "";
        let mut wanted: Vec<Opcodes> = Vec::new();
        let mut finish = |region: &mut Opcodes, global: &Opcodes, master: &Opcodes, group: &Opcodes, level: &str| {
            if level == "<region>" {
                let mut all = global.clone();
                all.extend(master.clone());
                all.extend(group.clone());
                all.extend(std::mem::take(region));
                wanted.push(all);
            }
        };
        for (key, value) in tokens(&text) {
            if key.starts_with('<') {
                finish(&mut region, &global, &master, &group, level);
                match key.as_str() {
                    "<global>" => {
                        global.clear();
                        master.clear();
                        group.clear();
                    }
                    "<master>" => {
                        master.clear();
                        group.clear();
                    }
                    "<group>" => group.clear(),
                    _ => {}
                }
                level = match key.as_str() {
                    "<control>" => "<control>",
                    "<global>" => "<global>",
                    "<master>" => "<master>",
                    "<group>" => "<group>",
                    "<region>" => "<region>",
                    _ => "",
                };
            } else {
                let target = match level {
                    "<control>" => &mut control,
                    "<global>" => &mut global,
                    "<master>" => &mut master,
                    "<group>" => &mut group,
                    "<region>" => &mut region,
                    _ => continue,
                };
                target.insert(key, value);
            }
        }
        finish(&mut region, &global, &master, &group, level);

        let default_path = control.get("default_path").map_or("", String::as_str).replace('\\', "/");
        let mut cache: HashMap<PathBuf, Option<Arc<ClipData>>> = HashMap::new();
        let mut frames = 0usize;
        let mut regions = Vec::new();
        let mut missing = 0usize;
        for opcodes in &wanted {
            let Some(sample) = opcodes.get("sample") else { continue };
            // "*sine" and the like are built-in oscillators, not files.
            if sample.starts_with('*') {
                continue;
            }
            let file = dir.join(format!("{default_path}{}", sample.trim().replace('\\', "/")));
            let clip = cache.entry(file.clone()).or_insert_with(|| {
                if frames > MAX_FRAMES {
                    return None;
                }
                let clip = codec::decode_file(&file).ok()?;
                frames += clip.frames();
                Some(Arc::new(clip))
            });
            match clip {
                Some(clip) if clip.frames() >= 2 => regions.extend(build_region(opcodes, Arc::clone(clip))),
                _ => missing += 1,
            }
        }
        if regions.is_empty() {
            return Err(if missing > 0 {
                format!("none of the {missing} samples of this SFZ instrument could be read")
            } else {
                "this SFZ file holds no playable region".to_owned()
            });
        }
        Ok(Self { name, regions })
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Stage {
    Attack,
    Decay,
    Release,
}

struct Voice {
    region: usize,
    key: u8,
    pos: f64,
    step: f64,
    /// Where a bend, a slide or a vibrato has taken the pitch: a ratio.
    shift: f64,
    gain: f32,
    env: f32,
    stage: Stage,
    age: u64,
}

/// Plays an SFZ instrument.
pub struct SfzSynth {
    instrument: Arc<SfzInstrument>,
    sample_rate: f32,
    pub gain: f32,
    voices: Vec<Voice>,
    rng: Rng,
    clock: u64,
}

impl SfzSynth {
    pub fn new(instrument: Arc<SfzInstrument>, sample_rate: f32, gain: f32) -> Self {
        Self { instrument, sample_rate, gain, voices: Vec::with_capacity(MAX_VOICES), rng: Rng(0x5F2A_11C3), clock: 0 }
    }

    pub fn note_on(&mut self, pitch: u8, velocity: u8) {
        self.clock += 1;
        let random = self.rng.unit();
        let instrument = Arc::clone(&self.instrument);
        for (index, region) in instrument.regions.iter().enumerate() {
            let matches = (region.lokey..=region.hikey).contains(&pitch)
                && (region.lovel..=region.hivel).contains(&velocity)
                && random >= region.lorand
                && (random < region.hirand || region.hirand >= 1.0);
            if !matches {
                continue;
            }
            // A new note of this group silences the regions that say "off_by" it.
            if region.group != 0 {
                for voice in &mut self.voices {
                    if instrument.regions[voice.region].off_by == region.group {
                        voice.stage = Stage::Release;
                    }
                }
            }
            if self.voices.len() >= MAX_VOICES {
                let oldest = self.voices.iter().enumerate().min_by_key(|(_, v)| v.age).map(|(i, _)| i);
                if let Some(oldest) = oldest {
                    self.voices.swap_remove(oldest);
                }
            }
            let cents = (f32::from(pitch) - region.root as f32) * region.keytrack + region.tune;
            let step = f64::from(2f32.powf(cents / 1200.0)) * f64::from(region.sample.sample_rate) / f64::from(self.sample_rate);
            let level = f32::from(velocity.min(127)) / 127.0;
            self.voices.push(Voice {
                region: index,
                key: pitch,
                pos: region.offset as f64,
                step,
                shift: 1.0,
                gain: 10f32.powf(region.volume_db / 20.0) * level * level,
                env: if region.attack <= 0.0005 { 1.0 } else { 0.0 },
                stage: Stage::Attack,
                age: self.clock,
            });
        }
    }

    pub fn note_off(&mut self, pitch: u8) {
        for voice in &mut self.voices {
            if voice.key == pitch && !self.instrument.regions[voice.region].one_shot {
                voice.stage = Stage::Release;
            }
        }
    }

    /// Moves the notes sounding at `pitch` by `semitones`.
    pub fn bend(&mut self, pitch: u8, semitones: f32) {
        for voice in &mut self.voices {
            if voice.key == pitch {
                voice.shift = f64::from(2f32.powf(semitones / 12.0));
            }
        }
    }

    pub fn unbend(&mut self) {
        for voice in &mut self.voices {
            voice.shift = 1.0;
        }
    }

    pub fn release_all(&mut self) {
        for voice in &mut self.voices {
            voice.stage = Stage::Release;
        }
    }

    pub fn is_silent(&self) -> bool {
        self.voices.is_empty()
    }

    /// Adds the mono output to `out`.
    pub fn render(&mut self, out: &mut [f32]) {
        let sr = self.sample_rate;
        let master = self.gain;
        let regions = &self.instrument.regions;
        self.voices.retain_mut(|voice| {
            let region = &regions[voice.region];
            let clip = &region.sample;
            let channels = usize::from(clip.channels.max(1));
            let samples = &clip.samples;
            let last = region.end.unwrap_or(usize::MAX).min(clip.frames().saturating_sub(1));
            let attack = 1.0 / (region.attack.max(0.0005) * sr);
            let decay = (-5.0 / (region.decay.max(0.005) * sr)).exp();
            let release = (-5.0 / (region.release * sr)).exp();
            for sample in out.iter_mut() {
                match voice.stage {
                    Stage::Attack => {
                        voice.env += attack;
                        if voice.env >= 1.0 {
                            voice.env = 1.0;
                            voice.stage = Stage::Decay;
                        }
                    }
                    Stage::Decay => voice.env = region.sustain + (voice.env - region.sustain) * decay,
                    Stage::Release => {
                        voice.env *= release;
                        if voice.env < 1e-4 {
                            return false;
                        }
                    }
                }
                if let Some((start, end)) = region.loop_points {
                    if voice.stage != Stage::Release && voice.pos >= end as f64 {
                        voice.pos -= (end - start) as f64;
                    }
                }
                let i = voice.pos as usize;
                if i >= last {
                    return false;
                }
                let frac = (voice.pos - i as f64) as f32;
                let (a, b) = if channels == 1 {
                    (samples[i], samples[i + 1])
                } else {
                    (0.5 * (samples[i * channels] + samples[i * channels + 1]), 0.5 * (samples[(i + 1) * channels] + samples[(i + 1) * channels + 1]))
                };
                *sample += (a + (b - a) * frac) * voice.env * voice.gain * master;
                voice.pos += voice.step * voice.shift;
            }
            true
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::detect_pitch;
    use crate::wav;

    fn sine(hz: f32, seconds: f32, level: f32) -> Vec<f32> {
        (0..(44_100.0 * seconds) as usize).map(|i| (i as f32 * hz * std::f32::consts::TAU / 44_100.0).sin() * level).collect()
    }

    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opengarage-sfz-{}-{name}", std::process::id()));
        std::fs::create_dir_all(dir.join("Samples")).unwrap();
        wav::write_wav(&dir.join("Samples").join("la 440.wav"), &sine(440.0, 1.0, 0.5), 44_100, 1, 16).unwrap();
        wav::write_wav(&dir.join("Samples").join("coup.wav"), &sine(1000.0, 0.2, 0.5), 44_100, 1, 16).unwrap();
        dir
    }

    fn rendered(synth: &mut SfzSynth, frames: usize) -> Vec<f32> {
        let mut buf = vec![0.0f32; frames];
        synth.render(&mut buf);
        buf
    }

    #[test]
    fn key_names() {
        assert_eq!(parse_key("60"), Some(60));
        assert_eq!(parse_key("c4"), Some(60));
        assert_eq!(parse_key("A#3"), Some(58));
        assert_eq!(parse_key("db2"), Some(37));
        assert_eq!(parse_key("c-1"), Some(0));
        assert_eq!(parse_key("x"), None);
    }

    #[test]
    fn an_instrument_loads_and_plays_in_tune() {
        let dir = fixture("joue");
        std::fs::write(
            dir.join("commun.sfz"),
            "// shared settings\n<global> ampeg_release=0.05\n",
        )
        .unwrap();
        let file = dir.join("Essai.sfz");
        std::fs::write(
            &file,
            "#define $ROOT a4\n\
             <control> default_path=Samples/\n\
             #include \"commun.sfz\"\n\
             <group> lokey=0 hikey=96 loop_mode=loop_continuous loop_start=0 loop_end=44000\n\
             <region> sample=la 440.wav pitch_keycenter=$ROOT // a name with a space\n\
             <group> key=100 loop_mode=one_shot\n\
             <region> sample=coup.wav hivel=63 volume=-12\n\
             <region> sample=coup.wav lovel=64\n\
             <region> sample=absent.wav key=101\n\
             <region> sample=*sine key=102\n",
        )
        .unwrap();
        let instrument = Arc::new(SfzInstrument::load(&file).unwrap());
        assert_eq!(instrument.name, "Essai");
        assert_eq!(instrument.regions.len(), 3);
        assert_eq!(instrument.regions[0].root, 69);
        assert_eq!(instrument.regions[0].loop_points, Some((0, 44_000)));

        // An octave below the sample, on a 48 kHz engine: 220 Hz, looping past its length.
        let mut synth = SfzSynth::new(Arc::clone(&instrument), 48_000.0, 1.0);
        synth.note_on(57, 100);
        let buf = rendered(&mut synth, 48_000 * 3);
        let found = detect_pitch(&buf[120_000..124_096], 48_000.0).unwrap();
        assert!((found - 220.0).abs() < 1.5, "{found}");
        synth.note_off(57);
        rendered(&mut synth, 48_000);
        assert!(synth.is_silent());

        // Velocity layers: the soft one is 12 dB down. One-shots ignore note-off.
        let peak = |synth: &mut SfzSynth, velocity: u8| {
            synth.note_on(100, velocity);
            synth.note_off(100);
            let buf = rendered(synth, 4800);
            assert!(!synth.is_silent(), "a one-shot keeps playing after note-off");
            rendered(synth, 48_000);
            buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
        };
        let soft = peak(&mut synth, 63);
        let loud = peak(&mut synth, 127);
        assert!(loud > soft * 8.0, "{soft} {loud}");
        assert!(synth.is_silent());
        // Keys with no region are silent.
        synth.note_on(101, 100);
        assert!(synth.is_silent());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn broken_instruments_are_errors() {
        let dir = fixture("erreurs");
        let write = |name: &str, text: &str| {
            let file = dir.join(name);
            std::fs::write(&file, text).unwrap();
            file
        };
        assert!(SfzInstrument::load(&dir.join("absent.sfz")).is_err());
        assert!(SfzInstrument::load(&write("vide.sfz", "")).is_err());
        assert!(SfzInstrument::load(&write("sans-son.sfz", "<region> sample=rien.wav")).unwrap_err().contains("samples"));
        assert!(SfzInstrument::load(&write("boucle.sfz", "#include \"boucle.sfz\"\n<region> key=60")).is_err());
        // Odd but harmless input still loads.
        let odd = write("bizarre.sfz", "<region>sample=Samples\\coup.wav lokey=zz hikey=200 volume=abc <curve> v000=1 <region> key=c4 sample=Samples/coup.wav");
        assert_eq!(SfzInstrument::load(&odd).unwrap().regions.len(), 2);
        let _ = std::fs::remove_dir_all(dir);
    }
}
