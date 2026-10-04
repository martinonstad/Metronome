use super::settings::AppSettings;
use super::songs::Song;
use super::storage::{MemStorage, Storage};
use super::*;
use crate::engine::Sound;
use proptest::prelude::*;

const SONGS: &str = "# My songs\n\nA note for myself.\n\n| Song             | BPM | Key | Beats | Notes      |\n|------------------|----:|-----|------:|------------|\n| Hotel California |  75 | Bm  |     4 | Bm, capo 7 |\n| Superstition     | 100 | Eb  |     4 |            |\n| Waltz for Debby  | 132 | C   |     3 |            |\n\nMore text after the table.\n";
const FRIDAY: &str = "---\nband: The Band\nowner: Martin # keep\n---\n# Friday Gig\n\nIntro for the sound engineer.\n\n1. Hotel California\n2. Superstition\n3. Waltz for Debby\n\nSoundcheck at 18:00.\n";
const SETTINGS: &str = "---\nformat: 2\nsound: wood\nvolume: 0.6\ntheme: night   # not ours\n---\n# Settings\nMy own notes.\n";

fn hand_written() -> MemStorage {
    MemStorage::new()
        .with("settings.md", SETTINGS)
        .with("songs.md", SONGS)
        .with("setlists/friday-gig.md", FRIDAY)
}

fn open(storage: &MemStorage) -> Library {
    Library::open(storage).unwrap().library
}

fn titles(library: &Library) -> Vec<String> {
    library.songs().iter().map(|s| s.title.clone()).collect()
}

#[test]
fn an_empty_folder_is_an_empty_library_with_no_warnings() {
    let opened = Library::open(&MemStorage::new()).unwrap();
    assert!(opened.warnings.is_empty());
    assert!(opened.library.songs().is_empty());
    assert!(opened.library.setlists().is_empty());
    assert_eq!(opened.library.settings(), &AppSettings::default());
    assert!(!opened.library.is_dirty());
}

#[test]
fn opening_hand_written_files() {
    let opened = Library::open(&hand_written()).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    let library = opened.library;
    assert_eq!(
        titles(&library),
        ["Hotel California", "Superstition", "Waltz for Debby"]
    );
    assert_eq!(library.song("superstition").unwrap().bpm, 100.0);
    assert_eq!(library.setlists().len(), 1);
    let friday = library.setlist("friday-gig").unwrap();
    assert_eq!(
        (friday.name().as_str(), friday.band()),
        ("Friday Gig", Some("The Band"))
    );
    assert_eq!(library.settings().sound, Sound::Wood);
    assert_eq!(library.settings().volume, 0.6);
}

#[test]
fn nothing_is_written_when_nothing_changed() {
    let storage = hand_written();
    let mut library = open(&storage);
    library.save(&storage).unwrap();
    assert_eq!(storage.write_count(), 0);
    assert_eq!(storage.text("songs.md").unwrap(), SONGS);
}

#[test]
fn a_full_session_saves_and_reopens() {
    let storage = MemStorage::new();
    let mut library = Library::empty();
    library
        .add_song(Song::new("Hotel California", 75.0, 4).with_notes("Bm, capo 7"))
        .unwrap();
    library
        .add_song(Song::new("Waltz for Debby", 132.0, 3))
        .unwrap();
    let stem = library
        .create_setlist("Friday Gig", Some("The Band"))
        .unwrap();
    assert_eq!(stem, "friday-gig");
    library
        .add_song_to_setlist(&stem, "waltz for debby")
        .unwrap();
    library
        .add_song_to_setlist(&stem, "HOTEL CALIFORNIA")
        .unwrap();
    assert!(library.is_dirty());
    library.save(&storage).unwrap();
    assert!(!library.is_dirty());

    assert_eq!(
        storage.text("setlists/friday-gig.md").unwrap(),
        "---\nband: The Band\n---\n# Friday Gig\n\n1. Waltz for Debby\n2. Hotel California\n"
    );
    let songs = storage.text("songs.md").unwrap();
    assert!(songs.starts_with("# Songs\n\n| Song"), "{songs}");

    let again = open(&storage);
    assert_eq!(titles(&again), ["Hotel California", "Waltz for Debby"]);
    assert_eq!(again.song("Hotel California").unwrap().notes, "Bm, capo 7");
    assert_eq!(
        again.setlist(&stem).unwrap().songs(),
        ["Waltz for Debby", "Hotel California"]
    );
}

