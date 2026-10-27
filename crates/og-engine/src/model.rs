//! The project document. All times are in beats (quarter notes) unless noted.

use serde::{Deserialize, Serialize};

pub const FORMAT_VERSION: u32 = 2;

/// The tempos the program plays, in beats per minute: what is read from a file
/// or a site is kept within them.
pub const TEMPO_MIN: f64 = 20.0;
pub const TEMPO_MAX: f64 = 400.0;

/// A note inside a region; `start` is relative to the region start.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Note {
    pub start: f64,
    pub length: f64,
    pub pitch: u8,
    pub velocity: u8,
    /// String of a tablature the note was written on (0 is the highest),
    /// when it came from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub string: Option<u8>,
    /// How the note is played on its string, when a tablature says.
    #[serde(default, skip_serializing_if = "Articulation::is_plain")]
    pub articulation: Articulation,
}

/// A slide written on a note of a tablature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Slide {
    /// To the next note on the string, which is not struck again.
    Legato,
    /// To the next note on the string, struck again.
    Shift,
    /// Into the note from a few frets below.
    FromBelow,
    /// Into the note from a few frets above.
    FromAbove,
    /// Out of the note, down the neck.
    OutDown,
    /// Out of the note, up the neck.
    OutUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vibrato {
    Slight,
    Wide,
}

/// How a note is played on a string, beyond its pitch and length: what a
/// tablature writes around the fret number. Nothing is set for a plain note.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Articulation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub slide: Option<Slide>,
    /// A bend: points along the note (0 at its start, 1 at its end) and how
    /// many semitones up the pitch is there (down, for a tremolo bar dive).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bend: Vec<(f32, f32)>,
    /// The next note on the string is hammered on or pulled off, not struck.
    #[serde(default, skip_serializing_if = "is_false")]
    pub hammer: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vibrato: Option<Vibrato>,
    /// A harmonic: the fret the string is touched at. The pitch of the note
    /// is the one that sounds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub harmonic: Option<u8>,
    /// Muted by the left hand: a click with no pitch.
    #[serde(default, skip_serializing_if = "is_false")]
    pub dead: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub ghost: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub staccato: bool,
    /// 0, 1 (an accent) or 2 (a heavy one).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub accent: u8,
    #[serde(default, skip_serializing_if = "is_false")]
    pub let_ring: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub palm_mute: bool,
    /// Tremolo picking: the note is struck again and again, every so many
    /// beats (0.5 for eighth notes, 0.25 for sixteenths, 0.125 for thirty-seconds).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tremolo: Option<f32>,
    /// A trill: the note alternates with this other pitch, each lasting so
    /// many beats.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trill: Option<(u8, f32)>,
}

/// How the off-beat notes are played from a beat of a region on: straight,
/// or swung, the first of each pair of eighth notes (or of sixteenths)
/// taking this share of their time: 2/3 for a triplet feel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swing {
    /// Where the feel starts, in beats from the start of the region.
    pub beat: f64,
    /// The notes swung: 0.5 beat for eighth notes, 0.25 for sixteenths; 0 for straight.
    pub unit: f64,
    /// The share of a pair the first note takes.
    pub first: f64,
}

impl Swing {
    /// The triplet feel of Guitar Pro: eighth notes (`sixteenths` false) or
    /// sixteenths played as the first two thirds and the last third of a triplet.
    pub fn triplet(beat: f64, sixteenths: bool) -> Swing {
        Swing { beat, unit: if sixteenths { 0.25 } else { 0.5 }, first: 2.0 / 3.0 }
    }

    pub fn straight(beat: f64) -> Swing {
        Swing { beat, unit: 0.0, first: 0.5 }
    }

    pub fn is_straight(&self) -> bool {
        self.unit <= 0.0 || (self.first - 0.5).abs() < 1e-9
    }
}

/// Where a time written straight is played, in beats from the start of the
/// region, under the feels of `swing` (sorted by beat): within each pair of
/// swung notes, the first half is stretched to the share the feel gives it.
pub fn swung(swing: &[Swing], beat: f64) -> f64 {
    let Some(feel) = swing.iter().rev().find(|s| s.beat <= beat + 1e-9) else { return beat };
    if feel.is_straight() {
        return beat;
    }
    let pair = feel.unit * 2.0;
    let from = feel.beat + ((beat - feel.beat) / pair).floor() * pair;
    let into = beat - from;
    let first = feel.first.clamp(0.05, 0.95);
    from + if into <= feel.unit { into / feel.unit * first * pair } else { first * pair + (into - feel.unit) / feel.unit * (1.0 - first) * pair }
}

