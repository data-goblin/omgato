use serde::Deserialize;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

fn request(cmd: &str) -> Result<String, String> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").map_err(|_| "HYPRLAND_INSTANCE_SIGNATURE not set")?;
    let xdg = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set")?;
    let path = PathBuf::from(xdg).join("hypr").join(sig).join(".socket.sock");
    let mut s = UnixStream::connect(&path).map_err(|e| format!("connect {path:?}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_millis(1500))).ok();
    s.write_all(cmd.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| format!("read: {e}"))?;
    Ok(out)
}

fn expect_ok(cmd: &str) -> Result<(), String> {
    let out = request(cmd)?;
    if out.trim() == "ok" { Ok(()) } else { Err(format!("hyprctl {cmd}: {}", out.trim())) }
}

#[derive(Deserialize)]
struct Monitor {
    name: String,
}

fn has_output(name: &str) -> Result<bool, String> {
    let raw = request("j/monitors all")?;
    let monitors: Vec<Monitor> = serde_json::from_str(&raw).map_err(|e| format!("monitors parse: {e}"))?;
    Ok(monitors.iter().any(|m| m.name == name))
}

pub fn ensure_output(name: &str) -> Result<(), String> {
    if has_output(name)? { Ok(()) } else { expect_ok(&format!("/output create headless {name}")) }
}

pub fn remove_output(name: &str) -> Result<(), String> {
    expect_ok(&format!("/output remove {name}"))
}
