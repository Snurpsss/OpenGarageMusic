//! The local API of the MIDI Pilot, for an agent (an AI's tool) running on
//! the same computer: HTTP on `127.0.0.1` only, a token in every request,
//! JSON bodies in the format of the scripts (`og_engine::script`). A thread
//! takes the connections and hands each request to the window's thread,
//! which answers once the pedal has done what was asked; a write into a
//! pedal waits for the user's yes in the window, unless they allowed
//! unattended writes.

#![allow(clippy::large_enum_variant, clippy::result_large_err)]

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use eframe::egui;
use og_engine::devices::{self, Description};
use og_engine::script::{self, Action, Script};
use serde_json::{json, Value as Json};

use crate::app::App;
use crate::i18n::tr;
use crate::pedal::Job;

/// The port the API takes when the user turns it on without choosing one.
pub const DEFAULT_PORT: u16 = 7311;
/// Most bytes in a request's body.
const MOST_BODY: usize = 1 << 20;
/// How long a request waits for the pedal, or the user, before 202 is answered.
const PATIENCE: Duration = Duration::from_secs(120);

/// A request, as the window's thread receives it.
pub struct Request {
    pub method: String,
    pub path: String,
    pub body: String,
    reply: mpsc::Sender<Response>,
}

/// What is answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub body: String,
}

impl Response {
    pub fn json(status: u16, value: Json) -> Response {
        Response { status, content_type: "application/json", body: value.to_string() }
    }

    pub fn error(status: u16, text: impl Into<String>) -> Response {
        Response::json(status, json!({ "error": text.into() }))
    }

    pub fn text(body: String) -> Response {
        Response { status: 200, content_type: "text/markdown", body }
    }
}

/// The server: its thread ends when it is dropped.
pub struct Server {
    pub port: u16,
    requests: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
    pub ctx: egui::Context,
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// A token nobody guesses: time and a hash, 32 hexadecimal characters.
pub fn new_token() -> String {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let a = crate::sessions::hash64(&format!("token {nanos} {}", std::process::id()));
    let b = crate::sessions::hash64(&format!("{a:x} {nanos}"));
    format!("{a:016x}{b:016x}")
}

/// Starts the server on `127.0.0.1:port` (0 for a port the system picks).
pub fn start(port: u16, token: String, ctx: egui::Context) -> Result<Server, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|e| tf!("the API cannot listen on port {port}: {e}" [port, e]))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    listener.set_nonblocking(true).map_err(|e| e.to_string())?;
    let (sender, requests) = mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let (stopped, expected, repaint) = (Arc::clone(&stop), token.clone(), ctx.clone());
    std::thread::spawn(move || {
        while !stopped.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, _)) => {
                    serve_one(stream, &expected, &sender);
                    repaint.request_repaint();
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                Err(_) => std::thread::sleep(Duration::from_millis(200)),
            }
        }
    });
    Ok(Server { port, requests, stop, ctx })
}

/// One connection: the request read, checked, handed over, its answer written.
fn serve_one(mut stream: TcpStream, token: &str, sender: &mpsc::Sender<Request>) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(10)));
    let response = match read_request(&mut stream) {
        Err(e) => Response::error(400, e),
        Ok((method, path, headers, body)) => {
            let given = headers.get("authorization").map(|a| a.trim_start_matches("Bearer ").trim());
            if given != Some(token) {
                Response::error(401, "the token is missing or wrong: Authorization: Bearer <token>")
            } else {
                let (reply, answer) = mpsc::channel();
                if sender.send(Request { method, path, body, reply }).is_err() {
                    Response::error(503, "the MIDI Pilot is closed")
                } else {
                    answer.recv_timeout(PATIENCE).unwrap_or_else(|_| Response::json(202, json!({ "waiting": "the user's confirmation, or the pedal" })))
                }
            }
        }
    };
    write_response(&mut stream, &response);
}

