//! The application as a program of its own. Its presence also makes sure the
//! program is built before the unit tests run: they start it to hold plug-ins.

use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::process::{Command, Stdio};
use std::time::Duration;

/// One message of the conversation: its kind and its payload.
fn read_message(from: &mut impl Read) -> (u8, Vec<u8>) {
    let mut header = [0u8; 5];
    from.read_exact(&mut header).expect("a message header");
    let mut payload = vec![0u8; u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize];
    from.read_exact(&mut payload).expect("the message");
    (header[4], payload)
}

#[test]
fn started_to_hold_a_plugin_that_does_not_exist_it_says_so_and_leaves() {
    // The application listens on a port of this computer and tells the process a secret.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port().to_string();
    let mut child = Command::new(env!("CARGO_BIN_EXE_opengaragemusic"))
        .args(["--plugin-host", "nothing-here.clap", "x", "48000", "en", &port, "open-sesame"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("the program starts");
    let (mut stream, _) = listener.accept().expect("the process calls");
    stream.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
    // First the secret, then the outcome of starting (kind 20) as JSON.
    let (kind, word) = read_message(&mut stream);
    assert_eq!((kind, word.as_slice()), (21, b"open-sesame".as_slice()));
    let (kind, said) = read_message(&mut stream);
    assert_eq!(kind, 20);
    assert!(String::from_utf8_lossy(&said).contains("Failed"), "{}", String::from_utf8_lossy(&said));
    assert!(child.wait().expect("the process ends").success());
}
