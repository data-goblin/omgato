#![allow(clippy::arc_with_non_send_sync)]

use anyhow::{anyhow, Result};
use elgato_streamdeck::{info::Kind, new_hidapi, StreamDeck};
use std::sync::Arc;

pub struct Deck {
    pub kind: Kind,
    pub serial: String,
    pub deck: Arc<StreamDeck>,
}

pub fn open_first() -> Result<Deck> {
    let hid = new_hidapi()?;
    let (kind, serial) = elgato_streamdeck::list_devices(&hid)
        .into_iter()
        .find(|(k, _)| !matches!(k, Kind::Pedal))
        .ok_or_else(|| anyhow!("no Stream Deck (non-pedal) found"))?;
    let deck = StreamDeck::connect(&hid, kind, &serial)?;
    Ok(Deck {
        kind,
        serial,
        deck: Arc::new(deck),
    })
}

