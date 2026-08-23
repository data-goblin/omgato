use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "elgatoctl", about = "Control Elgato Key Lights")]
pub struct Cli {
    /// Emit machine-readable JSON instead of tab-separated text
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Discover lights and drop into an interactive REPL
    Setup,
    /// Discover lights via mDNS and merge into the cache
    Discover {
        /// Drop cached lights that did not answer this scan
        #[arg(long)]
        prune: bool,
    },
    /// List cached lights with live state
    Ls,
    /// Turn on
    On {
        #[arg(default_value = "all")]
        target: String,
    },
    /// Turn off
    Off {
        #[arg(default_value = "all")]
        target: String,
    },
    /// Toggle (off if any on, else on)
    Toggle {
        #[arg(default_value = "all")]
        target: String,
    },
    /// Set brightness 0-100, or offset it with +N / -N
    Brightness {
        #[arg(allow_hyphen_values = true)]
        value: String,
        #[arg(default_value = "all")]
        target: String,
    },
    /// Set colour temperature in kelvin (2900-7000), or offset it with +N / -N
    Temperature {
        #[arg(allow_hyphen_values = true)]
        kelvin: String,
        #[arg(default_value = "all")]
        target: String,
    },
    /// Set power, brightness and temperature in a single request
    Set {
        #[arg(long, conflicts_with = "off")]
        on: bool,
        #[arg(long)]
        off: bool,
        #[arg(long)]
        brightness: Option<u8>,
        #[arg(long = "temp")]
        temp: Option<u32>,
        #[arg(default_value = "all")]
        target: String,
    },
    /// Rediscover if any light is unreachable, then toggle
    Click {
        #[arg(default_value = "all")]
        target: String,
    },
}
