use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::cli::Corner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_position")]
    pub position: String,
    #[serde(default = "default_size")]
    pub size: [u32; 2],
    #[serde(default = "default_margin")]
    pub margin: u32,
    #[serde(default = "default_capture_size")]
    pub capture_size: [u32; 2],
    #[serde(default = "default_framerate")]
    pub framerate: u32,
    #[serde(default = "default_device_pattern")]
    pub device_pattern: String,
    #[serde(default = "default_window_title")]
    pub window_title: String,
    /// When true, corner placement and fullscreen on a monitor that matches
    /// an OBS screen-capture source are constrained to OBS's cropped region.
    #[serde(default = "default_obs_aware")]
    pub obs_aware: bool,
    /// Optional override for the OBS scene JSON path. If None, the most
    /// recently modified `~/.config/obs-studio/basic/scenes/*.json` is used.
    #[serde(default)]
    pub obs_scene_path: Option<String>,
}

fn default_position() -> String { "bottom-right".into() }
fn default_size() -> [u32; 2] { [320, 180] }
fn default_margin() -> u32 { 18 }
fn default_capture_size() -> [u32; 2] { [1280, 720] }
fn default_framerate() -> u32 { 30 }
fn default_device_pattern() -> String { "Cam_Link_4K".into() }
fn default_window_title() -> String { "cam-overlay".into() }
fn default_obs_aware() -> bool { false }

impl Default for Config {
    fn default() -> Self {
        Self {
            position: default_position(),
            size: default_size(),
            margin: default_margin(),
            capture_size: default_capture_size(),
            framerate: default_framerate(),
            device_pattern: default_device_pattern(),
            window_title: default_window_title(),
            obs_aware: default_obs_aware(),
            obs_scene_path: None,
        }
    }
}

pub fn config_path() -> PathBuf {
    dirs::config_dir().expect("no config dir").join("camctl/config.toml")
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
            eprintln!("camctl: config parse error: {e} - using defaults");
            Config::default()
        }),
        Err(_) => Config::default(),
    }
}

pub fn corner_to_position(c: Corner) -> &'static str {
    match c {
        Corner::Tl => "top-left",
        Corner::Tr => "top-right",
        Corner::Bl => "bottom-left",
        Corner::Br => "bottom-right",
    }
}