#[test]
fn song_titles_must_be_unique_and_names_not_empty() {
    let mut library = open(&hand_written());
    assert_eq!(
        library.add_song(Song::new(" superstition ", 90.0, 4)),
        Err(LibraryError::DuplicateTitle("superstition".to_string()))
    );
    assert_eq!(
        library.add_song(Song::new("  ", 90.0, 4)),
        Err(LibraryError::EmptyName)
    );
    assert_eq!(
        library.update_song("Waltz for Debby", Song::new("Superstition", 90.0, 4)),
        Err(LibraryError::DuplicateTitle("Superstition".to_string()))
    );
    assert_eq!(
        library.update_song("Nope", Song::new("X", 90.0, 4)),
        Err(LibraryError::NoSuchSong("Nope".to_string()))
    );
    assert_eq!(
        library.create_setlist("  ", None),
        Err(LibraryError::EmptyName)
    );
    assert!(!library.is_dirty(), "failed operations change nothing");
}

#[test]
fn renaming_a_song_renames_it_in_every_setlist() {
    let storage = hand_written().with(
        "setlists/wedding-set.md",
        "# Wedding set\n\n- superstition\n- Hotel California\n- SUPERSTITION\n",
    );
    let mut library = open(&storage);
    let old = library.song("Superstition").unwrap().clone();
    library
        .update_song(
            "Superstition",
            Song {
                title: "Superstition (live)".into(),
                ..old
            },
        )
        .unwrap();
    library.save(&storage).unwrap();

    assert_eq!(
        library.setlist("friday-gig").unwrap().songs(),
        ["Hotel California", "Superstition (live)", "Waltz for Debby"]
    );
    let wedding = open(&storage);
    assert_eq!(
        wedding.setlist("wedding-set").unwrap().songs(),
        [
            "Superstition (live)",
            "Hotel California",
            "Superstition (live)"
        ]
    );
    assert!(wedding.missing_songs("wedding-set").is_empty());
    assert!(wedding.missing_songs("friday-gig").is_empty());
}

#[test]
fn changing_only_a_songs_tempo_leaves_setlists_alone() {
    let storage = hand_written();
    let mut library = open(&storage);
    let old = library.song("Superstition").unwrap().clone();
    library
        .update_song("superstition", Song { bpm: 104.0, ..old })
        .unwrap();
    library.save(&storage).unwrap();
    assert_eq!(storage.write_count(), 1, "only songs.md is written");
    assert_eq!(storage.text("setlists/friday-gig.md").unwrap(), FRIDAY);
}

#[test]
fn deleting_a_song_removes_it_from_every_setlist_and_reports_where() {
    let storage = hand_written().with(
        "setlists/wedding-set.md",
        "# Wedding set\n\n- Superstition\n- Superstition\n",
    );
    let mut library = open(&storage);
    assert_eq!(
        library.setlists_using("superstition"),
        vec![
            SetlistUse {
                stem: "friday-gig".into(),
                name: "Friday Gig".into(),
                count: 1
            },
            SetlistUse {
                stem: "wedding-set".into(),
                name: "Wedding set".into(),
                count: 2
            },
        ]
    );
    let removed = library.delete_song("SUPERSTITION").unwrap();
    assert_eq!(removed.len(), 2);
    assert_eq!(
        library.delete_song("Superstition"),
        Err(LibraryError::NoSuchSong("Superstition".into()))
    );
    library.save(&storage).unwrap();

    let again = open(&storage);
    assert_eq!(titles(&again), ["Hotel California", "Waltz for Debby"]);
    assert_eq!(
        again.setlist("friday-gig").unwrap().songs(),
        ["Hotel California", "Waltz for Debby"]
    );
    assert!(again.setlist("wedding-set").unwrap().songs().is_empty());
    assert!(again.setlists_using("Superstition").is_empty());
}

#[test]
fn only_existing_songs_can_be_added_to_a_setlist_and_with_the_librarys_spelling() {
    let mut library = open(&hand_written());
    library
        .add_song_to_setlist("friday-gig", "hotel california")
        .unwrap();
    assert_eq!(
        library
            .setlist("friday-gig")
            .unwrap()
            .songs()
            .last()
            .unwrap(),
        "Hotel California"
    );
    assert_eq!(
        library.add_song_to_setlist("friday-gig", "Not a song"),
        Err(LibraryError::NoSuchSong("Not a song".into()))
    );
    assert_eq!(
        library.add_song_to_setlist("nope", "Hotel California"),
        Err(LibraryError::NoSuchSetlist("nope".into()))
    );
}

