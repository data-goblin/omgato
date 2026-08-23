use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Registers a SIGHUP handler that flips the returned flag on signal.
/// Daemons poll this flag in their event loop and reload config in-place
/// instead of restarting via systemd.
pub fn install() -> Result<Arc<AtomicBool>> {
    let flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&flag))?;
    Ok(flag)
}

/// Registers a SIGUSR1 handler for a brightness-only refresh: the daemon
/// re-reads the configured level and pushes it, with no page re-render.
pub fn install_brightness() -> Result<Arc<AtomicBool>> {
    let flag = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGUSR1, Arc::clone(&flag))?;
    Ok(flag)
}

/// Returns true and clears the flag if a reload was requested. False otherwise.
pub fn take(flag: &Arc<AtomicBool>) -> bool {
    flag.swap(false, Ordering::SeqCst)
}
