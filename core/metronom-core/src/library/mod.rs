//! The song library: settings, songs and setlists stored as markdown files.
//!
//! See `docs/file-format.md` for the format and `docs/design.md` for what the app does with it.
//! Everything here is pure Rust with no OS dependencies: files are reached through the
//! [`storage::Storage`] trait.
//!
//! A [`Library`] is opened from storage, changed in memory, and saved back. Saving writes only
//! the files that changed, and each file is rewritten so that everything the app does not own
//! (unknown header keys, extra table columns, notes, comments) stays exactly as it was.

pub mod archive;
pub mod header;
pub mod setlist;
pub mod settings;
pub mod songs;
pub mod storage;
pub mod text;
pub mod zip;

use std::fmt;

use setlist::SetlistDoc;
use settings::{AppSettings, SettingsDoc};
use songs::{Song, SongsDoc, SongsError};
use storage::Storage;
use text::{normalize, single_line, slugify, title_key, unique_slug};

const SETTINGS_PATH: &str = "settings.md";
const SONGS_PATH: &str = "songs.md";
const SETLISTS_DIR: &str = "setlists";

fn setlist_path(stem: &str) -> String {
    format!("{SETLISTS_DIR}/{stem}.md")
}

/// Something wrong (or merely odd) in a file, reported without stopping the load.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// 1-based line in the file, when known.
    pub line: Option<usize>,
    pub message: String,
}

impl Problem {
    pub fn at(line: usize, message: String) -> Self {
        Self {
            line: Some(line),
            message,
        }
    }

    pub fn general(message: String) -> Self {
        Self {
            line: None,
            message,
        }
    }
}

/// A [`Problem`] and the file it was found in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Warning {
    pub file: String,
    pub problem: Problem,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LibraryError {
    /// Another song already has this title (ignoring case and surrounding spaces).
    DuplicateTitle(String),
    /// A song title or setlist name must not be empty.
    EmptyName,
    NoSuchSong(String),
    NoSuchSetlist(String),
    PositionOutOfRange,
    /// The file is not valid text, so it is left untouched and cannot be changed by the app.
    FileUnreadable(String),
    /// The file was written by a newer version of the app, so it is not changed.
    ReadOnly(String),
    Storage(String),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateTitle(t) => write!(f, "there is already a song called \"{t}\""),
            Self::EmptyName => write!(f, "the name is empty"),
            Self::NoSuchSong(t) => write!(f, "there is no song called \"{t}\""),
            Self::NoSuchSetlist(s) => write!(f, "there is no setlist \"{s}\""),
            Self::PositionOutOfRange => write!(f, "that position is outside the setlist"),
            Self::FileUnreadable(p) => {
                write!(
                    f,
                    "{p} is not valid text; it is left untouched and cannot be changed"
                )
            }
            Self::ReadOnly(p) => write!(f, "{p} was written by a newer version and is not changed"),
            Self::Storage(e) => write!(f, "could not read or write the library: {e}"),
        }
    }
}

impl std::error::Error for LibraryError {}

fn io(e: &std::io::Error) -> LibraryError {
    LibraryError::Storage(e.to_string())
}

/// Where a setlist was changed because of a song, and how many entries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetlistUse {
    pub stem: String,
    pub name: String,
    pub count: usize,
}

/// One line of a setlist and the song it refers to, if that song exists.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry<'a> {
    /// The title as written in the setlist.
    pub title: &'a str,
    /// `None` is a missing song.
    pub song: Option<&'a Song>,
}

/// A freshly opened library and what was odd about its files.
pub struct Opened {
    pub library: Library,
    pub warnings: Vec<Warning>,
}

pub struct Library {
    settings: SettingsDoc,
    songs: SongsDoc,
    setlists: Vec<SetlistDoc>,
    /// Setlists deleted in the app whose files are removed at the next save.
    removed: Vec<String>,
    /// Setlist files that are not valid text; they are left alone.
    unreadable_setlists: Vec<String>,
    settings_unreadable: bool,
    songs_unreadable: bool,
}

enum Text {
    Missing,
    Unreadable,
    Content(String),
}