#[test]
fn reordering_and_removing_songs_in_a_setlist() {
    let storage = hand_written();
    let mut library = open(&storage);
    library.move_song_in_setlist("friday-gig", 2, 0).unwrap();
    assert_eq!(
        library.setlist("friday-gig").unwrap().songs(),
        ["Waltz for Debby", "Hotel California", "Superstition"]
    );
    library.remove_song_from_setlist("friday-gig", 1).unwrap();
    assert_eq!(
        library.move_song_in_setlist("friday-gig", 0, 5),
        Err(LibraryError::PositionOutOfRange)
    );
    assert_eq!(
        library.remove_song_from_setlist("friday-gig", 9),
        Err(LibraryError::PositionOutOfRange)
    );
    library.save(&storage).unwrap();
    assert_eq!(
        storage.text("setlists/friday-gig.md").unwrap(),
        "---\nband: The Band\nowner: Martin # keep\n---\n# Friday Gig\n\nIntro for the sound engineer.\n\n1. Waltz for Debby\n2. Superstition\n\nSoundcheck at 18:00.\n"
    );
}

#[test]
fn copying_a_setlist() {
    let storage = hand_written();
    let mut library = open(&storage);
    let copy = library.copy_setlist("friday-gig").unwrap();
    assert_eq!(copy, "friday-gig-copy");
    let again = library.copy_setlist("friday-gig").unwrap();
    assert_eq!(again, "friday-gig-copy-2");
    let copy_of_copy = library.copy_setlist(&copy).unwrap();
    assert_eq!(
        library.setlist(&copy_of_copy).unwrap().name(),
        "Friday Gig (copy) (copy)"
    );
    library.save(&storage).unwrap();

    let reopened = open(&storage);
    let copy = reopened.setlist("friday-gig-copy").unwrap();
    assert_eq!(copy.name(), "Friday Gig (copy)");
    assert_eq!(copy.band(), Some("The Band"));
    assert_eq!(
        copy.songs(),
        reopened.setlist("friday-gig").unwrap().songs()
    );
    assert_eq!(
        storage.text("setlists/friday-gig.md").unwrap(),
        FRIDAY,
        "the original is untouched"
    );
}

#[test]
fn renaming_and_rebanding_a_setlist_keeps_its_file_name() {
    let storage = hand_written();
    let mut library = open(&storage);
    library
        .rename_setlist("friday-gig", "Saturday Gig")
        .unwrap();
    library
        .set_setlist_band("friday-gig", Some("The Duo"))
        .unwrap();
    library.save(&storage).unwrap();
    assert_eq!(
        storage
            .paths()
            .iter()
            .filter(|p| p.starts_with("setlists/"))
            .count(),
        1
    );
    let again = open(&storage);
    let s = again.setlist("friday-gig").unwrap();
    assert_eq!(
        (s.name().as_str(), s.band()),
        ("Saturday Gig", Some("The Duo"))
    );
    assert!(
        storage
            .text("setlists/friday-gig.md")
            .unwrap()
            .contains("owner: Martin # keep")
    );
    assert_eq!(
        library.rename_setlist("nope", "x"),
        Err(LibraryError::NoSuchSetlist("nope".into()))
    );
}

#[test]
fn deleting_a_setlist_removes_its_file_at_the_next_save() {
    let storage = hand_written();
    let mut library = open(&storage);
    library.delete_setlist("friday-gig").unwrap();
    assert!(library.setlists().is_empty());
    assert!(storage.text("setlists/friday-gig.md").is_some(), "not yet");
    assert!(library.is_dirty());
    // A new setlist with the same name must not reuse the file that is about to be deleted.
    let stem = library.create_setlist("Friday Gig", None).unwrap();
    assert_eq!(stem, "friday-gig-2");
    library.save(&storage).unwrap();
    assert!(storage.text("setlists/friday-gig.md").is_none());
    assert!(storage.text("setlists/friday-gig-2.md").is_some());
    assert_eq!(
        library.delete_setlist("nope"),
        Err(LibraryError::NoSuchSetlist("nope".into()))
    );
}

