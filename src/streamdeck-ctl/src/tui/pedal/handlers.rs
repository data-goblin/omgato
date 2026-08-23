use super::{join_action, split_action, ActionKind, Mode, ROWS};
use crate::action;
use crate::config;
use crate::tui::state::App;
use crate::waybar;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;
use std::process::Command;

pub fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) -> Result<bool> {
    match app.pedal.mode {
        Mode::Normal => normal(app, code, mods),
        Mode::EditChooseKind => {
            choose_kind(app, code);
            Ok(false)
        }
        Mode::EditInput => {
            input(app, code, mods);
            Ok(false)
        }
    }
}

fn normal(app: &mut App, code: KeyCode, _mods: KeyModifiers) -> Result<bool> {
    app.clear_msg();
    match code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
        KeyCode::Up | KeyCode::Char('k') => move_sel(app, -1),
        KeyCode::Down | KeyCode::Char('j') => move_sel(app, 1),
        KeyCode::Char('e') | KeyCode::Enter => begin_edit(app),
        KeyCode::Char('E') => svc(
            app,
            &["--user", "enable", "--now", waybar::PEDAL_SERVICE],
            "enabled",
        ),
        KeyCode::Char('D') => svc(
            app,
            &["--user", "disable", "--now", waybar::PEDAL_SERVICE],
            "disabled",
        ),
        KeyCode::Char('t') => {
            let args: &[&str] = if app.conn.pedal_active {
                &["--user", "disable", "--now", waybar::PEDAL_SERVICE]
            } else {
                &["--user", "enable", "--now", waybar::PEDAL_SERVICE]
            };
            svc(app, args, "toggled");
        }
        KeyCode::Char('r') => reload(app),
        _ => {}
    }
    Ok(false)
}

fn move_sel(app: &mut App, delta: i32) {
    let cur = app.pedal.table.selected().unwrap_or(0) as i32;
    let next = (cur + delta).rem_euclid(ROWS as i32);
    app.pedal.table.select(Some(next as usize));
}

fn begin_edit(app: &mut App) {
    let (pos, g) = app.pedal.selected_target();
    let cur = app.cfg.pedal.get(pos, g).to_string();
    let (kind, detail) = split_action(&cur);
    app.pedal.edit_pos = pos;
    app.pedal.edit_gesture = g;
    app.pedal.edit_kind = kind;
    app.pedal.edit_buffer = detail;
    app.pedal.mode = Mode::EditChooseKind;
}

fn choose_kind(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('k') => {
            app.pedal.edit_kind = ActionKind::Key;
            app.pedal.mode = Mode::EditInput;
        }
        KeyCode::Char('f') | KeyCode::Char('x') | KeyCode::Char('e') => {
            app.pedal.edit_kind = ActionKind::Exec;
            app.pedal.mode = Mode::EditInput;
        }
        KeyCode::Char('n') => {
            app.pedal.edit_kind = ActionKind::Noop;
            app.pedal.edit_buffer.clear();
            commit(app);
        }
        KeyCode::Esc => app.pedal.mode = Mode::Normal,
        _ => {}
    }
}

fn input(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.pedal.mode = Mode::Normal,
        KeyCode::Enter => commit(app),
        KeyCode::Backspace => {
            app.pedal.edit_buffer.pop();
        }
        KeyCode::Char('u') if mods.contains(KeyModifiers::CONTROL) => {
            app.pedal.edit_buffer.clear();
        }
        KeyCode::Char(c) => app.pedal.edit_buffer.push(c),
        _ => {}
    }
}

fn commit(app: &mut App) {
    let spec = join_action(app.pedal.edit_kind, &app.pedal.edit_buffer);
    if let Err(e) = action::parse(&spec) {
        app.flash(format!("invalid: {e}"), Color::Red);
        app.pedal.mode = Mode::EditInput;
        return;
    }
    app.cfg.pedal.set(app.pedal.edit_pos, app.pedal.edit_gesture, spec.clone());
    if let Err(e) = config::save(&app.cfg) {
        app.flash(format!("save failed: {e}"), Color::Red);
        return;
    }
    let _ = Command::new("systemctl")
        .args(["--user", "kill", "--signal=SIGHUP", waybar::PEDAL_SERVICE])
        .status();
    let _ = Command::new("pkill").args(["-RTMIN+12", "waybar"]).status();
    app.flash(
        format!(
            "{} {} = {}",
            app.pedal.edit_pos.label(),
            app.pedal.edit_gesture.label(),
            spec
        ),
        Color::Green,
    );
    app.pedal.mode = Mode::Normal;
    app.refresh();
}

fn reload(app: &mut App) {
    let _ = Command::new("systemctl")
        .args(["--user", "kill", "--signal=SIGHUP", waybar::PEDAL_SERVICE])
        .status();
    app.refresh();
    app.flash("daemon restarted", Color::Cyan);
}

fn svc(app: &mut App, args: &[&str], verb: &str) {
    match Command::new("systemctl").args(args).status() {
        Ok(s) if s.success() => {
            let _ = Command::new("pkill").args(["-RTMIN+12", "waybar"]).status();
            app.flash(format!("{verb} {}", waybar::PEDAL_SERVICE), Color::Cyan);
        }
        Ok(s) => app.flash(format!("systemctl exited {s}"), Color::Red),
        Err(e) => app.flash(format!("systemctl error: {e}"), Color::Red),
    }
    app.refresh();
}
