# File format

Status: **v2 — implemented in `metronom-core` (the `library` module), with tests.** This document is the contract between the
app and your files. Both the Android and iPhone apps use the same Rust parser, so a library
exported from one opens unchanged on the other. It replaces the earlier draft that had one folder
per project; nothing had been implemented, so there is nothing to migrate.

## Principles

- **Plain text you can edit.** UTF-8 markdown; songs in one table; setlists as numbered lists.
- **Never lose hand edits.** Unknown keys, extra table columns, comments and surrounding text are
  preserved when the app saves a file.
- **Forgiving on read, careful on write.** A bad value falls back to a default and produces a
  warning; it never crashes the app and never silently overwrites your file.

## Layout

```
Metronom/
  settings.md          global settings
  songs.md             every song, in one table
  setlists/
    friday-gig.md      one file per setlist, tagged with a band/project
    wedding-set.md
```

The same layout is used inside exported zip files.

## `settings.md`

```markdown
---
format: 2
sound: wood
volume: 0.8
keep_screen_on: true
visual_offset_ms: 0
last_setlist: friday-gig
---
# Settings
Edit the values above. Unknown keys are kept.
```

| Key | Type / range | Default | Meaning |
|---|---|---|---|
| `format` | integer | `2` | File-format version, for future migrations |
| `sound` | `click` `wood` `beep` `rim` | `click` | The click sound |
| `volume` | 0.0 – 1.0 | `0.8` | Output level |
| `keep_screen_on` | boolean | `true` | Keep the display awake while a setlist is open or playing |
| `visual_offset_ms` | −500 – 500 | `0` | Shift the flash relative to the sound (Bluetooth or timing correction). **Positive shows the flash later**, which is what to use when the flash comes before the sound; negative shows it earlier |
| `last_setlist` | setlist file name (no `.md`) | none | Setlist reopened on launch |

### Header syntax (also used by setlists)

| Element | Rule |
|---|---|
| Header | Optional. Starts on the first line with `---` and ends at the next line containing only `---` |
| Entry | `key: value`, one per line; keys are lowercase `snake_case` |
| Comment | `#` at the start of a line, or after whitespace following a value |
| Number | `120`, `97.5` |
| Boolean | `true` or `false` |
| Text | Bare (`The Band`) or double-quoted (`"Intro: slow"`) when it contains `#` or leading/trailing spaces |
| Duplicate key | The last one wins; a warning is reported |
| Unknown key | Preserved, ignored |

Everything after the header is the **body** and is kept exactly as written. Files are written
with `\n` line endings; `\r\n` is accepted on read.

## `songs.md`

Every song, in one markdown table:

```markdown
# Songs

| Song             | BPM | Beats | Notes      |
|------------------|-----|-------|------------|
| Hotel California |  75 |     4 | Bm, capo 7 |
| Superstition     | 100 |     4 |            |
| Waltz for Debby  | 132 |     3 |            |
```

| Column | Required | Meaning |
|---|---|---|
| `Song` | yes | The title; this is how setlists refer to the song |
| `BPM` | yes | Tempo, 30–300. Decimals are allowed (`97.5`); the app shows it rounded |
| `Beats` | no, default `4` | Beats per bar, 1–99. The first beat of each bar is accented |
| `Notes` | no | Free text (key, capo, intro length) |

Rules:

- The song list is **the first table that has both a `Song` and a `BPM` column**. Text before and
  after it (headings, paragraphs, other tables) is preserved.
- Columns are found by their header names, ignoring case and order. **Extra columns are
  preserved.** A `Beats` or `Notes` column is added only when a song needs one and the table has
  none.
- A `|` inside a title or note is written `\|`.
- **Titles are unique**, compared ignoring case and surrounding spaces. If a title appears
  twice, the first row wins and a warning is reported; the other row is kept in the file.
- A row without a title is ignored (with a warning) and kept in the file.
- Values: a `BPM` outside 30–300 is clamped (warning); a `BPM` that is not a number becomes 120
  (warning). An empty or missing `Beats` is 4; a `Beats` outside 1–99 is clamped and one that is
  not a number becomes 4 (warning).
- The order of the rows is the order in the app's song list.
- **Saving rewrites only the table.** Rows and cells you did not change keep their original
  spelling (`97.50` stays `97.50`); the table is re-aligned only when something in it changed.
  A new song is appended; a deleted one's row is removed. If the file has no song table, one is
  created after any existing text (or after a `# Songs` heading in a new file).

## Setlists — `setlists/<name>.md`

One file per setlist:

```markdown
---
band: The Band
---
# Friday Gig

1. Hotel California
2. Superstition
3. Waltz for Debby

Soundcheck at 18:00.
```

| Part | Meaning |
|---|---|
| `band` (header) | The band or project the setlist belongs to. Optional; a setlist without one is grouped under "No band" |
| First `# heading` | The setlist's name. If there is none, the file name (without `.md`) is used |
| Numbered or bulleted list items | The songs, in order. Each item's text is a song title from `songs.md` |
| Other lines | Notes; preserved |

