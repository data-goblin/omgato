use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub fn run_dir() -> PathBuf {
    let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let p = PathBuf::from(xdg).join("camctl");
    let _ = fs::create_dir_all(&p);
    p
}

pub fn position_file() -> PathBuf { run_dir().join("position") }
pub fn fullscreen_flag() -> PathBuf { run_dir().join("fullscreen") }
pub fn pid_file() -> PathBuf { run_dir().join("ffplay.pid") }
/// Set after the overlay has been hidden at least once. Triggers an
/// auto-reset before the next `show` to clear the UVC wedge that the
/// Cam Link enters when the V4L2 fd closes mid-stream. Lives in
/// XDG_RUNTIME_DIR so it clears on reboot.
pub fn needs_reset_flag() -> PathBuf { run_dir().join("needs_reset") }
/// A user unit that was stopped so the overlay could take the capture device.
/// Started again when the overlay is hidden, so borrowing is always paid back.
pub fn borrowed_unit() -> PathBuf { run_dir().join("borrowed_unit") }

pub fn pause_flag() -> PathBuf {
    dirs::config_dir().expect("no config dir").join("camctl/pause")
}

pub fn read(path: &PathBuf) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub fn write_atomic(path: &PathBuf, value: &str) -> std::io::Result<()> {
    // Include the pid: two camctl processes writing at once would otherwise
    // share one temporary file and could rename each other's half-written state.
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(value.as_bytes())?;
        f.write_all(b"\n")?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
}

pub fn remove(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

pub fn exists(path: &Path) -> bool {
    path.exists()
}
