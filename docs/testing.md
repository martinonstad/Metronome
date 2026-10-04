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

## What exists today (29 tests)

### `metronom-core` — buffer tuning (9 tests)

Never grows without underruns; grows by exactly one burst per new underrun; the same count is
not counted twice; a burst of underruns grows one step at a time; stops at the maximum and at
the stream's capacity; AAudio error codes (negative counts) are ignored; the stream's current
size is only queried when growth is needed; degenerate inputs do not panic.

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

## Measured results

### Pixel 8 Pro, Android 17 (API 37), debug build — 2026-10-04

Milestone 0 spike, 48 kHz. **The phone was connected to the computer by USB (charging) for every
run below.** Doze and several battery-saving behaviours do not apply while charging, so these
results say little about battery-powered behaviour.

| Check | Result |
|---|---|
| Stream granted | `LowLatency`, `Shared`, 48 000 Hz, burst 96 frames (2 ms), buffer 192 frames (4 ms) |
| Foreground service | Running as `mediaPlayback` with its notification |
| App in background, then screen off | Playback continued |
| **Hands-off, screen off, 10 minutes** | Audio callbacks ran at **500.0 per second throughout** (every 60-second sample matched; no stall). Service still running at the end. The screen stayed off (`Dozing`) the whole time |
| Underruns (xruns) in that run | **0 for the first ~4 minutes, then 4 within one minute (between 14:57:55 and 14:58:57), none after.** Not zero |
| Listening check | Reported as "sounds good" by the tester, in a short foreground session; the 10-minute screen-off run was not listened to |

Notes on how the run was measured:

- The numbers come from the app's own diagnostics line, sampled once a minute. The Android log
  buffer is small and kept only about the last four minutes, so the end-of-run summary covers
  that window only; the minute-by-minute samples cover the whole run.
- Two earlier attempts were discarded because playback never started (the screen was locked and
  the Start tap did not reach the app). The test script now refuses to lock the screen unless
  it has confirmed audio is running.

What this does and does not show:

- **Shown:** the low-latency path is granted; the audio thread keeps running with the screen off
  for 10 minutes; the foreground service survives.
- **Not shown:** behaviour on battery (Doze), other devices, Bluetooth or wired headphones,
  phone calls, battery saver, a 30-minute drift check, or any listening judgement of the
  10-minute run.
- **Open issue:** 4 underruns in 10 minutes means the 4 ms buffer (2 bursts) had too little
  slack on this device under some conditions. Each underrun can be an audible glitch.
- **Fix implemented, not yet re-measured:** the buffer now starts at 4 bursts (about 8 ms) and
  grows by one burst whenever the underrun count rises (see
  [architecture.md](architecture.md#audio-output-on-android-built-tested-on-one-device)). A
  short run confirmed the 384-frame start; the long run below still has to be repeated.

### Battery run (hands-off, unplugged) — to do

Goal: show behaviour with Doze and battery saving active, which USB power prevents.

1. Debug build installed; start playback from the app, then switch the screen off.
2. Unplug the USB cable. Leave the phone untouched and stationary for at least 30 minutes.
3. Plug it back in and read the record (the app writes it every 10 seconds):
   `adb shell run-as no.onstad.metronom cat files/diagnostics.log`
4. Pass criteria: the timestamps are continuous (no gap of more than about 10–20 seconds), the
   callback counter advances by about 500 per second, `charging=false` and `interactive=false`
   throughout, the last line is still a normal diagnostics line (a `service destroyed` line means
   the service stopped; no closing line at all means the system killed the process), and the
   underrun count stays at or near 0. `doze=true` appearing shows deep Doze was in effect.

## iPhone

iPhone checks (simulator only until a physical device is available) are added with Milestone 5.
Timing and background-audio claims for iPhone stay unverified until tested on a real device.
