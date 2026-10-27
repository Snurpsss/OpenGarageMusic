//! Finding and downloading freely licensed sound banks from the web. Nothing
//! here runs unless the user opens the download window and asks for it.

use crate::i18n::tr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};

use sha2::{Digest, Sha256};

/// What a download is: sounds to play, programs that process sound,
/// captures of real gear, or impulse responses of cabinets and rooms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Banks,
    Effects,
    Captures,
    Impulses,
}

impl Kind {
    /// File types of this kind that the application can use.
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Kind::Banks => &og_engine::synth::BANK_EXTENSIONS,
            Kind::Effects => &["clap"],
            Kind::Captures => &og_engine::capture::EXTENSIONS,
            Kind::Impulses => &og_engine::convolution::EXTENSIONS,
        }
    }

    /// Name of the folder that holds downloads of this kind.
    pub fn folder(self) -> &'static str {
        // Names on the disk, the same in every language: those of the first
        // versions, so that what was downloaded then is still found.
        match self {
            Kind::Banks => "Banques",
            Kind::Effects => "Effets",
            Kind::Captures => "Captures",
            Kind::Impulses => "Impulsions",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Provider {
    MuseScore,
    MusicalArtifacts,
    SfzInstruments,
    Polyphone,
    FreePats,
    /// Free plug-in projects that publish Windows builds on GitHub.
    ClapGithub,
    ClapDatabase,
    OpenAudio,
    X42,
}

/// Free plug-in projects that publish CLAP builds for Windows as zip archives:
/// (repository, name, what it is, licence of the repository). Each archive was
/// checked on 2026-10-11 to hold at least one `.clap` file.
pub const CLAP_PROJECTS: [(&str, &str, &str, &str); 5] = [
    ("michaelwillis/dragonfly-reverb", "Dragonfly Reverb", "Four reverbs: hall, room, plate, early reflections", "GPL-3.0"),
    ("wolf-plugins/wolf-shaper", "Wolf Shaper", "Distortion with a drawn curve", "GPL-3.0"),
    ("baconpaul/six-sines", "Six Sines", "Instrument: FM synthesizer", "MIT"),
    ("asb2m10/dexed", "Dexed", "Instrument: six-operator FM synthesizer", "GPL-3.0"),
    ("surge-synthesizer/surge", "Surge XT", "Instrument: synthesizer, with its effects as a separate plug-in", "GPL-3.0"),
];

impl Provider {
    pub const BANKS: [Provider; 5] =
        [Provider::MuseScore, Provider::MusicalArtifacts, Provider::SfzInstruments, Provider::Polyphone, Provider::FreePats];
    pub const EFFECTS: [Provider; 4] = [Provider::ClapGithub, Provider::X42, Provider::ClapDatabase, Provider::OpenAudio];

    pub fn of(kind: Kind) -> &'static [Provider] {
        match kind {
            Kind::Banks => &Self::BANKS,
            Kind::Effects => &Self::EFFECTS,
            Kind::Captures | Kind::Impulses => &[],
        }
    }

    pub fn kind(self) -> Kind {
        if Self::EFFECTS.contains(&self) { Kind::Effects } else { Kind::Banks }
    }

    pub fn label(self) -> &'static str {
        match self {
            Provider::X42 => "x42-plugins (LV2)",
            Provider::ClapGithub => tr("Free projects (GitHub)"),
            Provider::ClapDatabase => "CLAP Database",
            Provider::OpenAudio => "OpenAudio",
            Provider::MuseScore => "MuseScore",
            Provider::MusicalArtifacts => "Musical Artifacts",
            Provider::SfzInstruments => "SFZ Instruments",
            Provider::Polyphone => "Polyphone",
            Provider::FreePats => "FreePats",
        }
    }

    pub fn site(self) -> &'static str {
        match self {
            Provider::X42 => "https://x42-plugins.com/x42/",
            Provider::ClapGithub => "https://github.com/topics/clap-plugin",
            Provider::ClapDatabase => "https://clapdb.tech/",
            Provider::OpenAudio => "https://github.com/webprofusion/OpenAudio",
            Provider::MuseScore => "https://musescore.org/fr/manuel/soundfonts-et-fichiers-sfz",
            Provider::MusicalArtifacts => "https://musical-artifacts.com/",
            Provider::SfzInstruments => "https://sfzinstruments.github.io/",
            Provider::Polyphone => "https://www.polyphone.io/fr/soundfonts",
            Provider::FreePats => "https://freepats.zenvoid.org/",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Provider::ClapGithub => tr("Plug-ins in the CLAP format whose code is free, published by their authors on GitHub. The latest version for Windows is offered. A plug-in is a program: it runs inside the application with your rights."),
            Provider::X42 => tr("Free plug-ins in the LV2 format (equaliser, limiter, compressor, automatic tuning…), distributed by their author as installers. Install them from the site: those in the standard Windows LV2 folder then appear in “Installed effects”. Their windows are not shown, only their settings are."),
            Provider::ClapDatabase => tr("A directory of CLAP plug-ins, free or commercial. Download the plug-in there, then add its .clap file with “Import a plug-in”."),
            Provider::OpenAudio => tr("A list of several hundred open-source audio programs. This site only holds links: each project publishes its files its own way. Choose a project: the application then checks its licence and looks, in its latest version, for a Windows archive. Many exist only as an installer and cannot be installed from here."),
            Provider::MuseScore => tr("The General MIDI banks of the free score editor MuseScore, under the MIT licence."),
            Provider::MusicalArtifacts => tr("A community repository where each file carries its licence. Only those declared under a free licence are listed; the licence is declared by the person who uploaded the file."),
            Provider::SfzInstruments => tr("A collection of SFZ instruments hosted on GitHub. Only repositories that declare a free licence are listed."),
            Provider::Polyphone => tr("Each bank has its own terms and the download goes through the site. Download the file there, then import it with “Import a sound bank”."),
            Provider::FreePats => tr("Free banks distributed as archives that the application cannot open yet. Download them from the site, unpack them, then import the .sf2 or .sfz file."),
        }
    }

    /// Whether the application can list and download from this site itself.
    pub fn direct(self) -> bool {
        !matches!(self, Provider::Polyphone | Provider::FreePats | Provider::ClapDatabase | Provider::X42)
    }

    fn folder(self) -> &'static str {
        match self {
            Provider::X42 => "x42",
            Provider::ClapGithub => "GitHub",
            Provider::ClapDatabase => "ClapDatabase",
            Provider::OpenAudio => "OpenAudio",
            Provider::MuseScore => "MuseScore",
            Provider::MusicalArtifacts => "MusicalArtifacts",
            Provider::SfzInstruments => "SfzInstruments",
            Provider::Polyphone => "Polyphone",
            Provider::FreePats => "FreePats",
        }
    }
}

