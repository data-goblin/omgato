use crate::action::{self, Action};
use crate::config::{Gesture, PedalConfig, PedalPos};
use anyhow::{Context, Result};

pub struct ParsedActions {
    left: PedalSlots,
    center: PedalSlots,
    right: PedalSlots,
}

struct PedalSlots {
    tap: Action,
    long: Action,
    double: Action,
}

impl ParsedActions {
    pub fn from_config(cfg: &PedalConfig) -> Result<Self> {
        Ok(Self {
            left: PedalSlots::parse(&cfg.left)?,
            center: PedalSlots::parse(&cfg.center)?,
            right: PedalSlots::parse(&cfg.right)?,
        })
    }

    pub fn get(&self, pos: PedalPos, g: Gesture) -> &Action {
        let slots = match pos {
            PedalPos::Left => &self.left,
            PedalPos::Center => &self.center,
            PedalPos::Right => &self.right,
        };
        match g {
            Gesture::Tap => &slots.tap,
            Gesture::Long => &slots.long,
            Gesture::Double => &slots.double,
        }
    }

    pub fn all_actions(&self) -> [&Action; 9] {
        [
            &self.left.tap, &self.left.long, &self.left.double,
            &self.center.tap, &self.center.long, &self.center.double,
            &self.right.tap, &self.right.long, &self.right.double,
        ]
    }
}

impl PedalSlots {
    fn parse(a: &crate::config::PedalActions) -> Result<Self> {
        Ok(Self {
            tap: action::parse(&a.tap).context("tap action")?,
            long: action::parse(&a.long).context("long action")?,
            double: action::parse(&a.double).context("double action")?,
        })
    }
}
