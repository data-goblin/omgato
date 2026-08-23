use super::{buttons, persist};
use crate::action;
use crate::tui::deck::{join_action, parse_action, ActionKind, Mode};
use crate::tui::state::App;
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::style::Color;

pub fn begin(app: &mut App) {
    let idx = app.deck.selected_index();
    let cur = app
        .cfg
        .deck
        .pages
        .get(&app.deck.current_page)
        .and_then(|p| p.buttons.iter().find(|b| b.index == idx))
        .map(|b| b.action.clone())
        .unwrap_or_else(|| "noop".into());
    let (kind, detail) = parse_action(&cur);
    app.deck.edit_action_kind = kind;
    app.deck.edit_buffer = if cur.starts_with("page:") || cur == "back" {
        cur
    } else {
        detail
    };
    app.deck.mode = Mode::EditActionKind;
}

pub fn kind(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char('k') => {
            app.deck.edit_action_kind = ActionKind::Key;
            app.deck.mode = Mode::EditActionInput;
        }
        KeyCode::Char('x') | KeyCode::Char('e') | KeyCode::Char('f') => {
            app.deck.edit_action_kind = ActionKind::Exec;
            app.deck.mode = Mode::EditActionInput;
        }
        KeyCode::Char('p') => {
            app.deck.edit_buffer = "page:".into();
            app.deck.mode = Mode::EditActionInput;
        }
        KeyCode::Char('B') => {
            app.deck.edit_buffer = "back".into();
            commit_raw(app, "back".into());
        }
        KeyCode::Char('n') => {
            app.deck.edit_action_kind = ActionKind::Noop;
            app.deck.edit_buffer.clear();
            commit(app);
        }
        KeyCode::Esc => app.deck.mode = Mode::EditFieldPicker,
        _ => {}
    }
}

pub fn input(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    match code {
        KeyCode::Esc => app.deck.mode = Mode::EditActionKind,
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
    let buf = app.deck.edit_buffer.trim().to_string();
    let spec = if buf.starts_with("page:") || buf == "back" {
        buf
    } else {
        join_action(app.deck.edit_action_kind, &buf)
    };
    commit_raw(app, spec);
}

fn commit_raw(app: &mut App, spec: String) {
    if let Err(e) = action::parse(&spec) {
        app.flash(format!("invalid action: {e}"), Color::Red);
        app.deck.mode = Mode::EditActionInput;
        return;
    }
    let idx = app.deck.selected_index();
    let page_name = app.deck.current_page.clone();
    {
        let bref = buttons::ensure_button(app, &page_name, idx);
        bref.action = spec.clone();
    }
    if persist::save_and_restart(app, &format!("action: {spec}")) {
        app.deck.mode = Mode::Normal;
    }
}
