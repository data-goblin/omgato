use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::OnceLock;

pub const HISTORY_MAX: usize = 11;

pub const LIGHTS_HISTORY: &str = "history.json";
pub const DECK_HISTORY: &str = "deck-history.json";
pub const CAMERA_HISTORY: &str = "camera-history.json";
pub const SCOPE_HISTORY: &str = "scope-history.json";
pub const LIGHTS_DEFAULT: &str = "lights-default.json";

struct StateDirs {
    current: PathBuf,
    legacy: Option<PathBuf>,
}

static STATE_DIRS: OnceLock<StateDirs> = OnceLock::new();

pub fn init_dirs() -> Result<(), String> {
    if STATE_DIRS.get().is_some() {
        return Ok(());
    }
    let resolved = match dirs::state_dir().or_else(dirs::data_local_dir) {
        Some(base) => StateDirs {
            current: base.join("omgato-panel"),
            legacy: Some(base.join("elgato-panel")),
        },
        None => StateDirs {
            current: crate::privdir::temp_fallback("omgato-panel")?,
            legacy: None,
        },
    };
    STATE_DIRS
        .set(resolved)
        .map_err(|_| "panel state directories were initialized concurrently".to_string())
}

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

pub fn load_order() -> Vec<String> {
    read_json(&path("order.json")).unwrap_or_default()
}

pub fn save_order(order: &[String]) {
    write_json(&path("order.json"), &order.to_vec());
}

pub fn dir() -> PathBuf {
    STATE_DIRS
        .get()
        .expect("state::init_dirs must run before dispatch")
        .current
        .clone()
}

fn path(file: &str) -> PathBuf {
    let target = dir().join(file);
    if let Some(legacy) = legacy_dir() {
        migrate_file(&legacy.join(file), &target);
    }
    target
}

fn legacy_dir() -> Option<&'static std::path::Path> {
    STATE_DIRS
        .get()
        .expect("state::init_dirs must run before dispatch")
        .legacy
        .as_deref()
}

fn migrate_file(from: &std::path::Path, to: &std::path::Path) {
    if fs::symlink_metadata(to).is_ok()
        || !fs::symlink_metadata(from).is_ok_and(|meta| meta.file_type().is_file())
    {
        return;
    }
    let Some(parent) = to.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let _ = fs::rename(from, to);
}

pub fn read_state<T: DeserializeOwned>(file: &str) -> Option<T> {
    read_json(&path(file))
}

pub fn write_state<T: Serialize>(file: &str, value: &T) {
    write_json(&path(file), value);
}

pub fn write_state_checked<T: Serialize>(file: &str, value: &T) -> std::io::Result<()> {
    let target = path(file);
    let parent = target
        .parent()
        .ok_or_else(|| std::io::Error::other("state path has no parent"))?;
    fs::create_dir_all(parent)?;
    let tmp = target.with_extension(format!("{}.tmp", std::process::id()));
    let text = serde_json::to_string(value).map_err(std::io::Error::other)?;
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(&tmp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all().ok();
    drop(file);
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
    let Ok(mut file) = fs::OpenOptions::new().create_new(true).write(true).open(&tmp) else {
        return;
    };
    if file.write_all(text.as_bytes()).is_err() {
        return;
    }
    file.sync_all().ok();
    drop(file);
    let _ = fs::rename(&tmp, path);
}

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
}

const LOCK_EX: i32 = 2;
const LOCK_UN: i32 = 8;
