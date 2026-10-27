//! Small pieces of interface drawn the same way in several places: lists
//! of choices in the menus, and a box over a setting kept the other way round.

use eframe::egui;

use crate::i18n::tr;

/// Radio items in a menu, the current one marked: the item clicked, if any.
pub fn radio_list<T: PartialEq>(ui: &mut egui::Ui, current: &T, items: impl IntoIterator<Item = (T, String)>) -> Option<T> {
    let mut chosen = None;
    for (value, label) in items {
        if ui.radio(*current == value, label).clicked() {
            chosen = Some(value);
        }
    }
    chosen
}

/// The rows of a combo box, the current one marked: the row clicked, if any.
pub fn row_list<T: PartialEq>(ui: &mut egui::Ui, current: &T, items: impl IntoIterator<Item = (T, String)>) -> Option<T> {
    let mut chosen = None;
    for (value, label) in items {
        if ui.selectable_label(*current == value, label).clicked() {
            chosen = Some(value);
        }
    }
    chosen
}

/// Devices by name as items: first the one an empty name stands for, called
/// `none`, then each of `devices`.
pub fn device_items(devices: &[String], none: &str) -> Vec<(String, String)> {
    std::iter::once((String::new(), none.to_owned())).chain(devices.iter().map(|name| (name.clone(), name.clone()))).collect()
}

/// The name of a device, or of the one an empty name stands for.
pub fn device_name(name: &str) -> String {
    if name.is_empty() { tr("System default").to_owned() } else { name.to_owned() }
}

/// The channels of an input as items: all of them mixed (0), called `all`,
/// then each by number.
pub fn channel_items(count: u16, all: &str) -> Vec<(u16, String)> {
    std::iter::once((0, all.to_owned())).chain((1..=count.clamp(1, 32)).map(|number| (number, tf!("Input {}", number)))).collect()
}

/// A checkbox over a setting kept the other way round: the box says "shown"
/// where the setting says "hidden". Changed by the user, the setting follows.
pub fn checkbox_not(ui: &mut egui::Ui, setting: &mut bool, label: impl Into<egui::WidgetText>) -> egui::Response {
    let mut checked = !*setting;
    let response = ui.checkbox(&mut checked, label);
    if response.changed() {
        *setting = !checked;
    }
    response
}
