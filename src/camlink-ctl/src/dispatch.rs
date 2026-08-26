use std::time::Duration;

use crate::cli::{Cmd, Corner};
use crate::config::{self, Config};
use crate::positioning::{self, placement};
use crate::obs;
use crate::reset;
use crate::status::{self, CamState};
use crate::{hypr, overlay, state};

pub fn run(cmd: Cmd) -> i32 {
    let _lock = match state::command_lock() {
        Ok(lock) => lock,
        Err(e) => {
            eprintln!("camlink-ctl: could not lock overlay state: {e}");
            return 1;
        }
    };
    let cfg = config::load();
    let may_start = matches!(
        &cmd,
        Cmd::Show | Cmd::Toggle | Cmd::Move { .. } | Cmd::Place { .. } | Cmd::Full
    );
    if !may_start && !overlay::is_running(&cfg.window_title)
        && let Err(e) = return_borrowed(false)
    {
        eprintln!("camlink-ctl: {e}");
    }
    match cmd {
        Cmd::Show => cmd_show(&cfg, None),
        Cmd::Hide => cmd_hide(&cfg),
        Cmd::Toggle => {
            if overlay::is_running(&cfg.window_title) { cmd_hide(&cfg) } else { cmd_show(&cfg, None) }
        }
        Cmd::Move { corner } => cmd_move(&cfg, corner),
        Cmd::Place { geometry } => cmd_place(&cfg, &geometry),
        Cmd::Pick => cmd_pick(&cfg),
        Cmd::Avoid { geometry, monitor, owner } => {
            cmd_avoid(&cfg, &geometry, monitor.as_deref(), &owner)
        }
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
        Cmd::Reset => cmd_reset(&cfg),
    }
}

fn cmd_reset(cfg: &Config) -> i32 {
    if overlay::is_running(&cfg.window_title)
        && !overlay::kill_running(&cfg.window_title)
    {
        eprintln!("camlink-ctl: could not stop the camera overlay");
        return 1;
    }
    let result = match reset::reset() {
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
    };
    if let Err(e) = return_borrowed(true) {
        eprintln!("camlink-ctl: {e}");
        return 1;
    }
    result
}

fn cmd_show(cfg: &Config, override_position: Option<&str>) -> i32 {
    if overlay::is_running(&cfg.window_title) {
        if let Some(pos) = override_position {
            persist_position(pos);
        }
        return place_now(cfg);
    }

    let Some(dev) = overlay::find_device(&cfg.device_pattern) else {
        notify("Cam Link 4K not connected");
        return 1;
    };

    if let Some(pos) = override_position {
        persist_position(pos);
        state::remove(&state::fullscreen_flag());
    }

    let busy = crate::holder::holders(&dev);
    if !busy.is_empty() {
        let Some(unit) = crate::holder::borrowable_unit(&busy) else {
            let msg = format!(
                "camera is held by {}; {}",
                crate::holder::describe(&busy),
                crate::holder::remedy(&busy)
            );
            eprintln!("camlink-ctl: {msg}");
            notify(&msg);
            return 1;
        };
        if let Some(previous) = state::read(&state::borrowed_unit())
            && previous != unit
        {
            if let Err(e) = return_borrowed(false) {
                eprintln!("camlink-ctl: {e}");
            } else {
                eprintln!("camlink-ctl: returned {previous}; retry to borrow {unit}");
            }
            return 1;
        }
        let borrowed = match crate::holder::borrow(&unit, &dev, &busy) {
            Ok(borrowed) => borrowed,
            Err(e) => {
                let returned = return_borrowed(false);
                let msg = match returned {
                    Ok(()) => e,
                    Err(give_back) => format!("{e}; {give_back}"),
                };
                eprintln!("camlink-ctl: {msg}");
                notify(&msg);
                return 1;
            }
        };
        if borrowed {
            notify(&format!("Paused {unit} to use the camera"));
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
            let _ = return_borrowed(true);
            return 1;
        }
    };

    match overlay::wait_for_window(&cfg.window_title, pid, Duration::from_secs(4)) {
        overlay::WaitResult::Mapped(_addr) => {
            let _ = place_now(cfg);
            std::thread::sleep(Duration::from_millis(700));
            place_now(cfg)
        }
        overlay::WaitResult::Died => {
            notify("Cam Link 4K receiving no HDMI signal. Power on the source and try again.");
            let _ = return_borrowed(true);
            1
        }
        overlay::WaitResult::Timeout => {
            notify("Camera not outputting frames yet. mpv is waiting; overlay will appear when signal arrives.");
            0
        }
    }
}

fn cmd_hide(cfg: &Config) -> i32 {
    if overlay::is_running(&cfg.window_title)
        && !overlay::kill_running(&cfg.window_title)
    {
        eprintln!("camlink-ctl: could not stop the camera overlay");
        return 1;
    }
    state::remove(&state::fullscreen_flag());
    state::write_atomic(&state::needs_reset_flag(), "1").ok();
    if let Err(e) = return_borrowed(true) {
        eprintln!("camlink-ctl: {e}");
        return 1;
    }
    0
}

