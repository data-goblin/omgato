use std::process::Command;

pub const PEDAL_SERVICE: &str = "streamdeck-ctl.service";
pub const DECK_SERVICE: &str = "streamdeck-ctl-deck.service";

pub fn service_active() -> bool {
    is_active(PEDAL_SERVICE)
}

pub fn deck_service_active() -> bool {
    is_active(DECK_SERVICE)
}

fn is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
