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
| `Scheduler` | `scheduler.rs` | Decides on which audio frame each beat starts (`Beat`: the beat's index in the bar). Pure timing, no audio |
| `Synth` | `synth.rs` | Generates the click from a per-sound recipe: a sine burst with an attack, exponential decay, optional pitch sweep and noise, and a 2 ms fade-out so it never ends with a tick. Four sounds (click, wood, beep, rim), each with two levels: strong (first beat of the bar) and normal |
| `Settings`, `Sound` | `settings.rs` | The values the engine reads each audio callback: timing, sound, volume |
| `Controls` | `controls.rs` | Atomics holding tempo, beats per bar, a "bar generation" counter, sound and volume, shared between the UI and the audio thread. Clamps values and ignores NaN. `settings()` takes one consistent snapshot per audio callback |
| `Timeline` | `timeline.rs` | A lock-free log of recent beats, so the UI can show what is being heard (see Visual synchronisation) |
| `Engine` | `mod.rs` | Combines the above: `render(&mut [f32], &Settings)` fills a buffer; no allocation, no locks |

### Timing accuracy

- The next beat's position is kept as an **f64 frame position** and advanced by
  `sample_rate × 60 / bpm` per beat. Tempos that do not divide the sample rate evenly (e.g. 97
  BPM) therefore never accumulate rounding error.
- A beat starts on the first whole frame at or after its exact time (`ceil`), so error per beat
  is under one frame (about 21 µs at 48 kHz) and does not accumulate.
- The tempo that applies to the gap *after* a beat is the one in force when that beat starts, so
  a tempo change takes effect from the next beat and never produces a short or doubled beat. If
  the bar length is shortened mid-bar, the bar rolls over at the next beat so a beat outside the
  new bar is never played.
- **Song changes.** `Controls::restart_bar` bumps a counter; when the scheduler sees it change at
  a beat, that beat becomes beat 1 (the accented one) of a new bar. The tempo change applies
  from that beat on, so a new song starts on its downbeat without a stumble.
- Output depends only on how many frames a click has played, never on how the stream is split
  into callback blocks. A test renders the same audio with block sizes from 1 to 4096 frames and
  requires bit-identical results.

### Real-time rules

The audio callback must never wait. In the engine and the Android callback:

- no allocation (buffers are created before the stream starts),
- no locks (UI → audio communication is relaxed atomics),
- no file or network I/O.

### Conventions

- **Tempo** is 30–300 beats per minute. **Beats per bar** is 1–99; the first beat of each bar is
  the strong click, all others are normal. There is no time-signature denominator, subdivision,
  count-in or per-beat accent: see [design.md](design.md) for why.
- Out-of-range input is clamped (typed tempos in the UI are rejected with a message instead).

## Audio output on Android (built, tested on one device)

`core/metronom-ffi/src/audio/android.rs`, using the `ndk` crate's AAudio bindings.

| Setting | Value | Why |
|---|---|---|
| Direction / format | Output, 32-bit float, 2 channels (mono click duplicated) | Float avoids conversion; stereo is the safest universally supported layout |
| Performance mode | **Power saving** | A metronome is not interactive, so it does not need low latency. Power saving uses larger hardware bursts and about a tenth of the CPU wake-ups (on the Pixel 8 Pro: about 50 callbacks a second instead of 500). Measured on that phone: 0 underruns in 32 minutes on battery, versus 4 underruns per run with low latency (see [testing.md](testing.md#measured-results)). The cost is output latency: measured at **190–240 ms** on the Pixel 8 Pro (the 80 ms buffer plus the rest of the audio path), which the visual flash compensates for using the stream's own timestamps (see Visual synchronisation) |
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
| `beats_per_bar()` / `set_beats_per_bar()` | Beats per bar, 1–99 |
| `restart_bar()` | The next beat becomes beat 1 of a new bar (call on a song change) |
| `sound()` / `set_sound()` | One of four synthesized sounds |
| `visual_state(now_nanos)` | The beat being heard now and its age, for the flash (once per display frame) |
| `output_latency_ms(now_nanos)` | Measured output latency |
| `tap(now_nanos)` | Tap tempo: returns and applies the new tempo |
| `volume()` / `set_volume()` | Output level |
| `diagnostics()` | One-line stream description |

The second exported object, **`SongLibrary`** (`metronom-ffi/src/library.rs`), is the song library
over one folder: `open(root)`, `songs()`, `addSong`, `updateSong`, `deleteSong`, `setlistsUsing`,
the setlist calls (`setlistsByBand`, `setlist`, `createSetlist`, `renameSetlist`,
`setSetlistBand`, `addSongToSetlist`, `removeSongFromSetlist`, `moveSongInSetlist`, `copySetlist`,
`deleteSetlist`), `settings` / `updateSettings`, `exportArchive` / `importArchive` and `warnings`.
Data crosses as plain records (`SongRecord`, `SetlistSummary`, `BandGroup`, `SetlistDetail`, …) and
errors as `LibraryException` variants with the details the UI needs. **Every change is saved at
once** (write-through), so the app cannot forget to save. The calls do file I/O: call them off the
main thread. It is named `SongLibrary` because a class called `Library` would clash with JNA's
`com.sun.jna.Library`, which the generated Kotlin imports.

Bindings are generated from the **host** build of the library (`libmetronom_ffi.dylib`), not the
Android `.so`, because release builds are stripped and UniFFI's library mode needs the metadata
symbols. The exported interface is identical on every target.

## Android app (main screen built; library planned)

Built (see [testing.md](testing.md#measured-results) for what has been measured on a phone):
`MetronomApp` holds the single `Metronome`; `PlaybackService` owns playback; `MainActivity` and
`MetronomeScreen` are the one-screen manual metronome described in [design.md](design.md): a
flash bar and beat dots, the tempo number (tap to type) with a slider and −/+, beats per bar, tap
tempo and Start/Stop. It has been run on the Pixel 8 Pro and its controls checked with adb; the
results are in [testing.md](testing.md#the-one-screen-main-screen-on-the-pixel-8-pro-manual-2026-10-04-debug-build).

Also built (run on the Pixel; see [testing.md](testing.md#the-songs-screens-on-the-pixel-8-pro-manual-2026-10-04-debug-build)): the **songs list** (search, add) and the **song
editor** (title, tempo, beats, notes, save, delete with a note of which setlists use the song, and
"Play this song now"). `LibraryStore` opens the library off the main thread, runs every call on a
background thread and keeps the song list, the setlists grouped by band and a change counter as
Compose state; navigation is one current `Screen` with Back leading to its parent, kept across
rotation. A change that has started always finishes and refreshes those lists, even if the screen
that asked for it has been left.

The **setlists list** (grouped by band, copy and delete per row, "New setlist") and the
**setlist editor** (name and band in a dialog with the existing bands as chips; songs with move
up/down and remove; a searchable song picker that stays open to add several; copy; delete) are
built too; see [testing.md](testing.md#the-setlists-screens-on-the-pixel-8-pro-manual-2026-10-04-debug-build).
Every edit is saved at once, so the editor has no Save button, and the editor allows one change at
a time so a second tap meant for the old order cannot land on the new one.

Planned:

- The gig screen and the settings screen ([roadmap.md](roadmap.md), Milestone 3).
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

## Visual synchronisation (built; timing verified on one device, alignment by eye pending)

The blink must be locked to the *sound*, not to a UI timer, because the audio reaches the
speaker well after the engine renders it (190–240 ms in power-saving mode on the Pixel 8 Pro).

1. **Timeline** (`engine/timeline.rs`): the audio thread logs every beat it schedules into a
   small lock-free ring of 64 entries (frame and beat index), each packed into one atomic word.
   Nothing waits on anything.
2. **Anchor**: Android reports "stream frame F is presented at time T"
   (`AAudioStream_getTimestamp`, `CLOCK_MONOTONIC`). From it, `sync::heard_frame` works out which
   frame is reaching the speaker *now*: `F + (now − T) × sample rate`.
3. **Lookup**: `sync::flash_at` returns the newest beat at or before that frame and how many
   milliseconds ago it became audible. Beats the engine has already scheduled but the speaker
   has not reached yet are ignored.
4. **Display**: `Metronome::visual_state(now_nanos)` is called once per display frame with the
   Compose frame time (the same monotonic clock). The UI draws the flash from `since_ms` with a
   fast decay; the first beat of the bar flashes brighter.
5. A user setting, `visual_offset_ms`, will shift the result to correct for Bluetooth headphones
   or any systematic error in the reported timestamps (planned, not built).

The whole chain except the platform timestamp is pure Rust, tested on the host. The flash is a
bar across the top plus one dot per beat; other flash styles were considered and are not wanted.

**Tap tempo** (`tap.rs`) is also pure Rust: the average interval over the last six taps, a
measurement restarts after a 2 s pause or a clock that goes backwards, and the result is
clamped to 30–300 BPM.

## Library and storage (built in the core; not yet connected to the app)

Located in `core/metronom-core/src/library/`, pure Rust, no OS dependencies. The layout is
`settings.md`, one `songs.md` table holding every song, and one file per setlist in `setlists/`,
each tagged with a band/project. Rules are in [file-format.md](file-format.md); the screens that
edit these files are in [design.md](design.md).

| Piece | File | Role |
|---|---|---|
| `Library` | `mod.rs` | The one object the app talks to: open from storage, change in memory, save. Songs (add, edit, delete), setlists (create, rename, change band, add/remove/reorder songs, copy, delete), queries (bands, grouping, missing songs, which setlists use a song), settings. A song rename and a song delete are applied to every setlist. Warnings from reading are returned, never fatal |
| `Storage` | `storage.rs` | A four-method trait (read, write, delete, list) so the library never touches the OS directly. `MemStorage` for tests, `FsStorage` for a folder on disk: it writes a temporary file, flushes it and renames it into place, so a crash cannot leave a half-written file. Paths are validated (no `..`, no absolute paths) |
| `Document` / `Header` | `header.rs` | The optional `---` header: read and edit `key: value` lines while every other byte is preserved (unknown keys, comments, spacing, the delimiter lines) |
| `SongsDoc` | `songs.rs` | The song table: finds the first table with `Song` and `BPM` columns, edits rows without touching the text around it, extra columns or untouched cells; keeps rows it does not manage |
| `SetlistDoc` | `setlist.rs` | One setlist file: heading, `band`, and the first list as the song list; edits rewrite only the list |
| `SettingsDoc` | `settings.rs` | `settings.md`: reads and validates the values, writes only the keys that changed |
| `archive` | `archive.rs`, `zip.rs` | Export/import as a zip. A small purpose-written zip reader and writer (stored and deflate, via `miniz_oxide`), strict limits and path checks, conflict choices |

Design points:

- **Edit what you own, preserve the rest.** Every parser keeps the original text and re-renders
  only what the app changed. For untouched files the output is the input, byte for byte; a
  property test checks this for arbitrary text in each file type.
- **Safe by default.** A file that is not valid text is never overwritten; a settings file from a
  newer version is read but not changed; a failed save leaves the files already written intact
  and the rest pending; an import that is refused changes nothing.
- **Deterministic.** Setlists are kept in file-name order whether just created or just loaded, so
  the library looks the same before and after a save (a property test found a case where it did
  not).
- **Identity.** A setlist's file name (its "stem") is its identity and never changes on rename;
  songs are identified by title, compared ignoring case and surrounding spaces.
- **Tested hard.** A model-based test runs random sessions of add / edit / delete / reorder /
  copy / save-and-reopen and checks the rules (unique titles, no missing songs, save/reopen
  equals memory) after every step.

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
| One list of songs, setlists tagged with a band | Matches how a band works (a song is played in many sets), keeps songs from being duplicated per band, and makes the file layout flat and easy to hand-edit |
| Hand-written front-matter and table parser (planned) | Predictable, tiny, and able to preserve unknown keys, extra columns and comments; a full YAML library is unnecessary |
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
