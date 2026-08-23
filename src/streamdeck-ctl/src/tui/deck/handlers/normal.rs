use super::persist;
use crate::config::Page;
use crate::tui::deck::{Field, Mode, page_select};
use crate::tui::state::App;
use crate::waybar;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;
use std::process::Command;

pub fn handle(app: &mut App, code: KeyCode, _mods: KeyModifiers) -> Result<bool> {
    app.clear_msg();
    match code {
        KeyCode::Char('q') | KeyCode::Esc => return Ok(true),
        KeyCode::Up | KeyCode::Char('k') => move_sel(app, -1),
        KeyCode::Down | KeyCode::Char('j') => move_sel(app, 1),
        KeyCode::Char('[') => page_select::prev(app),
        KeyCode::Char(']') => page_select::next(app),
        KeyCode::Char('e') | KeyCode::Enter => begin_edit(app),
        KeyCode::Char('D') => delete_button(app),
        KeyCode::Char('P') => begin_page_add(app),
        KeyCode::Char('X') => begin_page_remove(app),
        KeyCode::Char('b') => begin_brightness(app),
        KeyCode::Char('t') => toggle_service(app),
        KeyCode::Char('r') => reload_service(app),
        _ => {}
    }
    Ok(false)
}

fn move_sel(app: &mut App, delta: i32) {
    let cur = app.deck.table.selected().unwrap_or(0) as i32;
    let next = (cur + delta).rem_euclid(crate::tui::deck::rows_per_page() as i32);
    app.deck.table.select(Some(next as usize));
}

fn begin_edit(app: &mut App) {
    app.deck.edit_field = Field::Label;
    app.deck.edit_buffer.clear();
    app.deck.mode = Mode::EditFieldPicker;
}

fn delete_button(app: &mut App) {
    let idx = app.deck.selected_index();
    let page_name = app.deck.current_page.clone();
    let removed = match app.cfg.deck.pages.get_mut(&page_name) {
        Some(page) => {
            let before = page.buttons.len();
            page.buttons.retain(|b| b.index != idx);
            page.buttons.len() != before
        }
        None => {
            app.flash("page not found", Color::Red);
            return;
        }
    };
    if !removed {
        app.flash(format!("no button at #{idx}"), Color::Yellow);
        return;
    }
    persist::save_and_restart(app, &format!("removed #{idx} from {}", page_name));
}

fn begin_page_add(app: &mut App) {
    app.deck.edit_buffer.clear();
    app.deck.mode = Mode::PageAddInput;
}

fn begin_page_remove(app: &mut App) {
    if app.deck.page_names.len() <= 1 {
        app.flash("can't remove the last page", Color::Yellow);
        // ensure at least one page exists
        if app.cfg.deck.pages.is_empty() {
            app.cfg.deck.pages.insert("main".into(), Page::default());
        }
        return;
    }
    app.deck.mode = Mode::PageRemoveConfirm;
}

fn begin_brightness(app: &mut App) {
    app.deck.edit_buffer = app.cfg.deck.brightness.to_string();
    app.deck.mode = Mode::BrightnessInput;
}

fn toggle_service(app: &mut App) {
    let args: &[&str] = if app.conn.deck_active {
        &["--user", "disable", "--now", waybar::DECK_SERVICE]
    } else {
        &["--user", "enable", "--now", waybar::DECK_SERVICE]
    };
    run_systemctl(app, args, "toggled");
}

fn reload_service(app: &mut App) {
    run_systemctl(
        app,
        &["--user", "kill", "--signal=SIGHUP", waybar::DECK_SERVICE],
        "restarted",
    );
}

pub fn run_systemctl(app: &mut App, args: &[&str], verb: &str) {
    match Command::new("systemctl").args(args).status() {
        Ok(s) if s.success() => {
            let _ = Command::new("pkill").args(["-RTMIN+13", "waybar"]).status();
            app.flash(format!("{verb} {}", waybar::DECK_SERVICE), Color::Cyan);
        }
        Ok(s) => app.flash(format!("systemctl exited {s}"), Color::Red),
        Err(e) => app.flash(format!("systemctl error: {e}"), Color::Red),
    }
    app.refresh();
}
