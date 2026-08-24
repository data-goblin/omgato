//! Panel-side state for the kurt.elgato Quickshell widget: one aggregated
//! status document, local light names, key previews, and capped undo/redo
//! histories. Device control itself lives in elgatoctl, streamdeck-ctl and
//! camctl.
use clap::{Parser, Subcommand};
use serde::Serialize;

mod camera;
mod deck;
mod lights;
mod record;
mod shortcuts;
mod sh;
mod state;

#[derive(Parser)]
#[command(name = "elgato-panel", about = "Aggregated status and history for the Elgato panel")]
struct Cli {
    /// Skip the Stream Deck and Cam Link sections
    #[arg(long, global = true)]
    lights_only: bool,

    /// Also report which shortcuts clash with an existing binding
    #[arg(long, global = true)]
    with_conflicts: bool,

    /// Search for a recorder this tool did not start
    #[arg(long, global = true)]
    with_record: bool,

    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Print the aggregated status document (default)
    Status,
    /// Set every light to the average brightness and temperature
    Sync,
    /// Remember the current light settings as the default
    SaveDefault,
    /// Put every light back to the saved default
    RestoreDefault,
    /// Step back one recorded light state
    Undo,
    /// Step forward one recorded light state
    Redo,
    /// Give a light a local display name
    Rename {
        #[arg(long)]
        ip: String,
        #[arg(long)]
        name: String,
    },
    /// Set the display order from a comma-separated list of light addresses
    Order {
        #[arg(long)]
        ips: String,
    },
    /// Step back one Stream Deck configuration
    DeckUndo,
    /// Step forward one Stream Deck configuration
    DeckRedo,
    /// Step back one Cam Link overlay placement
    CamUndo,
    /// Step forward one Cam Link overlay placement
    CamRedo,
    /// Write the plugin's keyboard shortcuts and source them from hypr
    InstallShortcuts,
    /// Remove the plugin's keyboard shortcuts
    UninstallShortcuts,
    /// Step back one remembered recording area
    ScopeUndo,
    /// Step forward one remembered recording area
    ScopeRedo,
    /// Rebind one shortcut; an empty value restores its default
    SetShortcut {
        #[arg(long)]
        id: String,
        #[arg(long, allow_hyphen_values = true)]
        keys: String,
    },
    /// Start a screen recording of a picked region or the whole screen
    Record {
        /// "region" opens the picker, "screen" records the focused monitor
        #[arg(long, default_value = "region")]
        target: String,
        #[arg(long)]
        desktop_audio: bool,
        #[arg(long)]
        mic: bool,
        /// Stop the recording that is running
        #[arg(long)]
        stop: bool,
    },
}

#[derive(Serialize)]
struct Document {
    lights: Vec<lights::Light>,
    history: state::Flags,
    /// Whether a saved default exists, so the panel knows if restoring is
    /// something the user can actually do yet.
    default_saved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    deck: Option<deck::Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    camera: Option<camera::Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    record: Option<record::Status>,
    #[serde(skip_serializing_if = "Option::is_none")]
    shortcuts: Option<shortcuts::Status>,
}

fn status(lights_only: bool, with_conflicts: bool, with_record: bool) {
    let (lights, deck, camera) = std::thread::scope(|scope| {
        let devices = (!lights_only).then(|| (scope.spawn(deck::status), scope.spawn(camera::status)));
        let lights = lights::read();
        match devices {
            Some((d, c)) => (lights, d.join().ok(), c.join().ok()),
            None => (lights, None, None),
        }
    });

    let mut history: state::History<Vec<state::Snap>> = state::History::load(state::LIGHTS_HISTORY);
    if let Some(snap) = lights::snapshot(&lights, history.current()) {
        history.fold(state::LIGHTS_HISTORY, snap);
    }

    let doc = Document {
        lights,
        history: history.flags(),
        default_saved: lights::has_default(),
        deck,
        camera,
        record: (!lights_only).then(|| record::status(with_record)),
        shortcuts: (!lights_only).then(|| shortcuts::status(with_conflicts)),
    };
    println!("{}", serde_json::to_string(&doc).unwrap_or_default());
}

fn travel_lights(step: i64) {
    let history: state::History<Vec<state::Snap>> = state::History::load(state::LIGHTS_HISTORY);
    let Some((pos, snap)) = history.seek(step) else {
        return;
    };
    lights::restore(snap);
    history.commit_pos(state::LIGHTS_HISTORY, pos);
}

fn report(result: Result<(), String>) {
    if let Err(e) = result {
        eprintln!("elgato-panel: {e}");
        std::process::exit(1);
    }
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        None | Some(Cmd::Status) => status(cli.lights_only, cli.with_conflicts, cli.with_record),
        Some(Cmd::Sync) => lights::sync(),
        Some(Cmd::SaveDefault) => report(lights::save_default().map(|n| {
            println!("saved {n} lights as the default");
        })),
        Some(Cmd::RestoreDefault) => report(lights::restore_default().map(|n| {
            println!("restored {n} lights to the default");
        })),
        Some(Cmd::Undo) => travel_lights(-1),
        Some(Cmd::Redo) => travel_lights(1),
        Some(Cmd::Rename { ip, name }) => {
            let mut aliases = state::load_aliases();
            let name = name.trim();
            if name.is_empty() {
                aliases.remove(&ip);
            } else {
                aliases.insert(ip, name.to_owned());
            }
            state::save_aliases(&aliases);
        }
        Some(Cmd::Order { ips }) => {
            let order: Vec<String> = ips
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_owned)
                .collect();
            state::save_order(&order);
        }
        Some(Cmd::DeckUndo) => deck::travel(-1),
        Some(Cmd::DeckRedo) => deck::travel(1),
        Some(Cmd::CamUndo) => camera::travel(-1),
        Some(Cmd::CamRedo) => camera::travel(1),
        Some(Cmd::ScopeUndo) => record::travel(-1),
        Some(Cmd::ScopeRedo) => record::travel(1),
        Some(Cmd::InstallShortcuts) => report(shortcuts::install()),
        Some(Cmd::UninstallShortcuts) => report(shortcuts::uninstall()),
        Some(Cmd::SetShortcut { id, keys }) => report(shortcuts::set(&id, &keys)),
        Some(Cmd::Record { target, desktop_audio, mic, stop }) => {
            if stop {
                record::stop();
            } else {
                record::start(&target, record::Options { desktop_audio, mic });
            }
        }
    }
}
