use super::parsed::ParsedActions;
use crate::action::Action;
use crate::synth::Synth;

pub fn build_optional(parsed: &ParsedActions) -> Option<Synth> {
    let mut wanted: Vec<evdev::KeyCode> = Vec::new();
    for action in parsed.all_actions() {
        if let Action::Key(k) = action
            && !wanted.contains(k) {
                wanted.push(*k);
            }
    }
    if wanted.is_empty() {
        return None;
    }
    match Synth::new_named("streamdeck-ctl-pedal", &wanted) {
        Ok(s) => Some(s),
        Err(e) => {
            eprintln!(
                "streamdeck-ctl: pedal synth disabled ({e}); key actions will be no-ops"
            );
            None
        }
    }
}
