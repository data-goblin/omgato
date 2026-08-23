use crate::config;
use crate::tui::state::App;
use crate::waybar;
use ratatui::style::Color;
use std::process::Command;

pub fn save_and_restart(app: &mut App, msg: &str) -> bool {
    if let Err(e) = config::save(&app.cfg) {
        app.flash(format!("save failed: {e}"), Color::Red);
        // roll the in-memory copy back to whatever survived on disk
        if let Ok(prev) = config::load() {
            app.cfg = prev;
        }
        return false;
    }
    let _ = Command::new("systemctl")
        .args(["--user", "kill", "--signal=SIGHUP", waybar::DECK_SERVICE])
        .status();
    let _ = Command::new("pkill").args(["-RTMIN+13", "waybar"]).status();
    app.refresh();
    app.flash(msg.to_string(), Color::Green);
    true
}
