use crate::tui::state::{App, Tab};
use ratatui::{
    layout::{Margin, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, Clear},
    Frame,
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let (w, h) = match app.tab {
        Tab::Pedal => (60, 11),
        Tab::Deck => (70, 13),
    };
    let modal = center_rect(area, w, h);

    f.render_widget(Clear, modal);
    let title = match app.tab {
        Tab::Pedal => app.pedal.modal_title(),
        Tab::Deck => app.deck.modal_title(),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Yellow));
    f.render_widget(block.clone(), modal);

    let inner = modal.inner(Margin { vertical: 1, horizontal: 2 });
    match app.tab {
        Tab::Pedal => app.pedal.draw_modal(f, inner),
        Tab::Deck => app.deck.draw_modal(f, inner),
    }
}

pub fn center_rect(area: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(area.width.saturating_sub(4));
    let h = h.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(w)) / 2;
    let y = (area.height.saturating_sub(h)) / 2;
    Rect { x, y, width: w, height: h }
}
