mod parsed;
mod reconnect;
mod render;
mod synth;

use crate::action::{self, Action, Outcome};
use crate::config::{self, Config};
use crate::daemon::reload;
use crate::render::Renderer;
use anyhow::{anyhow, Result};
use elgato_streamdeck::DeviceStateUpdate;
use std::collections::HashMap;
use std::time::Duration;

const READ_TIMEOUT: Duration = Duration::from_secs(1);

pub fn run(cfg: &Config) -> Result<()> {
    let mut deck = reconnect::open_with_retry()?;
    eprintln!(
        "streamdeck-ctl: deck connected kind={:?} serial={}",
        deck.kind, deck.serial
    );

    let key_count = deck.kind.key_count();
    let cols = deck.kind.column_count();
    let (img_w, _) = deck.kind.key_image_format().size;
    let mut renderer = Renderer::new(&cfg.deck, img_w as u32)?;
    deck.deck.set_brightness(cfg.deck.active_brightness())?;

    let mut cfg = cfg.clone();
    let mut parsed_pages = parsed::parse_all(&cfg.deck.pages)?;
    let start_page = cfg.deck.default_page.clone();
    if !parsed_pages.contains_key(&start_page) {
        return Err(anyhow!(
            "default_page '{}' not found in [deck.pages]",
            start_page
        ));
    }

    let mut synth = synth::build_optional(&parsed_pages);
    let mut history: Vec<String> = Vec::new();
    let mut current = start_page;
    render_or_log(&deck, &renderer, &cfg.deck, &current, &parsed_pages, key_count, cols);

    let reload_flag = reload::install()?;
    let brightness_flag = reload::install_brightness()?;
    let mut reader = deck.deck.get_reader();
    loop {
        if reload::take(&brightness_flag)
            && let Ok(fresh) = config::load() {
                cfg.deck.brightness = fresh.deck.brightness;
                cfg.deck.display_off = fresh.deck.display_off;
                let _ = deck.deck.set_brightness(cfg.deck.active_brightness());
            }
        if reload::take(&reload_flag) {
            apply_reload(
                &deck,
                &mut renderer,
                &mut cfg,
                &mut parsed_pages,
                &mut synth,
                &mut current,
                &mut history,
                img_w as u32,
                key_count,
                cols,
            );
        }

        let updates = match reader.read(Some(READ_TIMEOUT)) {
            Ok(u) => u,
            Err(e) => {
                if is_timeout(&e.to_string()) {
                    continue;
                }
                eprintln!("streamdeck-ctl: deck read error ({e}); reconnecting");
                deck = reconnect::open_with_retry()?;
                deck.deck.set_brightness(cfg.deck.active_brightness())?;
                reader = deck.deck.get_reader();
                render_or_log(&deck, &renderer, &cfg.deck, &current, &parsed_pages, key_count, cols);
                continue;
            }
        };
        for update in updates {
            let DeviceStateUpdate::ButtonDown(idx) = update else {
                continue;
            };

            let Some(action) = resolve_action(&cfg.deck, &current, idx, &parsed_pages, key_count, cols) else {
                continue;
            };
            eprintln!(
                "streamdeck-ctl: page={} button={} -> {:?}",
                current, idx, action
            );

            let outcome = match action::dispatch(&action, &mut synth) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("streamdeck-ctl: deck dispatch error: {e}");
                    continue;
                }
            };
            match outcome {
                Outcome::None => {}
                Outcome::GotoPage(p) => {
                    if !parsed_pages.contains_key(&p) {
                        eprintln!("streamdeck-ctl: page '{}' not defined; ignoring", p);
                        continue;
                    }
                    history.push(current.clone());
                    current = p;
                    render_or_log(&deck, &renderer, &cfg.deck, &current, &parsed_pages, key_count, cols);
                }
                Outcome::Back => {
                    if let Some(p) = history.pop() {
                        current = p;
                        render_or_log(
                            &deck, &renderer, &cfg.deck, &current, &parsed_pages, key_count, cols,
                        );
                    }
                }
            }
        }
    }
}

fn is_timeout(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("timeout") || m.contains("timed out") || m.contains("interrupted")
}

#[allow(clippy::too_many_arguments)]
fn apply_reload(
    deck: &crate::device::deck::Deck,
    renderer: &mut Renderer,
    cfg: &mut Config,
    parsed_pages: &mut HashMap<String, parsed::ParsedPage>,
    synth: &mut Option<crate::synth::Synth>,
    current: &mut String,
    history: &mut Vec<String>,
    img_w: u32,
    key_count: u8,
    cols: u8,
) {
    eprintln!("streamdeck-ctl: deck reloading config (SIGHUP)");
    let new_cfg = match config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("streamdeck-ctl: deck reload failed reading config: {e}");
            return;
        }
    };
    let new_pages = match parsed::parse_all(&new_cfg.deck.pages) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("streamdeck-ctl: deck reload failed parsing pages: {e}");
            return;
        }
    };
    // If the new config doesn't have our current page, fall back to default_page.
    if !new_pages.contains_key(current) {
        history.clear();
        *current = new_cfg.deck.default_page.clone();
        if !new_pages.contains_key(current)
            && let Some(first) = new_pages.keys().next() {
                *current = first.clone();
            }
    }
    // Rebuild the renderer if any rendering input changed (fonts, default colors).
    if renderer_inputs_changed(&cfg.deck, &new_cfg.deck) {
        match Renderer::new(&new_cfg.deck, img_w) {
            Ok(r) => *renderer = r,
            Err(e) => {
                eprintln!("streamdeck-ctl: deck reload failed rebuilding renderer: {e}");
                return;
            }
        }
    }
    *cfg = new_cfg;
    *parsed_pages = new_pages;
    *synth = synth::build_optional(parsed_pages);
    let _ = deck.deck.set_brightness(cfg.deck.active_brightness());
    render_or_log(deck, renderer, &cfg.deck, current, parsed_pages, key_count, cols);
    eprintln!("streamdeck-ctl: deck config reloaded");
}

fn renderer_inputs_changed(
    a: &crate::config::DeckConfig,
    b: &crate::config::DeckConfig,
) -> bool {
    a.font_label != b.font_label
        || a.font_glyph != b.font_glyph
        || a.bg_color != b.bg_color
        || a.text_color != b.text_color
}

fn resolve_action(
    deck_cfg: &crate::config::DeckConfig,
    page_name: &str,
    idx: u8,
    parsed_pages: &HashMap<String, parsed::ParsedPage>,
    key_count: u8,
    cols: u8,
) -> Option<Action> {
    if let Some(page) = parsed_pages.get(page_name)
        && let Some(b) = page.buttons.iter().find(|b| b.btn.index == idx) {
            return Some(b.action.clone());
        }
    let synth_btn = deck_cfg.synthetic_button(page_name, idx, key_count, cols)?;
    action::parse(&synth_btn.action).ok()
}

fn render_or_log(
    deck: &crate::device::deck::Deck,
    renderer: &Renderer,
    deck_cfg: &crate::config::DeckConfig,
    current: &str,
    parsed_pages: &HashMap<String, parsed::ParsedPage>,
    key_count: u8,
    cols: u8,
) {
    let Some(page) = parsed_pages.get(current) else {
        eprintln!("streamdeck-ctl: render: unknown page '{current}'");
        return;
    };
    if let Err(e) = render::render_page(deck, renderer, deck_cfg, current, page, key_count, cols) {
        eprintln!("streamdeck-ctl: render error: {e}");
    }
}
