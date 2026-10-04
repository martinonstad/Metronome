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
- [x] Markdown store for the agreed [file format](file-format.md): `settings.md`, the `songs.md`
      table, `setlists/*.md`; round-trip safe (unknown keys, extra columns, surrounding text),
      atomic writes
- [x] Library logic: create/edit/delete songs and setlists, rename a song everywhere it is used,
      delete a song from every setlist, reorder, copy a setlist, detect missing songs, unique
      titles
- [x] Zip export/import with zip-slip protection and size limits, tested against the real `zip`
      and `unzip`
- [x] Property tests: arbitrary text round-trips byte for byte in every parser; random library
      sessions keep the rules and survive a save and reopen
- [x] **Done when:** `cargo test` is green and [file-format.md](file-format.md) matches the code.
      *Done (145 library tests). Not yet reachable from the app: the FFI layer and the screens
      that use it are Milestone 3.*

### M2 — Android main screen

- [x] The one-screen manual metronome ([design.md](design.md)): tempo by tapping the number or
      the slider, −/+, beats per bar, tap tempo, flash bar and beat dots locked to the audio
      clock, Start/Stop. Checked on the Pixel with adb: it fits without scrolling, typing a
      tempo works (and out-of-range values are rejected with a message), the slider, −/+,
      beats per bar, Start and Stop work ([testing.md](testing.md#the-one-screen-main-screen-on-the-pixel-8-pro-manual-2026-10-04-debug-build))
- [ ] Judge by hand: does the flash line up with the click, how do the targets feel with a real
      finger, and does tap tempo work. The first beat of a run is shown about 80 ms late (no
      presentation timestamp yet); decide whether that matters
- [x] Settings screen: sound, volume, flash timing (`visual_offset_ms`), keep-screen-on, stored in
      `settings.md` (see M3 below)
- [x] Audio focus, notification controls, keep-screen-on (see M4)
- [ ] **Done when:** the manual metronome is pleasant to use on the phone and the flash is in
      time with the sound

### M3 — Library and gig mode (Android)

- [x] Library exposed to the app (`SongLibrary` over UniFFI, every change saved at once; 13 tests)
- [x] Songs list and editor: add, edit, delete (says which setlists use the song), search,
      "Play this song now". Run on the Pixel with adb, files read off the phone
      ([testing.md](testing.md#the-songs-screens-on-the-pixel-8-pro-manual-2026-10-04-debug-build));
      still to judge by hand: how the screens feel
- [x] Setlists list (grouped by band) and editor: create, rename, change band, add/remove/reorder
      songs (move up/down), copy, delete. Run on the Pixel with adb, files read off the phone
      ([testing.md](testing.md#the-setlists-screens-on-the-pixel-8-pro-manual-2026-10-04-debug-build));
      still to judge by hand: how the screens feel, and dragging to reorder (move up/down only for now)
- [x] Gig screen: current song, one-tap Next and a small Previous, song list (tap the strip or
      swipe up) to jump to any song, screen kept awake, Back to the manual screen. Run on the
      Pixel with adb and the beat log
      ([testing.md](testing.md#the-gig-screen-on-the-pixel-8-pro-manual-2026-10-04-debug-build));
      still to judge by hand: how it feels at a real rehearsal, and in bright light or on a stand.
      `last_setlist` in `settings.md` is not used yet (the app does not reopen the setlist on launch)
- [x] Settings screen: sound (click, wood, beep, rim), volume, flash timing, keep the screen on.
      Saved at once; sound and volume reach the audio engine at launch and when changed. Keep-awake
      also applies on the manual screen while the click runs. Run on the Pixel with adb, files
      read off the phone and the beat log
      ([testing.md](testing.md#the-settings-and-the-export--import-screens-on-the-pixel-8-pro-manual-2026-10-04-debug-build));
      **not judged:** whether a flash timing of a given size lines up with the sound by eye, and
      whether it can correct Bluetooth headphones (none were connected)
- [x] Export / import as zip: export saves the whole library through Android's file picker;
      import asks what to do with songs, setlists and settings that already exist and then shows a
      report. Refusals (unsafe paths, not a zip) change nothing and say why
- [x] **Done when:** you can build a setlist, walk through it with one tap per song while the
      click keeps running, export the library and import it on another install. **Done on one
      phone:** the library was exported, the app's data wiped (a fresh install) and the zip
      imported; the setlists and settings came back byte for byte, the songs with the same data
      in the app's own table layout. **Not tried:** a second physical device

### M4 — Hardening and release build

- [x] Interruption and permission handling (notifications, audio focus, disconnects): the click
      stops, and the service with its notification, when another app or a call takes the audio,
      when headphones are unplugged and when the audio device goes away; it never restarts by
      itself. Verified on the Pixel: another app taking the audio, and playback starting with the
      notification permission denied
      ([testing.md](testing.md#interruptions-and-accessibility-on-the-pixel-8-pro-manual-2026-10-04-debug-build)).
      **Not yet tried (needs hardware or your hands):** headphones unplugged, a Bluetooth device
      disconnecting, a real call, and the notification itself with the permission granted
- [x] Accessibility, first pass: screen-reader labels for the −/+ buttons, the song position and
      the song-list strip; the new song is announced when you press Next (a live region); touch
      targets of at least 48 dp; the manual and gig screens fit at the largest font setting and
      scroll instead of clipping on a small screen; large numerals keep a sensible size. Checked
      by screenshots at 2× text, on a small screen and in the light theme. **Not done:** a walk-through with TalkBack
      switched on, and a measured contrast check
- [x] Flash safety assessed (see testing.md): the flash is a thin bar and small dots, well below
      the area at which flashing counts as a risk, even at 300 BPM (5 flashes a second); there is
      no option to turn the flash off
- [x] App icon (a metronome, adaptive and themed), R8 shrinking with the JNA/UniFFI keep rules,
      signing from environment variables (no key in the repository), size gate under 10 MB in
      `scripts/test-all.sh` (the release APK is 2.4 MB), lint in `scripts/test-all.sh`. A signed
      release build was installed on the Pixel and run: the library opens, a song is saved,
      playback starts and stops, and the export works
      ([testing.md](testing.md#the-release-build-on-the-pixel-8-pro-2026-10-05)). **Not done:**
      your own release key (see [development.md](development.md#release-build)), and a
      bundle (`bundleRelease`) was not built
- [ ] Full [on-device checklist](testing.md#on-device-timing-checklist-physical-android-phone) passes
- [ ] Consider targeting API 37 (Android 17 background-audio rules): the platform is available
      from `sdkmanager` (`platforms;android-37.0`) but is not installed here and the app still
      targets 36; the foreground service is in place. Decision pending (it needs a download and a
      re-test)

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
