//! Translation of the interface.
//!
//! The texts written in the code are English, and English is what is shown
//! when nothing else is chosen or when a text has no translation. Every other
//! language is one gettext catalogue, `<code>.po` (`fr.po`, `de.po`…): the
//! ones in the `lang` folder of the sources are built into the program, and
//! any other `.po` file put in a `lang` folder next to the program or in its
//! settings folder is offered as well, without recompiling.

use std::collections::HashMap;
use std::fmt::{Display, Write};
use std::path::PathBuf;
use std::sync::{Mutex, RwLock};

// `BUILT_IN: &[(&str, &str)]`: language code and catalogue text of every
// `.po` file found in the `lang` folder when the program was built.
include!(concat!(env!("OUT_DIR"), "/languages.rs"));

/// Code of the language of the texts in the code.
pub const SOURCE: &str = "en";

/// The texts of one language.
pub struct Catalog {
    pub code: String,
    /// The language's name in that language, from the `X-Language-Name` header.
    pub name: String,
    entries: HashMap<String, &'static str>,
}

/// Reads the quoted part of a catalogue line, undoing its escapes.
fn quoted(line: &str) -> Option<String> {
    let inner = line.trim().strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    Some(out)
}

impl Catalog {
    /// Reads a gettext catalogue. Entries without a translation are left out,
    /// so that the English text shows; contexts and plural forms are not used
    /// by this program and are passed over.
    pub fn parse(code: &str, text: &str) -> Self {
        #[derive(PartialEq)]
        enum Part {
            None,
            Id,
            Str,
            Other,
        }
        let mut entries = HashMap::new();
        let mut name = String::new();
        let (mut id, mut translation, mut part) = (String::new(), String::new(), Part::None);
        let mut finish = |id: &mut String, translation: &mut String| {
            if id.is_empty() {
                // The header: one "Key: value" per line.
                for line in translation.lines() {
                    if let Some(value) = line.strip_prefix("X-Language-Name:") {
                        name = value.trim().to_owned();
                    }
                }
            } else if !translation.is_empty() {
                let kept: &'static str = Box::leak(std::mem::take(translation).into_boxed_str());
                entries.insert(std::mem::take(id), kept);
            }
            id.clear();
            translation.clear();
        };
        for line in text.trim_start_matches('\u{feff}').lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(rest) = line.strip_prefix("msgid ") {
                if part == Part::Str {
                    finish(&mut id, &mut translation);
                }
                id = quoted(rest).unwrap_or_default();
                part = Part::Id;
            } else if let Some(rest) = line.strip_prefix("msgstr ") {
                translation = quoted(rest).unwrap_or_default();
                part = Part::Str;
            } else if line.starts_with('"') {
                match part {
                    Part::Id => id.push_str(&quoted(line).unwrap_or_default()),
                    Part::Str => translation.push_str(&quoted(line).unwrap_or_default()),
                    _ => {}
                }
            } else {
                // msgctxt, msgid_plural, msgstr[n]: not used here.
                if part == Part::Str {
                    finish(&mut id, &mut translation);
                }
                part = Part::Other;
            }
        }
        if part == Part::Str {
            finish(&mut id, &mut translation);
        }
        if name.is_empty() {
            name = code.to_owned();
        }
        Self { code: code.to_owned(), name, entries }
    }

    pub fn get(&self, text: &str) -> Option<&'static str> {
        self.entries.get(text).copied()
    }

    /// The English text that this catalogue translates as `translated`.
    pub fn source_of(&self, translated: &str) -> Option<&str> {
        self.entries.iter().find(|(_, value)| **value == translated).map(|(key, _)| key.as_str())
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub fn entries(&self) -> impl Iterator<Item = (&str, &'static str)> {
        self.entries.iter().map(|(key, value)| (key.as_str(), *value))
    }
}

static CURRENT: RwLock<Option<&'static Catalog>> = RwLock::new(None);
/// Catalogues read so far, by code: each is read once and kept.
static LOADED: Mutex<Vec<&'static Catalog>> = Mutex::new(Vec::new());

/// Folders searched for more catalogues: `lang` next to the program, and
/// `lang` in the settings folder.
fn folders() -> Vec<PathBuf> {
    let mut found = Vec::new();
    if let Some(dir) = crate::app::beside_program("lang") {
        found.push(dir);
    }
    if let Some(roaming) = std::env::var_os("APPDATA") {
        found.push(PathBuf::from(roaming).join("OpenGarageMusic").join("lang"));
    }
    found
}

