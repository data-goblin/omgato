use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::{hypr, state};

pub fn find_device(pattern: &str) -> Option<PathBuf> {
    let dir = PathBuf::from("/dev/v4l/by-id");
    let entries = std::fs::read_dir(&dir).ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.contains(pattern) && name.ends_with("video-index0") {
            return Some(e.path());
        }
    }
    None
}

pub fn is_running(title: &str) -> bool {
    if let Some(process) = read_process()
        && process_alive(process)
    {
        if process.start.is_some() {
            return true;
        }
        let tracked_window = hypr::find_window(title)
            .ok()
            .flatten()
            .is_some_and(|client| client.pid == process.pid);
        if tracked_window
            && let Some(upgraded) = process_ref(process.pid)
        {
            write_process(upgraded);
            return true;
        }
    }
    state::remove(&state::pid_file());
    let process = hypr::find_window(title)
        .ok()
        .flatten()
        .filter(|client| client.pid > 0 && pid_alive(client.pid))
        .and_then(|client| process_ref(client.pid));
    if let Some(process) = process {
        write_process(process);
        true
    } else {
        false
    }
}

#[derive(Clone, Copy)]
struct ProcessRef {
    pid: u32,
    start: Option<u64>,
}

fn read_process() -> Option<ProcessRef> {
    let text = state::read(&state::pid_file())?;
    let mut fields = text.split_whitespace();
    Some(ProcessRef {
        pid: fields.next()?.parse().ok()?,
        start: fields.next().and_then(|value| value.parse().ok()),
    })
}

fn process_ref(pid: u32) -> Option<ProcessRef> {
    pid_alive(pid).then(|| ProcessRef { pid, start: process_start(pid) })
}

fn process_alive(process: ProcessRef) -> bool {
    pid_alive(process.pid)
        && process.start.is_none_or(|wanted| process_start(process.pid) == Some(wanted))
}

fn write_process(process: ProcessRef) {
    let value = match process.start {
        Some(start) => format!("{} {start}", process.pid),
        None => process.pid.to_string(),
    };
    let _ = state::write_atomic(&state::pid_file(), &value);
}

