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

## What exists today (231 tests)

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

### `metronom-ffi` (21 tests)

Settings round-trip through the exported object; tempo and bar length are clamped (30–300,
1–99); switching song changes tempo and bar length without touching the sound; the sound mirror
enum converts both ways; tapping sets the tempo through the exported object; there is nothing to
show while stopped; `stop()` is idempotent; starting without an audio backend returns an error
instead of crashing (host platforms).

The 13 library tests exercise the exported `SongLibrary` against a real temporary folder: an empty
or not-yet-created folder; **changes are saved at once and survive reopening**; errors carry what
the UI needs (the duplicate title, the missing song, the setlist); values are made safe on the way
in; a full setlist session persists; renaming and deleting a song reach the setlists; missing
songs are counted; settings round-trip; export then import moves a library between folders and
keeps both copies of a clashing setlist; a bad archive is a clear error and changes nothing; an
unreadable `songs.md` is reported and never overwritten; warnings carry file and line.

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

### The songs screens on the Pixel 8 Pro (manual, 2026-10-04, debug build)

Driven through the real app with adb, reading the files straight off the phone
(`adb shell run-as io.github.martinonstad.metronom.debug cat files/Metronom/songs.md`):

| Check | Result |
|---|---|
| Empty library | The songs list shows "0 songs" and "Start your song list" with an Add song button |
| Add "Hotel California", 75 BPM (typed title, typed tempo, Save) | List shows `Hotel California  75 · 4`; `songs.md` is a clean table (`# Songs`, `Song` and `BPM` columns, right-aligned numbers) |
| Add "Waltz for Debby", 132 BPM, 3 beats, notes "Intro 8 bars" | **`Beats` and `Notes` columns appear only now**; Hotel California's Beats cell is left empty (default 4) |
| Add "hotel california" (same title, different case) | Rejected: "There's already a song called “hotel california”."; `songs.md` unchanged |
| Edit Hotel California to 80 BPM | List and file updated |
| Restart the app | Songs and tempos persist |
| Delete "Waltz for Debby" while a hand-made `friday-gig.md` uses it | The dialog says "It will also be removed from: • Friday Gig (×1)"; afterwards the song is gone from `songs.md` **and from the setlist, whose list is renumbered and whose note "Soundcheck 18:00." is kept** |
| Play this song now | The main screen shows 80 BPM and 4 beats |
| **Hand-written `songs.md`** (own heading and prose before and after, an extra `Key` column, a row with `fast` as its tempo) | List shows 3 songs; the banner says "1 thing in your files was ignored or fixed", and its details read "songs.md, line 8: "fast" is not a tempo; 120 is used". After editing Wonderwall (87 → 88) the file keeps the prose before and after, the `Key` column, and the `fast` cell exactly as written; only the changed tempo differs (the table is re-aligned) |
| Crashes | None in the log |

Not judged: how the screens feel and look with a real finger (button sizes, spacing, the empty
state), and anything on the iPhone.

### The setlists screens on the Pixel 8 Pro (manual, 2026-10-04, debug build)

Driven through the real app with adb, reading the files off the phone. The test data was four
songs and one hand-written `setlists/hand-written.md` (band `Hand Band`, a lowercase
`superstition`, a song that is not in the library, and the note "Soundcheck at 18:00.").

| Check | Result |
|---|---|
| Open the list and then the editor without changing anything | Grouped under "Hand Band", "3 songs" and "1 missing"; the missing song is shown as "Not in your song list". **The files are byte-identical afterwards** (md5) |
| Move a song up | The file's list is renumbered in the new order; the header, the heading, the lowercase `superstition` and the note are unchanged |
| Add songs from the picker (Sweet Child, Hotel California, Sweet Child again) | The picker stays open and shows "×1 in this setlist", then "×2"; the file gets three new lines; a song can appear twice |
| Remove the missing song | Gone from the screen and the file |
| Edit name to "Friday Gig" and band to "The Band" | Only `band:` and the `# heading` change; **the file name stays `hand-written.md`** |
| Copy from the editor | The copy opens ("Friday Gig (copy)"); `hand-written-copy.md` has the same songs and keeps the note |
| New setlist "Wedding", band "Duo" | Opens its editor; `wedding.md` holds just the band and the heading; the list now has two groups |
| New setlist with a blank name | "Enter a name." and nothing is created; a band chip fills the band field |
| Delete the copy from the list | A confirmation names the setlist and says the songs stay; Cancel leaves the file, Delete removes it |
| Rename a song that is used twice in a setlist | Both lines in the setlist file and the songs table are updated |
| Delete a song used in a setlist | The dialog says "It will also be removed from: • Friday Gig (×1)"; the setlist file and the counts on the list ("4 songs") are updated |
| System Back | Editor → list → main screen |

