//! Keyboard shortcuts the plugin owns. They live in a Lua file of their own,
//! sourced by one guarded line, so the user's own bindings file is never
//! rewritten and removing the plugin cannot break their config.
use crate::sh;
use crate::state;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

const SETTINGS: &str = "shortcuts.json";
const MARKER: &str = "Omgato:";
/// The marker written before the plugin was renamed. Still matched when reading
/// so shortcuts installed under the old name are recognised and can be removed
/// or replaced rather than left orphaned in the user's bindings.
const LEGACY_MARKER: &str = "Elgato:";
const REQUIRE_LINE: &str = r#"pcall(require, "hypr.omgato-bindings")"#;
const LEGACY_REQUIRE_LINE: &str = r#"pcall(require, "hypr.elgato-bindings")"#;
const SOURCE_COMMENT: &str = "-- Omgato plugin shortcuts. Safe to delete along with the plugin.";
const LEGACY_SOURCE_COMMENT: &str = "-- Omarchy Elgato plugin shortcuts. Safe to delete along with the plugin.";

/// One bindable action: which panel view it belongs to, what it runs, and the
/// combination it is bound to.
pub struct Action {
    pub id: &'static str,
    pub view: &'static str,
    pub label: &'static str,
    pub command: &'static str,
    pub default_keys: &'static str,
}

/// An empty default leaves the action unbound: it is offered in the panel but
/// claims no combination until the user gives it one.
pub const ACTIONS: &[Action] = &[
    Action { id: "lights.toggle", view: "lights", label: "Toggle all lights", command: "keylight-ctl click", default_keys: "SUPER + ALT + L" },
    Action { id: "lights.brighter", view: "lights", label: "Lights brighter", command: "keylight-ctl brightness +10", default_keys: "" },
    Action { id: "lights.dimmer", view: "lights", label: "Lights dimmer", command: "keylight-ctl brightness -10", default_keys: "" },
    Action { id: "lights.warmer", view: "lights", label: "Lights warmer", command: "keylight-ctl temperature -300", default_keys: "" },
    Action { id: "lights.cooler", view: "lights", label: "Lights cooler", command: "keylight-ctl temperature +300", default_keys: "" },
    Action { id: "lights.sync", view: "lights", label: "Match all lights", command: "omgato-panel sync", default_keys: "" },
    Action { id: "deck.power", view: "deck", label: "Toggle deck display", command: "streamdeck-ctl deck power toggle", default_keys: "" },
    Action { id: "deck.reload", view: "deck", label: "Reload the deck", command: "streamdeck-ctl deck reload", default_keys: "" },
    Action { id: "camera.toggle", view: "camera", label: "Toggle camera overlay", command: "camlink-ctl toggle", default_keys: "SUPER + ALT + C" },
    Action { id: "camera.pick", view: "camera", label: "Place the camera", command: "camlink-ctl pick", default_keys: "SUPER + ALT + P" },
    Action { id: "camera.full", view: "camera", label: "Camera fullscreen", command: "camlink-ctl full", default_keys: "SUPER + SHIFT + C" },
    Action { id: "camera.tl", view: "camera", label: "Camera top-left", command: "camlink-ctl move tl", default_keys: "SUPER + ALT + 1" },
    Action { id: "camera.tr", view: "camera", label: "Camera top-right", command: "camlink-ctl move tr", default_keys: "SUPER + ALT + 2" },
    Action { id: "camera.bl", view: "camera", label: "Camera bottom-left", command: "camlink-ctl move bl", default_keys: "SUPER + ALT + 3" },
    Action { id: "camera.br", view: "camera", label: "Camera bottom-right", command: "camlink-ctl move br", default_keys: "SUPER + ALT + 4" },
    Action { id: "record.region", view: "camera", label: "Record an area", command: "omgato-panel record --target region", default_keys: "SUPER + ALT + R" },
    Action { id: "record.stop", view: "camera", label: "Stop recording", command: "omgato-panel record --stop", default_keys: "SUPER + ALT + SHIFT + R" },
];

/// Hypr spells punctuation out; a shortcut list should show the key on the cap.
fn symbol(key: &str) -> &str {
    match key {
        "semicolon" => ";",
        "apostrophe" => "'",
        "bracketleft" => "[",
        "bracketright" => "]",
        "backslash" => "\\",
        "comma" => ",",
        "period" => ".",
        "slash" => "/",
        "minus" => "-",
        "equal" => "=",
        "grave" => "`",
        "space" => "Space",
        "Return" => "Enter",
        "BackSpace" => "Backspace",
        other => other,
    }
}

