# Design

What the app does and how it behaves, as agreed on 2026-10-04 after reviewing interactive
mockups. The mockups themselves are not kept in the repository; the wireframes below capture
their layout. Items marked *default* are choices made where the discussion left a gap — change
them by editing this document and telling the maintainer.

## Principles

- **A gig tool first.** Few taps, big targets, nothing to navigate while playing.
- **One screen for the metronome.** Everything needed to play fits without scrolling.
- **Small on purpose.** Only what a band needs: tempo, bar length, songs and setlists.
- **Your files.** Everything is stored as plain markdown (see [file-format.md](file-format.md)).

## Two ways to use it

| Mode | What you do |
|---|---|
| **Manual** | Set the tempo and beats per bar yourself and press Start |
| **Setlist** | Choose a setlist, then step through its songs; each song sets the tempo and bar length for you |

## What the metronome controls

| Control | Range | Notes |
|---|---|---|
| Tempo | 30–300 BPM | Type it (tap the number), drag the slider, nudge with −/+, or tap tempo |
| Beats per bar | 1–99 | One number. The first beat of every bar is accented |
| Sound | click, wood, beep, rim | A global setting (in `settings.md`), not per song |
| Volume | 0–100 % | A global setting |

**Deliberately not included:** count-in, subdivisions, time-signature denominators, per-beat
accent editing, and per-song sounds. Reasons: timing depends only on the tempo and on how many
beats make a bar; the denominator in 4/4 versus 4/8 is a label; subdivisions and count-in are
practice features. A 6/8 song can be entered as 6 beats (only the first is accented) or as 2
beats at the dotted-quarter tempo. If an "accent every N beats" option is ever missed, it can be
added later.

## Screens

### Main screen (manual mode)

```
┌──────────────────────────┐
│          Manual          │   mode label
│ ████████████████████████ │   flash bar: lights on every audible beat
│        ●  ○  ○  ○        │   one dot per beat; the first is larger
│                          │
│           120            │   tap the number to type a tempo
│  BPM · tap the number    │
│  [−] ───────●─────── [+] │   slider 30–300
│                          │
│   Beats per bar  [−] 4 [+]
│        [ Tap tempo ]     │
│                          │
│ ┌──────────────────────┐ │
│ │        Start         │ │   large, bottom of the screen
│ └──────────────────────┘ │
└──────────────────────────┘
```

- Tapping the tempo number opens a number entry. Anything outside 30–300 is rejected with a
  message ("Enter a tempo from 30 to 300"), not silently changed.
- The flash and dots are driven by the audio clock, so they line up with what is heard (see
  [architecture.md](architecture.md#visual-synchronisation-built-timing-verified-on-one-device-alignment-by-eye-pending)).
  The first beat of the bar flashes brighter. A bar longer than 16 beats shows "Beat n of m"
  instead of dots.
- Flash style is fixed (bar plus dots). Other styles were considered and not wanted.

### Gig screen (setlist mode)

```
┌──────────────────────────┐
│ ← The Band · Friday Gig  │   back to manual · band · setlist       3 / 7
│ ████████████████████████ │   flash bar
│        ●  ○  ○  ○        │
│      Superstition        │   current song
│          100             │   tempo, large
│       BPM · 4 beats      │
│ Next: Sweet Child o' Mine · 125
│ ┌──────────────────────┐ │
│ │        Start         │ │
│ └──────────────────────┘ │
│ [ ‹ ] [    Next song    ] │   Next is the biggest target; previous is small
└──────────────────────────┘
```

- **Next song** is one tap. **Previous** is a smaller button beside it.
- **The song list** opens by swiping up from the bottom; tapping a song jumps straight to it.
- **The click runs for the whole song.** Start and Stop are separate from changing song.
- **Changing song while playing** *(default)*: the tempo and bar length switch on the next beat,
  and that beat becomes beat 1 of the new song (the accented one). The click keeps running.
- **Changing song while stopped** *(default)*: the new song is loaded; the click starts when you
  press Start.
- On the last song, Next shows "End of setlist" and does nothing.
- The screen stays awake while a setlist is open (setting `keep_screen_on`).

### Songs

- **Songs list:** every song in one list, searchable, with its tempo and beats per bar. Tap a
  song to edit it; "Add song" creates one.
- **Song editor:** title, tempo (number and slider), beats per bar, optional notes. Save, delete,
  and "Play this song now" (loads it into the manual screen).
- **Delete** asks for confirmation and warns if a setlist uses the song.
- Song titles are unique (compared ignoring case).

### Setlists

- **Setlists list:** grouped by band/project. Each row shows the setlist name and song count with
  copy and delete buttons. "New setlist" creates one.
- **Setlist editor:** name, band/project (typed, with the existing bands offered as quick
  choices), the songs in order, and:
  - reorder songs (drag handle or move up/down),
  - remove a song from the setlist,
  - add songs from the song list,
  - **copy** the whole setlist (named "… (copy)"), and delete it.
- A setlist stores only references to songs. There are **no per-setlist tempo overrides**: to
  change a song's tempo, edit the song.

## Behaviour details

| Situation | Behaviour |
|---|---|
| Phone locked or app in the background | Playback continues (foreground service with a notification) |
| Audio device disappears (headphones unplugged) | Playback stops; Start reopens it |
| A song is renamed | Every setlist that uses it is updated |
| A setlist names a song that does not exist (hand-edited file) | Shown as a missing song and kept in the file; never deleted automatically |
| Changing tempo while playing | Takes effect from the next beat; a beat is never shortened or doubled |

## Decision log

| Question | Decision |
|---|---|
| Are time signatures and subdivisions needed? | No. One number, beats per bar (1–99), with an accented first beat |
| Count-in? | Not needed |
| One screen without scrolling? | Yes, for the metronome |
| How is the tempo entered? | Tap the number to type it, or the slider (not a dial) |
| Where do songs live? | One list for all songs, in one file |
| What is a setlist? | A named list of songs, tagged with a band/project |
| Can everything be created, edited, deleted in the app? | Yes: songs and setlists, including reordering and copying setlists |
| Click during the song? | The click runs for the whole song |
| Foot pedal or volume keys for Next? | Not for now |
| Per-setlist tempo overrides? | No |
| More flash styles? | Not needed |
| Where does the sound choice live? | Global setting, not per song |

## Not now

Foot pedal / volume-key control, count-in, subdivisions, time-signature denominators, per-beat
accents, per-setlist overrides, extra flash styles, tempo changes inside a song, tempo ramps,
polyrhythms, cloud sync.
