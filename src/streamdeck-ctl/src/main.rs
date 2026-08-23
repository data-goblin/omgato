mod action;
mod cli;
mod config;
mod daemon;
mod device;
mod dispatch;
mod export;
mod render;
mod synth;
mod tui;
mod units;

use clap::Parser;

fn main() -> anyhow::Result<()> {
    dispatch::run(cli::Cli::parse().cmd)
}
