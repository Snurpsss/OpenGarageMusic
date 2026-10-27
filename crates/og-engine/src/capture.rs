//! Captures of real gear: neural networks trained on an amplifier, a pedal
//! or a whole rig, read from the files of Neural Amp Modeler (`.nam`, run by
//! the `nam-rs` crate) and of the LSTM trainers behind Proteus and AIDA-X
//! (`.json`, run here), and run on the sound of a track at the rate the
//! capture was made at.
//!
//! The program ships no capture: the user brings their own, and a project
//! refers to a capture by the path of its file.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use serde_json::Value;

use crate::convolution::Impulse;
use crate::resample::Resampler;

/// File extensions a capture may have.
pub const EXTENSIONS: [&str; 2] = ["nam", "json"];
/// Most units in a layer of an LSTM capture that is run.
const MOST_HIDDEN: usize = 256;

/// What is known of a capture, from its file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct About {
    pub name: String,
    /// Who made the capture.
    pub maker: String,
    /// What was captured, as the file says: "amp", "pedal", "amp_cab"…
    pub gear: String,
    /// Whether the loudspeaker cabinet is part of the capture, when the
    /// file says.
    pub has_cab: Option<bool>,
    /// The rate the capture was made at, in hertz.
    pub sample_rate: f64,
    /// Which trainer the file comes from.
    pub format: &'static str,
}

/// A capture as read from its file, shared by the tracks that use it.
#[derive(Debug)]
pub struct Capture {
    pub about: About,
    file: File,
}

#[derive(Debug)]
enum File {
    Nam(Box<nam_rs::NamModel>),
    Lstm(Arc<Lstm>),
}

/// The files loaded, by the path the project writes.
#[derive(Clone, Default)]
pub struct Captures {
    pub gear: HashMap<String, Arc<Capture>>,
    pub impulses: HashMap<String, Arc<Impulse>>,
}

impl Capture {
    pub fn load(path: &Path) -> Result<Capture, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let name = path.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        Self::parse(&text, &name)
    }

    /// Reads a capture from the text of its file; `name` stands in when the
    /// file gives none.
    pub fn parse(text: &str, name: &str) -> Result<Capture, String> {
        let value: Value = serde_json::from_str(text).map_err(|e| format!("not a capture file ({e})"))?;
        if value.get("architecture").is_some() {
            let model = nam_rs::NamModel::from_json_str(text).map_err(|e| e.to_string())?;
            let meta = model.metadata_typed();
            let about = About {
                name: meta.name.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| name.to_owned()),
                maker: meta.modeled_by.clone().unwrap_or_default(),
                gear: meta.gear_type.clone().unwrap_or_default(),
                has_cab: meta.includes_cab(),
                sample_rate: model.expected_sample_rate(),
                format: "NAM",
            };
            // Only a model that can be built is kept: the fault is told now, not on stage.
            nam_rs::Model::from_nam(&model).map_err(|e| e.to_string())?;
            return Ok(Capture { about, file: File::Nam(Box::new(model)) });
        }
        let (lstm, format) = if value.get("model_data").is_some() {
            (Lstm::from_proteus(&value)?, "Proteus")
        } else if value.get("layers").is_some() {
            (Lstm::from_rtneural(&value)?, "AIDA-X")
        } else {
            return Err("neither a NAM capture nor an LSTM one".to_owned());
        };
        let text_of = |key: &str| value.get(key).and_then(Value::as_str).unwrap_or_default().trim().to_owned();
        let rate = value.get("samplerate").and_then(|v| v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))).unwrap_or(48_000.0);
        let about = About {
            name: Some(text_of("name")).filter(|n| !n.is_empty()).unwrap_or_else(|| name.to_owned()),
            maker: text_of("author"),
            gear: text_of("style"),
            has_cab: None,
            sample_rate: rate,
            format,
        };
        Ok(Capture { about, file: File::Lstm(Arc::new(lstm)) })
    }

    /// Starts the capture on a stream at `sample_rate`, given `block`
    /// samples at most at a time.
    pub fn start(&self, sample_rate: f32, block: usize) -> Result<CaptureRt, String> {
        let run = match &self.file {
            File::Nam(model) => Runner::Nam(nam_rs::Model::from_nam(model).map_err(|e| e.to_string())?),
            File::Lstm(lstm) => Runner::Lstm(LstmRt::new(Arc::clone(lstm))),
        };
        let own = self.about.sample_rate;
        let same = (own - f64::from(sample_rate)).abs() < 1.0;
        let mut rt = CaptureRt { run, up: None, down: None, mid: Vec::new(), out: Vec::new() };
        if !same {
            let most = block * 2 + 64;
            rt.up = Some(Resampler::new(f64::from(sample_rate), own, block));
            rt.down = Some(Resampler::new(own, f64::from(sample_rate), most));
            rt.mid = Vec::with_capacity(most);
            rt.out = Vec::with_capacity(most * 2 + PRIME);
            // A little sound in hand, so that the two changes of rate, which
            // give a sample more or less each time, never leave a block short.
            rt.out.resize(PRIME, 0.0);
        }
        Ok(rt)
    }
}

