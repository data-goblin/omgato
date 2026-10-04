use clap::{Parser, Subcommand};
use std::io::Read;

mod capture;
mod card;
mod control;
mod daemon;
mod hypr;
mod scripts;
mod text;

#[derive(Parser)]
#[command(name = "prompter-ctl", about = "Drive an Elgato Prompter: scroll scripts or mirror a Hyprland monitor")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Own the Prompter and serve commands until stopped (run by prompter-ctl.service)
    Run,
    /// Print the daemon state and the saved scripts as JSON
    Status,
    /// Light the Prompter in the mode it last used
    On,
    /// Blank the Prompter
    Off,
    /// Switch between scrolling a script, mirroring the PROMPTER monitor, or off
    Mode { mode: daemon::Mode },
    /// Show a saved script from the top
    Load { name: String },
    /// Start scrolling
    Play,
    /// Stop scrolling
    Pause,
    /// Start or stop scrolling
    Toggle,
    /// Jump back to the start of the script
    Top,
    /// Move back one line
    Back,
    /// Move forward one line
    Forward,
    /// Scroll speed in pixels per second (5 to 600)
    Speed { value: u32 },
    /// Scroll 10 px/s faster
    Faster,
    /// Scroll 10 px/s slower
    Slower,
    /// Text size in pixels (20 to 200)
    Font { value: u32 },
    /// Line height as a percentage of the text size (100 to 300)
    Spacing { value: u32 },
    /// Dim the picture: 5 to 100 percent (the Prompter's backlight is not reachable on Linux)
    Brightness { value: u32 },
    /// Text 4 px bigger
    Bigger,
    /// Text 4 px smaller
    Smaller,
    /// Flip the text left to right: on, off or toggle
    Mirror { state: Option<String> },
    /// Manage saved scripts
    #[command(subcommand)]
    Script(ScriptCmd),
}

#[derive(Subcommand)]
enum ScriptCmd {
    /// List saved scripts
    List,
    /// Print a script
    Show { name: String },
    /// Save a script from --text or standard input
    Write {
        name: String,
        #[arg(long)]
        text: Option<String>,
    },
    /// Open a script in the Omarchy editor, creating it when missing
    Edit { name: String },
    /// Delete a script
    Rm { name: String },
}

fn line(cmd: &Cmd) -> String {
    match cmd {
        Cmd::Mode { mode } => format!("mode {mode:?}").to_lowercase(),
        Cmd::Load { name } => format!("load {name}"),
        Cmd::Speed { value } => format!("speed {value}"),
        Cmd::Font { value } => format!("font {value}"),
        Cmd::Spacing { value } => format!("spacing {value}"),
        Cmd::Brightness { value } => format!("brightness {value}"),
        Cmd::Mirror { state } => format!("mirror {}", state.as_deref().unwrap_or("toggle")),
        Cmd::On => "on".into(),
        Cmd::Off => "off".into(),
        Cmd::Play => "play".into(),
        Cmd::Pause => "pause".into(),
        Cmd::Toggle => "toggle".into(),
        Cmd::Top => "top".into(),
        Cmd::Back => "back".into(),
        Cmd::Forward => "forward".into(),
        Cmd::Faster => "faster".into(),
        Cmd::Slower => "slower".into(),
        Cmd::Bigger => "bigger".into(),
        Cmd::Smaller => "smaller".into(),
        Cmd::Run | Cmd::Status | Cmd::Script(_) => "status".into(),
    }
}

fn status() -> serde_json::Value {
    let mut v = control::send("status").unwrap_or_else(|_| serde_json::json!({ "running": false }));
    v["connected"] = std::path::Path::new("/dev/dri/prompter").exists().into();
    v["scripts"] = scripts::list().into();
    v["dir"] = scripts::dir().display().to_string().into();
    v
}

fn script(cmd: ScriptCmd) -> Result<(), String> {
    match cmd {
        ScriptCmd::List => scripts::list().iter().for_each(|n| println!("{n}")),
        ScriptCmd::Show { name } => print!("{}", scripts::read(&name)?),
        ScriptCmd::Write { name, text } => {
            let text = match text {
                Some(t) => t,
                None => {
                    let mut t = String::new();
                    std::io::stdin().read_to_string(&mut t).map_err(|e| format!("stdin: {e}"))?;
                    t
                }
            };
            scripts::write(&name, &text)?
        }
        ScriptCmd::Edit { name } => {
            let path = scripts::path(&name)?;
            if !path.exists() {
                scripts::write(&name, "")?;
            }
            std::process::Command::new("omarchy-launch-editor").arg(&path)
                .stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
                .spawn().map_err(|e| format!("omarchy-launch-editor: {e}"))?;
        }
        ScriptCmd::Rm { name } => scripts::remove(&name)?,
    }
    Ok(())
}

fn main() {
    let cmd = Cli::parse().cmd;
    let result = match cmd {
        Cmd::Run => daemon::run(),
        Cmd::Status => {
            println!("{}", status());
            Ok(())
        }
        Cmd::Script(c) => script(c),
        other => control::send(&line(&other)).map(|_| ()),
    };
    if let Err(e) = result {
        eprintln!("prompter-ctl: {e}");
        std::process::exit(1);
    }
}
