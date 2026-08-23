# Omarchy Elgato

[![Release](https://img.shields.io/github/v/release/data-goblin/omarchy-elgato?display_name=tag&sort=semver)](https://github.com/data-goblin/omarchy-elgato/releases)
[![Licence](https://img.shields.io/badge/licence-MIT-blue.svg)](LICENSE)
[![Omarchy](https://img.shields.io/badge/omarchy-quattro-8a63d2.svg)](https://omarchy.org)
[![Rust](https://img.shields.io/badge/rust-1.89%2B-orange.svg)](https://www.rust-lang.org)

One Omarchy Quattro bar panel for Elgato peripherals: Key Lights, a Stream Deck
with its Pedal, and a Cam Link 4K overlay. The panel is thin. Every device is
driven by a small command line tool in this repository, so anything the panel
does can also be scripted, bound to a key, or handed to an agent.

## What it does

```yaml
Key Lights:  per-light power, brightness and colour temperature, a dot showing
             the temperature, rename, drag to reorder, match all lights to the
             average, and undo/redo over the last ten changes
Stream Deck: the key grid drawn from the images the daemon pushes to the
             device, page navigation, single and multi-key editing, live
             brightness, display power, and undo/redo over the configuration
Pedal:       the three pedals drawn as the hardware looks, with tap, hold and
             double bindings for the selected pedal
Cam Link:    overlay placement by corner or by dragging out an area, and screen
             recording that remembers the area it captured
Shortcuts:   listed per section, rebindable from the panel, with a warning when
             a combination is already claimed
```

Each section can be switched off in the widget settings, so a rig with only Key
Lights shows only Key Lights.

## Install

```bash
./scripts/install
omarchy plugin enable io.github.data-goblin.omarchy-elgato
streamdeck-ctl enable
```

The installer builds the workspace, links the four binaries onto PATH, installs
the systemd user units, links the agent skill, and applies a starter Stream Deck
layout **only if no pages are configured yet**. Pass `--no-preset` or
`--no-skill` to skip either. It prints the one privileged step it will not take
for you: installing the udev rule.

## Device support

Layout comes from the device library rather than a table here, so any Stream
Deck the `elgato-streamdeck` crate recognises is laid out correctly. What
differs is how much has been exercised on real hardware.

```yaml
Stream Deck Mk.2 (5x3):   tested, all features
Stream Deck Pedal:        tested, all features
Key Light and Key Light Air: tested
Cam Link 4K:              tested
Stream Deck Original/V2 (5x3):  should work, untested
Stream Deck Mini and Mini Mk.2 (3x2): should work, untested
Stream Deck XL and XL V2 (8x4): should work, untested
Stream Deck Neo (4x2 plus two touch keys): grid works, the touch keys are not
                                           addressable and are not shown
Stream Deck + (4x2 plus four dials and a touch strip): grid works, the dials
                                           and the strip are not addressable
Key Light Mini:           should work, untested
Ring Light:               should work, untested
```

Two honest gaps: the models with dials, touch strips or secondary touch keys
are laid out as their key grid only, because nothing here reads or writes those
controls yet. And auto-pagination places its arrows at the two ends of the
bottom row, which is sensible on every grid but has only been seen on a 5x3.

**If you own hardware not marked tested, please try it and open an issue or a
pull request.** `streamdeck-ctl ls --json` output is the single most useful
thing to attach: it shows exactly what the library reports for your device.

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
panel, ready to be given a combination.

## Requirements

```yaml
Omarchy Quattro:  the Quickshell shell this plugin draws into
Rust 1.89+:       to build the workspace
avahi-browse:     Key Light discovery over mDNS
mpv:              the Cam Link overlay window
pw-dump:          PipeWire state for camera status (pipewire-utils)
gpu-screen-recorder, ffmpeg, slurp: screen recording
fonts:            a text font and a Nerd Font for rendering deck keys;
                  fontconfig supplies a stand-in if the configured paths
                  do not exist
systemd --user:   the Stream Deck and Pedal daemons
```

## Contributing

Issues and pull requests are welcome, particularly for hardware not marked
tested above. Please run `cargo clippy --release --workspace` and
`cargo build --release` before opening one.

## Licence

MIT. See [LICENSE](LICENSE).
