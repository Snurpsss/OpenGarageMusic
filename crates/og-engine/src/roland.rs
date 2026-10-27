//! The system exclusive dialect of the Roland and BOSS devices, as their
//! public MIDI implementation documents describe it: a memory read by
//! requests (RQ1) and written by data sets (DT1), at addresses of four
//! 7-bit bytes, each message closed by a checksum. This module knows the
//! bytes only; what the memory holds is in `devices`.
//!
//! Addresses are handled in two forms: as the documents write them, four
//! bytes of 7 bits ("30 00 10 00", the "wire" form), and linear, a count of
//! bytes, which is what offsets add to. A document's address converts with
//! [`from_wire`]; the wire form goes on the MIDI cable.

use std::time::{Duration, Instant};

/// The manufacturer byte of Roland and BOSS.
pub const MAKER: u8 = 0x41;
/// The commands: a request for data, and data (sent, or answered).
pub const RQ1: u8 = 0x11;
pub const DT1: u8 = 0x12;
/// Most bytes of data in one DT1: the devices cut what they answer at that,
/// and take no more in one message.
pub const MOST_DATA: usize = 128;
/// The universal identity request, which every device answers with its
/// maker and family.
pub const IDENTITY_REQUEST: [u8; 6] = [0xF0, 0x7E, 0x7F, 0x06, 0x01, 0xF7];

/// A linear address as its four 7-bit bytes, packed in a number.
pub fn to_wire(linear: u32) -> u32 {
    ((linear & 0x0FE0_0000) << 3) | ((linear & 0x001F_C000) << 2) | ((linear & 0x3F80) << 1) | (linear & 0x7F)
}

/// A wire address (four 7-bit bytes packed in a number) as a linear one.
pub fn from_wire(wire: u32) -> u32 {
    ((wire & 0x7F00_0000) >> 3) | ((wire & 0x007F_0000) >> 2) | ((wire & 0x7F00) >> 1) | (wire & 0x7F)
}

/// A wire address written as the documents do, "30 00 10 00", as a linear one.
pub fn parse_wire(text: &str) -> Option<u32> {
    let bytes: Vec<u8> = text.split_whitespace().map(|b| u8::from_str_radix(b, 16).ok()).collect::<Option<_>>()?;
    if bytes.len() != 4 || bytes.iter().any(|b| *b > 0x7F) {
        return None;
    }
    Some(from_wire(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])))
}

/// The four bytes of a wire address, high first.
fn wire_bytes(linear: u32) -> [u8; 4] {
    to_wire(linear).to_be_bytes()
}

/// The checksum that closes a message: the address and the data, summed,
/// taken to the next multiple of 128.
pub fn checksum(body: &[u8]) -> u8 {
    let sum: u32 = body.iter().map(|b| u32::from(*b)).sum();
    ((128 - sum % 128) % 128) as u8
}

/// A number as its 4-bit halves, `count` bytes, high first; how the devices
/// keep values above 127.
pub fn nibbles(value: u32, count: usize) -> Vec<u8> {
    (0..count).map(|i| ((value >> (4 * (count - 1 - i))) & 0x0F) as u8).collect()
}

/// A number read back from its 4-bit halves.
pub fn from_nibbles(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0, |value, b| (value << 4) | u32::from(b & 0x0F))
}

/// Which device the messages are for: its device number and the four bytes
/// of its model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Head {
    pub device_id: u8,
    pub model_id: [u8; 4],
}

impl Head {
    fn bytes(&self) -> [u8; 6] {
        [MAKER, self.device_id, self.model_id[0], self.model_id[1], self.model_id[2], self.model_id[3]]
    }

    fn message(&self, command: u8, body: &[u8]) -> Vec<u8> {
        let mut out = vec![0xF0];
        out.extend_from_slice(&self.bytes());
        out.push(command);
        out.extend_from_slice(body);
        out.push(checksum(body));
        out.push(0xF7);
        out
    }

    /// A request for `size` bytes at a linear address.
    pub fn request(&self, linear: u32, size: u32) -> Vec<u8> {
        let mut body = wire_bytes(linear).to_vec();
        body.extend_from_slice(&wire_bytes(size));
        self.message(RQ1, &body)
    }

