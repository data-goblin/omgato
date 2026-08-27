use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Read, Write};
use std::net::Ipv4Addr;
use std::path::PathBuf;

pub const MAX_LIGHTS: usize = 32;
pub const MAX_NAME_BYTES: usize = 128;
pub const MAX_MAC_BYTES: usize = 64;
const MAX_CACHE_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Light {
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub mac: String,
}

impl Light {
    pub fn url(&self, path: &str) -> String {
        format!("http://{}:{}{}", self.ip, self.port, path)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.is_empty() || self.name.len() > MAX_NAME_BYTES {
            return Err("invalid display name length");
        }
        if self.name.chars().any(char::is_control) {
            return Err("display name contains control characters");
        }
        if self.ip.parse::<Ipv4Addr>().is_err() {
            return Err("invalid IPv4 address");
        }
        if self.port == 0 {
            return Err("invalid port");
        }
        if self.mac.len() > MAX_MAC_BYTES
            || !self
                .mac
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || matches!(byte, b':' | b'-'))
        {
            return Err("invalid MAC address");
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub lights: Vec<Light>,
}

pub fn cache_path() -> PathBuf {
    let config = dirs::config_dir().expect("no config dir");
    let path = config.join("keylight-ctl/lights.toml");
    let legacy = config.join("elgatoctl/lights.toml");
    migrate_file(&legacy, &path);
    path
}

fn migrate_file(from: &std::path::Path, to: &std::path::Path) {
    if fs::symlink_metadata(to).is_ok()
        || !fs::symlink_metadata(from).is_ok_and(|meta| meta.file_type().is_file())
    {
        return;
    }
    let Some(parent) = to.parent() else { return };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let _ = fs::rename(from, to);
}

pub fn load() -> Cache {
    let path = cache_path();
    match read_cache(&path) {
        Ok(s) => bounded(toml::from_str(&s).unwrap_or_default()),
        Err(_) => Cache::default(),
    }
}

pub fn save(cache: &Cache) -> std::io::Result<()> {
    validate_cache(cache)?;
    let path = cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cache).expect("serialize");
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(&tmp)?;
    file.write_all(s.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, &path)
}

fn read_cache(path: &std::path::Path) -> io::Result<String> {
    let file = fs::File::open(path)?;
    if file.metadata()?.len() > MAX_CACHE_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "light cache is too large"));
    }
    let mut contents = String::new();
    file.take(MAX_CACHE_BYTES + 1).read_to_string(&mut contents)?;
    if contents.len() as u64 > MAX_CACHE_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "light cache is too large"));
    }
    Ok(contents)
}

fn bounded(mut cache: Cache) -> Cache {
    cache.lights.retain(|light| light.validate().is_ok());
    cache.lights.truncate(MAX_LIGHTS);
    cache
}

fn validate_cache(cache: &Cache) -> io::Result<()> {
    if cache.lights.len() > MAX_LIGHTS {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "too many lights"));
    }
    for light in &cache.lights {
        light
            .validate()
            .map_err(|reason| io::Error::new(io::ErrorKind::InvalidInput, reason))?;
    }
    Ok(())
}

pub fn select<'a>(cache: &'a Cache, target: &str) -> Vec<&'a Light> {
    if target == "all" {
        return cache.lights.iter().collect();
    }
    let needle = target.to_lowercase();
    let exact: Vec<&Light> = cache
        .lights
        .iter()
        .filter(|l| {
            l.name.to_lowercase() == needle
                || l.ip == target
                || l.mac.eq_ignore_ascii_case(target)
        })
        .collect();
    if !exact.is_empty() {
        return exact;
    }
    cache
        .lights
        .iter()
        .filter(|l| l.name.to_lowercase().contains(&needle))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn light(index: usize) -> Light {
        Light {
            name: format!("Light {index}"),
            ip: format!("192.0.2.{}", index + 1),
            port: 9123,
            mac: format!("00:11:22:33:44:{index:02X}"),
        }
    }

    #[test]
    fn bounds_cached_device_state() {
        let cache = bounded(Cache { lights: (0..MAX_LIGHTS + 8).map(light).collect() });
        assert_eq!(cache.lights.len(), MAX_LIGHTS);
    }

    #[test]
    fn rejects_oversized_or_non_ipv4_fields() {
        let mut candidate = light(0);
        candidate.name = "x".repeat(MAX_NAME_BYTES + 1);
        assert!(candidate.validate().is_err());

        candidate = light(0);
        candidate.ip = "not-an-address".into();
        assert!(candidate.validate().is_err());

        candidate = light(0);
        candidate.mac = "not a mac".into();
        assert!(candidate.validate().is_err());
    }
}
