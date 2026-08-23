use crate::config::{Button, Page};
use crate::tui::state::App;

pub fn ensure_button<'a>(app: &'a mut App, page_name: &str, idx: u8) -> &'a mut Button {
    let entry = app
        .cfg
        .deck
        .pages
        .entry(page_name.to_string())
        .or_insert_with(Page::default);
    let pos = entry.buttons.iter().position(|b| b.index == idx);
    match pos {
        Some(p) => &mut entry.buttons[p],
        None => {
            entry.buttons.push(Button {
                index: idx,
                label: String::new(),
                glyph: None,
                icon: None,
                bg: None,
                fg: None,
                action: "noop".into(),
            });
            entry.buttons.last_mut().unwrap()
        }
    }
}
