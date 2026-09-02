use crate::tui::state::{App, Tab};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let line = if let Some((m, c)) = &app.msg {
        Line::from(Span::styled(m.clone(), Style::default().fg(*c)))
    } else {
        Line::from(hints(app))
    };
    let p = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    f.render_widget(p, area);
}

fn hints(app: &App) -> Vec<Span<'static>> {
    if app.tab == Tab::Deck && app.deck.moving() {
        return vec![
            Span::raw("[\u{2191}\u{2193}] pick a spot  "),
            Span::raw("[\u{005B}\u{005D}] page  "),
            Span::raw("[Enter] drop  "),
            Span::raw("[Esc] cancel"),
        ];
    }
    let common: Vec<Span<'static>> = vec![
        Span::raw("[Tab] switch  "),
        Span::raw("[\u{2191}\u{2193}] move  "),
    ];
    let mut out = common;
    let extra: Vec<Span<'static>> = match app.tab {
        Tab::Pedal => vec![
            Span::raw("[e] edit  "),
            Span::raw("[t] toggle  [r] reload  "),
            Span::raw("[E/D] enable/disable  "),
        ],
        Tab::Deck => vec![
            Span::raw("[e] edit  [m] move  [D] delete  "),
            Span::raw("[\u{005B}\u{005D}] page  [P] +page  [X] -page  "),
            Span::raw("[t] toggle  [r] reload  [b] brightness  "),
        ],
    };
    out.extend(extra);
    out.push(Span::raw("[q] quit"));
    out
}
