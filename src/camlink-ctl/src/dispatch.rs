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
        Cmd::Avoid { geometry, owner } => cmd_avoid(&cfg, &geometry, &owner),
        Cmd::Release { owner } => cmd_release(&cfg, &owner),
        Cmd::Replace => cmd_replace(&cfg),
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
            notify(&format!("Cam Link reset: {}", p.display()));
            0
        }
        Err(e) => {
            notify(&format!("Cam Link reset failed: {e}"));
            eprintln!("camlink-ctl: reset failed: {e}");
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
        notify("Cam Link 4K not connected");
        return 1;
    }

    if let Some(pos) = override_position {
        persist_position(pos);
        state::remove(&state::fullscreen_flag());
    }

    // The Cam Link is single-open. If a user service is sitting on it, stop that
    // service and note it down, rather than making the user work out what to run.
    // cmd_hide starts it again, so the device is always handed back.
    if let Some(dev) = overlay::find_device(&cfg.device_pattern) {
        let busy = crate::holder::holders(&dev);
        if !busy.is_empty() {
            match crate::holder::borrow(&busy, &dev) {
                Some(unit) => {
                    state::write_atomic(&state::borrowed_unit(), &unit).ok();
                    notify(&format!("Paused {unit} to use the camera"));
                }
                None => {
                    let msg = format!(
                        "camera is held by {}; {}",
                        crate::holder::describe(&busy),
                        crate::holder::remedy(&busy)
                    );
                    eprintln!("camlink-ctl: {msg}");
                    notify(&msg);
                    return 1;
                }
            }
        }
    }

    if state::exists(&state::needs_reset_flag()) {
        if let Err(e) = reset::reset() {
            eprintln!("camlink-ctl: pre-show reset failed: {e}");
        } else {
            state::remove(&state::needs_reset_flag());
        }
    }

    let pid = match overlay::spawn(cfg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("camlink-ctl: {e}");
            notify(&format!("mpv spawn failed: {e}"));
            return 1;
        }
    };

    match overlay::wait_for_window(&cfg.window_title, pid, Duration::from_secs(4)) {
        overlay::WaitResult::Mapped(_addr) => {
            place_now(cfg);
            // A bar registers its exclusive zone slightly after it maps, so a
            // placement made the instant the overlay appears can be computed
            // against a reserved area of zero and sit under the bar. Place once
            // more when the layout has settled.
            std::thread::sleep(Duration::from_millis(700));
            place_now(cfg);
            0
        }
        overlay::WaitResult::Died => {
            notify("Cam Link 4K receiving no HDMI signal. Power on the source and try again.");
            1
        }
        overlay::WaitResult::Timeout => {
            notify("Camera not outputting frames yet. mpv is waiting; overlay will appear when signal arrives.");
            0
        }
    }
}

fn cmd_hide() -> i32 {
    overlay::kill_running();
    state::remove(&state::fullscreen_flag());
    state::write_atomic(&state::needs_reset_flag(), "1").ok();
    if let Some(unit) = state::read(&state::borrowed_unit()) {
        state::remove(&state::borrowed_unit());
        crate::holder::give_back(&unit);
        notify(&format!("Resumed {unit}"));
    }
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
        eprintln!("camlink-ctl: expected \"X,Y WxH\", got {geometry:?}");
        return 2;
    };
    persist_position(&rect);
    state::remove(&state::fullscreen_flag());
    if !overlay::is_running() {
        return cmd_show(cfg, Some(&rect));
    }
    place_now(cfg)
}

