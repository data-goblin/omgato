use crate::config::Button;
use crate::tui::deck::{parse_action, ActionKind, ROWS_PER_PAGE};
use crate::tui::state::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec![
        Cell::from("idx"),
        Cell::from("label"),
        Cell::from("icon"),
        Cell::from("kind"),
        Cell::from("binding / target"),
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Yellow),
    );

    let rows: Vec<Row> = (0..crate::tui::deck::rows_per_page())
        .map(|i| build_row(i, app))
        .collect();
    let widths = [
        Constraint::Length(4),
        Constraint::Length(14),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Min(20),
    ];
    let held = match &app.deck.move_from {
        Some((page, index)) if *page == app.deck.current_page => format!("  ·  moving #{index}"),
        Some((page, index)) => format!("  ·  moving {page} #{index}"),
        None => String::new(),
    };
    let title = format!(
        " {} ({}/{}){} ",
        app.deck.current_page,
        app.deck.selected_index() + 1,
        ROWS_PER_PAGE,
        held
    );
    let table = Table::new(rows, widths)
        .header(header)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol(" \u{f054} ")
        .block(Block::default().borders(Borders::ALL).title(title));

    let mut ts = app.deck.table.clone();
    f.render_stateful_widget(table, area, &mut ts);
}

fn build_row(index: u8, app: &App) -> Row<'static> {
    let page = app.cfg.deck.pages.get(&app.deck.current_page);
    let button = page.and_then(|p| p.buttons.iter().find(|b| b.index == index));
    let row = match button {
        Some(b) => button_row(index, b),
        None => empty_row(index),
    };
    let held = app
        .deck
        .move_from
        .as_ref()
        .is_some_and(|(page, at)| *page == app.deck.current_page && *at == index);
    if held {
        return row.style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::DIM | Modifier::ITALIC),
        );
    }
    row
}

fn button_row(index: u8, b: &Button) -> Row<'static> {
    let (kind, detail) = parse_action(&b.action);
    let (kind_label, kind_style) = match kind {
        ActionKind::Key => ("key", Style::default().fg(Color::Cyan)),
        ActionKind::Exec => ("exec", Style::default().fg(Color::Magenta)),
        ActionKind::Page => ("page", Style::default().fg(Color::Green)),
        ActionKind::Back => ("back", Style::default().fg(Color::Green)),
        ActionKind::Noop => ("noop", Style::default().fg(Color::DarkGray)),
    };
    let detail_text = if b.action.starts_with("page:") || b.action == "back" {
        format!("→ {}", b.action)
    } else {
        detail
    };
    let detail_style = if b.action.starts_with("page:") || b.action == "back" {
        Style::default().fg(Color::Green)
    } else {
        Style::default()
    };
    let (visual, visual_style) = match (&b.icon, &b.glyph) {
        (Some(_), _) => (
            "PNG".to_string(),
            Style::default().fg(Color::Yellow),
        ),
        (None, Some(g)) => (g.clone(), Style::default()),
        _ => (String::new(), Style::default()),
    };
    Row::new(vec![
        Cell::from(format!("{:>2}", index)),
        Cell::from(b.label.clone()),
        Cell::from(visual).style(visual_style),
        Cell::from(kind_label).style(kind_style),
        Cell::from(detail_text).style(detail_style),
    ])
}

fn empty_row(index: u8) -> Row<'static> {
    let dim = Style::default().fg(Color::DarkGray);
    Row::new(vec![
        Cell::from(format!("{:>2}", index)).style(dim),
        Cell::from("·").style(dim),
        Cell::from("").style(dim),
        Cell::from("").style(dim),
        Cell::from("(unset)").style(dim),
    ])
}
