use crate::config::Gesture;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub enum PedalState {
    Idle,
    Pressed { since: Instant },
    AwaitDouble { since: Instant },
    Held,
}

pub struct Detector {
    pub long: Duration,
    pub double: Duration,
}

impl Detector {
    pub fn new(long_ms: u64, double_ms: u64) -> Self {
        Self {
            long: Duration::from_millis(long_ms),
            double: Duration::from_millis(double_ms),
        }
    }

    pub fn on_down(&self, state: &mut PedalState, now: Instant) -> Option<Gesture> {
        match *state {
            PedalState::Idle => {
                *state = PedalState::Pressed { since: now };
                None
            }
            PedalState::AwaitDouble { .. } => {
                *state = PedalState::Held;
                Some(Gesture::Double)
            }
            _ => None,
        }
    }

    pub fn on_up(&self, state: &mut PedalState, now: Instant) -> Option<Gesture> {
        match *state {
            PedalState::Pressed { since } => {
                if now.saturating_duration_since(since) >= self.long {
                    *state = PedalState::Idle;
                    Some(Gesture::Long)
                } else {
                    *state = PedalState::AwaitDouble { since: now };
                    None
                }
            }
            PedalState::Held => {
                *state = PedalState::Idle;
                None
            }
            _ => None,
        }
    }

    pub fn tick(&self, state: &mut PedalState, now: Instant) -> Option<Gesture> {
        match *state {
            PedalState::Pressed { since }
                if now.saturating_duration_since(since) >= self.long =>
            {
                *state = PedalState::Held;
                Some(Gesture::Long)
            }
            PedalState::AwaitDouble { since }
                if now.saturating_duration_since(since) >= self.double =>
            {
                *state = PedalState::Idle;
                Some(Gesture::Tap)
            }
            _ => None,
        }
    }
}
