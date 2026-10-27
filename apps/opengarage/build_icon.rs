//! The icon of a program, linked into its executable (shared by the build
//! scripts of the program and of the MIDI Pilot launcher): drawn at the sizes
//! Windows asks for, written as a resource file, turned into an object file.

use std::path::Path;
use std::process::Command;

use crate::icon;

fn checksum(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xEDB8_8320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

/// A PNG picture of `size` by `size` pixels, written without compression:
/// simple, and an icon is small.
fn png(size: u32, rgba: &[u8]) -> Vec<u8> {
    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let sum = checksum(&out[start..]);
        out.extend_from_slice(&sum.to_be_bytes());
    }
    // Each row starts with a byte that says "no filter".
    let mut raw = Vec::with_capacity(rgba.len() + size as usize);
    for row in rgba.chunks(size as usize * 4) {
        raw.push(0);
        raw.extend_from_slice(row);
    }
    // The "zlib" wrapping, with the data in stored blocks of at most 65535 bytes.
    let mut packed = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (index, block) in blocks.iter().enumerate() {
        packed.push(u8::from(index + 1 == blocks.len()));
        packed.extend_from_slice(&(block.len() as u16).to_le_bytes());
        packed.extend_from_slice(&(!(block.len() as u16)).to_le_bytes());
        packed.extend_from_slice(block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for byte in &raw {
        a = (a + u32::from(*byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    packed.extend_from_slice(&((b << 16) | a).to_be_bytes());

    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut header = Vec::new();
    header.extend_from_slice(&size.to_be_bytes());
    header.extend_from_slice(&size.to_be_bytes());
    // Eight bits per channel, red-green-blue-opacity, no interlacing.
    header.extend_from_slice(&[8, 6, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &packed);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// One entry of a Windows resource file: a header, then the data, both padded
/// to a multiple of four bytes.
fn resource(out: &mut Vec<u8>, kind: u16, id: u16, flags: u16, data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    for number in [kind, id] {
        out.extend_from_slice(&0xFFFFu16.to_le_bytes());
        out.extend_from_slice(&number.to_le_bytes());
    }
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    // Language: none in particular.
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(data);
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

/// Draws the icon at the sizes Windows asks for, writes them as a resource
/// file, and has it turned into an object file that is linked into the program.
pub fn executable_icon(out: &Path) -> Result<(), String> {
    const SIZES: [u32; 6] = [16, 24, 32, 48, 64, 256];
    const ICON: u16 = 3;
    const GROUP: u16 = 14;
    let mut file = Vec::new();
    // A resource file starts with an empty entry.
    resource(&mut file, 0, 0, 0, &[]);
    let mut group = Vec::new();
    group.extend_from_slice(&0u16.to_le_bytes());
    group.extend_from_slice(&1u16.to_le_bytes());
    group.extend_from_slice(&(SIZES.len() as u16).to_le_bytes());
    for (index, size) in SIZES.iter().enumerate() {
        let picture = png(*size, &icon::rgba(*size));
        let id = index as u16 + 1;
        resource(&mut file, ICON, id, 0x1010, &picture);
        // Width and height on one byte each: 0 stands for 256.
        group.extend_from_slice(&[(*size % 256) as u8, (*size % 256) as u8, 0, 0]);
        group.extend_from_slice(&1u16.to_le_bytes());
        group.extend_from_slice(&32u16.to_le_bytes());
        group.extend_from_slice(&(picture.len() as u32).to_le_bytes());
        group.extend_from_slice(&id.to_le_bytes());
    }
    resource(&mut file, GROUP, 1, 0x1030, &group);
    let resources = out.join("icon.res");
    std::fs::write(&resources, file).map_err(|e| e.to_string())?;

    // `windres` (from binutils) turns the resource file into an object file.
    let object = out.join("icon.o");
    let tool = std::env::var("WINDRES").unwrap_or_else(|_| "windres".to_owned());
    let status = Command::new(&tool)
        .args(["-J", "res", "-O", "coff", "-i"])
        .arg(&resources)
        .arg("-o")
        .arg(&object)
        .status()
        .map_err(|e| format!("`{tool}` could not be run ({e}); see tools/setup-toolchain.ps1"))?;
    if !status.success() {
        return Err(format!("`{tool}` failed"));
    }
    println!("cargo:rustc-link-arg-bins={}", object.display());
    Ok(())
}
