# Melbourne Fire Brigade — Rust desktop app

A copy of the Victoria Ambulance Rust app, set up for the fire service. Fully
native UI (egui): one `MelbourneFireBrigade.exe`, no browser, no Node.

## Run it

Double-click `MelbourneFireBrigade.exe`. It opens maximised. **F11** toggles full screen.

Same flow as the ambulance app: boot screen → login / create local account →
desktop with menu bar, dock, launcher and windows. Fire-red colours, a flame
logo and the Ember wallpaper by default.

## Apps

- **Map** — interactive map of the city with every postal written on it
  (like gta-5-map.com). Drag to move, scroll or the +/− buttons to zoom,
  double-click to zoom in. Type a postal in the search box and press Enter to
  fly to it (it pulses yellow), or just click a postal number on the map.
  The box in the bottom left then shows the nearest street and the closest
  corner or intersection (biggest street first), e.g. *Intersection of
  Innocence Blvd & Power St*, with a yellow dot where they meet. It tells
  apart a 4-way **Intersection**, a **T-intersection** and a **Corner**.
  Right-click drops a waypoint and gets the same street details. Satellite /
  Atlas switch, and a button to hide the postals.
- **Voice control** (Settings → Voice) — press the listen key (default F10,
  works while the game has focus; a mouse side button works too) and say a
  postal, or say **"Fire Com dispatch"** and the dispatcher answers *"Go for
  Dispatch"*. Say the postal ("three six one",
  "three sixty one", "ten twenty nine"); the map opens and flies there, and the
  dispatcher reads it back with the nearest road ("Copy, postal three six
  one… Palomino Avenue"). Say "cancel" to
  stop. The phrase counts when said as its own command (at the start, or after
  a short pause), not mid-sentence. Modes: Off / key only / key or phrase.
  Pick the microphone and test it (level bar). Dispatcher voices: Ann (made
  from a recording of Ann's own voice) plus 5 female and 5 male, and an
  optional police-scanner radio effect.
- **Time Clock**, **Notes**, **Settings** — same as the ambulance app.
- **Field Guide** — placeholder until the fire cheat sheet is written. (The
  ambulance guide code is still in `src/fieldguide/` to reuse its layout.)
- **Incident Reports** — placeholder, waiting for the database.

## The postals

`src/postals.rs` holds every postal as `(number, x, y)`: the centre of the
number on the map, in map units (the whole map is 4096 × 4096). They were
copied from the in-game reference screenshots: each screenshot was lined up
with the map pictures automatically, every number was read and then checked
by eye, so each postal sits exactly where it is in game.

A few numbers are partly hidden under icons in the screenshots (listed in
`PARTLY_HIDDEN` at the bottom of that file); the map tells you when you search
for one of them. Postals that aren't in the screenshots aren't on the map yet:
add a line to `src/postals.rs` to put one on.

Street names come from the game's own road network (every named road point,
from github.com/DurtyFree/gta-v-data-dumps), lined up with the map pictures
and packed in `assets/map/roads.bin`. `src/streets.rs` works out the street
and the corner / intersection.

Voice control (`src/voice.rs`) only switches on once a microphone is found.
It listens on the microphone picked in Settings → Voice (with a live level bar)
and understands speech on this PC with the Vosk engine, limited to the wake
phrase and numbers. The engine (~56 MB) is downloaded once, the first time
voice control is on, into `%APPDATA%\Melbourne Fire Brigade\speech\`
(delete that folder to get it again). The listen key is watched with
GetAsyncKeyState, so any key or mouse button works while the game has focus,
and the key still reaches the game. The dispatcher voices are recordings made
with the open Kokoro neural voices, packed in `assets/voice/*.bin`.

The map pictures (`assets/map/*-z*.bin`) are the satellite and atlas tiles from
github.com/meesvrh/GTAV-Map-Tiles, packed so they're built into the exe. That
is why the exe is about 60 MB.

## Stream Deck

The control API listens on `http://localhost:47914` (the ambulance app uses
47913, so both can run at once). Same addresses as the ambulance app.

## Software updates

The app updates itself from the Releases of
`github.com/demondeye/BBRP-Melbourne-fire-Brocade-app-releases` (set in
`UPDATE_REPO` in `src/update.rs`). It checks when it starts and every 3 hours;
when a newer version is published a popup offers *Install now* or *Later*, and
Settings → Software Update shows it too. The repo must stay public.

### Publishing an update

1. Make your changes.
2. Raise `version` in `Cargo.toml` (e.g. `0.1.0` → `0.2.0`).
3. Write what's new in `RELEASE_NOTES.md` (shown in the popup).
4. Run `powershell -ExecutionPolicy Bypass -File release.ps1`

`release.ps1` builds the exe and publishes it as a release tagged `v0.2.0`
(needs the GitHub CLI, signed in once with `gh auth login`).

Or by hand: on the repo, *Releases → Draft a new release*, tag `v0.2.0`, title
`Melbourne Fire Brigade 0.2.0`, paste the notes, attach
`MelbourneFireBrigade.exe`, *Publish release*.

## Where data is saved

`%APPDATA%\Melbourne Fire Brigade\` — separate from the ambulance app.

## Build it yourself

Needs Rust (https://rustup.rs).

```
cargo build --release
```

The exe lands in `target\release\MelbourneFireBrigade.exe`.

For testing only: `MFB_FAST=1` shortens the boot and login waits,
`MFB_WINDOWED=1` starts in a window, `MFB_DATA_DIR=<folder>` keeps a separate
set of data, `MFB_UPDATE_URL=<url>` reads updates from a different release feed.

## Layout

```
src/main.rs            window setup
src/app.rs             boot → login → desktop, restart / shut down
src/postals.rs         every postal and where it sits on the map
src/streets.rs         nearest street, corner or intersection for any spot
src/voice.rs           voice control: listen key, wake phrase, dispatcher voice
src/ui/apps/map.rs     the Map app
src/core.rs            state shared with the Stream Deck server
src/relay.rs           Stream Deck control API (localhost:47914)
src/update.rs          software updates (GitHub Releases)
src/auth.rs            local account
src/settings.rs        per-user settings, wallpapers
src/timeclock.rs       clock in / out, shifts
src/ui/                desktop, window manager, theme, icons, wallpaper
src/ui/apps/           Map, Time Clock, Settings, placeholders
assets/                fonts, emoji, app icon, map tiles
```

Map tiles from GTAV-Map-Tiles (meesvrh). Emoji pictures are Twemoji (CC-BY 4.0,
© Twitter / jdecked). Fonts are Outfit and Syne (SIL Open Font License).
