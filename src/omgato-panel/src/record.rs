//! Screen recording. The area being recorded is remembered as a "scope", so a
//! second recording can reuse the last one, and the scopes can be stepped
//! through like any other history in the panel.
use crate::sh;
use crate::state::{self, History, SCOPE_HISTORY};
use serde::{Deserialize, Serialize};
use std::fs;

const OPTIONS_FILE: &str = "record.json";
const USER_HZ: u64 = 100; // fixed for /proc regardless of kernel HZ
const PICKER: &str = "omarchy-capture-region";
const PID_FILE: &str = "recorder-pid.json";

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
    /// What the next recording will capture, in a form a person can read.
    pub scope: String,
    pub history: state::Flags,
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

/// The same question the omarchy script asks, so both agree on what "recording"
/// means.
fn known_pid() -> Option<u32> {
    let pid = state::read_state::<u32>(PID_FILE).filter(|pid| *pid > 0)?;
    fs::metadata(format!("/proc/{pid}")).ok().map(|_| pid)
}

fn recorder_pid(search: bool) -> Option<u32> {
    if let Some(pid) = known_pid() {
        return Some(pid);
    }
    if !search {
        return None;
    }
    sh::run(&["pgrep", "-f", "^gpu-screen-recorder"])
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
        .inspect(|pid| state::write_state(PID_FILE, pid))
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

fn scopes() -> History<String> {
    History::load(SCOPE_HISTORY)
}

/// "monitor:DP-1" and "region:1920x1080+0+0" as gpu-screen-recorder wants them.
fn current_scope() -> Option<String> {
    scopes().current().cloned()
}

fn readable(scope: &str) -> String {
    match scope.split_once(':') {
        Some(("monitor", name)) => format!("screen {name}"),
        Some(("region", geometry)) => geometry
            .split_once('+')
            .map(|(size, at)| format!("area {size} at {}", at.replace('+', ",")))
            .unwrap_or_else(|| format!("area {geometry}")),
        _ => scope.to_owned(),
    }
}

/// Runs the shared picker and folds what it returns into the scope history.
fn pick_scope() -> Option<String> {
    let picked = sh::run(&[PICKER, "smart", "--match-monitor"]);
    let picked = picked.trim();
    if picked.is_empty() {
        return None;
    }
    let scope = if let Some(name) = picked.strip_prefix("monitor:") {
        format!("monitor:{name}")
    } else {
        let (origin, size) = picked.split_once(char::is_whitespace)?;
        let (x, y) = origin.split_once(',')?;
        format!("region:{size}+{x}+{y}")
    };
    remember(scope.clone());
    Some(scope)
}

fn remember(scope: String) {
    let mut history = scopes();
    history.fold(SCOPE_HISTORY, scope);
}

fn focused_monitor() -> String {
    let name = sh::run(&["omarchy-hyprland-monitor-focused"]).trim().to_owned();
    format!("monitor:{name}")
}

pub fn status(search: bool) -> Status {
    let pid = recorder_pid(search);
    let history = scopes();
    Status {
        active: pid.is_some(),
        seconds: pid.map(elapsed_seconds).unwrap_or(0),
        options: load_options(),
        directory: output_dir(),
        scope: history.current().map(|s| readable(s)).unwrap_or_default(),
        history: history.flags(),
    }
}

/// Steps the remembered scopes without starting anything.
pub fn travel(step: i64) {
    let history = scopes();
    if let Some((pos, _)) = history.seek(step) {
        history.commit_pos(SCOPE_HISTORY, pos);
    }
}

/// `pick` chooses an area, `last` reuses the remembered one, `screen` takes the
/// focused monitor. Each choice is remembered so the next recording can repeat it.
pub fn start(target: &str, options: Options) {
    state::write_state(OPTIONS_FILE, &options);
    let scope = match target {
        "screen" => {
            let scope = focused_monitor();
            remember(scope.clone());
            Some(scope)
        }
        "last" => current_scope(),
        _ => pick_scope(),
    };
    let Some(scope) = scope.filter(|s| !s.ends_with(':')) else {
        return;
    };

    let filename = format!("{}/screenrecording-{}.mp4", output_dir(), timestamp());
    let mut cmd: Vec<String> = ["gpu-screen-recorder", "-w"].iter().map(|s| s.to_string()).collect();
    cmd.push(scope.split_once(':').map(|(_, v)| v.to_owned()).unwrap_or(scope));
    cmd.extend(
        ["-k", "auto", "-f", "60", "-fm", "cfr", "-fallback-cpu-encoding", "yes", "-o"]
            .iter()
            .map(|s| s.to_string()),
    );
    cmd.push(filename.clone());

    let mut sources = Vec::new();
    if options.desktop_audio {
        sources.push("default_output");
    }
    if options.mic {
        sources.push("default_input");
    }
    if !sources.is_empty() {
        cmd.push("-a".to_owned());
        cmd.push(sources.join("|"));
        cmd.push("-ac".to_owned());
        cmd.push("aac".to_owned());
    }
    state::write_state("recording.json", &filename);
    let pid = sh::spawn_detached(&cmd).unwrap_or(0);
    state::write_state(PID_FILE, &pid);
}

fn timestamp() -> String {
    sh::run(&["date", "+%Y-%m-%d_%H-%M-%S"]).trim().to_owned()
}

pub fn stop() {
    let Some(pid) = recorder_pid(true) else {
        return;
    };
    sh::run(&["pkill", "-SIGINT", "-f", "^gpu-screen-recorder"]);
    for _ in 0..50 {
        if !fs::metadata(format!("/proc/{pid}")).is_ok() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    state::write_state(PID_FILE, &0u32);
    if let Some(file) = state::read_state::<String>("recording.json") {
        finalize(&file);
    }
}

/// Trims the warm-up frame and levels the audio, matching what omarchy's own
/// capture does on the way out.
fn finalize(file: &str) {
    if !std::path::Path::new(file).exists() {
        return;
    }
    let has_audio = !sh::run(&[
        "ffprobe", "-v", "error", "-select_streams", "a",
        "-show_entries", "stream=codec_type", "-of", "csv=p=0", file,
    ])
    .trim()
    .is_empty();

    let processed = file.replace(".mp4", "-processed.mp4");
    let mut cmd: Vec<String> = ["ffmpeg", "-y", "-ss", "0.1", "-i", file, "-c:v", "copy"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    if has_audio {
        cmd.push("-af".to_owned());
        cmd.push(
            "volume=enable='lt(t,0.4)':volume=0,afade=t=in:st=0.4:d=0.05,loudnorm=I=-14:TP=-1.5:LRA=11"
                .to_owned(),
        );
    }
    cmd.extend(["-loglevel", "quiet"].iter().map(|s| s.to_string()));
    cmd.push(processed.clone());
    sh::run_owned(&cmd);
    if std::path::Path::new(&processed).exists() {
        let _ = fs::rename(&processed, file);
    }
}
