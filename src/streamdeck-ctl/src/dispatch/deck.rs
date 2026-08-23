use super::service;
use crate::action;
use crate::cli::DeckCmd;
use crate::config::{self, Button, Config, Page};
use crate::render::Renderer;
use crate::{daemon, device, waybar};
use anyhow::Result;
use std::collections::HashMap;

pub fn dispatch(cmd: DeckCmd) -> Result<()> {
    match cmd {
        DeckCmd::Ls => list_devices(),
        DeckCmd::Run => daemon::deck::run(&config::load()?),
        DeckCmd::Render => render_once(&config::load()?),
        DeckCmd::Show => show(&config::load()?),
        DeckCmd::Brightness { value } => set_brightness(value),
        DeckCmd::Power { state } => set_power(&state),
        DeckCmd::Reset => reset(),
        DeckCmd::Reload => service::reload(waybar::DECK_SERVICE),
        DeckCmd::Set {
            page,
            index,
            label,
            glyph,
            icon,
            bg,
            fg,
            action,
        } => set_button(page, index, label, glyph, icon, bg, fg, action),
        DeckCmd::Unset { page, index } => unset_button(page, index),
        DeckCmd::Pages => list_pages(&config::load()?),
        DeckCmd::PageAdd { name } => page_add(name),
        DeckCmd::PageRm { name } => page_rm(name),
        DeckCmd::Default { name } => set_default_page(name),
        DeckCmd::Order => show_order(&config::load()?),
        DeckCmd::OrderSet { names } => set_order(names),
        DeckCmd::AutoPaginate { enabled } => set_auto_paginate(enabled),
        DeckCmd::Preset { name, replace } => apply_preset(&name, replace),
        DeckCmd::Export { out, page, size, keys, radius } => {
            crate::export::run(&config::load()?, &out, page, size, keys, radius)
        }
    }
}

fn show_order(cfg: &Config) -> Result<()> {
    let order = cfg.deck.ordered_pages();
    if order.is_empty() {
        println!("(no pages)");
        return Ok(());
    }
    println!(
        "auto_paginate: {}",
        if cfg.deck.auto_paginate { "on" } else { "off" }
    );
    for (i, name) in order.iter().enumerate() {
        println!("  {}. {}", i + 1, name);
    }
    if cfg.deck.page_order.is_empty() {
        println!();
        println!("(page_order unset; falling back to alphabetical)");
    }
    Ok(())
}

fn set_order(names: Vec<String>) -> Result<()> {
    let mut cfg = config::load()?;
    for n in &names {
        if !cfg.deck.pages.contains_key(n) {
            anyhow::bail!("page '{}' does not exist", n);
        }
    }
    cfg.deck.page_order = names;
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    Ok(())
}

fn set_auto_paginate(enabled: bool) -> Result<()> {
    let mut cfg = config::load()?;
    cfg.deck.auto_paginate = enabled;
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    Ok(())
}

fn list_devices() -> Result<()> {
    let decks = device::list_decks()?;
    if decks.is_empty() {
        println!("no Stream Deck (non-pedal) connected");
        return Ok(());
    }
    for (kind, serial) in decks {
        println!("{:?}  serial={}", kind, serial);
    }
    Ok(())
}

fn show(cfg: &Config) -> Result<()> {
    println!(
        "brightness: {}    default_page: {}    auto_paginate: {}",
        cfg.deck.brightness, cfg.deck.default_page, cfg.deck.auto_paginate
    );
    println!("page_order: {}", format_order(&cfg.deck.page_order));
    println!();
    println!("pages:");
    for (name, page) in &cfg.deck.pages {
        println!("  [{}]", name);
        for b in &page.buttons {
            let visual = match (&b.icon, &b.glyph) {
                (Some(p), _) => format!("icon={p}"),
                (None, Some(g)) => format!("glyph={g}"),
                _ => "-".into(),
            };
            println!(
                "    {:>2}  {:<12}  {:<32}  bg={:<8}  fg={:<8}  {}",
                b.index,
                b.label,
                visual,
                b.bg.as_deref().unwrap_or("-"),
                b.fg.as_deref().unwrap_or("-"),
                b.action
            );
        }
    }
    Ok(())
}

fn format_order(order: &[String]) -> String {
    if order.is_empty() {
        "(none)".into()
    } else {
        order.join(" → ")
    }
}

fn list_pages(cfg: &Config) -> Result<()> {
    for (name, page) in &cfg.deck.pages {
        let marker = if name == &cfg.deck.default_page { "*" } else { " " };
        println!("{} {} ({} buttons)", marker, name, page.buttons.len());
    }
    Ok(())
}

fn render_once(cfg: &Config) -> Result<()> {
    let d = device::deck::open_first()?;
    eprintln!(
        "streamdeck-ctl: rendering page '{}' on {:?} ({})",
        cfg.deck.default_page, d.kind, d.serial
    );
    let (w, _) = d.kind.key_image_format().size;
    let renderer = Renderer::new(&cfg.deck, w as u32)?;
    let key_count = d.kind.key_count();
    let page = cfg
        .deck
        .pages
        .get(&cfg.deck.default_page)
        .ok_or_else(|| anyhow::anyhow!("default_page not found"))?;
    let mut by_idx: HashMap<u8, &Button> = HashMap::new();
    for b in &page.buttons {
        by_idx.insert(b.index, b);
    }
    d.deck.set_brightness(cfg.deck.brightness)?;
    for i in 0..key_count {
        if let Some(btn) = by_idx.get(&i) {
            d.deck.set_button_image(i, renderer.render_button(btn)?)?;
        } else if let Some(synth) = cfg.deck.synthetic_button(&cfg.deck.default_page, i, key_count, d.kind.column_count()) {
            d.deck.set_button_image(i, renderer.render_button(&synth)?)?;
        } else {
            d.deck.set_button_image(i, renderer.blank())?;
        }
    }
    d.deck.flush()?;
    Ok(())
}

