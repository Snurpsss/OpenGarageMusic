//! Two jobs before the program is compiled:
//!
//! - every catalogue of the `lang` folder is built into the program: adding a
//!   `<code>.po` file there is all it takes to add a language;
//! - on Windows, the program's icon is read (by `src/icon.rs`) and linked
//!   into the executable, so that the file shows it in the explorer and on
//!   the task bar.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[path = "src/icon.rs"]
mod icon;
#[path = "build_icon.rs"]
mod build_icon;
use build_icon::executable_icon;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo"));
    // The version as it is shown: Cargo writes a fourth number as "+1" (0.9.9+1
    // is shown 0.9.9.1).
    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default().replace('+', ".");
    println!("cargo:rustc-env=OG_VERSION={version}");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    languages(&manifest, &out);
    println!("cargo:rerun-if-changed=src/icon.rs");
    println!("cargo:rerun-if-changed=build_icon.rs");
    println!("cargo:rerun-if-changed=icon");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=themes");
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        if let Err(reason) = executable_icon(&out) {
            println!("cargo:warning=the executable has no icon: {reason}");
        }
    }
}

fn languages(manifest: &Path, out: &Path) {
    let lang = manifest.join("lang");
    println!("cargo:rerun-if-changed={}", lang.display());
    let mut files: Vec<PathBuf> = std::fs::read_dir(&lang)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == "po")).collect())
        .unwrap_or_default();
    files.sort();
    let mut code = String::from("pub const BUILT_IN: &[(&str, &str)] = &[\n");
    for file in files {
        println!("cargo:rerun-if-changed={}", file.display());
        let name = file.file_stem().expect("a .po file has a name").to_string_lossy().to_lowercase();
        let _ = writeln!(code, "    ({name:?}, include_str!({:?})),", file.to_string_lossy());
    }
    code.push_str("];\n");
    std::fs::write(out.join("languages.rs"), code).expect("the build folder can be written");
}

