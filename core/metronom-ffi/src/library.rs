//! The song library as the app sees it: one `SongLibrary` object over a folder on the phone, plus
//! plain data records.
//!
//! **Every change is saved immediately** (write-through), so the app cannot forget to save. The
//! calls do file I/O, so call them off the UI thread. All logic lives in `metronom_core::library`.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use metronom_core::library::archive::{self, ImportOptions, ImportReport};
use metronom_core::library::settings::AppSettings;
use metronom_core::library::songs::Song;
use metronom_core::library::storage::FsStorage;
use metronom_core::library::{Library as CoreLibrary, LibraryError as CoreError, Warning};

use crate::Sound;

/// Field names must not be `message` or `cause`: the generated Kotlin exception inherits those.
#[derive(Debug, PartialEq, Eq, uniffi::Error)]
pub enum LibraryError {
    /// Another song already has this title (ignoring case and surrounding spaces).
    DuplicateTitle {
        title: String,
    },
    /// A song title or setlist name must not be empty.
    EmptyName,
    NoSuchSong {
        title: String,
    },
    NoSuchSetlist {
        stem: String,
    },
    PositionOutOfRange,
    /// The file is not valid text; it is left untouched and cannot be changed by the app.
    FileUnreadable {
        file: String,
    },
    /// The file was written by a newer version of the app and is not changed.
    ReadOnly {
        file: String,
    },
    Storage {
        detail: String,
    },
    /// The zip file cannot be imported (damaged, too large, unsafe paths, not a zip).
    BadArchive {
        detail: String,
    },
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTitle { title } => {
                write!(f, "there is already a song called \"{title}\"")
            }
            Self::EmptyName => write!(f, "the name is empty"),
            Self::NoSuchSong { title } => write!(f, "there is no song called \"{title}\""),
            Self::NoSuchSetlist { stem } => write!(f, "there is no setlist \"{stem}\""),
            Self::PositionOutOfRange => write!(f, "that position is outside the setlist"),
            Self::FileUnreadable { file } => {
                write!(f, "{file} is not valid text and is left untouched")
            }
            Self::ReadOnly { file } => write!(
                f,
                "{file} was written by a newer version and is not changed"
            ),
            Self::Storage { detail } => write!(f, "could not read or write the library: {detail}"),
            Self::BadArchive { detail } => write!(f, "{detail}"),
        }
    }
}

impl std::error::Error for LibraryError {}

impl From<CoreError> for LibraryError {
    fn from(e: CoreError) -> Self {
        match e {
            CoreError::DuplicateTitle(title) => Self::DuplicateTitle { title },
            CoreError::EmptyName => Self::EmptyName,
            CoreError::NoSuchSong(title) => Self::NoSuchSong { title },
            CoreError::NoSuchSetlist(stem) => Self::NoSuchSetlist { stem },
            CoreError::PositionOutOfRange => Self::PositionOutOfRange,
            CoreError::FileUnreadable(file) => Self::FileUnreadable { file },
            CoreError::ReadOnly(file) => Self::ReadOnly { file },
            CoreError::Storage(detail) => Self::Storage { detail },
        }
    }
}

impl From<archive::ArchiveError> for LibraryError {
    fn from(e: archive::ArchiveError) -> Self {
        match e {
            archive::ArchiveError::Library(inner) => inner.into(),
            archive::ArchiveError::Storage(detail) => Self::Storage { detail },
            other => Self::BadArchive {
                detail: other.to_string(),
            },
        }
    }
}

/// One song: a title and how to click it.
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct SongRecord {
    pub title: String,
    /// 30–300; may have decimals.
    pub bpm: f64,
    /// Beats per bar, 1–99.
    pub beats: u32,
    pub notes: String,
}

impl From<&Song> for SongRecord {
    fn from(song: &Song) -> Self {
        Self {
            title: song.title.clone(),
            bpm: song.bpm,
            beats: song.beats,
            notes: song.notes.clone(),
        }
    }
}

impl From<SongRecord> for Song {
    fn from(record: SongRecord) -> Self {
        Song::new(&record.title, record.bpm, record.beats).with_notes(&record.notes)
    }
}

/// How often a setlist uses a song.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SetlistUseRecord {
    pub stem: String,
    pub name: String,
    pub count: u32,
}

