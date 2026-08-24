use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Holder {
    pub pid: u32,
    pub command: String,
    pub unit: Option<String>,
}

/// The systemd user unit a process belongs to, if any. A process supervised by
/// systemd comes straight back when killed, so the useful advice is the unit
/// name rather than the pid.
fn unit_of(pid: u32) -> Option<String> {
    let cgroup = fs::read_to_string(format!("/proc/{pid}/cgroup")).ok()?;
    cgroup
        .lines()
        .filter_map(|line| line.rsplit('/').next())
        .find(|part| part.ends_with(".service"))
        .map(|s| s.to_string())
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
            let mut cmd = h.command.clone();
            if cmd.len() > 60 {
                cmd.truncate(57);
                cmd.push_str("...");
            }
            match &h.unit {
                Some(unit) => format!("{unit} (pid {})", h.pid),
                None => format!("pid {} ({cmd})", h.pid),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}