/// Something that can be downloaded.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub provider: Provider,
    pub name: String,
    pub author: String,
    pub license: String,
    pub url: String,
    pub file_name: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    /// Web page describing the item.
    pub page: String,
    pub note: String,
}

/// A name fit for a file or a folder: letters, digits, dots, dashes and underscores.
pub(crate) fn safe_name(text: &str) -> String {
    let cleaned: String = text.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    let trimmed = cleaned.trim_matches(|c| c == '.' || c == '_').to_owned();
    if trimmed.is_empty() { "fichier".to_owned() } else { trimmed.chars().take(80).collect() }
}

/// Licence names used by Musical Artifacts that are free licences: public
/// domain, attribution, share-alike, and free software licences. Anything with
/// "nc" (no commercial use), "nd" (no derivatives), "copyright" or "gray" is out.
pub fn artifact_license_is_free(license: &str) -> bool {
    matches!(license, "public" | "by" | "by-3" | "by-sa" | "by-sa-3" | "wtfpl" | "gpl" | "gpl-3" | "mit")
}

/// SPDX identifiers of free licences, as reported by GitHub.
pub fn spdx_is_free(id: &str) -> bool {
    matches!(
        id,
        "CC0-1.0" | "CC-BY-4.0" | "CC-BY-3.0" | "CC-BY-SA-4.0" | "CC-BY-SA-3.0" | "Unlicense" | "MIT" | "Apache-2.0"
            | "BSD-2-Clause" | "BSD-3-Clause" | "GPL-2.0" | "GPL-3.0" | "AGPL-3.0" | "LGPL-2.1" | "LGPL-3.0" | "WTFPL" | "Artistic-2.0"
    )
}

