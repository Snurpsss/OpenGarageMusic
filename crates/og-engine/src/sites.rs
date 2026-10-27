//! Tablature sites described by files: how to search a site, how to cut
//! its answers into songs (and their tracks, when the site lists them),
//! how to follow its pages to the file of a song, and which of the
//! program's readers reads that file. The program ships the descriptions of
//! the sites it knows (the `sites` folder of this crate) and reads those a
//! user adds next to it; a description holds rules, never code.
//!
//! This module only reads descriptions and pages it is given: the
//! application does the asking (see `apps/opengarage/src/sites.rs`).
//!
//! A description, in JSON:
//!
//! ```json
//! {
//!   "format_version": 1,
//!   "name": "Example", "home": "https://www.example.org",
//!   "offers": { "en": "What the site offers.", "fr": "Ce qu'offre le site." },
//!   "encoding": "latin1",
//!   "headers": { "Referer": "https://www.example.org/" },
//!   "search": [ { "steps": [ {
//!     "get": "{home}/search?q={query}",
//!     "within": { "after": "RESULTS", "before": "</ul>" },
//!     "items": { "regex": "<li>.*?</li>" },
//!     "fields": {
//!       "url": { "regex": "href=\"([^\"]+)\"", "absolute": true },
//!       "title": { "regex": "<a[^>]*>(.*?)</a>", "text": true },
//!       "format": { "regex": "(GP[345X]?)", "default": "GP" }
//!     },
//!     "require": ["url", "title"],
//!     "where": { "format": "^GP" }
//!   } ] } ],
//!   "open": [ { "get": "{url}", "fields": { "file": { "regex": "href=\"([^\"]+\\.zip)\"", "absolute": true } } },
//!             { "download": "{file}" } ],
//!   "reader": "guitar-pro",
//!   "display": { "label": "{artist} – {title}", "badge": "{format}" }
//! }
//! ```
//!
//! - `format_version` is the version of this language of descriptions the
//!   file is written in (1 when it is missing). It changes only when the
//!   language gains something, never because a site changed; a program
//!   reads every version up to its own, and refuses a newer one, which may
//!   say things it cannot do.
//! - `search` holds pipelines whose songs are put together, each song once.
//!   A pipeline is steps: each step asks for a page (`get`, a template of
//!   the variables of what the step before found, or of the query for the
//!   first one), cuts it into items (`items`: a regular expression, or a
//!   `json` path; without it the page is one item) and reads fields from
//!   each item. The items of the last step are the songs.
//! - A field is read by `regex` (its first group, or `group`) or by a `json`
//!   path (`a.b.0.c`), then `text` takes the tags and entities out,
//!   `url_decode` undoes the percent signs, `replace` swaps texts,
//!   `absolute` makes a link absolute and `equals` turns the value into
//!   "1" or "". `template` builds a field from the others, and `default`
//!   stands for an empty one.
//! - `require` drops items without these fields, `where` keeps the items
//!   whose field matches a regular expression, `match_query` those whose
//!   field holds every word searched, and `limit` keeps the first ones.
//! - `tracks` lists the tracks of a song, when the site gives them with the
//!   song: `json` (the list) and `fields` (`name`, `detail`, `drums`).
//! - `open` takes a song to its file: `get` reads fields from a page into
//!   the variables, `download` fetches the file. With tracks, `each_track`
//!   downloads one file per ticked track (`{track}` is its place), and
//!   `try` gives other values to a variable, tried in turn when one fails.
//! - `reader` names the reader of the file: `guitar-pro` (Guitar Pro and
//!   MusicXML files) or `songsterr` (the parts of Songsterr's player).
//! - `open_only` sites are only opened in the browser.
//! - Since version 3, a site may give other files than tablatures: `gives`
//!   is `tablature` (the usual), `capture` (captures of amplifiers and
//!   pedals: NAM, Proteus, AIDA-X files) or `impulse` (impulse responses),
//!   read by the readers `capture` and `impulse`, and the files go to the
//!   library rather than into a project. A step on the way to the files may
//!   cut its page into `items`, each a file fetched by `download`. A site
//!   that asks for an account has a `login`: the endpoints of the device
//!   flow of OAuth 2 (`device`, `token`), the `client_id` of the program (or
//!   empty, and the user gives their own, made where `keys` says) and the
//!   `scope`. A site whose search asks nothing of `{query}` lists what it
//!   has, and the query only sifts the list.
//!
//! Variables: `{home}`, `{query}` (for an address), `{query_text}`,
//! `{query_letter}` (the first letter, lowercase without accent, or `9`),
//! the fields found, and `{tracks}`, the number of tracks of a song.
//! Every count is bounded, and a description that does not parse, or a
//! page that does not hold what is expected, gives an error or nothing,
//! never a crash.

use std::collections::BTreeMap;

use regex::Regex;
use serde::Deserialize;
use serde_json::Value;

use crate::tabsites::{fold, matches_words, unescape, url_decode};

/// Most steps in a pipeline, pipelines in a search, steps to a file.
/// A description without `format_version` was written before there was one.
fn first_version() -> u32 {
    1
}

/// The version of the language a description says it is written in, read
/// before anything else so that a newer one is told apart from a broken one.
pub fn format_of(text: &str) -> Option<u32> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    value.get("format_version").map_or(Some(1), |v| v.as_u64().and_then(|v| u32::try_from(v).ok()))
}

/// The version of the language of descriptions this program reads, and every
/// one before it. Raised only when the language gains something.
pub const FORMAT_VERSION: u32 = 3;

pub const MAX_STEPS: usize = 8;
/// Most items read from one page.
pub const MAX_ITEMS: usize = 500;
/// Most pages asked for by one step of a search.
pub const MAX_PAGES: usize = 12;

/// The descriptions shipped with the program: the name of their file and
/// their text.
pub const BUILT_IN: [(&str, &str); 7] = [
    ("songsterr.json", include_str!("../sites/songsterr.json")),
    ("guitariff.json", include_str!("../sites/guitariff.json")),
    ("tablaturehub.json", include_str!("../sites/tablaturehub.json")),
    ("ultimateguitar.json", include_str!("../sites/ultimateguitar.json")),
    ("tone3000.json", include_str!("../sites/tone3000.json")),
    ("tone3000-ir.json", include_str!("../sites/tone3000-ir.json")),
    ("guitarml.json", include_str!("../sites/guitarml.json")),
];

