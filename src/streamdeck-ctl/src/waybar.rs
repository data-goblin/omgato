use crate::config::Config;
use crate::device;
use anyhow::Result;
use elgato_streamdeck::info::Kind;
use std::process::Command;

pub const PEDAL_SERVICE: &str = "streamdeck-ctl.service";
pub const DECK_SERVICE: &str = "streamdeck-ctl-deck.service";

pub fn emit(_cfg: &Config) -> Result<()> {
    let devices = device::list_all().unwrap_or_default();
    let pedal_active = service_active();
    let deck_active = deck_service_active();

    let any_connected = !devices.is_empty();
    let any_active = pedal_active || deck_active;

    let (alt, class) = match (any_connected, any_active) {
        (false, _) => ("disconnected", "disconnected"),
        (true, true) => ("on", "on"),
        (true, false) => ("off", "off"),
    };

    let tooltip = build_tooltip(&devices, pedal_active, deck_active);

    println!(
        "{}",
        serde_json::json!({
            "text": "",
            "alt": alt,
            "class": class,
            "tooltip": tooltip,
        })
    );
    Ok(())
}

fn build_tooltip(
    devices: &[(Kind, String)],
    pedal_active: bool,
    deck_active: bool,
) -> String {
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("Devices connected: {}", devices.len()));
    if devices.is_empty() {
        lines.push("• (none)".into());
    } else {
        for (kind, serial) in devices {
            lines.push(format!("• {} ({})", friendly_name(kind), serial));
        }
    }
    lines.push(String::new());
    lines.push(format!(
        "Pedal daemon: {}",
        if pedal_active { "active" } else { "inactive" }
    ));
    lines.push(format!(
        "Deck daemon:  {}",
        if deck_active { "active" } else { "inactive" }
    ));
    lines.join("\n")
}

fn friendly_name(kind: &Kind) -> &'static str {
    match kind {
        Kind::Pedal => "Stream Deck Pedal",
        Kind::Original => "Stream Deck (Original)",
        Kind::OriginalV2 => "Stream Deck (Original v2)",
        Kind::Mk2 => "Stream Deck Mk.2",
        Kind::Mk2Scissor => "Stream Deck Mk.2 Scissor",
        Kind::Mini => "Stream Deck Mini",
        Kind::MiniMk2 => "Stream Deck Mini Mk.2",
        Kind::MiniDiscord => "Stream Deck Mini Discord",
        Kind::Xl => "Stream Deck XL",
        Kind::XlV2 => "Stream Deck XL v2",
        Kind::Plus => "Stream Deck +",
        Kind::Neo => "Stream Deck Neo",
        Kind::MiniMk2Module => "Stream Deck Mini Mk.2 Module",
        Kind::Mk2Module => "Stream Deck Mk.2 Module",
        Kind::XlV2Module => "Stream Deck XL v2 Module",
    }
}

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