/// Samples kept ahead when the rates differ.
const PRIME: usize = 64;

/// A capture running on one stream of sound.
pub struct CaptureRt {
    run: Runner,
    up: Option<Resampler>,
    down: Option<Resampler>,
    mid: Vec<f32>,
    out: Vec<f32>,
}

enum Runner {
    Nam(nam_rs::Model),
    Lstm(LstmRt),
}

impl Runner {
    fn process(&mut self, buf: &mut [f32]) {
        match self {
            Runner::Nam(model) => model.process_buffer(buf),
            Runner::Lstm(lstm) => {
                for s in buf {
                    *s = lstm.sample(*s);
                }
            }
        }
    }
}

impl CaptureRt {
    /// Runs the capture on a block, in place. Allocates nothing.
    pub fn process(&mut self, buf: &mut [f32]) {
        let (Some(up), Some(down)) = (&mut self.up, &mut self.down) else {
            self.run.process(buf);
            return;
        };
        self.mid.clear();
        up.run(buf, &mut self.mid);
        self.run.process(&mut self.mid);
        down.run(&self.mid, &mut self.out);
        let have = self.out.len().min(buf.len());
        buf[..have].copy_from_slice(&self.out[..have]);
        buf[have..].fill(0.0);
        self.out.drain(..have);
        for s in buf.iter_mut() {
            if !s.is_finite() {
                *s = 0.0;
            }
        }
    }
}

// ----- the LSTM captures ---------------------------------------------------------

/// The weights of an LSTM capture, in one layout whatever the trainer: one
/// layer of `hidden` units fed the sample and `input - 1` settings, then
/// one output.
#[derive(Debug)]
pub struct Lstm {
    input: usize,
    hidden: usize,
    /// Gates i, f, g, o in turn, `hidden` rows each of `input` weights.
    w_in: Vec<f32>,
    /// The same rows, of `hidden` weights, for the state.
    w_state: Vec<f32>,
    bias: Vec<f32>,
    out_w: Vec<f32>,
    out_b: f32,
    /// The input is added to the output.
    skip: bool,
}

fn floats(value: &Value) -> Option<Vec<f32>> {
    value.as_array()?.iter().map(|v| v.as_f64().map(|f| f as f32)).collect()
}

/// A matrix as rows of numbers: the rows one after the other, and their count.
fn matrix(value: &Value) -> Option<(Vec<f32>, usize, usize)> {
    let rows = value.as_array()?;
    let mut out = Vec::new();
    let mut width = 0;
    for row in rows {
        let row = floats(row)?;
        if width == 0 {
            width = row.len();
        } else if row.len() != width {
            return None;
        }
        out.extend(row);
    }
    Some((out, rows.len(), width))
}

