use anyhow::Result;
use elgato_streamdeck::{info::Kind, new_hidapi};

pub mod deck;
pub mod pedal;

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
