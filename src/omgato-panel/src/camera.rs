use crate::sh;
use crate::state::{self, CAMERA_HISTORY, History};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const PAUSE_FLAG: &str = "camlink-ctl/pause";

#[derive(Default, Deserialize)]
struct CamOut {
    #[serde(default)]
    alt: String,
    #[serde(default)]
    tooltip: String,
    /// Whether the overlay window is up. Distinct from `alt`, which reports
    /// whether the capture device is streaming.
    #[serde(default)]
    overlay: bool,
}

/// The slice of overlay state the panel can step through: whether it is up, and
/// camlink-ctl's own placement string, which is either a corner or "rect:X,Y,W,H".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub overlay: bool,
    pub position: String,
}

#[derive(Serialize)]
pub struct Status {
    pub state: String,
    pub tooltip: String,
    pub paused: bool,
    pub overlay: bool,
    pub position: String,
    pub corner: String,
    pub history: state::Flags,
}

fn position_file() -> PathBuf {
    let runtime = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_owned());
    PathBuf::from(runtime).join("camlink-ctl/position")
}

/// camlink-ctl's placement, verbatim, so hotkey moves show up here too.
pub fn position() -> String {
    std::fs::read_to_string(position_file())
        .map(|s| s.trim().to_owned())
        .unwrap_or_default()
}

/// Short code the panel highlights with: a corner, or "area" for a free rectangle.
fn corner_of(position: &str) -> String {
    match position {
        "top-left" => "tl",
        "top-right" => "tr",
        "bottom-left" => "bl",
        "bottom-right" => "br",
        p if p.starts_with("rect:") => "area",
        _ => "",
    }
    .to_owned()
}

/// Returns false when the step could not be carried out, so the caller can
/// leave the history pointer where it was rather than losing a step to nothing.
fn apply(placement: &Placement) -> bool {
    if !placement.overlay {
        sh::run(&["camlink-ctl", "hide"]);
        return true;
    }
    match placement.position.strip_prefix("rect:") {
        Some(rect) => {
            let parts: Vec<&str> = rect.split(',').collect();
            let [x, y, w, h] = parts[..] else {
                return false;
            };
            sh::run(&["camlink-ctl", "place", &format!("{x},{y} {w}x{h}")]);
            true
        }
        None if placement.position.is_empty() => {
            sh::run(&["camlink-ctl", "show"]);
            true
        }
        None => {
            let corner = corner_of(&placement.position);
            if corner.is_empty() || corner == "area" {
                return false;
            }
            sh::run(&["camlink-ctl", "move", &corner]);
            true
        }
    }
}

/// One compact phrase for the panel, since camlink-ctl's tooltip repeats the state
/// word it is shown next to.
fn detail(alt: &str, tooltip: &str) -> String {
    match alt {
        "disconnected" => "not detected".to_owned(),
        "disabled" => "monitor paused".to_owned(),
        _ => tooltip.strip_prefix("Camera ").unwrap_or(tooltip).to_owned(),
    }
}

pub fn status() -> Status {
    let cam: CamOut = serde_json::from_str(&sh::run(&["camlink-ctl", "status"])).unwrap_or_default();
    let paused = dirs::config_dir()
        .map(|d| d.join(PAUSE_FLAG).exists())
        .unwrap_or(false);
    let overlay = cam.overlay;
    let position = position();

    let mut history: History<Placement> = History::load(CAMERA_HISTORY);
    history.fold(
        CAMERA_HISTORY,
        Placement { overlay, position: position.clone() },
    );

    let detail = detail(&cam.alt, &cam.tooltip);
    Status {
        overlay,
        state: if cam.alt.is_empty() { "unknown".to_owned() } else { cam.alt },
        tooltip: detail,
        paused,
        corner: corner_of(&position),
        position,
        history: history.flags(),
    }
}

pub fn travel(step: i64) {
    let history: History<Placement> = History::load(CAMERA_HISTORY);
    let Some((pos, placement)) = history.seek(step) else {
        return;
    };
    if !apply(placement) {
        return;
    }
    history.commit_pos(CAMERA_HISTORY, pos);
}
