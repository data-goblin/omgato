use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

pub fn temp_fallback(name: &str) -> Result<PathBuf, String> {
    let temp = std::env::temp_dir();
    verify_trusted_directory(&temp, "temporary directory")?;
    let p = temp.join(format!("{name}-{}", users_own_uid()));
    secure(&p)?;
    Ok(p)
}

fn secure(p: &Path) -> Result<(), String> {
    match fs::DirBuilder::new().mode(0o700).create(p) {
        Ok(()) => verify_private_dir(p),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => verify_private_dir(p),
        Err(e) => Err(format!("create {}: {e}", p.display())),
    }
}

pub fn verify_trusted_directory(p: &Path, label: &str) -> Result<(), String> {
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
                current.display(),
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

fn users_own_uid() -> u32 {
    unsafe { getuid() }
}

unsafe extern "C" {
    fn getuid() -> u32;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn scratch(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("omgato-privdir-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        let _ = fs::remove_file(&p);
        p
    }

    #[test]
    fn creates_a_directory_only_this_user_can_enter() {
        let p = scratch("fresh");
        secure(&p).unwrap();
        let mode = fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        fs::remove_dir_all(&p).unwrap();
    }

    #[test]
    fn rejects_a_directory_other_users_can_enter() {
        let p = scratch("open");
        fs::DirBuilder::new().mode(0o777).create(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o777)).unwrap();
        let err = secure(&p).unwrap_err();
        assert!(err.contains("reachable by other users"), "{err}");
        fs::remove_dir_all(&p).unwrap();
    }

    #[test]
    fn rejects_a_symlink_planted_at_the_path() {
        let target = scratch("link-target");
        fs::DirBuilder::new().mode(0o700).create(&target).unwrap();
        let p = scratch("link");
        symlink(&target, &p).unwrap();
        let err = secure(&p).unwrap_err();
        assert!(err.contains("not a directory"), "{err}");
        fs::remove_file(&p).unwrap();
        fs::remove_dir_all(&target).unwrap();
    }

    #[test]
    fn accepts_a_directory_it_created_earlier() {
        let p = scratch("reuse");
        secure(&p).unwrap();
        secure(&p).unwrap();
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
}