fn set_brightness(value: u8) -> Result<()> {
    let v = value.min(100);
    let mut cfg = config::load()?;
    cfg.deck.brightness = v;
    cfg.deck.display_off = false;
    config::save(&cfg)?;
    apply_brightness(&cfg)
}

fn set_power(state: &str) -> Result<()> {
    let mut cfg = config::load()?;
    cfg.deck.display_off = match state {
        "on" => false,
        "off" => true,
        "toggle" => !cfg.deck.display_off,
        other => anyhow::bail!("power: expected on, off or toggle, got {other}"),
    };
    config::save(&cfg)?;
    apply_brightness(&cfg)
}

/// Pushes the level straight to the device when nothing else holds it, and
/// otherwise asks the daemon to, so the change lands without a re-render.
fn apply_brightness(cfg: &Config) -> Result<()> {
    let level = cfg.deck.active_brightness();
    if let Ok(d) = device::deck::open_first() {
        let _ = d.deck.set_brightness(level);
    }
    let _ = service::refresh_brightness(waybar::DECK_SERVICE);
    Ok(())
}

fn reset() -> Result<()> {
    let d = device::deck::open_first()?;
    d.deck.reset()?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn set_button(
    page: String,
    index: u8,
    label: Option<String>,
    glyph: Option<String>,
    icon: Option<String>,
    bg: Option<String>,
    fg: Option<String>,
    action_spec: Option<String>,
) -> Result<()> {
    validate_page_name(&page)?;
    if let Some(a) = &action_spec {
        let _ = action::parse(a)?;
    }
    let mut cfg = config::load()?;
    let entry = cfg.deck.pages.entry(page.clone()).or_insert_with(Page::default);
    let pos = entry.buttons.iter().position(|b| b.index == index);
    let bref = match pos {
        Some(p) => &mut entry.buttons[p],
        None => {
            entry.buttons.push(Button {
                index,
                label: String::new(),
                glyph: None,
                icon: None,
                bg: None,
                fg: None,
                action: "noop".into(),
            });
            entry.buttons.last_mut().unwrap()
        }
    };
    apply_field(bref, label, |b, v| b.label = v);
    apply_opt(bref, glyph, |b, v| b.glyph = v);
    apply_opt(bref, icon, |b, v| b.icon = v);
    apply_opt(bref, bg, |b, v| b.bg = v);
    apply_opt(bref, fg, |b, v| b.fg = v);
    apply_field(bref, action_spec, |b, v| b.action = v);
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    Ok(())
}

fn apply_field<F>(b: &mut Button, value: Option<String>, mut f: F)
where
    F: FnMut(&mut Button, String),
{
    if let Some(v) = value {
        f(b, v);
    }
}

fn apply_opt<F>(b: &mut Button, value: Option<String>, mut f: F)
where
    F: FnMut(&mut Button, Option<String>),
{
    if let Some(v) = value {
        f(b, if v.is_empty() { None } else { Some(v) });
    }
}

fn unset_button(page: String, index: u8) -> Result<()> {
    let mut cfg = config::load()?;
    if let Some(p) = cfg.deck.pages.get_mut(&page) {
        p.buttons.retain(|b| b.index != index);
    }
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    Ok(())
}

/// Page names end up as directory names when previews are exported, so they may
/// not be empty or reach outside the directory they are written into.
fn validate_page_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        anyhow::bail!("page name cannot be empty");
    }
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        anyhow::bail!("page name cannot contain a path separator: {name}");
    }
    Ok(())
}

const PRESETS: &[(&str, &str)] = &[("omarchy", include_str!("../../preset/omarchy.toml"))];

fn apply_preset(name: &str, replace: bool) -> Result<()> {
    let Some((_, body)) = PRESETS.iter().find(|(id, _)| *id == name) else {
        anyhow::bail!(
            "unknown preset: {name} (have {})",
            PRESETS.iter().map(|(id, _)| *id).collect::<Vec<_>>().join(", ")
        );
    };
    let preset: Config = toml::from_str(body)?;
    let mut cfg = config::load()?;
    if replace {
        cfg.deck.pages.clear();
        cfg.deck.page_order.clear();
    }
    for (page, buttons) in preset.deck.pages {
        cfg.deck.pages.insert(page.clone(), buttons);
        if !cfg.deck.page_order.contains(&page) {
            cfg.deck.page_order.push(page);
        }
    }
    if cfg.deck.pages.contains_key("main") {
        cfg.deck.default_page = "main".into();
    }
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    println!("applied preset '{name}' ({} pages)", cfg.deck.pages.len());
    Ok(())
}

fn page_add(name: String) -> Result<()> {
    validate_page_name(&name)?;
    let mut cfg = config::load()?;
    cfg.deck.pages.entry(name).or_insert_with(Page::default);
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    Ok(())
}

fn page_rm(name: String) -> Result<()> {
    let mut cfg = config::load()?;
    cfg.deck.pages.remove(&name);
    if cfg.deck.default_page == name {
        if let Some(first) = cfg.deck.pages.keys().next().cloned() {
            cfg.deck.default_page = first;
        } else {
            cfg.deck.default_page = "main".into();
            cfg.deck.pages.insert("main".into(), Page::default());
        }
    }
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    Ok(())
}

fn set_default_page(name: String) -> Result<()> {
    let mut cfg = config::load()?;
    if !cfg.deck.pages.contains_key(&name) {
        anyhow::bail!("page '{}' does not exist", name);
    }
    cfg.deck.default_page = name;
    config::save(&cfg)?;
    let _ = service::reload(waybar::DECK_SERVICE);
    Ok(())
}