/// Which of a release's files is the 64-bit Windows zip, if any. Installers,
/// other systems, 32-bit and ARM builds are passed over.
pub fn pick_windows_asset(names: &[String]) -> Option<usize> {
    let score = |name: &str| -> i32 {
        let n = name.to_lowercase();
        let unwanted = ["linux", "mac", "osx", "darwin", "arm", "win32", "x86.", "i686", "source", "src", "portable", "lv2", "vst2", "debug", "symbols"];
        if !n.ends_with(".zip") || !n.contains("win") || unwanted.iter().any(|u| n.contains(u)) {
            return -1;
        }
        let mut score = 1;
        for (hint, bonus) in [("64", 2), ("clap", 3), ("vst3", 2), ("pluginsonly", 3), ("juce7", -1), ("beta", -2)] {
            if n.contains(hint) {
                score += bonus;
            }
        }
        score
    };
    names.iter().enumerate().map(|(i, n)| (score(n), i)).filter(|(s, _)| *s > 0).max_by_key(|(s, i)| (*s, std::cmp::Reverse(*i))).map(|(_, i)| i)
}

/// Reads GitHub's description of a project's latest release and returns the
/// download for 64-bit Windows.
pub fn parse_release(json: &str, project: &(&str, &str, &str, &str)) -> Option<Item> {
    let (repository, name, description, license) = *project;
    let description = tr(description);
    let release: serde_json::Value = serde_json::from_str(json).ok()?;
    let assets = release.get("assets")?.as_array()?;
    let names: Vec<String> = assets.iter().map(|a| a.get("name").and_then(|n| n.as_str()).unwrap_or("").to_owned()).collect();
    let asset = &assets[pick_windows_asset(&names)?];
    let url = asset.get("browser_download_url")?.as_str()?;
    if !url.starts_with(&format!("https://github.com/{repository}/releases/download/")) || !spdx_is_free(license) {
        return None;
    }
    let version = release.get("tag_name").and_then(|t| t.as_str()).unwrap_or("");
    Some(Item {
        provider: Provider::ClapGithub,
        name: name.to_owned(),
        author: repository.split('/').next().unwrap_or("").to_owned(),
        license: license.to_owned(),
        url: url.to_owned(),
        file_name: safe_name(&names[pick_windows_asset(&names)?]),
        size: asset.get("size").and_then(serde_json::Value::as_u64),
        sha256: None,
        page: format!("https://github.com/{repository}"),
        note: tf!("{description}. Version {version}." [description, version]),
    })
}

/// Reads OpenAudio's list: a table of links to projects, with a description
/// and a type. The list says neither the licence nor where the files are: the
/// items come out without a download address, to be completed by [`resolve`].
pub fn parse_openaudio(readme: &str) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::new();
    for line in readme.lines() {
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
        let Some((name, page)) = cells.first().and_then(|cell| cell.strip_prefix('[')?.strip_suffix(')')?.split_once("](")) else { continue };
        let Some(repository) = page.strip_prefix("https://github.com/") else { continue };
        let repository = repository.trim_end_matches('/');
        let plain = |text: &str| text.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
        let well_formed = repository.split_once('/').is_some_and(|(owner, repo)| !owner.is_empty() && !repo.is_empty() && plain(owner) && plain(repo));
        if !well_formed || cells.len() < 3 || items.iter().any(|i| i.page.eq_ignore_ascii_case(&format!("https://github.com/{repository}"))) {
            continue;
        }
        let kind = match cells[2..].iter().find(|c| matches!(**c, "Effect" | "Instrument" | "Misc")) {
            Some(&"Effect") => tr("Effect"),
            Some(&"Instrument") => tr("Instrument"),
            _ => tr("Miscellaneous"),
        };
        items.push(Item {
            provider: Provider::OpenAudio,
            name: name.trim().to_owned(),
            author: repository.split('/').next().unwrap_or("").to_owned(),
            license: tr("to be checked").to_owned(),
            url: String::new(),
            file_name: String::new(),
            size: None,
            sha256: None,
            page: format!("https://github.com/{repository}"),
            note: format!("{kind}. {}", cells[1]),
        });
    }
    items
}

