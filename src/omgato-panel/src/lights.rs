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
    let rows: Vec<Row> = serde_json::from_str(&sh::run(&["keylight-ctl", "--json", "ls"])).unwrap_or_default();
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
        .filter_map(|l| {
            if !l.reachable {
                return previous
                    .and_then(|p| {
                        p.iter().find(|s| {
                            if s.ip.is_empty() {
                                s.name == l.name
                            } else {
                                s.ip == l.ip
                            }
                        })
                    })
                    .cloned();
            }
            Some(Snap {
                name: l.name.clone(),
                ip: l.ip.clone(),
                on: l.on,
                brightness: l.brightness,
                kelvin: l.kelvin,
            })
        })
        .collect();
    snap.sort_by(|a, b| a.ip.cmp(&b.ip).then_with(|| a.name.cmp(&b.name)));
    Some(snap)
}

pub fn restore(snap: &[Snap]) -> Result<(), String> {
    let live = snap.iter().any(|s| s.ip.is_empty()).then(read);
    let targets: Result<Vec<String>, String> = snap
        .iter()
        .map(|s| {
            if !s.ip.is_empty() {
                return Ok(s.ip.clone());
            }
            let matches: Vec<&Light> = live
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter(|l| l.name == s.name)
                .collect();
            match matches.as_slice() {
                [light] => Ok(light.ip.clone()),
                [] => Err(format!("the saved light {:?} is no longer configured", s.name)),
                _ => Err(format!(
                    "the saved light name {:?} is ambiguous; save the default again",
                    s.name
                )),
            }
        })
        .collect();
    let cmds: Vec<Vec<String>> = snap
        .iter()
        .zip(targets?)
        .map(|(s, target)| {
            vec![
                "keylight-ctl".into(),
                "set".into(),
                if s.on { "--on".into() } else { "--off".into() },
                "--brightness".into(),
                s.brightness.to_string(),
                "--temp".into(),
                s.kelvin.to_string(),
                target,
            ]
        })
        .collect();
    let failed: Vec<&str> = sh::succeed_all(&cmds)
        .into_iter()
        .zip(snap)
        .filter_map(|(ok, light)| (!ok).then_some(light.name.as_str()))
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("could not restore: {}", failed.join(", ")))
    }
}

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
        .ok_or("no default saved yet - run: omgato-panel save-default")?;
    if snap.is_empty() {
        return Err("the saved default is empty".into());
    }
    let count = snap.len();
    restore(&snap)?;
    Ok(count)
}

/// Whether a default exists, for the panel to enable or grey out its button.
pub fn has_default() -> bool {
    crate::state::read_state::<Vec<Snap>>(crate::state::LIGHTS_DEFAULT)
        .is_some_and(|s| !s.is_empty())
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
        "keylight-ctl",
        "set",
        "--brightness",
        &brightness.to_string(),
        "--temp",
        &kelvin.to_string(),
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(ip: &str, reachable: bool, brightness: u8) -> Light {
        Light {
            name: "Key Light".into(),
            display: "Key Light".into(),
            ip: ip.into(),
            reachable,
            on: true,
            brightness,
            kelvin: 4000,
        }
    }

    #[test]
    fn snapshots_identically_named_lights_by_address() {
        let snap = snapshot(&[light("10.0.0.2", true, 20), light("10.0.0.3", true, 80)], None)
            .unwrap();
        assert_eq!(snap[0].ip, "10.0.0.2");
        assert_eq!(snap[0].brightness, 20);
        assert_eq!(snap[1].ip, "10.0.0.3");
        assert_eq!(snap[1].brightness, 80);

        let carried = snapshot(&[light("10.0.0.2", false, 0), light("10.0.0.3", true, 70)], Some(&snap))
            .unwrap();
        assert_eq!(carried[0].brightness, 20);
        assert_eq!(carried[1].brightness, 70);
    }
}
