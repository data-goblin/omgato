---
name: omarchy-elgato
description: Control Elgato peripherals on Omarchy from the command line - Key Light power, brightness and colour temperature; Stream Deck pages, keys and brightness; the Stream Deck Pedal; and the Cam Link 4K overlay and screen recording. Use whenever the user asks to turn lights on or off, change light brightness or warmth, edit or inspect Stream Deck keys or pages, move or place the camera overlay, or start and stop a screen recording.
---

# Omarchy Elgato

Four command line tools. Each prints plain text by default and JSON with `--json`
where a machine-readable answer helps.

## Key Lights

```bash
elgatoctl ls --json                     # every light with live state
elgatoctl on | off | toggle [TARGET]    # TARGET is a name, IP, MAC, or "all"
elgatoctl brightness 60 [TARGET]        # absolute 0-100
elgatoctl brightness +10 [TARGET]       # relative, per light
elgatoctl temperature 4500 [TARGET]     # kelvin 2900-7000
elgatoctl temperature -300 [TARGET]     # relative, per light
elgatoctl set --on --brightness 60 --temp 4500 [TARGET]
elgatoctl discover [--prune]            # merges into the cache; --prune drops absent lights
```

An exact name, IP or MAC wins over a substring match, so `Key` addresses the
light called `Key` even when `Key Light` also exists.

## Stream Deck and Pedal

```bash
streamdeck-ctl ls --json                          # kind, key count, rows, columns
streamdeck-ctl deck pages | show | order
streamdeck-ctl deck set PAGE INDEX --label L --glyph G --icon PATH \
                                   --bg '#rrggbb' --fg '#rrggbb' --action 'exec:firefox'
streamdeck-ctl deck unset PAGE INDEX
streamdeck-ctl deck page-add NAME | page-rm NAME | default NAME
streamdeck-ctl deck brightness 0-100
streamdeck-ctl deck power on | off | toggle       # blanks the display, keeps the level
streamdeck-ctl deck export --out DIR [--radius N] # render every key to PNG
streamdeck-ctl pedal show | set POSITION GESTURE ACTION
```

POSITION is `left`, `center` or `right`; GESTURE is `tap`, `long` or `double`.
An action is `exec:<command>`, `key:<KEY_NAME>`, `page:<name>` or `noop`.

## Cam Link overlay

```bash
camctl status                    # JSON: alt, class, tooltip
camctl show | hide | toggle | full
camctl move tl | tr | bl | br
camctl place "X,Y WxH"           # an explicit rectangle
camctl pick                      # drag one out with the shared picker
camctl reset                     # USB re-authorize a wedged Cam Link
```

## Panel state and recording

```bash
elgato-panel                     # one JSON document: lights, deck, camera, record, shortcuts
elgato-panel --lights-only       # cheaper, lights only
elgato-panel sync                # every light to the average brightness and temperature
elgato-panel undo | redo         # light history
elgato-panel deck-undo | deck-redo
elgato-panel cam-undo | cam-redo
elgato-panel rename --ip IP --name NAME
elgato-panel order --ips a,b,c
elgato-panel record --target pick | last | screen [--desktop-audio] [--mic]
elgato-panel record --stop
elgato-panel scope-undo | scope-redo
elgato-panel install-shortcuts | uninstall-shortcuts
elgato-panel set-shortcut --id ID --keys "SUPER + ALT + L"
```

## Notes

- The Cam Link is a single-open V4L2 device: the overlay and any recorder that
  wants the camera cannot both hold it.
- Light names come from the devices; display names, order and undo history are
  kept by `elgato-panel` and never written to the lights.
- Every tool exits non-zero on failure and prints the reason on stderr.
