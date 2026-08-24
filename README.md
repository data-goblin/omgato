# Omarchy Elgato

[![Release](https://img.shields.io/github/v/release/data-goblin/omarchy-elgato?display_name=tag&sort=semver)](https://github.com/data-goblin/omarchy-elgato/releases)
[![Licence](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)
[![Omarchy](https://img.shields.io/badge/omarchy-quattro-8a63d2.svg)](https://omarchy.org)

One Omarchy Quattro bar panel for Elgato peripherals: Key Lights, a Stream Deck
with its Pedal, and a Cam Link 4K overlay. This is an unofficial project. The
panel is thin. Every device is driven by a small command line tool in this
repository, so anything the panel does can also be scripted, bound to a key, or
handed to an agent.

<p align="center">
  <img src="docs/images/lights.png" alt="Key Lights view" width="300">
  <img src="docs/images/deck.png" alt="Stream Deck view" width="300">
  <img src="docs/images/camera.png" alt="Cam Link view" width="300">
</p>

## Supported hardware

Layout comes from the device library rather than a list here, so any Stream Deck
the `elgato-streamdeck` crate recognises is laid out correctly. Supported means
the plugin drives it; tested means it has been exercised on real hardware.

| Device | Supported | Tested | What the plugin does with it |
| --- | :---: | :---: | --- |
| Stream Deck Mk.2 (5x3) | ✓ | ✓ | Key grid, pages, multi-key editing, brightness, display power |
| Stream Deck Original and V2 (5x3) | ✓ | ✗ | Same 5x3 grid and paging as the Mk.2 |
| Stream Deck Mini and Mini Mk.2 (3x2) | ✓ | ✗ | Three by two grid, paging, same editing surface |
| Stream Deck XL and XL V2 (8x4) | ✓ | ✗ | Eight by four grid, paging, same editing surface |
| Stream Deck Neo (4x2) | ✓ | ✗ | Key grid and paging; the two touch keys are not addressable |
| Stream Deck + (4x2) | ✓ | ✗ | Key grid and paging; dials and touch strip are not addressable |
| Stream Deck Pedal | ✓ | ✓ | Tap, hold and double bindings for each of the three pedals |
| Key Light | ✓ | ✓ | Power, brightness and colour temperature, rename and reorder |
| Key Light Air | ✓ | ✓ | Power, brightness and colour temperature, rename and reorder |
| Key Light Mini | ✓ | ✗ | Same local HTTP API as the other Key Lights |
| Ring Light | ✓ | ✗ | Same local HTTP API as the other Key Lights |
| Cam Link 4K | ✓ | ✓ | Overlay placement by corner or dragged area, plus recording |

Auto-pagination puts its arrows at the two ends of the bottom row, which is
sensible on every grid but has only been seen on a 5x3.

> [!NOTE]
> If you own hardware that is not supported, please feel free to test, add it,
> and submit a pull request.

`streamdeck-ctl ls --json` output is the single most useful thing to attach to an
issue: it shows exactly what the library reports for your device.

## Install

```bash
omarchy plugin add https://github.com/data-goblin/omarchy-elgato --enable
~/.config/omarchy/plugins/io.github.data-goblin.omarchy-elgato/scripts/install
streamdeck-ctl enable
```

`omarchy plugin add` only clones the plugin and places the widget on the right of
the bar. `scripts/install` is what builds the workspace, links the four binaries
onto PATH, installs the systemd user units, links the agent skill, and applies a
starter Stream Deck layout **only if you have no configuration yet**. Pass
`--no-preset` or `--no-skill` to skip either. It prints the one privileged step it
will not take for you: installing the udev rule.

## Update

Omarchy installs plugins as git checkouts and never pulls them, so an update is
two steps. The second one rebuilds the binaries, and skipping it leaves you
running the old ones:

```bash
omarchy plugin update io.github.data-goblin.omarchy-elgato
~/.config/omarchy/plugins/io.github.data-goblin.omarchy-elgato/scripts/install
```

## Remove

Order matters. `scripts/uninstall` uses the binaries to undo their own Hyprland
edits, so run it before the plugin directory goes away:

```bash
~/.config/omarchy/plugins/io.github.data-goblin.omarchy-elgato/scripts/uninstall
omarchy plugin remove io.github.data-goblin.omarchy-elgato
```

What survives on purpose: your Stream Deck and Pedal configuration, your light
names and display order, and the udev rule at
`/etc/udev/rules.d/70-streamdeck-ctl.rules`, which needs root to remove. Pass
`--purge` to `scripts/uninstall` to drop the saved state as well.

## Settings

Each section can be switched off, so a rig with only Key Lights shows only Key
Lights. Set them in the widget settings, or from a script:

```bash
omarchy bar set io.github.data-goblin.omarchy-elgato showDeck false --json
```

| Setting | Default | What it controls |
| --- | :---: | --- |
| `showLights` | `true` | Brightness, temperature and per-light control for Key Lights |
| `showDeck` | `true` | Key grid, pages and pedal bindings for a Stream Deck |
| `showCamera` | `true` | Camera overlay placement and quick screen recording |

## Layout

```
manifest.json          plugin manifest, entry point and settings schema
ui/Panel.qml           the bar widget and panel
src/elgatoctl/         Key Lights over their local HTTP API
src/streamdeck-ctl/    Stream Deck and Pedal daemons, rendering, TUI, preset
src/camctl/            Cam Link overlay and status
src/elgato-panel/      status aggregation, names, shortcuts, undo/redo history
src/skill/             the agent skill describing the tools
scripts/install        build, link, install units, link the skill
scripts/install-skill  link only the agent skill
scripts/uninstall      reverse the installer
```

`elgato-panel` is the only crate the panel asks for state. It gathers one JSON
document from the other three tools, keeps local light names and display order,
and owns the undo histories. Device commands go straight from the panel to
`elgatoctl`, `streamdeck-ctl` and `camctl`, so nothing sits between a click and
the hardware.

## Shortcuts

The plugin writes `~/.config/hypr/elgato-bindings.lua` and adds a single guarded
line to your `bindings.lua` to source it. Your own bindings file is never
rewritten, and removing the plugin cannot break your config. Install them from
the panel, or:

```bash
elgato-panel install-shortcuts
elgato-panel set-shortcut --id lights.toggle --keys "SUPER + ALT + L"
elgato-panel uninstall-shortcuts
```

Only a handful are bound out of the box: one for the lights, none for the deck,
and the camera and recording ones. Everything else is listed unbound in the
panel, ready to be given a combination. Shortcuts are listed per section,
rebindable from the panel, and warn you when a combination is already claimed.

## What it installs, and where

Plugins run unsandboxed with your user permissions, so here is everything this
one touches outside its own directory.

```yaml
~/.local/bin/:            symlinks to the four binaries it builds
~/.config/systemd/user/:  streamdeck-ctl.service and streamdeck-ctl-deck.service,
                          the Stream Deck and Pedal daemons
~/.config/hypr/:          elgato-bindings.lua, plus one guarded pcall line added
                          to bindings.lua, only when you install shortcuts
~/.local/state/elgato-panel/: light display names, display order, undo history
~/.cache/elgato-panel/:   rendered key previews
~/.config/streamdeck-ctl/config.toml: your deck and pedal configuration
$XDG_RUNTIME_DIR/camctl/: overlay position and pid, cleared on reboot
agent skill directories:  a symlink to src/skill/, only where the directory
                          already exists, and only if you did not pass --no-skill
```

`scripts/uninstall` reverses all of it. It keeps your device configuration
unless you pass `--purge`.

### Privileges

The plugin never asks for root and contains no sudoers rule. Two things need
privilege and both are left to you, printed as instructions rather than run:

- installing the udev rule that grants access to Stream Deck hardware:

  ```bash
  sudo install -m 0644 \
    ~/.config/omarchy/plugins/io.github.data-goblin.omarchy-elgato/src/streamdeck-ctl/udev/70-streamdeck-ctl.rules \
    /etc/udev/rules.d/70-streamdeck-ctl.rules
  sudo udevadm control --reload-rules && sudo udevadm trigger
  ```

- `camctl reset`, which re-authorizes the Cam Link over USB to clear a wedged
  capture device, and needs passwordless sudo for that one write if you want it

Runtime pid state lives in `$XDG_RUNTIME_DIR/camctl/`, which is owner-only, not
in a shared temporary directory.

### Network and processes

`elgatoctl` talks HTTP to Key Lights on your local network, discovered over mDNS
with `avahi-browse`. Nothing else makes network calls and nothing phones home.
The daemons are ordinary systemd user services; no second Quickshell process is
ever started.

## Requirements

```yaml
Omarchy Quattro:      the Quickshell shell this plugin draws into
Rust 1.89+:           to build the workspace (rust)
avahi-browse:         Key Light discovery over mDNS (avahi)
mpv:                  the Cam Link overlay window (mpv)
pw-dump:              PipeWire state for camera status (pipewire)
gpu-screen-recorder:  screen recording (gpu-screen-recorder)
ffmpeg:               recording post-pass (ffmpeg)
slurp:                region picking for the overlay and recording (slurp)
fonts:                a text font and a Nerd Font for rendering deck keys;
                      fontconfig supplies a stand-in if the configured paths
                      do not exist
systemd --user:       the Stream Deck and Pedal daemons
```

Package names in brackets are the Arch packages that provide each binary.

## Troubleshooting

```yaml
No Stream Deck found:     the udev rule is missing. Install it as shown above,
                          then unplug and replug the device
Keys never update:        the daemons are not running. Check with
                          systemctl --user status streamdeck-ctl-deck
No lights discovered:     run elgatoctl discover. Key Lights answer over mDNS,
                          so the machine must be on the same subnet as the lights
A light reads unreachable: probes retry inside a 500ms budget, so a light that
                          still reports unreachable is genuinely not answering.
                          Confirm with elgatoctl ls --json
Camera overlay is black:  the Cam Link is single-open. Close anything else using
                          it, then camctl show. If it stays wedged, camctl reset
```

## Contributing

Issues and pull requests are welcome, particularly for hardware not marked tested
above. Please run `cargo clippy --release --workspace` and `cargo build --release`
before opening one. Release history is in [CHANGELOG.md](CHANGELOG.md).

## Licence

MIT. See [LICENSE](LICENSE).

Elgato, Stream Deck, Key Light and Cam Link are trademarks of Corsair Gaming and
its Elgato brand, which does not sponsor or endorse this project. This is an
unofficial, community-built plugin and is not affiliated with Elgato.
