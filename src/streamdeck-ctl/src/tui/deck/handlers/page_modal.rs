use super::persist;
use crate::tui::deck::Mode;
use crate::tui::state::App;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;

pub fn add(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.deck.mode = Mode::Normal,
        KeyCode::Enter => commit_add(app),
        KeyCode::Backspace => {
            app.deck.edit_buffer.pop();
        }
        KeyCode::Char('u') if mods.contains(KeyModifiers::CONTROL) => {
            app.deck.edit_buffer.clear();
        }
        KeyCode::Char(c) => app.deck.edit_buffer.push(c),
        _ => {}
    }
}

fn commit_add(app: &mut App) {
    let name = app.deck.edit_buffer.trim().to_string();
    if name.is_empty() {
        app.flash("empty name", Color::Yellow);
        return;
    }
    app.cfg.deck.pages.entry(name.clone()).or_default();
    if persist::save_and_restart(app, &format!("added page '{}'", name)) {
        app.deck.current_page = name;
        app.deck.mode = Mode::Normal;
    }
}

pub fn remove(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('y') | KeyCode::Enter => commit_remove(app),
        _ => app.deck.mode = Mode::Normal,
    }
}

fn commit_remove(app: &mut App) {
    let name = app.deck.current_page.clone();
    app.cfg.deck.pages.remove(&name);
    if app.cfg.deck.default_page == name
        && let Some(first) = app.cfg.deck.pages.keys().next().cloned() {
            app.cfg.deck.default_page = first;
        }
    if persist::save_and_restart(app, &format!("removed page '{}'", name)) {
        app.deck.mode = Mode::Normal;
    }
}