/// A setlist in a list of setlists.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct SetlistSummary {
    /// The file name without `.md`: the setlist's identity.
    pub stem: String,
    pub name: String,
    pub band: Option<String>,
    pub song_count: u32,
    /// Songs the setlist names that do not exist in the library.
    pub missing_count: u32,
}

/// Setlists of one band; `band` is `None` for the setlists without one.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct BandGroup {
    pub band: Option<String>,
    pub setlists: Vec<SetlistSummary>,
}

/// One line of a setlist: the title as written and the song it refers to (`None` = missing).
#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct EntryRecord {
    pub title: String,
    pub song: Option<SongRecord>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct SetlistDetail {
    pub stem: String,
    pub name: String,
    pub band: Option<String>,
    pub entries: Vec<EntryRecord>,
}

/// Something odd found in a file while reading it.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct WarningRecord {
    pub file: String,
    pub line: Option<u32>,
    pub detail: String,
}

impl From<&Warning> for WarningRecord {
    fn from(w: &Warning) -> Self {
        Self {
            file: w.file.clone(),
            line: w.problem.line.and_then(|l| u32::try_from(l).ok()),
            detail: w.problem.message.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct SettingsRecord {
    pub sound: Sound,
    pub volume: f32,
    pub keep_screen_on: bool,
    pub visual_offset_ms: i32,
    pub last_setlist: Option<String>,
}

impl From<&AppSettings> for SettingsRecord {
    fn from(s: &AppSettings) -> Self {
        Self {
            sound: s.sound.into(),
            volume: s.volume,
            keep_screen_on: s.keep_screen_on,
            visual_offset_ms: s.visual_offset_ms,
            last_setlist: s.last_setlist.clone(),
        }
    }
}

impl From<SettingsRecord> for AppSettings {
    fn from(r: SettingsRecord) -> Self {
        Self {
            sound: r.sound.into(),
            volume: r.volume,
            keep_screen_on: r.keep_screen_on,
            visual_offset_ms: r.visual_offset_ms,
            last_setlist: r.last_setlist,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SettingsConflict {
    Skip,
    Overwrite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SetlistConflict {
    Skip,
    Overwrite,
    KeepBoth,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum SongConflict {
    SkipExisting,
    Overwrite,
}

/// What to do when the archive has something the library already has.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Record)]
pub struct ImportOptionsRecord {
    pub settings: SettingsConflict,
    pub setlists: SetlistConflict,
    pub songs: SongConflict,
}

impl From<ImportOptionsRecord> for ImportOptions {
    fn from(o: ImportOptionsRecord) -> Self {
        use archive as a;
        Self {
            settings: match o.settings {
                SettingsConflict::Skip => a::SettingsConflict::Skip,
                SettingsConflict::Overwrite => a::SettingsConflict::Overwrite,
            },
            setlists: match o.setlists {
                SetlistConflict::Skip => a::SetlistConflict::Skip,
                SetlistConflict::Overwrite => a::SetlistConflict::Overwrite,
                SetlistConflict::KeepBoth => a::SetlistConflict::KeepBoth,
            },
            songs: match o.songs {
                SongConflict::SkipExisting => a::SongConflict::SkipExisting,
                SongConflict::Overwrite => a::SongConflict::Overwrite,
            },
        }
    }
}

/// A setlist imported under a new file name because its own was taken.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct RenamedRecord {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct ImportReportRecord {
    pub songs_added: u32,
    pub songs_overwritten: u32,
    pub songs_skipped: u32,
    pub setlists_added: Vec<String>,
    pub setlists_overwritten: Vec<String>,
    pub setlists_skipped: Vec<String>,
    pub setlists_kept_both: Vec<RenamedRecord>,
    pub settings_imported: bool,
    /// Entries of the zip that are not part of the library, with the reason.
    pub ignored: Vec<String>,
    pub warnings: Vec<WarningRecord>,
}

impl From<&ImportReport> for ImportReportRecord {
    fn from(r: &ImportReport) -> Self {
        let n = |v: usize| u32::try_from(v).unwrap_or(u32::MAX);
        Self {
            songs_added: n(r.songs_added),
            songs_overwritten: n(r.songs_overwritten),
            songs_skipped: n(r.songs_skipped),
            setlists_added: r.setlists_added.clone(),
            setlists_overwritten: r.setlists_overwritten.clone(),
            setlists_skipped: r.setlists_skipped.clone(),
            setlists_kept_both: r
                .setlists_kept_both
                .iter()
                .map(|(from, to)| RenamedRecord {
                    from: from.clone(),
                    to: to.clone(),
                })
                .collect(),
            settings_imported: r.settings_imported,
            ignored: r.ignored.clone(),
            warnings: r.warnings.iter().map(WarningRecord::from).collect(),
        }
    }
}

/// The songs, setlists and settings in one folder.
#[derive(uniffi::Object)]
pub struct SongLibrary {
    inner: Mutex<CoreLibrary>,
    storage: FsStorage,
    warnings: Vec<WarningRecord>,
}

impl SongLibrary {
    fn lock(&self) -> MutexGuard<'_, CoreLibrary> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Change the library and save it at once.
    fn change<T>(
        &self,
        f: impl FnOnce(&mut CoreLibrary) -> Result<T, CoreError>,
    ) -> Result<T, LibraryError> {
        let mut library = self.lock();
        let value = f(&mut library)?;
        library.save(&self.storage)?;
        Ok(value)
    }
}

#[uniffi::export]
impl SongLibrary {
    /// Open the library in the folder `root` (created when something is first saved). Odd
    /// content in the files is not an error: see [`SongLibrary::warnings`].
    #[uniffi::constructor]
    pub fn open(root: String) -> Result<Arc<Self>, LibraryError> {
        let storage = FsStorage::new(root);
        let opened = CoreLibrary::open(&storage)?;
        Ok(Arc::new(Self {
            warnings: opened.warnings.iter().map(WarningRecord::from).collect(),
            inner: Mutex::new(opened.library),
            storage,
        }))
    }

    /// What was odd about the files when the library was opened.
    pub fn warnings(&self) -> Vec<WarningRecord> {
        self.warnings.clone()
    }

    // ---- songs --------------------------------------------------------------------------------

    /// Every song, in file order.
    pub fn songs(&self) -> Vec<SongRecord> {
        self.lock()
            .songs()
            .into_iter()
            .map(SongRecord::from)
            .collect()
    }

    pub fn song(&self, title: String) -> Option<SongRecord> {
        self.lock().song(&title).map(SongRecord::from)
    }

    pub fn add_song(&self, song: SongRecord) -> Result<(), LibraryError> {
        self.change(|l| l.add_song(song.into()))
    }

    /// Replace the song titled `title`. If its title changes, every setlist is updated.
    pub fn update_song(&self, title: String, song: SongRecord) -> Result<(), LibraryError> {
        self.change(|l| l.update_song(&title, song.into()))
    }

    /// The setlists that use a song, e.g. to say what deleting it will affect.
    pub fn setlists_using(&self, title: String) -> Vec<SetlistUseRecord> {
        use_records(self.lock().setlists_using(&title))
    }

    /// Delete a song and remove it from every setlist; returns where it was removed.
    pub fn delete_song(&self, title: String) -> Result<Vec<SetlistUseRecord>, LibraryError> {
        self.change(|l| l.delete_song(&title)).map(use_records)
    }

    // ---- setlists -----------------------------------------------------------------------------

    /// The bands and projects in use, alphabetical.
    pub fn bands(&self) -> Vec<String> {
        self.lock().bands()
    }

    /// Setlists grouped by band (alphabetical), each group sorted by name; setlists without a
    /// band come last with `band == None`.
    pub fn setlists_by_band(&self) -> Vec<BandGroup> {
        let library = self.lock();
        library
            .setlists_by_band()
            .into_iter()
            .map(|(band, setlists)| BandGroup {
                band,
                setlists: setlists
                    .into_iter()
                    .map(|s| SetlistSummary {
                        stem: s.stem().to_string(),
                        name: s.name(),
                        band: s.band().map(str::to_string),
                        song_count: u32::try_from(s.songs().len()).unwrap_or(u32::MAX),
                        missing_count: u32::try_from(library.missing_songs(s.stem()).len())
                            .unwrap_or(u32::MAX),
                    })
                    .collect(),
            })
            .collect()
    }

    pub fn setlist(&self, stem: String) -> Option<SetlistDetail> {
        let library = self.lock();
        let setlist = library.setlist(&stem)?;
        Some(SetlistDetail {
            stem: setlist.stem().to_string(),
            name: setlist.name(),
            band: setlist.band().map(str::to_string),
            entries: library
                .entries(&stem)?
                .into_iter()
                .map(|e| EntryRecord {
                    title: e.title.to_string(),
                    song: e.song.map(SongRecord::from),
                })
                .collect(),
        })
    }

    /// Create a setlist; returns its file name (its identity).
    pub fn create_setlist(
        &self,
        name: String,
        band: Option<String>,
    ) -> Result<String, LibraryError> {
        self.change(|l| l.create_setlist(&name, band.as_deref()))
    }

    /// Change the setlist's name. Its file name stays the same.
    pub fn rename_setlist(&self, stem: String, name: String) -> Result<(), LibraryError> {
        self.change(|l| l.rename_setlist(&stem, &name))
    }

    /// `None` removes the band.
    pub fn set_setlist_band(&self, stem: String, band: Option<String>) -> Result<(), LibraryError> {
        self.change(|l| l.set_setlist_band(&stem, band.as_deref()))
    }

    /// Add an existing song at the end of a setlist.
    pub fn add_song_to_setlist(&self, stem: String, title: String) -> Result<(), LibraryError> {
        self.change(|l| l.add_song_to_setlist(&stem, &title))
    }

    pub fn remove_song_from_setlist(&self, stem: String, index: u32) -> Result<(), LibraryError> {
        self.change(|l| l.remove_song_from_setlist(&stem, index as usize))
    }

    /// Move the song at `from` so that it ends up at `to`.
    pub fn move_song_in_setlist(
        &self,
        stem: String,
        from: u32,
        to: u32,
    ) -> Result<(), LibraryError> {
        self.change(|l| l.move_song_in_setlist(&stem, from as usize, to as usize))
    }

    /// Copy a setlist as "<name> (copy)"; returns the new file name.
    pub fn copy_setlist(&self, stem: String) -> Result<String, LibraryError> {
        self.change(|l| l.copy_setlist(&stem))
    }

    pub fn delete_setlist(&self, stem: String) -> Result<(), LibraryError> {
        self.change(|l| l.delete_setlist(&stem))
    }

    // ---- settings -----------------------------------------------------------------------------

    pub fn settings(&self) -> SettingsRecord {
        SettingsRecord::from(self.lock().settings())
    }

    pub fn update_settings(&self, settings: SettingsRecord) -> Result<(), LibraryError> {
        self.change(|l| l.update_settings(settings.into()))
    }

    // ---- export and import --------------------------------------------------------------------

    /// The whole library as a zip file's bytes.
    pub fn export_archive(&self) -> Result<Vec<u8>, LibraryError> {
        let mut library = self.lock();
        library.save(&self.storage)?;
        Ok(archive::export(&self.storage)?)
    }

    /// Merge a zip file into the library. Nothing changes if the archive is refused.
    pub fn import_archive(
        &self,
        archive_bytes: Vec<u8>,
        options: ImportOptionsRecord,
    ) -> Result<ImportReportRecord, LibraryError> {
        let mut library = self.lock();
        let report = archive::import(&mut library, &archive_bytes, &options.into())?;
        library.save(&self.storage)?;
        Ok(ImportReportRecord::from(&report))
    }
}

fn use_records(uses: Vec<metronom_core::library::SetlistUse>) -> Vec<SetlistUseRecord> {
    uses.into_iter()
        .map(|u| SetlistUseRecord {
            stem: u.stem,
            name: u.name,
            count: u32::try_from(u.count).unwrap_or(u32::MAX),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn song(title: &str, bpm: f64, beats: u32) -> SongRecord {
        SongRecord {
            title: title.to_string(),
            bpm,
            beats,
            notes: String::new(),
        }
    }

    fn open(dir: &tempfile::TempDir) -> Arc<SongLibrary> {
        SongLibrary::open(dir.path().to_str().unwrap().to_string()).unwrap()
    }

    #[test]
    fn an_empty_folder_is_an_empty_library() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        assert!(library.songs().is_empty());
        assert!(library.setlists_by_band().is_empty());
        assert!(library.warnings().is_empty());
        assert_eq!(library.settings().sound, Sound::Click);
    }

    #[test]
    fn a_folder_that_does_not_exist_yet_is_created_on_the_first_change() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("not").join("yet");
        let library = SongLibrary::open(root.to_str().unwrap().to_string()).unwrap();
        library.add_song(song("A", 90.0, 4)).unwrap();
        assert!(root.join("songs.md").exists());
    }

    #[test]
    fn changes_are_saved_at_once_and_survive_reopening() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        library
            .add_song(SongRecord {
                notes: "Bm, capo 7".to_string(),
                ..song("Hotel California", 75.0, 4)
            })
            .unwrap();
        // No explicit save: the file is already there.
        let text = std::fs::read_to_string(dir.path().join("songs.md")).unwrap();
        assert!(
            text.contains("Hotel California") && text.contains("Bm, capo 7"),
            "{text}"
        );

        let again = open(&dir);
        assert_eq!(again.songs(), library.songs());
        assert_eq!(
            again.song("hotel CALIFORNIA".to_string()).unwrap().bpm,
            75.0
        );
        assert_eq!(again.song("Nope".to_string()), None);
    }

    #[test]
    fn errors_carry_what_the_ui_needs() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        library.add_song(song("A", 90.0, 4)).unwrap();
        assert_eq!(
            library.add_song(song(" a ", 90.0, 4)),
            Err(LibraryError::DuplicateTitle {
                title: "a".to_string()
            })
        );
        assert_eq!(
            library.add_song(song("  ", 90.0, 4)),
            Err(LibraryError::EmptyName)
        );
        assert_eq!(
            library.update_song("Nope".to_string(), song("X", 90.0, 4)),
            Err(LibraryError::NoSuchSong {
                title: "Nope".to_string()
            })
        );
        assert_eq!(
            library.delete_song("Nope".to_string()),
            Err(LibraryError::NoSuchSong {
                title: "Nope".to_string()
            })
        );
        assert_eq!(
            library.rename_setlist("nope".to_string(), "x".to_string()),
            Err(LibraryError::NoSuchSetlist {
                stem: "nope".to_string()
            })
        );
        assert!(LibraryError::EmptyName.to_string().contains("empty"));
    }

    #[test]
    fn values_are_made_safe_on_the_way_in() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        library.add_song(song("Fast\nSong", 9_999.0, 0)).unwrap();
        let s = &library.songs()[0];
        assert_eq!((s.title.as_str(), s.bpm, s.beats), ("Fast Song", 300.0, 1));
    }

    #[test]
    fn a_full_setlist_session_persists() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        library.add_song(song("Hotel California", 75.0, 4)).unwrap();
        library.add_song(song("Waltz for Debby", 132.0, 3)).unwrap();
        let stem = library
            .create_setlist("Friday Gig".to_string(), Some("The Band".to_string()))
            .unwrap();
        assert_eq!(stem, "friday-gig");
        library
            .add_song_to_setlist(stem.clone(), "waltz for debby".to_string())
            .unwrap();
        library
            .add_song_to_setlist(stem.clone(), "Hotel California".to_string())
            .unwrap();
        library.move_song_in_setlist(stem.clone(), 1, 0).unwrap();
        let copy = library.copy_setlist(stem.clone()).unwrap();
        library.set_setlist_band(copy.clone(), None).unwrap();
        library
            .rename_setlist(stem.clone(), "Saturday Gig".to_string())
            .unwrap();

        let again = open(&dir);
        let detail = again.setlist(stem.clone()).unwrap();
        assert_eq!(
            (detail.name.as_str(), detail.band.as_deref()),
            ("Saturday Gig", Some("The Band"))
        );
        let titles: Vec<&str> = detail.entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, ["Hotel California", "Waltz for Debby"]);
        assert_eq!(detail.entries[1].song.as_ref().unwrap().beats, 3);

        let groups = again.setlists_by_band();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].band.as_deref(), Some("The Band"));
        assert_eq!(groups[0].setlists[0].song_count, 2);
        assert_eq!(groups[1].band, None);
        assert_eq!(groups[1].setlists[0].name, "Friday Gig (copy)");
        assert_eq!(again.bands(), ["The Band"]);
        assert_eq!(again.setlist("nope".to_string()), None);

        again.delete_setlist(copy).unwrap();
        assert_eq!(open(&dir).setlists_by_band().len(), 1);
    }

