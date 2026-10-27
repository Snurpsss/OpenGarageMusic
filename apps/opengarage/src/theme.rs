//! Colours of the interface. A theme is a small JSON file: a name, a base
//! ("dark" or "light") and the colours that differ from that base. Three are
//! built in; the user can add theirs.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};

/// The theme of the program as it always looked.
pub const DEFAULT: &str = "studio";

const BUILT_IN: [(&str, &str); 3] =
    [("studio", include_str!("../themes/studio.json")), ("dark", include_str!("../themes/dark.json")), ("light", include_str!("../themes/light.json"))];

/// Every colour a theme file can give, under `colors`.
pub const KEYS: [&str; 18] = [
    "window",
    "panel",
    "field",
    "stripe",
    "button",
    "button_hover",
    "button_active",
    "border",
    "text",
    "button_text",
    "text_strong",
    "selection",
    "selection_text",
    "link",
    "outline",
    "record",
    "warning",
    "done",
];

/// A theme as it is written in its file.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Written {
    name: String,
    base: String,
    colors: BTreeMap<String, String>,
    tracks: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// Name of the file, without ".json".
    pub id: String,
    pub name: String,
    pub dark: bool,
    colors: BTreeMap<String, Color32>,
    /// Colours of the tracks and of their regions; also the accents.
    pub tracks: [Color32; 8],
}

fn parse_color(text: &str) -> Option<Color32> {
    let hex = text.trim().strip_prefix('#')?;
    if !hex.is_ascii() {
        return None;
    }
    let digit = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok();
    let pair = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    match hex.len() {
        3 => Some(Color32::from_rgb(digit(0)? * 17, digit(1)? * 17, digit(2)? * 17)),
        6 => Some(Color32::from_rgb(pair(0)?, pair(2)?, pair(4)?)),
        8 => Some(Color32::from_rgba_unmultiplied(pair(0)?, pair(2)?, pair(4)?, pair(6)?)),
        _ => None,
    }
}

