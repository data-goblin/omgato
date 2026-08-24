use crate::cli::{Cli, Cmd};
use crate::{config, discover, dispatch};
use clap::Parser;
use std::io::{self, BufRead, Write};

const HELP: &str = "\
commands:
  ls                              list lights with live state
  discover                        rescan mDNS and refresh cache
  on  [name|all]                  turn on
  off [name|all]                  turn off
  toggle [name|all]               toggle
  brightness <0-100> [name|all]   set brightness percent
  temperature <2900-7000> [n|all] set color temperature (kelvin)
  help                            show this
  quit                            exit
";

pub fn run() -> i32 {
    println!("scanning network for lights...");
    let cache = match discover::run() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    if let Err(e) = config::save(&cache) {
        eprintln!("save failed: {e}");
        return 1;
    }
    if cache.lights.is_empty() {
        eprintln!("no lights found on the network");
        eprintln!("ensure they're powered on and on the same wifi, then run: keylight-ctl setup");
        return 1;
    }
    println!("found {} light(s):", cache.lights.len());
    for l in &cache.lights {
        println!("  {:<20}  {:<16}  {}", l.name, l.ip, l.mac);
    }
    println!();
    println!("entering REPL. type 'help' or 'quit'.");
    repl()
}

fn repl() -> i32 {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();
    loop {
        line.clear();
        print!("elgato> ");
        let _ = io::stdout().flush();
        match handle.read_line(&mut line) {
            Ok(0) => {
                println!();
                break;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("read: {e}");
                break;
            }
        }
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if matches!(t, "quit" | "exit" | "q") {
            break;
        }
        if matches!(t, "help" | "?") {
            print!("{HELP}");
            continue;
        }
        let mut argv: Vec<String> = vec!["keylight-ctl".into()];
        argv.extend(t.split_whitespace().map(String::from));
        match Cli::try_parse_from(&argv) {
            Ok(cli) => {
                if matches!(cli.cmd, Cmd::Setup) {
                    eprintln!("not available in REPL");
                    continue;
                }
                let _ = dispatch::run(cli.cmd, dispatch::Out { json: cli.json });
            }
            Err(e) => {
                eprint!("{e}");
            }
        }
    }
    0
}
