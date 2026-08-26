use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
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
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let mut file = fs::OpenOptions::new().create_new(true).write(true).open(&tmp)?;
    file.write_all(s.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, &path)
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
