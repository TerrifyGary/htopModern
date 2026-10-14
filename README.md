# htopModern

A cozy, vintage-inspired macOS terminal dashboard combining htop-style system monitoring with animated scenes, music controls, todos, and weather.

## Status

The first Rust terminal milestone is implemented: full-screen responsive layout, real CPU/memory/process readings, uptime, and persistent todos. Generated LOFI animations (Fireplace, Beach, Coffee Shop) and a live analog clock are also implemented. CPU temperature, music integration, and live weather are pending in the terminal app. The browser prototype remains the visual reference.

## Run the terminal app

With a stable Rust toolchain installed:

```sh
cargo run --locked
```

To install the command:

```sh
cargo install --path . --locked
htopmodern
```

Cargo installs the executable in `~/.cargo/bin`; that directory must be on your `PATH`.

Use `Ctrl+B` then `L`, `M`, `T`, or `W` to choose a panel, or use Tab. In LOFI, Left/Right selects Previous/Next and Enter changes scene. The clock uses device local time; compact panels show digital time when the analog face cannot fit. In TODO, press `A` or select **Add item** and press Enter. Up/Down selects a task, Left/Right selects its checkbox or Delete, and Enter activates it. Enter saves a new task; Escape cancels. `Q` or Ctrl+C exits. In tmux, use Tab for now because tmux intercepts Ctrl+B; configurable prefixes are planned.

Todos are saved to `~/Library/Application Support/htopmodern/todos.json`. Set `HTOPMODERN_DATA_DIR` to override the storage directory. Invalid storage files are reported rather than overwritten. Narrow terminals show the selected bottom panel; widths of at least 110 columns show all four.

```sh
cargo run --locked -- --check  # read-only system snapshot
cargo test --locked
```

## Preview

![Current dashboard design preview](docs/images/dashboard-preview.png)

*Browser design prototype; system metrics and music information are illustrative.*

Open `cozy-terminal-preview.html` in a browser. LOFI scenes and the analog clock run locally. Weather uses Open-Meteo and requires internet access; Taipei is the default location. Weather loading errors are shown without hiding the character.

## Agreed design

- **COMPUTER:** individual CPU meters, memory and swap, task/thread counts, load averages, uptime, CPU temperature where supported, and a simplified process table in the same box. No PID or kill controls.
- **LOFI · L:** generated animated Fireplace, Beach, and Coffee Shop scenes; previous/next scene buttons; analog clock using device time. No GIF dependency.
- **MUSIC · M:** animated music bars, track and artist, playback timing on its own line, and selectable previous/play-pause/next buttons.
- **TODO · T:** an Add item button with task entry, selectable checkboxes and trash buttons, with persistent local storage planned.
- **WEATHER · W:** editable location defaulting to Taipei; a cute schoolgirl character reacts to Sunny, Windy, Cloudy, Rainy, and Snowy conditions, with day/night backgrounds chosen automatically. No manual weather or night buttons.
- Terminal app fills the terminal window and responds to resizing, adapting CPU meters and panel layout to available rows and columns.
- Configurable prefix shortcuts, initially `Ctrl+B` followed by `L`, `M`, `T`, or `W`; arrows select controls and Enter activates them. The prefix must be configurable to avoid conflicts with tmux.

## Initial implementation milestones

1. Choose the terminal rendering stack and test animation capabilities in the target terminal.
2. Build a responsive full-screen terminal shell and read real macOS system/process metrics.
3. Verify CPU temperature support on the target Mac.
4. Add generated scenes, analog clock, and persistent todos.
5. Integrate the user's music player and define whether bars are decorative or audio-reactive.
6. Integrate location lookup, weather caching, automatic daylight selection, and offline handling.
7. Package the executable and eventually offer Homebrew installation.

## Weather service

The prototype uses [Open-Meteo](https://open-meteo.com/) geocoding and current weather endpoints. It refreshes every ten minutes and uses the provider's `is_day` value for location-specific daylight. The provider offers free non-commercial API access; review its terms before commercial distribution.

## Open decisions

Target Mac hardware, preferred terminal, music player, implementation language, and final project name remain to be confirmed.
