use std::time::Duration;

use crate::cli::{Cmd, Corner};
use crate::config::{self, Config};
use crate::positioning::{self, placement};
use crate::obs;
use crate::reset;
use crate::status::{self, CamState};
use crate::{hypr, overlay, state};

pub fn run(cmd: Cmd) -> i32 {
    let cfg = config::load();
    match cmd {
        Cmd::Show => cmd_show(&cfg, None),
        Cmd::Hide => cmd_hide(),
        Cmd::Toggle => {
            if overlay::is_running() { cmd_hide() } else { cmd_show(&cfg, None) }
        }
        Cmd::Move { corner } => cmd_move(&cfg, corner),
        Cmd::Place { geometry } => cmd_place(&cfg, &geometry),
        Cmd::Pick => cmd_pick(&cfg),
        Cmd::Full => cmd_full(&cfg),
        Cmd::Status => cmd_status(&cfg),
        Cmd::Pause => {
            state::write_atomic(&state::pause_flag(), "1").ok();
            let _ = std::process::Command::new("omarchy-shell")
                .args(["notifications", "dismissAll"]).status();
            0
        }
        Cmd::Resume => {
            state::remove(&state::pause_flag());
            0
        }
        Cmd::Reset => cmd_reset(),
    }
}

fn cmd_reset() -> i32 {
    if overlay::is_running() {
        overlay::kill_running();
    }
    match reset::reset() {
        Ok(p) => {
            state::remove(&state::needs_reset_flag());
            notify(&format!("Cam Link reset: {}", p.display()), false);
            0
        }
        Err(e) => {
            notify(&format!("Cam Link reset failed: {e}"), true);
            eprintln!("camctl: reset failed: {e}");
            1
        }
    }
}

fn cmd_show(cfg: &Config, override_position: Option<&str>) -> i32 {
    if overlay::is_running() {
        if let Some(pos) = override_position {
            persist_position(pos);
            place_now(cfg);
        } else {
            place_now(cfg);
        }
        return 0;
    }

    if overlay::find_device(&cfg.device_pattern).is_none() {
        notify("Cam Link 4K not connected", false);
        return 1;
    }

    if let Some(pos) = override_position {
        persist_position(pos);
        state::remove(&state::fullscreen_flag());
    }

    if state::exists(&state::needs_reset_flag()) {
        if let Err(e) = reset::reset() {
            eprintln!("camctl: pre-show reset failed: {e}");
        } else {
            state::remove(&state::needs_reset_flag());
        }
    }

    let pid = match overlay::spawn(cfg) {
        Ok(p) => p,
        Err(e) => {
            notify(&format!("mpv spawn failed: {e}"), false);
            return 1;
        }
    };

    match overlay::wait_for_window(&cfg.window_title, pid, Duration::from_secs(4)) {
        overlay::WaitResult::Mapped(_addr) => {
            place_now(cfg);
            0
        }
        overlay::WaitResult::Died => {
            notify("Cam Link 4K receiving no HDMI signal. Power on the source and try again.", false);
            1
        }
        overlay::WaitResult::Timeout => {
            notify("Camera not outputting frames yet. mpv is waiting; overlay will appear when signal arrives.", false);
            0
        }
    }
}

fn cmd_hide() -> i32 {
    overlay::kill_running();
    state::remove(&state::fullscreen_flag());
    state::write_atomic(&state::needs_reset_flag(), "1").ok();
    0
}

fn cmd_move(cfg: &Config, corner: Corner) -> i32 {
    let pos = config::corner_to_position(corner);
    persist_position(pos);
    state::remove(&state::fullscreen_flag());
    if !overlay::is_running() {
        return cmd_show(cfg, Some(pos));
    }
    place_now(cfg)
}

fn cmd_place(cfg: &Config, geometry: &str) -> i32 {
    let Some(rect) = positioning::rect_from_geometry(geometry) else {
        eprintln!("camctl: expected \"X,Y WxH\", got {geometry:?}");
        return 2;
    };
    persist_position(&rect);
    state::remove(&state::fullscreen_flag());
    if !overlay::is_running() {
        return cmd_show(cfg, Some(&rect));
    }
    place_now(cfg)
}