/// Completes an item of a list of projects from what GitHub says of the
/// project (`repo_json`) and of its latest release (`release_json`): the
/// licence must be a free one and the release must hold a Windows archive.
pub fn resolve_from(item: &Item, repo_json: &str, release_json: &str) -> Result<Item, String> {
    let repository = item.page.strip_prefix("https://github.com/").ok_or(tr("this project is not hosted on GitHub"))?;
    let repo: serde_json::Value = serde_json::from_str(repo_json).map_err(|_| tr("unreadable answer from GitHub").to_owned())?;
    let license = repo.get("license").and_then(|l| l.get("spdx_id")).and_then(|id| id.as_str()).unwrap_or("");
    if license.is_empty() || license == "NOASSERTION" {
        return Err(tr("this project declares no recognised licence: nothing is downloaded from here. See its page.").to_owned());
    }
    if !spdx_is_free(license) {
        return Err(tf!("the licence of this project ({license}) is not in the list of accepted free licences." [license]));
    }
    let description = item.note.trim_end_matches('.');
    let mut found = parse_release(release_json, &(repository, &item.name, description, license)).ok_or(
        tr("the latest version of this project has no .zip archive for 64-bit Windows (installer, other system or other format only). Download it from its page, then use “Import a plug-in”."),
    )?;
    found.provider = item.provider;
    found.note.push_str(tr(" The archive is not guaranteed to hold a CLAP or VST3 plug-in: that is checked after the download."));
    Ok(found)
}

/// Asks GitHub about one project of a list. Blocks on the network.
pub fn resolve(item: &Item) -> Result<Item, String> {
    let repository = item.page.strip_prefix("https://github.com/").ok_or(tr("this project is not hosted on GitHub"))?;
    let ask = |url: String, absent: &str| {
        fetch_text(&url).map_err(|e| {
            if e.contains("403") || e.contains("429") {
                tr("GitHub limits anonymous requests (60 per hour): try again later.").to_owned()
            } else if e.contains("404") {
                absent.to_owned()
            } else {
                e
            }
        })
    };
    let repo = ask(format!("https://api.github.com/repos/{repository}"), tr("this project no longer exists at this address."))?;
    let release = ask(format!("https://api.github.com/repos/{repository}/releases/latest"), tr("this project publishes no version to download: it has to be built from source."))?;
    resolve_from(item, &repo, &release)
}

/// Looks an item up on another thread.
pub fn start_resolving(item: Item) -> mpsc::Receiver<Result<Item, String>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(resolve(&item));
    });
    receive
}

/// The fixed list for MuseScore's mirror. Both files are MIT-licensed.
pub fn musescore_items() -> Vec<Item> {
    let base = "https://ftp.osuosl.org/pub/musescore/soundfont";
    let item = |name: &str, file: &str, url: String, size: u64, note: &str| Item {
        provider: Provider::MuseScore,
        name: name.to_owned(),
        author: tr("Frank Wen; adaptations by Michael Cowgill and S. Christian Collins").to_owned(),
        license: "MIT".to_owned(),
        url,
        file_name: file.to_owned(),
        size: Some(size),
        sha256: None,
        page: "https://musescore.org/fr/manuel/soundfonts-et-fichiers-sfz".to_owned(),
        note: note.to_owned(),
    };
    vec![
        item(
            "FluidR3 GM",
            "fluid-soundfont.zip",
            format!("{base}/fluid-soundfont.zip"),
            131_800_000,
            tr("Pianos, strings, brass, guitars, drums. The bank best rendered by the OpenGarageMusic player."),
        ),
        item(
            "MuseScore General (SF3)",
            "MuseScore_General.sf3",
            format!("{base}/MuseScore_General/MuseScore_General.sf3"),
            39_950_000,
            tr("Lighter, but its grand pianos come out too quiet with the current player."),
        ),
    ]
}

