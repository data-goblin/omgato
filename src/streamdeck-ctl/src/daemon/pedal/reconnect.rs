use crate::daemon::retry_log::RetryLog;
use crate::device::pedal::{Pedal, open_first};
use anyhow::Result;
use std::time::Duration;

const MAX_DELAY: Duration = Duration::from_secs(10);

pub fn open_with_retry() -> Result<Pedal> {
    let mut delay = Duration::from_millis(500);
    let mut log = RetryLog::new();
    loop {
        match open_first() {
            Ok(p) => return Ok(p),
            Err(e) => {
                let reason = e.to_string();
                if log.should_log(&reason) {
                    eprintln!(
                        "streamdeck-ctl: pedal connect failed ({reason}); \
                         retrying in {delay:?}, doubling up to {MAX_DELAY:?}, until it appears"
                    );
                }
                std::thread::sleep(delay);
                if delay < MAX_DELAY {
                    delay = (delay * 2).min(MAX_DELAY);
                }
            }
        }
    }
}
