use crate::tui::state::{App, Tab};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

const TABS: [Tab; 2] = [Tab::Pedal, Tab::Deck];

/// Returns true when the key was handled and the tab handler should be skipped.
pub fn handle_global_key(app: &mut App, code: KeyCode, _mods: KeyModifiers) -> bool {
    if app.modal_open() {
        return false;
    }
    match code {
        KeyCode::Tab | KeyCode::BackTab => {
            app.tab = app.tab.next();
            app.clear_msg();
            true
        }
        KeyCode::Char('1') => {
            app.tab = Tab::Pedal;
            true
        }
        KeyCode::Char('2') => {
            app.tab = Tab::Deck;
            true
        }
        _ => false,
    }
}

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let lines = vec![tab_row(app), daemon_row(app)];
    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" streamdeck-ctl "),
    );
    f.render_widget(p, area);
}

fn tab_row(app: &App) -> Line<'static> {
    let mut spans: Vec<Span> = Vec::new();
    for (i, tab) in TABS.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("    "));
        }
        let style = if *tab == app.tab {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(format!("{} {}", i + 1, tab.label()), style));
        spans.push(Span::styled(status_dot(*tab, app), dot_style(*tab, app)));
    }
    Line::from(spans)
}

fn daemon_row(app: &App) -> Line<'static> {
    let pedal = service_label("pedal", app.conn.pedal_connected, app.conn.pedal_active);
    let deck = service_label("deck", app.conn.deck_connected, app.conn.deck_active);
    let mut spans: Vec<Span> = Vec::new();
    spans.extend(pedal);
    spans.push(Span::raw("    "));
    spans.extend(deck);
    Line::from(spans)
}

fn service_label(name: &str, connected: bool, active: bool) -> Vec<Span<'static>> {
    let (text, color) = match (connected, active) {
        (true, true) => ("active", Color::Green),
        (true, false) => ("inactive", Color::Yellow),
        (false, _) => ("disconnected", Color::DarkGray),
    };
    vec![
        Span::styled(
            format!("{:<5} ", name),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(text.to_string(), Style::default().fg(color).add_modifier(Modifier::BOLD)),
    ]
}

fn status_dot(tab: Tab, app: &App) -> &'static str {
    let (connected, active) = tab_state(tab, app);
    match (connected, active) {
        (true, true) => " \u{25cf}",
        (true, false) => " \u{25d0}",
        (false, _) => " \u{25cb}",
    }
}

fn dot_style(tab: Tab, app: &App) -> Style {
    let (connected, active) = tab_state(tab, app);
    match (connected, active) {
        (true, true) => Style::default().fg(Color::Green),
        (true, false) => Style::default().fg(Color::Yellow),
        (false, _) => Style::default().fg(Color::DarkGray),
    }
}

fn tab_state(tab: Tab, app: &App) -> (bool, bool) {
    match tab {
        Tab::Pedal => (app.conn.pedal_connected, app.conn.pedal_active),
        Tab::Deck => (app.conn.deck_connected, app.conn.deck_active),
    }
}
