use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// The directory holding this user's overlay state, created private to the
/// user. A directory that already exists is verified rather than trusted: the
/// temp-directory fallback is a predictable path, so another local user could
/// otherwise pre-create it and plant symlinks that redirect every state write.
/// A directory that cannot be secured ends the process instead of downgrading
/// to an unsafe one.
pub fn run_dir() -> PathBuf {
    match secure_run_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("camlink-ctl: {e}");
            std::process::exit(1);
        }
    }
}

fn secure_run_dir() -> Result<PathBuf, String> {
    let (p, legacy) = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(xdg) => {
            let runtime = PathBuf::from(xdg);
            (runtime.join("camlink-ctl"), Some(runtime.join("camctl")))
        }
        Err(_) => (
            std::env::temp_dir().join(format!("camlink-ctl-{}", users_own_uid())),
            None,
        ),
    };
    match fs::DirBuilder::new().mode(0o700).create(&p) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => verify_private_dir(&p)?,
        Err(e) => return Err(format!("create {}: {e}", p.display())),
    }
    if let Some(legacy) = legacy {
        migrate_entries(&legacy, &p);
    }
    Ok(p)
}

/// Accept an existing state directory only if it is a real directory this user
/// owns and no other user can enter. Reads metadata without following symlinks,
/// so a link planted at the path is rejected rather than resolved to its target.
fn verify_private_dir(p: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(p).map_err(|e| format!("stat {}: {e}", p.display()))?;
    if !meta.is_dir() {
        return Err(format!("{} exists but is not a directory", p.display()));
    }
    let uid = users_own_uid();
    if meta.uid() != uid {
        return Err(format!(
            "{} is owned by uid {}, not {uid}; refusing to use it",
            p.display(),
            meta.uid()
        ));
    }
    let mode = meta.permissions().mode() & 0o7777;
    if mode & 0o077 != 0 {
        return Err(format!(
            "{} is reachable by other users (mode {mode:o}); refusing to use it",
            p.display()
        ));
    }
    Ok(())
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
    let config = dirs::config_dir().expect("no config dir");
    let path = config.join("camlink-ctl/pause");
    migrate_file(&config.join("camctl/pause"), &path);
    path
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
        let _ = fs::remove_file(&tmp);
        let mut f = fs::OpenOptions::new().create_new(true).write(true).open(&tmp)?;
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

fn migrate_file(from: &Path, to: &Path) {
    if to.exists() || !from.is_file() {
        return;
    }
    let Some(parent) = to.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    if fs::rename(from, to).is_err() && !to.exists() {
        let _ = fs::copy(from, to);
    }
}

fn migrate_entries(from: &Path, to: &Path) {
    if !from.is_dir() || fs::create_dir_all(to).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(from) else { return };
    for entry in entries.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() && target.is_dir() {
            migrate_entries(&source, &target);
        } else if !target.exists() {
            let _ = fs::rename(source, target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn scratch(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("camlink-state-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn rejects_a_state_directory_other_users_can_enter() {
        let p = scratch("open");
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o777)).unwrap();
        let err = verify_private_dir(&p).unwrap_err();
        assert!(err.contains("reachable by other users"), "{err}");
        fs::remove_dir_all(&p).unwrap();
    }

    #[test]
    fn rejects_a_symlink_planted_at_the_state_directory() {
        let target = scratch("link-target");
        fs::DirBuilder::new().mode(0o700).create(&target).unwrap();
        let p = scratch("link");
        symlink(&target, &p).unwrap();
        let err = verify_private_dir(&p).unwrap_err();
        assert!(err.contains("not a directory"), "{err}");
        fs::remove_file(&p).unwrap();
        fs::remove_dir_all(&target).unwrap();
    }

    #[test]
    fn accepts_a_private_state_directory() {
        let p = scratch("ok");
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        verify_private_dir(&p).unwrap();
        fs::remove_dir_all(&p).unwrap();
    }
}
