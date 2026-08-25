use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

pub fn run_dir() -> PathBuf {
    let p = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(xdg) => PathBuf::from(xdg).join("camlink-ctl"),
        Err(_) => std::env::temp_dir().join(format!("camlink-ctl-{}", users_own_uid())),
    };
    let _ = fs::create_dir_all(&p);
    let _ = fs::set_permissions(&p, fs::Permissions::from_mode(0o700));
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

pub struct CommandLock(fs::File);

/// Serialise state-changing commands from the panel, shortcuts and display
/// hooks. Each CLI invocation is a separate process, so atomic files alone do
/// not prevent a hide from overtaking a show or a release from overtaking avoid.
pub fn command_lock() -> std::io::Result<CommandLock> {
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(run_dir().join("command.lock"))?;
    let result = unsafe { flock(file.as_raw_fd(), LOCK_EX) };
    if result == 0 {
        Ok(CommandLock(file))
    } else {
        Err(std::io::Error::last_os_error())
    }
}

impl Drop for CommandLock {
    fn drop(&mut self) {
        let _ = unsafe { flock(self.0.as_raw_fd(), LOCK_UN) };
    }
}

pub fn pause_flag() -> PathBuf {
    dirs::config_dir().expect("no config dir").join("camlink-ctl/pause")
}

pub fn read(path: &PathBuf) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

pub fn write_atomic(path: &PathBuf, value: &str) -> std::io::Result<()> {
    // Include the pid: two camlink-ctl processes writing at once would otherwise
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

fn users_own_uid() -> u32 {
    unsafe { getuid() }
}

unsafe extern "C" {
    fn flock(fd: i32, operation: i32) -> i32;
    fn getuid() -> u32;
}

const LOCK_EX: i32 = 2;
const LOCK_UN: i32 = 8;