/// Reads a page of Musical Artifacts' listing (JSON), keeping free bank files.
pub fn parse_musical_artifacts(json: &str) -> Vec<Item> {
    let Ok(serde_json::Value::Array(entries)) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    let text = |v: &serde_json::Value, key: &str| v.get(key).and_then(|x| x.as_str()).unwrap_or("").to_owned();
    entries
        .iter()
        .filter_map(|entry| {
            let (file, license) = (text(entry, "file"), text(entry, "license"));
            let lower = file.to_lowercase();
            let extension = lower.rsplit('.').next().unwrap_or("");
            if !file.starts_with("https://") || !artifact_license_is_free(&license) || !matches!(extension, "sf2" | "sf3" | "zip") {
                return None;
            }
            let file_name = safe_name(file.rsplit('/').next().unwrap_or("banque.sf2"));
            Some(Item {
                provider: Provider::MusicalArtifacts,
                name: text(entry, "name"),
                author: text(entry, "author"),
                license: tf!("{license} (as declared by the uploader)" [license]),
                sha256: Some(text(entry, "file_hash")).filter(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())),
                page: text(entry, "url").trim_end_matches(".json").to_owned(),
                url: file,
                file_name,
                size: None,
                note: String::new(),
            })
        })
        .collect()
}

/// Reads GitHub's list of the repositories of the sfzinstruments organisation.
pub fn parse_github_repos(json: &str) -> Vec<Item> {
    let Ok(serde_json::Value::Array(repos)) = serde_json::from_str::<serde_json::Value>(json) else { return Vec::new() };
    let mut items: Vec<Item> = repos
        .iter()
        .filter_map(|repo| {
            let name = repo.get("name")?.as_str()?;
            let license = repo.get("license")?.get("spdx_id")?.as_str()?;
            let branch = repo.get("default_branch")?.as_str()?;
            let page = repo.get("html_url")?.as_str()?;
            // Not instruments: the site itself, templates and tutorials.
            let tooling = ["sfzinstruments.github.io", "sfztemplates", "mappings", "tutorial"];
            if !spdx_is_free(license) || tooling.iter().any(|t| name.contains(t)) || !page.starts_with("https://github.com/") {
                return None;
            }
            Some(Item {
                provider: Provider::SfzInstruments,
                name: name.replace(['_', '-'], " "),
                author: name.split('.').next().filter(|_| name.contains('.')).unwrap_or("").to_owned(),
                license: license.to_owned(),
                url: format!("{page}/archive/refs/heads/{branch}.zip"),
                file_name: format!("{}.zip", safe_name(name)),
                // GitHub reports the repository size in kilobytes.
                size: repo.get("size").and_then(serde_json::Value::as_u64).map(|kb| kb * 1024),
                sha256: None,
                page: page.to_owned(),
                note: tr("SFZ instrument; some use settings that the player ignores.").to_owned(),
            })
        })
        .collect();
    items.sort_by_key(|a| a.name.to_lowercase());
    items
}

/// A web client that gives up when nothing has been read for `patience` seconds
/// in all: short for listings, long for large files (which can be cancelled).
pub(crate) fn agent(patience: u64) -> Result<ureq::Agent, String> {
    let tls = native_tls::TlsConnector::new().map_err(|e| tf!("secure connection unavailable: {e}" [e]))?;
    Ok(ureq::AgentBuilder::new()
        .tls_connector(Arc::new(tls))
        .timeout_connect(std::time::Duration::from_secs(15))
        .timeout_read(std::time::Duration::from_secs(patience))
        .user_agent(concat!("OpenGarageMusic/", env!("OG_VERSION")))
        .build())
}