#[test]
fn a_setlist_created_and_deleted_before_saving_leaves_no_trace() {
    let storage = MemStorage::new();
    let mut library = Library::empty();
    let stem = library.create_setlist("Temp", None).unwrap();
    library.delete_setlist(&stem).unwrap();
    library.save(&storage).unwrap();
    assert!(storage.paths().is_empty());
}

#[test]
fn new_setlist_files_do_not_clash_with_existing_ones() {
    let mut library = open(&hand_written());
    assert_eq!(
        library.create_setlist("Friday Gig", None).unwrap(),
        "friday-gig-2"
    );
    assert_eq!(
        library.create_setlist("Friday   GIG!", None).unwrap(),
        "friday-gig-3"
    );
    assert_eq!(library.create_setlist("日本語", None).unwrap(), "untitled");
}

#[test]
fn hand_edits_survive_everything_the_app_does() {
    let storage = hand_written();
    let mut library = open(&storage);
    // Add a song, edit a song, add a setlist, change a setting.
    library.add_song(Song::new("Wonderwall", 87.0, 4)).unwrap();
    let old = library.song("Superstition").unwrap().clone();
    library
        .update_song("Superstition", Song { bpm: 101.0, ..old })
        .unwrap();
    library
        .add_song_to_setlist("friday-gig", "Wonderwall")
        .unwrap();
    let mut settings = library.settings().clone();
    settings.volume = 0.5;
    library.update_settings(settings).unwrap();
    library.save(&storage).unwrap();

    let songs = storage.text("songs.md").unwrap();
    assert!(
        songs.starts_with("# My songs\n\nA note for myself.\n\n"),
        "{songs}"
    );
    assert!(songs.ends_with("\nMore text after the table.\n"), "{songs}");
    assert!(
        songs.contains("Key") && songs.contains("Bm") && songs.contains("Eb"),
        "extra column: {songs}"
    );
    assert!(songs.contains("Wonderwall"));

    let friday = storage.text("setlists/friday-gig.md").unwrap();
    assert!(friday.contains("owner: Martin # keep"), "{friday}");
    assert!(friday.contains("Intro for the sound engineer."), "{friday}");
    assert!(friday.contains("Soundcheck at 18:00."), "{friday}");
    assert!(friday.contains("4. Wonderwall"), "{friday}");

    let settings = storage.text("settings.md").unwrap();
    assert_eq!(
        settings,
        "---\nformat: 2\nsound: wood\nvolume: 0.5\ntheme: night   # not ours\n---\n# Settings\nMy own notes.\n"
    );
}

#[test]
fn missing_songs_are_reported_and_kept() {
    let storage = hand_written().with(
        "setlists/odd.md",
        "# Odd\n\n1. Superstition\n2. Song that does not exist\n3. waltz for debby\n",
    );
    let mut library = open(&storage);
    assert_eq!(library.missing_songs("odd"), [1]);
    let entries = library.entries("odd").unwrap();
    assert_eq!(entries[0].song.map(|s| s.bpm), Some(100.0));
    assert_eq!(entries[1].title, "Song that does not exist");
    assert!(entries[1].song.is_none());
    assert_eq!(entries[2].song.map(|s| s.beats), Some(3));
    assert!(library.entries("nope").is_none());
    // Editing another setlist never touches it, and saving keeps the missing entry.
    library.rename_setlist("odd", "Odd one").unwrap();
    library.save(&storage).unwrap();
    assert!(
        storage
            .text("setlists/odd.md")
            .unwrap()
            .contains("Song that does not exist")
    );
}

#[test]
fn setlists_are_grouped_by_band() {
    let storage = MemStorage::new()
        .with("setlists/b.md", "---\nband: the band\n---\n# Zebra\n")
        .with("setlists/a.md", "---\nband: The Band\n---\n# Alpha\n")
        .with("setlists/c.md", "---\nband: Duo\n---\n# Cafe\n")
        .with("setlists/d.md", "# Loose\n")
        .with("setlists/e.md", "---\nband:\n---\n# Also loose\n");
    let library = open(&storage);
    // "The Band" and "the band" are one band; the first spelling seen (in file-name order) is shown.
    assert_eq!(library.bands(), ["Duo", "The Band"]);
    let groups = library.setlists_by_band();
    let shape: Vec<(Option<&str>, Vec<String>)> = groups
        .iter()
        .map(|(band, lists)| (band.as_deref(), lists.iter().map(|s| s.name()).collect()))
        .collect();
    assert_eq!(
        shape,
        vec![
            (Some("Duo"), vec!["Cafe".to_string()]),
            (
                Some("The Band"),
                vec!["Alpha".to_string(), "Zebra".to_string()]
            ),
            (None, vec!["Also loose".to_string(), "Loose".to_string()]),
        ]
    );
}

