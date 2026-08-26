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
    if let Err(e) = state::init_run_dir() {
        eprintln!("camlink-ctl: {e}");
        std::process::exit(1);
    }
    std::process::exit(dispatch::run(parsed.cmd));
}
