mod deck;
mod pedal;
mod state;
mod widgets;

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Frame, Terminal,
};
use std::{io, time::Duration};

pub use state::{App, Tab};

pub fn run() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut term = Terminal::new(backend)?;

    let mut app = App::new();
    app.refresh();

    let result = run_loop(&mut term, &mut app);

    disable_raw_mode()?;
    term.backend_mut().execute(LeaveAlternateScreen)?;
    term.show_cursor()?;
    result
}

fn run_loop<B: ratatui::backend::Backend>(term: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        term.draw(|f| draw(f, app))?;
        if !event::poll(Duration::from_millis(400))? {
            if app.is_idle_for_refresh() {
                app.refresh();
            }
            continue;
        }
        let Event::Key(k) = event::read()? else {
            continue;
        };
        if k.kind != KeyEventKind::Press {
            continue;
        }
        if widgets::tabs::handle_global_key(app, k.code, k.modifiers) {
            continue;
        }
        let exit = match app.tab {
            Tab::Pedal => pedal::handle_key(app, k.code, k.modifiers)?,
            Tab::Deck => deck::handle_key(app, k.code, k.modifiers)?,
        };
        if exit {
            return Ok(());
        }
    }
}

fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(8),
            Constraint::Length(3),
        ])
        .split(f.area());

    widgets::tabs::draw(f, chunks[0], app);

    match app.tab {
        Tab::Pedal => pedal::draw(f, chunks[1], app),
        Tab::Deck => deck::draw(f, chunks[1], app),
    }

    widgets::footer::draw(f, chunks[2], app);

    if app.modal_open() {
        widgets::modal::draw(f, app);
    }
}