fn is_false(flag: &bool) -> bool {
    !*flag
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

impl Articulation {
    pub fn is_plain(&self) -> bool {
        *self == Articulation::default()
    }

    /// How far the bend goes, in semitones, at its highest.
    pub fn bend_peak(&self) -> f32 {
        self.bend.iter().map(|p| p.1).fold(0.0, f32::max)
    }
}

/// For each note, the next one played on the same string, when it is known
/// which string a note is on.
pub fn next_on_string(notes: &[Note]) -> Vec<Option<usize>> {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    order.sort_by(|a, b| notes[*a].start.total_cmp(&notes[*b].start));
    let mut next = vec![None; notes.len()];
    let mut last: std::collections::HashMap<u8, usize> = std::collections::HashMap::new();
    for i in order {
        let Some(string) = notes[i].string else { continue };
        if let Some(previous) = last.insert(string, i) {
            if notes[previous].start < notes[i].start - 1e-9 {
                next[previous] = Some(i);
            }
        }
    }
    next
}

/// A chord name written over a region, at a beat from its start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChordMark {
    pub beat: f64,
    pub name: String,
}

/// A point of an automation, tempo or transposition curve.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub beat: f64,
    pub value: f64,
}

/// Value of a curve drawn with straight lines between its points (sorted by beat).
pub fn curve_linear(points: &[Point], beat: f64, default: f64) -> f64 {
    let Some(first) = points.first() else { return default };
    if beat <= first.beat {
        return first.value;
    }
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        if beat < b.beat {
            let span = (b.beat - a.beat).max(1e-9);
            return a.value + (b.value - a.value) * (beat - a.beat) / span;
        }
    }
    points[points.len() - 1].value
}

/// Value of a curve that holds each point's value until the next point.
pub fn curve_step(points: &[Point], beat: f64, default: f64) -> f64 {
    points.iter().take_while(|p| p.beat <= beat).last().map_or(default, |p| p.value)
}

// ----- sounds ------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Wave {
    Sine,
    Saw,
    Square,
    Triangle,
}

/// Subtractive synthesiser settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SynthParams {
    pub wave: Wave,
    /// Detune of the second oscillator in cents; 0 turns it off.
    pub detune_cents: f32,
    /// Level of a sine one octave below.
    pub sub: f32,
    pub attack: f32,
    pub decay: f32,
    pub sustain: f32,
    pub release: f32,
    /// Filter cutoff in Hz with the envelope closed.
    pub cutoff: f32,
    /// Hz added to the cutoff when the envelope is fully open.
    pub cutoff_env: f32,
    /// Vibrato depth in semitones.
    pub vibrato: f32,
    pub gain: f32,
}

impl Default for SynthParams {
    fn default() -> Self {
        Self {
            wave: Wave::Saw,
            detune_cents: 8.0,
            sub: 0.0,
            attack: 0.01,
            decay: 0.3,
            sustain: 0.6,
            release: 0.2,
            cutoff: 1800.0,
            cutoff_env: 2000.0,
            vibrato: 0.0,
            gain: 0.4,
        }
    }
}

/// Plucked-string model settings.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluckParams {
    /// 0 dull to 1 bright.
    pub brightness: f32,
    /// 0 short to 1 long ring.
    pub sustain: f32,
    pub gain: f32,
}

