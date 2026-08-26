mod deck;
mod pedal;

pub use deck::{Button, DeckConfig, Page};
pub use pedal::{Gesture, PedalActions, PedalConfig, PedalPos};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    #[serde(default)]
    pub pedal: PedalConfig,
    #[serde(default)]
    pub deck: DeckConfig,
}

pub fn config_path() -> Result<PathBuf> {
    let dir = dirs::config_dir()
        .context("no config dir")?
        .join("streamdeck-ctl");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join("config.toml"))
}

pub fn load() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        let cfg = Config::default();
        std::fs::write(&path, toml::to_string_pretty(&cfg)?)?;
        return Ok(cfg);
    }
    let s = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&s)?)
}

pub fn save(cfg: &Config) -> Result<()> {
    use std::io::Write;
    let path = config_path()?;
    let serialized = toml::to_string_pretty(cfg)?;
    // The pid keeps two concurrent saves off one temporary file, and exclusive
    // creation after unlinking stops a symlink there redirecting the write.
    let tmp = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    {
        let _ = std::fs::remove_file(&tmp);
        let mut f = std::fs::OpenOptions::new().create_new(true).write(true).open(&tmp)?;
        f.write_all(serialized.as_bytes())?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(())
}
