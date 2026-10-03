#[derive(Default)]
pub struct RetryLog {
    last: Option<String>,
}

impl RetryLog {
    pub fn new() -> Self {
        Self::default()
    }

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
        assert!(log.should_log("no Stream Deck Pedal found"));
    }
}
