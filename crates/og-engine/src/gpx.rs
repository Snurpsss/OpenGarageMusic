//! The container of Guitar Pro 6 files (`.gpx`): a small file system,
//! usually compressed. Inside it, `score.gpif` is the same XML score as in
//! the files of Guitar Pro 7 and 8, read by [`crate::guitar_pro`].
//!
//! Two layers, from public descriptions of the format (see "External files"):
//!
//! - **BCFZ**, the compression: "BCFZ", the length of what it unpacks to
//!   (32 bits, little-endian), then a stream of bits, read from the highest
//!   bit of each byte down. Each chunk starts with one bit. A 0 is followed by
//!   a count of 0 to 3 on 2 bits, then that many bytes copied as they are. A
//!   1 is followed by a width on 4 bits, then a distance and a length of that
//!   width: bytes already unpacked, that far back, are copied again. The
//!   count, the distance and the length are written lowest bit first; the
//!   width and the bytes highest bit first.
//! - **BCFS**, the file system: "BCFS", then sectors of 4096 bytes, the
//!   first one unused. A sector that starts with the number 2 (32 bits)
//!   describes a file: its name from byte 4 (at most 127 bytes, ended by a
//!   zero), its size at byte 140, and from byte 148 the numbers of the
//!   sectors that hold its bytes, ended by a zero.
//!
//! Every read is bounded: a damaged file gives an error, never a crash.

use std::collections::HashMap;

const SECTOR: usize = 0x1000;
/// The largest container unpacked, in bytes: a song takes well under a megabyte.
const MAX_UNPACKED: usize = 1 << 26;
const NAME_AT: usize = 0x04;
const NAME_MAX: usize = 127;
const SIZE_AT: usize = 0x8C;
const SECTORS_AT: usize = 0x94;
/// What marks a sector that describes a file.
const FILE_ENTRY: u32 = 2;

/// Whether these are the bytes of a Guitar Pro 6 container.
pub fn is_container(bytes: &[u8]) -> bool {
    bytes.starts_with(b"BCFZ") || bytes.starts_with(b"BCFS")
}

/// The files of a Guitar Pro 6 container, by name.
pub fn files(bytes: &[u8]) -> Result<HashMap<String, Vec<u8>>, String> {
    let system = if bytes.starts_with(b"BCFZ") {
        unpack(&bytes[4..])?
    } else if bytes.starts_with(b"BCFS") {
        bytes.to_vec()
    } else {
        return Err("not a Guitar Pro 6 file: it starts with neither BCFZ nor BCFS".to_owned());
    };
    if !system.starts_with(b"BCFS") {
        return Err("the Guitar Pro 6 file did not unpack into its file system".to_owned());
    }
    Ok(read_system(&system[4..]))
}

/// Bits read from the highest bit of each byte down.
struct Bits<'a> {
    bytes: &'a [u8],
    at: usize,
    bit: u32,
}

impl Bits<'_> {
    fn next(&mut self) -> Option<u32> {
        let byte = *self.bytes.get(self.at)?;
        let value = u32::from(byte >> (7 - self.bit)) & 1;
        self.bit += 1;
        if self.bit == 8 {
            self.bit = 0;
            self.at += 1;
        }
        Some(value)
    }

    /// A number of `count` bits, the first one read the highest.
    fn high_first(&mut self, count: u32) -> Option<u32> {
        (0..count).try_fold(0, |value, _| Some(value << 1 | self.next()?))
    }

    /// A number of `count` bits, the first one read the lowest.
    fn low_first(&mut self, count: u32) -> Option<u32> {
        (0..count).try_fold(0, |value, place| Some(value | self.next()? << place))
    }
}

