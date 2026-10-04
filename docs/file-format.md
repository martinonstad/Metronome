# File format

Status: **draft v2 — specified, not yet implemented.** This document is the contract between the
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
| `visual_offset_ms` | −500 – 500 | `0` | Shift the flash relative to the sound (Bluetooth or timing correction) |
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

- The first table in the file is the song list. Text before and after it (headings, paragraphs)
  is preserved.
- Columns are found by their header names, ignoring case and order. **Extra columns are
  preserved** and shown nowhere.
- A `|` inside a title or note is written `\|`.
- **Titles are unique**, compared ignoring case and surrounding spaces. If a title appears
  twice, the first row wins and a warning is reported; the other row is kept in the file.
- A row without a title is ignored (with a warning) and kept in the file. A `BPM` outside 30–300
  is clamped, and a missing or invalid `Beats` becomes 4, each with a warning.
- The order of the rows is the order in the app's song list. The app writes the table with
  aligned columns, but any valid markdown table is accepted.

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

- A song title is matched against `songs.md` **ignoring case and surrounding spaces**. A title
  that matches nothing is shown as a **missing song** and kept in the file; it is never deleted
  automatically.
- The same song may appear more than once in a setlist.
- Renaming a song in the app updates every setlist that uses it. A rename done by hand-editing
  `songs.md` is not tracked: the setlists then show the old title as a missing song.
- A setlist stores only titles. It has no tempo of its own; the tempo and beats come from the
  song.
- The app writes the list renumbered (`1.`, `2.`, …); hand-written bullets (`-`) are accepted.

### File names (slugs)

Generated names are lowercase ASCII: accents are folded (`é` → `e`, `ø` → `o`), every run of
other characters becomes `-`, and a collision gets a numeric suffix (`friday-gig-2`). Copying a
setlist creates `<name>-copy.md` titled "<name> (copy)". Files you create by hand may use any
valid name ending in `.md`.

## Round-trip and write rules

- Saving a file rewrites only what the app owns; every other line of a header (unknown keys,
  comments), every extra table column, and all surrounding text are written back exactly as read.
- Writes are atomic: the app writes a temporary file and renames it over the target, so a crash
  cannot leave a half-written file.
- If a file cannot be parsed at all (for example, binary data), it is left untouched and listed
  as unreadable.

## Export and import

- **Export** produces a `.zip` of the layout above.
- **Import** accepts the same layout. `settings.md` and each setlist file can be skipped,
  overwritten, or kept as a second copy. For `songs.md` the default is to **add songs whose titles
  are not yet in the app**; songs with the same title can be skipped or overwritten.
- Import is defensive: it rejects entries with absolute paths or `..` segments (zip-slip), only
  accepts `.md` files in the layout above, and refuses archives over the planned limits of 1,000
  entries, 1 MB per file and 20 MB in total. All of this will be covered by tests.

## Versioning

`format` in `settings.md` is the format version (absent means 2). A newer app reading an older
library migrates on save; an older app reading a newer `format` opens files read-only and warns.
