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

pub fn snapshot(lights: &[Light]) -> Option<Vec<Snap>> {
    if lights.is_empty() || lights.iter().any(|l| !l.reachable) {
        return None;
    }
    let mut snap: Vec<Snap> = lights
        .iter()
        .map(|l| Snap {
            name: l.name.clone(),
            on: l.on,
            brightness: l.brightness,
            kelvin: l.kelvin,
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
