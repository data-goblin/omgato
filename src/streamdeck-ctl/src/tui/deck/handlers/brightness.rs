use super::persist;
use crate::tui::deck::Mode;
use crate::tui::state::App;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;

pub fn input(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.deck.mode = Mode::Normal,
        KeyCode::Enter => commit(app),
        KeyCode::Backspace => {
            app.deck.edit_buffer.pop();
        }
        KeyCode::Char('u') if mods.contains(KeyModifiers::CONTROL) => {
            app.deck.edit_buffer.clear();
        }
        KeyCode::Char(c) if c.is_ascii_digit() => app.deck.edit_buffer.push(c),
        _ => {}
    }
}

fn commit(app: &mut App) {
    match app.deck.edit_buffer.trim().parse::<u8>() {
        Ok(v) => {
            let v = v.min(100);
            app.cfg.deck.brightness = v;
            if persist::save_and_restart(app, &format!("brightness {}", v)) {
                app.deck.mode = Mode::Normal;
            }
        }
        Err(_) => app.flash("invalid number", Color::Red),
    }
}