impl Default for PluckParams {
    fn default() -> Self {
        Self { brightness: 0.6, sustain: 0.7, gain: 0.6 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrumKit {
    Acoustic,
    Electro,
    HipHop,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrumParams {
    pub kit: DrumKit,
    /// Semitones.
    pub tune: f32,
    /// Length multiplier of every hit.
    pub decay: f32,
    pub gain: f32,
}

impl Default for DrumParams {
    fn default() -> Self {
        Self { kit: DrumKit::Acoustic, tune: 0.0, decay: 1.0, gain: 0.8 }
    }
}

/// A preset of an imported SoundFont (sampled instruments).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SamplerParams {
    /// Id of a file in `Project::fonts`.
    pub font: u64,
    /// SoundFont bank; 128 is the percussion bank.
    pub bank: u16,
    pub preset: u8,
    pub gain: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Sound {
    Synth(SynthParams),
    Pluck(PluckParams),
    Drums(DrumParams),
    Sampler(SamplerParams),
}

impl Sound {
    /// Drum sounds are not transposed or arpeggiated: each key is an instrument.
    pub fn is_drums(&self) -> bool {
        match self {
            Sound::Drums(_) => true,
            Sound::Sampler(p) => p.bank == 128,
            _ => false,
        }
    }
}

// ----- effects -----------------------------------------------------------------

/// Three-band equaliser; gains in dB.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Eq {
    pub on: bool,
    pub low: f32,
    pub mid: f32,
    pub mid_freq: f32,
    pub high: f32,
    /// Where the low and the high bands turn, in hertz.
    pub low_freq: f32,
    pub high_freq: f32,
}

impl Default for Eq {
    fn default() -> Self {
        Self { on: false, low: 0.0, mid: 0.0, mid_freq: 1000.0, high: 0.0, low_freq: 120.0, high_freq: 5000.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AmpModel {
    Clean,
    Crunch,
    Lead,
    Bass,
}

/// Guitar or bass amplifier; every knob is 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Amp {
    pub on: bool,
    pub model: AmpModel,
    pub gain: f32,
    pub bass: f32,
    pub mid: f32,
    pub treble: f32,
    pub presence: f32,
    pub master: f32,
}

impl Default for Amp {
    fn default() -> Self {
        Self {
            on: false,
            model: AmpModel::Clean,
            gain: 0.4,
            bass: 0.5,
            mid: 0.5,
            treble: 0.5,
            presence: 0.5,
            master: 0.6,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PedalKind {
    Overdrive,
    Chorus,
    Tremolo,
    Phaser,
    Echo,
    /// A noise gate: silence below a threshold.
    Gate,
    /// A wah pedal, its position the first knob (which automation may move).
    Wah,
    /// A wah that the loudness of the playing moves.
    AutoWah,
    Flanger,
    /// An echo as a tape machine makes it: wobbling, saturating, darker at each pass.
    TapeEcho,
    /// An echo as a bucket-brigade chip makes it: darker still, a little gritty.
    BbdEcho,
}

impl PedalKind {
    pub const ALL: [PedalKind; 11] = [
        PedalKind::Overdrive,
        PedalKind::Gate,
        PedalKind::Wah,
        PedalKind::AutoWah,
        PedalKind::Chorus,
        PedalKind::Flanger,
        PedalKind::Tremolo,
        PedalKind::Phaser,
        PedalKind::Echo,
        PedalKind::TapeEcho,
        PedalKind::BbdEcho,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PedalKind::Overdrive => "Overdrive",
            PedalKind::Chorus => "Chorus",
            PedalKind::Tremolo => "Tremolo",
            PedalKind::Phaser => "Phaser",
            PedalKind::Echo => "Echo",
            PedalKind::Gate => "Noise gate",
            PedalKind::Wah => "Wah",
            PedalKind::AutoWah => "Auto-wah",
            PedalKind::Flanger => "Flanger",
            PedalKind::TapeEcho => "Tape echo",
            PedalKind::BbdEcho => "Analog echo",
        }
    }
}

/// A stompbox with two knobs, both 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pedal {
    pub kind: PedalKind,
    pub on: bool,
    pub amount: f32,
    pub rate: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TrackFx {
    pub eq: Eq,
    /// Compressor amount, 0 (off) to 1.
    pub comp: f32,
    /// Send levels to the shared echo and reverb, 0 to 1.
    pub echo: f32,
    pub reverb: f32,
    pub amp: Amp,
    pub pedals: Vec<Pedal>,
    /// A capture of real gear that takes the place of the amp's drive stage:
    /// the path of its file, absolute or relative to the project; empty for
    /// the built-in amp.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub capture: String,
    /// An impulse response that takes the place of the amp's loudspeaker
    /// filters: the path of its file; empty for the filters.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cabinet: String,
}

/// A setting of a track's effects that an automation curve can drive over time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FxParam {
    Reverb,
    Echo,
    Comp,
    EqLow,
    EqMid,
    EqHigh,
    AmpGain,
    /// The first knob of the pedal at this place in the chain (0 is the first).
    Pedal(u8),
}

impl FxParam {
    /// The settings every track has, in the order they are offered.
    pub const FIXED: [FxParam; 7] = [FxParam::Reverb, FxParam::Echo, FxParam::Comp, FxParam::EqLow, FxParam::EqMid, FxParam::EqHigh, FxParam::AmpGain];

    pub fn label(self) -> &'static str {
        match self {
            FxParam::Reverb => "Reverb",
            FxParam::Echo => "Echo",
            FxParam::Comp => "Compressor",
            FxParam::EqLow => "EQ low",
            FxParam::EqMid => "EQ mid",
            FxParam::EqHigh => "EQ high",
            FxParam::AmpGain => "Amp gain",
            FxParam::Pedal(_) => "Pedal",
        }
    }

    /// Lowest and highest value the setting takes.
    pub fn range(self) -> (f64, f64) {
        match self {
            FxParam::EqLow | FxParam::EqMid | FxParam::EqHigh => (-24.0, 24.0),
            _ => (0.0, 1.0),
        }
    }

    /// The setting's value in a set of effects.
    pub fn get(self, fx: &TrackFx) -> f64 {
        f64::from(match self {
            FxParam::Reverb => fx.reverb,
            FxParam::Echo => fx.echo,
            FxParam::Comp => fx.comp,
            FxParam::EqLow => fx.eq.low,
            FxParam::EqMid => fx.eq.mid,
            FxParam::EqHigh => fx.eq.high,
            FxParam::AmpGain => fx.amp.gain,
            FxParam::Pedal(i) => fx.pedals.get(usize::from(i)).map_or(0.0, |p| p.amount),
        })
    }

    /// Sets the setting in a set of effects. A band of the equaliser that is
    /// driven turns the equaliser on, so that the curve is heard.
    pub fn set(self, fx: &mut TrackFx, value: f64) {
        let (low, high) = self.range();
        let value = value.clamp(low, high) as f32;
        match self {
            FxParam::Reverb => fx.reverb = value,
            FxParam::Echo => fx.echo = value,
            FxParam::Comp => fx.comp = value,
            FxParam::EqLow => {
                fx.eq.low = value;
                fx.eq.on = true;
            }
            FxParam::EqMid => {
                fx.eq.mid = value;
                fx.eq.on = true;
            }
            FxParam::EqHigh => {
                fx.eq.high = value;
                fx.eq.on = true;
            }
            FxParam::AmpGain => fx.amp.gain = value,
            FxParam::Pedal(i) => {
                if let Some(pedal) = fx.pedals.get_mut(usize::from(i)) {
                    pedal.amount = value;
                }
            }
        }
    }
}

/// An automation curve of one setting of the effects of a track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FxCurve {
    pub param: FxParam,
    pub points: Vec<Point>,
}

/// Which reverb the shared one is, when no impulse of a room stands in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReverbKind {
    /// The built-in room: combs and allpasses.
    #[default]
    Room,
    /// A plate: denser, brighter.
    Plate,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MasterFx {
    pub eq: Eq,
    pub reverb_kind: ReverbKind,
    pub comp: f32,
    pub limiter: bool,
    /// Echo time in beats.
    pub echo_beats: f64,
    pub echo_feedback: f32,
    pub reverb_size: f32,
    pub reverb_damp: f32,
}

impl Default for MasterFx {
    fn default() -> Self {
        Self {
            eq: Eq::default(),
            comp: 0.0,
            limiter: true,
            echo_beats: 0.5,
            echo_feedback: 0.35,
            reverb_size: 0.6,
            reverb_damp: 0.5,
            reverb_kind: ReverbKind::Room,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArpMode {
    Up,
    Down,
    UpDown,
    Random,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Arp {
    pub on: bool,
    /// Step length in beats.
    pub rate: f64,
    pub mode: ArpMode,
    pub octaves: u8,
}

impl Default for Arp {
    fn default() -> Self {
        Self { on: false, rate: 0.25, mode: ArpMode::Up, octaves: 1 }
    }
}

// ----- drummer -----------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrumStyle {
    Rock,
    Pop,
    HipHop,
    Electro,
    Funk,
    Jazz,
    Latin,
}

impl DrumStyle {
    pub const ALL: [DrumStyle; 7] = [
        DrumStyle::Rock,
        DrumStyle::Pop,
        DrumStyle::HipHop,
        DrumStyle::Electro,
        DrumStyle::Funk,
        DrumStyle::Jazz,
        DrumStyle::Latin,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DrumStyle::Rock => "Rock",
            DrumStyle::Pop => "Pop",
            DrumStyle::HipHop => "Hip-hop",
            DrumStyle::Electro => "Electro",
            DrumStyle::Funk => "Funk",
            DrumStyle::Jazz => "Jazz",
            DrumStyle::Latin => "Latin",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HatMode {
    Off,
    Hats,
    Ride,
    Toms,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Perc {
    Off,
    Tambourine,
    Shaker,
    Clap,
}

/// Settings of the virtual drummer for one region.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DrummerParams {
    pub style: DrumStyle,
    pub complexity: f32,
    pub loudness: f32,
    pub swing: f32,
    pub fills: f32,
    pub kick_snare: bool,
    pub hats: HatMode,
    pub perc: Perc,
    pub seed: u32,
    /// The kick drum follows the notes of another track: that track's place
    /// in the project, counted from one; 0 to play the style's own pattern.
    pub follow: u16,
}

impl Default for DrummerParams {
    fn default() -> Self {
        Self {
            style: DrumStyle::Rock,
            complexity: 0.4,
            loudness: 0.7,
            swing: 0.0,
            fills: 0.4,
            kick_snare: true,
            hats: HatMode::Hats,
            perc: Perc::Off,
            seed: 1,
            follow: 0,
        }
    }
}

// ----- regions and tracks ------------------------------------------------------

/// Where a beat of the score falls in a recording of the song, in seconds
/// from the start of the recording.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timing {
    pub beat: f64,
    pub sec: f64,
}

/// A video of the song a video track can play: by its id, what the site
/// called it (empty when it said nothing), and where each beat falls in it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VideoChoice {
    pub id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub label: String,
    pub timing: Vec<Timing>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct AudioRef {
    /// Id of a clip in `Project::clips`.
    pub clip: u64,
    /// Seconds into the clip where the region starts playing.
    pub offset: f64,
    pub reverse: bool,
    /// Semitones; the duration is kept.
    pub transpose: i32,
    pub gain: f32,
    /// Pitch correction towards the nearest note, 0 (off) to 100 %.
    #[serde(default)]
    pub tune: u8,
}

impl AudioRef {
    pub fn new(clip: u64) -> Self {
        Self { clip, offset: 0.0, reverse: false, transpose: 0, gain: 1.0, tune: 0 }
    }

    /// Key of the audio to play: the clip itself, or a processed copy of it
    /// when transposition or pitch correction is on.
    pub fn key(&self) -> u64 {
        if self.transpose == 0 && self.tune == 0 {
            self.clip
        } else {
            let transpose = (self.transpose.clamp(-48, 48) + 64) as u64;
            (1 << 63) | (u64::from(self.tune.min(100)) << 48) | (transpose << 40) | (self.clip & 0xFF_FFFF_FFFF)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Content {
    Midi(Vec<Note>),
    Audio(AudioRef),
    Drummer(DrummerParams),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Region {
    pub name: String,
    pub start: f64,
    pub length: f64,
    /// When set, the first `loop_len` beats repeat until the end of the region.
    #[serde(default)]
    pub loop_len: Option<f64>,
    pub content: Content,
    /// Other takes recorded over the same span; `content` is the one heard.
    #[serde(default)]
    pub takes: Vec<Content>,
    /// Chord names written over the region, as a tablature gives them.
    #[serde(default)]
    pub chords: Vec<ChordMark>,
    /// The feels of the region, by beat: its notes are written straight and
    /// played swung where a feel says so.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub swing: Vec<Swing>,
}

impl Region {
    pub fn new(name: impl Into<String>, start: f64, length: f64) -> Self {
        Self { name: name.into(), start, length, loop_len: None, content: Content::Midi(Vec::new()), takes: Vec::new(), chords: Vec::new(), swing: Vec::new() }
    }

    pub fn end(&self) -> f64 {
        self.start + self.length
    }

    /// Length of one repetition.
    pub fn cycle_len(&self) -> f64 {
        self.loop_len.unwrap_or(self.length).max(0.0625)
    }

    pub fn notes(&self) -> &[Note] {
        match &self.content {
            Content::Midi(notes) => notes,
            _ => &[],
        }
    }

    pub fn notes_mut(&mut self) -> Option<&mut Vec<Note>> {
        match &mut self.content {
            Content::Midi(notes) => Some(notes),
            _ => None,
        }
    }

    pub fn is_midi(&self) -> bool {
        matches!(self.content, Content::Midi(_))
    }
}

/// An external plug-in (CLAP) placed on a track.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginSlot {
    /// Identifies the running instance; unique in the project.
    pub id: u64,
    /// Path of the plug-in file.
    pub file: String,
    /// Identifier of the plug-in inside the file.
    pub plugin: String,
    pub name: String,
    pub on: bool,
    /// Parameter values set from the host's sliders since `state` was taken: (id, value).
    #[serde(default)]
    pub params: Vec<(u32, f64)>,
    /// Everything the plug-in asked to have remembered, as hexadecimal text.
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackKind {
    Instrument,
    Audio,
    Guitar,
    Drummer,
    /// A video of the song, played by YouTube's own player in step with the
    /// score: it holds no sound of the program's.
    Video,
}

impl TrackKind {
    pub fn label(self) -> &'static str {
        match self {
            TrackKind::Instrument => "Software instrument",
            TrackKind::Audio => "Audio",
            TrackKind::Guitar => "Guitar or bass",
            TrackKind::Drummer => "Drummer",
            TrackKind::Video => "Video",
        }
    }

    /// Whether regions of this track hold recorded sound rather than notes.
    pub fn is_audio(self) -> bool {
        matches!(self, TrackKind::Audio | TrackKind::Guitar)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    pub name: String,
    pub kind: TrackKind,
    /// Name of the patch the sound came from, shown in the header.
    pub patch: String,
    pub sound: Sound,
    /// Linear gain, 1.0 is unity.
    pub volume: f32,
    /// -1.0 (left) to 1.0 (right).
    pub pan: f32,
    pub mute: bool,
    pub solo: bool,
    pub locked: bool,
    /// Audio tracks: hear the input through the track's effects.
    pub monitor: bool,
    pub color: u8,
    pub icon: u8,
    pub fx: TrackFx,
    /// External effects, applied in order after the built-in ones.
    pub plugins: Vec<PluginSlot>,
    /// External instrument that plays the notes instead of the built-in sound.
    pub instrument: Option<PluginSlot>,
    pub arp: Arp,
    /// Volume (linear gain) and pan automation; empty means "use the fader".
    pub vol_auto: Vec<Point>,
    pub pan_auto: Vec<Point>,
    /// Automation of the effects: one curve per setting that is driven.
    pub fx_auto: Vec<FxCurve>,
    /// Open strings of the instrument, highest first, for the tablature;
    /// empty for the standard tuning of a guitar or a bass.
    pub tuning: Vec<u8>,
    pub regions: Vec<Region>,
    /// Name of the audio output this track plays on; empty for the main mix.
    pub output: String,
    /// Audio tracks: name of the audio input recorded and listened to; empty
    /// for the input chosen for the whole program.
    pub input: String,
    /// Audio tracks: which channel of the input is recorded (1 is the first);
    /// 0 for the channel chosen for the whole program.
    pub input_channel: u16,
    /// A recording of the song (an audio file, or a video): where in it each
    /// beat of the score falls. Empty for the other tracks.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub timing: Vec<Timing>,
    /// Video tracks: the YouTube video, by its id.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub video: String,
    /// Video tracks: the videos of the song the site gave, the one played
    /// among them, for another to be chosen.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub videos: Vec<VideoChoice>,
    /// The recording that leads when the song is played with its original
    /// sound; one at most.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub lead: bool,
    /// Heard along with the recording when the song is played with its
    /// original sound: the other tracks are silent then.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub with_recording: bool,
}

impl Default for Track {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: TrackKind::Instrument,
            patch: String::new(),
            sound: Sound::Synth(SynthParams::default()),
            volume: 1.0,
            pan: 0.0,
            mute: false,
            solo: false,
            locked: false,
            monitor: false,
            color: 0,
            icon: 0,
            fx: TrackFx::default(),
            plugins: Vec::new(),
            instrument: None,
            arp: Arp::default(),
            vol_auto: Vec::new(),
            pan_auto: Vec::new(),
            fx_auto: Vec::new(),
            tuning: Vec::new(),
            regions: Vec::new(),
            output: String::new(),
            input: String::new(),
            input_channel: 0,
            timing: Vec::new(),
            video: String::new(),
            lead: false,
            with_recording: false,
            videos: Vec::new(),
        }
    }
}

impl Track {
    pub fn new(name: impl Into<String>, kind: TrackKind) -> Self {
        Self { name: name.into(), kind, ..Self::default() }
    }

    /// Whether any curve drives the effects.
    pub fn fx_automated(&self) -> bool {
        self.fx_auto.iter().any(|c| !c.points.is_empty())
    }

    /// The effects as their knobs and the automation curves set them at `beat`.
    pub fn fx_at(&self, beat: f64) -> TrackFx {
        let mut fx = TrackFx::default();
        self.fx_into(beat, &mut fx);
        fx
    }

    /// As `fx_at`, written into effects that exist already: the audio thread
    /// keeps one set per track, so that nothing is allocated while it plays.
    pub fn fx_into(&self, beat: f64, out: &mut TrackFx) {
        // Named one by one, so that a setting added later cannot be forgotten.
        let TrackFx { eq, comp, echo, reverb, amp, pedals, capture, cabinet } = &self.fx;
        out.eq = *eq;
        out.comp = *comp;
        out.echo = *echo;
        out.reverb = *reverb;
        out.amp.clone_from(amp);
        out.pedals.clear();
        out.pedals.extend_from_slice(pedals);
        if out.capture != *capture {
            out.capture.clone_from(capture);
        }
        if out.cabinet != *cabinet {
            out.cabinet.clone_from(cabinet);
        }
        for curve in self.fx_auto.iter().filter(|c| !c.points.is_empty()) {
            curve.param.set(out, curve_linear(&curve.points, beat, curve.param.get(&self.fx)));
        }
    }

    /// The curve of one setting; empty when there is none.
    pub fn fx_curve(&self, param: FxParam) -> &[Point] {
        self.fx_auto.iter().find(|c| c.param == param).map_or(&[], |c| c.points.as_slice())
    }

    /// The curve of one setting, made if there is none yet.
    pub fn fx_curve_mut(&mut self, param: FxParam) -> &mut Vec<Point> {
        let at = match self.fx_auto.iter().position(|c| c.param == param) {
            Some(at) => at,
            None => {
                self.fx_auto.push(FxCurve { param, points: Vec::new() });
                self.fx_auto.len() - 1
            }
        };
        &mut self.fx_auto[at].points
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    pub name: String,
    pub start: f64,
    pub length: f64,
}

/// An external file used by the project: audio, SoundFont or plug-in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileRef {
    pub id: u64,
    pub name: String,
    pub path: String,
}

/// An audio file used by the project. The samples live outside the document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub name: String,
    /// Path of the WAV file, absolute or relative to the project file.
    pub path: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Project {
    pub version: u32,
    pub name: String,
    /// The artist and the title of the song, when known (an import gives them).
    pub artist: String,
    pub title: String,
    /// What names the project among the sessions the program keeps: the same
    /// from one save to the next. Empty until the program gives it one.
    pub id: String,
    /// Beats per minute, used where the tempo track has no point.
    pub tempo: f64,
    /// Time signature: `beats_per_bar` over `beat_unit` (4 or 8).
    pub beats_per_bar: u32,
    pub beat_unit: u32,
    /// Key: 0 = C … 11 = B.
    pub key: u8,
    pub minor: bool,
    pub master_volume: f32,
    pub metronome: bool,
    pub count_in: bool,
    pub cycle_enabled: bool,
    pub cycle_start: f64,
    pub cycle_end: f64,
    /// End of the project; when unset it follows the last region.
    pub end: Option<f64>,
    pub tempo_points: Vec<Point>,
    /// Transposition in semitones over time.
    pub transpose_points: Vec<Point>,
    pub markers: Vec<Marker>,
    pub master_fx: MasterFx,
    pub master_auto: Vec<Point>,
    /// Free text kept with the project.
    pub notes: String,
    pub clips: Vec<Clip>,
    /// SoundFont files whose presets the tracks use.
    pub fonts: Vec<FileRef>,
    /// An impulse response of a room that takes the place of the built-in
    /// reverb: the path of its file; empty for the built-in one.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub reverb_impulse: String,
    pub tracks: Vec<Track>,
    /// The song is played with its original sound: in step with the recording
    /// that leads, at its pace, the other tracks silent unless asked for.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub follow: bool,
    /// The words of the song.
    #[serde(skip_serializing_if = "crate::lyrics::Lyrics::is_empty")]
    pub lyrics: crate::lyrics::Lyrics,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            name: "Untitled".to_owned(),
            artist: String::new(),
            title: String::new(),
            id: String::new(),
            tempo: 120.0,
            beats_per_bar: 4,
            beat_unit: 4,
            key: 0,
            minor: false,
            master_volume: 0.8,
            metronome: false,
            count_in: true,
            cycle_enabled: false,
            cycle_start: 0.0,
            cycle_end: 16.0,
            end: None,
            tempo_points: Vec::new(),
            transpose_points: Vec::new(),
            markers: Vec::new(),
            master_fx: MasterFx::default(),
            master_auto: Vec::new(),
            notes: String::new(),
            clips: Vec::new(),
            fonts: Vec::new(),
            reverb_impulse: String::new(),
            tracks: Vec::new(),
            follow: false,
            lyrics: crate::lyrics::Lyrics::default(),
        }
    }
}

pub const KEY_NAMES: [&str; 12] =
    ["C", "C♯", "D", "E♭", "E", "F", "F♯", "G", "A♭", "A", "B♭", "B"];

impl Project {
    pub fn empty() -> Self {
        Self::default()
    }

    /// Length of a bar in beats (quarter notes).
    pub fn bar_len(&self) -> f64 {
        f64::from(self.beats_per_bar.max(1)) * 4.0 / f64::from(self.beat_unit.clamp(1, 16))
    }

    /// Where playback stops.
    pub fn end_beat(&self) -> f64 {
        let bar = self.bar_len();
        if let Some(end) = self.end {
            return end.max(bar);
        }
        let last = self
            .tracks
            .iter()
            .flat_map(|t| t.regions.iter())
            .map(Region::end)
            .fold(0.0, f64::max);
        ((last / bar).ceil() * bar).max(bar)
    }

    pub fn tempo_at(&self, beat: f64) -> f64 {
        curve_step(&self.tempo_points, beat, self.tempo).clamp(TEMPO_MIN, TEMPO_MAX)
    }

    pub fn transpose_at(&self, beat: f64) -> i32 {
        curve_step(&self.transpose_points, beat, 0.0).round().clamp(-24.0, 24.0) as i32
    }

    /// Seconds from the start of the project to `beat`, following the tempo track.
    pub fn beat_to_sec(&self, beat: f64) -> f64 {
        let mut sec = 0.0;
        let mut at = 0.0;
        let mut tempo = self.tempo.clamp(TEMPO_MIN, TEMPO_MAX);
        for p in &self.tempo_points {
            if p.beat >= beat {
                break;
            }
            if p.beat > at {
                sec += (p.beat - at) * 60.0 / tempo;
                at = p.beat;
            }
            tempo = p.value.clamp(TEMPO_MIN, TEMPO_MAX);
        }
        sec + (beat - at) * 60.0 / tempo
    }

    /// The notes of a drummer region that starts at `start` and whose pattern
    /// is `cycle` beats long. When the drummer follows another track, its kick
    /// drum plays where that track's notes begin.
    pub fn drummer_notes(&self, params: &DrummerParams, start: f64, cycle: f64) -> Vec<Note> {
        let followed = usize::from(params.follow).checked_sub(1).and_then(|index| self.tracks.get(index));
        let Some(track) = followed.filter(|t| t.kind == TrackKind::Instrument) else {
            return crate::drummer::generate(params, cycle, self.bar_len());
        };
        let mut onsets: Vec<f64> = Vec::new();
        for region in &track.regions {
            let Content::Midi(notes) = &region.content else { continue };
            let length = region.cycle_len().min(region.length.max(0.0625));
            let repeats = ((region.length / length).ceil() as usize).clamp(1, 4096);
            for k in 0..repeats {
                let base = region.start + k as f64 * length;
                if base >= start + cycle {
                    break;
                }
                for note in notes {
                    let on = base + note.start;
                    if note.start >= 0.0 && note.start < length && on < region.end() && on >= start && on < start + cycle {
                        onsets.push(on - start);
                    }
                }
            }
        }
        crate::drummer::generate_following(params, cycle, self.bar_len(), Some(&onsets))
    }

    pub fn next_clip_id(&self) -> u64 {
        self.clips.iter().map(|c| c.id).max().unwrap_or(0) + 1
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curves() {
        let pts = [Point { beat: 4.0, value: 1.0 }, Point { beat: 8.0, value: 3.0 }];
        assert_eq!(curve_linear(&[], 2.0, 9.0), 9.0);
        assert_eq!(curve_linear(&pts, 0.0, 9.0), 1.0);
        assert_eq!(curve_linear(&pts, 6.0, 9.0), 2.0);
        assert_eq!(curve_linear(&pts, 20.0, 9.0), 3.0);
        assert_eq!(curve_step(&pts, 2.0, 9.0), 9.0);
        assert_eq!(curve_step(&pts, 6.0, 9.0), 1.0);
        assert_eq!(curve_step(&pts, 8.0, 9.0), 3.0);
    }

    #[test]
    fn tempo_track_changes_the_time_of_a_beat() {
        let mut project = Project::empty();
        project.tempo = 120.0;
        assert_eq!(project.beat_to_sec(4.0), 2.0);
        project.tempo_points.push(Point { beat: 4.0, value: 60.0 });
        assert_eq!(project.beat_to_sec(4.0), 2.0);
        assert_eq!(project.beat_to_sec(6.0), 4.0);
        assert_eq!(project.tempo_at(5.0), 60.0);
    }

    #[test]
    fn end_follows_regions_unless_set() {
        let mut project = Project::empty();
        project.tracks.push(Track::new("t", TrackKind::Instrument));
        assert_eq!(project.end_beat(), 4.0);
        project.tracks[0].regions.push(Region::new("r", 2.0, 3.5));
        assert_eq!(project.end_beat(), 8.0);
        project.end = Some(32.0);
        assert_eq!(project.end_beat(), 32.0);
        project.beats_per_bar = 6;
        project.beat_unit = 8;
        assert_eq!(project.bar_len(), 3.0);
    }

    #[test]
    fn old_files_with_missing_fields_still_load() {
        let project = Project::from_json(r#"{"name":"x","tempo":90.0}"#).unwrap();
        assert_eq!(project.tempo, 90.0);
        assert_eq!(project.beats_per_bar, 4);
    }
}