#[test]
fn windows_line_endings_and_a_bom_are_accepted() {
    let storage = MemStorage::new()
        .with(
            "songs.md",
            "\u{feff}# Songs\r\n\r\n| Song | BPM |\r\n|---|---|\r\n| A | 90 |\r\n",
        )
        .with(
            "setlists/x.md",
            "---\r\nband: B\r\n---\r\n# X\r\n\r\n1. A\r\n",
        );
    let opened = Library::open(&storage).unwrap();
    assert!(opened.warnings.is_empty(), "{:?}", opened.warnings);
    assert_eq!(titles(&opened.library), ["A"]);
    assert_eq!(opened.library.setlist("x").unwrap().songs(), ["A"]);
    assert_eq!(opened.library.setlist("x").unwrap().band(), Some("B"));
}

#[test]
fn problems_in_files_become_warnings_with_the_file_name() {
    let storage = MemStorage::new()
        .with("songs.md", "| Song | BPM |\n|---|---|\n| A | fast |\n")
        .with("settings.md", "---\nsound: gong\n---\n")
        .with("setlists/x.md", "---\nband: A\nband: B\n---\n# X\n");
    let opened = Library::open(&storage).unwrap();
    let files: Vec<&str> = opened.warnings.iter().map(|w| w.file.as_str()).collect();
    assert_eq!(files, ["settings.md", "songs.md", "setlists/x.md"]);
    assert_eq!(opened.warnings[1].problem.line, Some(3));
}

#[test]
fn an_unreadable_songs_file_is_left_alone() {
    let storage = hand_written();
    storage
        .write("songs.md", &[0xff, 0xfe, 0x00, 0x80, b'x'])
        .unwrap();
    let before = storage.write_count();
    let opened = Library::open(&storage).unwrap();
    assert!(
        opened
            .warnings
            .iter()
            .any(|w| w.file == "songs.md" && w.problem.message.contains("not valid text"))
    );
    let mut library = opened.library;
    assert!(library.songs().is_empty());
    assert_eq!(
        library.add_song(Song::new("A", 90.0, 4)),
        Err(LibraryError::FileUnreadable("songs.md".into()))
    );
    assert_eq!(
        library.update_song("A", Song::new("B", 90.0, 4)),
        Err(LibraryError::FileUnreadable("songs.md".into()))
    );
    assert_eq!(
        library.delete_song("A"),
        Err(LibraryError::FileUnreadable("songs.md".into()))
    );
    library.rename_setlist("friday-gig", "Renamed").unwrap();
    library.save(&storage).unwrap();
    assert_eq!(
        storage.read("songs.md").unwrap().unwrap(),
        [0xff, 0xfe, 0x00, 0x80, b'x']
    );
    assert_eq!(
        storage.write_count(),
        before + 1,
        "only the setlist was written"
    );
}

#[test]
fn an_unreadable_setlist_is_skipped_and_its_file_name_stays_reserved() {
    let storage = hand_written();
    storage
        .write("setlists/broken.md", &[0xc3, 0x28, 0xa0, 0xa1])
        .unwrap();
    let mut library = Library::open(&storage).unwrap().library;
    assert_eq!(library.unreadable_setlists(), ["broken"]);
    assert_eq!(library.setlists().len(), 1);
    assert_eq!(library.create_setlist("Broken", None).unwrap(), "broken-2");
    library.save(&storage).unwrap();
    assert_eq!(
        storage.read("setlists/broken.md").unwrap().unwrap(),
        [0xc3, 0x28, 0xa0, 0xa1]
    );
}

#[test]
fn files_that_are_not_setlists_are_ignored() {
    let storage = hand_written()
        .with("setlists/notes.txt", "x")
        .with("setlists/.hidden.md", "# hidden")
        .with("setlists/.friday-gig.md.tmp", "partial");
    let library = open(&storage);
    assert_eq!(library.setlists().len(), 1);
}

