use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "streamdeck-ctl", about = "Control Elgato Stream Deck devices (pedal, deck)")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// List all connected Stream Deck devices
    Ls {
        /// Emit one JSON object per device, with grid size and key count
        #[arg(long)]
        json: bool,
    },
    /// Pedal-specific commands
    Pedal {
        #[command(subcommand)]
        cmd: PedalCmd,
    },
    /// Deck-specific commands (Mk2 / XL / Mini)
    Deck {
        #[command(subcommand)]
        cmd: DeckCmd,
    },
    /// Open the TUI (status, bindings, daemon control)
    Tui,
    /// Print or install the udev rules
    InstallRules,
    /// Enable user systemd services (pedal + deck daemons)
    Enable,
    /// Disable user systemd services
    Disable,
}

#[derive(Subcommand, Debug)]
pub enum PedalCmd {
    /// Print pedal events to stdout without firing actions
    Test,
    /// Daemon: read pedal presses, fire configured actions
    Run,
    /// Show all configured pedal bindings (per gesture)
    Show,
    /// Get one binding: GESTURE is tap|long|double
    Get {
        position: String,
        gesture: String,
    },
    /// Set one binding. ACTION is one of:
    ///   key:KEY_F13  exec:firefox  noop
    Set {
        position: String,
        gesture: String,
        action: String,
    },
    /// Restart the pedal daemon to pick up new config
    Reload,
}

#[derive(Subcommand, Debug)]
pub enum DeckCmd {
    /// List connected Stream Deck (non-pedal) devices
    Ls,
    /// Daemon: render pages, dispatch actions on button press
    Run,
    /// Render the default page once (test) and exit
    Render,
    /// Print the configured pages and buttons
    Show,
    /// Set deck brightness (0-100); also writes config
    Brightness { value: u8 },
    /// Switch the key display on or off without losing the brightness level
    Power {
        /// on, off, or toggle
        #[arg(default_value = "toggle")]
        state: String,
    },
    /// Clear all buttons (black)
    Reset,
    /// Restart the deck daemon to pick up new config
    Reload,
    /// Add or update a button on a page; only specified fields are changed
    Set {
        page: String,
        index: u8,
        #[arg(long)]
        label: Option<String>,
        #[arg(long)]
        glyph: Option<String>,
        #[arg(long)]
        icon: Option<String>,
        #[arg(long)]
        bg: Option<String>,
        #[arg(long)]
        fg: Option<String>,
        #[arg(long)]
        action: Option<String>,
    },
    /// Remove a button from a page
    Unset { page: String, index: u8 },
    /// List all pages
    Pages,
    /// Add an empty page; without a name it is called Page N
    PageAdd { name: Option<String> },
    /// Remove a page
    PageRm { name: String },
    /// Rename a page, keeping its position and buttons
    PageRename { from: String, to: String },
    /// Set the default page (loaded on daemon start)
    Default { name: String },
    /// Show the page order array (used for auto-pagination)
    Order,
    /// Set the page order array. Pages NOT listed are excluded from auto-pagination.
    OrderSet { names: Vec<String> },
    /// Toggle auto-pagination on/off
    AutoPaginate { enabled: bool },
    /// Apply a bundled starter layout; existing pages of the same name are replaced
    Preset {
        #[arg(default_value = "omarchy")]
        name: String,
        /// Replace the whole page set rather than merging
        #[arg(long)]
        replace: bool,
    },
    /// Render every key to PNG files at <out>/<page>/<index>.png
    Export {
        #[arg(long)]
        out: std::path::PathBuf,
        #[arg(long)]
        page: Option<String>,
        #[arg(long)]
        size: Option<u32>,
        #[arg(long)]
        keys: Option<u8>,
        /// Feather the key corners to transparent by this many pixels
        #[arg(long)]
        radius: Option<f32>,
    },
}
