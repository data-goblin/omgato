use super::{row_to_target, split_action, ActionKind, Mode, PedalView, ROWS};
use crate::tui::state::App;
use ratatui::{
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
    Frame,
};

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let header = Row::new(vec![
        Cell::from("pedal"),
        Cell::from("gesture"),
        Cell::from("kind"),
        Cell::from("binding / command"),
    ])
    .style(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::Yellow),
    );

    let rows: Vec<Row> = (0..ROWS).map(|i| build_row(i, app)).collect();

    let title = format!(
        " pedal bindings (long={}ms double={}ms) ",
        app.cfg.pedal.long_ms, app.cfg.pedal.double_ms
    );

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .row_highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    )
    .highlight_symbol(" \u{f054} ")
    .block(Block::default().borders(Borders::ALL).title(title));

    let mut ts = app.pedal.table.clone();
    f.render_stateful_widget(table, area, &mut ts);
}

fn build_row(idx: usize, app: &App) -> Row<'static> {
    let (pos, g) = row_to_target(idx);
    let spec = app.cfg.pedal.get(pos, g);
    let (kind, detail) = split_action(spec);
    let kind_label = match kind {
        ActionKind::Key => "key",
        ActionKind::Exec => "exec",
        ActionKind::Page => "page",
        ActionKind::Back => "back",
        ActionKind::Noop => "noop",
    };
    let kind_style = match kind {
        ActionKind::Key => Style::default().fg(Color::Cyan),
        ActionKind::Exec => Style::default().fg(Color::Magenta),
        ActionKind::Page | ActionKind::Back => Style::default().fg(Color::Green),
        ActionKind::Noop => Style::default().fg(Color::DarkGray),
    };
    let is_first_of_pedal = idx.is_multiple_of(3);
    let pos_label = if is_first_of_pedal {
        pos.label().to_string()
    } else {
        String::new()
    };
    let pos_style = Style::default().add_modifier(Modifier::BOLD);
    let gesture_style = match g {
        crate::config::Gesture::Tap => Style::default(),
        crate::config::Gesture::Long => Style::default().fg(Color::Yellow),
        crate::config::Gesture::Double => Style::default().fg(Color::Cyan),
    };
    Row::new(vec![
        Cell::from(pos_label).style(pos_style),
        Cell::from(g.label()).style(gesture_style),
        Cell::from(kind_label).style(kind_style),
        Cell::from(detail),
    ])
}

pub fn draw_modal(f: &mut Frame, area: Rect, view: &PedalView) {
    match view.mode {
        Mode::EditChooseKind => draw_kind_picker(f, area),
        Mode::EditInput => draw_input(f, area, view),
        Mode::Normal => {}
    }
}

fn draw_kind_picker(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from("choose action kind:"),
        Line::from(""),
        Line::from(vec![
            Span::styled("[k] ", Style::default().fg(Color::Cyan).bold()),
            Span::raw("key   - emit a synthetic keypress (e.g. F13)"),
        ]),
        Line::from(vec![
            Span::styled("[f] ", Style::default().fg(Color::Magenta).bold()),
            Span::raw("file/exec - run a shell command or file path"),
        ]),
        Line::from(vec![
            Span::styled("[n] ", Style::default().fg(Color::DarkGray).bold()),
            Span::raw("noop  - do nothing"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "[esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn draw_input(f: &mut Frame, area: Rect, view: &PedalView) {
    let label = match view.edit_kind {
        ActionKind::Key => "key name (e.g. F13, KEY_PLAYPAUSE, A)",
        ActionKind::Exec => "shell command or file path",
        ActionKind::Page | ActionKind::Back | ActionKind::Noop => "",
    };
    let lines = vec![
        Line::from(Span::styled(label, Style::default().fg(Color::DarkGray))),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::raw(view.edit_buffer.clone()),
            Span::styled("\u{2588}", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "[enter] save  [esc] cancel  [^u] clear",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}