fn write_color(color: Color32) -> String {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

const TRACKS: [Color32; 8] = [
    Color32::from_rgb(96, 190, 120),
    Color32::from_rgb(90, 160, 230),
    Color32::from_rgb(235, 170, 70),
    Color32::from_rgb(200, 120, 220),
    Color32::from_rgb(235, 200, 80),
    Color32::from_rgb(230, 110, 110),
    Color32::from_rgb(90, 200, 200),
    Color32::from_rgb(170, 170, 180),
];

impl Theme {
    /// Reads a theme file. What it does not give is taken from its base.
    pub fn parse(id: &str, text: &str) -> Result<Self, String> {
        let written: Written = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| e.to_string())?;
        let dark = match written.base.trim().to_lowercase().as_str() {
            "" | "dark" => true,
            "light" => false,
            other => return Err(format!("base: \"{other}\" (dark, light)")),
        };
        let mut colors = BTreeMap::new();
        for (key, value) in &written.colors {
            // A colour this version does not know is left aside, so that a
            // theme written for a later version still loads.
            if KEYS.contains(&key.as_str()) {
                colors.insert(key.clone(), parse_color(value).ok_or_else(|| format!("{key}: \"{value}\" (#rrggbb)"))?);
            }
        }
        let mut tracks = TRACKS;
        for (slot, value) in tracks.iter_mut().zip(&written.tracks) {
            *slot = parse_color(value).ok_or_else(|| format!("tracks: \"{value}\" (#rrggbb)"))?;
        }
        let name = if written.name.trim().is_empty() { id.to_owned() } else { written.name.trim().to_owned() };
        Ok(Self { id: id.to_owned(), name, dark, colors, tracks })
    }

    /// The theme as a file that gives every colour: a model to start from.
    pub fn to_json(&self) -> String {
        let visuals = self.visuals();
        let colors = KEYS.iter().map(|key| ((*key).to_owned(), write_color(self.resolved(key, &visuals)))).collect();
        let written = Written {
            name: self.name.clone(),
            base: if self.dark { "dark" } else { "light" }.to_owned(),
            colors,
            tracks: self.tracks.iter().map(|c| write_color(*c)).collect(),
        };
        serde_json::to_string_pretty(&written).unwrap_or_default()
    }

    fn get(&self, key: &str) -> Option<Color32> {
        self.colors.get(key).copied()
    }

    /// A colour of the theme, or the one its base has in that place.
    fn resolved(&self, key: &str, visuals: &egui::Visuals) -> Color32 {
        match key {
            "window" => visuals.window_fill,
            "panel" => visuals.panel_fill,
            "field" => visuals.extreme_bg_color,
            // The bases give this one as a little light added to what is
            // behind; a file can only hold the colour that results.
            "stripe" if visuals.faint_bg_color.a() == 0 => {
                let (panel, more) = (visuals.panel_fill, visuals.faint_bg_color);
                Color32::from_rgb(panel.r().saturating_add(more.r()), panel.g().saturating_add(more.g()), panel.b().saturating_add(more.b()))
            }
            "stripe" => visuals.faint_bg_color,
            "button" => visuals.widgets.inactive.weak_bg_fill,
            "button_hover" => visuals.widgets.hovered.weak_bg_fill,
            "button_active" => visuals.widgets.active.weak_bg_fill,
            "border" => visuals.widgets.noninteractive.bg_stroke.color,
            "text" => visuals.widgets.noninteractive.fg_stroke.color,
            "button_text" => visuals.widgets.inactive.fg_stroke.color,
            "text_strong" => visuals.widgets.active.fg_stroke.color,
            "selection" => visuals.selection.bg_fill,
            "selection_text" => visuals.selection.stroke.color,
            "link" => visuals.hyperlink_color,
            "outline" => self.outline(),
            "record" => self.record(),
            "warning" => self.warning(),
            _ => self.done(),
        }
    }

    /// What egui draws its widgets with.
    pub fn visuals(&self) -> egui::Visuals {
        let mut v = if self.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        if let Some(c) = self.get("window") {
            v.window_fill = c;
        }
        if let Some(c) = self.get("panel") {
            v.panel_fill = c;
        }
        if let Some(c) = self.get("field") {
            v.extreme_bg_color = c;
        }
        if let Some(c) = self.get("stripe") {
            v.faint_bg_color = c;
        }
        if let Some(c) = self.get("button") {
            v.widgets.inactive.bg_fill = c;
            v.widgets.inactive.weak_bg_fill = c;
        }
        if let Some(c) = self.get("button_hover") {
            v.widgets.hovered.bg_fill = c;
            v.widgets.hovered.weak_bg_fill = c;
        }
        if let Some(c) = self.get("button_active") {
            v.widgets.active.bg_fill = c;
            v.widgets.active.weak_bg_fill = c;
        }
        if let Some(c) = self.get("border") {
            v.widgets.noninteractive.bg_stroke.color = c;
            v.window_stroke.color = c;
        }
        if let Some(c) = self.get("text") {
            v.widgets.noninteractive.fg_stroke.color = c;
        }
        if let Some(c) = self.get("button_text") {
            v.widgets.inactive.fg_stroke.color = c;
        }
        if let Some(c) = self.get("text_strong") {
            v.widgets.active.fg_stroke.color = c;
        }
        if let Some(c) = self.get("selection") {
            v.selection.bg_fill = c;
        }
        if let Some(c) = self.get("selection_text") {
            v.selection.stroke.color = c;
        }
        if let Some(c) = self.get("link") {
            v.hyperlink_color = c;
        }
        v
    }

    /// Line around what is selected, and the curves drawn over the tracks.
    pub fn outline(&self) -> Color32 {
        self.get("outline").unwrap_or(if self.dark { Color32::WHITE } else { Color32::from_gray(30) })
    }

    pub fn record(&self) -> Color32 {
        self.get("record").unwrap_or(Color32::from_rgb(230, 80, 80))
    }

    pub fn warning(&self) -> Color32 {
        self.get("warning").unwrap_or(if self.dark { Color32::from_rgb(235, 170, 70) } else { Color32::from_rgb(176, 110, 0) })
    }

    pub fn done(&self) -> Color32 {
        self.get("done").unwrap_or(if self.dark { Color32::from_rgb(110, 200, 120) } else { Color32::from_rgb(40, 140, 60) })
    }
}

/// The themes that come with the program.
pub fn built_in() -> Vec<Theme> {
    BUILT_IN.iter().filter_map(|(id, text)| Theme::parse(id, text).ok()).collect()
}

/// The built-in themes, then those of a folder (the user's), by name. A file
/// named as a built-in theme replaces it.
pub fn all(folder: &Path) -> Vec<Theme> {
    let mut themes = built_in();
    let mut files: Vec<_> = std::fs::read_dir(folder).map(|entries| entries.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    files.sort();
    for file in files {
        if !file.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
            continue;
        }
        let Some(id) = file.file_stem().map(|s| s.to_string_lossy().into_owned()) else { continue };
        let Some(theme) = std::fs::read_to_string(&file).ok().and_then(|text| Theme::parse(&id, &text).ok()) else { continue };
        match themes.iter_mut().find(|t| t.id == id) {
            Some(known) => *known = theme,
            None => themes.push(theme),
        }
    }
    themes
}

