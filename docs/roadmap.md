# Roadmap

Each milestone ends with something runnable and a go/no-go check. Order was chosen to prove the
riskiest part first (accurate audio on a real phone) before building features on top of it.

Platform order: **Android first** (physical phone available), **iPhone later** (simulator only
until a device is available).

## Milestones

### M0 — Setup and audio spike (in progress)

- [x] Repository, Rust workspace, `scripts/doctor.sh`, `scripts/test-all.sh`
- [x] Toolchain: Rust, cargo-ndk, JDK 21, Android SDK/NDK
- [x] Engine: scheduler, click synth, live controls — 17 tests incl. one-hour drift
- [x] FFI crate with UniFFI `Metronome` object and AAudio output; cross-compiles for arm64
- [x] Generate Kotlin bindings and wire the native library into a Gradle build (one command:
      `./gradlew assembleDebug`)
- [x] Minimal Compose app: start/stop, tempo ±, beats per bar, diagnostics line
- [x] Minimal foreground service so playback survives the screen locking
- [ ] **Go/no-go:** steady click on a physical phone with the screen locked, glitch-free
      stream confirmed in diagnostics, no audible jitter. *Mostly met on a Pixel 8 Pro
      (Android 17), see [testing.md](testing.md#measured-results): in `PowerSaving` mode a
      33-minute run on battery (30.5 min in deep Doze) had 0 underruns and 10× fewer CPU
      wake-ups; the earlier `LowLatency` runs had 4 underruns each. Adaptive buffer growth is
      implemented and unit-tested. Remaining: a listening check, a longer run using the new
      `delivered` frame counter, and at least one other device.*

### M1 — Core in Rust

- [x] Engine basics
- [ ] Subdivisions, per-beat accents, count-in, selectable sounds
- [ ] Markdown store: front-matter parser/writer, round-trip safe, atomic writes
- [ ] Library model: projects, songs, setlists, settings; slugs; setlist link resolution
- [ ] Zip export/import with zip-slip protection
- [ ] Parser fuzz/property tests
- [ ] **Done when:** `cargo test` is green and [file-format.md](file-format.md) matches the code

### M2 — Android metronome screen

- [ ] Big tempo display, ± buttons with hold-to-repeat, slider/dial, tap-to-type, tap tempo
- [ ] Time-signature, subdivision and accent editing
- [ ] Beat flash locked to the audio clock (three styles, intensity, offset); haptic tick
- [ ] Audio focus, notification controls, keep-screen-on
- [ ] **Done when:** every metronome feature is usable on the phone and the blink visibly follows
      the sound

### M3 — Android library

- [ ] Projects, songs, setlists (including drag reorder, rename, duplicate, delete)
- [ ] Setlist performance mode with large Next/Previous
- [ ] Settings screen backed by `settings.md`
- [ ] In-app "edit as markdown" with validation
- [ ] Export / import as zip
- [ ] **Done when:** create → save → reopen → export → import on another install works end to end

### M4 — Hardening and release build

- [ ] Interruption and permission handling (notifications, audio focus, disconnects)
- [ ] Accessibility (font scaling, contrast, screen reader labels, flash-safe defaults)
- [ ] App icon, signed release APK/AAB, R8 shrinking, size gate under 10 MB
- [ ] Full [on-device checklist](testing.md#on-device-timing-checklist-physical-android-phone) passes
- [ ] Consider targeting API 37 (Android 17 background-audio rules)

### M5 — iPhone app

- [ ] Xcode project, SwiftUI screens mirroring M2–M3, bindings from the same Rust core
- [ ] CoreAudio output behind the same `Output` interface, background audio session
- [ ] A library exported from Android imports unchanged on iPhone
- [ ] Timing verification on a physical iPhone when one is available

## Out of scope for v1

Tempo changes within a song (per-section tempo map), tempo ramps / speed trainer, polyrhythms,
footswitch or volume-button control, cloud sync, tuner. The file format leaves room for sections.

## Risks

| Risk | Mitigation |
|---|---|
| Audio glitches (underruns) vary between Android devices | Power-saving AAudio mode with a large buffer (a metronome does not need low latency), sample-accurate scheduling, a buffer that grows on underruns, M0 go/no-go on a real phone, xrun and delivered-frame diagnostics |
| Output latency (about 80 ms in power-saving mode) makes the flash appear early | Derive the beat flash from the stream's reported latency (Milestone 2); adjustable visual offset |
| Blink out of sync with sound | UI derives the beat from the audio clock minus reported latency; adjustable visual offset |
| OEM battery savers kill background playback | Foreground service with a notification; in-app hint to exempt the app from battery optimisation |
| Hand-edited files get corrupted or lost | Round-trip-safe parser, defaults plus warnings, atomic writes |
| Three languages to maintain | All logic in Rust, UIs kept thin, the file-format spec is the contract |
| iPhone behaviour cannot be verified without a device | Be explicit that iPhone timing and background claims are unverified until tested |

## Open decisions

- **License** — not chosen yet; the repository is public, so add a license file before accepting
  contributions or reuse.
- **Android application id** — placeholder `no.onstad.metronom` (in `android/app/build.gradle.kts`
  and the Kotlin package); change it before the first published release, since it is permanent
  once published.
- **Compound-meter convention** — BPM counts the denominator note (6/8 at 120 = 120 eighth
  pulses). Revisit if a "dotted-quarter" option is wanted.
