# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-08-23

First release. Brings four previously separate tools together as one Omarchy
Quattro plugin.

### Added

- One bar panel for Key Lights, Stream Deck, Stream Deck Pedal and Cam Link
- Key grid drawn from the images the daemon pushes to the device
- Multi-select on the key grid, with bulk editing of icon and colours
- Undo and redo for lights, Stream Deck configuration, camera placement and
  recording areas
- Screen recording that remembers the area it captured
- Shortcuts owned by the plugin, rebindable from the panel with conflict warnings
- An agent skill describing the command line tools
- A starter Stream Deck layout mirroring Omarchy's own bindings
- Per-section settings, so a rig with only Key Lights shows only Key Lights

### Fixed

- Camera placement had not worked since Hyprland 0.56 changed its dispatcher API
- Starting a recording froze the panel, leaving no way to stop it
- Device geometry was inferred from a debug string, so every deck that is not a
  Mk2 was drawn with the wrong grid
- The Stream Deck TUI overwrote the whole configuration with defaults after a
  parse error
- Discovery replaced the light cache rather than merging, so a light that was
  briefly offline was forgotten
