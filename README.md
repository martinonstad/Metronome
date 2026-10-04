# Metronom

A small, offline metronome for Android (iPhone later). Tempo and time signature are quick to
change, the beat is played **and** flashed on screen, and everything you save — settings, songs,
setlists, band/project groups — is a plain markdown file you can edit by hand and move to a new
phone.

> **Status: early development (Milestone 0 of 5).** The metronome engine is written and tested
> on the development machine, and the audio output cross-compiles for Android. The Android app,
> the markdown library and the iPhone app are not built yet. See [Status](#status) and the
> [roadmap](docs/roadmap.md).

## Goals

- **Simple and fully offline.** No account, no network access, no INTERNET permission.
- **Accurate.** Beats are placed on exact audio sample positions, so there is no drift and no
  jitter from UI timers. The blink is derived from the audio clock, so it stays locked to the
  sound.
- **Your data is yours.** Settings, songs, setlists and projects are markdown files with a tiny
  front-matter header. Edit them in any text editor; export/import moves them as a zip.
- **Small and reliable.** Rust for everything that matters (timing, files); thin native UIs.
  Target: release APK under 10 MB, sounds synthesized in code (no audio assets).

## Planned features (v1)

| Area | Features |
|---|---|
| Metronome | Tempo 20–400 BPM, tap tempo, time signatures (1–16 over 2/4/8/16), subdivisions, per-beat accents (strong / normal / muted), count-in, 4 built-in sounds, volume |
| Visual | Beat flash in three styles (full screen, edge glow, beat dots), adjustable intensity, optional haptic tick, visual-latency offset for Bluetooth |
| Library | Projects (bands) containing songs and setlists; a preset is simply a song; setlist performance mode with big Next/Previous |
| Files | Everything stored as markdown; in-app "edit as markdown"; export/import as a zip |
| Phone | Plays with the screen locked (foreground service), pauses on calls, optional keep-screen-on |

Not in v1: tempo changes inside a song, tempo ramps, polyrhythms, footswitch control, cloud sync.

## Status

| Milestone | State |
|---|---|
| **M0** Setup + audio spike | In progress. Repo, toolchain, scripts, Rust engine (with tests) and Android audio output are done. Android app shell and the on-device latency check are next |
| **M1** Core: engine + markdown store | Engine done; markdown store and the [file format](docs/file-format.md) are specified but not implemented |
| **M2** Android metronome screen | Not started |
| **M3** Android library (projects, songs, setlists, export/import) | Not started |
| **M4** Hardening + release build | Not started |
| **M5** iPhone app | Not started (needs Xcode) |

Nothing has been verified on a physical device yet; timing and background-playback claims in
these docs describe the design, not measured results.

## Quick start (development)

Requires macOS with Homebrew. Full setup is in [docs/development.md](docs/development.md).

```bash
scripts/doctor.sh      # shows what is installed, what is missing, and how to fix it
scripts/test-all.sh    # format check, clippy, all Rust tests
```

Homebrew's `rustup` is not on your `PATH` by default; the scripts source `scripts/env.sh` for
you. To use the toolchain in your own shell: `. scripts/env.sh`.

## Repository layout

| Path | Contents |
|---|---|
| `core/metronom-core` | Pure Rust: metronome engine (scheduler, click synth, live controls); later the markdown store. No OS dependencies, fully host-testable |
| `core/metronom-ffi` | UniFFI interface the UIs call, plus the platform audio output (Android AAudio today) |
| `android/` | Android app: Kotlin + Jetpack Compose |
| `ios/` | iPhone app: SwiftUI (later milestone) |
| `docs/` | Architecture, file format, development guide, testing, roadmap |
| `testdata/` | Sample libraries for tests |
| `scripts/` | `doctor.sh`, `test-all.sh`, `env.sh` |

## Documentation

| Document | What it covers |
|---|---|
| [docs/architecture.md](docs/architecture.md) | Stack, layers, timing and audio design, visual sync, decisions and rejected alternatives |
| [docs/file-format.md](docs/file-format.md) | The markdown file format (the contract between the app and your files) |
| [docs/development.md](docs/development.md) | Toolchain setup, build and run, troubleshooting |
| [docs/testing.md](docs/testing.md) | Test strategy, commands, on-device timing checklist |
| [docs/roadmap.md](docs/roadmap.md) | Milestones with acceptance criteria, risks, open decisions |

## License

Not yet chosen. Until a license file is added, all rights are reserved by the author.
