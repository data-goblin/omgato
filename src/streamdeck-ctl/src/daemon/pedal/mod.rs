mod dispatch;
mod parsed;
mod reconnect;
mod state;
mod synth;

use crate::config::{self, Config, Gesture, PedalPos};
use crate::daemon::reload;
use anyhow::Result;
use elgato_streamdeck::DeviceStateUpdate;
use parsed::ParsedActions;
use state::{Detector, PedalState};
use std::time::{Duration, Instant};

const POLL_TIMEOUT: Duration = Duration::from_millis(30);
const RECONNECT_AFTER_ERRORS: u32 = 5;

pub fn run(cfg: &Config, emit_keys: bool) -> Result<()> {
    let mut pedal = reconnect::open_with_retry()?;
    eprintln!(
        "streamdeck-ctl: pedal connected serial={} long_ms={} double_ms={}",
        pedal.serial, cfg.pedal.long_ms, cfg.pedal.double_ms
    );

    let mut cfg = cfg.clone();
    let mut actions = ParsedActions::from_config(&cfg.pedal)?;
    let mut detector = Detector::new(cfg.pedal.long_ms, cfg.pedal.double_ms);
    let mut synth = if emit_keys {
        synth::build_optional(&actions)
    } else {
        None
    };
    let mut states = [PedalState::Idle; 3];
    let reload_flag = reload::install()?;

    if !emit_keys {
        eprintln!("streamdeck-ctl: pedal test mode (events only, no key emit, no exec)");
    }

    let mut reader = pedal.deck.get_reader();
    let mut consecutive_errs: u32 = 0;
    loop {
        if reload::take(&reload_flag) {
            apply_reload(&mut cfg, &mut actions, &mut detector, &mut synth, emit_keys);
        }
        match reader.read(Some(POLL_TIMEOUT)) {
            Ok(updates) => {
                consecutive_errs = 0;
                for u in updates {
                    handle_update(&u, &mut states, &detector, &actions, emit_keys, &mut synth);
                }
            }
            Err(e) => {
                let s = e.to_string();
                if !is_timeout(&s) {
                    consecutive_errs += 1;
                    eprintln!(
                        "streamdeck-ctl: pedal read error ({}/{RECONNECT_AFTER_ERRORS}): {e}",
                        consecutive_errs
                    );
                    if consecutive_errs >= RECONNECT_AFTER_ERRORS {
                        eprintln!("streamdeck-ctl: pedal reconnecting");
                        states = [PedalState::Idle; 3];
                        pedal = reconnect::open_with_retry()?;
                        reader = pedal.deck.get_reader();
                        consecutive_errs = 0;
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
        run_ticks(&mut states, &detector, &actions, emit_keys, &mut synth);
    }
}

fn apply_reload(
    cfg: &mut Config,
    actions: &mut ParsedActions,
    detector: &mut Detector,
    synth: &mut Option<crate::synth::Synth>,
    emit_keys: bool,
) {
    eprintln!("streamdeck-ctl: pedal reloading config (SIGHUP)");
    let new_cfg = match config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("streamdeck-ctl: pedal reload failed reading config: {e}");
            return;
        }
    };
    let new_actions = match ParsedActions::from_config(&new_cfg.pedal) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("streamdeck-ctl: pedal reload failed parsing actions: {e}");
            return;
        }
    };
    *cfg = new_cfg;
    *actions = new_actions;
    *detector = Detector::new(cfg.pedal.long_ms, cfg.pedal.double_ms);
    if emit_keys {
        *synth = synth::build_optional(actions);
    }
    eprintln!("streamdeck-ctl: pedal config reloaded");
}

fn is_timeout(msg: &str) -> bool {
    let m = msg.to_ascii_lowercase();
    m.contains("timeout") || m.contains("timed out") || m.contains("interrupted")
}

fn handle_update(
    update: &DeviceStateUpdate,
    states: &mut [PedalState; 3],
    detector: &Detector,
    actions: &ParsedActions,
    emit_keys: bool,
    synth: &mut Option<crate::synth::Synth>,
) {
    let now = Instant::now();
    match update {
        DeviceStateUpdate::ButtonDown(idx) => {
            let Some(pos) = PedalPos::from_index(*idx as usize) else {
                return;
            };
            let g = detector.on_down(&mut states[*idx as usize], now);
            log_edge(pos, "down");
            if let Some(g) = g {
                fire(pos, g, actions, emit_keys, synth);
            }
        }
        DeviceStateUpdate::ButtonUp(idx) => {
            let Some(pos) = PedalPos::from_index(*idx as usize) else {
                return;
            };
            let g = detector.on_up(&mut states[*idx as usize], now);
            log_edge(pos, "up");
            if let Some(g) = g {
                fire(pos, g, actions, emit_keys, synth);
            }
        }
        _ => {}
    }
}

fn run_ticks(
    states: &mut [PedalState; 3],
    detector: &Detector,
    actions: &ParsedActions,
    emit_keys: bool,
    synth: &mut Option<crate::synth::Synth>,
) {
    let now = Instant::now();
    for i in 0..3 {
        let Some(pos) = PedalPos::from_index(i) else {
            continue;
        };
        if let Some(g) = detector.tick(&mut states[i], now) {
            fire(pos, g, actions, emit_keys, synth);
        }
    }
}

fn fire(
    pos: PedalPos,
    g: Gesture,
    actions: &ParsedActions,
    emit_keys: bool,
    synth: &mut Option<crate::synth::Synth>,
) {
    let action = actions.get(pos, g);
    eprintln!(
        "streamdeck-ctl: {} {} -> {:?}",
        pos.label(),
        g.label(),
        action
    );
    if !emit_keys {
        return;
    }
    dispatch::fire(action, synth);
}

fn log_edge(pos: PedalPos, edge: &str) {
    eprintln!("streamdeck-ctl: {} {}", pos.label(), edge);
}