/// The request line, the headers (names in lower case) and the body.
fn read_request(stream: &mut TcpStream) -> Result<(String, String, BTreeMap<String, String>, String), String> {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    let head_end = loop {
        let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("the request ended early".to_owned());
        }
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(at) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break at + 4;
        }
        if bytes.len() > 64 * 1024 {
            return Err("the headers are too long".to_owned());
        }
    };
    let head = String::from_utf8_lossy(&bytes[..head_end]).into_owned();
    let mut lines = head.lines();
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or_default().to_owned(), parts.next().unwrap_or_default().to_owned());
    if method.is_empty() || !path.starts_with('/') {
        return Err("not an HTTP request".to_owned());
    }
    let mut headers = BTreeMap::new();
    for line in lines {
        if let Some((name, value)) = line.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let length: usize = headers.get("content-length").and_then(|l| l.parse().ok()).unwrap_or(0);
    if length > MOST_BODY {
        return Err("the body is too long".to_owned());
    }
    let mut body = bytes[head_end..].to_vec();
    while body.len() < length {
        let n = stream.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&buffer[..n]);
    }
    body.truncate(length);
    Ok((method, path, headers, String::from_utf8_lossy(&body).into_owned()))
}

fn write_response(stream: &mut TcpStream, response: &Response) {
    let reason = match response.status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Unknown",
    };
    let head = format!("HTTP/1.1 {} {reason}\r\nContent-Type: {}; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nAccess-Control-Allow-Origin: null\r\n\r\n", response.status, response.content_type, response.body.len());
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(response.body.as_bytes());
    let _ = stream.flush();
}

// ----- what the window's thread does with a request -----------------------------------------

/// A write into a pedal, once the user says yes.
#[derive(Clone, Debug)]
pub enum PedalWrite {
    Memory { number: usize, raw: Vec<u8> },
    PcMap(Vec<u16>),
    System(Vec<u8>),
}

/// What a request waits for.
enum Step {
    /// The user's yes or no, shown in the window.
    Confirm(PedalWrite),
    /// A read asked of the pedal; then this.
    Reading(Then),
    /// A write asked of the pedal.
    Writing,
    /// The page of the scripts is applying a script.
    Script,
}

/// What comes after a read.
enum Then {
    Memory(usize),
    Names,
    PcMap,
    System,
    /// The memory read, the action is made and written.
    Action(Action),
}

/// A request not answered yet.
pub struct Pending {
    request: Request,
    step: Step,
    since: Instant,
}

impl Pending {
    /// The question the window asks the user, when there is one.
    pub fn question(&self, description: Option<&Description>) -> Option<String> {
        let Step::Confirm(write) = &self.step else { return None };
        let pedal = description.map_or_else(String::new, |d| d.name.clone());
        Some(match write {
            PedalWrite::Memory { number, .. } => tf!("An AI asks to write the memory {} of the {pedal}." [pedal], description.map_or_else(|| number.to_string(), |d| d.label(*number))),
            PedalWrite::PcMap(_) => tf!("An AI asks to write the program change map of the {pedal}." [pedal]),
            PedalWrite::System(_) => tf!("An AI asks to write the system settings of the {pedal}." [pedal]),
        })
    }
}

/// The description a pedal path names, and the device of the chain that has it.
fn pedal_of(app: &App, key: &str) -> Result<(Description, usize), Response> {
    let description = crate::pedal::description_of(key).ok_or_else(|| Response::error(404, format!("no pedal \"{key}\": one of {}", crate::pedal::descriptions().iter().map(|d| d.key.clone()).collect::<Vec<_>>().join(", "))))?;
    let device = app.settings.set_list.devices.iter().position(|d| d.model == key).ok_or_else(|| Response::error(404, format!("no device of model {key} in the chain: add one under Devices")))?;
    Ok((description, device))
}

/// A parameter of a description, for the API.
fn param_json(d: &Description, p: &devices::Param) -> Json {
    let mut value = json!({ "name": p.name, "group": p.group });
    match d.labels_of(p) {
        Some(labels) => value["values"] = json!(labels),
        None if p.enc == devices::Encoding::Ascii => value["text"] = json!(p.bytes),
        None => {
            value["min"] = json!(p.min - p.offset);
            value["max"] = json!(p.max - p.offset);
            if let Some(shown) = &p.shown {
                value["shown"] = json!(shown);
            }
        }
    }
    value
}

impl App {
    /// Starts the API when the settings ask for it; says what happened.
    pub fn start_api(&mut self, ctx: &egui::Context) {
        self.setlist.api = None;
        let port = self.settings.set_list.api_port;
        if port == 0 {
            return;
        }
        if self.settings.set_list.api_token.is_empty() {
            self.settings.set_list.api_token = new_token();
            self.save_settings();
        }
        match start(port, self.settings.set_list.api_token.clone(), ctx.clone()) {
            Ok(server) => {
                self.setlist.message = tf!("The API listens on http://127.0.0.1:{}/ for an AI's agent.", server.port);
                self.setlist.api = Some(server);
            }
            Err(e) => self.setlist.message = e,
        }
    }

