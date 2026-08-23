use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PedalConfig {
    #[serde(default)]
    pub left: PedalActions,
    #[serde(default)]
    pub center: PedalActions,
    #[serde(default)]
    pub right: PedalActions,
    #[serde(default = "default_long_ms")]
    pub long_ms: u64,
    #[serde(default = "default_double_ms")]
    pub double_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PedalActions {
    #[serde(default = "noop")]
    pub tap: String,
    #[serde(default = "noop")]
    pub long: String,
    #[serde(default = "noop")]
    pub double: String,
}

impl Default for PedalActions {
    fn default() -> Self {
        Self {
            tap: noop(),
            long: noop(),
            double: noop(),
        }
    }
}

impl Default for PedalConfig {
    fn default() -> Self {
        Self {
            left: PedalActions {
                tap: "key:F13".into(),
                long: noop(),
                double: noop(),
            },
            center: PedalActions {
                tap: "key:F14".into(),
                long: noop(),
                double: noop(),
            },
            right: PedalActions {
                tap: "key:F15".into(),
                long: noop(),
                double: noop(),
            },
            long_ms: default_long_ms(),
            double_ms: default_double_ms(),
        }
    }
}

fn noop() -> String {
    "noop".into()
}
fn default_long_ms() -> u64 {
    400
}
fn default_double_ms() -> u64 {
    250
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PedalPos {
    Left,
    Center,
    Right,
}

impl PedalPos {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "l" | "left" => Ok(PedalPos::Left),
            "c" | "center" | "centre" | "middle" | "mid" => Ok(PedalPos::Center),
            "r" | "right" => Ok(PedalPos::Right),
            other => anyhow::bail!("unknown pedal position: {}", other),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            PedalPos::Left => "left",
            PedalPos::Center => "center",
            PedalPos::Right => "right",
        }
    }
    pub fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(PedalPos::Left),
            1 => Some(PedalPos::Center),
            2 => Some(PedalPos::Right),
            _ => None,
        }
    }
    pub const ALL: [PedalPos; 3] = [PedalPos::Left, PedalPos::Center, PedalPos::Right];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    Tap,
    Long,
    Double,
}

impl Gesture {
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "t" | "tap" | "press" => Ok(Gesture::Tap),
            "l" | "long" | "hold" => Ok(Gesture::Long),
            "d" | "dbl" | "double" => Ok(Gesture::Double),
            other => anyhow::bail!("unknown gesture: {}", other),
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Gesture::Tap => "tap",
            Gesture::Long => "long",
            Gesture::Double => "double",
        }
    }
    pub const ALL: [Gesture; 3] = [Gesture::Tap, Gesture::Long, Gesture::Double];
}

impl PedalConfig {
    pub fn actions(&self, pos: PedalPos) -> &PedalActions {
        match pos {
            PedalPos::Left => &self.left,
            PedalPos::Center => &self.center,
            PedalPos::Right => &self.right,
        }
    }

    pub fn actions_mut(&mut self, pos: PedalPos) -> &mut PedalActions {
        match pos {
            PedalPos::Left => &mut self.left,
            PedalPos::Center => &mut self.center,
            PedalPos::Right => &mut self.right,
        }
    }

    pub fn get(&self, pos: PedalPos, g: Gesture) -> &str {
        let a = self.actions(pos);
        match g {
            Gesture::Tap => &a.tap,
            Gesture::Long => &a.long,
            Gesture::Double => &a.double,
        }
    }

    pub fn set(&mut self, pos: PedalPos, g: Gesture, action: String) {
        let a = self.actions_mut(pos);
        match g {
            Gesture::Tap => a.tap = action,
            Gesture::Long => a.long = action,
            Gesture::Double => a.double = action,
        }
    }
}