/// The matrix with its rows and columns swapped.
fn transposed(data: &[f32], rows: usize, cols: usize) -> Vec<f32> {
    let mut out = vec![0.0; data.len()];
    for r in 0..rows {
        for c in 0..cols {
            out[c * rows + r] = data[r * cols + c];
        }
    }
    out
}

impl Lstm {
    /// The format of Proteus and of the trainer behind it: PyTorch's weights
    /// as they are, with `model_data` describing the network.
    fn from_proteus(value: &Value) -> Result<Lstm, String> {
        let data = value.get("model_data").ok_or("no model_data")?;
        let number = |key: &str| data.get(key).and_then(Value::as_u64).map(|n| n as usize);
        if data.get("unit_type").and_then(Value::as_str) != Some("LSTM") || number("num_layers").unwrap_or(1) != 1 {
            return Err("only captures of one LSTM layer are read".to_owned());
        }
        let hidden = number("hidden_size").ok_or("no hidden_size")?;
        let input = number("input_size").unwrap_or(1);
        if hidden == 0 || hidden > MOST_HIDDEN || !(1..=3).contains(&input) {
            return Err("a size of the network is out of reach".to_owned());
        }
        let skip = number("skip").unwrap_or(0) > 0;
        let weights = value.get("state_dict").ok_or("no state_dict")?;
        let get = |key: &str| weights.get(key).ok_or_else(|| format!("no {key}"));
        let (w_in, rows, cols) = matrix(get("rec.weight_ih_l0")?).ok_or("unreadable rec.weight_ih_l0")?;
        if rows != 4 * hidden || cols != input {
            return Err("the input weights do not fit the network".to_owned());
        }
        let (w_state, rows, cols) = matrix(get("rec.weight_hh_l0")?).ok_or("unreadable rec.weight_hh_l0")?;
        if rows != 4 * hidden || cols != hidden {
            return Err("the state weights do not fit the network".to_owned());
        }
        let b_in = floats(get("rec.bias_ih_l0")?).ok_or("unreadable rec.bias_ih_l0")?;
        let b_state = floats(get("rec.bias_hh_l0")?).ok_or("unreadable rec.bias_hh_l0")?;
        if b_in.len() != 4 * hidden || b_state.len() != 4 * hidden {
            return Err("the biases do not fit the network".to_owned());
        }
        let bias = b_in.iter().zip(&b_state).map(|(a, b)| a + b).collect();
        let (out_w, rows, cols) = matrix(get("lin.weight")?).ok_or("unreadable lin.weight")?;
        if rows != 1 || cols != hidden {
            return Err("the output weights do not fit the network".to_owned());
        }
        let out_b = floats(get("lin.bias")?).and_then(|b| b.first().copied()).ok_or("unreadable lin.bias")?;
        Ok(Lstm { input, hidden, w_in, w_state, bias, out_w, out_b, skip })
    }

