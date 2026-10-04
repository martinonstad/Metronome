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

## What exists today (218 tests)

### `metronom-core` engine and helpers (65 tests)

| Module | Tests | What is proven |
|---|---|---|
| Scheduler | 11 | Beats land on exact frames (120 BPM / 48 kHz: 0, 24 000, 48 000 …); placement and beat numbers are identical for block sizes 1–1024; **one simulated hour** at 97 BPM stays within one frame; beats cycle through the bar, including a 99-beat bar; a tempo change waits for the next beat (no short or doubled beat); **a new bar generation makes the next beat beat 1 with the new tempo applying from it**; shrinking the bar mid-way never emits an out-of-range beat; restart begins a fresh bar; NaN, tempos below 30 or above 300, zero-beat and oversized bars are clamped, not fatal |
| Synth | 6 | All four sounds at both levels are audible, never clip, end faded out (no trailing tick) and finish within their length; strong > normal for every sound; the sounds are audibly different; a click is bit-identical every time and for block sizes 1–1024; retriggering restarts the voice |
| Engine (end to end) | 10 | Audio is bit-identical for block sizes 1–4096; the click starts exactly on the beat frame; silence between clicks; the first beat of the bar is louder; output within ±1 and volume scales it; each sound plays and differs; **a song change starts the new song on a strong beat**; every beat is logged for the UI; restart works |
| Controls | 7 | Defaults; tempo clamped to 30–300 and NaN ignored; beats per bar clamped to 1–99; `restart_bar` is visible to the engine and changes nothing else; sound choice; volume clamping; the snapshot reflects every setting |
| Settings | 2 | Sound index round-trip with fallback; defaults |
| Timeline | 6 | Entries round-trip through the packed word (beats up to 99); the newest beat at or before a frame is found; beats the speaker has not reached yet are not returned; only the 64 most recent beats are kept; `clear` forgets a previous stream |
| Visual sync | 7 | `heard_frame` follows the clock from the anchor, can look back, and is negative before the stream is presented; the flash reports the beat being heard and its age; scheduled-but-not-yet-audible beats are ignored; nothing is shown before the first beat is heard; the whole chain with an engine 80 ms ahead gives the beat the ear hears |
| Tap tempo | 8 | Needs two taps; steady taps give the exact tempo; uneven taps are averaged; only the latest six taps count so the tempo can change; a pause over 2 s or a backwards clock starts over; the result is clamped to 30–300; `reset` |
| Buffer tuning | 9 | Never grows without underruns; one burst per new underrun; the same count is not counted twice; bursts of underruns grow one step at a time; stops at the maximum and at capacity; AAudio error codes ignored; the stream size is only queried when needed; degenerate input does not panic |

### `metronom-core` library: markdown files, library logic, zip (145 tests)

| Module | Tests | What is proven |
|---|---|---|
| Text helpers | 7 | Line endings and byte-order mark; titles compare ignoring case and surrounding spaces; file-name slugs are lowercase ASCII with accents folded (`Blåbærsyltetøy` → `blabaersyltetoy`), never contain path characters, never empty; unique suffixes |
| Header (`---` block) | 15 | Bare, quoted and commented values; case-insensitive keys, last duplicate wins; no header, unclosed header, empty header; unknown lines kept and reported; **property: any text renders back byte for byte**; setting a value keeps position, spelling and comment; values that need quoting round-trip; a value cannot inject lines; **property: setting one key leaves every other line alone** |
| Songs table | 21 | The sample table; columns by name in any order; **extra columns, text before and after, and untouched cell spelling survive an edit**; `|` in titles and notes; repeated and empty titles kept in the file but ignored; bad, clamped and missing values with warnings; a file with no table gets one; another kind of table is not mistaken for the song list; Beats/Notes columns added only when needed; uniqueness; rename, remove; **properties: arbitrary text renders back exactly, written songs read back identical, editing one song never changes the others** |
| Setlists | 20 | Heading, band and songs; list markers and indentation; first list only; code fences; **editing the list keeps header, heading and notes**; reorder, remove, insert, rename song, remove song; name and band changes; new setlist; copy; an unedited setlist moved to a new file name is written as read; **properties: arbitrary text renders back exactly; random edits survive a write and read** |
| Settings file | 11 | Defaults and the template; every key; bad and out-of-range values with warnings; **only changed keys are written, unknown keys and comments kept**; a file from a newer version is read but never changed |
| Storage | 5 | Path validation (no `..`, absolute paths, backslashes); memory and disk storage behave the same; no temporary files left; **a failed disk write leaves the old file intact**; simulated interrupted saves |
| Library | 28 | Opening hand-written files; **nothing written when nothing changed**; a full session saves and reopens; unique titles and non-empty names; **rename a song → renamed in every setlist**; **delete a song → removed from every setlist, with a report**; only existing songs can be added to a setlist; reorder/remove/out-of-range; copy (and copy of a copy); rename keeps the file name; delete a setlist removes its file at the next save and never reuses a pending name; **hand edits survive everything the app does**; missing songs; grouping by band; CRLF and byte-order mark; warnings carry the file name; **an unreadable `songs.md` or setlist is never overwritten**; an interrupted save loses nothing and can be retried; saving twice writes once; a newer settings file is protected; **model-based property test: 64 random sessions of add/edit/delete/reorder/copy/save-and-reopen keep every rule** |
| Zip reader and writer | 18 | CRC-32 check value; stored and deflated entries; folders skipped; non-zip input; limits applied before anything big is made; **a zip bomb is refused**; encrypted and exotic entries refused; damaged content detected; duplicate names and backslashes refused; **our archives open in the real `unzip`, and archives made by the real `zip` are read**; **properties: arbitrary bytes never panic; a damaged archive never panics and never returns wrong data; any files round-trip** |
| Archive (export/import) | 20 | Export then import recreates the library byte for byte; only library files are exported; a zipped folder is unwrapped; **path tricks refuse the whole archive and change nothing**; files outside the layout and system files are ignored with a reason; songs skipped or overwritten; setlists skipped, overwritten or kept both; **an imported setlist is written exactly as read**; an import can never overwrite an unreadable file; settings skipped or overwritten, and a newer settings file does not stop the rest; an unreadable `songs.md` stops the import before anything changes; archives beyond the limits refused; **property: any library survives export and import** |