pub fn spawn(cfg: &Config) -> Result<u32, String> {
    let dev = find_device(&cfg.device_pattern)
        .ok_or_else(|| format!("device matching '{}' not found in /dev/v4l/by-id", cfg.device_pattern))?;
    let dev_str = dev.to_string_lossy().into_owned();

    // The Cam Link is single-open. Say which process to stop rather than
    // letting mpv fail later with a bare "resource busy".
    let busy = crate::holder::holders(&dev);
    if !busy.is_empty() {
        return Err(format!(
            "{} is already in use by {}; {}",
            dev.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(dev_str),
            crate::holder::describe(&busy),
            crate::holder::remedy(&busy)
        ));
    }

    let cap_w = cfg.capture_size[0];
    let cap_h = cfg.capture_size[1];
    let fps = cfg.framerate;
    let win_w = cfg.size[0];
    let win_h = cfg.size[1];

    // Create a separate inode exclusively, then rename it over the public log
    // name. The open cannot follow a symlink and rename replaces a symlink rather
    // than opening its target.
    let run_dir = state::run_dir();
    let log_path = run_dir.join("mpv.log");
    let log_tmp = run_dir.join(format!(".mpv.log.{}.tmp", std::process::id()));
    let log = std::fs::OpenOptions::new()
        .create_new(true).write(true)
        .open(&log_tmp)
        .map_err(|e| format!("open log: {e}"))?;
    if let Err(e) = std::fs::rename(&log_tmp, &log_path) {
        let _ = std::fs::remove_file(&log_tmp);
        return Err(format!("publish log: {e}"));
    }

    let geometry = format!("--geometry={}x{}", win_w, win_h);
    let autofit = format!("--autofit={}x{}", win_w, win_h);
    let title = format!("--title={}", cfg.window_title);
    let demux_opts = format!(
        "--demuxer-lavf-o=video_size={}x{},framerate={}",
        cap_w, cap_h, fps,
    );
    let input = format!("av://v4l2:{}", dev_str);

    let child = unsafe {
        Command::new("mpv")
            .args([
                "--no-config",
                "--no-input-default-bindings",
                "--no-osc",
                "--no-osd-bar",
                "--osd-level=0",
                "--profile=low-latency",
                "--untimed",
                "--force-window=immediate",
                "--keepaspect-window=no",
                "--border=no",
                "--ontop",
                &geometry,
                &autofit,
                &title,
                &demux_opts,
                &input,
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(log))
            .pre_exec(|| {
                nix_setsid();
                Ok(())
            })
            .spawn()
            .map_err(|e| format!("spawn mpv: {e}"))?
    };
    let pid = child.id();
    std::mem::forget(child);
    if let Some(process) = process_ref(pid) {
        write_process(process);
    } else {
        state::write_atomic(&state::pid_file(), &pid.to_string()).ok();
    }
    Ok(pid)
}

fn nix_setsid() {
    unsafe {
        libc::setsid();
    }
}

pub fn kill_running(title: &str) -> bool {
    if !is_running(title) {
        return false;
    }
    let process = match read_process() {
        Some(process) => process,
        None => return false,
    };
    if !process_alive(process) {
        state::remove(&state::pid_file());
        return false;
    }
    let pid = process.pid;
    unsafe { libc::kill(pid as i32, libc::SIGTERM); }
    let deadline = Instant::now() + Duration::from_millis(600);
    while Instant::now() < deadline {
        if !process_alive(process) {
            state::remove(&state::pid_file());
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    unsafe { libc::kill(pid as i32, libc::SIGKILL); }
    let deadline = Instant::now() + Duration::from_millis(600);
    while Instant::now() < deadline {
        if !process_alive(process) {
            state::remove(&state::pid_file());
            return true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    false
}

pub enum WaitResult {
    Mapped(String),
    Died,
    Timeout,
}

/// Wait up to `timeout` for the overlay window to appear in Hyprland and
/// return its address. mpv with `--force-window=immediate` maps a window
/// before the first frame, so this normally resolves in <500ms. If the
/// spawned process exits before mapping (e.g. Cam Link receiving no HDMI
/// signal -> libavformat fails to probe), return `Died` so the caller
/// can surface a clear error instead of silently waiting out the timeout.
pub fn wait_for_window(title: &str, pid: u32, timeout: Duration) -> WaitResult {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            state::remove(&state::pid_file());
            return WaitResult::Died;
        }
        if let Ok(Some(c)) = hypr::find_window(title)
            && c.mapped {
                std::thread::sleep(Duration::from_millis(30));
                if pid_alive(pid) {
                    return WaitResult::Mapped(c.address);
                }
                state::remove(&state::pid_file());
                return WaitResult::Died;
            }
        std::thread::sleep(Duration::from_millis(10));
    }
    WaitResult::Timeout
}

/// True if `pid` is running or sleeping. False for zombies (`Z`), dead (`X`)
/// or missing /proc entries. mpv becomes a zombie when it exits early because
/// we `mem::forget` the Child without reaping it, so a plain /proc/PID exists
/// check would lie. /proc/PID/stat is a single line; field 2 (the comm) may
/// contain spaces wrapped in parens, so split on the closing paren first.
fn pid_alive(pid: u32) -> bool {
    let stat = match std::fs::read_to_string(format!("/proc/{pid}/stat")) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let after_comm = match stat.rsplit_once(") ") {
        Some((_, rest)) => rest,
        None => return false,
    };
    if matches!(after_comm.chars().next(), Some('Z') | Some('X') | None) {
        return false;
    }
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|comm| comm.trim() == "mpv")
        .unwrap_or(false)
}

fn process_start(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, after_comm) = stat.rsplit_once(") ")?;
    after_comm.split_whitespace().nth(19)?.parse().ok()
}

#[allow(non_camel_case_types, non_snake_case)]
mod libc {
    pub type pid_t = i32;
    unsafe extern "C" {
        pub fn setsid() -> pid_t;
        pub fn kill(pid: pid_t, sig: i32) -> i32;
    }
    pub const SIGTERM: i32 = 15;
    pub const SIGKILL: i32 = 9;
}
