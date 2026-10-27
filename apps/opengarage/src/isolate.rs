//! Plug-ins in a process of their own.
//!
//! A plug-in is somebody else's program. Loaded into the application it can
//! take the whole application down with it; run in a second process, it can
//! only take that process down: the track falls silent or goes through
//! untouched, and the project stays open.
//!
//! The second process is this same program started with `--plugin-host`. The
//! two talk over a connection of this computer alone (a TCP socket on
//! 127.0.0.1 that the application opens and the process calls): sound blocks
//! and notes one way, processed sound the other, plus the requests of the
//! interface (window, state, parameter values). The standard output is left
//! to the plug-in, which may print to it as it likes (it goes nowhere)
//! without getting a word into the conversation. The plug-in's own window
//! belongs to the child process; it is an ordinary window of the desktop.

use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use og_engine::ExternalUnit;
use serde::{Deserialize, Serialize};

use crate::clap_host::ParamInfo;
use crate::i18n::tr;
use crate::plugin::{from_hex, to_hex, Idle, PluginHandle};

/// First argument of the program when it is started to hold a plug-in.
pub const FLAG: &str = "--plugin-host";

/// Most sound sent in one message, in samples.
const BLOCK: usize = 4096;
/// Most a message may weigh.
const MAX_MESSAGE: usize = 256 * 1024 * 1024;

// Kinds of messages. Sound and notes are raw bytes; the rest is JSON.
const SOUND: u8 = 1;
const NOTE_ON: u8 = 2;
const NOTE_OFF: u8 = 3;
const PARAM: u8 = 4;
const NOTES_OFF: u8 = 5;
const CONTROL: u8 = 10;
const READY: u8 = 20;
/// The first word of the process: the secret it was started with.
const HELLO: u8 = 21;

/// What the interface asks of the plug-in's process.
#[derive(Serialize, Deserialize)]
enum Request {
    OpenWindow(String),
    CloseWindow,
    /// With the parameters whose values the interface shows.
    Idle(Vec<u32>),
    SaveState,
    LoadState(String),
    ParamChanged(u32, f64),
    Quit,
}

#[derive(Serialize, Deserialize)]
enum Reply {
    Done,
    Refused(String),
    Idle { closed: bool, open: bool, changes: Vec<(u32, f64)>, values: Vec<(u32, f64)> },
    State(Option<String>),
}

/// What the plug-in's process says once the plug-in has started, or why it has not.
#[derive(Serialize, Deserialize)]
enum Start {
    Ready { params: Vec<ParamInfo>, has_window: bool },
    Failed(String),
}

fn write_message(to: &mut impl Write, kind: u8, payload: &[u8]) -> std::io::Result<()> {
    to.write_all(&(payload.len() as u32).to_le_bytes())?;
    to.write_all(&[kind])?;
    to.write_all(payload)?;
    to.flush()
}

fn read_message(from: &mut impl Read) -> std::io::Result<(u8, Vec<u8>)> {
    let mut header = [0u8; 5];
    from.read_exact(&mut header)?;
    let length = u32::from_le_bytes([header[0], header[1], header[2], header[3]]) as usize;
    if length > MAX_MESSAGE {
        return Err(std::io::Error::other("message too large"));
    }
    let mut payload = vec![0u8; length];
    from.read_exact(&mut payload)?;
    Ok((header[4], payload))
}

// ----- the application's side -----------------------------------------------------------

/// The program to start for a plug-in: this one. The tests are a different
/// program; they use the application built next to them.
fn host_program() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    if !cfg!(test) {
        return Some(current);
    }
    let name = if cfg!(windows) { "opengaragemusic.exe" } else { "opengaragemusic" };
    Some(current.parent()?.parent()?.join(name))
}

/// A secret for the process just started, so that no other program that
/// finds the port can take its place.
fn secret() -> String {
    let random = RandomState::new();
    format!("{:016x}{:016x}", random.hash_one(0u8), random.hash_one(1u8))
}

/// Messages on their way to the process. Both faces of the plug-in write
/// through here, each from its own thread; one thread of its own does the
/// writing, so that the audio thread never waits for the connection.
type Writer = Sender<(u8, Vec<u8>)>;

