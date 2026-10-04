//! Export and import of the whole library as one zip file, for moving to a new phone.
//!
//! The archive holds the same layout as the library folder: `settings.md`, `songs.md` and
//! `setlists/<name>.md`. Import is defensive: it is all-or-nothing, never reads outside that
//! layout, and refuses archives with path tricks, too many files or oversized content.

use std::fmt;

use super::setlist::SetlistDoc;
use super::settings::SettingsDoc;
use super::songs::SongsDoc;
use super::storage::Storage;
use super::text::normalize;
use super::zip::{self, Limits, ZipError};
use super::{Library, LibraryError, Problem, Warning};

/// The most an archive may contain: 1,000 files, 1 MB each, 20 MB in all.
pub const LIMITS: Limits = Limits {
    max_entries: 1_000,
    max_file_bytes: 1 << 20,
    max_total_bytes: 20 << 20,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArchiveError {
    Zip(ZipError),
    /// An entry's path would reach outside the library (`../x`, `/etc/x`, `C:x`). The whole
    /// archive is refused.
    UnsafePath(String),
    Library(LibraryError),
    Storage(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zip(e) => write!(f, "{e}"),
            Self::UnsafePath(p) => write!(
                f,
                "the zip file contains an unsafe path (\"{p}\") and was not imported"
            ),
            Self::Library(e) => write!(f, "{e}"),
            Self::Storage(e) => write!(f, "could not read the library: {e}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

impl From<LibraryError> for ArchiveError {
    fn from(e: LibraryError) -> Self {
        Self::Library(e)
    }
}

/// What to do with `settings.md` from the archive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsConflict {
    Skip,
    Overwrite,
}

/// What to do when a setlist with the same file name already exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetlistConflict {
    Skip,
    Overwrite,
    /// Import it under a new file name, next to the existing one.
    KeepBoth,
}

/// What to do with a song whose title already exists. Songs not yet in the library are always
/// added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongConflict {
    SkipExisting,
    Overwrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportOptions {
    pub settings: SettingsConflict,
    pub setlists: SetlistConflict,
    pub songs: SongConflict,
}

impl Default for ImportOptions {
    /// The safe choice: keep what is already there, add what is new.
    fn default() -> Self {
        Self {
            settings: SettingsConflict::Skip,
            setlists: SetlistConflict::KeepBoth,
            songs: SongConflict::SkipExisting,
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct ImportReport {
    pub songs_added: usize,
    pub songs_overwritten: usize,
    pub songs_skipped: usize,
    pub setlists_added: Vec<String>,
    pub setlists_overwritten: Vec<String>,
    pub setlists_skipped: Vec<String>,
    /// (name in the archive, name it was given)
    pub setlists_kept_both: Vec<(String, String)>,
    pub settings_imported: bool,
    /// Entries that are not part of the library layout, with the reason.
    pub ignored: Vec<String>,
    /// Oddities in the imported files.
    pub warnings: Vec<Warning>,
}

/// The library files as a zip archive. Save the library first if it has unsaved changes.
pub fn export(storage: &dyn Storage) -> Result<Vec<u8>, ArchiveError> {
    let io = |e: std::io::Error| ArchiveError::Storage(e.to_string());
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for path in ["settings.md", "songs.md"] {
        if let Some(bytes) = storage.read(path).map_err(io)? {
            files.push((path.to_string(), bytes));
        }
    }
    for name in storage.list("setlists").map_err(io)? {
        if name.ends_with(".md") && !name.starts_with('.') {
            let path = format!("setlists/{name}");
            if let Some(bytes) = storage.read(&path).map_err(io)? {
                files.push((path, bytes));
            }
        }
    }
    if files.len() > LIMITS.max_entries
        || files.iter().any(|(_, b)| b.len() > LIMITS.max_file_bytes)
        || files.iter().map(|(_, b)| b.len()).sum::<usize>() > LIMITS.max_total_bytes
    {
        // An archive that could not be imported again is not worth making.
        return Err(ArchiveError::Zip(ZipError::TooLarge));
    }
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice()))
        .collect();
    zip::write_zip(&refs).map_err(ArchiveError::Zip)
}

/// A path that could leave the library folder.
fn is_unsafe(name: &str) -> bool {
    let bytes = name.as_bytes();
    name.starts_with('/')
        || name.split('/').any(|part| part == "..")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
}

fn is_junk(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name
            .split('/')
            .any(|part| part == ".DS_Store" || part == "Thumbs.db" || part.starts_with("._"))
}

enum Kind {
    Settings,
    Songs,
    Setlist(String),
}

fn classify(name: &str) -> Option<Kind> {
    match name {
        "settings.md" => Some(Kind::Settings),
        "songs.md" => Some(Kind::Songs),
        _ => {
            let file = name.strip_prefix("setlists/")?;
            let stem = file.strip_suffix(".md")?;
            (!stem.is_empty() && !stem.contains('/') && !stem.starts_with('.'))
                .then(|| Kind::Setlist(stem.to_string()))
        }
    }
}

/// Merge an archive into `library` (in memory; save the library afterwards).
///
/// Nothing is changed unless the whole archive is acceptable.
pub fn import(
    library: &mut Library,
    archive: &[u8],
    options: &ImportOptions,
) -> Result<ImportReport, ArchiveError> {
    let entries = zip::read_zip(archive, &LIMITS).map_err(ArchiveError::Zip)?;
    // Refuse the whole archive for any path trick, even in entries that would be ignored.
    if let Some(bad) = entries.iter().find(|e| is_unsafe(&e.name)) {
        return Err(ArchiveError::UnsafePath(bad.name.clone()));
    }

    let mut report = ImportReport::default();
    let mut wanted = Vec::new();
    for entry in entries {
        let name = entry.name.trim_start_matches("./").to_string();
        if is_junk(&name) {
            report.ignored.push(format!("{name}: system file"));
        } else {
            wanted.push((name, entry.data));
        }
    }
    // A folder zipped on a computer has everything inside one top-level folder: look inside it.
    let first = |n: &str| n.split('/').next().unwrap_or("").to_string();
    if let Some((head, _)) = wanted.first()
        && wanted
            .iter()
            .all(|(n, _)| n.contains('/') && first(n) == first(head))
        && first(head) != "setlists"
    {
        let prefix = format!("{}/", first(head));
        for (name, _) in &mut wanted {
            *name = name.strip_prefix(&prefix).unwrap_or(name).to_string();
        }
    }

    let mut settings: Option<SettingsDoc> = None;
    let mut songs: Option<SongsDoc> = None;
    let mut setlists: Vec<SetlistDoc> = Vec::new();
    for (name, data) in wanted {
        let Some(kind) = classify(&name) else {
            report
                .ignored
                .push(format!("{name}: not part of the library"));
            continue;
        };
        let Ok(text) = String::from_utf8(data) else {
            report.ignored.push(format!("{name}: not valid text"));
            continue;
        };
        let text = normalize(&text);
        let mut problems: Vec<Problem> = Vec::new();
        match kind {
            Kind::Settings => settings = Some(SettingsDoc::parse(&text, &mut problems)),
            Kind::Songs => songs = Some(SongsDoc::parse(&text, &mut problems)),
            Kind::Setlist(stem) => setlists.push(SetlistDoc::parse(&stem, &text, &mut problems)),
        }
        report
            .warnings
            .extend(problems.into_iter().map(|problem| Warning {
                file: name.clone(),
                problem,
            }));
    }

    // Everything above only read the archive. Check what could stop the import, then apply.
    if songs.as_ref().is_some_and(|s| s.songs().next().is_some()) {
        library.songs_writable()?;
    }

    if let Some(incoming) = &songs {
        for song in incoming.songs() {
            match (library.song(&song.title).is_some(), options.songs) {
                (false, _) => {
                    library.add_song(song.clone())?;
                    report.songs_added += 1;
                }
                (true, SongConflict::SkipExisting) => report.songs_skipped += 1,
                (true, SongConflict::Overwrite) => {
                    library.update_song(&song.title, song.clone())?;
                    report.songs_overwritten += 1;
                }
            }
        }
    }

    for setlist in setlists {
        let stem = setlist.stem().to_string();
        let exists = library.setlist(&stem).is_some();
        let blocked = library.unreadable_setlists().contains(&stem);
        if !exists && !blocked {
            library.put_setlist(setlist.with_stem(&stem));
            report.setlists_added.push(stem);
            continue;
        }
        match if blocked {
            SetlistConflict::KeepBoth
        } else {
            options.setlists
        } {
            SetlistConflict::Skip => report.setlists_skipped.push(stem),
            SetlistConflict::Overwrite => {
                library.put_setlist(setlist.with_stem(&stem));
                report.setlists_overwritten.push(stem);
            }
            SetlistConflict::KeepBoth => {
                let new_stem = library.fresh_setlist_stem(&stem);
                library.put_setlist(setlist.with_stem(&new_stem));
                report.setlists_kept_both.push((stem, new_stem));
            }
        }
    }

    if let (Some(incoming), SettingsConflict::Overwrite) = (&settings, options.settings) {
        match library.replace_settings_with(incoming) {
            Ok(()) => report.settings_imported = true,
            Err(e) => report.warnings.push(Warning {
                file: "settings.md".to_string(),
                problem: Problem::general(format!("the settings were not imported: {e}")),
            }),
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::songs::Song;
    use crate::library::storage::MemStorage;
    use proptest::prelude::*;

    fn library_with_data() -> (MemStorage, Library) {
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
        library
            .add_song_to_setlist(&stem, "Waltz for Debby")
            .unwrap();
        library
            .add_song_to_setlist(&stem, "Hotel California")
            .unwrap();
        library.save(&storage).unwrap();
        (storage, library)
    }

    fn zip_of(files: &[(&str, &str)]) -> Vec<u8> {
        let refs: Vec<(&str, &[u8])> = files.iter().map(|(n, t)| (*n, t.as_bytes())).collect();
        zip::write_zip(&refs).unwrap()
    }

    fn titles(library: &Library) -> Vec<String> {
        library.songs().iter().map(|s| s.title.clone()).collect()
    }

    #[test]
    fn export_then_import_recreates_the_library() {
        let (storage, source) = library_with_data();
        let archive = export(&storage).unwrap();

        let mut target = Library::empty();
        let report = import(&mut target, &archive, &ImportOptions::default()).unwrap();
        assert_eq!(report.songs_added, 2);
        assert_eq!(report.setlists_added, ["friday-gig"]);
        assert!(
            report.ignored.is_empty() && report.warnings.is_empty(),
            "{report:?}"
        );

        let fresh = MemStorage::new();
        target.save(&fresh).unwrap();
        for path in ["songs.md", "setlists/friday-gig.md"] {
            assert_eq!(fresh.text(path), storage.text(path), "{path}");
        }
        assert_eq!(titles(&target), titles(&source));
    }

    #[test]
    fn export_contains_only_the_library_files() {
        let (storage, _) = library_with_data();
        storage.write("setlists/notes.txt", b"x").unwrap();
        storage.write("setlists/.songs.md.tmp", b"x").unwrap();
        storage.write("elsewhere/x.md", b"x").unwrap();
        let entries = zip::read_zip(&export(&storage).unwrap(), &LIMITS).unwrap();
        let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["songs.md", "setlists/friday-gig.md"]);
    }

    #[test]
    fn an_empty_library_exports_an_empty_archive() {
        let entries = zip::read_zip(&export(&MemStorage::new()).unwrap(), &LIMITS).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn a_folder_zipped_on_a_computer_is_unwrapped() {
        let archive = zip_of(&[
            (
                "Metronom/songs.md",
                "| Song | BPM |\n|---|---|\n| A | 90 |\n",
            ),
            ("Metronom/setlists/x.md", "# X\n\n1. A\n"),
        ]);
        let mut library = Library::empty();
        let report = import(&mut library, &archive, &ImportOptions::default()).unwrap();
        assert_eq!(titles(&library), ["A"]);
        assert_eq!(report.setlists_added, ["x"]);
        assert!(report.ignored.is_empty(), "{:?}", report.ignored);
    }

    #[test]
    fn path_tricks_refuse_the_whole_archive_and_change_nothing() {
        for bad in [
            "../evil.md",
            "setlists/../../evil.md",
            "/etc/passwd",
            "C:/x.md",
            "setlists/..",
            "a/b/../../../c.md",
        ] {
            let archive = zip_of(&[
                ("songs.md", "| Song | BPM |\n|---|---|\n| A | 90 |\n"),
                (bad, "x"),
            ]);
            let mut library = Library::empty();
            let result = import(&mut library, &archive, &ImportOptions::default());
            assert!(
                matches!(result, Err(ArchiveError::UnsafePath(_))),
                "{bad}: {result:?}"
            );
            assert!(library.songs().is_empty(), "{bad}: nothing may be imported");
            assert!(!library.is_dirty());
        }
    }

    #[test]
    fn backslash_paths_are_refused_by_the_zip_layer() {
        let archive = zip_of(&[("setlists\\..\\x.md", "x")]);
        assert!(matches!(
            import(&mut Library::empty(), &archive, &ImportOptions::default()),
            Err(ArchiveError::Zip(ZipError::BadName))
        ));
    }

    #[test]
    fn things_outside_the_layout_are_ignored_with_a_reason() {
        let archive = zip_of(&[
            ("songs.md", "| Song | BPM |\n|---|---|\n| A | 90 |\n"),
            ("readme.txt", "hi"),
            ("setlists/deep/x.md", "x"),
            ("setlists/notes.txt", "x"),
            ("setlists/.hidden.md", "x"),
            ("__MACOSX/._songs.md", "x"),
            ("setlists/.DS_Store", "x"),
            ("setlists/broken.md", "\u{0}"),
        ]);
        let mut library = Library::empty();
        let report = import(&mut library, &archive, &ImportOptions::default()).unwrap();
        assert_eq!(titles(&library), ["A"]);
        let reasons: Vec<&str> = report.ignored.iter().map(String::as_str).collect();
        assert!(
            reasons.contains(&"readme.txt: not part of the library"),
            "{reasons:?}"
        );
        assert!(
            reasons.contains(&"setlists/deep/x.md: not part of the library"),
            "{reasons:?}"
        );
        assert!(
            reasons.contains(&"__MACOSX/._songs.md: system file"),
            "{reasons:?}"
        );
        assert!(
            reasons.contains(&"setlists/.DS_Store: system file"),
            "{reasons:?}"
        );
        assert_eq!(
            report.setlists_added,
            ["broken"],
            "a file with a NUL byte is still valid UTF-8"
        );
    }

    #[test]
    fn invalid_text_files_are_ignored() {
        let archive = zip::write_zip(&[
            ("songs.md", &[0xff, 0xfe, 0xfd][..]),
            ("setlists/a.md", b"# A\n"),
        ])
        .unwrap();
        let mut library = Library::empty();
        let report = import(&mut library, &archive, &ImportOptions::default()).unwrap();
        assert!(
            report
                .ignored
                .contains(&"songs.md: not valid text".to_string())
        );
        assert_eq!(report.setlists_added, ["a"]);
    }

    #[test]
    fn songs_already_in_the_library_are_skipped_or_overwritten() {
        let archive = zip_of(&[(
            "songs.md",
            "| Song | BPM |\n|---|---|\n| hotel california | 90 |\n| Wonderwall | 87 |\n",
        )]);
        let (_, mut library) = library_with_data();
        let skip = import(&mut library, &archive, &ImportOptions::default()).unwrap();
        assert_eq!(
            (skip.songs_added, skip.songs_skipped, skip.songs_overwritten),
            (1, 1, 0)
        );
        assert_eq!(library.song("Hotel California").unwrap().bpm, 75.0);

        let (_, mut library) = library_with_data();
        let options = ImportOptions {
            songs: SongConflict::Overwrite,
            ..ImportOptions::default()
        };
        let over = import(&mut library, &archive, &options).unwrap();
        assert_eq!(
            (over.songs_added, over.songs_skipped, over.songs_overwritten),
            (1, 0, 1)
        );
        assert_eq!(library.song("Hotel California").unwrap().bpm, 90.0);
        assert_eq!(
            library.song("Hotel California").unwrap().title,
            "hotel california"
        );
    }

    #[test]
    fn setlist_conflicts_skip_overwrite_or_keep_both() {
        let archive = zip_of(&[(
            "setlists/friday-gig.md",
            "# Imported Friday\n\n- Wonderwall\n",
        )]);
        let run = |conflict| {
            let (_, mut library) = library_with_data();
            let options = ImportOptions {
                setlists: conflict,
                ..ImportOptions::default()
            };
            let report = import(&mut library, &archive, &options).unwrap();
            (library, report)
        };
        let (library, report) = run(SetlistConflict::Skip);
        assert_eq!(report.setlists_skipped, ["friday-gig"]);
        assert_eq!(library.setlist("friday-gig").unwrap().name(), "Friday Gig");

        let (library, report) = run(SetlistConflict::Overwrite);
        assert_eq!(report.setlists_overwritten, ["friday-gig"]);
        assert_eq!(
            library.setlist("friday-gig").unwrap().name(),
            "Imported Friday"
        );

        let (library, report) = run(SetlistConflict::KeepBoth);
        assert_eq!(
            report.setlists_kept_both,
            [("friday-gig".to_string(), "friday-gig-2".to_string())]
        );
        assert_eq!(library.setlist("friday-gig").unwrap().name(), "Friday Gig");
        assert_eq!(
            library.setlist("friday-gig-2").unwrap().name(),
            "Imported Friday"
        );
        assert_eq!(
            library.missing_songs("friday-gig-2"),
            [0],
            "Wonderwall is not in this library"
        );
    }

    #[test]
    fn imported_setlist_files_are_written_exactly_as_they_were() {
        let text =
            "---\nband: B\nweird:   spacing # keep\n---\n# Gig\n\nIntro.\n\n- A\n\n- B\n\nNotes\n";
        let archive = zip_of(&[("setlists/gig.md", text)]);
        let mut library = Library::empty();
        import(&mut library, &archive, &ImportOptions::default()).unwrap();
        let storage = MemStorage::new();
        library.save(&storage).unwrap();
        assert_eq!(storage.text("setlists/gig.md").as_deref(), Some(text));
    }

    #[test]
    fn a_setlist_cannot_overwrite_an_unreadable_file() {
        let storage = MemStorage::new();
        storage.write("setlists/gig.md", &[0xff, 0xfe]).unwrap();
        let mut library = Library::open(&storage).unwrap().library;
        let archive = zip_of(&[("setlists/gig.md", "# Gig\n")]);
        let options = ImportOptions {
            setlists: SetlistConflict::Overwrite,
            ..ImportOptions::default()
        };
        let report = import(&mut library, &archive, &options).unwrap();
        assert_eq!(
            report.setlists_kept_both,
            [("gig".to_string(), "gig-2".to_string())]
        );
        library.save(&storage).unwrap();
        assert_eq!(
            storage.read("setlists/gig.md").unwrap().unwrap(),
            [0xff, 0xfe]
        );
    }

    #[test]
    fn settings_are_skipped_or_overwritten() {
        let archive = zip_of(&[(
            "settings.md",
            "---\nsound: rim\nvolume: 0.3\nextra: kept\n---\n",
        )]);
        let mut library = Library::empty();
        let skipped = import(&mut library, &archive, &ImportOptions::default()).unwrap();
        assert!(!skipped.settings_imported);
        assert_eq!(library.settings().volume, 0.8);

        let options = ImportOptions {
            settings: SettingsConflict::Overwrite,
            ..ImportOptions::default()
        };
        let done = import(&mut library, &archive, &options).unwrap();
        assert!(done.settings_imported);
        assert_eq!(library.settings().volume, 0.3);
        let storage = MemStorage::new();
        library.save(&storage).unwrap();
        assert!(storage.text("settings.md").unwrap().contains("extra: kept"));
    }

    #[test]
    fn settings_from_a_newer_version_do_not_stop_the_rest_of_the_import() {
        let storage = MemStorage::new().with("settings.md", "---\nformat: 9\n---\n");
        let mut library = Library::open(&storage).unwrap().library;
        let archive = zip_of(&[
            ("settings.md", "---\nsound: rim\n---\n"),
            ("songs.md", "| Song | BPM |\n|---|---|\n| A | 90 |\n"),
        ]);
        let options = ImportOptions {
            settings: SettingsConflict::Overwrite,
            ..ImportOptions::default()
        };
        let report = import(&mut library, &archive, &options).unwrap();
        assert!(!report.settings_imported);
        assert!(
            report
                .warnings
                .iter()
                .any(|w| w.problem.message.contains("not imported"))
        );
        assert_eq!(titles(&library), ["A"]);
    }

    #[test]
    fn an_unreadable_songs_file_stops_a_songs_import_before_anything_changes() {
        let storage = MemStorage::new();
        storage.write("songs.md", &[0xff, 0xfe]).unwrap();
        let mut library = Library::open(&storage).unwrap().library;
        let archive = zip_of(&[
            ("songs.md", "| Song | BPM |\n|---|---|\n| A | 90 |\n"),
            ("setlists/x.md", "# X\n"),
        ]);
        let result = import(&mut library, &archive, &ImportOptions::default());
        assert!(matches!(
            result,
            Err(ArchiveError::Library(LibraryError::FileUnreadable(_)))
        ));
        assert!(
            library.setlists().is_empty(),
            "the setlist must not have been imported either"
        );
    }

    #[test]
    fn problems_in_imported_files_are_reported_with_the_archive_path() {
        let archive = zip_of(&[("songs.md", "| Song | BPM |\n|---|---|\n| A | fast |\n")]);
        let mut library = Library::empty();
        let report = import(&mut library, &archive, &ImportOptions::default()).unwrap();
        assert_eq!(report.warnings.len(), 1);
        assert_eq!(report.warnings[0].file, "songs.md");
    }

    #[test]
    fn archives_beyond_the_limits_are_refused() {
        let big = "x".repeat((1 << 20) + 1);
        let archive = zip_of(&[("setlists/a.md", &big)]);
        assert_eq!(
            import(&mut Library::empty(), &archive, &ImportOptions::default()),
            Err(ArchiveError::Zip(ZipError::TooLarge))
        );
        let many: Vec<(String, String)> = (0..1_001)
            .map(|i| (format!("setlists/s{i}.md"), "#".to_string()))
            .collect();
        let refs: Vec<(&str, &[u8])> = many
            .iter()
            .map(|(n, d)| (n.as_str(), d.as_bytes()))
            .collect();
        let archive = zip::write_zip(&refs).unwrap();
        assert_eq!(
            import(&mut Library::empty(), &archive, &ImportOptions::default()),
            Err(ArchiveError::Zip(ZipError::TooManyEntries))
        );
    }

    #[test]
    fn something_that_is_not_a_zip_is_refused() {
        assert_eq!(
            import(
                &mut Library::empty(),
                b"just text",
                &ImportOptions::default()
            ),
            Err(ArchiveError::Zip(ZipError::NotAZip))
        );
    }

    #[test]
    fn importing_the_same_archive_twice_adds_nothing_new_except_a_second_copy_of_setlists() {
        let (storage, _) = library_with_data();
        let archive = export(&storage).unwrap();
        let mut library = Library::empty();
        import(&mut library, &archive, &ImportOptions::default()).unwrap();
        let again = import(&mut library, &archive, &ImportOptions::default()).unwrap();
        assert_eq!((again.songs_added, again.songs_skipped), (0, 2));
        assert_eq!(library.setlists().len(), 2);
        let skip = ImportOptions {
            setlists: SetlistConflict::Skip,
            ..ImportOptions::default()
        };
        import(&mut library, &archive, &skip).unwrap();
        assert_eq!(library.setlists().len(), 2);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(48))]

        #[test]
        fn any_library_survives_export_and_import(
            songs in proptest::collection::vec(("[A-Za-z0-9 |'éøå.-]{1,16}", 30u32..=300, 1u32..=20, "[A-Za-z0-9 ,.-]{0,12}"), 0..8),
            sets in proptest::collection::vec(("[A-Za-z0-9 é-]{1,12}", proptest::option::of("[A-Za-z ]{1,8}"), proptest::collection::vec(0usize..8, 0..6)), 0..5),
        ) {
            let storage = MemStorage::new();
            let mut source = Library::empty();
            for (title, bpm, beats, notes) in &songs {
                let _ = source.add_song(Song::new(title, f64::from(*bpm), *beats).with_notes(notes));
            }
            for (name, band, picks) in &sets {
                if let Ok(stem) = source.create_setlist(name, band.as_deref()) {
                    for &p in picks {
                        let titles = titles(&source);
                        if !titles.is_empty() {
                            source.add_song_to_setlist(&stem, &titles[p % titles.len()]).unwrap();
                        }
                    }
                }
            }
            source.save(&storage).unwrap();
            let archive = export(&storage).unwrap();

            let mut target = Library::empty();
            let report = import(&mut target, &archive, &ImportOptions::default()).unwrap();
            prop_assert!(report.ignored.is_empty(), "{:?}", report.ignored);
            let copy = MemStorage::new();
            target.save(&copy).unwrap();
            for path in storage.paths() {
                prop_assert_eq!(copy.text(&path), storage.text(&path), "{}", path);
            }
            prop_assert_eq!(copy.paths(), storage.paths());
        }
    }
}
