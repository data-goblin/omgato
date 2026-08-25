use crate::positioning::{self, Placement};
use crate::state;
use std::fs;
use std::path::PathBuf;

/// Rectangles that panels have claimed, one file per owner.
///
/// A single shared file could not describe two panels, or the same panel on two
/// monitors, and a shell that died left its claim behind for good. Each claim is
/// therefore its own file carrying the pid that made it, so a claim whose owner
/// is gone is ignored and cleaned up rather than blocking the overlay forever.
pub fn dir() -> PathBuf {
    let p = state::run_dir().join("blockers");
    let _ = fs::create_dir_all(&p);
    p
}

/// Keep an owner id to something that cannot escape the directory.
fn safe_name(owner: &str) -> String {
    let cleaned: String = owner
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if cleaned.is_empty() { "panel".to_string() } else { cleaned }
}

/// The process to hold the claim against. camlink-ctl is spawned by the shell, so
/// its parent is the process whose life the claim should follow. Read from
/// /proc/self/status rather than /proc/self/stat, whose comm field can itself
/// contain spaces and brackets.
fn owning_pid() -> u32 {
    fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("PPid:"))
                .and_then(|v| v.trim().parse().ok())
        })
        .unwrap_or_else(std::process::id)
}

fn start_time(pid: u32) -> Option<u64> {
    let stat = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let (_, after_comm) = stat.rsplit_once(") ")?;
    after_comm.split_whitespace().nth(19)?.parse().ok()
}

fn identity(pid: u32) -> String {
    match start_time(pid) {
        Some(start) => format!("{pid} {start}"),
        None => pid.to_string(),
    }
}

fn alive(line: &str) -> bool {
    let mut fields = line.split_whitespace();
    let Some(pid) = fields.next().and_then(|v| v.parse::<u32>().ok()) else {
        return false;
    };
    match fields.next() {
        Some(wanted) => start_time(pid).is_some_and(|actual| wanted == actual.to_string()),
        None => PathBuf::from(format!("/proc/{pid}")).exists(),
    }
}

pub fn claim(owner: &str, rect: &Placement) -> std::io::Result<()> {
    let path = dir().join(safe_name(owner));
    let body = format!("{}\n{}\n", positioning::rect_to_position(rect), identity(owning_pid()));
    let tmp = path.with_file_name(format!(
        "{}.{}.tmp",
        path.file_name().and_then(|n| n.to_str()).unwrap_or("panel"),
        std::process::id()
    ));
    fs::write(&tmp, body)?;
    fs::rename(&tmp, &path)
}

pub fn release(owner: &str) {
    let _ = fs::remove_file(dir().join(safe_name(owner)));
}

/// Every claim whose owner is still running. Claims from a dead owner are
/// deleted on the way past, so a crashed shell cleans itself up.
pub fn live() -> Vec<Placement> {
    let Ok(entries) = fs::read_dir(dir()) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "tmp") {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let mut lines = text.lines();
        let Some(rect) = lines.next().and_then(positioning::parse_rect) else {
            let _ = fs::remove_file(&path);
            continue;
        };
        let Some(owner) = lines.next() else {
            let _ = fs::remove_file(&path);
            continue;
        };
        if alive(owner) {
            out.push(rect);
        } else {
            let _ = fs::remove_file(&path);
        }
    }
    out
}

/// The single rectangle to dodge: the bounding box of every live claim that the
/// overlay actually overlaps. Dodging each in turn could bounce the overlay
/// between two panels without ever clearing either.
pub fn obstruction(overlay: &Placement) -> Option<Placement> {
    let overlapping: Vec<Placement> = live()
        .into_iter()
        .filter(|b| {
            overlay.x < b.x.saturating_add(b.w)
                && b.x < overlay.x.saturating_add(overlay.w)
                && overlay.y < b.y.saturating_add(b.h)
                && b.y < overlay.y.saturating_add(overlay.h)
        })
        .collect();
    if overlapping.is_empty() {
        return None;
    }
    let left = overlapping.iter().map(|b| b.x).min()?;
    let top = overlapping.iter().map(|b| b.y).min()?;
    let right = overlapping.iter().map(|b| b.x.saturating_add(b.w)).max()?;
    let bottom = overlapping.iter().map(|b| b.y.saturating_add(b.h)).max()?;
    Some(Placement {
        x: left,
        y: top,
        w: right.saturating_sub(left),
        h: bottom.saturating_sub(top),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_identity_rejects_malformed_and_reused_processes() {
        let pid = std::process::id();
        let current = identity(pid);
        assert!(alive(&current));
        assert!(!alive("not-a-pid"));
        assert!(!alive(&format!("{pid} 0")));
    }
}