/// Unpacks what follows "BCFZ". Some files end before the length they
/// announce, on bits that only pad the last byte: what came is kept.
fn unpack(packed: &[u8]) -> Result<Vec<u8>, String> {
    let announced = packed.get(..4).ok_or("the Guitar Pro 6 file is cut short")?;
    let length = u32::from_le_bytes([announced[0], announced[1], announced[2], announced[3]]) as usize;
    if length > MAX_UNPACKED {
        return Err("the Guitar Pro 6 file is too large to read".to_owned());
    }
    let mut bits = Bits { bytes: &packed[4..], at: 0, bit: 0 };
    let mut out: Vec<u8> = Vec::with_capacity(length.min(1 << 20));
    while out.len() < length {
        let Some(flag) = bits.next() else { break };
        if flag == 1 {
            let Some(width) = bits.high_first(4) else { break };
            let (Some(distance), Some(count)) = (bits.low_first(width), bits.low_first(width)) else { break };
            let (distance, count) = (distance as usize, count as usize);
            if distance == 0 || distance > out.len() {
                return Err("the Guitar Pro 6 file is damaged: it repeats bytes it has not got".to_owned());
            }
            // A copy never reaches past what is already there.
            let from = out.len() - distance;
            let take = count.min(distance).min(length - out.len());
            out.extend_from_within(from..from + take);
        } else {
            let Some(count) = bits.low_first(2) else { break };
            for _ in 0..count {
                let Some(byte) = bits.high_first(8) else { break };
                out.push(byte as u8);
            }
        }
    }
    Ok(out)
}

use crate::sf3::u32_at;

