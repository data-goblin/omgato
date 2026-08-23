mod handlers;
mod view;

use crate::action::ActionKind;
use crate::config::{Gesture, PedalPos};
use ratatui::widgets::TableState;

pub use handlers::handle_key;
pub use view::draw;

#[derive(Clone, Copy, PartialEq)]
pub enum Mode {
    Normal,
    EditChooseKind,
    EditInput,
}

pub const ROWS: usize = 9;

pub struct PedalView {
    pub table: TableState,
    pub mode: Mode,
    pub edit_pos: PedalPos,
    pub edit_gesture: Gesture,
    pub edit_kind: ActionKind,
    pub edit_buffer: String,
}

impl PedalView {
    pub fn new() -> Self {
        let mut t = TableState::default();
        t.select(Some(0));
        Self {
            table: t,
            mode: Mode::Normal,
            edit_pos: PedalPos::Left,
            edit_gesture: Gesture::Tap,
            edit_kind: ActionKind::Key,
            edit_buffer: String::new(),
        }
    }

    pub fn modal_open(&self) -> bool {
        !matches!(self.mode, Mode::Normal)
    }

    pub fn modal_title(&self) -> String {
        format!(
            " edit: {} {} ",
            self.edit_pos.label(),
            self.edit_gesture.label()
        )
    }

    pub fn draw_modal(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        view::draw_modal(f, area, self)
    }

    pub fn selected_target(&self) -> (PedalPos, Gesture) {
        row_to_target(self.table.selected().unwrap_or(0))
    }
}

pub fn row_to_target(idx: usize) -> (PedalPos, Gesture) {
    let pos_idx = (idx / 3).min(2);
    let g_idx = (idx % 3).min(2);
    (
        PedalPos::from_index(pos_idx).unwrap_or(PedalPos::Left),
        Gesture::ALL[g_idx],
    )
}

pub use crate::action::{join as join_action, split as split_action};
