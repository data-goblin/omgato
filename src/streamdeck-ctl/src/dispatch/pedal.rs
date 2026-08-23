use super::service;
use crate::action;
use crate::cli::PedalCmd;
use crate::config::{self, Gesture, PedalPos};
use crate::{daemon, waybar};
use anyhow::Result;

pub fn dispatch(cmd: PedalCmd) -> Result<()> {
    match cmd {
        PedalCmd::Test => daemon::pedal::run(&config::load()?, false),
        PedalCmd::Run => daemon::pedal::run(&config::load()?, true),
        PedalCmd::Show => show(),
        PedalCmd::Get { position, gesture } => get(&position, &gesture),
        PedalCmd::Set { position, gesture, action } => set(&position, &gesture, action),
        PedalCmd::Reload => service::reload(waybar::PEDAL_SERVICE),
    }
}

fn show() -> Result<()> {
    let cfg = config::load()?;
    println!("long_ms={}  double_ms={}", cfg.pedal.long_ms, cfg.pedal.double_ms);
    println!();
    println!("{:<7}  {:<7}  {}", "pedal", "gesture", "action");
    for pos in PedalPos::ALL {
        for g in Gesture::ALL {
            println!(
                "{:<7}  {:<7}  {}",
                pos.label(),
                g.label(),
                cfg.pedal.get(pos, g)
            );
        }
    }
    Ok(())
}

fn get(position: &str, gesture: &str) -> Result<()> {
    let cfg = config::load()?;
    let pos = PedalPos::parse(position)?;
    let g = Gesture::parse(gesture)?;
    println!("{}", cfg.pedal.get(pos, g));
    Ok(())
}

fn set(position: &str, gesture: &str, action_spec: String) -> Result<()> {
    let pos = PedalPos::parse(position)?;
    let g = Gesture::parse(gesture)?;
    action::parse(&action_spec)?;
    let mut cfg = config::load()?;
    cfg.pedal.set(pos, g, action_spec);
    config::save(&cfg)?;
    let _ = service::reload(waybar::PEDAL_SERVICE);
    Ok(())
}
