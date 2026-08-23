use super::{buttons, persist};
use crate::tui::deck::{Field, Mode};
use crate::tui::state::App;
use crossterm::event::{KeyCode, KeyModifiers};

pub fn picker(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('l') => start(app, Field::Label),
        KeyCode::Char('g') => start(app, Field::Glyph),
        KeyCode::Char('i') => start(app, Field::Icon),
        KeyCode::Char('b') => start(app, Field::Bg),
        KeyCode::Char('f') => start(app, Field::Fg),
        KeyCode::Char('a') => super::edit_action::begin(app),
        KeyCode::Esc => app.deck.mode = Mode::Normal,
        _ => {}
    }
}

fn start(app: &mut App, field: Field) {
    app.deck.edit_field = field;
    app.deck.edit_buffer = current_value(app, field);
    app.deck.mode = Mode::EditFieldInput;
}

fn current_value(app: &App, field: Field) -> String {
    let idx = app.deck.selected_index();
    let Some(page) = app.cfg.deck.pages.get(&app.deck.current_page) else {
        return String::new();
    };
    let Some(b) = page.buttons.iter().find(|b| b.index == idx) else {
        return String::new();
    };
    match field {
        Field::Label => b.label.clone(),
        Field::Glyph => b.glyph.clone().unwrap_or_default(),
        Field::Icon => b.icon.clone().unwrap_or_default(),
        Field::Bg => b.bg.clone().unwrap_or_default(),
        Field::Fg => b.fg.clone().unwrap_or_default(),
    }
}

pub fn input(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.deck.mode = Mode::EditFieldPicker,
        KeyCode::Enter => commit(app),
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

fn commit(app: &mut App) {
    let idx = app.deck.selected_index();
    let page_name = app.deck.current_page.clone();
    let field = app.deck.edit_field;
    let value = app.deck.edit_buffer.clone();
    let value_opt = if value.is_empty() { None } else { Some(value.clone()) };

    let bref = buttons::ensure_button(app, &page_name, idx);
    match field {
        Field::Label => bref.label = value,
        Field::Glyph => bref.glyph = value_opt,
        Field::Icon => bref.icon = value_opt,
        Field::Bg => bref.bg = value_opt,
        Field::Fg => bref.fg = value_opt,
    }
    if persist::save_and_restart(app, &format!("set {} on #{idx}", field.label())) {
        app.deck.mode = Mode::Normal;
    }
}
