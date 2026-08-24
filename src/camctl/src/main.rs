use clap::Parser;

mod blocker;
mod cli;
mod config;
mod dispatch;
mod holder;
mod hypr;
mod obs;
mod overlay;
mod positioning;
mod reset;
mod state;
mod status;

fn main() {
    let parsed = cli::Cli::parse();
    std::process::exit(dispatch::run(parsed.cmd));
}
