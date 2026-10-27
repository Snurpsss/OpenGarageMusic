//! The video player: YouTube's own player, in the program `og-video` beside
//! this one, which draws it in a window inside ours. This side starts it,
//! tells it where to show itself and what to do, and reads where the video
//! is (see `apps/og-video`).

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;

/// States of YouTube's player, as it numbers them.
pub const PLAYING: i64 = 1;
pub const BUFFERING: i64 = 3;

/// The program that holds the player: next to this one.
pub fn helper() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    ["og-video.exe", "og-video"].iter().map(|name| dir.join(name)).find(|path| path.is_file()).or_else(|| {
        // Tests run from target/<profile>/deps.
        dir.parent().map(|up| up.join("og-video.exe")).filter(|path| path.is_file())
    })
}

pub struct VideoPlayer {
    child: Child,
    input: ChildStdin,
    events: mpsc::Receiver<serde_json::Value>,
    /// The page loaded YouTube's player.
    pub api: bool,
    /// What went wrong, as the player says it.
    pub failed: Option<String>,
    pub error: Option<i64>,
    /// The video loaded, by its id.
    pub loaded: String,
    shown: bool,
    bounds: [i32; 4],
    /// Where the video was when it last said, its state, and when that was.
    report: Option<(f64, i64, Instant)>,
    /// When the video was last told to go somewhere, or to play.
    pub sought: Option<Instant>,
    pub asked_to_play: Option<Instant>,
    /// When the video was last paused by the program.
    pub paused: Option<Instant>,
    /// The state seen at the last frame: the video started from YouTube's own
    /// player is told by its going over to playing.
    pub seen_state: i64,
    volume: i32,
}

impl VideoPlayer {
    /// Starts the player inside the window `parent`; it keeps its data in `data`.
    pub fn start(parent: isize, data: &Path) -> Result<VideoPlayer, String> {
        let helper = helper().ok_or("og-video.exe is missing next to the program")?;
        let mut child = Command::new(helper)
            .arg(parent.to_string())
            .arg(data)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let input = child.stdin.take().ok_or("no input")?;
        let output = child.stdout.take().ok_or("no output")?;
        let (send, events) = mpsc::channel();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(output).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str(&line) {
                    if send.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        Ok(VideoPlayer { child, input, events, api: false, failed: None, error: None, loaded: String::new(), shown: false, bounds: [0; 4], report: None, sought: None, asked_to_play: None, paused: None, seen_state: -1, volume: -1 })
    }

    fn send(&mut self, value: serde_json::Value) {
        if writeln!(self.input, "{value}").and_then(|()| self.input.flush()).is_err() {
            self.failed.get_or_insert_with(|| "the video player stopped".to_owned());
        }
    }

    /// Calls the page.
    pub fn call(&mut self, script: &str) {
        self.send(serde_json::json!({ "js": script }));
    }

    /// Reads what the player said since the last frame.
    pub fn poll(&mut self) {
        for event in self.events.try_iter().collect::<Vec<_>>() {
            if let Some(t) = event.get("t").and_then(serde_json::Value::as_f64) {
                let state = event.get("s").and_then(serde_json::Value::as_i64).unwrap_or(-1);
                self.report = Some((t, state, Instant::now()));
            } else if let Some(state) = event.get("s").and_then(serde_json::Value::as_i64) {
                if let Some((t, _, at)) = self.report {
                    self.report = Some((t, state, at));
                }
            }
            if event.get("api").is_some() || event.get("ready").is_some() {
                self.api = true;
            }
            if let Some(code) = event.get("error").and_then(serde_json::Value::as_i64) {
                self.error = Some(code);
            }
            if let Some(why) = event.get("failed").and_then(serde_json::Value::as_str) {
                self.failed = Some(why.to_owned());
            }
        }
        if let Ok(Some(_)) = self.child.try_wait() {
            self.failed.get_or_insert_with(|| "the video player stopped".to_owned());
        }
    }

    /// The state of the player (1 playing, 2 paused, 3 loading…), -1 unknown.
    pub fn state(&self) -> i64 {
        self.report.map_or(-1, |(_, state, _)| state)
    }

    /// Where the video is now: where it last said, and the time since when it plays.
    pub fn time(&self) -> Option<f64> {
        let (t, state, at) = self.report?;
        Some(if state == PLAYING { t + at.elapsed().as_secs_f64() } else { t })
    }

    /// Shows the player at these bounds, in pixels of the window, or hides it.
    pub fn place(&mut self, bounds: Option<[i32; 4]>) {
        match bounds {
            Some(bounds) => {
                if bounds != self.bounds {
                    self.bounds = bounds;
                    self.send(serde_json::json!({ "bounds": bounds }));
                }
                if !self.shown {
                    self.shown = true;
                    self.send(serde_json::json!({ "show": true }));
                }
            }
            None if self.shown => {
                self.shown = false;
                self.send(serde_json::json!({ "show": false }));
            }
            None => {}
        }
    }

    pub fn load(&mut self, id: &str, at: f64) {
        self.loaded = id.to_owned();
        self.error = None;
        self.report = None;
        self.call(&format!("og.load({}, {:.3})", serde_json::json!(id), at.max(0.0)));
    }

    pub fn seek(&mut self, at: f64) {
        self.sought = Some(Instant::now());
        // Playing, the video may take a moment to go there: the wait counts from now.
        if self.asked_to_play.is_some() {
            self.asked_to_play = Some(Instant::now());
        }
        self.call(&format!("og.seek({:.3})", at.max(0.0)));
        if let Some((_, state, _)) = self.report {
            self.report = Some((at.max(0.0), state, Instant::now()));
        }
    }

    pub fn play(&mut self) {
        self.asked_to_play = Some(Instant::now());
        self.call("og.play()");
    }

    pub fn pause(&mut self) {
        self.asked_to_play = None;
        self.paused = Some(Instant::now());
        self.call("og.pause()");
    }

    /// From 0 to 100.
    pub fn set_volume(&mut self, volume: i32) {
        let volume = volume.clamp(0, 100);
        if volume != self.volume {
            self.volume = volume;
            self.call(&format!("og.volume({volume})"));
        }
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        // Without its input, the player ends; it is stopped if it does not.
        self.send(serde_json::json!({ "quit": true }));
        let _ = self.child.kill();
    }
}