### `metronom-ffi` (8 tests)

Settings round-trip through the exported object; tempo and bar length are clamped (30–300,
1–99); switching song changes tempo and bar length without touching the sound; the sound mirror
enum converts both ways; tapping sets the tempo through the exported object; there is nothing to
show while stopped; `stop()` is idempotent; starting without an audio backend returns an error
instead of crashing (host platforms).

### The one-screen main screen on the Pixel 8 Pro (manual, 2026-10-04, debug build)

Driven through the real app with adb (screen taps and UI dumps):

| Check | Result |
|---|---|
| Everything fits on one screen without scrolling | **Yes**: mode label, beat dots, tempo, slider with −/+, beats per bar, tap tempo and Start were all visible at once |
| Tap the tempo number, type 88, OK | Tempo became 88 |
| Type 999, OK | "Enter a tempo from 30 to 300" is shown; Cancel leaves the tempo at 88 |
| Beats per bar + | 4 → 6 |
| Drag the slider from 25 % to 75 % of its width | Tempo 228 (30 + 0.75 × 270 ≈ 232) |
| − button three times | 228 → 225 |
| Tap tempo | Works, but could not be measured: each `adb input tap` takes a few hundred milliseconds to launch, so taps meant to be 500 ms apart were about 625 ms apart and the tempo came out as 96. Needs a real finger |
| Start, play 14 s, Stop | Beats cycled 0-1-2-3 correctly; 0 underruns; `delivered` and latency (191 ms) normal; Stop ended the service and silenced the audio |
| Flash timing, 32 beats at 120 BPM | Spacing between beats on screen: mean 497.6 ms, max 500.2 ms. Age of a beat on the first frame that showed it: 2.5–10 ms in steady state. **The first beat of a run appeared 77 ms late**: the system has no presentation timestamp for the first few tens of milliseconds, so nothing can be shown yet |

Not judged: whether the flash looks in time with the click, how big and easy the targets feel
when used with a real finger, and tap tempo.

### Beat flash timing (manual, 2026-10-04, Pixel 8 Pro, debug build, `PowerSaving` mode)

The screen's frame loop calls `visual_state` once per display frame and, in debug builds, logs
every time the displayed click changes (`adb logcat -s MetronomFlash`), including how old the
click already was on the first frame that showed it. 34 beats at 120 BPM were captured:

| Check | Result |
|---|---|
| Age of a click on the first frame showing it | 6–16 ms: always within one display frame (the phone alternated between 120 Hz and 60 Hz) |
| Time between consecutive beats on screen | mean 501.8 ms (500 expected); min 491.7 ms; one 558 ms gap between the first two beats, at start-up |
| **Output latency measured from the stream timestamps** | **190–238 ms** (eight readings, median about 200 ms) |

What this shows: the pipeline from audio timeline to display frame is correct and steady **as
far as the stream's own timestamps are concerned**. What it does *not* show: whether those
timestamps match what the speaker really does. That can only be judged by looking and
listening: **does the flash appear together with the click, or early or late?** If it is
consistently off, the planned `visual_offset_ms` setting corrects it. The latency readings
varied by about ±25 ms between samples, so some flash jitter may be visible; this is unjudged.

## Planned suites