fn fetch_text(url: &str) -> Result<String, String> {
    let response = agent(30)?.get(url).call().map_err(|e| tf!("the site did not answer ({e})" [e]))?;
    response.into_string().map_err(|e| tf!("unreadable answer ({e})" [e]))
}

/// The site's answer to something it refused, in a sentence.
pub fn site_error(status: u16, body: &str) -> String {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    match value.get("error").or_else(|| value.get("message")).and_then(|e| e.as_str()) {
        Some(error) if !error.is_empty() => tf!("the site answered {status}: {error}" [status, error]),
        _ => tf!("the site answered {status}" [status]),
    }
}

pub(crate) fn encode_query(text: &str) -> String {
    text.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect()
}

/// Asks a site what it offers. Blocks on the network: call it from a thread.
pub fn list(provider: Provider, search: &str) -> Result<Vec<Item>, String> {
    match provider {
        Provider::MuseScore => Ok(musescore_items()),
        Provider::MusicalArtifacts => {
            let mut items = Vec::new();
            for format in ["sf2", "sf3"] {
                for page in 1..=3 {
                    let mut url = format!("https://musical-artifacts.com/artifacts.json?formats={format}&page={page}");
                    if !search.trim().is_empty() {
                        url.push_str(&format!("&q={}", encode_query(search.trim())));
                    }
                    let found = parse_musical_artifacts(&fetch_text(&url)?);
                    items.extend(found.into_iter().filter(|new| !items.iter().any(|old: &Item| old.url == new.url)).collect::<Vec<_>>());
                }
            }
            Ok(items)
        }
        Provider::SfzInstruments => {
            let all = parse_github_repos(&fetch_text("https://api.github.com/orgs/sfzinstruments/repos?per_page=100")?);
            let wanted = search.trim().to_lowercase();
            Ok(all.into_iter().filter(|i| wanted.is_empty() || i.name.to_lowercase().contains(&wanted)).collect())
        }
        Provider::ClapGithub => {
            let wanted = search.trim().to_lowercase();
            let mut items = Vec::new();
            let mut last_error = None;
            for project in &CLAP_PROJECTS {
                if !wanted.is_empty() && !project.1.to_lowercase().contains(&wanted) {
                    continue;
                }
                match fetch_text(&format!("https://api.github.com/repos/{}/releases/latest", project.0)) {
                    Ok(json) => items.extend(parse_release(&json, project)),
                    Err(e) => last_error = Some(e),
                }
            }
            // One project being unreachable should not hide the others.
            match (items.is_empty(), last_error) {
                (true, Some(error)) => Err(error),
                _ => Ok(items),
            }
        }
        Provider::OpenAudio => {
            // The list is one file; nothing is asked about each project here.
            let all = parse_openaudio(&fetch_text("https://raw.githubusercontent.com/webprofusion/OpenAudio/master/README.md")?);
            let wanted = search.trim().to_lowercase();
            Ok(all.into_iter().filter(|i| wanted.is_empty() || i.name.to_lowercase().contains(&wanted) || i.note.to_lowercase().contains(&wanted)).collect())
        }
        Provider::Polyphone | Provider::FreePats | Provider::ClapDatabase | Provider::X42 => Ok(Vec::new()),
    }
}

#[derive(Default)]
pub struct Progress {
    pub received: u64,
    pub total: Option<u64>,
    pub stage: &'static str,
}

