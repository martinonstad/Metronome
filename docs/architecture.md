# Architecture

This document describes how Metronom is built and why. Parts marked **(built)** exist in the
repository today; parts marked **(planned)** are designed but not yet implemented.

## Overview

All timing, state and file handling live in a shared **Rust core**. Each platform has a thin
native UI that renders state and forwards taps.

```
┌──────────── Android (Kotlin / Compose) ───────┐  ┌──── iPhone (SwiftUI) — later ────┐
│ screens · foreground service · file pickers   │  │ screens · audio session · pickers │
└───────────────┬───────────────────────────────┘  └──────────────┬───────────────────┘
                │ UniFFI-generated bindings                        │
┌───────────────▼──────────────────────────────────────────────────▼───────────────────┐
│ metronom-ffi                                                                          │
│   Metronome object exported through UniFFI · platform audio output                    │
│   (AAudio on Android; CoreAudio on iOS — planned)                                     │
├───────────────────────────────────────────────────────────────────────────────────────┤
│ metronom-core  (pure Rust, no OS dependencies, `#![forbid(unsafe_code)]`)             │
│   engine/  Scheduler · Synth · Controls · Engine                              (built) │
│   store/   markdown + front-matter parser/writer, library, zip import/export (planned)│
└───────────────────────────────────────────────────────────────────────────────────────┘
```

| Layer | Language | Responsibility |
|---|---|---|
| `metronom-core` | Rust | Everything that must be correct and testable without a phone: beat timing, click synthesis, live parameters, and (planned) the markdown library |
| `metronom-ffi` | Rust | The API the UIs call, and the only code that touches OS audio APIs (and therefore the only crate allowed `unsafe`) |
| Android UI | Kotlin + Jetpack Compose | Screens, foreground service, permissions, file pickers |
| iPhone UI | Swift + SwiftUI | Same role (later milestone) |

The UIs hold no business logic. That keeps the two UIs small and prevents the platforms from
drifting apart.

## The engine (built)

Located in `core/metronom-core/src/engine/`.

| Piece | File | Role |
|---|---|---|
| `Scheduler` | `scheduler.rs` | Decides on which audio frame each pulse (beat) starts. Pure timing, no audio |
| `Synth` | `synth.rs` | Generates the click: a short sine burst with a 0.5 ms attack and exponential decay; 1600 Hz on the downbeat, 1000 Hz (quieter) on other beats |
| `Controls` | `controls.rs` | Atomics holding tempo, beats per bar and volume, shared between the UI and the audio thread. Clamps values and ignores NaN |
| `Engine` | `mod.rs` | Combines the above: `render(&mut [f32], timing, volume)` fills a buffer; no allocation, no locks |

### Timing accuracy

- The next pulse's position is kept as an **f64 frame position** and advanced by
  `sample_rate × 60 / bpm` per pulse. Tempos that do not divide the sample rate evenly (e.g. 97
  BPM) therefore never accumulate rounding error.
- A pulse starts on the first whole frame at or after its exact time (`ceil`), so error per pulse
  is under one frame (about 21 µs at 48 kHz) and does not accumulate.
- The tempo is read **when a pulse fires**, so a tempo change takes effect from the next pulse and
  never produces a short or doubled beat. Changing the time signature works the same way.
- Output depends only on how many frames a click has played, never on how the stream is split
  into callback blocks. A test renders the same audio with block sizes from 1 to 4096 frames and
  requires bit-identical results.

### Real-time rules

The audio callback must never wait. In the engine and the Android callback:

- no allocation (buffers are created before the stream starts),
- no locks (UI → audio communication is relaxed atomics),
- no file or network I/O.

### Conventions

- BPM counts **pulses of the time signature's denominator note**. 6/8 at 120 BPM is 120
  eighth-note pulses per minute. Accent patterns express compound grouping (e.g. `X o o X o o`).
- Ranges: 20–400 BPM, 1–16 beats per bar. Out-of-range input is clamped, never rejected.

## Audio output on Android (built, tested on one device)

`core/metronom-ffi/src/audio/android.rs`, using the `ndk` crate's AAudio bindings.

| Setting | Value | Why |
|---|---|---|
| Direction / format | Output, 32-bit float, 2 channels (mono click duplicated) | Float avoids conversion; stereo is the safest universally supported layout |
| Performance mode | **Power saving** | A metronome is not interactive, so it does not need low latency. Power saving uses larger hardware bursts and about a tenth of the CPU wake-ups (on the Pixel 8 Pro: about 50 callbacks a second instead of 500). Measured on that phone: 0 underruns in 32 minutes on battery, versus 4 underruns per run with low latency (see [testing.md](testing.md#measured-results)). The cost is about 80 ms of output latency, which the visual flash must compensate for |
| Sharing mode | Shared | Exclusive can fail on some devices, and nothing here needs it |
| Sample rate | The device's native rate, passed in by the Kotlin side (`AudioManager`) | Avoids resampling, which adds latency; falls back to 48 kHz |
| Buffer size | Starts at 4 bursts (never more than the stream's capacity); grows by one burst each time the device underruns, up to 12 bursts | An underrun is an audible glitch; a few extra milliseconds are not. In power-saving mode on the Pixel 8 Pro the burst is 1922 frames (40 ms) and the buffer starts at its 3844-frame capacity (80 ms), so the tuner has no room to grow there; it matters on devices that grant smaller bursts, and it was exercised with the low-latency mode (buffer 384 → 576 frames after an underrun) |

- **Disconnection.** If the stream errors (headphones unplugged, device switched), an atomic flag
  is set. `Metronome::is_running()` then reports `false`, and `start()` reopens the stream.
- **`unsafe` is limited to two places**, both commented with a SAFETY argument: viewing the
  buffer AAudio hands to the callback as a slice, and marking the stream handle `Send` (AAudio
  streams are documented as thread-safe; the `ndk` crate omits `Send` only because it holds a raw
  handle).
- **Buffer tuning.** The policy lives in `metronom-core` (`tuning.rs`, `BufferTuner`) so it is
  tested on the host. The audio callback owns buffer sizing: on its first call it reads the
  stream's burst and capacity, sets the starting size, then once per callback compares the
  stream's underrun count and requests a larger buffer when it has risen. Doing it all in the
  callback avoids a race with `open()` that could briefly shrink a buffer that had just grown.
- **Diagnostics.** `Metronome::diagnostics()` returns one line (performance mode, sharing, rate,
  burst, buffer, xruns, callbacks, delivered frames, state) for the on-device timing check.
  `delivered` is the total number of frames handed to the stream: it should advance by the
  sample rate per second, which is the direct proof that audio is continuous. (Callback counts
  are not: in power-saving mode callbacks carry roughly 980–1000 frames and their size varies.)
  In debug builds the Android service also logs the line every 2 s and appends a timestamped
  line to `files/diagnostics.log` every 10 s, together with the phone's Doze, interactive and
  charging state and the battery percentage, so a run on battery leaves evidence without a USB
  connection.
- On other platforms a stub `Output` returns an error from `start()`, so the workspace builds and
  tests on a Mac or Linux.

## Rust ⇄ app bridge (built)

[UniFFI](https://mozilla.github.io/uniffi-rs/) generates Kotlin (and later Swift) bindings from
proc-macro annotations on `Metronome`; there is no hand-written JNI. Kotlin loads the library
through JNA, which UniFFI's Kotlin bindings require.

The exported `Metronome` object:

| Method | Purpose |
|---|---|
| `start(sample_rate)` / `stop()` / `is_running()` | Control playback |
| `bpm()` / `set_bpm()` | Tempo |
| `beats_per_bar()` / `set_beats_per_bar()` | Time-signature numerator |
| `volume()` / `set_volume()` | Output level |
| `diagnostics()` | One-line stream description |

Bindings are generated from the **host** build of the library (`libmetronom_ffi.dylib`), not the
Android `.so`, because release builds are stripped and UniFFI's library mode needs the metadata
symbols. The exported interface is identical on every target.

## Android app (spike built; features planned)

Built (Milestone 0 spike, run on a Pixel 8 Pro; see [testing.md](testing.md#measured-results)): `MetronomApp` holds the single
`Metronome`; `PlaybackService` owns playback; `MainActivity` and `MetronomeScreen` offer
start/stop, tempo ±1/±5, beats per bar and a diagnostics line. Everything else below is planned.

- The `Metronome` object lives in the `Application` and is owned by a **foreground service**
  (`mediaPlayback` type) so playback survives the screen locking and the activity going away.
  Apps that target Android 17 (API 37) must run a foreground service to play audio in the
  background ([Android docs](https://developer.android.com/about/versions/17/changes/bg-audio)),
  and it is the right design for earlier versions too.
- The activity sends start/stop intents and reads state; the notification offers Stop.
- Audio focus: pause on calls and when another app takes focus.
- Min SDK 26 (Android 8.0, required by AAudio). Compile/target SDK 36 for now; revisit 37 with the
  foreground-service work.
- ABI: arm64-v8a only. 32-bit ARM and x86 devices are not supported.
- No `INTERNET` permission, ever.

## Visual synchronisation (planned)

The blink must be locked to the *sound*, not to a UI timer.

1. The audio callback records recent pulses `(frame, beat)` in a small lock-free ring buffer and
   publishes the number of frames rendered.
2. On every display frame (Choreographer) the UI estimates the frame currently being *heard*:
   frames rendered minus the output latency reported by AAudio's timestamps.
3. The newest pulse at or before that frame decides which beat is lit and how far its flash has
   decayed.
4. A user setting, `visual_offset_ms`, shifts the result to compensate for Bluetooth headphones.

Flash styles (full screen, edge glow, dots only) and an intensity setting exist because fast
tempos can flash at about 5 Hz, which can be uncomfortable for photosensitive users.

## Storage (planned)

Markdown files in the app's private storage, behind a small storage interface (read, write,
list, delete) so a visible/shared folder can be added later without touching the parser.
Export/import moves a zip of the same layout. Details and rules are in
[file-format.md](file-format.md).

## Build and size

- Release profile: `opt-level = "z"`, LTO, one codegen unit, `panic = "abort"`, stripped.
- The arm64 release library is about 360 KB today.
- Budget: release APK under 10 MB.

## Decisions and rejected alternatives

| Decision | Reasoning |
|---|---|
| Rust core + native UIs | Sample-accurate timing without GC pauses, tiny binaries, and one tested implementation of logic and file handling for both platforms |
| `ndk` crate AAudio instead of `cpal` | `cpal` on Android needs the JavaVM context handed to it; the `ndk` crate talks to AAudio directly with no JNI setup. The output sits behind a tiny interface, so it can be swapped |
| Sounds synthesized in code | No audio assets, a smaller app, and the click is identical on every device |
| A preset is just a song | One concept instead of two; songs outside any band live in an automatic "General" project |
| Hand-written front-matter parser (planned) | Predictable, tiny, and able to preserve unknown keys and comments; a full YAML library is unnecessary |
| Private storage + export/import | Simplest and most robust for v1; in-app "edit as markdown" keeps files hand-editable |

Rejected stacks:

| Option | Why not |
|---|---|
| Flutter / React Native | A 10–20 MB runtime and a third language; accurate audio would still need native or Rust code underneath |
| Tauri 2 (Rust + web UI) | WebView audio and background playback on mobile depend on young plugins and still need custom native code |
| Kotlin Multiplatform | Audio timing and file handling would run on garbage-collected runtimes; weaker fit for "small and effective" |
| Native only, no shared core | Fastest to a first Android app, but logic and file format would be written and debugged twice |

The cost of the chosen stack is three languages (Rust, Kotlin, Swift). It is contained by keeping
the UIs thin and the Rust core the single source of truth.
