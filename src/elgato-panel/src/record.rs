use crate::sh;
use crate::state;
use serde::{Deserialize, Serialize};
use std::fs;

const RECORDER: &str = "omarchy-capture-screenrecording";
const OPTIONS_FILE: &str = "record.json";
const USER_HZ: u64 = 100; // fixed for /proc regardless of kernel HZ

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Options {
    #[serde(default)]
    pub desktop_audio: bool,
    #[serde(default)]
    pub mic: bool,
}

#[derive(Serialize)]
pub struct Status {
    pub active: bool,
    pub seconds: u64,
    pub options: Options,
    pub directory: String,
}

pub fn load_options() -> Options {
    state::read_state(OPTIONS_FILE).unwrap_or_default()
}

fn output_dir() -> String {
    std::env::var("OMARCHY_SCREENRECORD_DIR")
        .ok()
        .or_else(|| std::env::var("XDG_VIDEOS_DIR").ok())
        .unwrap_or_else(|| {
            dirs::video_dir()
                .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join("Videos"))
                .to_string_lossy()
                .into_owned()
        })
}

/// The omarchy script decides "am I recording" with this exact pattern, so the
/// panel asks the same question rather than guessing at a process name.
fn recorder_pid() -> Option<u32> {
    sh::run(&["pgrep", "-f", "^gpu-screen-recorder"])
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
}

fn elapsed_seconds(pid: u32) -> u64 {
    let Ok(uptime) = fs::read_to_string("/proc/uptime") else {
        return 0;
    };
    let Some(uptime) = uptime.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()) else {
        return 0;
    };
    let Ok(stat) = fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return 0;
    };
    // Field 22 is starttime; the executable name can hold spaces, so counting
    // starts after the closing parenthesis.
    let Some((_, rest)) = stat.rsplit_once(')') else {
        return 0;
    };
    let started = rest
        .split_whitespace()
        .nth(19)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(0);
    (uptime as u64).saturating_sub(started / USER_HZ)
}

pub fn status() -> Status {
    let pid = recorder_pid();
    Status {
        active: pid.is_some(),
        seconds: pid.map(elapsed_seconds).unwrap_or(0),
        options: load_options(),
        directory: output_dir(),
    }
}

/// Starts a recording of a picked region or the focused screen, remembering the
/// audio choices so the panel comes back with the same switches set.
pub fn start(target: &str, options: Options) {
    state::write_state(OPTIONS_FILE, &options);
    let mut cmd = vec![RECORDER.to_owned()];
    if target == "screen" {
        cmd.push("--fullscreen".to_owned());
    }
    if options.desktop_audio {
        cmd.push("--with-desktop-audio".to_owned());
    }
    if options.mic {
        cmd.push("--with-microphone-audio".to_owned());
    }
    sh::spawn_detached(&cmd);
}

pub fn stop() {
    sh::spawn_detached(&[RECORDER.to_owned(), "--stop-recording".to_owned()]);
}
