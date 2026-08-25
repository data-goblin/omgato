use crate::sh;
use crate::state::{self, CAMERA_HISTORY, History};
use serde::{Deserialize, Serialize};

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
    #[serde(default)]
    position: String,
    #[serde(default)]
    paused: bool,
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
        return sh::succeeded(&["camlink-ctl", "hide"]);
    }
    match placement.position.strip_prefix("rect:") {
        Some(rect) => {
            let parts: Vec<&str> = rect.split(',').collect();
            let [x, y, w, h] = parts[..] else {
                return false;
            };
            sh::succeeded(&["camlink-ctl", "place", &format!("{x},{y} {w}x{h}")])
        }
        None if placement.position.is_empty() => {
            sh::succeeded(&["camlink-ctl", "show"])
        }
        None => {
            let corner = corner_of(&placement.position);
            if corner.is_empty() || corner == "area" {
                return false;
            }
            sh::succeeded(&["camlink-ctl", "move", &corner])
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
    let overlay = cam.overlay;
    let position = cam.position;

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
        paused: cam.paused,
        corner: corner_of(&position),
        position,
        history: history.flags(),
    }
}

pub fn travel(step: i64) -> Result<(), String> {
    let history: History<Placement> = History::load(CAMERA_HISTORY);
    let Some((pos, placement)) = history.seek(step) else {
        return Ok(());
    };
    if !apply(placement) {
        return Err("could not restore the camera placement".into());
    }
    history.commit_pos(CAMERA_HISTORY, pos);
    Ok(())
}
