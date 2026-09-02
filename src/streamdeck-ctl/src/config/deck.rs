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
    pub fn ordered_pages(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .page_order
            .iter()
            .filter(|n| self.pages.contains_key(*n))
            .cloned()
            .collect();
        let rest: Vec<String> = self.pages.keys().filter(|n| !names.contains(n)).cloned().collect();
        names.extend(rest);
        names
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

    /// Move the button at `from` on `page` to index `to` on `dest`, swapping
    /// with whatever already sits there. Nothing is changed unless the whole
    /// move succeeds.
    pub fn move_button(&mut self, page: &str, from: u8, dest: &str, to: u8) -> Result<(), String> {
        if page == dest && from == to {
            return Ok(());
        }
        for name in [page, dest] {
            if !self.pages.contains_key(name) {
                return Err(format!("page '{name}' does not exist"));
            }
        }
        let mut moved = self
            .take_button(page, from)
            .ok_or_else(|| format!("no button at index {from} on page '{page}'"))?;
        let displaced = self.take_button(dest, to);
        moved.index = to;
        self.put_button(dest, moved);
        if let Some(mut displaced) = displaced {
            displaced.index = from;
            self.put_button(page, displaced);
        }
        Ok(())
    }

    pub fn has_button(&self, page: &str, index: u8) -> bool {
        self.pages
            .get(page)
            .is_some_and(|p| p.buttons.iter().any(|b| b.index == index))
    }

    fn take_button(&mut self, page: &str, index: u8) -> Option<Button> {
        let entry = self.pages.get_mut(page)?;
        let at = entry.buttons.iter().position(|b| b.index == index)?;
        Some(entry.buttons.remove(at))
    }

    fn put_button(&mut self, page: &str, button: Button) {
        let entry = self.pages.entry(page.to_owned()).or_default();
        entry.buttons.push(button);
        entry.buttons.sort_by_key(|b| b.index);
    }

    pub fn synthetic_button(&self, page: &str, idx: u8, key_count: u8, cols: u8) -> Option<Button> {
        let (prev_idx, next_idx) = pagination_indices(key_count, cols)?;
        let (prev, next) = self.neighbours(page);
        if idx == prev_idx {
            return prev.map(|p| synth(idx, &self.prev_glyph, &p));
        }
        if idx == next_idx {
            return next.map(|n| synth(idx, &self.next_glyph, &n));
        }
        None
    }
}

pub fn pagination_indices(key_count: u8, cols: u8) -> Option<(u8, u8)> {
    if key_count == 0 || cols == 0 || key_count < cols {
        return None;
    }
    Some((key_count - cols, key_count - 1))
}

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
    "\u{f0141}".into()
}
fn default_next_glyph() -> String {
    "\u{f0142}".into()
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
    pub fn active_brightness(&self) -> u8 {
        if self.display_off { 0 } else { self.brightness }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(index: u8, label: &str) -> Button {
        Button {
            index,
            label: label.to_owned(),
            glyph: Some("g".to_owned()),
            icon: None,
            bg: None,
            fg: None,
            action: format!("exec:{label}"),
        }
    }

    fn config(pages: &[(&str, &[u8])]) -> DeckConfig {
        let mut cfg = DeckConfig::default();
        cfg.pages.clear();
        for (name, indices) in pages {
            let buttons = indices.iter().map(|i| button(*i, &format!("b{i}"))).collect();
            cfg.pages.insert((*name).to_owned(), Page { buttons });
        }
        cfg
    }

    fn labels(cfg: &DeckConfig, page: &str) -> Vec<(u8, String)> {
        cfg.pages[page]
            .buttons
            .iter()
            .map(|b| (b.index, b.label.clone()))
            .collect()
    }

    #[test]
    fn moves_a_button_onto_a_free_index() {
        let mut cfg = config(&[("main", &[0, 1])]);
        cfg.move_button("main", 0, "main", 7).unwrap();
        assert_eq!(
            labels(&cfg, "main"),
            vec![(1, "b1".to_owned()), (7, "b0".to_owned())]
        );
    }

    #[test]
    fn swaps_when_the_destination_is_taken() {
        let mut cfg = config(&[("main", &[0, 1])]);
        cfg.move_button("main", 0, "main", 1).unwrap();
        assert_eq!(
            labels(&cfg, "main"),
            vec![(0, "b1".to_owned()), (1, "b0".to_owned())]
        );
    }

    #[test]
    fn carries_every_field_across_the_move() {
        let mut cfg = config(&[("main", &[3])]);
        cfg.move_button("main", 3, "main", 9).unwrap();
        let moved = &cfg.pages["main"].buttons[0];
        assert_eq!(moved.index, 9);
        assert_eq!(moved.label, "b3");
        assert_eq!(moved.glyph.as_deref(), Some("g"));
        assert_eq!(moved.action, "exec:b3");
    }

    #[test]
    fn moves_and_swaps_across_pages() {
        let mut cfg = config(&[("one", &[0]), ("two", &[4])]);
        cfg.move_button("one", 0, "two", 4).unwrap();
        assert_eq!(labels(&cfg, "one"), vec![(0, "b4".to_owned())]);
        assert_eq!(labels(&cfg, "two"), vec![(4, "b0".to_owned())]);
    }

    #[test]
    fn moving_onto_itself_changes_nothing() {
        let mut cfg = config(&[("main", &[2])]);
        cfg.move_button("main", 2, "main", 2).unwrap();
        assert_eq!(labels(&cfg, "main"), vec![(2, "b2".to_owned())]);
    }

    #[test]
    fn keeps_buttons_ordered_by_index() {
        let mut cfg = config(&[("main", &[0, 5, 9])]);
        cfg.move_button("main", 9, "main", 1).unwrap();
        let indices: Vec<u8> = cfg.pages["main"].buttons.iter().map(|b| b.index).collect();
        assert_eq!(indices, vec![0, 1, 5]);
    }

    #[test]
    fn refuses_an_empty_source_without_touching_anything() {
        let mut cfg = config(&[("main", &[0])]);
        let before = labels(&cfg, "main");
        assert!(cfg.move_button("main", 6, "main", 0).is_err());
        assert_eq!(labels(&cfg, "main"), before);
    }

    #[test]
    fn refuses_a_page_that_does_not_exist() {
        let mut cfg = config(&[("main", &[0])]);
        let before = labels(&cfg, "main");
        assert!(cfg.move_button("main", 0, "ghost", 0).is_err());
        assert!(cfg.move_button("ghost", 0, "main", 0).is_err());
        assert_eq!(labels(&cfg, "main"), before);
    }

    #[test]
    fn has_button_reports_occupancy() {
        let cfg = config(&[("main", &[0, 4])]);
        assert!(cfg.has_button("main", 4));
        assert!(!cfg.has_button("main", 1));
        assert!(!cfg.has_button("ghost", 0));
    }
}
