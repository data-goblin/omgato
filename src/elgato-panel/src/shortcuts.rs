//! Keyboard shortcuts the plugin owns. They live in a Lua file of their own,
//! sourced by one guarded line, so the user's own bindings file is never
//! rewritten and removing the plugin cannot break their config.
use crate::sh;
use crate::state;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const SETTINGS: &str = "shortcuts.json";
const MARKER: &str = "Elgato:";
const REQUIRE_LINE: &str = r#"pcall(require, "hypr.elgato-bindings")"#;

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
    Action { id: "lights.toggle", view: "lights", label: "Toggle all lights", command: "elgatoctl click", default_keys: "SUPER + ALT + L" },
    Action { id: "lights.brighter", view: "lights", label: "Lights brighter", command: "elgatoctl brightness +10", default_keys: "" },
    Action { id: "lights.dimmer", view: "lights", label: "Lights dimmer", command: "elgatoctl brightness -10", default_keys: "" },
    Action { id: "lights.warmer", view: "lights", label: "Lights warmer", command: "elgatoctl temperature -300", default_keys: "" },
    Action { id: "lights.cooler", view: "lights", label: "Lights cooler", command: "elgatoctl temperature +300", default_keys: "" },
    Action { id: "lights.sync", view: "lights", label: "Match all lights", command: "elgato-panel sync", default_keys: "" },
    Action { id: "deck.power", view: "deck", label: "Toggle deck display", command: "streamdeck-ctl deck power toggle", default_keys: "" },
    Action { id: "deck.reload", view: "deck", label: "Reload the deck", command: "streamdeck-ctl deck reload", default_keys: "" },
    Action { id: "camera.toggle", view: "camera", label: "Toggle camera overlay", command: "camctl toggle", default_keys: "SUPER + ALT + C" },
    Action { id: "camera.pick", view: "camera", label: "Place the camera", command: "camctl pick", default_keys: "SUPER + ALT + P" },
    Action { id: "camera.full", view: "camera", label: "Camera fullscreen", command: "camctl full", default_keys: "SUPER + SHIFT + C" },
    Action { id: "camera.tl", view: "camera", label: "Camera top-left", command: "camctl move tl", default_keys: "SUPER + ALT + 1" },
    Action { id: "camera.tr", view: "camera", label: "Camera top-right", command: "camctl move tr", default_keys: "SUPER + ALT + 2" },
    Action { id: "camera.bl", view: "camera", label: "Camera bottom-left", command: "camctl move bl", default_keys: "SUPER + ALT + 3" },
    Action { id: "camera.br", view: "camera", label: "Camera bottom-right", command: "camctl move br", default_keys: "SUPER + ALT + 4" },
    Action { id: "record.region", view: "camera", label: "Record an area", command: "elgato-panel record --target region", default_keys: "SUPER + ALT + R" },
    Action { id: "record.stop", view: "camera", label: "Stop recording", command: "elgato-panel record --stop", default_keys: "SUPER + ALT + SHIFT + R" },
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
    fs::read_to_string(user_bindings_path()).is_ok_and(|s| s.contains(REQUIRE_LINE))
}

fn render(configured: &Bindings) -> String {
    let mut out = String::from(
        "-- Generated by the Omarchy Elgato plugin. Edit the shortcuts from the\n\
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
    let mut text = fs::read_to_string(&user).unwrap_or_default();
    if !text.contains(REQUIRE_LINE) {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&format!(
            "\n-- Omarchy Elgato plugin shortcuts. Safe to delete along with the plugin.\n{REQUIRE_LINE}\n"
        ));
        fs::write(&user, text).map_err(|e| format!("write {}: {e}", user.display()))?;
    }
    reload();
    Ok(())
}

pub fn uninstall() -> Result<(), String> {
    let user = user_bindings_path();
    if let Ok(text) = fs::read_to_string(&user) {
        let kept: String = text
            .lines()
            .filter(|line| !line.contains(REQUIRE_LINE) && !line.contains("Omarchy Elgato plugin shortcuts"))
            .collect::<Vec<_>>()
            .join("\n");
        let _ = fs::write(&user, kept + "\n");
    }
    let _ = fs::remove_file(bindings_path());
    reload();
    Ok(())
}

fn write_bindings(configured: &Bindings) -> Result<(), String> {
    let path = bindings_path();
    fs::write(&path, render(configured)).map_err(|e| format!("write {}: {e}", path.display()))
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
    state::write_state(SETTINGS, &configured);
    if bindings_path().exists() {
        write_bindings(&configured)?;
        reload();
    }
    Ok(())
}

fn reload() {
    sh::run(&["hyprctl", "reload"]);
}