Not tested: screen rotation and the app being recreated while a setlist is open, a setlist whose
file is changed by hand while the app is open, and long lists (hundreds of songs). Not judged:
how the screens feel with a real finger, and reordering by dragging (only move up/down exists).

### The gig screen on the Pixel 8 Pro (manual, 2026-10-04, debug build)

Driven through the real app with adb. The test setlist "Friday Gig" had five lines: Superstition
(100, 4 beats), a song that is **not** in the library, Waltz for Debby (132, 3 beats), Sweet
Child (125, 4 beats, notes "Intro 8 bars") and Hotel California (75, 4 beats). The click was
followed through the debug beat log (`adb logcat -s MetronomFlash`), which prints each beat change
with the time the displayed beat began.

| Check | Result |
|---|---|
| Tap the setlist row | Gig screen at 1 / 5: "Superstition", 100, "BPM · 4 beats", "Next: Waltz for Debby · 132" (the missing song is stepped over) |
| Start, then **Next song while playing** (twice) | Beat spacing 600.0 ms (100 BPM), then 454.5 ms with beats 0, 1, 2 (132 BPM, 3 beats), then 480.0 ms with beats 0–3 (125 BPM). **The click never stopped; the tempo changed from the beat after the tap, and that beat was beat 1 of the new bar** |
| Last song | "Last song" instead of the next-up line; the button says "End of setlist" and is disabled (tapping it does nothing) |
| Previous (three times from the last song) | 5 → 4 → 3 → 1: the missing song is skipped; Previous is disabled at song 1 |
| Song list: swipe up from the strip | The list opens with the current song highlighted and the missing song marked "Not in your song list"; tapping the missing song does nothing; tapping "Sweet Child" jumps to 4 / 5 and closes the list |
| Notes | "Intro 8 bars" is shown under the tempo |
| Back to the manual screen while playing | Shows 125 and 4 beats, still running; opening the same setlist again resumes at 4 / 5 and **the beat sequence continues with no restart** (spacing stays 480.0 ms, bar position unbroken across 8 beats) |
| Screen kept awake | The app's window holds the screen on while the gig screen is open; `mHoldScreenWindow` is empty again on the manual screen |
| **Next while stopped** | Loads the song (the manual screen then shows 132 and 3 beats); no playback service is started |
| An empty setlist | "There is no song to play in this setlist yet." with an Edit setlist button that opens the editor |
| Manual screen after its beat code was moved to a shared file | Start, flash log and Stop work as before; the first beat of a run is again shown late (104 ms this time; 77 ms before): the known start-up effect |

Not tested: screen rotation while a setlist is open, a setlist that is edited while it is being
played, a long set (hundreds of songs), the Previous and Next buttons with real thumbs on a
stand, and what happens when the phone rings during the set. Not judged: how it looks in bright
light, whether the sizes are right, and whether a swipe up on the strip is easy to do (or too easy
to do by accident) at a gig.

Two things in the debug beat log look odd but are the logger, not the click: it prints only when
the beat *number* changes (a new bar that begins with beat 0 right after a beat 0 prints nothing, so
one gap looks twice as long), and a screen that starts it prints the current beat again.

### The settings and the export / import screens on the Pixel 8 Pro (manual, 2026-10-04, debug build)

Driven through the real app with adb. The export and import went through Android's own file
picker. Test zips were made on the Mac (with the Mac's `zip`, wrapped in a `Metronom/` folder,
with `.DS_Store` and `__MACOSX` junk) and a small script (a zip with a `../evil.md` entry, and a
text file named `.zip`). The debug diagnostics line under the Start button shows the sound and
volume the audio engine actually has.

