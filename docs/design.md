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
| Play together with other audio | on / off (off) | A global setting. Off: the click takes the audio focus (it pauses a music app, and stops when a call or another app takes the audio). On: it never takes the focus, so other apps keep playing and are not interrupted by the click |
| Flash timing | −500…+500 ms | A global setting; positive shows the flash later (for Bluetooth headphones) |
| Keep the screen on | on / off (on) | While the click runs or a setlist is open |

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

- **Next song** is one tap. **Previous** is a smaller button beside it. Both step over a song
  that is not in the library (a missing song).
- The song's **notes** (key, capo, intro length) are shown under the tempo.
- **The song list** opens by swiping up from the bottom; tapping a song jumps straight to it.
- **The click runs for the whole song.** Start and Stop are separate from changing song.
- **Changing song while playing** *(default)*: the tempo and bar length switch on the next beat,
  and that beat becomes beat 1 of the new song (the accented one). The click keeps running.
- **Changing song while stopped** *(default)*: the new song is loaded; the click starts when you
  press Start.
- On the last song, Next shows "End of setlist" and does nothing.
- The screen stays awake while a setlist is open (setting `keep_screen_on`).
- **Back** returns to the manual screen; the click keeps running. Opening the same setlist again
  resumes at the same song (the position is kept for the setlist played last, until the app is
  closed by the system). The app does not reopen a setlist on launch: `last_setlist` in
  `settings.md` is reserved for that.

### Songs

- **Songs list:** every song in one list, searchable, with its tempo and beats per bar. Tap a
  song to edit it; "Add song" creates one.
- **Song editor:** title, tempo (number and slider), beats per bar, optional notes. Save, delete,
  and "Play this song now" (loads it into the manual screen).
- **Delete** asks for confirmation and says which setlists use the song; **deleting removes it
  from all of them** (so no setlist is left with a missing song).
- Song titles are unique (compared ignoring case).

### Setlists

- **Setlists list:** grouped by band/project. Each row shows the setlist name and song count.
  **Tapping the row plays it** (opens the gig screen); the row has **Edit, Copy and Delete**
  buttons. "New setlist" creates one and opens its editor.
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
| Audio device disappears (headphones unplugged, Bluetooth gone) | Playback stops, and so does the service with its notification; Start reopens it |
| Another app or a call takes the audio | Playback stops (not when "play together with other audio" is on); it never restarts by itself. If a call is active, Start does not start and says so |
| The notification permission is denied | Playback works; only the notification is not shown |
| An import into a library with no songs | The zip's `songs.md` is taken over exactly as it is (extra columns, text around the table); into a library that has songs, the songs are merged |
| A song is renamed | Every setlist that uses it is updated |
| A song is deleted | It is removed from every setlist that uses it, after you confirm |
| A setlist is renamed | Only its heading changes; its file name stays the same |
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
| Tapping a setlist row? | Plays it; Edit, Copy and Delete are buttons on the row (decided 2026-10-05) |
| Notes on the gig screen? | Yes, under the tempo (decided 2026-10-05) |
| A missing song in a setlist while playing? | Next and Previous step over it (decided 2026-10-05) |
| Resume position | Kept for the setlist played last; no reopening on launch yet (decided 2026-10-05) |
| Another app's audio? | Default: the click takes the audio focus and stops when another app or a call takes it; a setting lets it play together with other audio (decided 2026-10-05) |
| Auto Backup | Stays on for the `Metronom` folder only (decided 2026-10-05) |
| Flash on/off switch | Not added: the flash is a thin bar and small dots, below the size at which flashing is a risk (decided 2026-10-05; revisit if anyone needs it) |

## Not now

Foot pedal / volume-key control, count-in, subdivisions, time-signature denominators, per-beat
accents, per-setlist overrides, extra flash styles, tempo changes inside a song, tempo ramps,
polyrhythms, cloud sync.
