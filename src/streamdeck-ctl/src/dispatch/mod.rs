mod deck;
mod ls;
mod pedal;
mod rules;
mod service;

use crate::cli::Cmd;
use crate::tui;
use anyhow::Result;

pub fn run(cmd: Cmd) -> Result<()> {
    match cmd {
        Cmd::Ls { json } => ls::run(json),
        Cmd::Pedal { cmd } => pedal::dispatch(cmd),
        Cmd::Deck { cmd } => deck::dispatch(cmd),
        Cmd::Tui => tui::run(),
        Cmd::InstallRules => rules::install(),
        Cmd::Enable => service::enable_all(),
        Cmd::Disable => service::disable_all(),
    }
}