/// Writes the messages in their order until the connection or the senders are gone.
fn write_from(mut stream: TcpStream, messages: Receiver<(u8, Vec<u8>)>, dead: Arc<AtomicBool>) {
    for (kind, payload) in messages {
        if write_message(&mut stream, kind, &payload).is_err() {
            dead.store(true, Ordering::Release);
            break;
        }
    }
    let _ = stream.shutdown(Shutdown::Write);
}

/// The plug-in's process. It is told to leave when both faces of the plug-in
/// are gone: an export keeps the audio face only.
struct Process {
    child: Mutex<Child>,
    writer: Writer,
    dead: Arc<AtomicBool>,
}

impl Process {
    fn end(&self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Asked to leave, given a moment, then made to.
        if !self.dead.load(Ordering::Acquire) {
            if let Ok(payload) = serde_json::to_vec(&Request::Quit) {
                let _ = self.writer.send((CONTROL, payload));
            }
            for _ in 0..50 {
                if self.child.lock().is_ok_and(|mut child| matches!(child.try_wait(), Ok(Some(_)))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        self.end();
    }
}

/// The audio face of a plug-in that runs in a process of its own.
pub struct IsolatedUnit {
    _process: Arc<Process>,
    writer: Writer,
    sound: Receiver<Vec<u8>>,
    /// The process is gone or has stopped answering.
    dead: Arc<AtomicBool>,
}

impl IsolatedUnit {
    fn send(&self, kind: u8, payload: Vec<u8>) {
        if self.dead.load(Ordering::Acquire) {
            return;
        }
        if self.writer.send((kind, payload)).is_err() {
            self.dead.store(true, Ordering::Release);
        }
    }
}

impl ExternalUnit for IsolatedUnit {
    /// A plug-in that is gone leaves the sound as it came: an effect is
    /// bypassed, an instrument is silent.
    fn process(&mut self, buf: &mut [f32]) {
        for chunk in buf.chunks_mut(BLOCK) {
            if self.dead.load(Ordering::Acquire) {
                return;
            }
            self.send(SOUND, chunk.iter().flat_map(|s| s.to_le_bytes()).collect());
            // Long enough for a slow plug-in, short enough not to freeze the sound for good.
            match self.sound.recv_timeout(Duration::from_secs(2)) {
                Ok(answer) if answer.len() == 4 * chunk.len() => {
                    let (words, _) = answer.as_chunks::<4>();
                    for (sample, bytes) in chunk.iter_mut().zip(words) {
                        let value = f32::from_le_bytes(*bytes);
                        *sample = if value.is_finite() { value } else { 0.0 };
                    }
                }
                _ => {
                    self.dead.store(true, Ordering::Release);
                    return;
                }
            }
        }
    }

    fn note_on(&mut self, pitch: u8, velocity: u8) {
        self.send(NOTE_ON, vec![pitch, velocity]);
    }

    fn note_off(&mut self, pitch: u8) {
        self.send(NOTE_OFF, vec![pitch]);
    }

    fn set_param(&mut self, id: u32, value: f64) {
        let mut payload = id.to_le_bytes().to_vec();
        payload.extend_from_slice(&value.to_le_bytes());
        self.send(PARAM, payload);
    }

    fn all_notes_off(&mut self) {
        self.send(NOTES_OFF, Vec::new());
    }
}

/// The main-thread face of a plug-in that runs in a process of its own.
pub struct IsolatedHandle {
    process: Arc<Process>,
    /// Number of the process, to let it bring its window to the front.
    process_id: u32,
    writer: Writer,
    replies: Receiver<Vec<u8>>,
    dead: Arc<AtomicBool>,
    /// The loss of the process has been told to the application.
    reported: bool,
    has_window: bool,
    open: bool,
    /// Values of the parameters the interface shows, as of the last idle call,
    /// and the parameters it asked about since.
    values: HashMap<u32, f64>,
    shown: std::cell::RefCell<Vec<u32>>,
}

impl IsolatedHandle {
    fn ask(&self, request: &Request, patience: Duration) -> Option<Reply> {
        if self.dead.load(Ordering::Acquire) {
            return None;
        }
        let payload = serde_json::to_vec(request).ok()?;
        let sent = self.writer.send((CONTROL, payload)).ok();
        let answer = sent.and_then(|()| match self.replies.recv_timeout(patience) {
            Ok(answer) => serde_json::from_slice(&answer).ok(),
            Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => None,
        });
        if answer.is_none() {
            self.dead.store(true, Ordering::Release);
        }
        answer
    }

    /// Loads what `save_state` wrote.
    pub fn load_state(&self, data: &[u8]) -> bool {
        matches!(self.ask(&Request::LoadState(to_hex(data)), Duration::from_secs(20)), Some(Reply::Done))
    }
}

impl PluginHandle for IsolatedHandle {
    fn has_window(&self) -> bool {
        self.has_window && !self.dead.load(Ordering::Acquire)
    }

    fn window_open(&self) -> bool {
        self.open && !self.dead.load(Ordering::Acquire)
    }

    fn open_window(&mut self, title: &str) -> Result<(), String> {
        allow_foreground(self.process_id);
        match self.ask(&Request::OpenWindow(title.to_owned()), Duration::from_secs(20)) {
            Some(Reply::Done) => {
                self.open = true;
                Ok(())
            }
            Some(Reply::Refused(reason)) => Err(reason),
            _ => Err(tr("the plug-in's process stopped answering").to_owned()),
        }
    }

    fn close_window(&mut self) {
        self.ask(&Request::CloseWindow, Duration::from_secs(5));
        self.open = false;
    }

    fn idle(&mut self) -> Idle {
        let mut result = Idle::default();
        let shown = std::mem::take(&mut *self.shown.borrow_mut());
        if let Some(Reply::Idle { closed, open, changes, values }) = self.ask(&Request::Idle(shown), Duration::from_secs(5)) {
            self.open = open;
            self.values = values.into_iter().collect();
            (result.closed, result.changes) = (closed, changes);
        }
        if self.dead.load(Ordering::Acquire) && !self.reported {
            self.reported = true;
            self.open = false;
            // Whatever is left of the process goes.
            self.process.end();
            result.failed = Some(tr("its process stopped; the track goes on without it").to_owned());
        }
        result
    }

    fn param_value(&self, id: u32) -> Option<f64> {
        // Asked for now, answered at the next idle call.
        let mut shown = self.shown.borrow_mut();
        if !shown.contains(&id) && shown.len() < 256 {
            shown.push(id);
        }
        self.values.get(&id).copied()
    }

    fn param_changed(&mut self, id: u32, value: f64) {
        self.values.insert(id, value);
        self.ask(&Request::ParamChanged(id, value), Duration::from_secs(5));
    }

    fn save_state(&self) -> Option<Vec<u8>> {
        match self.ask(&Request::SaveState, Duration::from_secs(20))? {
            Reply::State(state) => from_hex(&state?),
            _ => None,
        }
    }

    fn load_state(&self, data: &[u8]) -> bool {
        IsolatedHandle::load_state(self, data)
    }

    #[cfg(test)]
    fn window_handle(&self) -> Option<*mut std::ffi::c_void> {
        None
    }
}

impl Drop for IsolatedHandle {
    fn drop(&mut self) {
        // The window does not outlive the face that shows it.
        if self.open && !self.dead.load(Ordering::Acquire) {
            self.ask(&Request::CloseWindow, Duration::from_secs(5));
        }
    }
}

/// Lets the plug-in's process bring its window to the front.
fn allow_foreground(process: u32) {
    #[cfg(windows)]
    // SAFETY: a plain permission given to a process we started.
    unsafe {
        let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(process);
    }
    #[cfg(not(windows))]
    let _ = process;
}

/// Waits for the process just started to call and to say the secret; gives
/// up when it dies or after half a minute. Anybody else who calls is not
/// listened to.
fn answer(listener: &TcpListener, child: &mut Child, secret: &str) -> Result<TcpStream, String> {
    let stopped = || tr("the plug-in's process stopped while starting the plug-in").to_owned();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                if matches!(read_message(&mut stream), Ok((HELLO, word)) if word == secret.as_bytes()) {
                    let _ = stream.set_read_timeout(None);
                    let _ = stream.set_nodelay(true);
                    return Ok(stream);
                }
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                if child.try_wait().is_ok_and(|status| status.is_some()) || Instant::now() > until {
                    return Err(stopped());
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(tf!("the plug-in's process could not be started ({e})" [e])),
        }
    }
}

/// Starts a plug-in in a process of its own. Returns its two faces and its parameters.
pub fn start(file: &Path, plugin_id: &str, sample_rate: f64) -> Result<(IsolatedUnit, IsolatedHandle, Vec<ParamInfo>), String> {
    let program = host_program().filter(|p| p.is_file()).ok_or(tr("the program that holds plug-ins was not found"))?;
    let could_not = |e: std::io::Error| tf!("the plug-in's process could not be started ({e})" [e]);
    // A port of this computer alone, chosen by the system, told to the process.
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(could_not)?;
    let port = listener.local_addr().map_err(could_not)?.port();
    let secret = secret();
    let mut command = Command::new(program);
    command.arg(FLAG).arg(file).arg(plugin_id).arg(sample_rate.to_string()).arg(crate::i18n::language()).arg(port.to_string()).arg(&secret);
    // Whatever the plug-in prints goes nowhere.
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // No console window for the second process.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(could_not)?;
    let stream = match answer(&listener, &mut child, &secret).and_then(|stream| stream.try_clone().map(|reading| (stream, reading)).map_err(|e| e.to_string())) {
        Ok(ends) => ends,
        Err(reason) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(reason);
        }
    };
    let (stream, mut reading) = stream;

    // One thread reads what the process says and sorts it: sound for the audio
    // thread, the rest for the interface. Another writes what both send it.
    let dead = Arc::new(AtomicBool::new(false));
    let (sound_out, sound) = mpsc::channel();
    let (reply_out, replies) = mpsc::channel();
    let (start_out, started) = mpsc::channel();
    let reader_dead = Arc::clone(&dead);
    std::thread::spawn(move || {
        while let Ok((kind, payload)) = read_message(&mut reading) {
            let delivered = match kind {
                SOUND => sound_out.send(payload).is_ok(),
                READY => start_out.send(payload).is_ok(),
                _ => reply_out.send(payload).is_ok(),
            };
            if !delivered {
                break;
            }
        }
        reader_dead.store(true, Ordering::Release);
    });
    let (writer, to_write) = mpsc::channel();
    let writer_dead = Arc::clone(&dead);
    std::thread::spawn(move || write_from(stream, to_write, writer_dead));

    let first = started.recv_timeout(Duration::from_secs(30)).ok().and_then(|payload| serde_json::from_slice::<Start>(&payload).ok());
    let (params, has_window) = match first {
        Some(Start::Ready { params, has_window }) => (params, has_window),
        other => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(match other {
                Some(Start::Failed(reason)) => reason,
                _ => tr("the plug-in's process stopped while starting the plug-in").to_owned(),
            });
        }
    };
    let process_id = child.id();
    let process = Arc::new(Process { child: Mutex::new(child), writer: writer.clone(), dead: Arc::clone(&dead) });
    let unit = IsolatedUnit { _process: Arc::clone(&process), writer: writer.clone(), sound, dead: Arc::clone(&dead) };
    let handle = IsolatedHandle {
        process,
        process_id,
        writer,
        replies,
        dead,
        reported: false,
        has_window,
        open: false,
        values: HashMap::new(),
        shown: std::cell::RefCell::default(),
    };
    Ok((unit, handle, params))
}

