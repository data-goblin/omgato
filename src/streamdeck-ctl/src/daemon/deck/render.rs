use super::parsed::{ParsedButton, ParsedPage};
use crate::config::DeckConfig;
use crate::device::deck::Deck;
use crate::render::Renderer;
use anyhow::Result;
use std::collections::HashMap;

pub fn render_page(
    deck: &Deck,
    renderer: &Renderer,
    cfg: &DeckConfig,
    page_name: &str,
    page: &ParsedPage,
    key_count: u8,
    cols: u8,
) -> Result<()> {
    let mut by_index: HashMap<u8, &ParsedButton> = HashMap::new();
    for b in &page.buttons {
        by_index.insert(b.btn.index, b);
    }
    for i in 0..key_count {
        if let Some(pb) = by_index.get(&i) {
            let img = renderer.render_button(&pb.btn, page.bg.as_deref())?;
            deck.deck.set_button_image(i, img)?;
        } else if let Some(synth) = cfg.synthetic_button(page_name, i, key_count, cols) {
            let img = renderer.render_button(&synth, None)?;
            deck.deck.set_button_image(i, img)?;
        } else {
            deck.deck.set_button_image(i, renderer.blank())?;
        }
    }
    deck.deck.flush()?;
    Ok(())
}
