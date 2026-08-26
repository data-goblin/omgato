use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const VENDOR: &str = "0fd9";
const PRODUCT: &str = "007b";

pub fn find_usb_path() -> Option<PathBuf> {
    let dir = Path::new("/sys/bus/usb/devices");
    let entries = fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        let v = fs::read_to_string(p.join("idVendor")).ok();
        let pr = fs::read_to_string(p.join("idProduct")).ok();
        if let (Some(v), Some(pr)) = (v, pr)
            && v.trim() == VENDOR && pr.trim() == PRODUCT {
                return Some(p);
            }
    }
    None
}

pub fn reset() -> Result<PathBuf, String> {
    let path = find_usb_path()
        .ok_or_else(|| format!("Cam Link 4K (USB {VENDOR}:{PRODUCT}) not found"))?;
    let auth = path.join("authorized");

    write_via_sudo(&auth, "0")?;
    std::thread::sleep(Duration::from_millis(800));
    write_via_sudo(&auth, "1")?;
    std::thread::sleep(Duration::from_millis(1500));
    Ok(path)
}

fn write_via_sudo(path: &Path, value: &str) -> Result<(), String> {
    let path_str = path.to_str().ok_or("non-utf8 path")?;
    let mut child = Command::new("sudo")
        .args(["-n", "tee", path_str])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn sudo tee: {e}"))?;
    if let Some(mut sin) = child.stdin.take() {
        sin.write_all(value.as_bytes())
            .map_err(|e| format!("write stdin: {e}"))?;
    }
    let out = child.wait_with_output()
        .map_err(|e| format!("wait sudo tee: {e}"))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!(
            "sudo tee {} <- {}: exit {:?}. {}",
            path_str, value, out.status.code(), stderr.trim(),
        ));
    }
    Ok(())
}
