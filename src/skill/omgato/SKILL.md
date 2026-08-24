---
name: omarchy-omgato
description: Control Key Light, Stream Deck and Cam Link hardware on Omarchy from the command line - Key Light power, brightness and colour temperature; Stream Deck pages, keys and brightness; the Stream Deck Pedal; and the Cam Link 4K overlay and screen recording. Use whenever the user asks to turn lights on or off, change light brightness or warmth, edit or inspect Stream Deck keys or pages, move or place the camera overlay, or start and stop a screen recording.
---

# Omgato

Four command line tools. Each prints plain text by default and JSON with `--json`
where a machine-readable answer helps.

## Key Lights

```bash
keylight-ctl ls --json                     # every light with live state
keylight-ctl on | off | toggle [TARGET]    # TARGET is a name, IP, MAC, or "all"
keylight-ctl brightness 60 [TARGET]        # absolute 0-100
keylight-ctl brightness +10 [TARGET]       # relative, per light
keylight-ctl temperature 4500 [TARGET]     # kelvin 2900-7000
keylight-ctl temperature -300 [TARGET]     # relative, per light
keylight-ctl set --on --brightness 60 --temp 4500 [TARGET]
keylight-ctl discover [--prune]            # merges into the cache; --prune drops absent lights
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
camlink-ctl status                    # JSON: alt, class, tooltip
camlink-ctl show | hide | toggle | full
camlink-ctl move tl | tr | bl | br
camlink-ctl place "X,Y WxH"           # an explicit rectangle
camlink-ctl pick                      # drag one out with the shared picker
camlink-ctl reset                     # USB re-authorize a wedged Cam Link
```

## Panel state and recording

```bash
omgato-panel                     # one JSON document: lights, deck, camera, record, shortcuts
omgato-panel --lights-only       # cheaper, lights only
omgato-panel sync                # every light to the average brightness and temperature
omgato-panel undo | redo         # light history
omgato-panel deck-undo | deck-redo
omgato-panel cam-undo | cam-redo
omgato-panel rename --ip IP --name NAME
omgato-panel order --ips a,b,c
omgato-panel record --target pick | last | screen [--desktop-audio] [--mic]
omgato-panel record --stop
omgato-panel scope-undo | scope-redo
omgato-panel install-shortcuts | uninstall-shortcuts
omgato-panel set-shortcut --id ID --keys "SUPER + ALT + L"
```

## Notes

- The Cam Link is a single-open V4L2 device: the overlay and any recorder that
  wants the camera cannot both hold it.
- Light names come from the devices; display names, order and undo history are
  kept by `omgato-panel` and never written to the lights.
- Every tool exits non-zero on failure and prints the reason on stderr.
