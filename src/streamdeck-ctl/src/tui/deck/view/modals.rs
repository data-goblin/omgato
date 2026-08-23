use crate::tui::deck::{ActionKind, DeckView, Mode};
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

pub fn draw_modal(f: &mut Frame, area: Rect, view: &DeckView) {
    match view.mode {
        Mode::EditFieldPicker => field_picker(f, area),
        Mode::EditFieldInput => field_input(f, area, view),
        Mode::EditActionKind => action_kind_picker(f, area),
        Mode::EditActionInput => action_input(f, area, view),
        Mode::PageAddInput => text_input(f, area, "new page name", &view.edit_buffer),
        Mode::PageRemoveConfirm => confirm(f, area, &view.current_page),
        Mode::BrightnessInput => text_input(f, area, "brightness 0-100", &view.edit_buffer),
        Mode::Normal => {}
    }
}

fn field_picker(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from("which field?"),
        Line::from(""),
        opt("l", "label", "the text label below the icon", Color::White),
        opt("g", "glyph", "single nerd-font codepoint", Color::White),
        opt("i", "icon", "PNG/JPG file path (overrides glyph)", Color::White),
        opt("a", "action", "what the button does", Color::Yellow),
        opt("b", "bg", "background color (#RRGGBB or empty)", Color::Cyan),
        opt("f", "fg", "foreground color (#RRGGBB or empty)", Color::Cyan),
        Line::from(""),
        Line::from(Span::styled(
            "[esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn field_input(f: &mut Frame, area: Rect, view: &DeckView) {
    let label = view.edit_field.label().to_string();
    text_input(f, area, &label, &view.edit_buffer);
}

fn action_kind_picker(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from("action kind:"),
        Line::from(""),
        opt("k", "key", "synthetic keypress (e.g. F13)", Color::Cyan),
        opt("x", "exec", "shell command or file path", Color::Magenta),
        opt("p", "page", "switch to another page", Color::Green),
        opt("B", "back", "pop history (go back one page)", Color::Green),
        opt("n", "noop", "do nothing", Color::DarkGray),
        Line::from(""),
        Line::from(Span::styled(
            "[esc] back",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn action_input(f: &mut Frame, area: Rect, view: &DeckView) {
    let label = if view.edit_buffer.starts_with("page:") {
        "page name (after page:)"
    } else {
        match view.edit_action_kind {
            ActionKind::Key => "key name (e.g. F13, KEY_PLAYPAUSE, A)",
            ActionKind::Exec => "shell command or file path",
            ActionKind::Page => "page name",
            ActionKind::Back | ActionKind::Noop => "",
        }
    };
    text_input(f, area, label, &view.edit_buffer);
}

fn text_input(f: &mut Frame, area: Rect, label: &str, buffer: &str) {
    let lines = vec![
        Line::from(Span::styled(
            label.to_string(),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Yellow)),
            Span::raw(buffer.to_string()),
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

fn confirm(f: &mut Frame, area: Rect, what: &str) {
    let lines = vec![
        Line::from(Span::styled(
            format!("remove page '{what}'?"),
            Style::default().fg(Color::Red),
        )),
        Line::from(""),
        Line::from("this will delete all buttons on this page"),
        Line::from(""),
        Line::from(Span::styled(
            "[y]/[enter] yes    [esc] cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn opt(key: &'static str, label: &'static str, desc: &'static str, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("[{}] ", key), Style::default().fg(color).bold()),
        Span::styled(
            format!("{:<7}", label),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(desc),
    ])
}
