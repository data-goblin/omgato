use crate::synth::{parse_key, Synth};
use anyhow::Result;
use evdev::KeyCode;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub enum Action {
    Noop,
    Exec(String),
    Key(KeyCode),
    Page(String),
    Back,
}

pub fn parse(spec: &str) -> Result<Action> {
    let s = spec.trim();
    if s.is_empty() || s == "noop" {
        return Ok(Action::Noop);
    }
    if s == "back" {
        return Ok(Action::Back);
    }
    if let Some(rest) = s.strip_prefix("exec:") {
        return Ok(Action::Exec(rest.trim().to_string()));
    }
    if let Some(rest) = s.strip_prefix("key:") {
        return Ok(Action::Key(parse_key(rest.trim())?));
    }
    if let Some(rest) = s.strip_prefix("page:") {
        return Ok(Action::Page(rest.trim().to_string()));
    }
    anyhow::bail!("unknown action spec: {}", spec)
}

pub enum Outcome {
    None,
    GotoPage(String),
    Back,
}

pub fn dispatch(action: &Action, synth: &mut Option<Synth>) -> Result<Outcome> {
    match action {
        Action::Noop => Ok(Outcome::None),
        Action::Exec(cmd) => {
            spawn_shell(cmd)?;
            Ok(Outcome::None)
        }
        Action::Key(k) => {
            if let Some(s) = synth.as_mut() {
                s.tap(*k)?;
            } else {
                eprintln!("streamdeck-ctl: key action but no uinput synth available");
            }
            Ok(Outcome::None)
        }
        Action::Page(p) => Ok(Outcome::GotoPage(p.clone())),
        Action::Back => Ok(Outcome::Back),
    }
}

fn spawn_shell(cmd: &str) -> Result<()> {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActionKind {
    Key,
    Exec,
    Page,
    Back,
    Noop,
}

pub fn split(spec: &str) -> (ActionKind, String) {
    let s = spec.trim();
    if s.is_empty() || s == "noop" {
        return (ActionKind::Noop, String::new());
    }
    if s == "back" {
        return (ActionKind::Back, String::new());
    }
    if let Some(rest) = s.strip_prefix("key:") {
        return (ActionKind::Key, rest.trim().to_string());
    }
    if let Some(rest) = s.strip_prefix("exec:") {
        return (ActionKind::Exec, rest.trim().to_string());
    }
    if let Some(rest) = s.strip_prefix("page:") {
        return (ActionKind::Page, rest.trim().to_string());
    }
    (ActionKind::Key, s.to_string())
}

pub fn join(kind: ActionKind, detail: &str) -> String {
    match kind {
        ActionKind::Noop => "noop".into(),
        ActionKind::Back => "back".into(),
        ActionKind::Key => format!("key:{}", detail.trim()),
        ActionKind::Exec => format!("exec:{}", detail.trim()),
        ActionKind::Page => format!("page:{}", detail.trim()),
    }
}