| Check | Result |
|---|---|
| Open the Settings screen when there is no `settings.md` yet (songs and setlists already exist) | Defaults shown (Click, 80 %, 0 ms, keep screen on); **no `settings.md` is written until something is changed** |
| Choose Wood | `settings.md` is created with `sound: wood` and the header, heading and other keys as written by the app |
| Volume slider, tapped to about 50 % | First saved as `0.49887767`: **fixed**, it is now rounded to whole percent and the file reads `0.5` (also `0.38`) |
| Flash timing slider to +100 ms | `visual_offset_ms: 100`; the Reset button sets it back to 0 |
| Cold start of the app | The engine has WOOD and volume 0.50 straight away (the diagnostics line says so) |
| Choose Beep on the Settings screen, go back | The diagnostics line says BEEP |
| Flash offset +100 ms and −100 ms with the click running | The beat log shows `offsetMs=100` / `-100` and the displayed beat still changes on every beat (with −100 too). The log's "age" is relative to the shifted time, so it **does not prove the flash looks later or earlier**; that needs eyes and ears |
| Keep-awake switch off / on | `keep_screen_on` is written; the screen is held only while the click runs and the switch is on (`mHoldScreenWindow`) |
| **Export** | Android's picker opens with `Metronom-2026-10-04.zip`; after Save a dialog says "Saved…"; the 1 053-byte file is a valid zip for the system `unzip`, with `settings.md`, `songs.md` and both setlists **identical to the files on the phone** |
| **Import** of the Mac-made zip, default options | Report: "Songs: 1 added, 2 already there, yours kept. Setlists: 1 added, 1 added next to yours (hand-written → hand-written-2). Settings: yours kept." plus "Left out of the import: Metronom/.DS_Store, __MACOSX/._x (system file)"; my Superstition (100) stayed; the clashing setlist file is untouched; the new files are as in the zip |
| Same zip, all three set to Replace | "Songs: 3 replaced. Setlists: 2 replaced. Settings: replaced"; Superstition is 110, the setlist and the sound (rim, 0.3) are the zip's; the new settings are in effect at once |
| Cancel in the options dialog | Nothing changes (all files compared) |
| Zip with a `../evil.md` entry | A dialog: "the zip file contains an unsafe path ("../evil.md") and was not imported"; **all files unchanged** (md5 over the library) |
| A text file named `.zip` | A dialog: "this is not a zip file"; nothing changed |
| A refusal shown as a line under the buttons | **Found and fixed:** the message appeared at the very bottom of the scrolling screen, half off-screen, and looked like "nothing happened". Every outcome is now a dialog |
| **A fresh install**: export, wipe the app's data (`pm clear`), import the zip | The app starts with 0 songs and 0 setlists; the import options preselect Replace for the settings only (nothing was ever changed); afterwards `settings.md` and all four setlist files are **byte for byte** the same as before, the songs have the same data, and the sound and volume are applied (WOOD, 0.60) |