fn read_text(storage: &dyn Storage, path: &str) -> Result<Text, LibraryError> {
    match storage.read(path).map_err(|e| io(&e))? {
        None => Ok(Text::Missing),
        Some(bytes) => Ok(match String::from_utf8(bytes) {
            Ok(text) => Text::Content(normalize(&text)),
            Err(_) => Text::Unreadable,
        }),
    }
}

impl Library {
    /// An empty library, as on first launch.
    pub fn empty() -> Self {
        Self {
            settings: SettingsDoc::new(),
            songs: SongsDoc::new(),
            setlists: Vec::new(),
            removed: Vec::new(),
            unreadable_setlists: Vec::new(),
            settings_unreadable: false,
            songs_unreadable: false,
        }
    }

    /// Read the library from storage. Missing files mean an empty library; odd content is
    /// reported in `warnings` and never stops the load. Only a storage failure is an error.
    pub fn open(storage: &dyn Storage) -> Result<Opened, LibraryError> {
        let mut library = Self::empty();
        let mut warnings = Vec::new();
        let mut report = |file: &str, problems: Vec<Problem>| {
            warnings.extend(problems.into_iter().map(|problem| Warning {
                file: file.to_string(),
                problem,
            }));
        };

        match read_text(storage, SETTINGS_PATH)? {
            Text::Missing => {}
            Text::Unreadable => {
                library.settings_unreadable = true;
                report(
                    SETTINGS_PATH,
                    vec![Problem::general(
                        "this file is not valid text; it is left untouched and the defaults are used".to_string(),
                    )],
                );
            }
            Text::Content(text) => {
                let mut problems = Vec::new();
                library.settings = SettingsDoc::parse(&text, &mut problems);
                report(SETTINGS_PATH, problems);
            }
        }
        match read_text(storage, SONGS_PATH)? {
            Text::Missing => {}
            Text::Unreadable => {
                library.songs_unreadable = true;
                report(
                    SONGS_PATH,
                    vec![Problem::general(
                        "this file is not valid text; it is left untouched, so no songs are shown"
                            .to_string(),
                    )],
                );
            }
            Text::Content(text) => {
                let mut problems = Vec::new();
                library.songs = SongsDoc::parse(&text, &mut problems);
                report(SONGS_PATH, problems);
            }
        }
        for name in storage.list(SETLISTS_DIR).map_err(|e| io(&e))? {
            let Some(stem) = name.strip_suffix(".md") else {
                continue;
            };
            if stem.is_empty() || stem.starts_with('.') {
                continue;
            }
            let path = setlist_path(stem);
            match read_text(storage, &path)? {
                Text::Missing => {}
                Text::Unreadable => {
                    library.unreadable_setlists.push(stem.to_string());
                    report(
                        &path,
                        vec![Problem::general(
                            "this file is not valid text; it is left untouched and not shown"
                                .to_string(),
                        )],
                    );
                }
                Text::Content(text) => {
                    let mut problems = Vec::new();
                    library
                        .setlists
                        .push(SetlistDoc::parse(stem, &text, &mut problems));
                    report(&path, problems);
                }
            }
        }
        // Whatever order the storage lists files in, setlists are kept in file-name order.
        library.sort_setlists();
        Ok(Opened { library, warnings })
    }

    /// Whether anything is waiting to be written.
    pub fn is_dirty(&self) -> bool {
        self.settings.is_dirty()
            || self.songs.is_dirty()
            || !self.removed.is_empty()
            || self.setlists.iter().any(SetlistDoc::is_dirty)
    }

    /// Write what changed. Each file is written atomically; if one write fails, the files
    /// written before it stay saved, the rest stay pending, and saving again retries them.
    pub fn save(&mut self, storage: &dyn Storage) -> Result<(), LibraryError> {
        if self.songs.is_dirty() && !self.songs_unreadable {
            let text = self.songs.render();
            storage
                .write(SONGS_PATH, text.as_bytes())
                .map_err(|e| io(&e))?;
            self.songs = SongsDoc::parse(&text, &mut Vec::new());
        }
        for i in 0..self.setlists.len() {
            if self.setlists[i].is_dirty() {
                let stem = self.setlists[i].stem().to_string();
                let text = self.setlists[i].render();
                storage
                    .write(&setlist_path(&stem), text.as_bytes())
                    .map_err(|e| io(&e))?;
                self.setlists[i] = SetlistDoc::parse(&stem, &text, &mut Vec::new());
            }
        }
        if self.settings.is_dirty() && !self.settings_unreadable {
            let text = self.settings.render();
            storage
                .write(SETTINGS_PATH, text.as_bytes())
                .map_err(|e| io(&e))?;
            self.settings = SettingsDoc::parse(&text, &mut Vec::new());
        }
        while let Some(stem) = self.removed.last() {
            storage.delete(&setlist_path(stem)).map_err(|e| io(&e))?;
            self.removed.pop();
        }
        Ok(())
    }

