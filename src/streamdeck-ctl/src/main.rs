mod action;
mod cli;
mod config;
mod daemon;
mod device;
mod dispatch;
mod export;
mod pipe;
mod render;
mod synth;
mod tui;
mod units;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    let cmd = cli::Cli::parse().cmd;
    if !cmd.is_daemon() {
        pipe::quiet_on_broken_pipe();
    }
    dispatch::run(cmd)
}