    /// The format of RTNeural, which AIDA-X uses: Keras's layout, the
    /// weights of a layer as columns.
    fn from_rtneural(value: &Value) -> Result<Lstm, String> {
        let layers = value.get("layers").and_then(Value::as_array).ok_or("no layers")?;
        let kind = |layer: &Value| layer.get("type").and_then(Value::as_str).unwrap_or_default().to_owned();
        let [lstm, dense] = layers.as_slice() else { return Err("only captures of one LSTM layer and one output are read".to_owned()) };
        if kind(lstm) != "lstm" || kind(dense) != "dense" {
            return Err("only captures of one LSTM layer and one output are read".to_owned());
        }
        let input = value.get("in_shape").and_then(Value::as_array).and_then(|s| s.last()).and_then(Value::as_u64).unwrap_or(1) as usize;
        let skip = value.get("in_skip").and_then(Value::as_u64).unwrap_or(0) > 0;
        let weights = lstm.get("weights").and_then(Value::as_array).ok_or("no weights")?;
        let [kernel, recurrent, bias] = weights.as_slice() else { return Err("the LSTM layer has not three sets of weights".to_owned()) };
        let (kernel, rows, cols) = matrix(kernel).ok_or("unreadable kernel")?;
        let hidden = cols / 4;
        if hidden == 0 || hidden > MOST_HIDDEN || cols != 4 * hidden || rows != input || !(1..=3).contains(&input) {
            return Err("the input weights do not fit the network".to_owned());
        }
        let (recurrent, rows, cols) = matrix(recurrent).ok_or("unreadable recurrent kernel")?;
        if rows != hidden || cols != 4 * hidden {
            return Err("the state weights do not fit the network".to_owned());
        }
        let bias = floats(bias).ok_or("unreadable bias")?;
        if bias.len() != 4 * hidden {
            return Err("the biases do not fit the network".to_owned());
        }
        let out = dense.get("weights").and_then(Value::as_array).ok_or("no output weights")?;
        let [out_w, out_b] = out.as_slice() else { return Err("the output has not two sets of weights".to_owned()) };
        let (out_w, rows, cols) = matrix(out_w).ok_or("unreadable output weights")?;
        if rows != hidden || cols != 1 {
            return Err("the output weights do not fit the network".to_owned());
        }
        let out_b = floats(out_b).and_then(|b| b.first().copied()).ok_or("unreadable output bias")?;
        Ok(Lstm {
            input,
            hidden,
            w_in: transposed(&kernel, input, 4 * hidden),
            w_state: transposed(&recurrent, hidden, 4 * hidden),
            bias,
            out_w,
            out_b,
            skip,
        })
    }
}

/// An LSTM capture running: its state.
struct LstmRt {
    net: Arc<Lstm>,
    h: Vec<f32>,
    c: Vec<f32>,
    gates: Vec<f32>,
    /// The settings a conditioned capture takes with the sound, 0 to 1.
    settings: [f32; 2],
}

#[inline]
fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

impl LstmRt {
    fn new(net: Arc<Lstm>) -> Self {
        let hidden = net.hidden;
        Self { net, h: vec![0.0; hidden], c: vec![0.0; hidden], gates: vec![0.0; 4 * hidden], settings: [0.5; 2] }
    }

    #[inline]
    fn sample(&mut self, x: f32) -> f32 {
        let net = &self.net;
        let (hidden, input) = (net.hidden, net.input);
        for (g, gate) in self.gates.iter_mut().enumerate() {
            let w_in = &net.w_in[g * input..(g + 1) * input];
            let mut acc = net.bias[g] + w_in[0] * x;
            for (w, setting) in w_in[1..].iter().zip(&self.settings) {
                acc += w * setting;
            }
            let w_state = &net.w_state[g * hidden..(g + 1) * hidden];
            acc += w_state.iter().zip(&self.h).map(|(w, h)| w * h).sum::<f32>();
            *gate = acc;
        }
        let mut y = net.out_b;
        for k in 0..hidden {
            let i = sigmoid(self.gates[k]);
            let f = sigmoid(self.gates[hidden + k]);
            let g = self.gates[2 * hidden + k].tanh();
            let o = sigmoid(self.gates[3 * hidden + k]);
            self.c[k] = f * self.c[k] + i * g;
            self.h[k] = o * self.c[k].tanh();
            y += net.out_w[k] * self.h[k];
        }
        if net.skip {
            y += x;
        }
        if y.is_finite() {
            y
        } else {
            self.h.fill(0.0);
            self.c.fill(0.0);
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Proteus file of `hidden` units, every weight zero but the output bias.
    fn proteus(hidden: usize, skip: u64, out_b: f32) -> String {
        let row = |n: usize| format!("[{}]", vec!["0"; n].join(","));
        let rows = |count: usize, n: usize| format!("[{}]", vec![row(n); count].join(","));
        format!(
            r#"{{"model_data": {{"model": "SimpleRNN", "input_size": 1, "skip": {skip}, "output_size": 1, "unit_type": "LSTM", "num_layers": 1, "hidden_size": {hidden}, "bias_fl": true}},
            "state_dict": {{"rec.weight_ih_l0": {}, "rec.weight_hh_l0": {}, "rec.bias_ih_l0": {}, "rec.bias_hh_l0": {}, "lin.weight": {}, "lin.bias": [{out_b}]}}}}"#,
            rows(4 * hidden, 1),
            rows(4 * hidden, hidden),
            row(4 * hidden),
            row(4 * hidden),
            rows(1, hidden)
        )
    }

