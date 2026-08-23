use crate::config;
use crate::tui::state::App;
use crate::units;
use ratatui::style::Color;
use std::process::Command;

pub fn save_and_restart(app: &mut App, msg: &str) -> bool {
    if let Err(e) = config::save(&app.cfg) {
        app.flash(format!("save failed: {e}"), Color::Red);
        if let Ok(prev) = config::load() {
            app.cfg = prev;
        }
        return false;
    }
    let _ = Command::new("systemctl")
        .args(["--user", "kill", "--signal=SIGHUP", units::DECK_SERVICE])
        .status();
    app.refresh();
    app.flash(msg.to_string(), Color::Green);
    true
}
