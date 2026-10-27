//! The MIDI ports of the computer, for the MIDI Pilot: an output that the
//! songs and the pedal modules send to, and an input that the modules read
//! the pedals' answers from. Windows lets one program at a time use a port.

use std::sync::mpsc;
use std::time::Duration;

use og_engine::roland::Port;

use crate::i18n::tr;

/// The names of the MIDI outputs of the computer.
pub fn midi_outputs() -> Vec<String> {
    let Ok(output) = midir::MidiOutput::new("OpenGarageMusic") else { return Vec::new() };
    output.ports().iter().filter_map(|port| output.port_name(port).ok()).collect()
}

/// The names of the MIDI inputs of the computer.
pub fn midi_inputs() -> Vec<String> {
    let Ok(input) = midir::MidiInput::new("OpenGarageMusic") else { return Vec::new() };
    input.ports().iter().filter_map(|port| input.port_name(port).ok()).collect()
}

/// A MIDI output, open.
pub struct MidiOut {
    connection: midir::MidiOutputConnection,
    pub name: String,
}

impl MidiOut {
    pub fn open(name: &str) -> Result<MidiOut, String> {
        let output = midir::MidiOutput::new("OpenGarageMusic").map_err(|e| e.to_string())?;
        let port = output.ports().into_iter().find(|port| output.port_name(port).is_ok_and(|n| n == name)).ok_or_else(|| tf!("the MIDI output “{name}” is not connected" [name]))?;
        let connection = output.connect(&port, "OpenGarageMusic").map_err(|e| tf!("the MIDI output “{name}” cannot be opened ({e}): is another program using it?" [name, e]))?;
        Ok(MidiOut { connection, name: name.to_owned() })
    }

    pub fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.connection.send(bytes).map_err(|e| e.to_string())
    }
}

/// A MIDI input, open: every message that comes, system exclusive ones
/// included, waits in `messages`.
pub struct MidiIn {
    _connection: midir::MidiInputConnection<()>,
    pub messages: mpsc::Receiver<Vec<u8>>,
}

impl MidiIn {
    pub fn open(name: &str) -> Result<MidiIn, String> {
        let mut input = midir::MidiInput::new("OpenGarageMusic").map_err(|e| e.to_string())?;
        input.ignore(midir::Ignore::None);
        let port = input.ports().into_iter().find(|port| input.port_name(port).is_ok_and(|n| n == name)).ok_or_else(|| tf!("the MIDI input “{name}” is not connected" [name]))?;
        let (sender, messages) = mpsc::channel();
        let connection = input
            .connect(
                &port,
                "OpenGarageMusic pedal",
                move |_stamp, message, ()| {
                    let _ = sender.send(message.to_vec());
                },
                (),
            )
            .map_err(|e| tf!("the MIDI input “{name}” cannot be opened ({e}): is another program using it?" [name, e]))?;
        Ok(MidiIn { _connection: connection, messages })
    }
}

/// An output and an input together: a pedal talked to, both ways.
pub struct Ports {
    pub output: MidiOut,
    pub input: MidiIn,
}

impl Ports {
    pub fn open(output: &str, input: &str) -> Result<Ports, String> {
        if output.is_empty() {
            return Err(tr("no MIDI output chosen (Settings > MIDI output)").to_owned());
        }
        if input.is_empty() {
            return Err(tr("no MIDI input chosen (Settings > MIDI input)").to_owned());
        }
        Ok(Ports { output: MidiOut::open(output)?, input: MidiIn::open(input)? })
    }
}

impl Port for Ports {
    fn send(&mut self, message: &[u8]) -> Result<(), String> {
        self.output.send(message)
    }

    fn receive(&mut self, wait: Duration) -> Option<Vec<u8>> {
        self.input.messages.recv_timeout(wait).ok()
    }
}