    /// Starts the API on a port the system picks (the tests), whatever the settings.
    #[cfg(test)]
    pub fn start_api_on_any_port(&mut self, ctx: &egui::Context) -> u16 {
        self.settings.set_list.api_token = new_token();
        let server = start(0, self.settings.set_list.api_token.clone(), ctx.clone()).expect("a free port");
        let port = server.port;
        self.setlist.api = Some(server);
        port
    }

    /// Once a frame: takes the requests that came, and goes on with the one waiting.
    pub fn poll_api(&mut self) {
        let Some(server) = &self.setlist.api else { return };
        let came: Vec<Request> = server.requests.try_iter().collect();
        let ctx = server.ctx.clone();
        for request in came {
            if self.setlist.api_pending.is_some() {
                let _ = request.reply.send(Response::error(409, "another request is being done; try again"));
                continue;
            }
            match self.handle_api(request, &ctx) {
                Ok(()) => {}
                Err((request, response)) => {
                    let _ = request.reply.send(response);
                }
            }
        }
        self.step_api(&ctx);
    }

    /// Answers a request, or keeps it until the pedal or the user has answered.
    fn handle_api(&mut self, request: Request, ctx: &egui::Context) -> Result<(), (Request, Response)> {
        let (full_path, method) = (request.path.clone(), request.method.clone());
        let parts: Vec<String> = full_path.trim_end_matches('/').split('/').filter(|p| !p.is_empty()).map(str::to_owned).collect();
        let path: Vec<&str> = parts.iter().map(String::as_str).collect();
        let method = method.as_str();
        let answer = |request: Request, response: Response| -> Result<(), (Request, Response)> { Err((request, response)) };
        match (method, path.as_slice()) {
            ("GET", []) => {
                let pedals: Vec<String> = crate::pedal::descriptions().iter().map(|d| d.key.clone()).collect();
                let devices: Vec<Json> = self.settings.set_list.devices.iter().map(|d| json!({ "name": d.name, "model": d.model, "channel": d.channel + 1 })).collect();
                answer(request, Response::json(200, json!({ "program": "OpenGarageMusic", "version": env!("CARGO_PKG_VERSION"), "format": script::FORMAT, "pedals": pedals, "devices": devices, "routes": ["GET /guide", "GET /devices", "GET /pedals/{key}", "GET /pedals/{key}/memories", "GET|PUT /pedals/{key}/memories/{01A}", "POST /pedals/{key}/try", "GET|PUT /pedals/{key}/pc_map", "GET|PUT /pedals/{key}/system", "GET /g3s", "POST /g3s/actions", "POST /g3s/save", "GET /songs", "GET /pedalboard", "POST /actions"] })))
            }
            ("GET", ["guide"]) => answer(request, Response::text(crate::script::guide_text())),
            ("GET", ["songs"]) => {
                let list = &self.settings.set_list;
                let switcher = json!({ "channel": list.channel + 1, "message": crate::setlist::message_name(list.message), "numbers_from_one": list.numbers_from_one });
                answer(request, Response::json(200, json!({ "opengaragemusic": script::FORMAT, "mode": format!("{:?}", list.mode).to_ascii_lowercase(), "switcher": switcher, "actions": script::export_songs(list) })))
            }
            ("GET", ["pedalboard"]) => answer(request, Response::json(200, serde_json::to_value(self.pedalboard_script()).unwrap_or_default())),
            ("GET", ["devices"]) => {
                let devices: Vec<Json> = self.settings.set_list.devices.iter().map(|d| json!({ "name": d.name, "model": d.model, "channel": d.channel + 1 })).collect();
                answer(request, Response::json(200, json!(devices)))
            }
            ("GET", ["pedals", key]) => match crate::pedal::description_of(key) {
                Some(d) => {
                    let patch: Vec<Json> = d.patch.iter().map(|p| param_json(&d, p)).collect();
                    let system: Vec<Json> = d.system.iter().map(|p| param_json(&d, p)).collect();
                    answer(request, Response::json(200, json!({ "key": d.key, "name": d.name, "memories": d.patches(), "first": d.label(0), "last": d.label(d.patches() - 1), "patch": patch, "system": system })))
                }
                None => answer(request, Response::error(404, format!("no pedal \"{key}\""))),
            },
            ("GET", ["pedals", key, "memories"]) => {
                let (description, device) = match pedal_of(self, key) {
                    Ok(found) => found,
                    Err(response) => return answer(request, response),
                };
                if let Err(response) = self.api_pedal_session(device, ctx) {
                    return answer(request, response);
                }
                let session = self.setlist.pedal.as_mut().expect("opened");
                if session.names.len() == description.patches() && session.names.iter().any(|n| !n.is_empty()) {
                    let names: BTreeMap<String, String> = session.names.iter().enumerate().map(|(n, name)| (description.label(n), name.clone())).collect();
                    return answer(request, Response::json(200, json!(names)));
                }
                session.ask(Job::ReadNames);
                self.setlist.api_pending = Some(Pending { request, step: Step::Reading(Then::Names), since: Instant::now() });
                Ok(())
            }
            ("GET" | "PUT", ["pedals", key, "memories", label]) => {
                let (description, device) = match pedal_of(self, key) {
                    Ok(found) => found,
                    Err(response) => return answer(request, response),
                };
                let Some(number) = description.number(label) else { return answer(request, Response::error(404, format!("\"{label}\" is no memory of the {}", description.name))) };
                let then = if method == "GET" {
                    Then::Memory(number)
                } else {
                    let body: Json = match serde_json::from_str(&request.body) {
                        Ok(body) => body,
                        Err(e) => return answer(request, Response::error(400, format!("the body is not JSON: {e}"))),
                    };
                    let mut action: Action = match serde_json::from_value(body) {
                        Ok(action) => action,
                        Err(e) => return answer(request, Response::error(400, format!("the body is not an action: {e}"))),
                    };
                    action.pedal = Some(key.to_string());
                    action.memory = Some(description.label(number));
                    if let Err(e) = script::patch_from(&description, &action, None) {
                        return answer(request, Response::error(400, e));
                    }
                    Then::Action(action)
                };
                if let Err(response) = self.api_pedal_session(device, ctx) {
                    return answer(request, response);
                }
                let session = self.setlist.pedal.as_mut().expect("opened");
                session.ask(Job::ReadPatch(number));
                self.setlist.api_pending = Some(Pending { request, step: Step::Reading(then), since: Instant::now() });
                Ok(())
            }
            ("POST", ["pedals", key, "try"]) => {
                let (description, device) = match pedal_of(self, key) {
                    Ok(found) => found,
                    Err(response) => return answer(request, response),
                };
                let set: BTreeMap<String, Json> = match serde_json::from_str::<Json>(&request.body).ok().and_then(|b| b.get("set").cloned()).and_then(|s| serde_json::from_value(s).ok()) {
                    Some(set) => set,
                    None => return answer(request, Response::error(400, "a body { \"set\": { … } }")),
                };
                if let Err(response) = self.api_pedal_session(device, ctx) {
                    return answer(request, response);
                }
                let session = self.setlist.pedal.as_mut().expect("opened");
                let Some(mut raw) = session.raw.clone() else { return answer(request, Response::error(409, "no memory is chosen in the module yet: GET a memory first")) };
                if let Err(e) = script::apply_set(&description, &description.patch, &mut raw, &set) {
                    return answer(request, Response::error(400, e));
                }
                session.raw = Some(raw.clone());
                session.edited = true;
                let slot = session.slot();
                session.ask(Job::SendTemporary { slot, raw });
                self.setlist.api_pending = Some(Pending { request, step: Step::Writing, since: Instant::now() });
                Ok(())
            }
            ("GET" | "PUT", ["pedals", key, what @ ("pc_map" | "system")]) => {
                let (description, device) = match pedal_of(self, key) {
                    Ok(found) => found,
                    Err(response) => return answer(request, response),
                };
                if let Err(response) = self.api_pedal_session(device, ctx) {
                    return answer(request, response);
                }
                let session = self.setlist.pedal.as_mut().expect("opened");
                let is_map = *what == "pc_map";
                let known = if is_map { !session.pc_map.is_empty() } else { !session.system.is_empty() };
                if method == "GET" {
                    if known {
                        let response = if is_map { pc_map_response(&description, &session.pc_map) } else { Response::json(200, json!(script::export_values(&description, &description.system, &session.system))) };
                        return answer(request, response);
                    }
                    session.ask(if is_map { Job::ReadPcMap } else { Job::ReadSystem });
                    self.setlist.api_pending = Some(Pending { request, step: Step::Reading(if is_map { Then::PcMap } else { Then::System }), since: Instant::now() });
                    return Ok(());
                }
                let body: Json = match serde_json::from_str(&request.body) {
                    Ok(body) => body,
                    Err(e) => return answer(request, Response::error(400, format!("the body is not JSON: {e}"))),
                };
                let write = if is_map {
                    let map: BTreeMap<String, String> = match serde_json::from_value(body) {
                        Ok(map) => map,
                        Err(e) => return answer(request, Response::error(400, format!("a body {{ \"program\": \"memory\" }}: {e}"))),
                    };
                    let entries = match script::pc_map_from(&description, &map) {
                        Ok(entries) => entries,
                        Err(e) => return answer(request, Response::error(400, e)),
                    };
                    let mut whole = session.pc_map.clone();
                    whole.resize(description.pc_map_entries, 0);
                    for (index, number) in entries {
                        whole[index] = number;
                    }
                    PedalWrite::PcMap(whole)
                } else {
                    let set: BTreeMap<String, Json> = match serde_json::from_value(body) {
                        Ok(set) => set,
                        Err(e) => return answer(request, Response::error(400, format!("a body {{ \"setting\": value }}: {e}"))),
                    };
                    let mut raw = session.system.clone();
                    raw.resize(description.system_size(), 0);
                    if let Err(e) = script::apply_set(&description, &description.system, &mut raw, &set) {
                        return answer(request, Response::error(400, e));
                    }
                    PedalWrite::System(raw)
                };
                if !known {
                    // Read first, so that the entries not given keep what the pedal holds.
                    session.ask(if is_map { Job::ReadPcMap } else { Job::ReadSystem });
                    let mut action = Action::default();
                    if is_map {
                        action.pc_map = serde_json::from_str(&request.body).ok();
                    } else {
                        action.system = serde_json::from_str(&request.body).ok();
                    }
                    action.pedal = Some(key.to_string());
                    self.setlist.api_pending = Some(Pending { request, step: Step::Reading(Then::Action(action)), since: Instant::now() });
                    return Ok(());
                }
                self.api_confirm(request, write);
                Ok(())
            }
            ("GET", ["g3s"]) => match &self.setlist.g3s {
                Some(g) => answer(request, Response::json(200, json!({ "opengaragemusic": script::FORMAT, "path": g.path.as_ref().map(|p| p.display().to_string()), "actions": script::export_g3s(&g.backup) }))),
                None => answer(request, Response::error(404, "no G3S backup is loaded: open the G3S module and load one (or POST /g3s/actions to start from one bank)")),
            },
            ("POST", ["g3s", "actions"]) => {
                let script = match Script::parse(&request.body) {
                    Ok(script) => script,
                    Err(e) => return answer(request, Response::error(400, e)),
                };
                if script.actions.iter().any(|a| a.g3s.is_none()) {
                    return answer(request, Response::error(400, "only G3S actions here; POST /actions for the pedals"));
                }
                if self.setlist.g3s.is_none() {
                    let device = self.settings.set_list.devices.iter().position(|d| d.model == "g3s").unwrap_or(0);
                    self.open_g3s(device);
                }
                let g = self.setlist.g3s.as_mut().expect("opened");
                let mut results = Vec::new();
                for action in &script.actions {
                    match script::apply_g3s(&mut g.backup, action) {
                        Ok(text) => results.push(json!({ "done": text })),
                        Err(e) => {
                            results.push(json!({ "error": e }));
                            return answer(request, Response::json(400, json!({ "results": results })));
                        }
                    }
                }
                g.edited = true;
                answer(request, Response::json(200, json!({ "results": results, "banks": g.backup.banks.len() })))
            }
            ("POST", ["g3s", "save"]) => {
                let given: Option<String> = serde_json::from_str::<Json>(&request.body).ok().and_then(|b| b.get("path").and_then(Json::as_str).map(str::to_owned));
                let Some(g) = &self.setlist.g3s else { return answer(request, Response::error(404, "no G3S backup is loaded")) };
                let Some(path) = given.map(std::path::PathBuf::from).or_else(|| g.path.clone()) else { return answer(request, Response::error(400, "a body { \"path\": \"…\\\\file.g3b\" }: the backup was never saved")) };
                self.write_g3s_file(&path);
                let g = self.setlist.g3s.as_ref().expect("still open");
                answer(request, Response::json(if g.edited { 500 } else { 200 }, json!({ "result": g.message, "path": path.display().to_string() })))
            }
            ("POST", ["actions"]) => {
                self.review_ai_script(&request.body, None, ctx);
                let Some(s) = &self.setlist.script else { return answer(request, Response::error(400, self.setlist.message.clone())) };
                if s.errors() > 0 {
                    let actions: Vec<Json> = s.summaries.iter().map(|(summary, error)| json!({ "action": summary, "error": error })).collect();
                    return answer(request, Response::json(400, json!({ "actions": actions })));
                }
                if !self.settings.set_list.api_unattended && s.script.actions.iter().any(|a| a.pedal.is_some()) {
                    return answer(request, Response::error(403, "a script that writes pedals needs the user: they open List > Apply an AI script…, or allow unattended writes in Settings > API for an AI…"));
                }
                self.apply_ai_script();
                self.setlist.api_pending = Some(Pending { request, step: Step::Script, since: Instant::now() });
                Ok(())
            }
            _ => answer(request, Response::error(404, format!("no route {method} {full_path}; GET / lists them"))),
        }
    }