    // ---- settings -------------------------------------------------------------------------

    pub fn settings(&self) -> &AppSettings {
        self.settings.settings()
    }

    pub fn update_settings(&mut self, settings: AppSettings) -> Result<(), LibraryError> {
        if self.settings_unreadable {
            return Err(LibraryError::FileUnreadable(SETTINGS_PATH.to_string()));
        }
        if self.settings.update(settings) {
            Ok(())
        } else {
            Err(LibraryError::ReadOnly(SETTINGS_PATH.to_string()))
        }
    }

    // ---- songs ----------------------------------------------------------------------------

    /// Every song, in file order.
    pub fn songs(&self) -> Vec<&Song> {
        self.songs.songs().collect()
    }

    pub fn song(&self, title: &str) -> Option<&Song> {
        self.songs.find(title)
    }

    pub(crate) fn songs_writable(&self) -> Result<(), LibraryError> {
        if self.songs_unreadable {
            Err(LibraryError::FileUnreadable(SONGS_PATH.to_string()))
        } else {
            Ok(())
        }
    }

    /// Import into a library that has no songs and no songs file: take the incoming file over
    /// exactly as it is, so extra columns and the text around the table survive a move to a new
    /// phone. Returns how many songs it holds, or `None` (nothing changed) when the library
    /// already has songs or text of its own, or the incoming file has no songs.
    pub(crate) fn adopt_songs_file(&mut self, text: &str) -> Option<usize> {
        if self.songs_unreadable || !self.songs.is_pristine() {
            return None;
        }
        let doc = SongsDoc::adopt(text, &mut Vec::new());
        let count = doc.songs().count();
        if count == 0 {
            return None;
        }
        self.songs = doc;
        Some(count)
    }

    pub fn add_song(&mut self, song: Song) -> Result<(), LibraryError> {
        self.songs_writable()?;
        self.songs.push(song.clone()).map_err(|e| match e {
            SongsError::DuplicateTitle => LibraryError::DuplicateTitle(single_line(&song.title)),
            SongsError::EmptyTitle => LibraryError::EmptyName,
            SongsError::NotFound => LibraryError::NoSuchSong(song.title),
        })
    }

    /// Replace the song titled `title`. Renaming it renames it in every setlist too.
    pub fn update_song(&mut self, title: &str, song: Song) -> Result<(), LibraryError> {
        self.songs_writable()?;
        let old_title = self
            .songs
            .find(title)
            .map(|s| s.title.clone())
            .ok_or_else(|| LibraryError::NoSuchSong(title.to_string()))?;
        let new_title = single_line(&song.title);
        self.songs.update(title, song).map_err(|e| match e {
            SongsError::DuplicateTitle => LibraryError::DuplicateTitle(new_title.clone()),
            SongsError::EmptyTitle => LibraryError::EmptyName,
            SongsError::NotFound => LibraryError::NoSuchSong(title.to_string()),
        })?;
        if old_title != new_title {
            for setlist in &mut self.setlists {
                setlist.rename_song(&old_title, &new_title);
            }
        }
        Ok(())
    }

    /// The setlists that use a song and how often, e.g. to warn before deleting it.
    pub fn setlists_using(&self, title: &str) -> Vec<SetlistUse> {
        self.setlists
            .iter()
            .filter_map(|s| {
                let count = s.positions_of(title).len();
                (count > 0).then(|| SetlistUse {
                    stem: s.stem().to_string(),
                    name: s.name(),
                    count,
                })
            })
            .collect()
    }

