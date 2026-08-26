# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[semantic versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.3] - 2026-08-26

### Security

- Anchor every private runtime and fallback directory below trusted ancestors,
  rejecting symlinks, foreign ownership and non-sticky shared parents before
  any state file is opened
- Refuse unsafe legacy migration sources and move only real files and secured
  directories, without a copy fallback that could follow a replaced symlink
- Publish the camera log and every atomic state or configuration update from an
  exclusively created per-process inode rather than unlinking a predictable name
- Reserve screen recording, processed-video and Stream Deck export paths before
  external tools can open them, preventing predictable names in shared output
  directories from following attacker-planted symlinks
- Require `XDG_RUNTIME_DIR` to reach the compositor socket rather than falling
  back to a world-writable `/tmp` path another user could answer on
- Bind saved and discovered processes to their `/proc` start time, rescan camera
  holders before stopping a systemd user unit, and signal overlays through a
  pidfd so PID reuse cannot redirect an action

### Fixed

- Initialise and cache secured state paths before acquiring resources, and
  return normally from panel commands so Rust cleanup guards always run
- Reject Stream Deck page names that could escape a caller-selected export root

## [0.1.1] - 2026-08-25

### Fixed

- Serialise camera overlay commands so show, hide, avoidance and release cannot
  race each other
- Keep overlays visible after display changes by resolving and clamping placement
  against the current monitor layout
- Recover orphaned camera processes and expire claims left behind by crashed
  owners without disturbing live owners
- Borrow camera devices safely from every matching systemd user unit and avoid
  restarting them while a failed process termination could still hold the device
- Migrate configuration, state, runtime data, links and shortcuts left under the
  plugin's former name
- Restore Key Lights by their network identity and report partial restore failures
- Keep panel claims aligned with the panel's actual screen and release them when
  camera controls are disabled or the panel is destroyed
- Centre the Omgato mark reliably within its panel item

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