/// The files the sectors of a BCFS file system describe.
fn read_system(system: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut files = HashMap::new();
    let sectors = system.len() / SECTOR + 1;
    for entry in (SECTOR..system.len()).step_by(SECTOR) {
        if u32_at(system, entry) != Some(FILE_ENTRY) {
            continue;
        }
        let Some(name) = system.get(entry + NAME_AT..(entry + NAME_AT + NAME_MAX).min(system.len())) else { continue };
        let name = String::from_utf8_lossy(&name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())]).into_owned();
        let Some(size) = u32_at(system, entry + SIZE_AT) else { continue };
        if name.is_empty() {
            continue;
        }
        let mut data = Vec::new();
        // The numbers of the sectors fill the rest of the entry's sector at most.
        for place in (entry + SECTORS_AT..entry + SECTOR).step_by(4).take(sectors) {
            let Some(sector) = u32_at(system, place).filter(|s| *s != 0) else { break };
            let start = (sector as usize).saturating_mul(SECTOR);
            if let Some(bytes) = system.get(start..(start + SECTOR).min(system.len())) {
                data.extend_from_slice(bytes);
            }
        }
        data.truncate(size as usize);
        files.insert(name, data);
    }
    files
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Bits written as the reader reads them.
    #[derive(Default)]
    struct Writer {
        bytes: Vec<u8>,
        bit: u32,
    }

    impl Writer {
        fn push(&mut self, value: u32) {
            if self.bit == 0 {
                self.bytes.push(0);
            }
            if value & 1 == 1 {
                *self.bytes.last_mut().unwrap() |= 1 << (7 - self.bit);
            }
            self.bit = (self.bit + 1) % 8;
        }
        fn high_first(&mut self, value: u32, count: u32) {
            for place in (0..count).rev() {
                self.push(value >> place);
            }
        }
        fn low_first(&mut self, value: u32, count: u32) {
            for place in 0..count {
                self.push(value >> place);
            }
        }
        fn literal(&mut self, bytes: &[u8]) {
            self.push(0);
            self.low_first(bytes.len() as u32, 2);
            for byte in bytes {
                self.high_first(u32::from(*byte), 8);
            }
        }
        fn repeat(&mut self, distance: u32, count: u32) {
            self.push(1);
            self.high_first(8, 4);
            self.low_first(distance, 8);
            self.low_first(count, 8);
        }
    }

    /// A BCFS file system holding these files, each in sectors of its own.
    pub(crate) fn system(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut sectors: Vec<Vec<u8>> = vec![vec![0; SECTOR]];
        for (name, bytes) in files {
            let entry = sectors.len();
            sectors.push(vec![0; SECTOR]);
            let mut numbers = Vec::new();
            for chunk in bytes.chunks(SECTOR) {
                numbers.push(sectors.len() as u32);
                let mut sector = chunk.to_vec();
                sector.resize(SECTOR, 0);
                sectors.push(sector);
            }
            let sector = &mut sectors[entry];
            sector[..4].copy_from_slice(&FILE_ENTRY.to_le_bytes());
            sector[NAME_AT..NAME_AT + name.len()].copy_from_slice(name.as_bytes());
            sector[SIZE_AT..SIZE_AT + 4].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
            for (i, number) in numbers.iter().enumerate() {
                sector[SECTORS_AT + 4 * i..SECTORS_AT + 4 * i + 4].copy_from_slice(&number.to_le_bytes());
            }
        }
        let mut out = b"BCFS".to_vec();
        out.extend(sectors.concat());
        out
    }

    /// The same, packed in literal chunks of three bytes: a BCFZ file.
    pub(crate) fn packed(system: &[u8]) -> Vec<u8> {
        let mut writer = Writer::default();
        for chunk in system.chunks(3) {
            writer.literal(chunk);
        }
        let mut out = b"BCFZ".to_vec();
        out.extend((system.len() as u32).to_le_bytes());
        out.extend(writer.bytes);
        out
    }

    #[test]
    fn bytes_are_unpacked_copied_as_they_are_or_repeated() {
        let mut writer = Writer::default();
        writer.literal(b"abc");
        writer.literal(b"d");
        writer.repeat(4, 6);
        writer.repeat(2, 1);
        writer.literal(b"");
        writer.literal(b"Z");
        let mut packed = 12u32.to_le_bytes().to_vec();
        packed.extend(&writer.bytes);
        // A repeat copies at most what lies between it and the end.
        assert_eq!(unpack(&packed).unwrap(), b"abcdabcdcZ");
        // Announced longer than what comes: what came is kept.
        assert_eq!(unpack(&packed).unwrap().len(), 10);
        // Shorter: it stops there.
        packed[..4].copy_from_slice(&5u32.to_le_bytes());
        assert_eq!(unpack(&packed).unwrap(), b"abcda");
        // A repeat of what is not there yet is a damaged file.
        let mut bad = Writer::default();
        bad.repeat(3, 3);
        let mut packed = 9u32.to_le_bytes().to_vec();
        packed.extend(&bad.bytes);
        assert!(unpack(&packed).unwrap_err().contains("damaged"));
        assert!(unpack(&[1, 2]).is_err());
        assert!(unpack(&u32::MAX.to_le_bytes()).unwrap_err().contains("too large"));
    }

    #[test]
    fn the_files_of_the_container_are_read_back() {
        let score = "<GPIF>".to_owned() + &"x".repeat(5000) + "</GPIF>";
        let system = system(&[("score.gpif", score.as_bytes()), ("misc.xml", b"<misc/>")]);
        for bytes in [system.clone(), packed(&system)] {
            let files = files(&bytes).unwrap();
            assert_eq!(files.len(), 2);
            assert_eq!(files["score.gpif"], score.as_bytes(), "over two sectors");
            assert_eq!(files["misc.xml"], b"<misc/>");
        }
        assert!(is_container(b"BCFZ....") && is_container(b"BCFS") && !is_container(b"PK\x03\x04"));
        assert!(files(b"PK\x03\x04").is_err());
        assert!(files(&packed(b"not a file system")).unwrap_err().contains("file system"));
        // Damaged: sector numbers past the end, a size larger than the data.
        let mut broken = system.clone();
        broken[4 + SECTOR + SECTORS_AT..4 + SECTOR + SECTORS_AT + 4].copy_from_slice(&999u32.to_le_bytes());
        broken[4 + SECTOR + SIZE_AT..4 + SECTOR + SIZE_AT + 4].copy_from_slice(&u32::MAX.to_le_bytes());
        let files = files(&broken).unwrap();
        assert!(files["score.gpif"].len() < score.len());
        // Anything at all gives an answer, never a crash.
        for cut in [0, 3, 4, 9, 100, 4100, 8200] {
            let _ = super::files(&system[..cut.min(system.len())]);
            let _ = super::files(&packed(&system)[..cut.min(packed(&system).len())]);
        }
    }
}
