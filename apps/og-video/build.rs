//! Puts WebView2Loader.dll next to the player once it is built: the player
//! needs it to start, and it comes with the crate that binds WebView2 (under
//! the BSD licence of its makers, Microsoft). The packaging script takes it
//! from there.

use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_WINDOWS").is_err() {
        return;
    }
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x64",
        Ok("x86") => "x86",
        Ok("aarch64") => "arm64",
        _ => return,
    };
    // The registry where cargo keeps the sources of the crates.
    let home = std::env::var_os("CARGO_HOME").map(PathBuf::from).or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".cargo")));
    let Some(registry) = home.map(|h| h.join("registry").join("src")) else { return };
    let Some(loader) = find_loader(&registry, arch) else {
        println!("cargo:warning=WebView2Loader.dll was not found: the video player will not start without it");
        return;
    };
    // OUT_DIR is target/<profile>/build/<crate>-<hash>/out: three folders up.
    let Some(out) = std::env::var_os("OUT_DIR").map(PathBuf::from) else { return };
    if let Some(profile) = out.ancestors().nth(3) {
        let _ = std::fs::copy(&loader, profile.join("WebView2Loader.dll"));
    }
}

/// The loader in the newest webview2-com-sys found.
fn find_loader(registry: &Path, arch: &str) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(registry)
        .ok()?
        .flatten()
        .flat_map(|index| std::fs::read_dir(index.path()).into_iter().flatten().flatten())
        .map(|entry| entry.path())
        .filter(|path| path.file_name().is_some_and(|n| n.to_string_lossy().starts_with("webview2-com-sys-")))
        .map(|path| path.join(arch).join("WebView2Loader.dll"))
        .filter(|path| path.is_file())
        .collect();
    found.sort();
    found.pop()
}
