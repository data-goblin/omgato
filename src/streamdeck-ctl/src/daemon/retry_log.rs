//! Rate-limits connect-failure logging.
//!
//! A device that is simply absent fails with the same message on every
//! attempt, and the retry delay is capped at 10s - roughly 8,600 identical
//! lines a day for a Stream Deck Pedal nobody owns. Log a reason the first
//! time it appears and whenever it changes; stay quiet while it repeats, so
//! the retry loop still finds a device that is plugged in later without
//! drowning the journal while it waits.
//!
//! Each call to a reconnect helper builds its own RetryLog, so a failure
//! after a device drops out is reported again even when the reason matches
//! one seen before it first connected.

#[derive(Default)]
pub struct RetryLog {
    last: Option<String>,
}

impl RetryLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// True when this failure is worth a line: the first one, and any time
    /// the reason differs from the one before it.
    pub fn should_log(&mut self, reason: &str) -> bool {
        if self.last.as_deref() == Some(reason) {
            return false;
        }
        self.last = Some(reason.to_owned());
        true
    }
}

#[cfg(test)]
mod tests {
    use super::RetryLog;

    #[test]
    fn logs_the_first_failure() {
        let mut log = RetryLog::new();
        assert!(log.should_log("no Stream Deck Pedal found"));
    }

    #[test]
    fn stays_quiet_while_the_reason_repeats() {
        let mut log = RetryLog::new();
        assert!(log.should_log("no Stream Deck Pedal found"));
        for _ in 0..100 {
            assert!(!log.should_log("no Stream Deck Pedal found"));
        }
    }

    #[test]
    fn logs_again_when_the_reason_changes() {
        let mut log = RetryLog::new();
        assert!(log.should_log("no Stream Deck Pedal found"));
        assert!(log.should_log("permission denied"));
        assert!(!log.should_log("permission denied"));
        // Back to the original reason: still a change from the one before it.
        assert!(log.should_log("no Stream Deck Pedal found"));
    }
}