#[test]
fn an_interrupted_save_loses_nothing_and_can_be_retried() {
    let storage = MemStorage::new();
    let mut library = Library::empty();
    library.add_song(Song::new("A", 90.0, 4)).unwrap();
    let stem = library.create_setlist("Gig", None).unwrap();
    library.add_song_to_setlist(&stem, "A").unwrap();
    let mut settings = library.settings().clone();
    settings.sound = Sound::Rim;
    library.update_settings(settings).unwrap();

    storage.fail_writes_after(1); // songs.md succeeds, the setlist write fails
    assert!(matches!(
        library.save(&storage),
        Err(LibraryError::Storage(_))
    ));
    assert!(library.is_dirty());
    assert!(storage.text("songs.md").is_some());
    assert!(storage.text("setlists/gig.md").is_none());

    let healthy = MemStorage::new().with("songs.md", &storage.text("songs.md").unwrap());
    library.save(&healthy).unwrap();
    assert!(!library.is_dirty());
    let again = open(&healthy);
    assert_eq!(titles(&again), ["A"]);
    assert_eq!(again.setlist("gig").unwrap().songs(), ["A"]);
    assert_eq!(again.settings().sound, Sound::Rim);
}

#[test]
fn saving_twice_writes_once() {
    let storage = MemStorage::new();
    let mut library = Library::empty();
    library.add_song(Song::new("A", 90.0, 4)).unwrap();
    library.save(&storage).unwrap();
    library.save(&storage).unwrap();
    assert_eq!(storage.write_count(), 1);
}

#[test]
fn settings_are_saved_and_a_newer_file_is_protected() {
    let storage = MemStorage::new();
    let mut library = Library::empty();
    let mut s = library.settings().clone();
    s.keep_screen_on = false;
    s.last_setlist = Some("friday-gig".into());
    library.update_settings(s).unwrap();
    library.save(&storage).unwrap();
    let again = open(&storage);
    assert!(!again.settings().keep_screen_on);
    assert_eq!(again.settings().last_setlist.as_deref(), Some("friday-gig"));

    let newer = MemStorage::new().with("settings.md", "---\nformat: 9\nsound: wood\n---\n");
    let mut library = open(&newer);
    assert_eq!(library.settings().sound, Sound::Wood);
    let mut s = library.settings().clone();
    s.sound = Sound::Click;
    assert_eq!(
        library.update_settings(s),
        Err(LibraryError::ReadOnly("settings.md".into()))
    );
}

#[test]
fn errors_read_as_sentences() {
    assert_eq!(
        LibraryError::DuplicateTitle("Hotel".into()).to_string(),
        "there is already a song called \"Hotel\""
    );
    assert!(
        LibraryError::FileUnreadable("songs.md".into())
            .to_string()
            .contains("untouched")
    );
}

// ---- a model-based test: random operations never break the library's rules ------------------

#[derive(Clone, Debug)]
enum Op {
    AddSong(usize, u32, u32),
    UpdateSong(usize, usize, u32),
    DeleteSong(usize),
    CreateSetlist(usize, bool),
    AddToSetlist(usize, usize),
    RemoveFromSetlist(usize, usize),
    Move(usize, usize, usize),
    Copy(usize),
    DeleteSetlist(usize),
    Rename(usize, usize),
    SetBand(usize, usize),
    SaveAndReopen,
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        (0usize..6, 30u32..300, 1u32..12).prop_map(|(t, b, n)| Op::AddSong(t, b, n)),
        (0usize..6, 0usize..6, 30u32..300).prop_map(|(i, t, b)| Op::UpdateSong(i, t, b)),
        (0usize..6).prop_map(Op::DeleteSong),
        (0usize..4, any::<bool>()).prop_map(|(n, b)| Op::CreateSetlist(n, b)),
        (0usize..4, 0usize..6).prop_map(|(s, i)| Op::AddToSetlist(s, i)),
        (0usize..4, 0usize..6).prop_map(|(s, i)| Op::RemoveFromSetlist(s, i)),
        (0usize..4, 0usize..6, 0usize..6).prop_map(|(s, a, b)| Op::Move(s, a, b)),
        (0usize..4).prop_map(Op::Copy),
        (0usize..4).prop_map(Op::DeleteSetlist),
        (0usize..4, 0usize..4).prop_map(|(s, n)| Op::Rename(s, n)),
        (0usize..4, 0usize..3).prop_map(|(s, b)| Op::SetBand(s, b)),
        Just(Op::SaveAndReopen),
    ]
}

