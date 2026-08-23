use crate::device::pedal::{open_first, Pedal};
use anyhow::Result;
use std::time::Duration;

pub fn open_with_retry() -> Result<Pedal> {
    let mut delay = Duration::from_millis(500);
    loop {
        match open_first() {
            Ok(p) => return Ok(p),
            Err(e) => {
                eprintln!(
                    "streamdeck-ctl: pedal connect failed ({e}); retry in {:?}",
                    delay
                );
                std::thread::sleep(delay);
                if delay < Duration::from_secs(10) {
                    delay = (delay * 2).min(Duration::from_secs(10));
                }
            }
        }
    }
}
