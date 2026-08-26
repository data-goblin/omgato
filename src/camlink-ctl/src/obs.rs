use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::hypr::Monitor;

#[derive(Debug, Clone, Copy)]
pub struct Region {
    /// Logical desktop coordinates (matches Hyprland's monitor.x/y space).
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Top-level OBS scene JSON we care about.
#[derive(Deserialize)]
struct Scene {
    #[serde(default)]
    sources: Vec<Source>,
}

#[derive(Deserialize)]
struct Source {
    #[serde(default)]
    id: String,
    #[serde(default)]
    settings: SourceSettings,
}

#[derive(Deserialize, Default)]
struct SourceSettings {
    /// Present on the synthetic `scene` source - holds the scene items.
    #[serde(default)]
    items: Vec<SceneItem>,
}

#[derive(Deserialize)]
struct SceneItem {
    #[serde(default)]
    scale_ref: Xy,
    #[serde(default)]
    crop_left: i32,
    #[serde(default)]
    crop_top: i32,
    #[serde(default)]
    crop_right: i32,
    #[serde(default)]
    crop_bottom: i32,
}

#[derive(Deserialize, Default, Clone, Copy)]
struct Xy {
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
}

/// Resolve the OBS scene JSON path. If `override_path` is provided use it
/// verbatim; otherwise pick the most recently modified `*.json` in
/// `~/.config/obs-studio/basic/scenes/`.
pub fn scene_path(override_path: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = override_path {
        return Some(expand(p));
    }
    let dir = dirs::config_dir()?
        .join("obs-studio")
        .join("basic")
        .join("scenes");
    let mut best: Option<(PathBuf, std::time::SystemTime)> = None;
    for e in fs::read_dir(&dir).ok()?.flatten() {
        let p = e.path();
        if p.extension().and_then(|s| s.to_str()) != Some("json") { continue; }
        let Ok(mt) = e.metadata().and_then(|m| m.modified()) else { continue };
        if best.as_ref().is_none_or(|(_, t)| mt > *t) {
            best = Some((p, mt));
        }
    }
    best.map(|(p, _)| p)
}

fn expand(s: &str) -> PathBuf {
    if let Some(rest) = s.strip_prefix("~/")
        && let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    PathBuf::from(s)
}

/// Find an OBS scene item whose source resolution (`scale_ref`) matches the
/// monitor's physical pixel size. Returns the cropped recording region as
/// logical desktop coordinates.
pub fn find_region_for_monitor(scene_file: &Path, mon: &Monitor) -> Option<Region> {
    let raw = fs::read_to_string(scene_file).ok()?;
    let scene: Scene = serde_json::from_str(&raw).ok()?;

    let scene_source = scene.sources.iter().find(|s| s.id == "scene")?;
    let mw = mon.width as f32;
    let mh = mon.height as f32;
    let mut matches = scene_source.settings.items.iter().filter(|it| {
        (it.scale_ref.x - mw).abs() < 1.0 && (it.scale_ref.y - mh).abs() < 1.0
    });
    let item = matches.next()?;
    if matches.next().is_some() {
        return None;
    }

    let scale = if mon.scale > 0.0 { mon.scale } else { 1.0 };
    let src_w = item.scale_ref.x as i32;
    let src_h = item.scale_ref.y as i32;
    let left = item.crop_left;
    let top = item.crop_top;
    let right = src_w - item.crop_right;
    let bottom = src_h - item.crop_bottom;

    let to_log = |p: i32| (p as f32 / scale) as i32;
    let x0 = mon.x + to_log(left);
    let y0 = mon.y + to_log(top);
    let x1 = mon.x + to_log(right);
    let y1 = mon.y + to_log(bottom);

    Some(Region {
        x: x0,
        y: y0,
        w: (x1 - x0).max(0),
        h: (y1 - y0).max(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor() -> Monitor {
        Monitor {
            id: 1,
            name: "DP-1".into(),
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale: 1.0,
            focused: true,
            reserved: [0; 4],
        }
    }

    #[test]
    fn rejects_ambiguous_same_resolution_sources() {
        let dir = std::env::temp_dir().join(format!("omgato-obs-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("scene.json");
        let scene = r#"{
            "sources": [{
                "id": "scene",
                "settings": {"items": [
                    {"scale_ref": {"x": 1920, "y": 1080}},
                    {"scale_ref": {"x": 1920, "y": 1080}, "crop_left": 100}
                ]}
            }]
        }"#;
        fs::write(&path, scene).unwrap();
        assert!(find_region_for_monitor(&path, &monitor()).is_none());
        fs::remove_dir_all(dir).unwrap();
    }
}
