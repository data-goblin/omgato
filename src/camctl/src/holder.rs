use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Holder {
    pub pid: u32,
    pub command: String,
    pub unit: Option<String>,
}

/// The systemd **user** unit a process belongs to, if any.
///
/// Only a user unit counts. A user unit sits under `user@<uid>.service` and can
/// be stopped and started again without privilege, which is what makes it safe
/// to borrow the capture device from. A system unit looks the same at the end of
/// the cgroup path but is not ours to touch.
fn unit_of(pid: u32) -> Option<String> {
    let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    let marker = format!("/user@{}.service/", users_own_uid());
    cgroup.lines().find_map(|line| {
        if !line.contains(&marker) {
            return None;
        }
        line.rsplit('/').next().filter(|p| p.ends_with(".service")).map(str::to_string)
    })
}

fn users_own_uid() -> u32 {
    // Safe: getuid never fails and touches no memory we own.
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}

/// Stop the user unit holding the device and wait for it to let go, so the
/// overlay can take it. Returns the unit that was stopped, for giving back.
pub fn borrow(found: &[Holder], device: &Path) -> Option<String> {
    let unit = found.iter().find_map(|h| h.unit.clone())?;
    let stopped = std::process::Command::new("systemctl")
        .args(["--user", "stop", &unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !stopped {
        return None;
    }
    for _ in 0..40 {
        if holders(device).is_empty() {
            return Some(unit);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Some(unit)
}

/// Start a unit that was stopped to free the device.
pub fn give_back(unit: &str) {
    let _ = std::process::Command::new("systemctl")
        .args(["--user", "start", unit])
        .status();
}

fn command_of(pid: u32) -> String {
    let raw = fs::read(format!("/proc/{pid}/cmdline")).unwrap_or_default();
    let text: String = raw
        .split(|b| *b == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).to_string())
        .collect::<Vec<_>>()
        .join(" ");
    if text.is_empty() {
        fs::read_to_string(format!("/proc/{pid}/comm"))
            .unwrap_or_default()
            .trim()
            .to_string()
    } else {
        text
    }
}

/// Find every process holding the capture device open.
///
/// The Cam Link is single-open, so anything already on it stops the overlay from
/// starting. Scanning /proc for descriptors pointing at the resolved device says
/// which process to blame, rather than leaving a bare "resource busy".
pub fn holders(device: &Path) -> Vec<Holder> {
    let Ok(target) = fs::canonicalize(device) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return found;
    };
    let me = std::process::id();
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if pid == me {
            continue;
        }
        let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if fs::read_link(fd.path()).is_ok_and(|link| link == target) {
                found.push(Holder { pid, command: command_of(pid), unit: unit_of(pid) });
                break;
            }
        }
    }
    found
}

/// How to actually free the device, given who is holding it.
pub fn remedy(found: &[Holder]) -> String {
    match found.iter().find_map(|h| h.unit.as_ref()) {
        Some(unit) => format!("stop it with: systemctl --user stop {unit}"),
        None => match found.first() {
            Some(h) => format!("stop it with: kill {}", h.pid),
            None => String::new(),
        },
    }
}

/// A one-line summary naming what to stop, short enough for a notification.
pub fn describe(found: &[Holder]) -> String {
    found
        .iter()
        .map(|h| {
            // Truncating by bytes panics when the cut lands inside a multibyte
            // character, which a path or argument can easily contain.
            let cmd: String = if h.command.chars().count() > 60 {
                h.command.chars().take(57).chain("...".chars()).collect()
            } else {
                h.command.clone()
            };
            match &h.unit {
                Some(unit) => format!("{unit} (pid {})", h.pid),
                None => format!("pid {} ({cmd})", h.pid),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
