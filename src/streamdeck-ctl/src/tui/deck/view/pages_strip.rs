use crate::tui::state::App;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let mut spans: Vec<Span> = Vec::new();
    spans.push(Span::styled("page  ", Style::default().fg(Color::DarkGray)));
    if app.deck.page_names.is_empty() {
        spans.push(Span::styled("(none)", Style::default().fg(Color::DarkGray)));
    }
    for (i, name) in app.deck.page_names.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("  "));
        }
        let is_current = name == &app.deck.current_page;
        let is_default = name == &app.cfg.deck.default_page;
        let mut s = Style::default();
        if is_current {
            s = s.fg(Color::Yellow).add_modifier(Modifier::BOLD);
        } else {
            s = s.fg(Color::DarkGray);
        }
        let prefix = if is_default { "★ " } else { "" };
        spans.push(Span::styled(format!("{prefix}{name}"), s));
    }
    spans.push(Span::raw("    "));
    spans.push(Span::styled(
        format!("{}%", app.cfg.deck.brightness),
        Style::default().fg(Color::DarkGray),
    ));
    let p = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).title(" pages "));
    f.render_widget(p, area);
}