fn cmd_pick(cfg: &Config) -> i32 {
    let picked = std::process::Command::new("omarchy-capture-region")
        .arg("region")
        .output();
    match picked {
        Ok(out) if out.status.success() => {
            let geometry = String::from_utf8_lossy(&out.stdout).trim().to_string();
            cmd_place(cfg, &geometry)
        }
        Ok(_) => 1,
        Err(e) => {
            eprintln!("camctl: region picker: {e}");
            1
        }
    }
}

fn cmd_full(cfg: &Config) -> i32 {
    if !overlay::is_running()
        && cmd_show(cfg, None) != 0 { return 1; }
    if state::exists(&state::fullscreen_flag()) {
        state::remove(&state::fullscreen_flag());
    } else {
        state::write_atomic(&state::fullscreen_flag(), "1").ok();
    }
    place_now(cfg)
}

fn cmd_status(cfg: &Config) -> i32 {
    let s = status::detect(cfg);
    let last = state::read(&state::last_state_file()).unwrap_or_default();
    let (alt, class, tooltip) = match &s {
        CamState::On(why)        => ("on", "on", format!("Camera ON ({why})")),
        CamState::Off(_)         => ("off", "off", "Camera connected, idle".to_string()),
        CamState::Disconnected   => ("disconnected", "disconnected", "Cam Link 4K not detected".into()),
        CamState::Disabled       => ("disabled", "disabled", "Camera monitor paused. Click to resume.".into()),
    };

    let new_state = if matches!(s, CamState::On(_)) { "on" } else { "off" };
    if matches!(s, CamState::On(_)) && last != "on" {
        state::write_atomic(&state::last_state_file(), new_state).ok();
        notify_critical(&tooltip);
    } else if !matches!(s, CamState::On(_)) && last == "on" {
        state::write_atomic(&state::last_state_file(), new_state).ok();
        notify_off();
    } else {
        if last != new_state {
            state::write_atomic(&state::last_state_file(), new_state).ok();
        }
    }

    let json = serde_json::json!({
        "text": "",
        "alt": alt,
        "class": class,
        "tooltip": tooltip,
    });
    println!("{}", json);
    0
}

fn place_now(cfg: &Config) -> i32 {
    let pos = state::read(&state::position_file()).unwrap_or_else(|| cfg.position.clone());
    let full = state::exists(&state::fullscreen_flag());

    let win = match hypr::find_window(&cfg.window_title) {
        Ok(Some(w)) => w,
        Ok(None) => return 1,
        Err(e) => { eprintln!("camctl: {e}"); return 1; }
    };

    let mon = match hypr::focused_monitor() {
        Ok(m) => m,
        Err(_) => match hypr::monitor_for_address(&win.address) {
            Ok(Some(m)) => m,
            _ => return 1,
        },
    };

    let obs_region = if cfg.obs_aware {
        obs::scene_path(cfg.obs_scene_path.as_deref())
            .and_then(|p| obs::find_region_for_monitor(&p, &mon))
    } else {
        None
    };

    let p = placement(cfg, &mon, &pos, full, obs_region);
    let _ = hypr::resize_window_pixel(&win.address, p.w, p.h);
    let _ = hypr::move_window_pixel(&win.address, p.x, p.y);
    let _ = hypr::pin_window(&win.address);
    0
}

fn persist_position(pos: &str) {
    state::write_atomic(&state::position_file(), pos).ok();
}

fn notify(msg: &str, _critical: bool) {
    let _ = std::process::Command::new("notify-send")
        .args(["-u", "low", "Camera overlay", msg])
        .status();
}

fn notify_critical(msg: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["-u", "critical", "-t", "0",
               "-h", "string:synchronous:camctl",
               "Camera is ON", msg])
        .status();
}

fn notify_off() {
    let _ = std::process::Command::new("notify-send")
        .args(["-u", "low", "-t", "2000",
               "-h", "string:synchronous:camctl",
               "Camera off"])
        .status();
}