    /// One data set at a linear address: at most `MOST_DATA` bytes.
    pub fn data_set(&self, linear: u32, data: &[u8]) -> Vec<u8> {
        debug_assert!(data.len() <= MOST_DATA);
        let mut body = wire_bytes(linear).to_vec();
        body.extend_from_slice(data);
        self.message(DT1, &body)
    }

    /// The data sets that write `data` at a linear address, each of at most
    /// `MOST_DATA` bytes.
    pub fn data_sets(&self, linear: u32, data: &[u8]) -> Vec<Vec<u8>> {
        data.chunks(MOST_DATA).enumerate().map(|(i, chunk)| self.data_set(linear + (i * MOST_DATA) as u32, chunk)).collect()
    }

    /// A data set of this device, read: its linear address and its data;
    /// none for any other message, or a checksum that does not add up.
    pub fn parse(&self, message: &[u8]) -> Option<(u32, Vec<u8>)> {
        let inner = message.strip_prefix(&[0xF0])?.strip_suffix(&[0xF7])?;
        let body = inner.strip_prefix(&self.bytes())?.strip_prefix(&[DT1])?;
        let (body, sum) = body.split_at(body.len().checked_sub(1)?);
        if body.len() < 4 || checksum(body) != sum[0] {
            return None;
        }
        let address = from_wire(u32::from_be_bytes([body[0], body[1], body[2], body[3]]));
        Some((address, body[4..].to_vec()))
    }
}

/// The answer to the identity request: the maker and the family of the
/// device, when the message is one.
pub fn identity(message: &[u8]) -> Option<(u8, [u8; 2])> {
    let inner = message.strip_prefix(&[0xF0, 0x7E])?.strip_suffix(&[0xF7])?;
    if inner.len() < 7 || inner[1] != 0x06 || inner[2] != 0x02 {
        return None;
    }
    Some((inner[3], [inner[4], inner[5]]))
}

/// A MIDI link to a device: what the program sends, and what comes back.
pub trait Port {
    fn send(&mut self, message: &[u8]) -> Result<(), String>;
    /// The next message that came, waiting at most `wait` for it.
    fn receive(&mut self, wait: Duration) -> Option<Vec<u8>>;
    /// Throws away what came and was not read: the echoes of what was sent,
    /// the clock, and the answers to a read that ended early.
    fn drain(&mut self) {
        while self.receive(Duration::ZERO).is_some() {}
    }
}

/// A device talked to over a port: reads and writes of its memory.
pub struct Link<'a> {
    pub head: Head,
    pub port: &'a mut dyn Port,
    /// How long an answer is waited for.
    pub timeout: Duration,
    /// A rest between two data sets, for the device to take each in.
    pub pause: Duration,
}

impl Link<'_> {
    pub fn new(head: Head, port: &mut dyn Port) -> Link<'_> {
        Link { head, port, timeout: Duration::from_secs(2), pause: Duration::from_millis(30) }
    }

    /// Reads `size` bytes at a linear address: the request is sent, and the
    /// data sets that answer it are put together, whatever their order.
    /// The device's echo of the request, and anything else, are passed over.
    pub fn read(&mut self, linear: u32, size: usize) -> Result<Vec<u8>, String> {
        self.port.drain();
        self.port.send(&self.head.request(linear, size as u32))?;
        let mut data = vec![0u8; size];
        let mut got = vec![false; size];
        let mut missing = size;
        let mut deadline = Instant::now() + self.timeout;
        while missing > 0 {
            let now = Instant::now();
            if now >= deadline {
                return Err(format!("{} of {size} bytes came from the device", size - missing));
            }
            let Some(message) = self.port.receive(deadline - now) else { continue };
            let Some((address, bytes)) = self.head.parse(&message) else { continue };
            for (i, byte) in bytes.into_iter().enumerate() {
                let Some(place) = (address as usize + i).checked_sub(linear as usize) else { continue };
                if place < size {
                    data[place] = byte;
                    if !got[place] {
                        got[place] = true;
                        missing -= 1;
                    }
                }
            }
            deadline = Instant::now() + self.timeout;
        }
        Ok(data)
    }

    /// Writes data at a linear address, in data sets of at most `MOST_DATA`
    /// bytes, with a pause after each.
    pub fn write(&mut self, linear: u32, data: &[u8]) -> Result<(), String> {
        for message in self.head.data_sets(linear, data) {
            self.port.send(&message)?;
            if !self.pause.is_zero() {
                std::thread::sleep(self.pause);
            }
        }
        Ok(())
    }

    /// Asks the device who it is: its maker and family bytes.
    pub fn identify(&mut self) -> Result<(u8, [u8; 2]), String> {
        self.port.drain();
        self.port.send(&IDENTITY_REQUEST)?;
        let deadline = Instant::now() + self.timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                return Err("no answer to the identity request".to_owned());
            }
            if let Some(message) = self.port.receive(deadline - now) {
                if let Some(answer) = identity(&message) {
                    return Ok(answer);
                }
            }
        }
    }
}

