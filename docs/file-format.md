# File format

Status: **draft v1 — specified, not yet implemented.** This document is the contract between the
app and your files. Both the Android and iPhone apps use the same Rust parser, so a library
exported from one opens unchanged on the other.

## Principles

- **Plain text you can edit.** UTF-8 markdown with a small header of `key: value` lines.
- **Never lose hand edits.** Unknown keys, comments and the markdown body are preserved when the
  app saves a file.
- **Forgiving on read, careful on write.** A bad value falls back to a default and produces a
  warning; it never crashes the app and never silently overwrites your file.

## Layout

```
Metronom/
  settings.md
  library/
    <project-slug>/
      project.md
      songs/<song-slug>.md
      setlists/<setlist-slug>.md
```

- A **project** is a band or group of songs. Songs not assigned to a project live in an
  automatic project called `General` (`library/general/`).
- A **preset** is just a song.
- The same layout is used inside exported zip files.

## File structure

```
---
key: value
other_key: value   # a comment
---
# Title

Free markdown text.
```

- The header is optional. It must start on the first line with `---` and end with a line
  containing only `---`.
- Everything after the header is the **body** and is kept exactly as written.
- Files are written with `\n` line endings; `\r\n` is accepted on read.

### Header syntax

| Element | Rule |
|---|---|
| Entry | `key: value`, one per line. Keys are lowercase `snake_case` |
| Comment | `#` at the start of a line, or after whitespace following a value |
| Integer / decimal | `120`, `97.5` |
| Boolean | `true` or `false` |
| Text | Bare (`wood`) or double-quoted (`"Intro: slow"`) when it contains `#` or leading/trailing spaces |
| List | Space-separated tokens (`X o o o`) |
| Duplicate key | The last one wins; a warning is reported |
| Unknown key | Preserved, ignored |

### Titles

The title of a song, setlist or project is the first level-1 heading (`# Title`) in the body. If
there is none, the file name (without `.md`) is used. The **file name is the identifier**; the
title is what is shown.

### File names (slugs)

Generated names are lowercase ASCII: accents are folded (`é` → `e`, `ø` → `o`), every run of
other characters becomes `-`, and a collision gets a numeric suffix (`my-song-2`). Files you
create by hand may use any valid name ending in `.md`.

## `settings.md`

```markdown
---
format: 1
sound: wood
volume: 0.8
flash: edge            # full | edge | dots | off
flash_intensity: 0.6
haptics: false
keep_screen_on: true
visual_offset_ms: 0
last_project: the-band
---
# Settings
Edit the values above. Unknown keys are kept.
```

| Key | Type / range | Default | Meaning |
|---|---|---|---|
| `format` | integer | `1` | File-format version, for future migrations |
| `sound` | `click` `wood` `beep` `rim` | `click` | Default click sound |
| `volume` | 0.0 – 1.0 | `0.8` | Output level |
| `flash` | `full` `edge` `dots` `off` | `edge` | Visual beat style |
| `flash_intensity` | 0.0 – 1.0 | `0.6` | Strength of the flash |
| `haptics` | boolean | `false` | Vibrate on the downbeat |
| `keep_screen_on` | boolean | `true` | Keep the display awake while playing |
| `visual_offset_ms` | −500 – 500 | `0` | Shift the blink relative to the sound (Bluetooth compensation) |
| `last_project` | project slug | none | Project shown on launch |

## Songs — `library/<project>/songs/<song>.md`

A song is a saved metronome configuration plus notes.

```markdown
---
bpm: 75
time: 4/4
subdivision: 1         # clicks per pulse: 1, 2 (eighths), 3 (triplets), 4 (sixteenths)
accents: X o o o       # X strong · o normal · - muted, one token per pulse
count_in: 1            # bars of count-in before the song
sound: wood            # optional, overrides settings.md
---
# Hotel California

Key: Bm · Capo 7 · Intro 8 bars
```

| Key | Type / range | Default | Meaning |
|---|---|---|---|
| `bpm` | 20 – 400, decimals allowed | `120` | Pulses per minute (see below) |
| `time` | `N/D`: N 1–16, D one of 2, 4, 8, 16 | `4/4` | Time signature |
| `subdivision` | 1 – 4 | `1` | Clicks per pulse |
| `accents` | `X`, `o`, `-` tokens | derived from `time` | Accent of each pulse in the bar |
| `count_in` | 0 – 4 | `0` | Count-in bars |
| `sound` | as in settings | from settings | Click sound |

- **Pulses.** `bpm` counts pulses of the denominator note: `6/8` at `120` is 120 eighth-note
  pulses per minute.
- **Accents.** The number of tokens must equal the time signature's numerator. On a mismatch the
  app falls back to the default grouping for that signature and reports a warning. Defaults:
  strong first pulse, normal elsewhere; `6/8` also accents pulse 4, `9/8` pulses 4 and 7,
  `12/8` pulses 4, 7 and 10.
- A missing or invalid value uses its default.

## Setlists — `library/<project>/setlists/<setlist>.md`

An ordered list of songs. Each entry is a numbered or bulleted item containing a `[[link]]`.

```markdown
# Friday Gig
1. [[Hotel California]]
2. [[Wonderwall]]
3. [[other-band/Another Song]]

Soundcheck at 18:00.
```

- `[[Name]]` matches a song in the **same project** by title or file name, ignoring case.
- `[[project/Name]]` matches a song in another project (project slug, then title or file name).
- A link that matches nothing is kept in the file and shown as a missing song; it is never
  deleted automatically.
- Lines that are not list items are notes and are preserved.

## Projects — `library/<project>/project.md`

```markdown
# The Band

Rehearsal Tuesdays. Drummer: Anna.
```

A title and notes only; no header keys are defined for projects or setlists yet, so the header is
omitted.

## Round-trip and write rules

- Saving a file rewrites only the keys the app owns; every other line of the header (unknown keys,
  comments) and the whole body are written back exactly as read.
- Writes are atomic: the app writes a temporary file and renames it over the target, so a crash
  cannot leave a half-written file.
- If a file cannot be parsed at all (for example, binary data), it is left untouched and listed
  as unreadable.

## Export and import

- **Export** produces a `.zip` of the layout above (a whole library or one project).
- **Import** accepts the same layout and asks, per conflict, to skip, overwrite or keep both.
- Import is defensive: it rejects entries with absolute paths or `..` segments (zip-slip), only
  accepts `.md` files, and refuses archives over the planned limits of 5,000 entries, 1 MB per
  file and 50 MB in total. All of this will be covered by tests.

## Versioning

`format` in `settings.md` is the format version (absent means 1). A newer app reading an older
library migrates on save; an older app reading a newer `format` opens files read-only and warns.
