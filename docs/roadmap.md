# Roadmap

Each milestone ends with something runnable and a go/no-go check. Order was chosen to prove the
riskiest part first (accurate audio on a real phone) before building features on top of it.

Platform order: **Android first** (physical phone available), **iPhone later** (simulator only
until a device is available).

## Milestones

### M0 — Setup and audio spike (done, apart from the go/no-go checks below)

- [x] Repository, Rust workspace, `scripts/doctor.sh`, `scripts/test-all.sh`
- [x] Toolchain: Rust, cargo-ndk, JDK 21, Android SDK/NDK
- [x] Engine: scheduler, click synth, live controls, with a one-hour drift test
- [x] FFI crate with UniFFI `Metronome` object and AAudio output; cross-compiles for arm64
- [x] Generate Kotlin bindings and wire the native library into a Gradle build (one command:
      `./gradlew assembleDebug`)
- [x] Minimal Compose app with a diagnostics line (since replaced by the real main screen, M2)
- [x] Minimal foreground service so playback survives the screen locking
- [ ] **Go/no-go:** steady click on a physical phone with the screen locked, glitch-free
      stream confirmed in diagnostics, no audible jitter. *Mostly met on a Pixel 8 Pro
      (Android 17), see [testing.md](testing.md#measured-results): in `PowerSaving` mode a
      33-minute run on battery (30.5 min in deep Doze) had 0 underruns and 10× fewer CPU
      wake-ups; the earlier `LowLatency` runs had 4 underruns each. Adaptive buffer growth is
      implemented and unit-tested. The sounds were listened to in a short foreground session and
      judged "ok". Remaining: a listening check of a long screen-off run, a longer run using the
      `delivered` frame counter, and at least one other device.*

### M1 — Core in Rust

- [x] Engine: sample-accurate beats, accented first beat, four synthesized sounds, tap tempo,
      buffer tuning, the beat timeline and clock sync (65 core tests, including a one-hour drift
      test)
- [ ] Markdown store for the agreed [file format](file-format.md): `settings.md`, the `songs.md`
      table, `setlists/*.md`; round-trip safe (unknown keys, extra columns, surrounding text),
      atomic writes
- [ ] Library logic: create/edit/delete songs and setlists, rename a song everywhere it is used,
      reorder, copy a setlist, detect missing songs, unique titles
- [ ] Zip export/import with zip-slip protection
- [ ] Parser fuzz/property tests
- [ ] **Done when:** `cargo test` is green and [file-format.md](file-format.md) matches the code

### M2 — Android main screen

- [x] The one-screen manual metronome ([design.md](design.md)): tempo by tapping the number or
      the slider, −/+, beats per bar, tap tempo, flash bar and beat dots locked to the audio
      clock, Start/Stop. Checked on the Pixel with adb: it fits without scrolling, typing a
      tempo works (and out-of-range values are rejected with a message), the slider, −/+,
      beats per bar, Start and Stop work ([testing.md](testing.md#the-one-screen-main-screen-on-the-pixel-8-pro-manual-2026-10-04-debug-build))
- [ ] Judge by hand: does the flash line up with the click, how do the targets feel with a real
      finger, and does tap tempo work. The first beat of a run is shown about 80 ms late (no
      presentation timestamp yet); decide whether that matters
- [ ] Settings: sound and volume, keep-screen-on, visual offset (stored in `settings.md`)
- [ ] Audio focus, notification controls, keep-screen-on
- [ ] **Done when:** the manual metronome is pleasant to use on the phone and the flash is in
      time with the sound

### M3 — Library and gig mode (Android)

- [ ] Songs list and editor: add, edit, delete (with a warning if a setlist uses the song)
- [ ] Setlists list (grouped by band) and editor: create, rename, change band, add/remove/reorder
      songs, copy, delete
- [ ] Gig screen: current song, one-tap Next and a small Previous, song list to jump to any song,
      switching between manual and setlist modes
- [ ] Export / import as zip
- [ ] **Done when:** you can build a setlist, walk through it with one tap per song while the
      click keeps running, export the library and import it on another install

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

## Out of scope for now

Decided against (see [design.md](design.md)): count-in, subdivisions, time-signature
denominators, per-beat accent editing, per-song sounds, per-setlist tempo overrides, extra flash
styles. Possible later: foot pedal / volume-key control for Next, "accent every N beats", tempo
changes inside a song, tempo ramps, polyrhythms, cloud sync.

## Risks

| Risk | Mitigation |
|---|---|
| Audio glitches (underruns) vary between Android devices | Power-saving AAudio mode with a large buffer (a metronome does not need low latency), sample-accurate scheduling, a buffer that grows on underruns, M0 go/no-go on a real phone, xrun and delivered-frame diagnostics |
| Output latency (190–240 ms in power-saving mode on the Pixel 8 Pro) makes a naive flash appear far too early | The flash is derived from the stream's own presentation timestamps (built); adjustable visual offset (planned); alignment judged by eye on the phone |
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
- **Changing song while stopped** — the song loads and the click waits for Start (a default; see
  [design.md](design.md)).