// ----- the plug-in's side ------------------------------------------------------------------

/// Lets the windows of this process live: without this they would never draw
/// nor answer the mouse.
fn pump_window_messages() {
    #[cfg(windows)]
    // SAFETY: the ordinary message loop of a thread that owns windows.
    unsafe {
        use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Runs as the process that holds one plug-in. `args` follow the flag: the
/// plug-in's file, its id in the file, the sample rate, the language, the
/// port the application listens on and the secret to say first.
pub fn serve(args: &[String]) {
    let (Some(file), Some(plugin_id), Some(rate)) = (args.first(), args.get(1), args.get(2).and_then(|r| r.parse::<f64>().ok())) else { return };
    let (Some(port), Some(secret)) = (args.get(4).and_then(|p| p.parse::<u16>().ok()), args.get(5)) else { return };
    crate::i18n::set_language(args.get(3).map_or("", String::as_str));
    let Ok(stream) = TcpStream::connect((Ipv4Addr::LOCALHOST, port)) else { return };
    let _ = stream.set_nodelay(true);
    let Ok(mut input) = stream.try_clone() else { return };
    let output = Arc::new(Mutex::new(stream));
    let say = |kind: u8, payload: &[u8]| {
        if let Ok(mut output) = output.lock() {
            let _ = write_message(&mut *output, kind, payload);
        }
    };
    say(HELLO, secret.as_bytes());

    let (mut unit, mut handle, params) = match crate::plugin::load(Path::new(file), plugin_id, rate) {
        Ok(loaded) => loaded,
        Err(reason) => {
            say(READY, &serde_json::to_vec(&Start::Failed(reason)).unwrap_or_default());
            return;
        }
    };
    let ready = Start::Ready { params, has_window: handle.as_ref().is_some_and(|h| h.has_window()) };
    say(READY, &serde_json::to_vec(&ready).unwrap_or_default());

    // One thread reads what the application sends. Sound and notes are dealt
    // with there and then; the requests of the interface go to the main
    // thread, which is the one that may touch the plug-in's window.
    let (request_out, requests) = mpsc::channel::<Request>();
    let sound_output = Arc::clone(&output);
    std::thread::spawn(move || {
        let mut samples: Vec<f32> = Vec::with_capacity(BLOCK);
        let mut bytes: Vec<u8> = Vec::with_capacity(4 * BLOCK);
        while let Ok((kind, payload)) = read_message(&mut input) {
            match (kind, payload.as_slice()) {
                (SOUND, data) if data.len() <= 4 * BLOCK => {
                    samples.clear();
                    samples.extend(data.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])));
                    unit.process(&mut samples);
                    bytes.clear();
                    bytes.extend(samples.iter().flat_map(|s| s.to_le_bytes()));
                    let sent = sound_output.lock().map_err(|_| ()).and_then(|mut output| write_message(&mut *output, SOUND, &bytes).map_err(|_| ()));
                    if sent.is_err() {
                        break;
                    }
                }
                (NOTE_ON, [pitch, velocity]) => unit.note_on(*pitch, *velocity),
                (NOTE_OFF, [pitch]) => unit.note_off(*pitch),
                (PARAM, data) if data.len() == 12 => {
                    let id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
                    let value = f64::from_le_bytes([data[4], data[5], data[6], data[7], data[8], data[9], data[10], data[11]]);
                    unit.set_param(id, value);
                }
                (NOTES_OFF, _) => unit.all_notes_off(),
                (CONTROL, data) => {
                    if let Ok(request) = serde_json::from_slice::<Request>(data) {
                        if request_out.send(request).is_err() {
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        // The application is gone or has closed the conversation: so does this process.
        drop(request_out);
    });

    // What happened in the plug-in between two questions of the application.
    let (mut closed, mut changes): (bool, Vec<(u32, f64)>) = (false, Vec::new());
    loop {
        pump_window_messages();
        if let Some(handle) = &mut handle {
            let idle = handle.idle();
            closed |= idle.closed;
            changes.extend(idle.changes);
            changes.truncate(4096);
        }
        loop {
            let request = match requests.try_recv() {
                Ok(request) => request,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => return,
            };
            let reply = match (request, &mut handle) {
                (Request::Quit, _) => return,
                (Request::OpenWindow(title), Some(handle)) => match handle.open_window(&title) {
                    Ok(()) => Reply::Done,
                    Err(reason) => Reply::Refused(reason),
                },
                (Request::OpenWindow(_), None) => Reply::Refused(tr("this plug-in has no window that can be used here").to_owned()),
                (Request::CloseWindow, handle) => {
                    if let Some(handle) = handle {
                        handle.close_window();
                    }
                    Reply::Done
                }
                (Request::Idle(shown), handle) => Reply::Idle {
                    closed: std::mem::take(&mut closed),
                    open: handle.as_ref().is_some_and(|h| h.window_open()),
                    changes: std::mem::take(&mut changes),
                    values: shown.into_iter().filter_map(|id| Some((id, handle.as_ref()?.param_value(id)?))).collect(),
                },
                (Request::SaveState, handle) => Reply::State(handle.as_ref().and_then(|h| h.save_state()).map(|state| to_hex(&state))),
                (Request::LoadState(state), handle) => {
                    let loaded = from_hex(&state).zip(handle.as_ref()).is_some_and(|(bytes, handle)| handle.load_state(&bytes));
                    if loaded { Reply::Done } else { Reply::Refused(String::new()) }
                }
                (Request::ParamChanged(id, value), handle) => {
                    if let Some(handle) = handle {
                        handle.param_changed(id, value);
                    }
                    Reply::Done
                }
            };
            say(CONTROL, &serde_json::to_vec(&reply).unwrap_or_default());
        }
        std::thread::sleep(Duration::from_millis(3));
    }
}