    /// The module of a device open for the API, or why it cannot be.
    fn api_pedal_session(&mut self, device: usize, ctx: &egui::Context) -> Result<(), Response> {
        if self.setlist.pedal.as_ref().is_some_and(|s| s.device == device) {
            return Ok(());
        }
        if self.setlist.pedal.is_some() {
            self.close_pedal();
        }
        let page = self.setlist.page;
        self.open_pedal(device, ctx);
        self.setlist.page = page;
        if self.setlist.pedal.is_none() {
            return Err(Response::error(503, self.setlist.message.clone()));
        }
        Ok(())
    }

    /// Asks the user, or writes at once when they allowed it.
    fn api_confirm(&mut self, request: Request, write: PedalWrite) {
        if self.settings.set_list.api_unattended {
            self.api_write(write);
            self.setlist.api_pending = Some(Pending { request, step: Step::Writing, since: Instant::now() });
        } else {
            self.setlist.api_pending = Some(Pending { request, step: Step::Confirm(write), since: Instant::now() });
        }
    }

    fn api_write(&mut self, write: PedalWrite) {
        let Some(session) = &mut self.setlist.pedal else { return };
        match write {
            PedalWrite::Memory { number, raw } => {
                session.selected = Some(number);
                session.raw = Some(raw.clone());
                session.ask(Job::Store { number, raw });
            }
            PedalWrite::PcMap(map) => {
                session.pc_map.clone_from(&map);
                session.ask(Job::WritePcMap(map));
            }
            PedalWrite::System(raw) => {
                session.system.clone_from(&raw);
                session.ask(Job::WriteSystem(raw));
            }
        }
    }

