use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

pub fn socket_path() -> Result<PathBuf, String> {
    let xdg = std::env::var("XDG_RUNTIME_DIR").map_err(|_| "XDG_RUNTIME_DIR not set")?;
    Ok(PathBuf::from(xdg).join("prompter-ctl").join("control.sock"))
}

pub fn send(line: &str) -> Result<serde_json::Value, String> {
    let path = socket_path()?;
    let mut s = UnixStream::connect(&path).map_err(|_| "prompter-ctl is not running; is the Prompter plugged in?".to_string())?;
    s.set_read_timeout(Some(Duration::from_secs(3))).ok();
    s.write_all(format!("{line}\n").as_bytes()).map_err(|e| format!("send: {e}"))?;
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| format!("reply: {e}"))?;
    let reply: serde_json::Value = serde_json::from_str(out.trim()).map_err(|e| format!("reply {out:?}: {e}"))?;
    match reply.get("error").and_then(|e| e.as_str()) {
        Some(e) => Err(e.to_owned()),
        None => Ok(reply),
    }
}
