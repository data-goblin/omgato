use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DeckConfig {
    #[serde(default = "default_brightness")]
    pub brightness: u8,
    #[serde(default = "default_page_name")]
    pub default_page: String,
    #[serde(default = "default_font_label")]
    pub font_label: String,
    #[serde(default = "default_font_glyph")]
    pub font_glyph: String,
    #[serde(default = "default_text_color")]
    pub text_color: String,
    #[serde(default = "default_bg_color")]
    pub bg_color: String,
    #[serde(default = "default_auto_paginate")]
    pub auto_paginate: bool,
    /// Display blanked without losing the configured brightness level.
    #[serde(default)]
    pub display_off: bool,
    #[serde(default = "default_prev_glyph")]
    pub prev_glyph: String,
    #[serde(default = "default_next_glyph")]
    pub next_glyph: String,
    #[serde(default)]
    pub page_order: Vec<String>,
    #[serde(default)]
    pub pages: BTreeMap<String, Page>,
}

impl DeckConfig {
    /// Return the canonical ordered page list. Uses page_order if set;
    /// otherwise falls back to alphabetical (BTreeMap key order).
    pub fn ordered_pages(&self) -> Vec<String> {
        if !self.page_order.is_empty() {
            self.page_order
                .iter()
                .filter(|n| self.pages.contains_key(*n))
                .cloned()
                .collect()
        } else {
            self.pages.keys().cloned().collect()
        }
    }

    pub fn neighbours(&self, page: &str) -> (Option<String>, Option<String>) {
        if !self.auto_paginate {
            return (None, None);
        }
        let order = self.ordered_pages();
        let Some(i) = order.iter().position(|n| n == page) else {
            return (None, None);
        };
        let prev = if i > 0 { Some(order[i - 1].clone()) } else { None };
        let next = if i + 1 < order.len() { Some(order[i + 1].clone()) } else { None };
        (prev, next)
    }

    /// Auto-paginated synthetic button for a slot, when no user button claims it.
    /// idx 10 = previous-page arrow (or None on first/missing); idx 14 = next-page arrow.
    pub fn synthetic_button(&self, page: &str, idx: u8) -> Option<Button> {
        let (prev, next) = self.neighbours(page);
        match idx {
            PREV_IDX => prev.map(|p| synth(idx, &self.prev_glyph, &p)),
            NEXT_IDX => next.map(|n| synth(idx, &self.next_glyph, &n)),
            _ => None,
        }
    }
}

pub const PREV_IDX: u8 = 10;
pub const NEXT_IDX: u8 = 14;

fn synth(index: u8, glyph: &str, target_page: &str) -> Button {
    Button {
        index,
        label: target_page.to_string(),
        glyph: Some(glyph.to_string()),
        icon: None,
        bg: None,
        fg: None,
        action: format!("page:{}", target_page),
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Page {
    #[serde(default)]
    pub buttons: Vec<Button>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Button {
    pub index: u8,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub glyph: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub bg: Option<String>,
    #[serde(default)]
    pub fg: Option<String>,
    #[serde(default = "default_action")]
    pub action: String,
}

fn default_brightness() -> u8 {
    70
}
fn default_page_name() -> String {
    "main".into()
}
fn default_font_label() -> String {
    "/usr/share/fonts/noto/NotoSans-Regular.ttf".into()
}
fn default_font_glyph() -> String {
    "/usr/share/fonts/TTF/JetBrainsMonoNerdFont-Regular.ttf".into()
}
fn default_text_color() -> String {
    "#ffffff".into()
}
fn default_bg_color() -> String {
    "#1a1a1a".into()
}
fn default_action() -> String {
    "noop".into()
}
fn default_auto_paginate() -> bool {
    true
}
fn default_prev_glyph() -> String {
    "\u{f0141}".into() // mdi chevron-left
}
fn default_next_glyph() -> String {
    "\u{f0142}".into() // mdi chevron-right
}

impl Default for DeckConfig {
    fn default() -> Self {
        let mut pages = BTreeMap::new();
        let mut main = Page::default();
        main.buttons.push(Button {
            index: 0,
            label: "hello".into(),
            glyph: Some("\u{f0a0c}".into()),
            icon: None,
            bg: None,
            fg: None,
            action: "exec:notify-send hello".into(),
        });
        pages.insert("main".into(), main);
        Self {
            brightness: default_brightness(),
            default_page: default_page_name(),
            font_label: default_font_label(),
            font_glyph: default_font_glyph(),
            text_color: default_text_color(),
            bg_color: default_bg_color(),
            auto_paginate: default_auto_paginate(),
            display_off: false,
            prev_glyph: default_prev_glyph(),
            next_glyph: default_next_glyph(),
            page_order: vec!["main".into()],
            pages,
        }
    }
}

impl DeckConfig {
    /// Level to push to the device: zero while the display is switched off.
    pub fn active_brightness(&self) -> u8 {
        if self.display_off { 0 } else { self.brightness }
    }
}
