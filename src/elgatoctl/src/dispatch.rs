use crate::cli::Cmd;
use crate::config::{self, Cache, Light};
use crate::light::{self, LightPatch, LightState};
use crate::{discover, setup, waybar};
use serde::Serialize;

#[derive(Serialize)]
struct Row {
    name: String,
    ip: String,
    reachable: bool,
    on: bool,
    brightness: u8,
    kelvin: u32,
}

pub struct Out {
    pub json: bool,
}

impl Out {
    fn emit(&self, lights: &[Light], states: &[Result<LightState, String>]) {
        if self.json {
            let rows: Vec<Row> = lights
                .iter()
                .zip(states)
                .map(|(l, s)| match s {
                    Ok(s) => Row {
                        name: l.name.clone(),
                        ip: l.ip.clone(),
                        reachable: true,
                        on: s.on == 1,
                        brightness: s.brightness,
                        kelvin: light::mired_to_kelvin(s.temperature),
                    },
                    Err(_) => Row {
                        name: l.name.clone(),
                        ip: l.ip.clone(),
                        reachable: false,
                        on: false,
                        brightness: 0,
                        kelvin: 0,
                    },
                })
                .collect();
            println!("{}", serde_json::to_string(&rows).unwrap_or_default());
            return;
        }
        for (l, s) in lights.iter().zip(states) {
            match s {
                Ok(s) => println!(
                    "{}\t{}\t{}\tbr={}\tk={}",
                    l.name,
                    l.ip,
                    if s.on == 1 { "on" } else { "off" },
                    s.brightness,
                    light::mired_to_kelvin(s.temperature)
                ),
                Err(e) => {
                    eprintln!("{e}");
                    println!("{}\t{}\tunreachable", l.name, l.ip);
                }
            }
        }
    }
}

pub fn run(cmd: Cmd, out: Out) -> i32 {
    match cmd {
        Cmd::Setup => setup::run(),
        Cmd::Discover { prune } => cmd_discover(prune),
        Cmd::Ls => cmd_ls(&out),
        Cmd::On { target } => apply_each(
            &target,
            LightPatch { on: Some(1), ..Default::default() },
            &out,
        ),
        Cmd::Off { target } => apply_each(
            &target,
            LightPatch { on: Some(0), ..Default::default() },
            &out,
        ),
        Cmd::Toggle { target } => cmd_toggle(&target, &out),
        Cmd::Brightness { value, target } => apply_each(
            &target,
            LightPatch { brightness: Some(value.min(100)), ..Default::default() },
            &out,
        ),
        Cmd::Temperature { kelvin, target } => apply_each(
            &target,
            LightPatch { temperature: Some(light::kelvin_to_mired(kelvin)), ..Default::default() },
            &out,
        ),
        Cmd::Set { on, off, brightness, temp, target } => {
            let patch = LightPatch {
                on: if on {
                    Some(1)
                } else if off {
                    Some(0)
                } else {
                    None
                },
                brightness: brightness.map(|b| b.min(100)),
                temperature: temp.map(light::kelvin_to_mired),
            };
            if patch.is_empty() {
                eprintln!("set: nothing to change - pass --on/--off, --brightness or --temp");
                return 2;
            }
            apply_each(&target, patch, &out)
        }
        Cmd::Waybar => {
            let cache = config::load();
            waybar::emit(&cache);
            0
        }
        Cmd::Click { target } => cmd_click(&target, &out),
    }
}

fn cmd_click(target: &str, out: &Out) -> i32 {
    let cache = config::load();
    let selected: Vec<Light> = config::select(&cache, target).into_iter().cloned().collect();
    let probes = light::probe(&selected);
    let any_unreachable = probes.is_empty() || probes.iter().any(|r| r.is_err());
    if any_unreachable {
        match discover::run() {
            Ok(c) => {
                let (merged, _) = merge(c.lights, false);
                if let Err(e) = config::save(&merged) {
                    eprintln!("save failed: {e}");
                }
                if merged.lights.is_empty() {
                    return 1;
                }
            }
            Err(e) => {
                eprintln!("discover: {e}");
                return 1;
            }
        }
        return cmd_toggle(target, out);
    }
    let any_on = probes.iter().any(|r| matches!(r, Ok(s) if s.on == 1));
    let new_on = if any_on { 0 } else { 1 };
    apply_each(target, LightPatch { on: Some(new_on), ..Default::default() }, out)
}

fn same_light(a: &Light, b: &Light) -> bool {
    if !a.mac.is_empty() && !b.mac.is_empty() {
        return a.mac.eq_ignore_ascii_case(&b.mac);
    }
    a.ip == b.ip
}

/// Folds a scan into the cache. A light that is merely asleep or off the network
/// keeps its entry, so one scan at a bad moment cannot drop it from the rig.
fn merge(found: Vec<Light>, prune: bool) -> (Cache, Vec<Light>) {
    let mut lights = found;
    let mut kept = Vec::new();
    if !prune {
        for old in config::load().lights {
            if !lights.iter().any(|l| same_light(l, &old)) {
                kept.push(old.clone());
                lights.push(old);
            }
        }
    }
    lights.sort_by(|a, b| a.name.cmp(&b.name));
    (Cache { lights }, kept)
}

fn cmd_discover(prune: bool) -> i32 {
    let found = match discover::run() {
        Ok(cache) => cache.lights,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let seen = found.len();
    let (cache, kept) = merge(found, prune);
    if let Err(e) = config::save(&cache) {
        eprintln!("save failed: {e}");
        return 1;
    }
    for l in &cache.lights {
        let note = if kept.iter().any(|k| same_light(k, l)) { "\tcached" } else { "" };
        println!("{}\t{}\t{}{}", l.name, l.ip, l.mac, note);
    }
    if cache.lights.is_empty() {
        eprintln!("no lights found");
        return 1;
    }
    if seen == 0 {
        eprintln!("scan found nothing; kept {} cached light(s)", kept.len());
    }
    0
}

fn cmd_ls(out: &Out) -> i32 {
    let cache = config::load();
    if cache.lights.is_empty() {
        eprintln!("no lights cached - run: elgatoctl discover");
        if out.json {
            println!("[]");
        }
        return 1;
    }
    let states = light::probe(&cache.lights);
    out.emit(&cache.lights, &states);
    0
}

fn select_or_die(target: &str) -> Result<Vec<Light>, i32> {
    let cache = config::load();
    let lights: Vec<Light> = config::select(&cache, target).into_iter().cloned().collect();
    if lights.is_empty() {
        eprintln!("no matching lights for '{target}' - run: elgatoctl discover");
        return Err(1);
    }
    Ok(lights)
}

fn apply_each(target: &str, patch: LightPatch, out: &Out) -> i32 {
    let lights = match select_or_die(target) {
        Ok(v) => v,
        Err(c) => {
            if out.json {
                println!("[]");
            }
            return c;
        }
    };
    let states = light::each(&lights, |l| light::apply(l, &patch));
    let failed = states.iter().filter(|r| r.is_err()).count();
    out.emit(&lights, &states);
    if failed == lights.len() {
        1
    } else {
        0
    }
}

fn cmd_toggle(target: &str, out: &Out) -> i32 {
    let lights = match select_or_die(target) {
        Ok(v) => v,
        Err(c) => {
            if out.json {
                println!("[]");
            }
            return c;
        }
    };
    let any_on = light::probe(&lights)
        .iter()
        .any(|r| matches!(r, Ok(s) if s.on == 1));
    let new_on = if any_on { 0 } else { 1 };
    apply_each(target, LightPatch { on: Some(new_on), ..Default::default() }, out)
}
