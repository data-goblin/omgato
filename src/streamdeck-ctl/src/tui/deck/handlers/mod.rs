mod brightness;
mod buttons;
mod edit_action;
mod edit_field;
mod normal;
mod page_modal;
mod persist;

use super::Mode;
use crate::tui::state::App;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};

pub fn handle_key(app: &mut App, code: KeyCode, mods: KeyModifiers) -> Result<bool> {
    match app.deck.mode {
        Mode::Normal => normal::handle(app, code, mods),
        Mode::EditFieldPicker => {
            edit_field::picker(app, code);
            Ok(false)
        }
        Mode::EditFieldInput => {
            edit_field::input(app, code, mods);
            Ok(false)
        }
        Mode::EditActionKind => {
            edit_action::kind(app, code);
            Ok(false)
        }
        Mode::EditActionInput => {
            edit_action::input(app, code, mods);
            Ok(false)
        }
        Mode::PageAddInput => {
            page_modal::add(app, code, mods);
            Ok(false)
        }
        Mode::PageRemoveConfirm => {
            page_modal::remove(app, code);
            Ok(false)
        }
        Mode::BrightnessInput => {
            brightness::input(app, code, mods);
            Ok(false)
        }
    }
}
