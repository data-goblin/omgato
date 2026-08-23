use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::config::Config;
use crate::state;

#[derive(Debug, Clone, PartialEq)]
pub enum CamState {
    On(String),       // reason, e.g. "ffplay" or "PipeWire client"
    Off(String),      // pw state, e.g. "suspended"
    Disconnected,
    Disabled,
}

pub fn detect(cfg: &Config) -> CamState {
    if state::exists(&state::pause_flag()) {
        return CamState::Disabled;
    }

    match pw_state(&cfg.device_pattern) {
        Some(s) if s == "running" => {
            CamState::On(direct_holder(cfg).unwrap_or_else(|| "PipeWire client".into()))
        }
        Some(s) => CamState::Off(s),
        None => match direct_holder(cfg) {
            Some(holder) => CamState::On(holder),
            None => CamState::Disconnected,
        },
    }
}

fn direct_holder(cfg: &Config) -> Option<String> {
    let by_id = PathBuf::from("/dev/v4l/by-id");
    let mut dev_targets: Vec<PathBuf> = Vec::new();
    if let Ok(entries) = fs::read_dir(&by_id) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.contains(&cfg.device_pattern)
                && let Ok(t) = fs::canonicalize(e.path()) {
                    dev_targets.push(t);
                }
        }
    }
    if dev_targets.is_empty() {
        return None;
    }

    for proc_e in fs::read_dir("/proc").ok()?.flatten() {
        let pid_str = proc_e.file_name().to_string_lossy().to_string();
        if !pid_str.chars().all(|c| c.is_ascii_digit()) { continue; }
        let fd_dir = proc_e.path().join("fd");
        let fds = match fs::read_dir(&fd_dir) {
            Ok(f) => f,
            Err(_) => continue,
        };
        for fd_e in fds.flatten() {
            let target = match fs::read_link(fd_e.path()) {
                Ok(t) => t,
                Err(_) => continue,
            };
            if dev_targets.iter().any(|d| d == &target) {
                let comm = fs::read_to_string(proc_e.path().join("comm"))
                    .ok().map(|s| s.trim().to_string()).unwrap_or_else(|| pid_str.clone());
                return Some(comm);
            }
        }
    }
    None
}

fn pw_state(pattern: &str) -> Option<String> {
    let out = Command::new("pw-dump")
        .output().ok()?;
    if !out.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let arr = json.as_array()?;
    let needle_spaced = pattern.replace('_', " ");
    for item in arr {
        let info = match item.get("info") { Some(v) => v, None => continue };
        let props = match info.get("props") { Some(v) => v, None => continue };
        let media_class = props.get("media.class").and_then(|v| v.as_str()).unwrap_or("");
        if media_class != "Video/Source" { continue; }
        let desc = props.get("node.description").and_then(|v| v.as_str()).unwrap_or("");
        let name = props.get("node.name").and_then(|v| v.as_str()).unwrap_or("");
        if desc.contains(needle_spaced.as_str()) || name.contains(pattern) {
            let st = info.get("state").and_then(|v| v.as_str()).unwrap_or("unknown");
            return Some(st.to_string());
        }
    }
    None
}
