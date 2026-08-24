use crate::sh;
use crate::state::{Snap, load_aliases, load_order};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Row {
    pub name: String,
    pub ip: String,
    pub reachable: bool,
    pub on: bool,
    pub brightness: u8,
    pub kelvin: u32,
}

#[derive(Debug, Serialize)]
pub struct Light {
    pub name: String,
    pub display: String,
    pub ip: String,
    pub reachable: bool,
    pub on: bool,
    pub brightness: u8,
    pub kelvin: u32,
}

pub fn read() -> Vec<Light> {
    let aliases = load_aliases();
    let order = load_order();
    let rows: Vec<Row> = serde_json::from_str(&sh::run(&["elgatoctl", "--json", "ls"])).unwrap_or_default();
    let mut lights: Vec<Light> = rows
        .into_iter()
        .map(|r| Light {
            display: aliases.get(&r.ip).cloned().unwrap_or_else(|| r.name.clone()),
            name: r.name,
            ip: r.ip,
            reachable: r.reachable,
            on: r.on,
            brightness: r.brightness,
            kelvin: r.kelvin,
        })
        .collect();
    lights.sort_by_key(|l| order.iter().position(|ip| *ip == l.ip).unwrap_or(order.len()));
    lights
}

/// Carries forward the last recorded values for a light that is not answering.
/// Skipping the snapshot outright used to freeze undo for every light for as
/// long as one stayed offline.
pub fn snapshot(lights: &[Light], previous: Option<&Vec<Snap>>) -> Option<Vec<Snap>> {
    if lights.is_empty() || lights.iter().all(|l| !l.reachable) {
        return None;
    }
    let mut snap: Vec<Snap> = lights
        .iter()
        .map(|l| {
            let carried = if l.reachable {
                None
            } else {
                previous.and_then(|p| p.iter().find(|s| s.name == l.name)).cloned()
            };
            carried.unwrap_or_else(|| Snap {
                name: l.name.clone(),
                on: l.on,
                brightness: l.brightness,
                kelvin: l.kelvin,
            })
        })
        .collect();
    snap.sort_by(|a, b| a.name.cmp(&b.name));
    Some(snap)
}

pub fn restore(snap: &[Snap]) {
    let cmds: Vec<Vec<String>> = snap
        .iter()
        .map(|s| {
            vec![
                "elgatoctl".into(),
                "set".into(),
                if s.on { "--on".into() } else { "--off".into() },
                "--brightness".into(),
                s.brightness.to_string(),
                "--temp".into(),
                s.kelvin.to_string(),
                s.name.clone(),
            ]
        })
        .collect();
    sh::run_all(&cmds);
}

/// Averages brightness and temperature across reachable lights and pushes the
/// result to all of them in one call.
/// Remember the lights exactly as they are, so a session can be put back to a
/// known starting point without hunting through the undo history.
pub fn save_default() -> Result<usize, String> {
    let lights = read();
    // An unreachable light has no state to record, and snapshot() would store a
    // placeholder of off/0/0. Restoring that later would switch the light off
    // and drive it to minimum, so refuse rather than save something destructive.
    let missing: Vec<&str> = lights
        .iter()
        .filter(|l| !l.reachable)
        .map(|l| l.name.as_str())
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "not saving a default while these lights are unreachable: {}",
            missing.join(", ")
        ));
    }
    let snap = snapshot(&lights, None).ok_or("no reachable lights to save")?;
    let count = snap.len();
    crate::state::write_state_checked(crate::state::LIGHTS_DEFAULT, &snap)
        .map_err(|e| format!("could not write the default: {e}"))?;
    Ok(count)
}

/// Put every light back to the saved default.
pub fn restore_default() -> Result<usize, String> {
    let snap: Vec<Snap> = crate::state::read_state(crate::state::LIGHTS_DEFAULT)
        .ok_or("no default saved yet - run: elgato-panel save-default")?;
    if snap.is_empty() {
        return Err("the saved default is empty".into());
    }
    let count = snap.len();
    restore(&snap);
    Ok(count)
}

/// Whether a default exists, for the panel to enable or grey out its button.
pub fn has_default() -> bool {
    crate::state::read_state::<Vec<Snap>>(crate::state::LIGHTS_DEFAULT)
        .is_some_and(|s| !s.is_empty())
}

pub fn sync() {
    let live: Vec<Light> = read().into_iter().filter(|l| l.reachable).collect();
    if live.is_empty() {
        return;
    }
    let n = live.len() as u32;
    let brightness =
        ((live.iter().map(|l| l.brightness as u32).sum::<u32>() as f64 / n as f64).round() as u32).min(100);
    let kelvin = (live.iter().map(|l| l.kelvin).sum::<u32>() as f64 / n as f64 / 50.0).round() as u32 * 50;
    sh::run(&[
        "elgatoctl",
        "set",
        "--brightness",
        &brightness.to_string(),
        "--temp",
        &kelvin.to_string(),
    ]);
}
