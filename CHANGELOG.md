# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-08-23

First release. Brings four previously separate tools together as one Omarchy
Quattro plugin.

While the version stays below 1.0 the command line surfaces and the
configuration format may still change between minor versions. Device support
beyond the hardware listed in the README is untested; reports and pull requests
for other models are welcome.

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
- A single dropped request marked a Key Light unreachable, which on wifi showed
  as a light flickering in and out of the panel
- The starter layout never applied on a first install, because reading the
  configuration created a default page and made the machine look configured
- The starter layout ignored its own page order, so pages landed alphabetically
  and the default page could name a page that did not exist
