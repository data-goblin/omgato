use crate::config::{Cache, Light, MAX_LIGHTS, MAX_MAC_BYTES};
use crate::light;
use serde::Deserialize;
use std::collections::HashMap;
use std::io::Read;
use std::net::Ipv4Addr;
use std::process::{Command, Stdio};
use std::time::Duration;

const MAX_DISCOVERY_OUTPUT_BYTES: usize = 256 * 1024;
const MAX_ARP_OUTPUT_BYTES: usize = 64 * 1024;
const MAX_ARP_ENTRIES: usize = 256;
const MAX_HOST_BYTES: usize = 253;

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
        if let Some(v) = tok.trim().strip_prefix("id=")
            && valid_mac(v)
        {
            return Some(v.to_ascii_uppercase());
        }
    }
    None
}

fn valid_mac(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_MAC_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || matches!(byte, b':' | b'-'))
}

fn read_bounded(reader: impl Read, max_bytes: usize, context: &str) -> Result<Vec<u8>, String> {
    let mut output = Vec::with_capacity(max_bytes.min(8192));
    reader
        .take((max_bytes + 1) as u64)
        .read_to_end(&mut output)
        .map_err(|error| format!("{context}: {error}"))?;
    if output.len() > max_bytes {
        return Err(format!("{context}: output exceeds {max_bytes} bytes"));
    }
    Ok(output)
}

fn bounded_stdout(program: &str, args: &[&str], max_bytes: usize) -> Result<Vec<u8>, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!("{program}: no stdout pipe"));
    };
    let output = match read_bounded(stdout, max_bytes, program) {
        Ok(output) => output,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let status = child.wait().map_err(|e| format!("{program}: {e}"))?;
    if !status.success() {
        return Err(format!("{program} failed with {status}"));
    }
    Ok(output)
}

fn arp_table() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let output = match bounded_stdout("ip", &["neigh"], MAX_ARP_OUTPUT_BYTES) {
        Ok(output) => output,
        _ => return map,
    };
    let Ok(output) = std::str::from_utf8(&output) else { return map };
    for line in output.lines() {
        if map.len() >= MAX_ARP_ENTRIES {
            break;
        }
        let toks: Vec<&str> = line.split_whitespace().collect();
        let ip = match toks.first() {
            Some(value) => match value.parse::<Ipv4Addr>() {
                Ok(address) => address.to_string(),
                Err(_) => continue,
            },
            _ => continue,
        };
        if let Some(idx) = toks.iter().position(|t| *t == "lladdr")
            && let Some(mac) = toks.get(idx + 1)
            && valid_mac(mac)
        {
            map.insert(mac.to_ascii_uppercase(), ip);
        }
    }
    map
}

fn browse_entries(output: &[u8]) -> Result<(HashMap<String, Entry>, usize), String> {
    let output = std::str::from_utf8(output).map_err(|_| "avahi-browse: invalid UTF-8".to_string())?;
    let mut by_host: HashMap<String, Entry> = HashMap::new();
    let mut dropped = 0;
    for line in output.lines() {
        if !line.starts_with('=') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(10, ';').collect();
        if parts.len() < 9 {
            continue;
        }
        let proto = parts[2];
        let host = parts[6];
        if host.is_empty()
            || host.len() > MAX_HOST_BYTES
            || host.chars().any(char::is_control)
        {
            continue;
        }
        let Ok(port) = parts[8].parse::<u16>() else { continue };
        if port == 0 {
            continue;
        }
        if !by_host.contains_key(host) && by_host.len() >= MAX_LIGHTS {
            dropped += 1;
            continue;
        }
        let mac = parts.get(9).and_then(|text| parse_txt_mac(text));
        let entry = by_host.entry(host.to_string()).or_insert(Entry {
            port,
            ipv4: None,
            mac: None,
        });
        if proto == "IPv4"
            && let Ok(address) = parts[7].parse::<Ipv4Addr>()
        {
            entry.ipv4 = Some(address.to_string());
        }
        if entry.mac.is_none() && mac.is_some() {
            entry.mac = mac;
        }
    }
    Ok((by_host, dropped))
}

pub fn run() -> Result<Cache, String> {
    let output = bounded_stdout(
        "avahi-browse",
        &["-r", "-p", "-t", "_elg._tcp"],
        MAX_DISCOVERY_OUTPUT_BYTES,
    )?;
    let (by_host, dropped) = browse_entries(&output)?;
    if dropped > 0 {
        eprintln!("discovery limit reached; ignored {dropped} additional service record(s)");
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
            && !candidates.contains(ip)
        {
            candidates.push(ip.clone());
        }
        let mut found: Option<(AccessoryInfo, String)> = None;
        for ip in candidates {
            let url = format!("http://{}:{}/elgato/accessory-info", ip, e.port);
            match agent.get(&url).call() {
                Ok(response) => match light::response_json::<AccessoryInfo>(response, &ip) {
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
            let candidate = Light {
                name: info.display_name,
                ip,
                port: e.port,
                mac: info.mac_address,
            };
            if let Err(reason) = candidate.validate() {
                eprintln!("skip {host}: {reason}");
                continue;
            }
            if !lights.iter().any(|light| {
                (!candidate.mac.is_empty() && light.mac.eq_ignore_ascii_case(&candidate.mac))
                    || light.ip == candidate.ip
            }) {
                lights.push(candidate);
            }
        } else {
            eprintln!("no IPv4 reachable for {host}");
        }
    }
    lights.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(Cache { lights })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn service(index: usize) -> String {
        format!(
            "=;eth0;IPv4;Light {index};_elg._tcp;local;light-{index}.local;192.0.2.{};9123;\"id=00:11:22:33:44:{index:02X}\"\n",
            index + 1
        )
    }

    #[test]
    fn caps_mdns_services() {
        let output: String = (0..MAX_LIGHTS + 8).map(service).collect();
        let (entries, dropped) = browse_entries(output.as_bytes()).unwrap();
        assert_eq!(entries.len(), MAX_LIGHTS);
        assert_eq!(dropped, 8);
    }

    #[test]
    fn rejects_oversized_discovery_output() {
        let output = vec![b'x'; MAX_DISCOVERY_OUTPUT_BYTES + 1];
        assert!(read_bounded(Cursor::new(output), MAX_DISCOVERY_OUTPUT_BYTES, "test").is_err());
    }

    #[test]
    fn rejects_unbounded_or_invalid_mdns_fields() {
        let oversized_host = "x".repeat(MAX_HOST_BYTES + 1);
        let output = format!(
            "=;eth0;IPv4;Light;_elg._tcp;local;{oversized_host};192.0.2.1;9123;\"id=00:11:22:33:44:55\"\n\
             =;eth0;IPv4;Light;_elg._tcp;local;valid.local;not-an-ip;9123;\"id=00:11:22:33:44:55\"\n"
        );
        let (entries, _) = browse_entries(output.as_bytes()).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries["valid.local"].ipv4.is_none());
    }
}