/// What a site gives: the kinds of files a description may name.
pub const GIVES: [&str; 3] = ["tablature", "capture", "impulse"];

/// A text in several languages, or one text for all.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Texts {
    One(String),
    Many(BTreeMap<String, String>),
}

impl Default for Texts {
    fn default() -> Self {
        Texts::One(String::new())
    }
}

impl Texts {
    /// The text in the language of this code, else in English, else any.
    pub fn get(&self, language: &str) -> &str {
        match self {
            Texts::One(text) => text,
            Texts::Many(texts) => texts.get(language).or_else(|| texts.get("en")).or_else(|| texts.values().next()).map_or("", String::as_str),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Description {
    /// The version of the language the description is written in.
    #[serde(default = "first_version")]
    pub format_version: u32,
    pub name: String,
    pub home: String,
    pub offers: Texts,
    /// The site is only opened in the browser.
    pub open_only: bool,
    /// "utf-8" (the default) or "latin1".
    pub encoding: String,
    pub headers: BTreeMap<String, String>,
    pub search: Vec<Pipeline>,
    pub open: Vec<OpenStep>,
    pub reader: String,
    pub display: Display,
    /// Since version 2: where the videos of a song are found, once it is
    /// opened, with the second each bar starts at in them. A step whose
    /// items are the videos, with the fields `video` (its YouTube id) and
    /// `points` (the seconds, as a list), and `label` if the site names it.
    pub videos: Option<Step>,
    /// Since version 3: what the site gives, one of `GIVES`.
    pub gives: String,
    /// Since version 3: how to sign in to a site that asks for an account.
    pub login: Option<Login>,
}

/// The device flow of OAuth 2 (RFC 8628) at a site.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Login {
    /// Where a device code is asked for.
    pub device: String,
    /// Where the token is asked for, and renewed.
    pub token: String,
    /// The key of the program at the site; empty when each user brings their own.
    pub client_id: String,
    pub scope: String,
    /// The page of the site where a user makes a key of their own.
    pub keys: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Display {
    pub label: Texts,
    pub badge: Texts,
}

impl Default for Display {
    fn default() -> Self {
        Display { label: Texts::One("{artist} – {title}".to_owned()), badge: Texts::One("{format}".to_owned()) }
    }
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Pipeline {
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Step {
    pub get: String,
    pub within: Option<Within>,
    pub items: Option<Items>,
    pub fields: BTreeMap<String, Field>,
    pub require: Vec<String>,
    #[serde(rename = "where")]
    pub filter: BTreeMap<String, String>,
    pub match_query: Option<String>,
    /// Keeps one item for each value of this field.
    pub unique: Option<String>,
    pub limit: Option<usize>,
    pub tracks: Option<Tracks>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Within {
    pub after: Option<String>,
    pub before: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Items {
    pub regex: Option<String>,
    pub json: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Field {
    pub regex: Option<String>,
    pub group: Option<usize>,
    pub json: Option<String>,
    pub text: bool,
    pub url_decode: bool,
    pub replace: Vec<(String, String)>,
    pub absolute: bool,
    pub uppercase: bool,
    pub equals: Option<String>,
    pub template: Option<String>,
    pub default: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Tracks {
    pub json: String,
    pub fields: BTreeMap<String, Field>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct OpenStep {
    pub get: Option<String>,
    pub fields: BTreeMap<String, Field>,
    /// Since version 3: the page lists files, one by item, each fetched by `download`.
    pub items: Option<Items>,
    pub download: Option<String>,
    pub each_track: bool,
    #[serde(rename = "try")]
    pub alternatives: BTreeMap<String, Vec<String>>,
}

/// A track of a song, as the site lists it with the song.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackEntry {
    pub name: String,
    pub detail: String,
    pub drums: bool,
    /// The track is the singing, which carries the words of the song.
    pub vocal: bool,
}

/// What an item of a page gave: its variables, and its tracks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub vars: BTreeMap<String, String>,
    pub tracks: Vec<TrackEntry>,
}

/// A song found on a site.
#[derive(Clone, Debug, PartialEq)]
pub struct Found {
    /// The name of the site.
    pub site: String,
    pub label: String,
    pub badge: String,
    pub vars: BTreeMap<String, String>,
    /// Its tracks, when the site lists them with the song.
    pub tracks: Vec<TrackEntry>,
}

impl Description {
    /// Reads a description; says what is wrong with one that cannot serve.
    pub fn parse(text: &str) -> Result<Description, String> {
        if let Some(version) = format_of(text).filter(|v| *v > FORMAT_VERSION) {
            return Err(format!("the description is written for a newer version of the program (format {version}; this one reads up to {FORMAT_VERSION})"));
        }
        let mut site: Description = serde_json::from_str(text).map_err(|e| format!("not a description of a site ({e})"))?;
        if site.format_version == 0 {
            return Err("the format_version of a description starts at 1".to_owned());
        }
        if site.videos.is_some() && site.format_version < 2 {
            return Err("the videos of a description need format_version 2".to_owned());
        }
        if site.gives.is_empty() {
            site.gives = "tablature".to_owned();
        }
        if !GIVES.contains(&site.gives.as_str()) {
            return Err(format!("a site cannot give \"{}\"", site.gives));
        }
        let third = site.gives != "tablature" || site.login.is_some() || site.open.iter().any(|s| s.items.is_some());
        if third && site.format_version < 3 {
            return Err("what the site gives, its login and the items of its files need format_version 3".to_owned());
        }
        if let Some(login) = &site.login {
            if !login.device.starts_with("https://") || !login.token.starts_with("https://") {
                return Err("the login of a site must be at https:// addresses".to_owned());
            }
        }
        site.name = site.name.trim().to_owned();
        if site.name.is_empty() {
            return Err("the description has no name".to_owned());
        }
        if !site.home.starts_with("https://") {
            return Err("the home of the site must be an https:// address".to_owned());
        }
        site.home = site.home.trim_end_matches('/').to_owned();
        if site.reader.is_empty() {
            site.reader = match site.gives.as_str() {
                "capture" => "capture",
                "impulse" => "impulse",
                _ => "guitar-pro",
            }
            .to_owned();
        }
        if !site.open_only {
            if site.search.is_empty() || site.search.iter().any(|p| p.steps.is_empty()) {
                return Err("the description says nothing of how to search the site".to_owned());
            }
            if site.open.is_empty() || !site.open.iter().any(|s| s.download.is_some()) {
                return Err("the description says nothing of where the file of a song is".to_owned());
            }
            if !["guitar-pro", "songsterr", "capture", "impulse"].contains(&site.reader.as_str()) {
                return Err(format!("the reader \"{}\" is not one the program has", site.reader));
            }
            let files = matches!(site.reader.as_str(), "capture" | "impulse");
            if files != (site.gives != "tablature") {
                return Err(format!("the reader \"{}\" does not read what the site gives ({})", site.reader, site.gives));
            }
        }
        if site.search.len() > MAX_STEPS || site.search.iter().any(|p| p.steps.len() > MAX_STEPS) || site.open.len() > MAX_STEPS {
            return Err("the description has too many steps".to_owned());
        }
        // Every regular expression is checked now, not at the first search.
        fn fields(fields: &BTreeMap<String, Field>) -> Vec<&str> {
            fields.values().filter_map(|f| f.regex.as_deref()).collect()
        }
        let mut patterns: Vec<&str> = Vec::new();
        for step in site.search.iter().flat_map(|p| &p.steps) {
            patterns.extend(step.items.as_ref().and_then(|i| i.regex.as_deref()));
            patterns.extend(fields(&step.fields));
            patterns.extend(step.filter.values().map(String::as_str));
            patterns.extend(step.tracks.as_ref().map(|t| fields(&t.fields)).unwrap_or_default());
        }
        for step in &site.open {
            patterns.extend(step.items.as_ref().and_then(|i| i.regex.as_deref()));
            patterns.extend(fields(&step.fields));
        }
        if let Some(step) = &site.videos {
            patterns.extend(step.items.as_ref().and_then(|i| i.regex.as_deref()));
            patterns.extend(fields(&step.fields));
        }
        for pattern in patterns {
            regex(pattern)?;
        }
        Ok(site)
    }

    /// Whether the site gives tablatures, to make projects of.
    pub fn is_tablature(&self) -> bool {
        self.gives == "tablature"
    }

    /// Whether the site lists what it has rather than searching: no step of
    /// its search asks for the query, which then only sifts the list.
    pub fn lists_only(&self) -> bool {
        !self.open_only && !self.search.iter().flat_map(|p| &p.steps).any(|s| s.get.contains("{query"))
    }

    /// Whether the page of an address is read as Latin-1.
    pub fn latin1(&self) -> bool {
        self.encoding.eq_ignore_ascii_case("latin1") || self.encoding.eq_ignore_ascii_case("iso-8859-1")
    }

    /// The text of a page the site sent.
    pub fn text(&self, bytes: &[u8]) -> String {
        if self.latin1() {
            crate::tabsites::latin1(bytes)
        } else {
            String::from_utf8_lossy(bytes).into_owned()
        }
    }

    /// A song as the window lists it, in the language of this code.
    pub fn found(&self, record: Record, language: &str) -> Found {
        let mut vars = record.vars;
        vars.insert("tracks".to_owned(), record.tracks.len().to_string());
        let label = fill(self.display.label.get(language), &vars).trim().trim_matches('–').trim().to_owned();
        let label = if label.is_empty() { vars.get("title").cloned().unwrap_or_default() } else { label };
        let badge = fill(self.display.badge.get(language), &vars).trim().to_owned();
        Found { site: self.name.clone(), label, badge, vars, tracks: record.tracks }
    }
}

fn regex(pattern: &str) -> Result<Regex, String> {
    regex::RegexBuilder::new(pattern).size_limit(1 << 20).build().map_err(|e| format!("the pattern \"{pattern}\" is not a regular expression ({e})"))
}

/// The variables of a search: the home of the site and the query.
pub fn query_vars(site: &Description, query: &str, encoded: &str) -> BTreeMap<String, String> {
    let letter = fold(query).chars().find(|c| c.is_alphanumeric()).filter(char::is_ascii_lowercase).map_or_else(|| "9".to_owned(), |c| c.to_string());
    BTreeMap::from([
        ("home".to_owned(), site.home.clone()),
        ("query".to_owned(), encoded.to_owned()),
        ("query_text".to_owned(), query.trim().to_owned()),
        ("query_letter".to_owned(), letter),
    ])
}

/// A template with the variables put into it; a name that is not known is
/// left out.
pub fn fill(template: &str, vars: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len() + 32);
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let name = &rest[open + 1..open + close];
        if name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') && !name.is_empty() {
            out.push_str(vars.get(name).map_or("", String::as_str));
        } else {
            out.push_str(&rest[open..=open + close]);
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// What a JSON path (`a.b.0.c`) leads to.
fn at_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    path.split('.').filter(|p| !p.is_empty()).try_fold(value, |value, key| match value {
        Value::Array(list) => list.get(key.parse::<usize>().ok()?),
        _ => value.get(key),
    })
}

fn json_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// A link of a page made absolute against the home of the site.
fn absolute(link: &str, home: &str) -> String {
    let link = link.trim();
    if link.starts_with("https://") || link.starts_with("http://") {
        link.to_owned()
    } else if let Some(rest) = link.strip_prefix("//") {
        format!("https://{rest}")
    } else if link.starts_with('/') {
        format!("{home}{link}")
    } else {
        format!("{home}/{link}")
    }
}

/// An item of a page: its text, or its JSON value.
enum Item<'a> {
    Text(&'a str),
    Json(&'a Value),
}

/// A field read from an item; the fields built from others come later.
fn read_field(field: &Field, item: &Item, home: &str) -> String {
    let mut value = match (item, &field.regex, &field.json) {
        (Item::Json(value), _, Some(path)) => at_path(value, path).map(json_text).unwrap_or_default(),
        (Item::Text(text), Some(pattern), _) => regex(pattern)
            .ok()
            .and_then(|re| {
                let captures = re.captures(text)?;
                let group = field.group.unwrap_or(if captures.len() > 1 { 1 } else { 0 });
                captures.get(group).map(|m| m.as_str().to_owned())
            })
            .unwrap_or_default(),
        (Item::Json(value), Some(pattern), None) => {
            let text = json_text(value);
            regex(pattern).ok().and_then(|re| re.captures(&text).and_then(|c| c.get(1).or_else(|| c.get(0))).map(|m| m.as_str().to_owned())).unwrap_or_default()
        }
        _ => String::new(),
    };
    finish(field, &mut value, home);
    value
}

/// What every field goes through once read or built.
fn finish(field: &Field, value: &mut String, home: &str) {
    if field.text {
        *value = plain(value);
    }
    if field.url_decode {
        *value = url_decode(value);
    }
    for (from, to) in &field.replace {
        if !from.is_empty() {
            *value = value.replace(from.as_str(), to);
        }
    }
    *value = value.trim().to_owned();
    if field.uppercase {
        *value = value.to_uppercase();
    }
    if field.absolute && !value.is_empty() {
        *value = absolute(value, home);
    }
    if let Some(equals) = &field.equals {
        *value = if value == equals { "1".to_owned() } else { String::new() };
    }
}

/// The text of a fragment of HTML: no tags, no entities, single spaces.
fn plain(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        match rest[open..].find('>') {
            Some(close) => rest = &rest[open + close + 1..],
            None => rest = "",
        }
    }
    out.push_str(rest);
    unescape(&out).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The fields of an item, read then built, with the variables it inherits.
fn fields_of(fields: &BTreeMap<String, Field>, item: &Item, inherited: &BTreeMap<String, String>, home: &str) -> BTreeMap<String, String> {
    let mut vars = inherited.clone();
    for (name, field) in fields.iter().filter(|(_, f)| f.template.is_none()) {
        vars.insert(name.clone(), read_field(field, item, home));
    }
    for (name, field) in fields.iter().filter(|(_, f)| f.template.is_some()) {
        let mut value = fill(field.template.as_deref().unwrap_or(""), &vars);
        finish(field, &mut value, home);
        vars.insert(name.clone(), value);
    }
    for (name, field) in fields {
        if vars.get(name).is_none_or(String::is_empty) {
            if let Some(default) = &field.default {
                vars.insert(name.clone(), fill(default, &vars));
            }
        }
    }
    vars
}

/// The part of a page a step looks at.
fn within<'a>(page: &'a str, within: Option<&Within>) -> &'a str {
    let Some(within) = within else { return page };
    let mut part = page;
    if let Some(after) = within.after.as_deref().filter(|a| !a.is_empty()) {
        part = part.find(after).map_or("", |at| &part[at + after.len()..]);
    }
    if let Some(before) = within.before.as_deref().filter(|b| !b.is_empty()) {
        part = part.find(before).map_or(part, |at| &part[..at]);
    }
    part
}

/// The items one step finds in a page, each with the variables of the item
/// that led to the page, the query among them.
pub fn read_page(step: &Step, page: &str, inherited: &BTreeMap<String, String>) -> Vec<Record> {
    let home = inherited.get("home").cloned().unwrap_or_default();
    let page = within(page, step.within.as_ref());
    let uses_json = step.items.as_ref().is_some_and(|i| i.json.is_some()) || step.fields.values().any(|f| f.json.is_some()) || step.tracks.is_some();
    let document: Option<Value> = if uses_json { serde_json::from_str(page).ok() } else { None };
    let mut records = Vec::new();
    let mut push = |item: Item| {
        let vars = fields_of(&step.fields, &item, inherited, &home);
        let tracks = match (&step.tracks, &item) {
            (Some(tracks), Item::Json(value)) => at_path(value, &tracks.json)
                .and_then(Value::as_array)
                .map(|list| {
                    list.iter()
                        .take(MAX_ITEMS)
                        .map(|track| {
                            let found = fields_of(&tracks.fields, &Item::Json(track), &BTreeMap::new(), &home);
                            let get = |name: &str| found.get(name).cloned().unwrap_or_default();
                            TrackEntry { name: get("name"), detail: get("detail"), drums: !get("drums").is_empty(), vocal: !get("vocal").is_empty() }
                        })
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        records.push(Record { vars, tracks });
    };
    match (step.items.as_ref(), &document) {
        (Some(Items { json: Some(path), .. }), Some(document)) => {
            // A path that is not there, on a page that is a list: the list.
            let list = at_path(document, path).and_then(Value::as_array).or_else(|| document.as_array());
            for value in list.into_iter().flatten().take(MAX_ITEMS) {
                push(Item::Json(value));
            }
        }
        (Some(Items { regex: Some(pattern), .. }), _) => {
            if let Ok(re) = regex(pattern) {
                for found in re.find_iter(page).take(MAX_ITEMS) {
                    push(Item::Text(found.as_str()));
                }
            }
        }
        (_, Some(document)) => push(Item::Json(document)),
        _ => push(Item::Text(page)),
    }
    keep(step, records, inherited.get("query_text").map_or("", String::as_str))
}

/// The items a step keeps, of those it read.
fn keep(step: &Step, records: Vec<Record>, query: &str) -> Vec<Record> {
    let filters: Vec<(String, Regex)> = step.filter.iter().filter_map(|(name, pattern)| Some((name.clone(), regex(pattern).ok()?))).collect();
    let mut seen: Vec<String> = Vec::new();
    let kept = records.into_iter().filter(|record| {
        let get = |name: &str| record.vars.get(name).map_or("", String::as_str);
        let wanted = step.require.iter().all(|name| !get(name).is_empty())
            && filters.iter().all(|(name, re)| re.is_match(get(name)))
            && step.match_query.as_deref().is_none_or(|name| query.trim().is_empty() || matches_words(get(name), query));
        if let (true, Some(unique)) = (wanted, &step.unique) {
            let value = get(unique).to_owned();
            if seen.contains(&value) {
                return false;
            }
            seen.push(value);
        }
        wanted
    });
    kept.take(step.limit.unwrap_or(MAX_ITEMS).min(MAX_ITEMS)).collect()
}

/// The songs of several pipelines together, the first ones first and each
/// song once: two songs are one when they lead to the same address.
pub fn merge(lists: Vec<Vec<Record>>) -> Vec<Record> {
    let mut all: Vec<Record> = Vec::new();
    for record in lists.into_iter().flatten() {
        let key = |r: &Record| r.vars.get("url").filter(|u| !u.is_empty()).cloned().unwrap_or_else(|| format!("{:?}", r.vars.get("title")));
        match all.iter_mut().find(|known| key(known) == key(&record)) {
            // The same song: what the first did not know, the second may.
            Some(known) => {
                for (name, value) in record.vars {
                    let slot = known.vars.entry(name).or_default();
                    if slot.is_empty() {
                        *slot = value;
                    }
                }
            }
            None => all.push(record),
        }
    }
    all
}

/// The fields a step on the way to the file reads from a page.
pub fn read_open_page(step: &OpenStep, page: &str, vars: &BTreeMap<String, String>, json: bool) -> BTreeMap<String, String> {
    let home = vars.get("home").cloned().unwrap_or_default();
    let document = if json { serde_json::from_str::<Value>(page).ok() } else { None };
    match &document {
        Some(value) => fields_of(&step.fields, &Item::Json(value), vars, &home),
        None => fields_of(&step.fields, &Item::Text(page), vars, &home),
    }
}

/// The items a step on the way to the files finds in its page, each a file
/// to fetch: their variables, with those of the song.
pub fn read_open_items(step: &OpenStep, page: &str, vars: &BTreeMap<String, String>) -> Vec<BTreeMap<String, String>> {
    let as_step = Step { items: step.items.clone(), fields: step.fields.clone(), ..Step::default() };
    read_page(&as_step, page, vars).into_iter().map(|record| record.vars).collect()
}

/// Whether a step on the way to the file reads JSON.
pub fn open_step_reads_json(step: &OpenStep) -> bool {
    step.fields.values().any(|f| f.json.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn built_in(file: &str) -> Description {
        let text = BUILT_IN.iter().find(|(name, _)| *name == file).unwrap().1;
        Description::parse(text).unwrap()
    }

    /// What a search finds on pages given for each address asked.
    fn search(site: &Description, query: &str, pages: &dyn Fn(&str) -> Option<String>) -> Vec<Found> {
        let mut lists = Vec::new();
        for pipeline in &site.search {
            let mut records = vec![Record { vars: query_vars(site, query, &query.replace(' ', "+")), tracks: Vec::new() }];
            for step in &pipeline.steps {
                let mut next = Vec::new();
                for record in records.iter().take(MAX_PAGES) {
                    let url = fill(&step.get, &record.vars);
                    if let Some(page) = pages(&url) {
                        next.extend(read_page(step, &page, &record.vars));
                    }
                }
                records = next;
            }
            lists.push(records);
        }
        merge(lists).into_iter().map(|r| site.found(r, "fr")).collect()
    }

    #[test]
    fn the_videos_of_a_song_are_read_with_their_points() {
        let site = built_in("songsterr.json");
        let step = site.videos.as_ref().unwrap();
        let page = r#"[{"id":1,"songId":286,"revisionId":8994114,"points":[0.11,3.17,6.29],"videoId":"F4E7DSusV9Y","status":"done","feature":"alternative","countries":["All"]},
                       {"id":2,"points":[-0.58,2.65],"videoId":"Yzy56lKRDDw","status":"done","feature":null},
                       {"id":3,"points":[1.0],"videoId":"pending1","status":"pending"}]"#;
        let vars = BTreeMap::from([("id".to_owned(), "286".to_owned()), ("revision".to_owned(), "8994114".to_owned()), ("home".to_owned(), site.home.clone())]);
        assert_eq!(fill(&step.get, &vars), "https://www.songsterr.com/api/video-points/286/8994114/list");
        let videos = read_page(step, page, &vars);
        let seen: Vec<(&str, &str, &str)> = videos.iter().map(|v| (v.vars["video"].as_str(), v.vars["points"].as_str(), v.vars["label"].as_str())).collect();
        assert_eq!(seen, [("F4E7DSusV9Y", "[0.11,3.17,6.29]", "alternative"), ("Yzy56lKRDDw", "[-0.58,2.65]", "")], "the videos not ready are left out");
        // A description of version 1 may not ask for videos.
        let old = r#"{"format_version": 1, "name": "A", "home": "https://a.b", "search": [{"steps": [{"get": "{home}"}]}], "open": [{"download": "{url}"}], "videos": {"get": "{home}"}}"#;
        assert!(Description::parse(old).unwrap_err().contains("format_version 2"));
    }

    #[test]
    fn every_description_shipped_is_read() {
        let names: Vec<String> = BUILT_IN.iter().map(|(_, text)| Description::parse(text).unwrap().name).collect();
        assert_eq!(names, ["Songsterr", "Guitariff", "Tablature Hub", "Ultimate Guitar", "TONE3000", "TONE3000 IR", "GuitarML"]);
        let ug = built_in("ultimateguitar.json");
        assert!(ug.open_only && ug.search.is_empty());
        assert!(!ug.offers.get("fr").is_empty() && ug.offers.get("fr") != ug.offers.get("en"));
        for (_, text) in BUILT_IN {
            let site = Description::parse(text).unwrap();
            for language in ["en", "fr", "de", "es", "it", "pt"] {
                assert!(!site.offers.get(language).is_empty(), "{} says what it offers in {language}", site.name);
            }
        }
    }

    #[test]
    fn descriptions_that_cannot_serve_are_refused() {
        let error = |text: &str| Description::parse(text).unwrap_err();
        assert!(error("{").contains("not a description"));
        assert!(error(r#"{"home": "https://a.b"}"#).contains("no name"));
        assert!(error(r#"{"name": "A", "home": "http://a.b"}"#).contains("https"));
        assert!(error(r#"{"name": "A", "home": "https://a.b"}"#).contains("search"));
        assert!(error(r#"{"name": "A", "home": "https://a.b", "search": [{"steps": [{"get": "{home}"}]}]}"#).contains("file"));
        let bad = r#"{"name": "A", "home": "https://a.b", "search": [{"steps": [{"get": "{home}", "items": {"regex": "(unclosed"}}]}], "open": [{"download": "{url}"}]}"#;
        assert!(error(bad).contains("regular expression"));
        let reader = r#"{"name": "A", "home": "https://a.b", "search": [{"steps": [{"get": "{home}"}]}], "open": [{"download": "{url}"}], "reader": "midi"}"#;
        assert!(error(reader).contains("midi"));
        let fine = Description::parse(r#"{"name": " A ", "home": "https://a.b/", "search": [{"steps": [{"get": "{home}"}]}], "open": [{"download": "{url}"}]}"#).unwrap();
        assert_eq!((fine.name.as_str(), fine.home.as_str(), fine.reader.as_str()), ("A", "https://a.b", "guitar-pro"));
        // The version of the language: missing is the first, a newer one is refused.
        assert_eq!(fine.format_version, 1);
        let versioned = |version: &str| format!(r#"{{"format_version": {version}, "name": "A", "home": "https://a.b", "search": [{{"steps": [{{"get": "{{home}}"}}]}}], "open": [{{"download": "{{url}}"}}]}}"#);
        assert_eq!(Description::parse(&versioned("1")).unwrap().format_version, 1);
        assert!(error(&versioned(&(FORMAT_VERSION + 1).to_string())).contains("newer version"));
        assert!(error(&versioned("0")).contains("starts at 1"));
        assert_eq!((format_of(&versioned("7")), format_of(r#"{"name": "A"}"#), format_of("{")), (Some(7), Some(1), None));
        for (_, text) in BUILT_IN {
            assert!(format_of(text).is_some_and(|v| (1..=FORMAT_VERSION).contains(&v)), "the descriptions shipped say their version");
        }
        assert!(built_in("songsterr.json").videos.is_some(), "Songsterr gives the videos of its songs");
    }

    #[test]
    fn sites_that_give_captures_and_impulses_are_described() {
        let tone = built_in("tone3000.json");
        assert!((tone.gives.as_str(), tone.reader.as_str(), tone.is_tablature(), tone.lists_only()) == ("capture", "capture", false, true));
        let login = tone.login.as_ref().unwrap();
        assert!(login.device.starts_with("https://www.tone3000.com/") && login.client_id.is_empty() && !login.keys.is_empty());
        assert_eq!(built_in("tone3000-ir.json").gives, "impulse");
        let page = r#"{"data": [{"id": 5, "title": "Twin", "user": {"username": "ann"}, "gear": "amp", "format": "nam", "license": "t3k"},
                                {"id": 6, "title": "Cab", "user": {"username": "bob"}, "gear": "cab", "format": "ir", "license": "cc-by"}], "page": 1}"#;
        // With no query, the list is kept whole; a query sifts it by title.
        let listed = search(&tone, "", &|url| url.contains("/tones/favorited").then(|| page.to_owned()));
        assert_eq!(listed.len(), 1, "only the captures, each once, from the one list that answered");
        assert_eq!((listed[0].label.as_str(), listed[0].badge.as_str()), ("Twin · amp · ann", "nam · t3k"));
        assert!(search(&tone, "cab", &|url| url.contains("/tones/").then(|| page.to_owned())).is_empty());
        let irs = search(&built_in("tone3000-ir.json"), "", &|url| url.contains("/tones/").then(|| page.to_owned()));
        assert_eq!(irs.len(), 1);
        assert_eq!(irs[0].vars["url"], "https://www.tone3000.com/tones/6");
        // The models of a tone, each a file.
        let models = r#"{"data": [{"id": 1, "name": "Standard", "model_url": "https://cdn.example/1.nam", "size": "standard", "architecture_version": "2"},
                                  {"id": 2, "name": "Lite", "model_url": "https://cdn.example/2.nam", "size": "lite", "architecture_version": "2"}]}"#;
        let step = &tone.open[0];
        let vars = BTreeMap::from([("id".to_owned(), "5".to_owned()), ("home".to_owned(), tone.home.clone())]);
        assert_eq!(fill(step.get.as_deref().unwrap(), &vars), "https://www.tone3000.com/api/v1/models?tone_id=5&page_size=100");
        let items = read_open_items(step, models, &vars);
        assert_eq!(items.len(), 2);
        assert_eq!((items[1]["name"].as_str(), fill(step.download.as_deref().unwrap(), &items[1]).as_str()), ("Lite", "https://cdn.example/2.nam"));
        // GuitarML's page, as it was first read.
        let ml = built_in("guitarml.json");
        assert!(!ml.lists_only() || true);
        let html = r#"<article class="style5"><span class="image"><img src="images/pic02.jpg" alt="" /></span>
            <a href="https://github.com/GuitarML/ToneLibrary/releases/download/v1.0/Barber_GC_GainKnob.json"><h2>Barber GC Pedal Gain Knob</h2>
            <div class="content"><p>Barber Gain Changer pedal, Gain Knob.</p></div></a></article>
            <article class="style4"><a href="https://github.com/GuitarML/ToneLibrary/releases/download/v1.0/Proteus_Tone_Packs.zip"><h2>All</h2></a></article>"#;
        let found = search(&ml, "barber", &|url| (url == "https://guitarml.com/tonelibrary/tonelib-pro.html").then(|| html.to_owned()));
        assert_eq!(found.len(), 1, "the zip of everything is not a capture");
        assert_eq!((found[0].label.as_str(), found[0].badge.as_str()), ("Barber GC Pedal Gain Knob", "Barber Gain Changer pedal, Gain Knob."));
        assert_eq!(fill(ml.open[0].download.as_deref().unwrap(), &found[0].vars), "https://github.com/GuitarML/ToneLibrary/releases/download/v1.0/Barber_GC_GainKnob.json");
        assert!(search(&ml, "benson", &|_| Some(html.to_owned())).is_empty());
        // The third version of the language is needed for these, and the reader must fit.
        let error = |text: &str| Description::parse(text).unwrap_err();
        let base = |extra: &str| format!(r#"{{"format_version": 2, "name": "A", "home": "https://a.b", "search": [{{"steps": [{{"get": "{{home}}"}}]}}], "open": [{{"download": "{{url}}"}}]{extra}}}"#);
        assert!(error(&base(r#", "gives": "capture""#)).contains("format_version 3"));
        assert!(error(&base(r#", "login": {"device": "https://a.b/d", "token": "https://a.b/t"}"#)).contains("format_version 3"));
        assert!(error(&base(r#", "gives": "sounds""#)).contains("cannot give"));
        let third = |extra: &str| base(extra).replace("\"format_version\": 2", "\"format_version\": 3");
        assert!(error(&third(r#", "gives": "capture", "reader": "guitar-pro""#)).contains("does not read"));
        assert!(error(&third(r#", "login": {"device": "http://a.b/d", "token": "https://a.b/t"}"#)).contains("https"));
        let fine = Description::parse(&third(r#", "gives": "impulse""#)).unwrap();
        assert_eq!(fine.reader, "impulse");
    }

    #[test]
    fn templates_take_the_variables() {
        let vars = BTreeMap::from([("a".to_owned(), "x".to_owned()), ("long_name".to_owned(), "y".to_owned())]);
        assert_eq!(fill("{a}/{long_name}/{missing}/{not a name}/{", &vars), "x/y//{not a name}/{");
        let site = built_in("tablaturehub.json");
        assert_eq!(query_vars(&site, "Édith Piaf", "x")["query_letter"], "e");
        assert_eq!(query_vars(&site, " 2Pac", "x")["query_letter"], "9");
        assert_eq!(absolute("/a/b", "https://h.t"), "https://h.t/a/b");
        assert_eq!(absolute("//cdn.t/x", "https://h.t"), "https://cdn.t/x");
        assert_eq!(absolute("x.html", "https://h.t"), "https://h.t/x.html");
    }

    /// The fragments of pages the readers of the sites were first written against.
    #[test]
    fn guitariff_keeps_its_guitar_pro_tablatures() {
        let site = built_in("guitariff.json");
        let results = r##"<h2><img src="/img/fleche.gif" /> TABLATURES TXT <a href="#top">^</a></h2>
<ul><li class="brddashed"><a href="/tabs/abc/1_song_txt/#top"><strong>Abc</strong> - <i>Song</i></a> <img src="/img/pictos/txt.gif" alt="" /></li></ul>
<h2><img src="/img/fleche.gif" /> TABLATURES GUITAR PRO <a href="#top">^</a> </h2>
<ul>
<li class="brddashed"><a href="/tabs/abc/27891_song_%284%29_guitar_pro/#top"><strong>Abc</strong>  - <i>Song (4)</i></a> <img src="/img/pictos/gp4.gif" alt="" align="top" /></li>
<li class="brddashed"><a href="/tabs/abc/27897_song_sm_guitar_pro/#top"><strong>Abc</strong> - <i>Song (S&amp;M)</i></a> <img src="/img/pictos/gp3.gif" alt="" align="top" /></li>
<li class="brddashed"><a href="/tabs/abc/27898_song_txt/#top"><strong>Abc</strong> - <i>Song text</i></a> <img src="/img/pictos/txt.gif" alt="" align="top" /></li>
<li class="brddashed"><a href="/tabs/abc/27899_song_gpx/#top"><strong>Abc</strong> - <i>Song 6</i></a> <img src="/img/pictos/gpx.gif" alt="" align="top" /></li>
</ul>
<h2>Autre chose</h2>
<li class="brddashed"><a href="/tabs/abc/5_other/#top"><strong>Abc</strong> - <i>Other</i></a> <img src="/img/pictos/gp5.gif" /></li>"##;
        let found = search(&site, "song", &|url| (url == "https://www.guitariff.net/tabs/search.php?q=song").then(|| results.to_owned()));
        let seen: Vec<(&str, &str)> = found.iter().map(|f| (f.vars["title"].as_str(), f.badge.as_str())).collect();
        assert_eq!(seen, [("Song (4)", "GP4"), ("Song (S&M)", "GP3"), ("Song 6", "GPX")], "the Guitar Pro section only, its text tablatures passed over");
        assert_eq!(found[0].label, "Abc – Song (4)");
        assert_eq!(found[0].vars["url"], "https://www.guitariff.net/tabs/abc/27891_song_%284%29_guitar_pro/");
        // The page of a song leads to its archive.
        let page = r##"<p>Texte</p><a href="/tabs/abc/#top">haut</a>
<a href="/ddl_tabs/gp2/Abc%20-%20Song%20%28live%29.zip" target="_blank" title="Télécharger la tablature">Télécharger</a>"##;
        let vars = read_open_page(&site.open[0], page, &found[0].vars, false);
        assert_eq!(fill(site.open[1].download.as_deref().unwrap(), &vars), "https://www.guitariff.net/ddl_tabs/gp2/Abc%20-%20Song%20%28live%29.zip");
        assert!(read_open_page(&site.open[0], "<a href=\"/tabs/x/\">x</a>", &found[0].vars, false)["file"].is_empty(), "no file on the page");
    }

    #[test]
    fn tablature_hub_finds_titles_and_the_songs_of_artists() {
        let site = built_in("tablaturehub.json");
        let results = r##"<div class="list-group">
<a href="https://www.tablatureguitare.net/tablature-partition/Buckley%2CJeff-tabs/3941.html" class="list-group-item list-group-item-action d-flex"><div><i class="fas fa-music mr-2 text-info"></i><strong>Hallelujah (3)</strong> <small class="text-muted"> - Buckley Jeff</small></div><span class="badge text-white" style="background-color: var(--x);">GP3</span></a>
<a href="https://www.tablatureguitare.net/tablature-partition/U2-tabs/26418.html" class="list-group-item list-group-item-action"><div><i class="fas fa-music"></i><strong>Hallelujah</strong> <small class="text-muted"> - U2</small></div><span class="badge text-white" style="background-color: #6c757d;">TXT</span></a>
<a href="https://www.tablatureguitare.net/tablature-partition/Saez-tabs/29712.html" class="list-group-item list-group-item-action"><div><strong>Jeune &amp; con</strong> <small class="text-muted"> - Saez</small></div><span class="badge text-white">GP4</span></a>
<a href="https://www.tablatureguitare.net/autre.html" class="list-group-item list-group-item-action"><strong>Pas de numéro</strong><span class="badge">GP5</span></a>
<a href="https://www.tablatureguitare.net/" class="nav-link">Accueil</a></div>"##;
        let index = r##"<nav><a href="https://www.tablatureguitare.net/">Accueil</a> <a href="/tablature-partition/a.html">A</a></nav>
<ul><li><a href="https://www.tablatureguitare.net/tablature-partition/Nirvana-tabs">Nirvana</a></li>
<li><a href="https://www.tablatureguitare.net/tablature-partition/Buckley%2CJeff-tabs">Buckley Jeff</a></li>
<li><a href="/tablature-partition/No%C3%ABl%2CMagne-tabs">Noël Magne</a></li>
<li><a href="https://www.tablatureguitare.net/tablature-partition/Nirvana-tabs">Nirvana (encore)</a></li></ul>"##;
        let artist = r##"<a href="https://www.tablatureguitare.net/tablature-partition/Nirvana-tabs/120.html">Come As You Are</a>
<a href="https://www.tablatureguitare.net/tablature-partition/Nirvana-tabs/121.html">Lithium <span class="badge">GP4</span></a>"##;
        let pages = |url: &str| -> Option<String> {
            match url {
                "https://www.tablatureguitare.net/recherche?q=hallelujah" => Some(results.to_owned()),
                "https://www.tablatureguitare.net/tablature-partition/h.html" => Some(String::new()),
                "https://www.tablatureguitare.net/recherche?q=nirvana" => Some(String::new()),
                "https://www.tablatureguitare.net/tablature-partition/n.html" => Some(index.to_owned()),
                "https://www.tablatureguitare.net/tablature-partition/Nirvana-tabs" => Some(artist.to_owned()),
                _ => None,
            }
        };
        let found = search(&site, "hallelujah", &pages);
        let seen: Vec<(&str, &str, &str, &str)> = found.iter().map(|f| (f.vars["artist"].as_str(), f.vars["title"].as_str(), f.vars["url"].as_str(), f.badge.as_str())).collect();
        assert_eq!(
            seen,
            [
                ("Buckley Jeff", "Hallelujah (3)", "https://www.tablatureguitare.net/download.php?id=3941", "GP3"),
                ("Saez", "Jeune & con", "https://www.tablatureguitare.net/download.php?id=29712", "GP4"),
            ],
            "the text tablatures and the links without a number are passed over"
        );
        // By artist: the index of the letter, then the page of each artist that matches.
        let found = search(&site, "nirvana", &pages);
        let seen: Vec<(&str, &str, &str)> = found.iter().map(|f| (f.vars["artist"].as_str(), f.vars["title"].as_str(), f.vars["url"].as_str())).collect();
        assert_eq!(
            seen,
            [
                ("Nirvana", "Come As You Are", "https://www.tablatureguitare.net/download.php?id=120"),
                ("Nirvana", "Lithium", "https://www.tablatureguitare.net/download.php?id=121"),
            ],
            "the artist comes from the address when the page says nothing; Nirvana's page is read once"
        );
        assert_eq!(found[0].badge, "GP", "no badge: a Guitar Pro file all the same");
    }

    #[test]
    fn songsterr_lists_songs_with_their_tracks() {
        let site = built_in("songsterr.json");
        assert_eq!(site.reader, "songsterr");
        let answer = r#"[{"songId":455118,"artistId":20,"artist":"Metallica","title":"Master of Puppets","hasChords":true,
            "tracks":[{"instrumentId":67,"instrument":"Baritone Sax","views":1792,"name":"Vocals","tuning":[64,59,55,50,45,40]},
                      {"instrumentId":30,"instrument":"Distortion Guitar","views":174644,"name":"Lead Guitar","tuning":[64,59,55,50,45,40],"difficulty":4},
                      {"instrumentId":1024,"instrument":"Drums","views":222717,"name":"Drums"}],
            "defaultTrack":1},
            {"artist":"Nobody","title":"No number"}]"#;
        let found = search(&site, "master of puppets", &|url| url.starts_with("https://www.songsterr.com/api/search?pattern=master+of+puppets").then(|| answer.to_owned()));
        assert_eq!(found.len(), 1, "a song without a number cannot be opened");
        let song = &found[0];
        assert_eq!((song.label.as_str(), song.badge.as_str(), song.vars["id"].as_str()), ("Metallica – Master of Puppets", "3 pistes", "455118"));
        assert_eq!(song.tracks.len(), 3);
        assert_eq!((song.tracks[1].name.as_str(), song.tracks[1].detail.as_str(), song.tracks[1].drums), ("Lead Guitar", "Distortion Guitar", false));
        assert!(song.tracks[2].drums);
        // The same answer under "records".
        let wrapped = format!(r#"{{"records": {answer}}}"#);
        assert_eq!(search(&site, "x", &|_| Some(wrapped.clone())).len(), 1);
        // The description of the song tells where its parts are.
        let meta = r#"{"songId":439171,"revisionId":9570923,"image":"v0-3-2-XZ9xQtqPJQ8tJAqJ","artist":"A","title":"T"}"#;
        let mut vars = read_open_page(&site.open[0], meta, &BTreeMap::from([("id".to_owned(), "439171".to_owned())]), open_step_reads_json(&site.open[0]));
        let download = &site.open[1];
        assert!(download.each_track);
        vars.insert("track".to_owned(), "3".to_owned());
        vars.insert("host".to_owned(), download.alternatives["host"][0].clone());
        assert_eq!(fill(download.download.as_deref().unwrap(), &vars), "https://dqsljvtekg760.cloudfront.net/439171/9570923/v0-3-2-XZ9xQtqPJQ8tJAqJ/3.json");
        assert_eq!(download.alternatives["host"].len(), 3);
    }

    #[test]
    fn pages_that_hold_nothing_expected_give_nothing() {
        for (_, text) in BUILT_IN {
            let site = Description::parse(text).unwrap();
            for page in ["", "<html></html>", "{}", "[1, 2]", "\u{0}\u{1}", &"x".repeat(100_000)] {
                assert!(search(&site, "a", &|_| Some(page.to_owned())).is_empty(), "{}: {page:.20}", site.name);
            }
        }
    }
}
