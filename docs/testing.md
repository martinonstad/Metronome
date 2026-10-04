# Testing

The principle: as much as possible is tested on the development machine, with no phone. What
can only be judged on real hardware (latency, background behaviour) has a written checklist.

## Run everything

```bash
scripts/test-all.sh
```

This runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, the same lint
cross-compiled for Android (so the Android-only audio code is checked too) and `cargo test`.
Android and iPhone suites will be added to the same script as those milestones land.

## What exists today (20 tests)

### `metronom-core` — engine (17 tests)

| Area | What is proven |
|---|---|
| Placement | At 120 BPM / 48 kHz pulses start exactly on frames 0, 24 000, 48 000 … |
| Block independence | Pulse positions are identical for block sizes 1, 7, 64, 480, 513, 1024 and 4096; rendered audio is **bit-identical** for block sizes 1–4096 |
| Drift | One simulated hour at 97 BPM (an awkward tempo): every pulse is within one frame of its exact time; error does not accumulate |
| Beats and bars | Beat numbers cycle correctly through a bar (e.g. 3/4) |
| Live changes | A tempo change lands on the next pulse with no short or doubled beat; `restart` begins a fresh bar immediately |
| Robustness | NaN and absurd tempos (1e9 BPM) and zero-beat bars are clamped, not fatal |
| Sound | The click starts at the pulse frame, silence between clicks, downbeat louder, output within ±1, volume scales the output |
| Controls | Defaults, clamping of tempo/beats/volume, NaN ignored |

### `metronom-ffi` (3 tests)

Settings round-trip through the exported object; `stop()` is idempotent; starting without an
audio backend returns an error instead of crashing (host platforms).

## Planned suites

| Suite | Milestone | Content |
|---|---|---|
| Parser and store | M1 | Round-trip golden files; unknown-key and comment preservation; malformed, empty and binary files; long and Unicode names; property tests and fuzzing of the parser |
| Import safety | M1 | Zip-slip (`../` and absolute paths), non-`.md` entries, size and entry-count limits, name collisions, interrupted writes |
| Engine extensions | M1–M2 | Subdivisions, accent patterns, count-in, tempo ramps if added |
| Android UI | M2–M3 | Compose tests: change tempo, save a song, build a setlist, export → import round trip |
| Binding smoke test | M2 | The Kotlin ↔ Rust call path on a device or emulator |
| Size gate | M4 | Fails the build if the release APK exceeds 10 MB |

## On-device timing checklist (physical Android phone)

Automated tests prove the engine places beats correctly. They cannot prove what the phone's
audio stack does with them, so run this checklist on real hardware at the end of Milestone 0 and
again before each release. Use `Metronome::diagnostics()` (shown in the app) for the numbers.

| # | Check | Pass criteria |
|---|---|---|
| 1 | Start at 120 BPM, listen for 5 minutes | Steady, no audible jitter or clicks/pops |
| 2 | Diagnostics line after 5 minutes | Performance mode is `LowLatency`; xruns stays at 0 (or very low) |
| 3 | Lock the screen for 10 minutes | Playback continues without gaps |
| 4 | Switch to another app and back | Playback continues |
| 5 | Plug and unplug wired headphones | Playback resumes on the new output, or stops cleanly and can be restarted |
| 6 | Connect Bluetooth headphones | Plays; note the delay and check `visual_offset_ms` can compensate |
| 7 | Incoming call or another app taking audio focus | Pauses, and does not leave a stuck notification |
| 8 | Battery saver / low-power mode on | Still plays with the screen locked |
| 9 | 30-minute run at 97 BPM | No perceptible drift against a reference metronome or a recorded click track |
| 10 | Optional loopback: record the speaker click with a second device and measure spacing | Inter-click intervals match the expected period to within a few milliseconds |

Record the device model, Android version and results of each run in the pull request or issue
for the milestone. Results from one phone do not generalise to all phones, so note which devices
have been tried.

## iPhone

iPhone checks (simulator only until a physical device is available) are added with Milestone 5.
Timing and background-audio claims for iPhone stay unverified until tested on a real device.
