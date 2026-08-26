use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::os::fd::AsRawFd;
use std::path::PathBuf;

pub const HISTORY_MAX: usize = 11; // baseline + 10 undoable changes

pub const LIGHTS_HISTORY: &str = "history.json";
pub const DECK_HISTORY: &str = "deck-history.json";
pub const CAMERA_HISTORY: &str = "camera-history.json";
pub const SCOPE_HISTORY: &str = "scope-history.json";
pub const LIGHTS_DEFAULT: &str = "lights-default.json";

pub struct CommandLock(fs::File);

pub fn command_lock() -> std::io::Result<CommandLock> {
    fs::create_dir_all(dir())?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(dir().join("command.lock"))?;
    let result = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
    if result == 0 {
        Ok(CommandLock(file))
    } else {
        Err(std::io::Error::last_os_error())
    }
}

impl Drop for CommandLock {
    fn drop(&mut self) {
        let _ = unsafe { flock(self.0.as_raw_fd(), LOCK_UN) };
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snap {
    pub name: String,
    #[serde(default)]
    pub ip: String,
    pub on: bool,
    pub brightness: u8,
    pub kelvin: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct History<T> {
    #[serde(default = "Vec::new")]
    pub stack: Vec<T>,
    #[serde(default = "empty_pos")]
    pub pos: i64,
}

impl<T> Default for History<T> {
    fn default() -> Self {
        Self { stack: Vec::new(), pos: -1 }
    }
}

fn empty_pos() -> i64 {
    -1
}

#[derive(Serialize)]
pub struct Flags {
    pub can_undo: bool,
    pub can_redo: bool,
}

impl<T: PartialEq + Serialize + DeserializeOwned> History<T> {
    pub fn load(file: &str) -> Self {
        let mut h: History<T> = read_json(&path(file)).unwrap_or_default();
        if h.stack.is_empty() {
            h.pos = -1;
        } else {
            h.pos = h.pos.clamp(0, h.stack.len() as i64 - 1);
        }
        h
    }

    pub fn save(&self, file: &str) {
        write_json(&path(file), self);
    }

    pub fn flags(&self) -> Flags {
        Flags {
            can_undo: self.pos > 0,
            can_redo: self.pos >= 0 && self.pos < self.stack.len() as i64 - 1,
        }
    }

    pub fn current(&self) -> Option<&T> {
        usize::try_from(self.pos).ok().and_then(|p| self.stack.get(p))
    }

    /// Folds a freshly read state into the stack, dropping the redo tail on a
    /// new change and keeping at most HISTORY_MAX entries.
    pub fn fold(&mut self, file: &str, value: T) {
        if self.current() == Some(&value) {
            return;
        }
        let keep = (self.pos + 1).max(0) as usize;
        self.stack.truncate(keep);
        self.stack.push(value);
        if self.stack.len() > HISTORY_MAX {
            let excess = self.stack.len() - HISTORY_MAX;
            self.stack.drain(..excess);
        }
        self.pos = self.stack.len() as i64 - 1;
        self.save(file);
    }

    pub fn seek(&self, step: i64) -> Option<(i64, &T)> {
        let target = self.pos + step;
        usize::try_from(target)
            .ok()
            .and_then(|t| self.stack.get(t))
            .map(|value| (target, value))
    }

    pub fn commit_pos(&self, file: &str, pos: i64) {
        write_json(&path(file), &serde_json::json!({ "stack": &self.stack, "pos": pos }));
    }
}

pub type Aliases = BTreeMap<String, String>;

pub fn load_aliases() -> Aliases {
    read_json(&path("aliases.json")).unwrap_or_default()
}

pub fn save_aliases(aliases: &Aliases) {
    write_json(&path("aliases.json"), aliases);
}

/// Display order as a list of light addresses; anything absent sorts last.
pub fn load_order() -> Vec<String> {
    read_json(&path("order.json")).unwrap_or_default()
}

pub fn save_order(order: &[String]) {
    write_json(&path("order.json"), &order.to_vec());
}

pub fn dir() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .map(|d| d.join("omgato-panel"))
        .unwrap_or_else(|| crate::privdir::temp_fallback("omgato-panel"))
}

fn path(file: &str) -> PathBuf {
    let target = dir().join(file);
    let legacy = legacy_dir().join(file);
    migrate_file(&legacy, &target);
    target
}

fn legacy_dir() -> PathBuf {
    dirs::state_dir()
        .or_else(dirs::data_local_dir)
        .map(|d| d.join("elgato-panel"))
        .unwrap_or_else(|| crate::privdir::temp_fallback("elgato-panel"))
}

fn migrate_file(from: &std::path::Path, to: &std::path::Path) {
    if to.exists() || !from.is_file() {
        return;
    }
    let Some(parent) = to.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if fs::rename(from, to).is_err() && !to.exists() {
        let _ = fs::copy(from, to);
    }
}

/// Reads and writes an arbitrary small document in the panel's state directory.
pub fn read_state<T: DeserializeOwned>(file: &str) -> Option<T> {
    read_json(&path(file))
}

pub fn write_state<T: Serialize>(file: &str, value: &T) {
    write_json(&path(file), value);
}

/// Same as `write_state`, but says whether it worked. Anything the user is told
/// succeeded needs this rather than the silent form.
pub fn write_state_checked<T: Serialize>(file: &str, value: &T) -> std::io::Result<()> {
    let target = path(file);
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("state path has no parent"))?;
    fs::create_dir_all(parent)?;
    let tmp = target.with_extension(format!("{}.tmp", std::process::id()));
    let text = serde_json::to_string(value).map_err(std::io::Error::other)?;
    fs::write(&tmp, text)?;
    fs::rename(&tmp, &target)
}

fn read_json<T: DeserializeOwned>(path: &PathBuf) -> Option<T> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn write_json<T: Serialize>(path: &PathBuf, value: &T) {
    let Some(parent) = path.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let Ok(text) = serde_json::to_string(value) else {
        return;
    };
    if fs::write(&tmp, text).is_ok() {
        let _ = fs::rename(&tmp, path);
    }
}

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

const LOCK_EX: i32 = 2;
const LOCK_UN: i32 = 8;
