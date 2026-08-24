use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(name = "camctl", about = "Picture-in-picture overlay + status for the Elgato Cam Link 4K")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Show the overlay (no-op if already running)
    Show,
    /// Hide the overlay (waits for mpv to release the device)
    Hide,
    /// Show if hidden, hide if shown
    Toggle,
    /// Move the overlay to a corner; starts it if not running
    Move {
        #[arg(value_enum)]
        corner: Corner,
    },
    /// Pin the overlay to an explicit rectangle, in slurp's "X,Y WxH" form
    Place {
        geometry: String,
    },
    /// Drag out where the overlay should sit, using the shared region picker
    Pick,
    /// Toggle fullscreen on the overlay's monitor
    Full,
    /// Emit waybar status JSON
    Status,
    /// Pause notifications + status updates (touches a flag file)
    Pause,
    /// Undo `pause`
    Resume,
    /// USB-authorize toggle the Cam Link to clear a wedged UVC state.
    /// Requires passwordless sudo for `tee /sys/bus/usb/devices/.../authorized`.
    Reset,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum Corner {
    /// top-left
    Tl,
    /// top-right
    Tr,
    /// bottom-left
    Bl,
    /// bottom-right
    Br,
}