/// Catalogue files found on the disk: code and path. Tests see none, so that
/// they do not depend on the machine.
fn files() -> Vec<(String, PathBuf)> {
    if cfg!(test) {
        return Vec::new();
    }
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    for folder in folders() {
        let Ok(entries) = std::fs::read_dir(folder) else { continue };
        for path in entries.flatten().map(|e| e.path()) {
            let po = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("po"));
            let Some(code) = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()) else { continue };
            if po && code != SOURCE && !found.iter().any(|(known, _)| *known == code) {
                found.push((code, path));
            }
        }
    }
    found
}

/// The catalogue of a language: a file on the disk if there is one, which
/// lets a user correct a built-in translation, else the built-in one.
fn catalog(code: &str) -> Option<&'static Catalog> {
    let mut loaded = LOADED.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(known) = loaded.iter().find(|c| c.code == code) {
        return Some(known);
    }
    let text = files()
        .into_iter()
        .find(|(candidate, _)| candidate == code)
        .and_then(|(_, path)| std::fs::read_to_string(path).ok())
        .or_else(|| BUILT_IN.iter().find(|(candidate, _)| *candidate == code).map(|(_, text)| (*text).to_owned()))?;
    let parsed: &'static Catalog = Box::leak(Box::new(Catalog::parse(code, &text)));
    loaded.push(parsed);
    Some(parsed)
}

/// A language built into the program.
pub fn built_in(code: &str) -> Option<Catalog> {
    BUILT_IN.iter().find(|(candidate, _)| *candidate == code).map(|(_, text)| Catalog::parse(code, text))
}

/// Every language that can be chosen: code and name, English first.
pub fn languages() -> Vec<(String, String)> {
    let mut codes: Vec<String> = BUILT_IN.iter().map(|(code, _)| (*code).to_owned()).collect();
    for (code, _) in files() {
        if !codes.contains(&code) {
            codes.push(code);
        }
    }
    let mut list: Vec<(String, String)> = codes.into_iter().filter_map(|code| catalog(&code).map(|c| (code, c.name.clone()))).collect();
    list.sort_by_key(|a| a.1.to_lowercase());
    list.insert(0, (SOURCE.to_owned(), "English".to_owned()));
    list
}

/// Switches the interface to a language. An unknown code gives English.
pub fn set_language(code: &str) {
    let chosen = if code.is_empty() || code == SOURCE { None } else { catalog(code) };
    *CURRENT.write().unwrap_or_else(|e| e.into_inner()) = chosen;
}

/// Code of the language in use.
pub fn language() -> String {
    active().map_or_else(|| SOURCE.to_owned(), |c| c.code.clone())
}

fn active() -> Option<&'static Catalog> {
    // The tests were written against the French texts: they keep checking them,
    // which also checks the French catalogue against what the program showed
    // before it was translated.
    #[cfg(test)]
    {
        static FRENCH: std::sync::Once = std::sync::Once::new();
        FRENCH.call_once(|| set_language("fr"));
    }
    *CURRENT.read().unwrap_or_else(|e| e.into_inner())
}

/// The text in the language in use, or the text itself when that language
/// does not have it.
pub fn tr(text: &str) -> &str {
    active().and_then(|catalog| catalog.get(text)).unwrap_or(text)
}

fn write_spec(out: &mut String, value: &dyn Display, spec: &str) {
    let mut rest = spec.trim_end_matches('?');
    let align = rest.chars().next().filter(|c| matches!(c, '<' | '>' | '^'));
    if align.is_some() {
        rest = &rest[1..];
    }
    let zero = rest.starts_with('0') && rest.len() > 1 && !rest.starts_with("0.");
    if zero {
        rest = &rest[1..];
    }
    let (width, precision) = rest.split_once('.').map_or((rest, None), |(w, p)| (w, p.parse::<usize>().ok()));
    let w = width.parse::<usize>().unwrap_or(0);
    let _ = match (zero, align, precision) {
        (true, _, Some(p)) => write!(out, "{value:0w$.p$}"),
        (true, _, None) => write!(out, "{value:0w$}"),
        (false, Some('>'), Some(p)) => write!(out, "{value:>w$.p$}"),
        (false, Some('>'), None) => write!(out, "{value:>w$}"),
        (false, Some('^'), Some(p)) => write!(out, "{value:^w$.p$}"),
        (false, Some('^'), None) => write!(out, "{value:^w$}"),
        (false, Some(_), Some(p)) => write!(out, "{value:<w$.p$}"),
        (false, Some(_), None) => write!(out, "{value:<w$}"),
        (false, None, Some(p)) => write!(out, "{value:w$.p$}"),
        (false, None, None) => write!(out, "{value:w$}"),
    };
}

