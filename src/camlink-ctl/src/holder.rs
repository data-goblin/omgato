use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Holder {
    pub pid: u32,
    start: u64,
    pub command: String,
    pub unit: Option<String>,
}

fn start_time(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, after_comm) = stat.rsplit_once(") ")?;
    after_comm.split_whitespace().nth(19)?.parse().ok()
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

/// The one user unit that owns every holder. Stopping only one of several owners
/// would disrupt it without making the device available.
pub fn borrowable_unit(found: &[Holder]) -> Option<String> {
    let unit = found.first()?.unit.clone()?;
    found.iter().all(|h| h.unit.as_deref() == Some(&unit)).then_some(unit)
}

fn same_processes(left: &[Holder], right: &[Holder]) -> bool {
    let mut left: Vec<(u32, u64)> = left.iter().map(|holder| (holder.pid, holder.start)).collect();
    let mut right: Vec<(u32, u64)> = right.iter().map(|holder| (holder.pid, holder.start)).collect();
    left.sort_unstable();
    right.sort_unstable();
    left == right
}

/// Stop a user unit and wait for the device to become free. Returns false when
/// the holder disappeared before the stop, in which case no service was touched.
pub fn borrow(unit: &str, device: &Path, expected: &[Holder]) -> Result<bool, String> {
    let current = holders(device);
    if current.is_empty() {
        return Ok(false);
    }
    if !same_processes(expected, &current)
        || borrowable_unit(&current).as_deref() != Some(unit)
    {
        return Err("the camera holders changed while they were being inspected; retry".into());
    }
    crate::state::write_atomic(&crate::state::borrowed_unit(), unit)
        .map_err(|e| format!("could not record the borrowed unit: {e}"))?;
    let stopped = std::process::Command::new("systemctl")
        .args(["--user", "stop", "--", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !stopped {
        crate::state::remove(&crate::state::borrowed_unit());
        return Err(format!("could not stop {unit}"));
    }
    for _ in 0..40 {
        if holders(device).is_empty() {
            return Ok(true);
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let still_busy = describe(&holders(device));
    Err(format!("the camera stayed held by {still_busy}"))
}

/// Start a unit that was stopped to free the device.
pub fn give_back(unit: &str) -> Result<(), String> {
    let started = std::process::Command::new("systemctl")
        .args(["--user", "start", "--", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    started.then_some(()).ok_or_else(|| format!("could not restart {unit}"))
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
        let Some(start) = start_time(pid) else {
            continue;
        };
        let Ok(fds) = fs::read_dir(entry.path().join("fd")) else {
            continue;
        };
        for fd in fds.flatten() {
            if fs::read_link(fd.path()).is_ok_and(|link| link == target) {
                let command = command_of(pid);
                let unit = unit_of(pid);
                // All three /proc reads must describe the same process. Without
                // this second check a reused pid could lend an unrelated unit to
                // the device holder found through the already-open fd directory.
                if start_time(pid) == Some(start) {
                    found.push(Holder { pid, start, command, unit });
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn holder(unit: Option<&str>) -> Holder {
        Holder { pid: 1, start: 1, command: "camera".into(), unit: unit.map(str::to_owned) }
    }

    #[test]
    fn borrows_only_when_every_holder_has_the_same_user_unit() {
        assert_eq!(
            borrowable_unit(&[holder(Some("camera.service")), holder(Some("camera.service"))]),
            Some("camera.service".into())
        );
        assert_eq!(
            borrowable_unit(&[holder(Some("camera.service")), holder(Some("other.service"))]),
            None
        );
        assert_eq!(borrowable_unit(&[holder(Some("camera.service")), holder(None)]), None);
    }

    #[test]
    fn compares_both_pid_and_process_start() {
        let first = holder(Some("camera.service"));
        let mut reused = first.clone();
        reused.start += 1;
        assert!(!same_processes(&[first], &[reused]));
    }
}
