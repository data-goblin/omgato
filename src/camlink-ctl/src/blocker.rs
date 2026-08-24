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

fn alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

pub fn claim(owner: &str, rect: &Placement) -> std::io::Result<()> {
    let path = dir().join(safe_name(owner));
    let body = format!("{}\n{}\n", positioning::rect_to_position(rect), owning_pid());
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
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
        let owner_pid: Option<u32> = lines.next().and_then(|p| p.trim().parse().ok());
        match owner_pid {
            Some(pid) if !alive(pid) => {
                let _ = fs::remove_file(&path);
            }
            _ => out.push(rect),
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
            overlay.x < b.x + b.w
                && b.x < overlay.x + overlay.w
                && overlay.y < b.y + b.h
                && b.y < overlay.y + overlay.h
        })
        .collect();
    if overlapping.is_empty() {
        return None;
    }
    let left = overlapping.iter().map(|b| b.x).min()?;
    let top = overlapping.iter().map(|b| b.y).min()?;
    let right = overlapping.iter().map(|b| b.x + b.w).max()?;
    let bottom = overlapping.iter().map(|b| b.y + b.h).max()?;
    Some(Placement { x: left, y: top, w: right - left, h: bottom - top })
}
