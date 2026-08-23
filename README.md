# Omarchy Elgato

One Omarchy Quattro bar panel for Elgato peripherals: Key Lights, a Stream Deck
with its Pedal, and a Cam Link 4K overlay. The panel is thin. Every device is
driven by a small command line tool in this repository, so anything the panel
does can also be scripted or bound to a key.

## What it does

```yaml
Key Lights:  per-light power, brightness and colour temperature, a colour dot
             showing the temperature, rename, drag to reorder, sync to the
             average, and a capped undo/redo history
Stream Deck: the key grid drawn from the images the daemon pushes to the
             device, page navigation, key editing, live brightness, display
             power, and undo/redo over the whole configuration
Pedal:       the three pedals drawn as the hardware looks, with tap, hold and
             double bindings for the selected pedal
Cam Link:    overlay placement by corner or by dragging out an area, plus
             one-press screen recording of a region or the whole screen
```

Each section can be switched off in the widget settings, so a rig with only
Key Lights shows only Key Lights.

## Layout

```
manifest.json          plugin manifest, entry point and settings schema
ui/Panel.qml           the bar widget and panel
src/elgatoctl/         Key Lights over their local HTTP API
src/streamdeck-ctl/    Stream Deck and Pedal daemons, rendering, TUI
src/camctl/            Cam Link overlay and status
src/elgato-panel/      status aggregation, light names, undo/redo history
scripts/install        build the workspace and link the binaries onto PATH
```

`elgato-panel` is the only crate the panel talks to directly for state. It
gathers one JSON document from the other three tools, keeps local light names
and display order, and owns the undo/redo histories. Device commands go
straight from the panel to `elgatoctl`, `streamdeck-ctl` and `camctl`, so
nothing sits between a click and the hardware.

## Install

```bash
./scripts/install
omarchy plugin enable io.github.data-goblin.omarchy-elgato
```

The panel shells out to `elgatoctl`, `streamdeck-ctl`, `camctl` and
`elgato-panel` by name, so they must be on the PATH the Omarchy shell sees.

Screen recording drives `omarchy-capture-screenrecording`, which ships with
Omarchy. Stream Deck access needs the udev rule in
`src/streamdeck-ctl/udev/`; install it with `streamdeck-ctl install-rules`.

## Requirements

- Omarchy Quattro (Quickshell shell)
- Rust 1.89 or newer to build
- `avahi-browse` for Key Light discovery
- `ffplay` for the Cam Link overlay

## Licence

MIT. See LICENSE.