    /// Delete a song and remove it from every setlist that uses it. Returns where it was removed.
    pub fn delete_song(&mut self, title: &str) -> Result<Vec<SetlistUse>, LibraryError> {
        self.songs_writable()?;
        let song = self
            .songs
            .remove(title)
            .ok_or_else(|| LibraryError::NoSuchSong(title.to_string()))?;
        let mut removed_from = Vec::new();
        for setlist in &mut self.setlists {
            let count = setlist.remove_title(&song.title);
            if count > 0 {
                removed_from.push(SetlistUse {
                    stem: setlist.stem().to_string(),
                    name: setlist.name(),
                    count,
                });
            }
        }
        Ok(removed_from)
    }

    // ---- setlists -------------------------------------------------------------------------

    pub fn setlists(&self) -> &[SetlistDoc] {
        &self.setlists
    }

    pub fn setlist(&self, stem: &str) -> Option<&SetlistDoc> {
        self.setlists.iter().find(|s| s.stem() == stem)
    }

    /// Setlist files that are not valid text and are therefore not shown.
    pub fn unreadable_setlists(&self) -> &[String] {
        &self.unreadable_setlists
    }

    /// The bands and projects in use, alphabetical ignoring case.
    pub fn bands(&self) -> Vec<String> {
        let mut bands: Vec<String> = Vec::new();
        for band in self.setlists.iter().filter_map(SetlistDoc::band) {
            if !bands.iter().any(|b| title_key(b) == title_key(band)) {
                bands.push(band.to_string());
            }
        }
        bands.sort_by_key(|b| title_key(b));
        bands
    }

    /// Setlists grouped by band, bands alphabetical, setlists in name order within each, and
    /// setlists without a band (`None`) last.
    pub fn setlists_by_band(&self) -> Vec<(Option<String>, Vec<&SetlistDoc>)> {
        let mut groups: Vec<(Option<String>, Vec<&SetlistDoc>)> = self
            .bands()
            .into_iter()
            .map(|band| {
                let mut members: Vec<&SetlistDoc> = self
                    .setlists
                    .iter()
                    .filter(|s| s.band().is_some_and(|b| title_key(b) == title_key(&band)))
                    .collect();
                members.sort_by_key(|s| title_key(&s.name()));
                (Some(band), members)
            })
            .collect();
        let mut loose: Vec<&SetlistDoc> = self
            .setlists
            .iter()
            .filter(|s| s.band().is_none())
            .collect();
        if !loose.is_empty() {
            loose.sort_by_key(|s| title_key(&s.name()));
            groups.push((None, loose));
        }
        groups
    }