    #[test]
    fn renaming_and_deleting_a_song_reach_the_setlists() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        library.add_song(song("A", 90.0, 4)).unwrap();
        library.add_song(song("B", 100.0, 4)).unwrap();
        let stem = library.create_setlist("Gig".to_string(), None).unwrap();
        for t in ["A", "B", "A"] {
            library
                .add_song_to_setlist(stem.clone(), t.to_string())
                .unwrap();
        }
        assert_eq!(
            library.setlists_using("a".to_string()),
            vec![SetlistUseRecord {
                stem: stem.clone(),
                name: "Gig".to_string(),
                count: 2
            }]
        );
        library
            .update_song("A".to_string(), song("Alpha", 91.0, 4))
            .unwrap();
        let titles = |l: &SongLibrary| -> Vec<String> {
            l.setlist(stem.clone())
                .unwrap()
                .entries
                .iter()
                .map(|e| e.title.clone())
                .collect()
        };
        assert_eq!(titles(&library), ["Alpha", "B", "Alpha"]);
        let removed = library.delete_song("Alpha".to_string()).unwrap();
        assert_eq!(removed[0].count, 2);
        assert_eq!(titles(&open(&dir)), ["B"]);
    }

    #[test]
    fn missing_songs_are_counted_and_visible() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("setlists")).unwrap();
        std::fs::write(dir.path().join("setlists/x.md"), "# X\n\n- Nowhere Man\n").unwrap();
        let library = open(&dir);
        let summary = &library.setlists_by_band()[0].setlists[0];
        assert_eq!((summary.song_count, summary.missing_count), (1, 1));
        assert_eq!(
            library.setlist("x".to_string()).unwrap().entries[0].song,
            None
        );
    }

    #[test]
    fn settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        let mut s = library.settings();
        s.sound = Sound::Rim;
        s.volume = 0.4;
        s.keep_screen_on = false;
        s.last_setlist = Some("friday-gig".to_string());
        library.update_settings(s.clone()).unwrap();
        assert_eq!(open(&dir).settings(), s);
    }

    #[test]
    fn export_and_import_move_a_library_between_folders() {
        let source = tempfile::tempdir().unwrap();
        let from = open(&source);
        from.add_song(song("A", 90.0, 4)).unwrap();
        let stem = from
            .create_setlist("Gig".to_string(), Some("Band".to_string()))
            .unwrap();
        from.add_song_to_setlist(stem, "A".to_string()).unwrap();
        let archive = from.export_archive().unwrap();

        let target = tempfile::tempdir().unwrap();
        let to = open(&target);
        let options = ImportOptionsRecord {
            settings: SettingsConflict::Skip,
            setlists: SetlistConflict::KeepBoth,
            songs: SongConflict::SkipExisting,
        };
        let report = to.import_archive(archive.clone(), options).unwrap();
        assert_eq!(
            (report.songs_added, report.setlists_added.clone()),
            (1, vec!["gig".to_string()])
        );
        assert_eq!(open(&target).songs(), from.songs(), "and it was saved");

        let again = to.import_archive(archive, options).unwrap();
        assert_eq!(
            (again.songs_skipped, again.setlists_kept_both),
            (
                1,
                vec![RenamedRecord {
                    from: "gig".into(),
                    to: "gig-2".into()
                }]
            )
        );
    }

    #[test]
    fn a_bad_archive_is_a_clear_error_and_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let library = open(&dir);
        let options = ImportOptionsRecord {
            settings: SettingsConflict::Skip,
            setlists: SetlistConflict::Skip,
            songs: SongConflict::SkipExisting,
        };
        let result = library.import_archive(b"not a zip".to_vec(), options);
        assert!(
            matches!(result, Err(LibraryError::BadArchive { .. })),
            "{result:?}"
        );
        assert!(library.songs().is_empty());
    }

    #[test]
    fn an_unreadable_songs_file_is_reported_and_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("songs.md"), [0xff, 0xfe, 0x00]).unwrap();
        let library = open(&dir);
        assert_eq!(library.warnings()[0].file, "songs.md");
        assert_eq!(
            library.add_song(song("A", 90.0, 4)),
            Err(LibraryError::FileUnreadable {
                file: "songs.md".to_string()
            })
        );
        assert_eq!(
            std::fs::read(dir.path().join("songs.md")).unwrap(),
            [0xff, 0xfe, 0x00]
        );
    }

    #[test]
    fn warnings_describe_odd_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("songs.md"),
            "| Song | BPM |\n|---|---|\n| A | fast |\n",
        )
        .unwrap();
        let w = &open(&dir).warnings()[0];
        assert_eq!((w.file.as_str(), w.line), ("songs.md", Some(3)));
        assert!(w.detail.contains("not a tempo"), "{}", w.detail);
    }
}
