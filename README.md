# htopModern

A cozy, vintage-inspired macOS terminal dashboard combining htop-style system monitoring with animated scenes, music controls, todos, and weather.

## Status

This repository currently contains the agreed interactive browser design prototype. The terminal application has not been implemented yet. System metrics and music information in the prototype are illustrative; music controls do not control a real player, and todos are not persisted.

## Preview

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