/// A device made up for the tests (of the engine and of the programs): a
/// memory that answers requests and takes data sets, echoing what it
/// receives as the real ones do.
pub mod fake {
    use super::*;
    use std::collections::{HashMap, VecDeque};

    pub struct Device {
        pub head: Head,
        pub memory: HashMap<u32, u8>,
        pub family: [u8; 2],
        pub answers: VecDeque<Vec<u8>>,
        /// Every message received, for the tests to look at.
        pub received: Vec<Vec<u8>>,
    }

    impl Device {
        pub fn new(head: Head, family: [u8; 2]) -> Self {
            Device { head, memory: HashMap::new(), family, answers: VecDeque::new(), received: Vec::new() }
        }

        pub fn fill(&mut self, linear: u32, data: &[u8]) {
            for (i, byte) in data.iter().enumerate() {
                self.memory.insert(linear + i as u32, *byte);
            }
        }

        pub fn bytes_at(&self, linear: u32, size: usize) -> Vec<u8> {
            (0..size as u32).map(|i| self.memory.get(&(linear + i)).copied().unwrap_or(0)).collect()
        }
    }

    impl Port for Device {
        fn send(&mut self, message: &[u8]) -> Result<(), String> {
            self.received.push(message.to_vec());
            // The echo first, as the real devices with THRU on do.
            self.answers.push_back(message.to_vec());
            if message == IDENTITY_REQUEST {
                self.answers.push_back(vec![0xF0, 0x7E, self.head.device_id, 0x06, 0x02, MAKER, self.family[0], self.family[1], 0, 0, 0, 0, 0, 1, 0xF7]);
                return Ok(());
            }
            let inner = &message[1..message.len() - 1];
            if inner.len() < 8 || inner[..6] != [MAKER, self.head.device_id, self.head.model_id[0], self.head.model_id[1], self.head.model_id[2], self.head.model_id[3]] {
                return Ok(());
            }
            let body = &inner[7..inner.len() - 1];
            let address = from_wire(u32::from_be_bytes([body[0], body[1], body[2], body[3]]));
            match inner[6] {
                RQ1 => {
                    let size = from_wire(u32::from_be_bytes([body[4], body[5], body[6], body[7]])) as usize;
                    let data = self.bytes_at(address, size);
                    // Answered in pieces, the last first, to test the putting together.
                    for message in self.head.data_sets(address, &data).into_iter().rev() {
                        self.answers.push_back(message);
                    }
                }
                DT1 => {
                    let data = body[4..].to_vec();
                    self.fill(address, &data);
                }
                _ => {}
            }
            Ok(())
        }

