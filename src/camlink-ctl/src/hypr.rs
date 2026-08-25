use serde::Deserialize;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

fn socket_path() -> Result<PathBuf, String> {
    let sig = std::env::var("HYPRLAND_INSTANCE_SIGNATURE")
        .map_err(|_| "HYPRLAND_INSTANCE_SIGNATURE not set".to_string())?;
    let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    Ok(PathBuf::from(xdg).join("hypr").join(sig).join(".socket.sock"))
}

pub fn dispatch(cmd: &str) -> Result<String, String> {
    let path = socket_path()?;
    let mut s = UnixStream::connect(&path).map_err(|e| format!("connect {path:?}: {e}"))?;
    s.set_read_timeout(Some(Duration::from_millis(1500))).ok();
    s.write_all(cmd.as_bytes()).map_err(|e| format!("write: {e}"))?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| format!("read: {e}"))?;
    Ok(out)
}

pub fn dispatch_j(cmd: &str) -> Result<String, String> {
    dispatch(&format!("j/{cmd}"))
}

#[derive(Debug, Clone, Deserialize)]
pub struct Client {
    pub address: String,
    pub title: String,
    #[serde(default)]
    pub mapped: bool,
    pub monitor: i32,
    /// A pinned floating window stays visible across workspace switches, which
    /// is what makes the overlay a picture-in-picture rather than a window that
    /// disappears the moment you move workspace.
    #[serde(default)]
    pub pinned: bool,
}

pub fn clients() -> Result<Vec<Client>, String> {
    let raw = dispatch_j("clients")?;
    serde_json::from_str(&raw).map_err(|e| format!("clients parse: {e}"))
}

pub fn find_window(title: &str) -> Result<Option<Client>, String> {
    Ok(clients()?.into_iter().find(|c| c.title == title))
}

#[derive(Debug, Clone, Deserialize)]
pub struct Monitor {
    #[allow(dead_code)] pub id: i32,
    #[serde(default)]
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub scale: f32,
    pub focused: bool,
    /// Layer-shell reserved space: [left, top, right, bottom] in logical pixels.
    /// E.g. a top waybar with exclusive_zone=26 reports [0, 26, 0, 0].
    #[serde(default)]
    pub reserved: [i32; 4],
}

pub fn monitors() -> Result<Vec<Monitor>, String> {
    let raw = dispatch_j("monitors")?;
    serde_json::from_str(&raw).map_err(|e| format!("monitors parse: {e}"))
}

pub fn focused_monitor() -> Result<Monitor, String> {
    monitors()?.into_iter().find(|m| m.focused)
        .ok_or_else(|| "no focused monitor".into())
}

pub fn named_monitor(name: &str) -> Result<Option<Monitor>, String> {
    Ok(monitors()?.into_iter().find(|m| m.name == name))
}

pub fn monitor_for_address(addr: &str) -> Result<Option<Monitor>, String> {
    let cs = clients()?;
    let c = match cs.into_iter().find(|c| c.address == addr) {
        Some(c) => c,
        None => return Ok(None),
    };
    Ok(monitors()?.into_iter().find(|m| m.id == c.monitor))
}


// Hyprland 0.56 replaced the flat dispatcher strings with a Lua API, and the
// old "movewindowpixel exact ..." form now fails to parse, silently leaving the
// overlay wherever it was mapped.
pub fn move_window_pixel(addr: &str, x: i32, y: i32) -> Result<(), String> {
    dispatch_action(&format!(
        "/dispatch hl.dsp.window.move({{ x = {x}, y = {y}, exact = true, window = \"address:{addr}\" }})"
    ))
}

pub fn resize_window_pixel(addr: &str, w: i32, h: i32) -> Result<(), String> {
    dispatch_action(&format!(
        "/dispatch hl.dsp.window.resize({{ x = {w}, y = {h}, exact = true, window = \"address:{addr}\" }})"
    ))
}

/// Keeps the overlay visible across workspace switches.
///
/// The dispatcher toggles, so calling it on an already pinned window unpins it.
/// Every reposition used to call this, which left the overlay pinned only on an
/// even number of moves and stranded on one workspace the rest of the time.
pub fn pin_window(addr: &str) -> Result<(), String> {
    dispatch_action(&format!(
        "/dispatch hl.dsp.window.pin({{ window = \"address:{addr}\" }})"
    ))
}

fn dispatch_action(cmd: &str) -> Result<(), String> {
    let response = dispatch(cmd)?;
    if response.trim() == "ok" {
        Ok(())
    } else {
        Err(format!("Hyprland rejected the command: {}", response.trim()))
    }
}
