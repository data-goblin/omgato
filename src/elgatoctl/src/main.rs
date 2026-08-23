use clap::Parser;

mod cli;
mod config;
mod discover;
mod dispatch;
mod light;
mod setup;
mod waybar;

fn main() {
    let parsed = cli::Cli::parse();
    let out = dispatch::Out { json: parsed.json };
    std::process::exit(dispatch::run(parsed.cmd, out));
}
