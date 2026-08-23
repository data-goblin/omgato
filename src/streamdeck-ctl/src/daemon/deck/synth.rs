use super::parsed::ParsedPage;
use crate::action::Action;
use crate::synth::Synth;
use std::collections::HashMap;

pub fn build_optional(pages: &HashMap<String, ParsedPage>) -> Option<Synth> {
    let mut wanted: Vec<evdev::KeyCode> = Vec::new();
    for p in pages.values() {
        for b in &p.buttons {
            if let Action::Key(k) = &b.action
                && !wanted.contains(k) {
                    wanted.push(*k);
                }
        }
    }
    if wanted.is_empty() {
        return None;
    }
    match Synth::new_named("streamdeck-ctl-deck", &wanted) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!("streamdeck-ctl: deck synth disabled ({e}); key actions will be no-ops");
            None
        }
    }
}
