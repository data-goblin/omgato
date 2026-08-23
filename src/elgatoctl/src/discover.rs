use crate::config::{Cache, Light};
use serde::Deserialize;
use std::collections::HashMap;
use std::process::Command;
use std::time::Duration;

#[derive(Deserialize)]
struct AccessoryInfo {
    #[serde(rename = "displayName")]
    display_name: String,
    #[serde(rename = "macAddress")]
    mac_address: String,
}

struct Entry {
    port: u16,
    ipv4: Option<String>,
    mac: Option<String>,
}

fn parse_txt_mac(rest: &str) -> Option<String> {
    for tok in rest.split('"') {
        if let Some(v) = tok.trim().strip_prefix("id=") {
            return Some(v.to_uppercase());
        }
    }
    None
}

fn arp_table() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let out = match Command::new("ip").args(["neigh"]).output() {
        Ok(o) if o.status.success() => o,
        _ => return map,
    };
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let toks: Vec<&str> = line.split_whitespace().collect();
        let ip = match toks.first() {
            Some(s) if !s.contains(':') => s.to_string(),
            _ => continue,
        };
        if let Some(idx) = toks.iter().position(|t| *t == "lladdr")
            && let Some(mac) = toks.get(idx + 1) {
                map.insert(mac.to_uppercase(), ip);
            }
    }
    map
}

pub fn run() -> Result<Cache, String> {
    let out = Command::new("avahi-browse")
        .args(["-r", "-p", "-t", "_elg._tcp"])
        .output()
        .map_err(|e| format!("avahi-browse: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "avahi-browse failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);

    let mut by_host: HashMap<String, Entry> = HashMap::new();
    for line in stdout.lines() {
        if !line.starts_with('=') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(10, ';').collect();
        if parts.len() < 9 {
            continue;
        }
        let proto = parts[2];
        let host = parts[6].to_string();
        let ip = parts[7].to_string();
        let port: u16 = parts[8].parse().unwrap_or(9123);
        let mac = parts.get(9).and_then(|t| parse_txt_mac(t));
        let entry = by_host.entry(host).or_insert(Entry {
            port,
            ipv4: None,
            mac: None,
        });
        if proto == "IPv4" {
            entry.ipv4 = Some(ip);
        }
        if entry.mac.is_none() && mac.is_some() {
            entry.mac = mac;
        }
    }

    let need_arp = by_host
        .values()
        .any(|e| e.ipv4.is_none() && e.mac.is_some());
    let arp = if need_arp { arp_table() } else { HashMap::new() };

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_millis(2000))
        .build();

    let mut lights: Vec<Light> = Vec::new();
    for (host, e) in by_host {
        let mut candidates: Vec<String> = Vec::new();
        if let Some(ip) = &e.ipv4 {
            candidates.push(ip.clone());
        }
        if let Some(mac) = &e.mac
            && let Some(ip) = arp.get(mac)
                && !candidates.contains(ip) {
                    candidates.push(ip.clone());
                }
        let mut found: Option<(AccessoryInfo, String)> = None;
        for ip in candidates {
            let url = format!("http://{}:{}/elgato/accessory-info", ip, e.port);
            match agent.get(&url).call() {
                Ok(r) => match r.into_json::<AccessoryInfo>() {
                    Ok(info) => {
                        found = Some((info, ip));
                        break;
                    }
                    Err(err) => eprintln!("skip {ip}: parse: {err}"),
                },
                Err(err) => eprintln!("skip {ip}: {err}"),
            }
        }
        if let Some((info, ip)) = found {
            lights.push(Light {
                name: info.display_name,
                ip,
                port: e.port,
                mac: info.mac_address,
            });
        } else {
            eprintln!("no IPv4 reachable for {host}");
        }
    }
    lights.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Cache { lights })
}
