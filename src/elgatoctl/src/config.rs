use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Cache {
    #[serde(default)]
    pub lights: Vec<Light>,
}

pub fn cache_path() -> PathBuf {
    dirs::config_dir()
        .expect("no config dir")
        .join("elgatoctl/lights.toml")
}

pub fn load() -> Cache {
    let path = cache_path();
    match fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => Cache::default(),
    }
}

pub fn save(cache: &Cache) -> std::io::Result<()> {
    let path = cache_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let s = toml::to_string_pretty(cache).expect("serialize");
    // Written through a temporary so a reader mid-write cannot see a short file
    // and conclude a light has disappeared.
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    fs::write(&tmp, s)?;
    fs::rename(&tmp, &path)
}

/// Resolves a target to lights. An exact name, address or MAC wins outright, so
/// a light called "Key" can be addressed even when "Key Light" also exists;
/// substring matching is only the fallback.
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
