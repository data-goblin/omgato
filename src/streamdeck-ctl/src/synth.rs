use anyhow::{Result, anyhow};
use evdev::{uinput::VirtualDevice, AttributeSet, EventType, InputEvent, KeyCode};

pub struct Synth {
    dev: VirtualDevice,
}

impl Synth {
    pub fn new_named(name: &str, keys: &[KeyCode]) -> Result<Self> {
        let mut set = AttributeSet::<KeyCode>::new();
        for k in keys {
            set.insert(*k);
        }
        let dev = VirtualDevice::builder()?
            .name(name)
            .with_keys(&set)?
            .build()?;
        Ok(Self { dev })
    }

    pub fn press(&mut self, key: KeyCode) -> Result<()> {
        self.dev.emit(&[InputEvent::new(EventType::KEY.0, key.0, 1)])?;
        Ok(())
    }

    pub fn release(&mut self, key: KeyCode) -> Result<()> {
        self.dev.emit(&[InputEvent::new(EventType::KEY.0, key.0, 0)])?;
        Ok(())
    }

    pub fn tap(&mut self, key: KeyCode) -> Result<()> {
        self.press(key)?;
        self.release(key)?;
        Ok(())
    }
}

pub fn parse_key(name: &str) -> Result<KeyCode> {
    let upper = name.to_ascii_uppercase();
    let with_prefix = if upper.starts_with("KEY_") || upper.starts_with("BTN_") {
        upper
    } else {
        format!("KEY_{}", upper)
    };
    with_prefix
        .parse()
        .map_err(|_| anyhow!("unknown key name: {}", name))
}
