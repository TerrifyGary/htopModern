# Inside htopModern

htopModern is a Rust terminal application. [Ratatui](https://ratatui.rs/) draws the interface, Crossterm reads keyboard events, and `sysinfo` supplies computer statistics.

## Find your way around

| File | Responsibility |
| --- | --- |
| `src/main.rs` | Startup, event loop, panel selection, todo storage, and page layout |
| `src/computer.rs` | CPU meters, system details, and top-three process table |
| `src/lofi.rs` | Procedural scenes and flip-style clock |
| `src/music.rs` | Apple Music workers, playback state, CD, equalizer, and progress |
| `src/apple_music.js` | JXA bridge for reading Apple Music metadata |
| `src/weather.rs` | City lookup, forecasts, cache, and weather artwork |
| `src/graphics.rs` | Terminal graphics detection and background image encoding |
| `src/media_layout.rs` | Shared animation regions, dividers, and information baselines |
| `src/temperature.rs` | Optional `macmon` sensor helper with a timeout |
| `src/settings.rs` | Saved theme and terminal cell proportions |
| `src/theme.rs` | Five interface palettes |

## Keep the terminal responsive

The UI polls worker results through channels. Network requests, temperature sampling, Apple Music calls, and native image encoding run outside the rendering loop.

Music metadata reads and playback commands have separate workers. Playback commands are queued in order. Each command advances an epoch counter; a metadata response from an older epoch is discarded, so an earlier poll cannot overwrite newer playback state. Between polls, elapsed time moves the progress dot when playback is active. The equalizer is decorative.

`media_layout` gives LOFI, Music, and Weather a common baseline. Terminal cells are usually taller than they are wide, so square artwork must account for the cell aspect ratio. Native weather images use the terminal’s image protocol when supported; half-block graphics provide a fallback.

## Documentation in Rust

- `//!` documents a module and its purpose.
- `///` documents the item immediately below it.
- Ordinary `//` comments explain implementation decisions inside a function.

Documentation comments support Markdown, including headings, lists, links, and code fences. Prefer explaining a constraint or decision over restating a function’s name.

Generate browsable documentation, including internal items:

```sh
cargo doc --locked --no-deps --document-private-items --open
```

## Local development

```sh
cargo run --locked
cargo fmt --check
cargo test --locked
cargo clippy --locked -- -D warnings
```

To keep experiments separate from your normal tasks and settings:

```sh
HTOPMODERN_DATA_DIR=/tmp/htopmodern-dev cargo run --locked
```

See the [README](../README.md#build-and-check-from-source) for integration diagnostics. Those commands may access local Music, sensors, or weather services.

## How the release reaches Homebrew

1. The application repository holds the source, MIT license, and versioned release notes.
2. A Git tag such as `v0.1.0` identifies the release commit. GitHub provides its source archive and release page.
3. The separate [Homebrew tap](https://github.com/TerrifyGary/homebrew-tap) contains `Formula/htopmodern.rb`. Its URL selects the archive; its SHA-256 checksum verifies the downloaded contents.
4. Homebrew installs the Rust build dependency and `macmon`, builds the application, and installs `htopmodern` plus the `htm` symlink.
5. The tap’s GitHub Actions workflow checks installation and command smoke tests on an Apple Silicon macOS runner.

The initial formula builds from source. It does not ship prebuilt bottles or belong to Homebrew’s core repository. Documentation changes on `main` do not change an already published tag; a future application release needs a new version, tag, archive checksum, and formula update.