const TITLES: [&str; 6] = [
    "Alpha",
    "beta",
    " Gamma ",
    "alpha",
    "Delta | Two",
    "Épsilon",
];
const NAMES: [&str; 4] = ["Friday Gig", "Wedding set", "Café evening", "friday gig"];
const BANDS: [Option<&str>; 3] = [Some("The Band"), Some("Duo"), None];

type Snapshot = (
    Vec<(String, u64, u32)>,
    Vec<(String, String, Option<String>, Vec<String>)>,
);

fn snapshot(library: &Library) -> Snapshot {
    (
        library
            .songs()
            .iter()
            .map(|s| (s.title.clone(), s.bpm.to_bits(), s.beats))
            .collect(),
        library
            .setlists()
            .iter()
            .map(|s| {
                (
                    s.stem().to_string(),
                    s.name(),
                    s.band().map(str::to_string),
                    s.songs().to_vec(),
                )
            })
            .collect(),
    )
}

fn check_rules(library: &Library) -> Result<(), TestCaseError> {
    let mut seen = std::collections::HashSet::new();
    for song in library.songs() {
        prop_assert!(
            seen.insert(title_key(&song.title)),
            "duplicate title {:?}",
            song.title
        );
        prop_assert!((30.0..=300.0).contains(&song.bpm) && (1..=99).contains(&song.beats));
    }
    let mut stems = std::collections::HashSet::new();
    for setlist in library.setlists() {
        prop_assert!(stems.insert(setlist.stem().to_string()), "duplicate stem");
        prop_assert!(
            library.missing_songs(setlist.stem()).is_empty(),
            "missing songs in {}",
            setlist.stem()
        );
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn random_sessions_keep_the_rules_and_survive_saving(ops in proptest::collection::vec(op(), 0..40)) {
        let storage = MemStorage::new();
        let mut library = Library::empty();
        for op in ops {
            let stems: Vec<String> = library.setlists().iter().map(|s| s.stem().to_string()).collect();
            let titles_now = titles(&library);
            let stem = |i: usize| stems.get(i % stems.len().max(1)).cloned().unwrap_or_default();
            let title = |i: usize| titles_now.get(i % titles_now.len().max(1)).cloned().unwrap_or_default();
            match op {
                Op::AddSong(t, b, n) => { let _ = library.add_song(Song::new(TITLES[t], f64::from(b), n)); }
                Op::UpdateSong(i, t, b) => { let _ = library.update_song(&title(i), Song::new(TITLES[t], f64::from(b), 4)); }
                Op::DeleteSong(i) => { let _ = library.delete_song(&title(i)); }
                Op::CreateSetlist(n, band) => { let _ = library.create_setlist(NAMES[n], if band { Some("The Band") } else { None }); }
                Op::AddToSetlist(s, i) => { let _ = library.add_song_to_setlist(&stem(s), &title(i)); }
                Op::RemoveFromSetlist(s, i) => { let _ = library.remove_song_from_setlist(&stem(s), i); }
                Op::Move(s, a, b) => { let _ = library.move_song_in_setlist(&stem(s), a, b); }
                Op::Copy(s) => { let _ = library.copy_setlist(&stem(s)); }
                Op::DeleteSetlist(s) => { let _ = library.delete_setlist(&stem(s)); }
                Op::Rename(s, n) => { let _ = library.rename_setlist(&stem(s), NAMES[n]); }
                Op::SetBand(s, b) => { let _ = library.set_setlist_band(&stem(s), BANDS[b]); }
                Op::SaveAndReopen => {
                    library.save(&storage).unwrap();
                    let reopened = Library::open(&storage).unwrap();
                    prop_assert!(reopened.warnings.is_empty(), "{:?}", reopened.warnings);
                    prop_assert_eq!(snapshot(&reopened.library), snapshot(&library));
                    library = reopened.library;
                }
            }
            check_rules(&library)?;
        }
        library.save(&storage).unwrap();
        let reopened = Library::open(&storage).unwrap();
        prop_assert_eq!(snapshot(&reopened.library), snapshot(&library));
    }
}