| Suite | Milestone | Content |
|---|---|---|
| Android UI | M2–M3 | Compose tests: type a tempo (valid and invalid), change tempo and beats, save a song, build a setlist, walk through it with Next/Previous, export → import round trip |
| Binding smoke test | M2 | The Kotlin ↔ Rust call path on a device or emulator |
| Size gate | M4 | Fails the build if the release APK exceeds 10 MB |

## On-device timing checklist (physical Android phone)

Automated tests prove the engine places beats correctly. They cannot prove what the phone's
audio stack does with them, so run this checklist on real hardware at the end of Milestone 0 and
again before each release. Use `Metronome::diagnostics()` (shown in the app) for the numbers.

| # | Check | Pass criteria |
|---|---|---|
| 1 | Start at 120 BPM, listen for 5 minutes | Steady, no audible jitter or clicks/pops |
| 2 | Diagnostics line after 5 minutes | Performance mode is the one requested (`PowerSaving`); xruns stays at 0 (or very low); `delivered` advances by about 48 000 per second |
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

### Battery run, `PowerSaving` mode — 2026-10-04, 33 minutes

The experiment proposed after the run above: request `PowerSaving` instead of `LowLatency`
(a metronome does not need low latency). Same phone, same procedure: started on USB power,
screen off, unplugged 18:03:02, replugged 18:35:11; 198 lines recorded.

| Check | `LowLatency` (above) | `PowerSaving` |
|---|---|---|
| Granted | burst 96 frames (2 ms), buffer 384 → 576 | burst 1922 frames (40 ms), buffer 3844 frames (80 ms, the capacity) |
| Total output latency | not measured at the time | measured later: **190–240 ms** (see below) |
| On battery, screen off | 30.8 min | 32.0 min |
| Deep Doze | 29.7 min | **30.5 min** |
| Service survived | yes | yes (no `service destroyed` line) |
| **Underruns** | **4** (at 6.4 min into Doze) | **0 for the whole run** |
| CPU wake-ups (audio callbacks) | 500 per second | **about 49 per second** (10× fewer) |
| Callbacks per 10 s interval | 5012–5029 | 488–498 (mean 493.6) |
| Battery level | not recorded | 99 % → 98 % (whole percent: too coarse to compare) |

Reading the results:

- **Underruns: 0 versus 4.** With one run each on one phone this is encouraging rather than
  proof, but the earlier underruns appeared in both `LowLatency` runs within minutes of the
  screen going off, and none appeared in 30.5 minutes of deep Doze here.
- **The callback rate is the same on USB power (49.06 per second) and in deep Doze (49.12 per
  second),** and the per-interval counts form a tight bell curve (no low outliers, which a stall
  would produce). The 488–498 spread is timer jitter in the logging, not lost audio.
- **Callback counts are a weaker continuity measure in this mode** than they were at 500 per
  second, because callbacks carry about 980–1000 frames and their size varies. The `delivered`
  frame counter was added afterwards for this reason. A 14-second check on the phone showed
  48 058 frames per second against 48 000 expected; it has not yet been used in a long run.
- **Battery drain is not established.** The earlier run did not record the battery level, and
  one percentage point is too coarse. Ten times fewer wake-ups should help, but that is
  expected, not measured.
- **Costs:** output latency. The buffer alone is 80 ms, but the *measured* total is
  **190–240 ms** (an early draft of these notes said "about 80 ms"; that was only the buffer).
  It is inaudible for a click, but the visual flash has to be derived from the stream's own
  timestamps or it appears far too early (it now is, see "Beat flash timing" below).
  Start/stop and tempo changes are heard up to a fifth of a second late.
- **Not covered:** listening judgement of the run, other devices (a device may not grant
  `PowerSaving` or may use smaller bursts, which is what the buffer tuner is for), headphones,
  Bluetooth, calls.

### How to repeat a battery run

1. Debug build installed; start playback from the app, then switch the screen off.
2. Unplug the USB cable. Leave the phone untouched and stationary for at least 30 minutes.
3. Plug it back in and read the record (the app writes it every 10 seconds):
   `adb shell run-as no.onstad.metronom cat files/diagnostics.log`
4. Check: timestamps continuous (a gap of 11 s is normal), every 10-second interval advances
   `delivered` by about 480 000 frames (10 s × 48 kHz), `charging=false` and `interactive=false`
   throughout, the last line is a normal diagnostics line (a `service destroyed` line means the
   service stopped; no closing line means the system killed the process), and the underrun count
   stays at or near 0. `doze=true` shows deep Doze was in effect. (Runs before 2026-10-04 evening
   have no `delivered` field; use the callback counter for those.)
5. Stop playback and check it really stopped (no service record, no further diagnostics lines).
   With the phone locked, `adb shell am force-stop no.onstad.metronom` works; the service is not
   exported, so `am stopservice` does not.

## iPhone

iPhone checks (simulator only until a physical device is available) are added with Milestone 5.
Timing and background-audio claims for iPhone stay unverified until tested on a real device.