    /// The user answered the question of the API.
    pub fn api_decide(&mut self, yes: bool) {
        let Some(pending) = self.setlist.api_pending.take() else { return };
        let Step::Confirm(write) = pending.step else {
            self.setlist.api_pending = Some(pending);
            return;
        };
        if yes {
            self.api_write(write);
            self.setlist.api_pending = Some(Pending { request: pending.request, step: Step::Writing, since: Instant::now() });
        } else {
            let _ = pending.request.reply.send(Response::error(403, "refused by the user"));
        }
    }

    /// Goes on with the request waiting, once the pedal or the script has answered.
    fn step_api(&mut self, _ctx: &egui::Context) {
        let Some(pending) = self.setlist.api_pending.take() else { return };
        if pending.since.elapsed() > PATIENCE * 2 {
            // Answered 202 long ago; nothing waits for it any more.
            return;
        }
        let response = match &pending.step {
            Step::Confirm(_) => None,
            Step::Script => match &self.setlist.script {
                Some(s) if s.running.is_some() => None,
                Some(s) => {
                    let results: Vec<Json> = s.summaries.iter().zip(&s.results).map(|((summary, _), result)| match result {
                        Some(Ok(text)) => json!({ "action": summary, "done": text }),
                        Some(Err(e)) => json!({ "action": summary, "error": e }),
                        None => json!({ "action": summary }),
                    }).collect();
                    let failed = s.results.iter().filter(|r| matches!(r, Some(Err(_)))).count();
                    Some(Response::json(if failed > 0 { 500 } else { 200 }, json!({ "actions": results })))
                }
                None => Some(Response::error(500, "the script page was closed")),
            },
            Step::Reading(_) | Step::Writing => {
                let Some(session) = &mut self.setlist.pedal else { return self.api_reply(pending, Response::error(500, "the module was closed")) };
                if session.busy() {
                    None
                } else {
                    match session.outcome.take() {
                        None => Some(Response::error(500, "the pedal answered nothing")),
                        Some(Err(e)) => Some(Response::error(500, e)),
                        Some(Ok(text)) => match &pending.step {
                            Step::Writing => Some(Response::json(200, json!({ "result": text }))),
                            Step::Reading(then) => {
                                let description = session.description.clone();
                                match then {
                                    Then::Names => {
                                        let names: BTreeMap<String, String> = session.names.iter().enumerate().map(|(n, name)| (description.label(n), name.clone())).collect();
                                        Some(Response::json(200, json!(names)))
                                    }
                                    Then::Memory(number) => Some(match session.file.patch(&description, *number) {
                                        Some(raw) => Response::json(200, serde_json::to_value(script::export_patch(&description, *number, &raw)).unwrap_or_default()),
                                        None => Response::error(500, "the memory was not read"),
                                    }),
                                    Then::PcMap => Some(pc_map_response(&description, &session.pc_map)),
                                    Then::System => Some(Response::json(200, json!(script::export_values(&description, &description.system, &session.system)))),
                                    Then::Action(action) => {
                                        // The read is done: the write is made, and asked of the user.
                                        let write = if let Some(memory) = &action.memory {
                                            let number = description.number(memory).unwrap_or(0);
                                            match script::patch_from(&description, action, session.file.patch(&description, number).as_deref()) {
                                                Ok(raw) => PedalWrite::Memory { number, raw },
                                                Err(e) => return self.api_reply(pending, Response::error(400, e)),
                                            }
                                        } else if let Some(map) = &action.pc_map {
                                            match script::pc_map_from(&description, map) {
                                                Ok(entries) => {
                                                    let mut whole = session.pc_map.clone();
                                                    whole.resize(description.pc_map_entries, 0);
                                                    for (index, number) in entries {
                                                        whole[index] = number;
                                                    }
                                                    PedalWrite::PcMap(whole)
                                                }
                                                Err(e) => return self.api_reply(pending, Response::error(400, e)),
                                            }
                                        } else {
                                            let mut raw = session.system.clone();
                                            raw.resize(description.system_size(), 0);
                                            if let Err(e) = script::apply_set(&description, &description.system, &mut raw, action.system.as_ref().unwrap_or(&BTreeMap::new())) {
                                                return self.api_reply(pending, Response::error(400, e));
                                            }
                                            PedalWrite::System(raw)
                                        };
                                        self.api_confirm(pending.request, write);
                                        return;
                                    }
                                }
                            }
                            Step::Confirm(_) | Step::Script => None,
                        },
                    }
                }
            }
        };
        match response {
            Some(response) => self.api_reply(pending, response),
            None => self.setlist.api_pending = Some(pending),
        }
    }

    fn api_reply(&mut self, pending: Pending, response: Response) {
        let _ = pending.request.reply.send(response);
    }

    /// The question of the API waiting for the user, when there is one.
    pub fn api_question(&self) -> Option<String> {
        let pending = self.setlist.api_pending.as_ref()?;
        pending.question(self.setlist.pedal.as_ref().map(|s| &s.description))
    }
}

/// A program change map as the API shows it: program → memory.
fn pc_map_response(description: &Description, map: &[u16]) -> Response {
    let entries: BTreeMap<String, String> = map.iter().enumerate().map(|(i, n)| (if i < 128 { i.to_string() } else { format!("{}:{}", i / 128, i % 128) }, description.label(usize::from(*n)))).collect();
    Response::json(200, json!(entries))
}

/// What the window says about the API, for the settings.
pub fn where_it_listens(server: Option<&Server>) -> String {
    match server {
        Some(server) => format!("http://127.0.0.1:{}/", server.port),
        None => tr("off").to_owned(),
    }
}
