use crate::action::Action;
use crate::synth::Synth;
use std::process::{Command, Stdio};

pub fn fire(action: &Action, synth: &mut Option<Synth>) {
    match action {
        Action::Noop => {}
        Action::Key(k) => {
            if let Some(s) = synth.as_mut()
                && let Err(e) = s.tap(*k) {
                    eprintln!("streamdeck-ctl: pedal key emit failed: {e}");
                }
        }
        Action::Exec(cmd) => {
            // Backgrounded in the shell so nothing is left to reap here.
            let _ = Command::new("sh")
                .arg("-c")
                .arg(format!("{cmd} &"))
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
        }
        Action::Page(_) | Action::Back => {
            eprintln!("streamdeck-ctl: pedal: page/back actions ignored (no pages on pedal)");
        }
    }
}
