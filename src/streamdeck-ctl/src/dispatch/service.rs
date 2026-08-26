use crate::units;
use anyhow::Result;
use std::process::Command;

pub fn run(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl").args(args).status()?;
    if !status.success() {
        anyhow::bail!("systemctl {:?} failed", args);
    }
    Ok(())
}

pub fn reload(unit: &str) -> Result<()> {
    if !is_active(unit) {
        return Ok(());
    }
    run(&["--user", "kill", "--signal=SIGHUP", unit])
}

pub fn refresh_brightness(unit: &str) -> Result<()> {
    if !is_active(unit) {
        return Ok(());
    }
    run(&["--user", "kill", "--signal=SIGUSR1", unit])
}

fn is_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", "--quiet", unit])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub fn enable_all() -> Result<()> {
    run(&["--user", "enable", "--now", units::PEDAL_SERVICE])?;
    run(&["--user", "enable", "--now", units::DECK_SERVICE])
}

pub fn disable_all() -> Result<()> {
    let _ = run(&["--user", "disable", "--now", units::PEDAL_SERVICE]);
    run(&["--user", "disable", "--now", units::DECK_SERVICE])
}