static CURRENT: RwLock<Option<Arc<Theme>>> = RwLock::new(None);

/// The theme in use.
/// The colour of a warning, in the theme in use.
pub fn warning() -> Color32 {
    current().warning()
}

/// The colour of what is done, in the theme in use.
pub fn done() -> Color32 {
    current().done()
}

/// The colour of recording and of what is wrong, in the theme in use.
pub fn record() -> Color32 {
    current().record()
}

pub fn current() -> Arc<Theme> {
    if let Some(theme) = CURRENT.read().ok().and_then(|current| current.clone()) {
        return theme;
    }
    let theme = Arc::new(built_in().into_iter().next().unwrap_or(Theme { id: DEFAULT.to_owned(), name: "Studio".to_owned(), dark: true, colors: BTreeMap::new(), tracks: TRACKS }));
    set(Arc::clone(&theme));
    theme
}

pub fn set(theme: Arc<Theme>) {
    if let Ok(mut current) = CURRENT.write() {
        *current = Some(theme);
    }
}

/// One of the eight colours of the tracks.
pub fn palette(index: usize) -> Color32 {
    current().tracks[index % 8]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_themes_load_and_differ() {
        let themes = built_in();
        assert_eq!(themes.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), ["studio", "dark", "light"]);
        let (studio, dark, light) = (&themes[0], &themes[1], &themes[2]);
        assert!(studio.dark && dark.dark && !light.dark);
        // The first theme is the look the program always had.
        assert_eq!(studio.visuals().panel_fill, egui::Visuals::dark().panel_fill);
        assert_eq!(studio.tracks, TRACKS);
        assert_ne!(dark.visuals().panel_fill, studio.visuals().panel_fill);
        let brightness = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        assert!(brightness(dark.visuals().panel_fill) < brightness(studio.visuals().panel_fill));
        assert!(brightness(light.visuals().panel_fill) > 600, "a light background");
        assert!(brightness(light.visuals().text_color()) < 300, "with dark text on it");
        assert!(brightness(light.outline()) < 200);
    }

    #[test]
    fn a_theme_file_gives_only_what_it_changes_and_mistakes_are_told() {
        let theme = Theme::parse("mine", r##"{ "name": "Mine", "base": "light", "colors": { "panel": "#102030", "record": "#f00", "later": "#123456" }, "tracks": ["#010203"] }"##).unwrap();
        assert_eq!((theme.id.as_str(), theme.name.as_str(), theme.dark), ("mine", "Mine", false));
        assert_eq!(theme.visuals().panel_fill, Color32::from_rgb(0x10, 0x20, 0x30));
        assert_eq!(theme.record(), Color32::from_rgb(255, 0, 0));
        assert_eq!(theme.visuals().window_fill, egui::Visuals::light().window_fill, "not given: as the base");
        assert_eq!(theme.tracks[0], Color32::from_rgb(1, 2, 3));
        assert_eq!(theme.tracks[1], TRACKS[1]);
        // No name: the file's.
        assert_eq!(Theme::parse("plain", "{}").unwrap().name, "plain");
        assert!(Theme::parse("bad", r#"{ "colors": { "panel": "blue" } }"#).unwrap_err().contains("panel"));
        assert!(Theme::parse("bad", r#"{ "base": "sepia" }"#).unwrap_err().contains("sepia"));
        assert!(Theme::parse("bad", "not json").is_err());
        // The model written from a theme gives every colour and reads back the same.
        let model = theme.to_json();
        for key in KEYS {
            assert!(model.contains(&format!("\"{key}\"")), "{key} is in the model");
        }
        let again = Theme::parse("mine", &model).unwrap();
        assert_eq!(again.to_json(), model);
        let (seen, wanted) = (again.visuals(), theme.visuals());
        assert_eq!((seen.panel_fill, seen.window_fill, seen.text_color(), seen.selection, seen.widgets.inactive), (wanted.panel_fill, wanted.window_fill, wanted.text_color(), wanted.selection, wanted.widgets.inactive));
        assert_eq!((again.tracks, again.record(), again.outline()), (theme.tracks, theme.record(), theme.outline()));
    }
}
