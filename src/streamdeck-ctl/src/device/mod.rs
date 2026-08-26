use anyhow::Result;
use elgato_streamdeck::{info::Kind, new_hidapi};
use serde::Serialize;

pub mod deck;
pub mod pedal;

#[derive(Debug, Clone, Serialize)]
pub struct DeviceInfo {
    pub kind: String,
    pub name: String,
    pub serial: String,
    pub keys: u8,
    pub rows: u8,
    pub cols: u8,
    pub encoders: u8,
    pub visual: bool,
    pub pedal: bool,
    pub image_size: u32,
}

impl DeviceInfo {
    pub fn new(kind: &Kind, serial: String) -> Self {
        Self {
            kind: format!("{kind:?}"),
            name: friendly_name(kind).to_owned(),
            serial,
            keys: kind.key_count(),
            rows: kind.row_count(),
            cols: kind.column_count(),
            encoders: kind.encoder_count(),
            visual: kind.is_visual(),
            pedal: matches!(kind, Kind::Pedal),
            image_size: kind.key_image_format().size.0 as u32,
        }
    }
}

pub fn describe_all() -> Result<Vec<DeviceInfo>> {
    Ok(list_all()?
        .into_iter()
        .map(|(kind, serial)| DeviceInfo::new(&kind, serial))
        .collect())
}

pub fn friendly_name(kind: &Kind) -> &'static str {
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

pub fn list_all() -> Result<Vec<(Kind, String)>> {
    let hid = new_hidapi()?;
    Ok(elgato_streamdeck::list_devices(&hid))
}

pub fn list_pedals() -> Result<Vec<(Kind, String)>> {
    Ok(list_all()?
        .into_iter()
        .filter(|(k, _)| matches!(k, Kind::Pedal))
        .collect())
}

pub fn list_decks() -> Result<Vec<(Kind, String)>> {
    Ok(list_all()?
        .into_iter()
        .filter(|(k, _)| !matches!(k, Kind::Pedal))
        .collect())
}