**Known difference:** an import rebuilds the song table in the app's own layout. After the
fresh-install round trip `songs.md` had the same songs but `Notes` before `Beats` and right-aligned numbers instead
of the original table; extra columns or text around a hand-made table are not carried over by an
import (see [file-format.md](file-format.md#export-and-import)). The setlists and settings are
copied exactly.

Not tested: a second physical phone, Bluetooth headphones (so the flash timing against a real
Bluetooth delay is unknown), a zip near the size limits, the picker with a cloud provider such as
Drive, and an export while the storage is full. Not judged: how the Settings screen feels, and
whether the explanations are clear to someone who has not seen them.

### Interruptions and accessibility on the Pixel 8 Pro (manual, 2026-10-04, debug build)

Driven through the real app with adb and screenshots.

| Check | Result |
|---|---|
| Start with the notification permission **denied** (the system dialog answered "Don't allow") | Playback starts anyway (foreground service running, the button reads Stop); only the notification is not shown |
| Another app takes the audio while the click runs (the Files app plays a WAV) | The log says "Audio focus lost (-1); stopping"; the service ends and the Start button returns |
| Font scale 2.0 (forced for this app only by a debug-only file; the phone's own setting is untouched) | Manual, gig, setlists and setlist-editor screens show everything; the tempo numeral keeps its size |
| Font scale 2.0 on a 317 dp-wide, about 690 dp-tall screen (density 1.3) | The gig screen still fits completely; the manual screen scrolls (Start is one scroll away), the top buttons wrap onto two lines and the beats stepper onto two lines: **found and fixed**, before the fix "Settings" broke letter by letter and the + button was clipped |
| Light theme (forced for this app only) | Manual and gig screens are legible with good contrast |
| Screen-reader labels (read from the accessibility tree, not by TalkBack) | The −/+ buttons for tempo and beats, the song position ("Song 1 of 2") and the song-list strip all have descriptions; the tempo number is a button |
| Numeral size | Found: Android 14 and later scale big text less than linearly, so the first helper made the tempo number *smaller* at 2× text. **Fixed:** the size is now worked out in pixels, never below the normal size and at most 1.3× |

**Flash safety (an estimate, not a certified assessment).** The flash is a 14 dp-high bar across the
width of the screen and dots of 15–22 dp. On this phone the bar is about 2 × 65 mm, roughly
150 mm²; the area at which web-content guidelines count flashing as a general-flash risk
(0.006 steradian) is about 500 mm² at a 30 cm viewing distance. The flash is therefore below that area
even at 300 BPM (5 flashes a second, more than the usual limit of three). There is no setting to turn
the flash off; if someone needs that, it is a small addition.

**Not tested, needs hardware or your hands:** unplugging wired headphones or a Bluetooth device
disconnecting (the "becoming noisy" stop and the lost-device stop are written but have not been
seen to fire), an incoming call (the refusal to start during a call), the notification and its
Stop action with the permission granted (not looked at in this session), a TalkBack walk-through
(TalkBack was not switched on, since that is a system setting), screen rotation, and a long run
with the new "engine watchdog" (it only ran a few minutes; the aim is that it never stops a healthy click).

### The release build on the Pixel 8 Pro (2026-10-05)

A **signed release build** (a throwaway key made for this test and kept outside the repository)
was installed on the phone, which first meant uninstalling the debug build. Driven with adb.

| Check | Result |
|---|---|
| Size | 2.38 MB (the limit is 10 MB). Unsigned and signed builds are the same size to within 10 KB |
| `apksigner verify` | Verifies (APK Signature Scheme v2) |
| Start the app, open the song list | The library opens through JNA and the UniFFI bindings after R8 shrinking: "0 songs", **no crash** (the first worry with R8) |
| Add a song | Saved; the list shows it |
| Start (notification permission denied) and Stop | The foreground service runs and the audio engine starts; it was still running after 8 s; Stop ends it |
| Export | Saved a valid zip with `songs.md` |
| Afterwards | The release build was removed, the debug build reinstalled and the test library put back |

**Not checked:** how the new launcher icon looks on the phone's own launcher (only a drawing of
the same shapes in a circular mask was looked at), a signed `bundleRelease` (an unsigned one builds, 4.1 MB), and a release build on any
phone other than the Pixel 8 Pro.

**Lint** (`./gradlew lintDebug`, now part of `scripts/test-all.sh`): 0 errors. Findings fixed: four
`context.getString` calls in a Composable (now `LocalResources`), unused strings. Findings
not changed: the UniFFI-generated bindings call `java.lang.ref.Cleaner` (API 33) but check for the class first and
fall back to JNA's own cleaner on older versions, so lint is told to skip that check for the
generated package; the manifest says `allowBackup="true"` with the template's backup rule files
not wired up (see the decision in the milestone notes); newer versions exist of the Android Gradle plugin,
the Compose BOM and some libraries. The app has only been run on Android 17 (API 37); it is
built for 26 and up, and nothing older has been tried.

### Release-prep changes (2026-10-05): automated checks and the phone

Automated, in `scripts/test-all.sh`: 216 core tests and 21 FFI tests; lint with 0 errors; the license
notices up to date (`scripts/generate-licenses.sh --check`); the release APK is 2.4 MB with
**only** the permissions FOREGROUND_SERVICE, FOREGROUND_SERVICE_MEDIA_PLAYBACK and
POST_NOTIFICATIONS (a build that asks for INTERNET fails).

New core tests (6 for the import, plus the setting):

| Test | What it proves |
|---|---|
| A fresh library takes the zip's `songs.md` exactly | Extra column, text before and after the table and spacing are written back byte for byte |
| Editing after such an import | The table is rewritten as usual and the text around it and the extra column are kept |
| A library that has songs | The songs are merged; the zip's text is **not** taken over |
| A `songs.md` with text but no table | Not replaced: the songs are merged and the text stays |
| An archive without songs | A fresh library is left untouched (nothing to write) |
| Two trips (A → B → C) | The hand-made `songs.md` comes back byte for byte each time |
| `mix_with_other_audio` | Read in any letter case, a bad value falls back to false with a warning, only that key is written |

**Run on the Pixel 8 Pro afterwards (2026-10-05, debug build, adb):**

| Check | Result |
|---|---|
| About screen (Settings → About, privacy and licenses) | Shows "Version 0.1.0", the MIT line, the privacy text and 53 notices; tapping one opens its text (the Metronom entry shows the full MIT license); no crash |
| Notice titles | The first version showed titles such as "Permission is hereby granted, free of charge, to a…": **fixed**, they now read "MIT License: adler2, anyhow, …", "Apache License 2.0: …", "BSD License", "zlib License", "0BSD License", "Mozilla Public License 2.0" |
| The mix switch | Switching it on writes `mix_with_other_audio: true` to `settings.md` (and nothing else changes) |
| Another app plays audio while the click runs, **mix on** | The service kept running and the engine kept delivering audio (callbacks continued, 0 underruns). With mix **off** the click stops in the same situation (checked the day before) |
| **Fresh install** (`pm clear`), import a Mac-made zip with a hand-made `songs.md` (prose before and after the table, an extra `Key` column, right-aligned numbers) and a setlist, default options | "Songs: 3 added. Setlists: 1 added. Settings: yours kept."; **`songs.md` and the setlist are byte for byte identical to the zip's files** |
| Then add a song in the app | The new row is added; the prose before and after, the `Key` column and the alignment survive; only the table's padding is recomputed |
| Afterwards | The test library was put back, the mix setting off, my test files removed from Downloads |

Not run: Auto Backup limited to the `Metronom` folder (it needs a backup transport, and a real one
uploads to a Google account; the rules only pass lint), and the notification (permission denied).

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
consistently off, the `visual_offset_ms` setting (Settings screen) corrects it. The latency readings
varied by about ±25 ms between samples, so some flash jitter may be visible; this is unjudged.

## Planned suites

| Suite | Milestone | Content |
|---|---|---|
| Android UI | M2–M3 | Compose tests: type a tempo (valid and invalid), change tempo and beats, save a song, build a setlist, walk through it with Next/Previous, export → import round trip |
| Binding smoke test | M2 | The Kotlin ↔ Rust call path on a device or emulator |
| Size gate | M4 | Fails the build if the release APK exceeds 10 MB |

### Hardware checks on the signed release build (2026-10-05, Pixel 8 Pro)

Release 0.9.0, signed with the real release key, installed next to the debug app. The user did the
hardware checks by hand with the phone connected over USB while the app's log was recorded
(`adb logcat -s Metronom`, which the release build writes too).

| Check | Result |
|---|---|
| The signed release installs and starts; About says "Version 0.9.0" | Yes (adb) |
| **Wired headphones unplugged while the click plays** | The log shows `Audio output is becoming noisy (headphones unplugged?); stopping` (09:34:19): **the service stops itself on the signed release build** |
| Everything else the user tried (Bluetooth, a call, the notification and its Stop button, battery saver, flash timing, tap tempo, how it feels) | The user reports that it "seems to work", without details. **The log shows no other stop event**: no "Audio focus lost", no "engine stopped" line. So either those cases were not run, or they did not stop playback. They are **not recorded as verified** |

Never seen to fire yet: the lost-audio-device stop (a Bluetooth device switching off mid-run), the
refusal to start during a call, and the audio-focus stop on a real call (it was seen with another app's
audio). Still not run by anyone: a TalkBack walk-through, screen rotation, a second phone.

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
   `adb shell run-as io.github.martinonstad.metronom.debug cat files/diagnostics.log`
4. Check: timestamps continuous (a gap of 11 s is normal), every 10-second interval advances
   `delivered` by about 480 000 frames (10 s × 48 kHz), `charging=false` and `interactive=false`
   throughout, the last line is a normal diagnostics line (a `service destroyed` line means the
   service stopped; no closing line means the system killed the process), and the underrun count
   stays at or near 0. `doze=true` shows deep Doze was in effect. (Runs before 2026-10-04 evening
   have no `delivered` field; use the callback counter for those.)
5. Stop playback and check it really stopped (no service record, no further diagnostics lines).
   With the phone locked, `adb shell am force-stop io.github.martinonstad.metronom.debug` works; the service is not
   exported, so `am stopservice` does not.

## iPhone

iPhone checks (simulator only until a physical device is available) are added with Milestone 5.
Timing and background-audio claims for iPhone stay unverified until tested on a real device.
