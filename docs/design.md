# Design

What the app does and how it behaves, as agreed on 2026-10-04 after reviewing interactive
mockups. The mockups themselves are not kept in the repository; the wireframes below capture
their layout. Items marked *default* are choices made where the discussion left a gap — change
them by editing this document and telling the maintainer.

Revised on 2026-11-16 after using the app: the main screen can save what is set as a song, the
song page is built like the main screen and plays the click, and the setlist page is the whole
list of songs (see the [decision log](#decision-log)).

## Principles

- **A gig tool first.** Few taps, big targets, nothing to navigate while playing.
- **One screen for the metronome.** Everything needed to play fits without scrolling.
- **Small on purpose.** Only what a band needs: tempo, bar length, songs and setlists.
- **Your files.** Everything is stored as plain markdown (see [file-format.md](file-format.md)).

## Two ways to use it

| Mode | What you do |
|---|---|
| **Manual** | Set the tempo and beats per bar yourself and press Start |
| **Setlist** | Open a list (all songs, or a setlist), then tap a song; the song sets the tempo and bar length and the click starts |

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

Revised on 2026-11-18: the app is **one metronome view, which is also where a song is edited, and
lists**, under a tab bar that is on every page (see the [decision log](#decision-log)).

### The tab bar

```
┌──────────────────────────┐
│ [♩ Metronome] [☰ Lists] [⚙ Settings]
```

- Three tabs, each an icon above a short label, at the top of every page. There are no Back
  buttons on the pages.
- **Every tab remembers its page**: the Lists tab stays on Friday Gig while you look at the
  Metronome tab. Tapping the tab you are on goes to its first page.
- **System Back** leaves a list's Edit mode, then closes the list, then goes to the Metronome tab,
  and from there leaves the app. About goes back to Settings. If ✎ in a list opened the Metronome
  tab, Back returns to that list.

### Metronome tab: the metronome and the current song

```
┌──────────────────────────┐
│ Superstition ✎         ✕ │   the current song, or "No song"; tap or ✎ for its details
│ Intro 8 bars             │   its notes
│ ████████████████████████ │   flash bar: lights on every audible beat
│  [−]   ●  ○  ○  ○   [+]  │   beats per bar: − and + around the dots
│         4 beats          │
│           100            │   tap the number to type a tempo
│  BPM · tap the number    │
│  [−] ───────●─────── [+] │   slider 30–300
│   [Tap tempo]  [Save]    │   Save: the tempo and beats into the song
│ ┌──────────────────────┐ │
│ │        Start         │ │   large, bottom of the screen
│ └──────────────────────┘ │
└──────────────────────────┘
```

- **The current song.** The metronome always has a current song or none. Tapping a song in a
  list, or its ✎, makes it the current song (its tempo and bar length are loaded; a running click
  switches on the next beat). **✕** puts the song away and keeps the tempo: a plain metronome.
- **Save** stores the tempo and beats in the current song. It reads "Save" when they differ from
  the saved song, "Saved" (greyed) when they match, and "Save as song" when there is no song, which
  asks for a title. Changes that are not saved are dropped when another song becomes current.
- **Song details** (tap the title or ✎): title, notes, **Add to setlist** (tick any number of
  setlists; the song is added at the end of each, and an existing song shows where it is used) and
  **Delete song**. Saving keeps the song's saved tempo; a new song takes the tempo on the page. A
  title that is empty or taken is refused and nothing is written. A tempo with decimals in a
  hand-written file is kept as long as you do not change it.
- **Delete** asks for confirmation and says which setlists use the song; **deleting removes it
  from all of them** (so no setlist is left with a missing song).
- Tapping the tempo number opens a number entry. Anything outside 30–300 is rejected with a
  message ("Enter a tempo from 30 to 300"), not silently changed. Tap tempo sets a whole-number
  tempo, so the number shown is the tempo that plays and is saved.
- The flash and dots are driven by the audio clock, so they line up with what is heard (see
  [architecture.md](architecture.md#visual-synchronisation-built-timing-verified-on-one-device-alignment-by-eye-pending)).
  The first beat of the bar flashes brighter. The dots shrink to stay on one line when the bar is
  long; a bar longer than 16 beats shows "Beat n of m" instead of dots.
- Flash style is fixed (bar plus dots). Other styles were considered and not wanted.
- Song titles are unique (compared ignoring case).

### Lists tab

```
┌──────────────────────────┐
│  All songs      21 songs │   every song, always first
│ ──────────────────────── │
│  The Band                │   setlists grouped by band/project
│    Friday Gig    7 songs │
│  Duo                     │
│    Wedding      13 songs │
│ [      New setlist     ] │
└──────────────────────────┘
```

Tapping a row opens the list. "New setlist" asks for a name and band and opens the new setlist in
Edit mode.

**A list page** (All songs, or a setlist):

```
┌──────────────────────────┐
│ The Band · Friday Gig  Edit
│ ████████████████████████ │   flash bar and beat dots stay above the list
│        ●  ○  ○  ○        │
│  1  Superstition   100 ✎ │   tap a song: the click starts; ✎: edit it
│                  4 beats │
│  2  Nowhere Man          │   not in the song list: greyed, cannot be tapped
│ ■3  Waltz for Debby 132 ✎│   the song that is playing is highlighted;
│     Intro 8 bars 3 beats │   tap it again: the click stops
│  4  Sweet Child    125 ✎ │   tap another: the click switches to it
└──────────────────────────┘
```

- **The whole list is on the page**, each song with its tempo, beats per bar and **notes** (key,
  capo, intro length). There is no Next, Previous or Start button: the list is the control.
- **Tap a song** and the click starts with its tempo and bar length. **Tap the song that is
  playing** and the click stops. **Tap another song while the click runs** and it switches on the
  next beat, which becomes beat 1 of the new song (the accented one); the click keeps running.
- **✎** makes the song current and opens the Metronome tab to edit it; Back returns to the list.
- A row is lit while the click runs, it is the row tapped last in this list, and it is still the
  current song. Adjusting the tempo on the Metronome tab keeps it lit; another song becoming
  current, or putting the song away, un-lights it.
- The song tapped last stays marked when the click is stopped, and when you come back to the same
  list. Opening a list changes nothing in the metronome. The app does not reopen a list on
  launch: `last_setlist` in `settings.md` is reserved for that.
- A song that is not in the library (a missing song) is greyed out and cannot be played or edited.
- On a narrow phone with very large text, a row puts the tempo under the title, so titles break
  between words.
- **All songs** lists every song in the order of `songs.md`, with a search field and **New song**
  (which puts the current song away and opens the Metronome tab, where "Save as song" names it).
  It has no Edit mode: a song is renamed or deleted in its details.
- **Edit mode** (a setlist's Edit / Done): the rows get move up, move down and remove; the page
  gets **Name and band** (typed, with the existing bands offered as quick choices), **Add songs**
  (a searchable picker that stays open to add several), **Copy** (named "… (copy)", which opens in
  Edit mode) and **Delete**. Every change is saved at once. The flash is hidden and rows do not
  play; a running click keeps running.
- A setlist stores only references to songs. There are **no per-setlist tempo overrides**: to
  change a song's tempo, edit the song.
- The screen stays awake while a list is open (setting `keep_screen_on`).

### Settings tab

Sound, volume, flash timing, keep-awake, play together with other audio, export and import of
your files, and About (version, license, privacy, the libraries' notices).

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
| Notes on the gig screen? | Yes, under the tempo (decided 2026-10-05); under each song's title since the setlist page became a list (2026-11-16) |
| A missing song in a setlist while playing? | Next and Previous step over it (decided 2026-10-05); since 2026-11-16 there is no Next, and a missing song cannot be tapped |
| Resume position | Kept for the setlist played last; no reopening on launch yet (decided 2026-10-05) |
| Another app's audio? | Default: the click takes the audio focus and stops when another app or a call takes it; a setting lets it play together with other audio (decided 2026-10-05) |
| Auto Backup | Stays on for the `Metronom` folder only (decided 2026-10-05) |
| Flash on/off switch | Not added: the flash is a thin bar and small dots, below the size at which flashing is a risk (decided 2026-10-05; revisit if anyone needs it) |
| Saving what is set on the main screen? | A "Save as song" button opens the new-song page with the tempo and beats already there; the song can be added to setlists in the same save (decided 2026-11-16) |
| What does the song page look like? | Like the main screen, and live: flash, the number to type, −/+, slider, beats, tap tempo and Start/Stop, because an exact tempo is hard to hit with a slider alone. "Play this song now" is gone: the page plays (decided 2026-11-16) |
| What does the setlist page look like? | The whole list. Tap a song to start the click, tap it again to stop, tap another to switch while the click keeps running. The flash stays at the top, notes are under each song, Edit is in the header. Replaces the one-song view with Next / Previous and the swipe-up list (decided 2026-11-16) |
| How do you get around? | A tab bar on every page: Metronome, Lists, Settings, each an icon with a short label; no Back buttons; each tab remembers its page (decided 2026-11-18) |
| A separate Songs section? | No. "All songs" is a built-in list at the top of the Lists tab and works like a setlist (decided 2026-11-18) |
| Where is a song edited? | On the Metronome tab, which shows the current song: Save next to Tap tempo stores the tempo and beats, the title opens title, notes, setlists and Delete. ✎ on any list row opens it there; tapping the row still plays it (decided 2026-11-18) |
| Where is a setlist edited? | In an Edit mode on its list page; the separate editor page is gone (decided 2026-11-18) |
| Where is the bar length set? | − and + on either side of the beat dots, above the tempo (decided 2026-11-18) |

## Not now

Foot pedal / volume-key control, count-in, subdivisions, time-signature denominators, per-beat
accents, per-setlist overrides, extra flash styles, tempo changes inside a song, tempo ramps,
polyrhythms, cloud sync.