/// The same combination written for a person to read.
fn pretty(keys: &str) -> String {
    if keys.trim().is_empty() {
        return String::new();
    }
    let parts: Vec<String> = keys
        .split('+')
        .map(str::trim)
        .map(|part| if modmask(part) == 0 { symbol(part).to_owned() } else { part.to_owned() })
        .collect();
    parts.join(" + ")
}

#[derive(Serialize)]
pub struct Shortcut {
    pub id: String,
    pub view: String,
    pub label: String,
    pub keys: String,
    /// The same combination with punctuation shown as the key it is printed on.
    pub display: String,
    /// Description of the binding that already owns this combination, if any.
    pub conflict: String,
}

#[derive(Serialize)]
pub struct Status {
    pub installed: bool,
    pub shortcuts: Vec<Shortcut>,
}

type Bindings = BTreeMap<String, String>;

fn bindings_path() -> PathBuf {
    hypr_dir().join("omgato-bindings.lua")
}

fn legacy_bindings_path() -> PathBuf {
    hypr_dir().join("elgato-bindings.lua")
}

fn user_bindings_path() -> PathBuf {
    hypr_dir().join("bindings.lua")
}

fn hypr_dir() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("hypr")
}

fn configured() -> Bindings {
    state::read_state(SETTINGS).unwrap_or_default()
}

fn keys_for(action: &Action, configured: &Bindings) -> String {
    configured
        .get(action.id)
        .cloned()
        .unwrap_or_else(|| action.default_keys.to_owned())
}

/// X11 modifier bits, as Hyprland reports them in `hyprctl binds`.
fn modmask(keys: &str) -> u32 {
    keys.split('+')
        .map(str::trim)
        .filter_map(|part| match part.to_ascii_uppercase().as_str() {
            "SHIFT" => Some(1),
            "CAPS" => Some(2),
            "CTRL" | "CONTROL" => Some(4),
            "ALT" => Some(8),
            "SUPER" | "MOD" | "WIN" => Some(64),
            _ => None,
        })
        .sum()
}

fn key_of(keys: &str) -> String {
    keys.split('+')
        .map(str::trim).rfind(|part| modmask(part) == 0)
        .unwrap_or("")
        .to_owned()
}

#[derive(Deserialize)]
struct Bind {
    #[serde(default)]
    modmask: u32,
    #[serde(default)]
    key: String,
    #[serde(default)]
    description: String,
}

fn current_binds() -> Vec<Bind> {
    serde_json::from_str(&sh::run(&["hyprctl", "binds", "-j"])).unwrap_or_default()
}

/// Names the binding that already owns a combination, ignoring the plugin's own.
fn conflict_for(keys: &str, binds: &[Bind]) -> String {
    let (mask, key) = (modmask(keys), key_of(keys));
    if key.is_empty() {
        return String::new();
    }
    binds
        .iter()
        .find(|b| {
            b.modmask == mask
                && b.key.eq_ignore_ascii_case(&key)
                && !b.description.starts_with(MARKER)
                && !b.description.starts_with(LEGACY_MARKER)
        })
        .map(|b| {
            if b.description.is_empty() {
                "another binding".to_owned()
            } else {
                b.description.clone()
            }
        })
        .unwrap_or_default()
}

pub fn status(with_conflicts: bool) -> Status {
    let configured = configured();
    let binds = if with_conflicts { current_binds() } else { Vec::new() };
    Status {
        installed: bindings_path().exists() && sourced(),
        shortcuts: ACTIONS
            .iter()
            .map(|action| {
                let keys = keys_for(action, &configured);
                Shortcut {
                    conflict: conflict_for(&keys, &binds),
                    display: pretty(&keys),
                    id: action.id.to_owned(),
                    view: action.view.to_owned(),
                    label: action.label.to_owned(),
                    keys,
                }
            })
            .collect(),
    }
}

fn sourced() -> bool {
    fs::read_to_string(user_bindings_path()).is_ok_and(|s| {
        s.lines().any(|line| line.trim() == REQUIRE_LINE)
    })
}

