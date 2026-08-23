use crate::device::deck::{open_first, Deck};
use anyhow::Result;
use std::time::Duration;

pub fn open_with_retry() -> Result<Deck> {
    let mut delay = Duration::from_millis(500);
    loop {
        match open_first() {
            Ok(d) => return Ok(d),
            Err(e) => {
                eprintln!(
                    "streamdeck-ctl: deck connect failed ({e}); retry in {:?}",
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
