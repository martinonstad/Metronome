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
- **Fix implemented:** the buffer now starts at 4 bursts (about 8 ms) and grows by one burst
  whenever the underrun count rises (see
  [architecture.md](architecture.md#audio-output-on-android-built-tested-on-one-device)). Its
  effect is measured in the battery run below.

### Battery run, 4-burst starting buffer — 2026-10-04, 32 minutes

Same phone and debug build, hands-off. Playback was started on USB power, the screen was
switched off, and the cable was unplugged (17:18:32 phone time) and replugged 31 minutes later
(17:49:29). The app recorded a line every 10 seconds (`files/diagnostics.log`, 191 lines).

| Check | Result |
|---|---|
| On battery with the screen off | `charging=false` and `interactive=false` for 30.8 minutes |
| Deep Doze | `doze=true` from 17:19:42 until the cable was replugged: **29.7 minutes in deep Doze** |
| Service survival | Still running at the end; no `service destroyed` line; no non-`Started` state |
| Audio thread never stalled | **All 190 ten-second intervals contained 5012–5029 callbacks** (about 5000 expected; none fell short). Overall rate 500.04 per second |
| Timestamps | Continuous; longest gap 11 s. (Timestamps have whole-second resolution, so a per-interval rate computed from them swings between 456 and 503; the callback counts above are the reliable measure) |
| **Underruns** | **4 underruns at 17:26:04, in deep Doze, 6.4 minutes after Doze began.** The tuner reacted in the same interval: buffer 384 → 576 frames (4 → 6 bursts, about 12 ms). **No further underruns in the remaining 23 minutes** |

What this shows and does not show:

- **Shown:** on battery, in deep Doze, the foreground service and the audio thread keep running
  with a steady callback rate for half an hour, and the tuner grows the buffer when the device
  underruns.
- **Not fixed:** underruns still happened once, with a buffer already doubled from the first
  run, so at least one glitch was probably audible. It is the same count (4), and again a few
  minutes after the screen went off, as in the first run, which points to a recurring
  stall of more than 8 ms rather than random noise. **The cause is unknown.** Hypotheses
  (untested): deep CPU idle states or frequency scaling while the screen is off, or a periodic
  system task.
- **Not established:** whether growing to 576 frames is what kept the remaining 23 minutes
  clean, or whether the stall simply did not recur. One run on one device cannot say.
- **Not covered:** listening judgement of the run, other devices, headphones, calls.

### Next experiment (proposed, not done)

Try `PowerSaving` performance mode instead of `LowLatency`. A metronome does not need low
latency, and this mode uses larger hardware bursts and far fewer wake-ups (the current stream
wakes the CPU 500 times a second), which should both reduce the chance of a missed deadline and
use less battery during long rehearsals. The blink stays in sync because it is derived from the
stream's reported latency. Repeat the same battery run and compare underruns, callbacks per
second and, if possible, battery drain.

### How to repeat a battery run

1. Debug build installed; start playback from the app, then switch the screen off.
2. Unplug the USB cable. Leave the phone untouched and stationary for at least 30 minutes.
3. Plug it back in and read the record (the app writes it every 10 seconds):
   `adb shell run-as no.onstad.metronom cat files/diagnostics.log`
4. Check: timestamps continuous (a gap of 11 s is normal), every 10-second interval advances the
   callback counter by about 5000, `charging=false` and `interactive=false` throughout, the last
   line is a normal diagnostics line (a `service destroyed` line means the service stopped; no
   closing line means the system killed the process), and the underrun count stays at or near 0.
   `doze=true` shows deep Doze was in effect.
5. Stop playback and check it really stopped (no service record, no further diagnostics lines).
   With the phone locked, `adb shell am force-stop no.onstad.metronom` works; the service is not
   exported, so `am stopservice` does not.

## iPhone

iPhone checks (simulator only until a physical device is available) are added with Milestone 5.
Timing and background-audio claims for iPhone stay unverified until tested on a real device.
