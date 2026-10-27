//! What the application asks, from the interface thread, of a running
//! plug-in, whatever its format (CLAP, LV2, VST3), and what the three hosts
//! share: the limits, the telling of the formats, the loading of the files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::clap_host::{ClapInstance, ClapLibrary, ParamInfo, PluginInfo};
use crate::lv2_host::{Lv2Handle, Lv2Instance};
use crate::vst3_host::{is_vst3, Vst3Instance, Vst3Library};

/// Largest block handed to a plug-in at once.
pub const MAX_FRAMES: usize = 4096;
/// Largest state a plug-in may save (64 MB).
pub const MAX_STATE: usize = 64 << 20;

/// Whether a plug-in path is an LV2 folder rather than a CLAP file.
pub fn is_lv2(path: &Path) -> bool {
    !is_vst3(path) && (path.is_dir() || path.extension().is_some_and(|e| e.eq_ignore_ascii_case("lv2")))
}

/// Short name of the format of a plug-in file, for lists.
pub fn plugin_format(path: &Path) -> &'static str {
    if is_vst3(path) {
        "VST3"
    } else if is_lv2(path) {
        "LV2"
    } else {
        "CLAP"
    }
}

/// The state of a plug-in as the project writes it.
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok()).collect()
}

/// What happened in a plug-in since it was last asked.
#[derive(Default)]
pub struct Idle {
    /// Its window was just closed, by its close box or by the plug-in.
    pub closed: bool,
    /// Parameters the user changed in the plug-in's own window: id and value.
    pub changes: Vec<(u32, f64)>,
    /// The plug-in is gone (its process stopped): why, told once.
    pub failed: Option<String>,
}

/// What a started plug-in is made of: its audio face, its main-thread face,
/// and its parameters.
pub type Loaded = (Box<dyn og_engine::ExternalUnit>, Option<Box<dyn PluginHandle>>, Vec<ParamInfo>);

/// The plug-in files loaded so far, each once: the plug-ins of a file share it.
#[derive(Default)]
pub struct Libraries {
    clap: HashMap<PathBuf, Arc<ClapLibrary>>,
    vst3: HashMap<PathBuf, Arc<Vst3Library>>,
}

/// The library of `file`, loaded now if it was not before.
fn cached<T>(cache: &mut HashMap<PathBuf, Arc<T>>, file: &Path, load: impl FnOnce(&Path) -> Result<Arc<T>, String>) -> Result<Arc<T>, String> {
    if let Some(library) = cache.get(file) {
        return Ok(Arc::clone(library));
    }
    let library = load(file)?;
    cache.insert(file.to_path_buf(), Arc::clone(&library));
    Ok(library)
}

impl Libraries {
    /// The plug-ins of a file. An LV2 folder is described by text files;
    /// a CLAP or VST3 file has to be loaded, which runs code from it.
    pub fn describe(&mut self, file: &Path) -> Result<Vec<PluginInfo>, String> {
        if is_vst3(file) {
            cached(&mut self.vst3, file, Vst3Library::load).map(|library| library.plugins.clone())
        } else if is_lv2(file) {
            crate::lv2_host::describe(file)
        } else {
            cached(&mut self.clap, file, ClapLibrary::load).map(|library| library.plugins.clone())
        }
    }

    /// Starts one of the plug-ins of a file in this process, whatever its
    /// format. This runs code from the file.
    pub fn load(&mut self, file: &Path, plugin_id: &str, sample_rate: f64) -> Result<Loaded, String> {
        if is_vst3(file) {
            let library = cached(&mut self.vst3, file, Vst3Library::load)?;
            let (instance, handle, params) = Vst3Instance::new(&library, plugin_id, sample_rate)?;
            return Ok((Box::new(instance), Some(Box::new(handle)), params));
        }
        if is_lv2(file) {
            let (instance, params) = Lv2Instance::new(file, plugin_id, sample_rate)?;
            let handle = Lv2Handle::new(file, plugin_id, instance.shared());
            return Ok((Box::new(instance), Some(Box::new(handle)), params));
        }
        let library = cached(&mut self.clap, file, ClapLibrary::load)?;
        let (instance, handle, params) = ClapInstance::new(&library, plugin_id, sample_rate)?;
        Ok((Box::new(instance), Some(Box::new(handle)), params))
    }
}

/// Loads a plug-in file and starts one of its plug-ins in this process.
pub fn load(file: &Path, plugin_id: &str, sample_rate: f64) -> Result<Loaded, String> {
    Libraries::default().load(file, plugin_id, sample_rate)
}

/// The main-thread face of a running plug-in: its window and its state.
pub trait PluginHandle {
    /// Whether the plug-in has a window of its own that can be shown here.
    fn has_window(&self) -> bool;
    fn window_open(&self) -> bool;
    /// Shows the plug-in's window, in a window of ours that it draws into.
    fn open_window(&mut self, title: &str) -> Result<(), String>;
    fn close_window(&mut self);
    /// Does what the plug-in asked of the main thread since the last call.
    fn idle(&mut self) -> Idle;
    /// Current value of a parameter, as the plug-in reports it.
    fn param_value(&self, _id: u32) -> Option<f64> {
        None
    }
    /// A parameter was changed outside the plug-in's window, which may show it.
    fn param_changed(&mut self, _id: u32, _value: f64) {}
    /// Everything the plug-in wants remembered, as it writes it.
    fn save_state(&self) -> Option<Vec<u8>> {
        None
    }
    /// Loads what `save_state` wrote.
    fn load_state(&self, _data: &[u8]) -> bool {
        false
    }
    /// The native window that holds the plug-in's, for tests.
    #[cfg(test)]
    fn window_handle(&self) -> Option<*mut std::ffi::c_void>;
}
