# Metronom

A small, offline metronome for Android (iPhone later), built for gigs. Set the tempo and beats
per bar on one screen, see the beat flash in time with the click, and step through a setlist with
one tap per song. Everything you save — settings, songs, setlists — is a plain markdown file you
can edit by hand and move to a new phone.

> **Status: early development.** The engine is written and well tested, the audio has been
> measured on one phone (a Pixel 8 Pro: steady for 30+ minutes with the screen off, including in
> deep Doze on battery, with no underruns in the power-saving mode), and the one-screen main
> screen is built and its controls work on the phone (whether the flash looks in time with the
> click is still for a human to judge). The song/setlist library exists in the core and is
> tested, but no screens use it yet; gig mode and the iPhone app are not built. The design was agreed on 2026-10-04: see [docs/design.md](docs/design.md).
> Progress is in the [roadmap](docs/roadmap.md).

## Goals

- **Simple and fully offline.** No account, no network access, no INTERNET permission.
- **Accurate.** Beats are placed on exact audio sample positions, so there is no drift and no
  jitter from UI timers. The blink is derived from the audio clock, so it stays locked to the
  sound.
- **Your data is yours.** Settings, songs and setlists are markdown files: songs in one table,
  one file per setlist. Edit them in any text editor; export/import moves them as a zip.
- **Small and reliable.** Rust for everything that matters (timing, files); thin native UIs.
  Target: release APK under 10 MB, sounds synthesized in code (no audio assets).

## Features

| Area | What it does |
|---|---|
| Metronome | One screen, no scrolling. Tempo 30–300 BPM: tap the number to type it, or use the slider, −/+ or tap tempo. Beats per bar 1–99; the first beat of each bar is accented. Four synthesized sounds, volume |
| Visual | A flash bar and one dot per beat, driven by the audio clock so they line up with what you hear |
| Library | Every song in one list; setlists tagged with a band/project; create, edit, delete, reorder and copy in the app |
| Gig mode | Pick a setlist, then **Next song** is one tap; the click keeps running and switches tempo on the next beat. Jump to any song from a list |
| Files | Everything stored as markdown; export/import as a zip |
| Phone | Plays with the screen locked (foreground service), pauses on calls, optional keep-screen-on |

Deliberately left out: count-in, subdivisions, time-signature denominators, per-beat accents,
per-setlist tempo overrides, extra flash styles. Possible later: foot pedal / volume-key control.
See [docs/design.md](docs/design.md) for the reasoning.

## Status

| Milestone | State |
|---|---|
| **M0** Setup + audio spike | Done except a listening check and a second device: the audio is steady and glitch-free on one phone (see [testing](docs/testing.md#measured-results)) |
| **M1** Core in Rust | Done. Engine (65 tests) and the markdown library: songs table, setlists, settings, zip export/import ([file format](docs/file-format.md); 145 library tests). Not yet connected to the app |
| **M2** Android main screen | Built and checked on the phone with adb; judging the flash by eye and settings are still to do |
| **M3** Library and gig mode | In progress. The library is exposed to the app; the songs list and editor and the setlists list and editor are built and work on the phone. Gig mode, settings and export/import screens are next |
| **M4** Hardening + release build | Not started |
| **M5** iPhone app | Not started (needs Xcode) |

Everything measured so far comes from one phone, a Pixel 8 Pro running Android 17; the iPhone
side is unverified.

## Quick start (development)

Requires macOS with Homebrew. Full setup is in [docs/development.md](docs/development.md).

```bash
scripts/doctor.sh      # shows what is installed, what is missing, and how to fix it
scripts/test-all.sh    # format check, clippy, all Rust tests, Android debug build
```

Build and install the Android app on a connected phone (USB debugging on):

```bash
cd android && . ../scripts/env.sh && ./gradlew installDebug
```

Homebrew's `rustup` is not on your `PATH` by default; the scripts source `scripts/env.sh` for
you. To use the toolchain in your own shell: `. scripts/env.sh`.

## Repository layout

| Path | Contents |
|---|---|
| `core/metronom-core` | Pure Rust: metronome engine (scheduler, click synth, live controls); later the markdown store. No OS dependencies, fully host-testable |
| `core/metronom-ffi` | UniFFI interface the UIs call, plus the platform audio output (Android AAudio today) |
| `android/` | Android app: Kotlin + Jetpack Compose. Gradle builds the Rust library and Kotlin bindings automatically |
| `ios/` | iPhone app: SwiftUI (later milestone) |
| `docs/` | Design, architecture, file format, development guide, testing, roadmap |
| `testdata/` | Sample libraries for tests |
| `scripts/` | `doctor.sh`, `test-all.sh`, `env.sh` |

## Documentation

| Document | What it covers |
|---|---|
| [docs/design.md](docs/design.md) | The agreed screens, behaviour and decisions (what the app does and does not do) |
| [docs/architecture.md](docs/architecture.md) | Stack, layers, timing and audio design, visual sync, decisions and rejected alternatives |
| [docs/file-format.md](docs/file-format.md) | The markdown file format (the contract between the app and your files) |
| [docs/development.md](docs/development.md) | Toolchain setup, build and run, troubleshooting |
| [docs/testing.md](docs/testing.md) | Test strategy, commands, on-device timing checklist |
| [docs/roadmap.md](docs/roadmap.md) | Milestones with acceptance criteria, risks, open decisions |

## License

Not yet chosen. Until a license file is added, all rights are reserved by the author.