fn render(configured: &Bindings) -> String {
    let mut out = String::from(
        "-- Generated by the Omgato plugin. Edit the shortcuts from the\n\
         -- panel rather than here; this file is rewritten when they change.\n\n",
    );
    for action in ACTIONS {
        let keys = keys_for(action, configured);
        if keys.trim().is_empty() {
            continue;
        }
        out.push_str(&format!(
            "o.bind({:?}, {:?}, {:?})\n",
            keys,
            format!("{MARKER} {}", action.label),
            action.command
        ));
    }
    out
}

/// Writes the bindings file and adds the one guarded line that sources it.
pub fn install() -> Result<(), String> {
    let dir = hypr_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    write_bindings(&configured())?;

    let user = user_bindings_path();
    let original = match fs::read_to_string(&user) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("read {}: {e}", user.display())),
    };
    let text = source_text(&original, true);
    if text != original {
        write_text_atomic(&user, &text)?;
    }
    remove_file(&legacy_bindings_path())?;
    reload();
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let user = user_bindings_path();
    match fs::read_to_string(&user) {
        Ok(text) => {
            let kept = source_text(&text, false);
            if kept != text {
                write_text_atomic(&user, &kept)?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(format!("read {}: {e}", user.display())),
    }
    remove_file(&bindings_path())?;
    remove_file(&legacy_bindings_path())?;
    reload();
    Ok(())
}

fn write_bindings(configured: &Bindings) -> Result<(), String> {
    let path = bindings_path();
    write_text_atomic(&path, &render(configured))
}

/// Rebinds one action and regenerates the file, if it is already installed.
pub fn set(id: &str, keys: &str) -> Result<(), String> {
    if !ACTIONS.iter().any(|a| a.id == id) {
        return Err(format!("unknown shortcut: {id}"));
    }
    let mut configured = configured();
    if keys.trim().is_empty() {
        configured.remove(id);
    } else {
        configured.insert(id.to_owned(), keys.trim().to_owned());
    }
    state::write_state_checked(SETTINGS, &configured)
        .map_err(|e| format!("write shortcut settings: {e}"))?;
    if bindings_path().exists() {
        write_bindings(&configured)?;
        reload();
    }
    Ok(())
}

fn reload() {
    sh::run(&["hyprctl", "reload"]);
}

fn source_text(text: &str, install: bool) -> String {
    let mut kept: Vec<&str> = text
        .lines()
        .filter(|line| {
            let line = line.trim();
            line != REQUIRE_LINE
                && line != LEGACY_REQUIRE_LINE
                && line != SOURCE_COMMENT
                && line != LEGACY_SOURCE_COMMENT
        })
        .collect();
    while kept.last() == Some(&"") {
        kept.pop();
    }
    if install {
        if !kept.is_empty() {
            kept.push("");
        }
        kept.push(SOURCE_COMMENT);
        kept.push(REQUIRE_LINE);
    }
    let mut out = kept.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    out
}

fn write_text_atomic(path: &std::path::Path, text: &str) -> Result<(), String> {
    let parent = path.parent().ok_or_else(|| format!("{} has no parent", path.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("bindings");
    let tmp = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&tmp)
        .map_err(|e| format!("write {}: {e}", path.display()))?;
    file.write_all(text.as_bytes()).map_err(|e| format!("write {}: {e}", path.display()))?;
    if let Ok(meta) = fs::symlink_metadata(path)
        && meta.file_type().is_file()
    {
        let _ = file.set_permissions(fs::Permissions::from_mode(meta.permissions().mode()));
    }
    file.sync_all().ok();
    fs::rename(&tmp, path).map_err(|e| format!("write {}: {e}", path.display()))
}

fn remove_file(path: &std::path::Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("remove {}: {e}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_replaces_legacy_and_duplicate_source_lines() {
        let input = format!(
            "o.bind({{}})\n{LEGACY_SOURCE_COMMENT}\n{LEGACY_REQUIRE_LINE}\n{SOURCE_COMMENT}\n{REQUIRE_LINE}\n"
        );
        assert_eq!(
            source_text(&input, true),
            format!("o.bind({{}})\n\n{SOURCE_COMMENT}\n{REQUIRE_LINE}\n")
        );
    }

    #[test]
    fn uninstall_removes_both_generations_of_source_line() {
        let input = format!("before\n{LEGACY_REQUIRE_LINE}\n{REQUIRE_LINE}\nafter\n");
        assert_eq!(source_text(&input, false), "before\nafter\n");
    }
}