/// Unpacks a zip archive into `dest`, refusing entries that would land outside it.
pub fn extract_zip(archive: &Path, dest: &Path) -> Result<(), String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| tf!("unreadable archive ({e})" [e]))?;
    if zip.len() > 50_000 {
        return Err(tr("abnormally large archive").to_owned());
    }
    let mut written = 0u64;
    for index in 0..zip.len() {
        let mut entry = zip.by_index(index).map_err(|e| e.to_string())?;
        // `enclosed_name` is None for absolute paths and paths that climb with "..".
        let Some(relative) = entry.enclosed_name() else { continue };
        let target = dest.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&target).map_err(|e| e.to_string())?;
        // Reads at most what is left of the budget, so a zip bomb cannot fill the disk.
        let budget = 8_000_000_000u64.saturating_sub(written);
        written += std::io::copy(&mut (&mut entry).take(budget), &mut out).map_err(|e| e.to_string())?;
        if written >= 8_000_000_000 {
            return Err(tr("the archive is too large once unpacked").to_owned());
        }
    }
    Ok(())
}

/// Files of the given types under `folder`, at most a few levels down.
pub fn find_files(folder: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: u32, extensions: &[&str], out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.flatten().map(|e| e.path()) {
            // On Windows a CLAP plug-in is a single file; elsewhere it may be a folder.
            let matches = path.extension().is_some_and(|e| extensions.contains(&e.to_string_lossy().to_lowercase().as_str()));
            if matches && path.is_file() {
                out.push(path);
            } else if path.is_dir() && depth < 6 {
                walk(&path, depth + 1, extensions, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(folder, 0, extensions, &mut out);
    out.sort();
    out.truncate(40);
    out
}

/// LV2 plug-in folders (`something.lv2` holding a manifest) under `folder`.
pub fn find_bundles(folder: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()) {
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lv2")) && path.join("manifest.ttl").is_file() {
                out.push(path);
            } else if depth < 5 {
                walk(&path, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(folder, 0, &mut out);
    out.sort();
    out.truncate(200);
    out
}

/// VST3 plug-ins under `folder`: files or folders named `something.vst3`. A
/// folder of that name is one plug-in; what it holds is not listed.
pub fn find_vst3(folder: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.flatten().map(|e| e.path()) {
            if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("vst3")) {
                out.push(path);
            } else if path.is_dir() && depth < 6 {
                walk(&path, depth + 1, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(folder, 0, &mut out);
    out.sort();
    out.truncate(200);
    out
}

/// What a download brought that the application can use.
pub fn usable(folder: &Path, kind: Kind) -> Vec<PathBuf> {
    let mut found = find_files(folder, kind.extensions());
    if kind == Kind::Effects {
        found.extend(find_vst3(folder));
    }
    found
}

/// Bank files under `folder`.
#[cfg(test)]
pub fn find_banks(folder: &Path) -> Vec<PathBuf> {
    find_files(folder, &og_engine::synth::BANK_EXTENSIONS)
}

/// Folder an item is downloaded into: `data/<kind>/<site>/<name>`.
pub fn item_folder(data: &Path, item: &Item) -> PathBuf {
    data.join(item.provider.kind().folder()).join(item.provider.folder()).join(safe_name(&item.name))
}

/// Whether an item has already been downloaded: its folder holds the note
/// written at the end of a download and at least one usable file.
pub fn is_downloaded(data: &Path, item: &Item) -> bool {
    let folder = item_folder(data, item);
    folder.join("SOURCE.txt").is_file() && !usable(&folder, item.provider.kind()).is_empty()
}

/// Downloads `item`, checks it, unpacks it if it is an archive, and writes a
/// note of where it came from. Returns the bank files it brought.
pub fn download(item: &Item, sounds: &Path, progress: &Mutex<Progress>, cancel: &AtomicBool) -> Result<Vec<PathBuf>, String> {
    let set_stage = |stage: &'static str| {
        if let Ok(mut p) = progress.lock() {
            p.stage = stage;
        }
    };
    if !item.url.starts_with("https://") && !cfg!(test) {
        return Err(tr("only secure downloads (https) are accepted").to_owned());
    }
    let folder = item_folder(sounds, item);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    let name = safe_name(&item.file_name);
    let partial = folder.join(format!("{name}.part"));
    let finished = folder.join(&name);

    set_stage(tr("Downloading"));
    // Two hours: enough for a large bank on a slow line.
    let response = agent(7200)?.get(&item.url).call().map_err(|e| tf!("the download failed ({e})" [e]))?;
    let total = response.header("Content-Length").and_then(|v| v.parse::<u64>().ok());
    if let Ok(mut p) = progress.lock() {
        p.total = total.or(item.size);
    }
    let mut reader = response.into_reader();
    let mut out = std::fs::File::create(&partial).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut received = 0u64;
    let fail = |message: String| {
        let _ = std::fs::remove_file(&partial);
        Err(message)
    };
    loop {
        if cancel.load(Ordering::Relaxed) {
            drop(out);
            return fail(tr("download cancelled").to_owned());
        }
        let n = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                drop(out);
                return fail(tf!("the connection was cut ({e})" [e]));
            }
        };
        if let Err(e) = out.write_all(&buffer[..n]) {
            drop(out);
            return fail(tf!("cannot write ({e})" [e]));
        }
        hasher.update(&buffer[..n]);
        received += n as u64;
        if received > 6_000_000_000 {
            drop(out);
            return fail(tr("abnormally large file").to_owned());
        }
        if let Ok(mut p) = progress.lock() {
            p.received = received;
        }
    }
    drop(out);
    if total.is_some_and(|t| t != received) {
        return fail(tr("the file received is incomplete").to_owned());
    }
    let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if item.sha256.as_ref().is_some_and(|expected| !expected.eq_ignore_ascii_case(&digest)) {
        return fail(tr("the file received does not match the fingerprint announced by the site").to_owned());
    }
    std::fs::rename(&partial, &finished).map_err(|e| e.to_string())?;

    if name.to_lowercase().ends_with(".zip") {
        set_stage(tr("Unpacking"));
        let unpacked = extract_zip(&finished, &folder);
        let _ = std::fs::remove_file(&finished);
        unpacked?;
    }

    // What this is and where it came from, kept next to the files.
    let stamp = crate::sessions::now();
    let source = tf!(
        "Name: {}\nAuthor: {}\nLicence: {}\nSite: {}\nPage: {}\nDownloaded from: {}\nDate (Unix seconds): {stamp}\nSHA-256 of the downloaded file: {digest}\n" [stamp, digest],
        item.name,
        if item.author.is_empty() { tr("not given") } else { &item.author },
        item.license,
        item.provider.label(),
        item.page,
        item.url
    );
    std::fs::write(folder.join("SOURCE.txt"), source).map_err(|e| e.to_string())?;

    let kind = item.provider.kind();
    let found = usable(&folder, kind);
    if found.is_empty() {
        return Err(match kind {
            Kind::Banks => tr("the download holds no SF2, SF3 or SFZ bank").to_owned(),
            Kind::Effects => tr("the download holds no CLAP or VST3 plug-in (this project may only provide other formats)").to_owned(),
            Kind::Captures => tr("the download holds no capture (.nam or .json file)").to_owned(),
            Kind::Impulses => tr("the download holds no impulse response (.wav or .flac file)").to_owned(),
        });
    }
    Ok(found)
}

/// A download running on its own thread.
pub struct Job {
    pub item: Item,
    pub progress: Arc<Mutex<Progress>>,
    pub cancel: Arc<AtomicBool>,
    pub done: mpsc::Receiver<Result<Vec<PathBuf>, String>>,
}

pub fn start(item: Item, sounds: PathBuf) -> Job {
    let progress = Arc::new(Mutex::new(Progress::default()));
    let cancel = Arc::new(AtomicBool::new(false));
    let (send, done) = mpsc::channel();
    let (thread_item, thread_progress, thread_cancel) = (item.clone(), Arc::clone(&progress), Arc::clone(&cancel));
    std::thread::spawn(move || {
        let _ = send.send(download(&thread_item, &sounds, &thread_progress, &thread_cancel));
    });
    Job { item, progress, cancel, done }
}

/// Asks a site for its list on another thread.
pub fn start_listing(provider: Provider, search: String) -> mpsc::Receiver<Result<Vec<Item>, String>> {
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = send.send(list(provider, &search));
    });
    receive
}