/// Push the overlay out from under a panel that has just opened over it, and
/// remember where it was so `release` can put it back. Doing nothing when the
/// two do not overlap keeps a deliberate placement intact.
fn cmd_avoid(cfg: &Config, geometry: &str, owner: &str) -> i32 {
    let mon = match hypr::focused_monitor() {
        Ok(m) => m,
        Err(e) => { eprintln!("camlink-ctl: {e}"); return 1; }
    };

    let blocker = if let Some((w, h)) = positioning::size_from_text(geometry) {
        positioning::panel_rect(&mon, w, h)
    } else if let Some(pos) = positioning::rect_from_geometry(geometry) {
        match positioning::parse_rect(&pos) {
            Some(r) => r,
            None => { eprintln!("camlink-ctl: avoid: bad rectangle {geometry:?}"); return 2; }
        }
    } else {
        eprintln!("camlink-ctl: avoid: expected \"WxH\" or \"X,Y WxH\", got {geometry:?}");
        return 2;
    };

    if let Err(e) = crate::blocker::claim(owner, &blocker) {
        eprintln!("camlink-ctl: could not record the panel's claim: {e}");
        return 1;
    }
    if !overlay::is_running() {
        return 0;
    }
    place_now(cfg)
}

fn cmd_release(cfg: &Config, owner: &str) -> i32 {
    crate::blocker::release(owner);
    if !overlay::is_running() {
        return 0;
    }
    place_now(cfg)
}

/// Re-apply the current placement. The reserved area a bar claims changes when
/// the shell restarts or the display is reconfigured, and the overlay is
/// otherwise left wherever it was put.
fn cmd_replace(cfg: &Config) -> i32 {
    if !overlay::is_running() {
        return 0;
    }
    place_now(cfg)
}

fn cmd_pick(cfg: &Config) -> i32 {
    let picked = std::process::Command::new("omarchy-capture-region")
        .arg("region")
        .output();
    match picked {
        Ok(out) => {
            let geometry = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if geometry.is_empty() {
                return 0;
            }
            cmd_place(cfg, &geometry)
        }
        Err(e) => {
            eprintln!("camlink-ctl: region picker: {e}");
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
    let (alt, class, tooltip) = match &s {
        CamState::On(why)        => ("on", "on", format!("Camera ON ({why})")),
        CamState::Off(_)         => ("off", "off", "Camera connected, idle".to_string()),
        CamState::Disconnected   => ("disconnected", "disconnected", "Cam Link 4K not detected".into()),
        CamState::Disabled       => ("disabled", "disabled", "Camera monitor paused. Click to resume.".into()),
    };

    // Whether the overlay window is up is a different question from whether the
    // capture device is producing frames. A Cam Link showing "no signal" has the
    // overlay running and nothing streaming, so a caller that wants to know
    // about the window has to be told about the window.
    let json = serde_json::json!({
        "text": "",
        "alt": alt,
        "class": class,
        "tooltip": tooltip,
        "overlay": overlay::is_running(),
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
        Err(e) => { eprintln!("camlink-ctl: {e}"); return 1; }
    };

    // The monitor the overlay actually sits on, not whichever happens to have
    // focus. Placing against the focused monitor moves the window using another
    // screen's geometry the moment the two differ.
    let mon = match hypr::monitor_for_address(&win.address) {
        Ok(Some(m)) => m,
        _ => match hypr::focused_monitor() {
            Ok(m) => m,
            Err(_) => return 1,
        },
    };

    let obs_region = if cfg.obs_aware {
        obs::scene_path(cfg.obs_scene_path.as_deref())
            .and_then(|p| obs::find_region_for_monitor(&p, &mon))
    } else {
        None
    };

    let mut p = placement(cfg, &mon, &pos, full, obs_region);
    // A panel that is currently open claims a rectangle. Dodging here rather
    // than when the panel opened means an overlay shown afterwards also lands
    // clear of it, and the position the user chose is never overwritten.
    if !full
        && let Some(blocker) = crate::blocker::obstruction(&p)
        && let Some(moved) = positioning::dodge(&p, &blocker, &mon, cfg.margin as i32)
    {
        p = moved;
    }
    let _ = hypr::resize_window_pixel(&win.address, p.w, p.h);
    let _ = hypr::move_window_pixel(&win.address, p.x, p.y);
    if !win.pinned {
        let _ = hypr::pin_window(&win.address);
    }
    0
}

fn persist_position(pos: &str) {
    state::write_atomic(&state::position_file(), pos).ok();
}

fn notify(msg: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["-u", "low", "Camera overlay", msg])
        .status();
}


