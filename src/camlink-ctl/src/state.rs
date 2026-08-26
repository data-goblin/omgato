use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static RUN_DIR: OnceLock<PathBuf> = OnceLock::new();

pub fn init_run_dir() -> Result<(), String> {
    if RUN_DIR.get().is_some() {
        return Ok(());
    }
    let path = secure_run_dir()?;
    RUN_DIR
        .set(path)
        .map_err(|_| "overlay state directory was initialized concurrently".to_string())
}

pub fn run_dir() -> PathBuf {
    RUN_DIR
        .get()
        .expect("state::init_run_dir must run before dispatch")
        .clone()
}

fn secure_run_dir() -> Result<PathBuf, String> {
    let (p, legacy) = match std::env::var("XDG_RUNTIME_DIR") {
        Ok(xdg) => {
            let runtime = PathBuf::from(xdg);
            verify_private_base(&runtime, "XDG_RUNTIME_DIR")?;
            (runtime.join("camlink-ctl"), Some(runtime.join("camctl")))
        }
        Err(_) => {
            let temp = std::env::temp_dir();
            verify_trusted_directory(&temp, "temporary directory")?;
            (temp.join(format!("camlink-ctl-{}", users_own_uid())), None)
        }
    };
    secure_private_dir(&p)?;
    if let Some(legacy) = legacy {
        migrate_entries(&legacy, &p);
    }
    secure_private_dir(&p.join("blockers"))?;
    Ok(p)
}

fn secure_private_dir(p: &Path) -> Result<(), String> {
    match fs::DirBuilder::new().mode(0o700).create(p) {
        Ok(()) => verify_private_dir(p),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            tighten_if_ours(p)?;
            verify_private_dir(p)
        }
        Err(e) => Err(format!("create {}: {e}", p.display())),
    }
}

fn tighten_if_ours(p: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(p).map_err(|e| format!("stat {}: {e}", p.display()))?;
    if !meta.is_dir() || meta.uid() != users_own_uid() || meta.permissions().mode() & 0o077 == 0 {
        return Ok(());
    }
    fs::set_permissions(p, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("tighten {}: {e}", p.display()))
}

fn verify_private_base(p: &Path, label: &str) -> Result<(), String> {
    if !p.is_absolute() {
        return Err(format!("{label} must be an absolute path: {}", p.display()));
    }
    let parent = p.parent().ok_or_else(|| format!("{label} has no parent"))?;
    verify_trusted_directory(parent, &format!("{label} parent"))?;
    verify_private_dir(p)
}

fn verify_trusted_directory(p: &Path, label: &str) -> Result<(), String> {
    if !p.is_absolute() {
        return Err(format!("{label} must be an absolute path: {}", p.display()));
    }
    let own_uid = users_own_uid();
    let root_uid = fs::symlink_metadata("/")
        .map_err(|e| format!("stat /: {e}"))?
        .uid();
    let mut current = PathBuf::from("/");
    for component in p.components().skip(1) {
        current.push(component.as_os_str());
        let meta = fs::symlink_metadata(&current)
            .map_err(|e| format!("stat {}: {e}", current.display()))?;
        if !meta.is_dir() {
            return Err(format!("{} is not a real directory", current.display()));
        }
        if meta.uid() != own_uid && meta.uid() != root_uid {
            return Err(format!(
                "{} is owned by untrusted uid {}; refusing to use {label}",
                current.display(),
                meta.uid()
            ));
        }
        let mode = meta.permissions().mode() & 0o7777;
        if mode & 0o022 != 0 && mode & 0o1000 == 0 {
            return Err(format!(
                "{} is writable by other users without the sticky bit (mode {mode:o}); refusing to use {label}",
                current.display()
            ));
        }
    }
    Ok(())
}

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
pub fn needs_reset_flag() -> PathBuf { run_dir().join("needs_reset") }
pub fn borrowed_unit() -> PathBuf { run_dir().join("borrowed_unit") }

pub struct CommandLock(fs::File);

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
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    {
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
    if fs::symlink_metadata(to).is_ok()
        || !fs::symlink_metadata(from).is_ok_and(|meta| meta.file_type().is_file())
    {
        return;
    }
    let Some(parent) = to.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let _ = fs::rename(from, to);
}

fn migrate_entries(from: &Path, to: &Path) {
    if !fs::symlink_metadata(from).is_ok_and(|meta| meta.file_type().is_dir()) {
        return;
    }
    let Ok(entries) = fs::read_dir(from) else { return };
    for entry in entries.flatten() {
        let source = entry.path();
        let target = to.join(entry.file_name());
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_dir() {
            if secure_private_dir(&target).is_ok() {
                migrate_entries(&source, &target);
            }
        } else if kind.is_file() && fs::symlink_metadata(&target).is_err() {
            let _ = fs::rename(&source, &target);
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
    fn tightens_a_loose_directory_this_user_owns() {
        let p = scratch("loose");
        fs::DirBuilder::new().mode(0o755).create(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        secure_private_dir(&p).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o700);
        fs::remove_dir_all(&p).unwrap();
    }

    #[test]
    fn accepts_a_private_state_directory() {
        let p = scratch("ok");
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        verify_private_dir(&p).unwrap();
        fs::remove_dir_all(&p).unwrap();
    }

    #[test]
    fn rejects_a_non_sticky_writable_temp_parent() {
        let p = scratch("writable-parent");
        fs::DirBuilder::new().mode(0o700).create(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o777)).unwrap();
        let err = verify_trusted_directory(&p, "test temp").unwrap_err();
        assert!(err.contains("without the sticky bit"), "{err}");
        fs::remove_dir_all(&p).unwrap();
    }

    #[test]
    fn migration_does_not_import_symlinks() {
        let from = scratch("migration-from");
        let to = scratch("migration-to");
        let outside = scratch("migration-outside");
        fs::DirBuilder::new().mode(0o700).create(&from).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&to).unwrap();
        fs::DirBuilder::new().mode(0o700).create(&outside).unwrap();
        fs::write(from.join("position"), "top-left").unwrap();
        symlink(&outside, from.join("blockers")).unwrap();

        migrate_entries(&from, &to);

        assert_eq!(fs::read_to_string(to.join("position")).unwrap(), "top-left");
        assert!(fs::symlink_metadata(to.join("blockers")).is_err());
        assert!(fs::symlink_metadata(from.join("blockers")).unwrap().file_type().is_symlink());
        fs::remove_dir_all(&from).unwrap();
        fs::remove_dir_all(&to).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }
}