/// Puts values into a translated text, as `format!` does for a text known
/// when the program is built: `{}` takes the next value, `{0}` a value by its
/// place (a translation may need another order), `{name}` a named one; a
/// width or a number of decimals may follow a colon (`{:.1}`, `{:02}`).
/// A placeholder without a value is left as it is.
pub fn fill(template: &str, positional: &[&dyn Display], named: &[(&str, &dyn Display)]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(at) = rest.find(['{', '}']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if tail.starts_with("{{") || tail.starts_with("}}") || tail.starts_with('}') {
            out.push_str(&tail[..1]);
            rest = &tail[if tail.starts_with("{{") || tail.starts_with("}}") { 2 } else { 1 }..];
            continue;
        }
        let Some(close) = tail.find('}') else {
            out.push_str(tail);
            return out;
        };
        let inner = &tail[1..close];
        let (name, spec) = inner.split_once(':').unwrap_or((inner, ""));
        let value: Option<&dyn Display> = if name.is_empty() {
            next += 1;
            positional.get(next - 1).copied()
        } else if let Ok(place) = name.parse::<usize>() {
            positional.get(place).copied()
        } else {
            named.iter().find(|(known, _)| *known == name).map(|(_, value)| *value)
        };
        match value {
            Some(value) => write_spec(&mut out, value, spec),
            None => out.push_str(&tail[..=close]),
        }
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// `format!` for a text that is translated: `tf!("Track {}", n)`. Values the
/// text names directly are listed in brackets: `tf!("{count} notes" [count])`.
macro_rules! tf {
    ($text:literal $([$($name:ident),* $(,)?])? $(, $value:expr)* $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::tr($text),
            &[$(&$value as &dyn ::std::fmt::Display),*],
            &[$($((stringify!($name), &$name as &dyn ::std::fmt::Display)),*)?],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_catalogue_is_read_with_its_escapes_and_long_lines() {
        let text = "# comment\nmsgid \"\"\nmsgstr \"\"\n\"Language: xx\\n\"\n\"X-Language-Name: Essai\\n\"\n\n\
msgid \"Open\"\nmsgstr \"Ouvrir\"\n\n#: somewhere.rs\nmsgid \"Two \"\n\"lines\\n\"\nmsgstr \"\"\n\"Deux \"\n\"lignes\\n\"\n\n\
msgid \"Say \\\"hi\\\"\"\nmsgstr \"Dire « salut »\"\n\nmsgid \"Not done\"\nmsgstr \"\"\n\n\
msgctxt \"menu\"\nmsgid \"Ignored context\"\nmsgstr \"Contexte\"\n\nmsgid \"One\"\nmsgid_plural \"Many\"\nmsgstr[0] \"Un\"\nmsgstr[1] \"Plusieurs\"\n";
        let catalog = Catalog::parse("xx", text);
        assert_eq!((catalog.code.as_str(), catalog.name.as_str()), ("xx", "Essai"));
        assert_eq!(catalog.get("Open"), Some("Ouvrir"));
        assert_eq!(catalog.get("Two lines\n"), Some("Deux lignes\n"));
        assert_eq!(catalog.get("Say \"hi\""), Some("Dire « salut »"));
        assert_eq!(catalog.get("Not done"), None, "an empty translation leaves the English text");
        assert_eq!(catalog.get("One"), None);
        assert_eq!(catalog.source_of("Ouvrir"), Some("Open"));
        // A catalogue without a name is called by its code; garbage is not an error.
        assert_eq!(Catalog::parse("yy", "msgid \"a\"\nmsgstr \"b\"").name, "yy");
        assert_eq!(Catalog::parse("zz", "\u{feff}not a catalogue\nmsgstr\n\"").len(), 0);
    }

    #[test]
    fn values_are_put_into_translated_texts() {
        let (count, name) = (3, "Essai");
        assert_eq!(fill("{} of {}", &[&1, &2], &[]), "1 of 2");
        assert_eq!(fill("{1} sur {0}", &[&1, &2], &[]), "2 sur 1");
        assert_eq!(fill("{count} notes in {name}", &[], &[("count", &count), ("name", &name)]), "3 notes in Essai");
        assert_eq!(fill("{:.1} s, {:02}, {:>3}|{:<3}|", &[&1.26f32, &7, &5, &5], &[]), "1.3 s, 07,   5|5  |");
        assert_eq!(fill("{seconds:.0} s", &[], &[("seconds", &2.6f64)]), "3 s");
        assert_eq!(fill("{{}} {} }", &[&"x"], &[]), "{} x }");
        assert_eq!(fill("{} {missing} {", &[], &[]), "{} {missing} {", "what has no value is left alone");
        // The macro, in the tests' language.
        assert_eq!(tf!("Track {}", 4), "Piste 4");
        assert_eq!(tf!("{count} notes recorded." [count]), "3 notes enregistrées.");
        assert_eq!(tf!("A text nobody translated: {}", name), "A text nobody translated: Essai");
        assert_eq!(tr("Cancel"), "Annuler");
        assert_eq!(tr("A text nobody translated"), "A text nobody translated");
    }

    /// The parts of a text that values replace, sorted.
    fn placeholders(text: &str) -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find('{') {
            let Some(close) = rest[open..].find('}') else { break };
            found.push(rest[open..=open + close].to_owned());
            rest = &rest[open + close + 1..];
        }
        found.sort();
        found
    }

    /// Every text of the code between `tr("` or `tf!("` and the closing quote.
    fn texts_in_the_code() -> Vec<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = Vec::new();
        for file in std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()) {
            let name = file.file_name().unwrap().to_string_lossy().into_owned();
            if name == "tests.rs" || name == "i18n.rs" {
                continue;
            }
            let source = std::fs::read_to_string(&file).unwrap();
            for marker in ["tr(\"", "tf!(\""] {
                let mut rest = source.as_str();
                while let Some(at) = rest.find(marker) {
                    let body = &rest[at + marker.len()..];
                    // `push_str("` and the like end the same way.
                    if rest[..at].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_') {
                        rest = body;
                        continue;
                    }
                    let mut end = 0;
                    let bytes = body.as_bytes();
                    while bytes[end] != b'"' {
                        end += if bytes[end] == b'\\' { 2 } else { 1 };
                    }
                    found.push(quoted(&format!("\"{}\"", &body[..end])).unwrap());
                    rest = &body[end..];
                }
            }
        }
        found.sort();
        found.dedup();
        found
    }

    #[test]
    fn every_built_in_language_translates_every_text() {
        let wanted = texts_in_the_code();
        assert!(wanted.len() > 400, "{} texts found in the code", wanted.len());
        let codes: Vec<&str> = BUILT_IN.iter().map(|(code, _)| *code).collect();
        for expected in ["de", "es", "fr", "it", "pt"] {
            assert!(codes.contains(&expected), "{expected} is built in");
        }
        let mut names = Vec::new();
        for (code, text) in BUILT_IN {
            let catalog = Catalog::parse(code, text);
            assert_ne!(catalog.name, *code, "{code} gives its name");
            names.push(catalog.name.clone());
            let missing: Vec<&String> = wanted.iter().filter(|text| catalog.get(text).is_none()).collect();
            if !missing.is_empty() {
                // The whole list, one text a line, for whoever translates them.
                let list: Vec<&str> = missing.iter().map(|text| text.as_str()).collect();
                let _ = std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/i18n-missing.txt"), list.join("\n"));
            }
            assert!(missing.is_empty(), "{code} lacks {} texts, among them {:?}", missing.len(), &missing[..missing.len().min(8)]);
            for (source, translated) in catalog.entries() {
                assert_eq!(placeholders(source), placeholders(translated), "{code}: values of « {source} » in « {translated} »");
                assert_eq!(source.starts_with(' '), translated.starts_with(' '), "{code}: leading space of « {source} »");
                assert_eq!(source.ends_with(' '), translated.ends_with(' '), "{code}: trailing space of « {source} »");
            }
        }
        names.sort();
        names.dedup();
        assert_eq!(names.len(), BUILT_IN.len(), "each language has its own name");
        let listed = languages();
        assert_eq!(listed[0], ("en".to_owned(), "English".to_owned()));
        assert_eq!(listed.len(), BUILT_IN.len() + 1);
    }

    #[test]
    fn the_texts_of_the_engine_are_translated_too() {
        // Names that come from the engine are shown through `tr` as well.
        use og_engine::{loops, patches, templates};
        let mut wanted: Vec<String> = Vec::new();
        for patch in patches::builtin() {
            wanted.push(patch.name);
            wanted.push(patch.category);
        }
        for one in loops::library() {
            wanted.push(one.name.to_owned());
        }
        for tag in loops::INSTRUMENTS.iter().chain(&loops::GENRES).chain(&loops::MOODS) {
            wanted.push((*tag).to_owned());
        }
        for kind in [og_engine::TrackKind::Instrument, og_engine::TrackKind::Audio, og_engine::TrackKind::Guitar, og_engine::TrackKind::Drummer] {
            wanted.push(kind.label().to_owned());
        }
        for kind in og_engine::PedalKind::ALL {
            wanted.push(kind.label().to_owned());
        }
        for param in og_engine::FxParam::FIXED.into_iter().chain([og_engine::FxParam::Pedal(0)]) {
            wanted.push(param.label().to_owned());
        }
        for template in templates::all() {
            wanted.push(template.name.to_owned());
            wanted.push(template.description.to_owned());
        }
        wanted.sort();
        wanted.dedup();
        for (code, text) in BUILT_IN {
            let catalog = Catalog::parse(code, text);
            let missing: Vec<&String> = wanted.iter().filter(|text| catalog.get(text).is_none()).collect();
            assert!(missing.is_empty(), "{code} lacks {:?}", &missing[..missing.len().min(8)]);
        }
    }
}