Rules:

- **The first list in the file is the song list.** A later list is notes. Items are numbered
  (`1.`, `1)`) or bulleted (`-`, `*`, `+`); blank lines between items are fine; an item indented
  four or more spaces is a continuation of the one above, not a song. Lists inside code fences
  are ignored.
- A song title is matched against `songs.md` **ignoring case and surrounding spaces**. A title
  that matches nothing is shown as a **missing song** and kept in the file; it is never deleted
  automatically.
- The same song may appear more than once in a setlist.
- Renaming a song in the app updates every setlist that uses it (matching ignoring case).
  **Deleting a song in the app removes it from every setlist** (the app first tells you which).
  A rename done by hand-editing `songs.md` is not tracked: the setlists then show the old title
  as a missing song.
- A setlist stores only titles. It has no tempo of its own; the tempo and beats come from the
  song.
- **Saving rewrites only the song list** (renumbered `1.`, `2.`, … with one blank line around it)
  **and the heading or `band` line if you changed them.** The header's other keys and comments,
  and all notes, stay exactly as written. A setlist you did not change is never rewritten.
- **A setlist's file name never changes when it is renamed**; the file name is its identity (it
  is what `last_setlist` refers to). Only the `# heading` changes.
- The app lists setlists in file-name order internally; screens group them by band (bands
  alphabetical ignoring case, "The Band" and "the band" being one band) and sort them by name.

### File names (slugs)

Generated names are lowercase ASCII: accents are folded (`é` → `e`, `ø` → `o`), every run of
other characters becomes `-`, and a collision gets a numeric suffix (`friday-gig-2`). A name with
nothing usable becomes `untitled`. A new name also avoids files that cannot be read and files
that are about to be deleted. Copying a setlist creates `<name>-copy.md` titled "<name> (copy)". Files you create by hand may use any
valid name ending in `.md`.

## Round-trip and write rules

- **A file you did not change is never written, and one the app reads and does not change is
  returned byte for byte.** This is tested for arbitrary text.
- Saving rewrites only what the app owns; every other line of a header (unknown keys,
  comments), every extra table column, and all surrounding text are written back exactly as read.
- Writes are atomic: the app writes a temporary file, flushes it, and renames it over the target,
  so a crash cannot leave a half-written file.
- If saving fails part-way, the files already written stay saved, the rest stay pending, and
  saving again retries them.
- `\r\n` line endings and a leading byte-order mark are accepted on read; files are written with
  `\n` and without a byte-order mark.
- A file that is not valid UTF-8 text is **left untouched and never overwritten**: `songs.md` or
  `settings.md` then cannot be changed by the app until fixed, and an unreadable setlist is not
  shown. A `settings.md` with a newer `format` is read but not changed.
- Files in `setlists/` that do not end in `.md`, or whose name starts with `.`, are ignored.

## Export and import

- **Export** produces a `.zip` of the layout above, taken from the saved files (save first if
  there are unsaved changes). It refuses to make an archive it could not import again.
- **Import** merges an archive into the library in memory; nothing is written until the library
  is saved. It is **all-or-nothing**: if the archive is refused, nothing changes.
  - `settings.md`: skip (default) or overwrite. If your settings file is from a newer version, the
    rest of the import still goes ahead and the report says the settings were not imported.
  - `songs.md`: songs whose titles are not yet in the library are always added; for a title that
    already exists, skip it (default) or overwrite it. Only the song values are imported (extra
    columns in the archive's table are not).
  - Setlists: if the file name is free the setlist is added; if it exists, skip it, overwrite it,
    or keep both (the default; the import gets a new file name such as `friday-gig-2`). A name
    that belongs to an unreadable file is always kept both. An imported setlist file is written
    exactly as it was read.
- A folder zipped on a computer (everything inside one top-level folder such as `Metronom/`) is
  unwrapped automatically. System files (`__MACOSX/`, `.DS_Store`, `._*`, `Thumbs.db`) and any
  other entry outside the layout are ignored and listed in the import report with the reason;
  files that are not valid text are ignored the same way.
- Import is defensive:
  - **Any path that could leave the library folder** (`../x`, an absolute path, a drive letter,
    a backslash) **refuses the whole archive**, even if that entry would have been ignored.
  - Limits: **1,000 files, 1 MB per file, 20 MB in all** (uncompressed). They are checked before
    anything large is allocated, and a compressed file must decompress to exactly the size the
    archive declares, so a zip bomb is refused.
  - Encrypted, zip64, multi-disk and non-deflate archives are not supported; every file's CRC is
    checked; duplicate names are refused.
  - The reader is a small purpose-written one (stored and deflate only), tested against the real
    `zip` and `unzip` tools in both directions.

## Versioning

`format` in `settings.md` is the format version (absent means 2). A newer app reading an older
library migrates on save; an older app reading a newer `format` opens files read-only and warns.