    #[test]
    fn an_lstm_capture_with_no_weights_passes_the_sound_or_holds_still() {
        // With the skip, the sound comes out as it went in.
        let capture = Capture::parse(&proteus(3, 1, 0.0), "test").unwrap();
        assert_eq!((capture.about.format, capture.about.sample_rate, capture.about.name.as_str()), ("Proteus", 48_000.0, "test"));
        let mut rt = capture.start(48_000.0, 64).unwrap();
        let mut buf: Vec<f32> = (0..64).map(|i| (i as f32 * 0.1).sin()).collect();
        let wanted = buf.clone();
        rt.process(&mut buf);
        assert!(buf.iter().zip(&wanted).all(|(a, b)| (a - b).abs() < 1e-6));
        // Without it, only the output bias is heard.
        let capture = Capture::parse(&proteus(2, 0, 0.25), "test").unwrap();
        let mut rt = capture.start(48_000.0, 64).unwrap();
        rt.process(&mut buf);
        assert!(buf.iter().all(|s| (s - 0.25).abs() < 1e-6));
        // At another rate, the sound goes through the two changes of rate and
        // comes back whole, a little late.
        let capture = Capture::parse(&proteus(2, 1, 0.0), "test").unwrap();
        let mut rt = capture.start(44_100.0, 64).unwrap();
        let mut heard = Vec::new();
        for block in 0..40 {
            let mut buf: Vec<f32> = (0..64).map(|i| ((block * 64 + i) as f32 * 0.05).sin()).collect();
            rt.process(&mut buf);
            heard.extend(buf);
        }
        let late = (0..200).find(|d| (300..2000).all(|i| (heard[i + d] - ((i as f32) * 0.05).sin()).abs() < 0.02));
        assert!(late.is_some_and(|d| d > 0 && d < 200), "{late:?}");
    }

    #[test]
    fn the_formats_are_told_apart_and_faults_are_told() {
        let rtneural = r#"{"in_shape": [null, null, 1], "name": "Small amp", "author": "Someone", "samplerate": "44100",
            "layers": [{"type": "lstm", "activation": "", "shape": [null, null, 2], "weights": [[[0,0,0,0,0,0,0,0]], [[0,0,0,0,0,0,0,0],[0,0,0,0,0,0,0,0]], [0,0,0,0,0,0,0,0]]},
                       {"type": "dense", "shape": [null, null, 1], "weights": [[[0],[0]], [0.5]]}]}"#;
        let capture = Capture::parse(rtneural, "file").unwrap();
        assert_eq!((capture.about.format, capture.about.sample_rate), ("AIDA-X", 44_100.0));
        assert_eq!((capture.about.name.as_str(), capture.about.maker.as_str()), ("Small amp", "Someone"));
        let mut rt = capture.start(44_100.0, 16).unwrap();
        let mut buf = [0.3f32; 16];
        rt.process(&mut buf);
        assert!(buf.iter().all(|s| (s - 0.5).abs() < 1e-6));
        assert!(Capture::parse("{}", "x").unwrap_err().contains("neither"));
        assert!(Capture::parse("not json", "x").unwrap_err().contains("not a capture"));
        assert!(Capture::parse(r#"{"architecture": "WaveNet", "config": {}, "weights": []}"#, "x").is_err(), "a broken NAM file is refused, not a crash");
        let wrong = proteus(2, 1, 0.0).replace("\"hidden_size\": 2", "\"hidden_size\": 3");
        assert!(Capture::parse(&wrong, "x").unwrap_err().contains("do not fit"));
    }
}
