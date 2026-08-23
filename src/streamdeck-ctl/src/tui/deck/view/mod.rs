mod buttons_table;
mod modals;
mod pages_strip;

use crate::tui::state::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    Frame,
};

pub use modals::draw_modal;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(8)])
        .split(area);

    pages_strip::draw(f, chunks[0], app);
    buttons_table::draw(f, chunks[1], app);
}
