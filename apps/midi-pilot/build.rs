//! Links the program's icon into the launcher, the same way the program does.

#[path = "../opengarage/src/icon.rs"]
mod icon;
#[path = "../opengarage/build_icon.rs"]
mod build_icon;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../opengarage/src/icon.rs");
    println!("cargo:rerun-if-changed=../opengarage/build_icon.rs");
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
        if let Err(reason) = build_icon::executable_icon(&out) {
            println!("cargo:warning=the launcher has no icon: {reason}");
        }
    }
}