        fn receive(&mut self, _wait: Duration) -> Option<Vec<u8>> {
            self.answers.pop_front()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: Head = Head { device_id: 0x10, model_id: [0, 0, 0, 0x4D] };

    #[test]
    fn addresses_go_to_the_wire_and_back_as_the_documents_write_them() {
        assert_eq!(to_wire(0x0600_1000), 0x3000_2000, "the temporary patch B");
        assert_eq!(from_wire(0x3000_2000), 0x0600_1000);
        assert_eq!(parse_wire("30 00 10 00"), Some(0x0600_0800));
        assert_eq!(parse_wire("33 0C 20 00"), Some(0x0601_0000 + 0x1_0000 * 98 + 0x1000));
        assert_eq!((parse_wire("30 00 10"), parse_wire("80 00 00 00"), parse_wire("zz 00 00 00")), (None, None, None));
        for linear in [0, 0x7F, 0x80, 0x3FFF, 0x0601_0000, 0x0FFF_FFFF] {
            assert_eq!(from_wire(to_wire(linear)), linear);
        }
        assert_eq!(nibbles(440, 4), [0, 1, 0xB, 8]);
        assert_eq!(from_nibbles(&[0, 1, 0xB, 8]), 440);
        assert_eq!(from_nibbles(&nibbles(295, 3)), 295);
    }

    #[test]
    fn messages_carry_their_checksum_and_are_read_back() {
        let request = HEAD.request(0x0600_0800, 475);
        assert_eq!(request, [0xF0, 0x41, 0x10, 0, 0, 0, 0x4D, 0x11, 0x30, 0, 0x10, 0, 0, 0, 0x03, 0x5B, 0x62, 0xF7]);
        let set = HEAD.data_set(0x0600_1011, &[0, 1, 0xB, 8]);
        assert_eq!(set, [0xF0, 0x41, 0x10, 0, 0, 0, 0x4D, 0x12, 0x30, 0, 0x20, 0x11, 0, 1, 0xB, 8, 0x0B, 0xF7], "TIME 440 ms in the temporary patch B");
        assert_eq!(HEAD.parse(&set), Some((0x0600_1011, vec![0, 1, 0xB, 8])));
        let mut wrong = set.clone();
        wrong[16] ^= 1;
        assert_eq!(HEAD.parse(&wrong), None, "a checksum that does not add up");
        assert_eq!(HEAD.parse(&request), None, "a request is not data");
        let other = Head { device_id: 0x10, model_id: [0, 0, 0, 0x42] };
        assert_eq!(other.parse(&set), None, "another model's");
        let sets = HEAD.data_sets(0x0601_0800, &[7; 300]);
        assert_eq!(sets.len(), 3);
        assert_eq!(HEAD.parse(&sets[2]).unwrap(), (0x0601_0800 + 256, vec![7; 44]));
        assert_eq!(identity(&[0xF0, 0x7E, 0x10, 0x06, 0x02, 0x41, 0x4D, 0x03, 0, 0, 0, 0, 0, 1, 0xF7]), Some((0x41, [0x4D, 3])));
    }

    #[test]
    fn a_link_reads_in_pieces_writes_in_pieces_and_identifies() {
        let mut device = fake::Device::new(HEAD, [0x4D, 3]);
        let patch: Vec<u8> = (0..475).map(|i| (i % 128) as u8).collect();
        device.fill(0x0601_0800, &patch);
        let mut link = Link::new(HEAD, &mut device);
        link.pause = Duration::ZERO;
        assert_eq!(link.read(0x0601_0800, 475).unwrap(), patch, "put together from four answers that came backwards, the echo passed over");
        assert_eq!(link.read(0x0601_0800 + 16, 1).unwrap(), [16]);
        link.write(0x0600_0800, &patch).unwrap();
        assert_eq!(link.identify().unwrap(), (0x41, [0x4D, 3]));
        assert_eq!(device.bytes_at(0x0600_0800, 475), patch);
        assert_eq!(device.received.iter().filter(|m| m.len() > 7 && m[7] == DT1).count(), 4, "475 bytes go in four data sets");
        // Nothing comes: said after the time allowed.
        let mut mute = fake::Device::new(Head { device_id: 0x11, ..HEAD }, [0, 0]);
        let mut link = Link::new(HEAD, &mut mute);
        link.timeout = Duration::from_millis(10);
        assert!(link.read(0, 3).unwrap_err().contains("0 of 3"));
        struct Silent;
        impl Port for Silent {
            fn send(&mut self, _: &[u8]) -> Result<(), String> {
                Ok(())
            }
            fn receive(&mut self, _: Duration) -> Option<Vec<u8>> {
                None
            }
        }
        let mut silent = Silent;
        let mut link = Link::new(HEAD, &mut silent);
        link.timeout = Duration::from_millis(10);
        assert!(link.identify().is_err());
    }
}
