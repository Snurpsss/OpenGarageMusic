//! The video player of OpenGarageMusic: YouTube's own player, in a web view
//! drawn in a window of its own inside the program's window.
//!
//! Started by the program with the handle of its window and the folder the
//! web view keeps its data in; it then reads orders from its input, one JSON
//! object a line:
//!
//! - `{"bounds": [x, y, width, height]}`: where it is in the program's window,
//!   in pixels;
//! - `{"show": true}` / `{"show": false}`;
//! - `{"js": "og.load('id', 12.5)"}`: a call to the page (`og.load`, `og.play`,
//!   `og.pause`, `og.seek`, `og.volume`);
//!
//! and writes what the page says, one JSON object a line: `{"t": 12.6, "s": 1}`
//! (the second the video is at, and the state of the player), `{"ready":
//! true}`, `{"error": 150}`. It ends when its input does.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
mod host;

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let Some(parent) = args.get(1).and_then(|a| a.parse::<isize>().ok()).filter(|h| *h != 0) else {
        eprintln!("usage: og-video <window> [data folder]");
        std::process::exit(2);
    };
    let data = args.get(2).map(std::path::PathBuf::from);
    if let Err(e) = host::run(parent, data) {
        println!("{}", serde_json::json!({ "failed": e }));
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    println!("{}", serde_json::json!({ "failed": "the video player is only made for Windows" }));
    std::process::exit(1);
}