    /// Each song of a setlist with the library song it refers to (`None` if missing).
    pub fn entries(&self, stem: &str) -> Option<Vec<Entry<'_>>> {
        let setlist = self.setlist(stem)?;
        Some(
            setlist
                .songs()
                .iter()
                .map(|title| Entry {
                    title,
                    song: self.songs.find(title),
                })
                .collect(),
        )
    }

    /// Positions in a setlist whose song does not exist.
    pub fn missing_songs(&self, stem: &str) -> Vec<usize> {
        self.entries(stem)
            .map(|entries| {
                entries
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| e.song.is_none())
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Setlists are kept in file-name order, the same order they come back in after a save.
    fn sort_setlists(&mut self) {
        self.setlists.sort_by(|a, b| a.stem().cmp(b.stem()));
    }

    fn setlist_mut(&mut self, stem: &str) -> Result<&mut SetlistDoc, LibraryError> {
        self.setlists
            .iter_mut()
            .find(|s| s.stem() == stem)
            .ok_or_else(|| LibraryError::NoSuchSetlist(stem.to_string()))
    }

    /// A file name for a new setlist that clashes with no existing, unreadable or pending-
    /// deletion file.
    fn fresh_stem(&self, base: &str) -> String {
        unique_slug(&slugify(base), |candidate| {
            self.setlists.iter().any(|s| s.stem() == candidate)
                || self.unreadable_setlists.iter().any(|s| s == candidate)
                || self.removed.iter().any(|s| s == candidate)
        })
    }

    /// Create a setlist; returns its file name (without `.md`).
    pub fn create_setlist(
        &mut self,
        name: &str,
        band: Option<&str>,
    ) -> Result<String, LibraryError> {
        let name = single_line(name);
        if name.is_empty() {
            return Err(LibraryError::EmptyName);
        }
        let stem = self.fresh_stem(&name);
        self.setlists.push(SetlistDoc::new(&stem, &name, band));
        self.sort_setlists();
        Ok(stem)
    }

    /// Change a setlist's name (its heading). The file keeps its name, which is its identity.
    pub fn rename_setlist(&mut self, stem: &str, name: &str) -> Result<(), LibraryError> {
        let name = single_line(name);
        if name.is_empty() {
            return Err(LibraryError::EmptyName);
        }
        self.setlist_mut(stem)?.set_name(&name);
        Ok(())
    }

    pub fn set_setlist_band(&mut self, stem: &str, band: Option<&str>) -> Result<(), LibraryError> {
        self.setlist_mut(stem)?.set_band(band);
        Ok(())
    }

    /// Add a song (which must exist in the library) at the end of a setlist.
    pub fn add_song_to_setlist(&mut self, stem: &str, title: &str) -> Result<(), LibraryError> {
        let canonical = self
            .songs
            .find(title)
            .map(|s| s.title.clone())
            .ok_or_else(|| LibraryError::NoSuchSong(title.to_string()))?;
        self.setlist_mut(stem)?.push_song(&canonical);
        Ok(())
    }

    pub fn remove_song_from_setlist(
        &mut self,
        stem: &str,
        index: usize,
    ) -> Result<(), LibraryError> {
        self.setlist_mut(stem)?
            .remove_song(index)
            .map(|_| ())
            .ok_or(LibraryError::PositionOutOfRange)
    }

    /// Move the song at `from` so it ends up at `to`.
    pub fn move_song_in_setlist(
        &mut self,
        stem: &str,
        from: usize,
        to: usize,
    ) -> Result<(), LibraryError> {
        if self.setlist_mut(stem)?.move_song(from, to) {
            Ok(())
        } else {
            Err(LibraryError::PositionOutOfRange)
        }
    }

    /// Copy a setlist as "<name> (copy)"; returns the new file name.
    pub fn copy_setlist(&mut self, stem: &str) -> Result<String, LibraryError> {
        let source = self
            .setlist(stem)
            .ok_or_else(|| LibraryError::NoSuchSetlist(stem.to_string()))?;
        let new_stem = self.fresh_stem(&format!("{stem}-copy"));
        let copy = source.copy_as(&new_stem, &format!("{} (copy)", source.name()));
        self.setlists.push(copy);
        self.sort_setlists();
        Ok(new_stem)
    }

    pub fn delete_setlist(&mut self, stem: &str) -> Result<(), LibraryError> {
        let index = self
            .setlists
            .iter()
            .position(|s| s.stem() == stem)
            .ok_or_else(|| LibraryError::NoSuchSetlist(stem.to_string()))?;
        let removed = self.setlists.remove(index);
        // Deleting a file that was never written is harmless, so a removal is always scheduled.
        if !self.removed.iter().any(|s| s == removed.stem()) {
            self.removed.push(removed.stem().to_string());
        }
        Ok(())
    }

    // ---- used by the import code ---------------------------------------------------------

    /// Add or replace a setlist file as it is (an imported one).
    pub(crate) fn put_setlist(&mut self, setlist: SetlistDoc) {
        self.removed.retain(|s| s != setlist.stem());
        match self
            .setlists
            .iter()
            .position(|s| s.stem() == setlist.stem())
        {
            Some(i) => self.setlists[i] = setlist,
            None => {
                self.setlists.push(setlist);
                self.sort_setlists();
            }
        }
    }

    pub(crate) fn fresh_setlist_stem(&self, base: &str) -> String {
        self.fresh_stem(base)
    }

    pub(crate) fn replace_settings_with(
        &mut self,
        other: &SettingsDoc,
    ) -> Result<(), LibraryError> {
        if self.settings_unreadable {
            return Err(LibraryError::FileUnreadable(SETTINGS_PATH.to_string()));
        }
        if self.settings.replace_with(other) {
            Ok(())
        } else {
            Err(LibraryError::ReadOnly(SETTINGS_PATH.to_string()))
        }
    }
}

#[cfg(test)]
mod tests;
