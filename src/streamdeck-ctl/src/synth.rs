use anyhow::{anyhow, Result};
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
    keycode_from_str(&with_prefix).ok_or_else(|| anyhow!("unknown key name: {}", name))
}

fn keycode_from_str(s: &str) -> Option<KeyCode> {
    Some(match s {
        "KEY_F1" => KeyCode::KEY_F1,
        "KEY_F2" => KeyCode::KEY_F2,
        "KEY_F3" => KeyCode::KEY_F3,
        "KEY_F4" => KeyCode::KEY_F4,
        "KEY_F5" => KeyCode::KEY_F5,
        "KEY_F6" => KeyCode::KEY_F6,
        "KEY_F7" => KeyCode::KEY_F7,
        "KEY_F8" => KeyCode::KEY_F8,
        "KEY_F9" => KeyCode::KEY_F9,
        "KEY_F10" => KeyCode::KEY_F10,
        "KEY_F11" => KeyCode::KEY_F11,
        "KEY_F12" => KeyCode::KEY_F12,
        "KEY_F13" => KeyCode::KEY_F13,
        "KEY_F14" => KeyCode::KEY_F14,
        "KEY_F15" => KeyCode::KEY_F15,
        "KEY_F16" => KeyCode::KEY_F16,
        "KEY_F17" => KeyCode::KEY_F17,
        "KEY_F18" => KeyCode::KEY_F18,
        "KEY_F19" => KeyCode::KEY_F19,
        "KEY_F20" => KeyCode::KEY_F20,
        "KEY_F21" => KeyCode::KEY_F21,
        "KEY_F22" => KeyCode::KEY_F22,
        "KEY_F23" => KeyCode::KEY_F23,
        "KEY_F24" => KeyCode::KEY_F24,
        "KEY_A" => KeyCode::KEY_A,
        "KEY_B" => KeyCode::KEY_B,
        "KEY_C" => KeyCode::KEY_C,
        "KEY_D" => KeyCode::KEY_D,
        "KEY_E" => KeyCode::KEY_E,
        "KEY_F" => KeyCode::KEY_F,
        "KEY_G" => KeyCode::KEY_G,
        "KEY_H" => KeyCode::KEY_H,
        "KEY_I" => KeyCode::KEY_I,
        "KEY_J" => KeyCode::KEY_J,
        "KEY_K" => KeyCode::KEY_K,
        "KEY_L" => KeyCode::KEY_L,
        "KEY_M" => KeyCode::KEY_M,
        "KEY_N" => KeyCode::KEY_N,
        "KEY_O" => KeyCode::KEY_O,
        "KEY_P" => KeyCode::KEY_P,
        "KEY_Q" => KeyCode::KEY_Q,
        "KEY_R" => KeyCode::KEY_R,
        "KEY_S" => KeyCode::KEY_S,
        "KEY_T" => KeyCode::KEY_T,
        "KEY_U" => KeyCode::KEY_U,
        "KEY_V" => KeyCode::KEY_V,
        "KEY_W" => KeyCode::KEY_W,
        "KEY_X" => KeyCode::KEY_X,
        "KEY_Y" => KeyCode::KEY_Y,
        "KEY_Z" => KeyCode::KEY_Z,
        "KEY_0" => KeyCode::KEY_0,
        "KEY_1" => KeyCode::KEY_1,
        "KEY_2" => KeyCode::KEY_2,
        "KEY_3" => KeyCode::KEY_3,
        "KEY_4" => KeyCode::KEY_4,
        "KEY_5" => KeyCode::KEY_5,
        "KEY_6" => KeyCode::KEY_6,
        "KEY_7" => KeyCode::KEY_7,
        "KEY_8" => KeyCode::KEY_8,
        "KEY_9" => KeyCode::KEY_9,
        "KEY_ESC" => KeyCode::KEY_ESC,
        "KEY_ENTER" => KeyCode::KEY_ENTER,
        "KEY_TAB" => KeyCode::KEY_TAB,
        "KEY_SPACE" => KeyCode::KEY_SPACE,
        "KEY_BACKSPACE" => KeyCode::KEY_BACKSPACE,
        "KEY_DELETE" => KeyCode::KEY_DELETE,
        "KEY_INSERT" => KeyCode::KEY_INSERT,
        "KEY_HOME" => KeyCode::KEY_HOME,
        "KEY_END" => KeyCode::KEY_END,
        "KEY_PAGEUP" => KeyCode::KEY_PAGEUP,
        "KEY_PAGEDOWN" => KeyCode::KEY_PAGEDOWN,
        "KEY_LEFT" => KeyCode::KEY_LEFT,
        "KEY_RIGHT" => KeyCode::KEY_RIGHT,
        "KEY_UP" => KeyCode::KEY_UP,
        "KEY_DOWN" => KeyCode::KEY_DOWN,
        "KEY_LEFTSHIFT" => KeyCode::KEY_LEFTSHIFT,
        "KEY_RIGHTSHIFT" => KeyCode::KEY_RIGHTSHIFT,
        "KEY_LEFTCTRL" => KeyCode::KEY_LEFTCTRL,
        "KEY_RIGHTCTRL" => KeyCode::KEY_RIGHTCTRL,
        "KEY_LEFTALT" => KeyCode::KEY_LEFTALT,
        "KEY_RIGHTALT" => KeyCode::KEY_RIGHTALT,
        "KEY_LEFTMETA" => KeyCode::KEY_LEFTMETA,
        "KEY_RIGHTMETA" => KeyCode::KEY_RIGHTMETA,
        "KEY_CAPSLOCK" => KeyCode::KEY_CAPSLOCK,
        "KEY_PLAYPAUSE" => KeyCode::KEY_PLAYPAUSE,
        "KEY_NEXTSONG" => KeyCode::KEY_NEXTSONG,
        "KEY_PREVIOUSSONG" => KeyCode::KEY_PREVIOUSSONG,
        "KEY_STOP" => KeyCode::KEY_STOP,
        "KEY_PLAY" => KeyCode::KEY_PLAY,
        "KEY_PAUSE" => KeyCode::KEY_PAUSE,
        "KEY_MUTE" => KeyCode::KEY_MUTE,
        "KEY_MICMUTE" => KeyCode::KEY_MICMUTE,
        "KEY_VOLUMEUP" => KeyCode::KEY_VOLUMEUP,
        "KEY_VOLUMEDOWN" => KeyCode::KEY_VOLUMEDOWN,
        "KEY_BRIGHTNESSUP" => KeyCode::KEY_BRIGHTNESSUP,
        "KEY_BRIGHTNESSDOWN" => KeyCode::KEY_BRIGHTNESSDOWN,
        "KEY_MINUS" => KeyCode::KEY_MINUS,
        "KEY_EQUAL" => KeyCode::KEY_EQUAL,
        "KEY_LEFTBRACE" => KeyCode::KEY_LEFTBRACE,
        "KEY_RIGHTBRACE" => KeyCode::KEY_RIGHTBRACE,
        "KEY_SEMICOLON" => KeyCode::KEY_SEMICOLON,
        "KEY_APOSTROPHE" => KeyCode::KEY_APOSTROPHE,
        "KEY_GRAVE" => KeyCode::KEY_GRAVE,
        "KEY_BACKSLASH" => KeyCode::KEY_BACKSLASH,
        "KEY_COMMA" => KeyCode::KEY_COMMA,
        "KEY_DOT" => KeyCode::KEY_DOT,
        "KEY_SLASH" => KeyCode::KEY_SLASH,
        _ => return None,
    })
}