fn cmd_move(cfg: &Config, corner: Corner) -> i32 {
    let pos = config::corner_to_position(corner);
    persist_position(pos);
    state::remove(&state::fullscreen_flag());
    if !overlay::is_running(&cfg.window_title) {
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
    if !overlay::is_running(&cfg.window_title) {
        return cmd_show(cfg, Some(&rect));
    }
    place_now(cfg)
}

fn cmd_avoid(cfg: &Config, geometry: &str, monitor: Option<&str>, owner: &str) -> i32 {
    let mon = match monitor {
        Some(name) => match hypr::named_monitor(name) {
            Ok(Some(m)) => m,
            Ok(None) => {
                eprintln!("camlink-ctl: avoid: output {name:?} is not active");
                return 1;
            }
            Err(e) => { eprintln!("camlink-ctl: {e}"); return 1; }
        },
        None => match hypr::focused_monitor() {
            Ok(m) => m,
            Err(e) => { eprintln!("camlink-ctl: {e}"); return 1; }
        },
    };

    let blocker = if let Some((w, h)) = positioning::size_from_text(geometry) {
        positioning::panel_rect(&mon, w, h)
    } else if let Some(pos) = positioning::rect_from_geometry(geometry) {
        match positioning::parse_rect(&pos) {
            Some(mut r) => {
                if monitor.is_some() {
                    let Some(x) = r.x.checked_add(mon.x) else {
                        eprintln!("camlink-ctl: avoid: horizontal coordinate is out of range");
                        return 2;
                    };
                    let Some(y) = r.y.checked_add(mon.y) else {
                        eprintln!("camlink-ctl: avoid: vertical coordinate is out of range");
                        return 2;
                    };
                    r.x = x;
                    r.y = y;
                }
                r
            }
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
    if !overlay::is_running(&cfg.window_title) {
        return 0;
    }
    place_now(cfg)
}

fn cmd_release(cfg: &Config, owner: &str) -> i32 {
    crate::blocker::release(owner);
    if !overlay::is_running(&cfg.window_title) {
        return 0;
    }
    place_now(cfg)
}

fn cmd_replace(cfg: &Config) -> i32 {
    if !overlay::is_running(&cfg.window_title) {
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
    if !overlay::is_running(&cfg.window_title)
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

    let json = serde_json::json!({
        "text": "",
        "alt": alt,
        "class": class,
        "tooltip": tooltip,
        "overlay": overlay::is_running(&cfg.window_title),
        "position": state::read(&state::position_file()).unwrap_or_else(|| cfg.position.clone()),
        "paused": state::exists(&state::pause_flag()),
    });
    println!("{}", json);
    0
}

fn place_now(cfg: &Config) -> i32 {
    let mut last_error = String::new();
    for attempt in 0..2 {
        match place_once(cfg) {
            Ok(()) => return 0,
            Err(e) => last_error = e,
        }
        if attempt == 0 {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    eprintln!("camlink-ctl: could not place the overlay: {last_error}");
    1
}

fn place_once(cfg: &Config) -> Result<(), String> {
    let pos = state::read(&state::position_file()).unwrap_or_else(|| cfg.position.clone());
    let full = state::exists(&state::fullscreen_flag());

    let win = match hypr::find_window(&cfg.window_title) {
        Ok(Some(w)) => w,
        Ok(None) => return Err("overlay window is not mapped".into()),
        Err(e) => return Err(e),
    };

    let mon = match hypr::monitor_for_address(&win.address) {
        Ok(Some(m)) => m,
        _ => hypr::focused_monitor()?,
    };

    let obs_region = if cfg.obs_aware {
        obs::scene_path(cfg.obs_scene_path.as_deref())
            .and_then(|p| obs::find_region_for_monitor(&p, &mon))
    } else {
        None
    };

    let mut p = placement(cfg, &mon, &pos, full, obs_region);
    if !full
        && let Some(saved) = positioning::parse_rect(&pos)
        && saved != p
    {
        persist_position(&positioning::rect_to_position(&p));
    }
    if !full
        && let Some(blocker) = crate::blocker::obstruction(&p)
        && let Some(moved) = positioning::dodge(&p, &blocker, &mon, cfg.margin as i32)
    {
        p = moved;
    }
    hypr::resize_window_pixel(&win.address, p.w, p.h)?;
    hypr::move_window_pixel(&win.address, p.x, p.y)?;
    if !win.pinned {
        hypr::pin_window(&win.address)?;
    }
    Ok(())
}

fn persist_position(pos: &str) {
    state::write_atomic(&state::position_file(), pos).ok();
}

fn notify(msg: &str) {
    let _ = std::process::Command::new("notify-send")
        .args(["-u", "low", "Camera overlay", msg])
        .status();
}

fn return_borrowed(tell_user: bool) -> Result<(), String> {
    let Some(unit) = state::read(&state::borrowed_unit()) else {
        return Ok(());
    };
    crate::holder::give_back(&unit)?;
    state::remove(&state::borrowed_unit());
    if tell_user {
        notify(&format!("Resumed {unit}"));
    }
    Ok(())
}
