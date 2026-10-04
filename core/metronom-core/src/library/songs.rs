//! `songs.md`: every song, in one markdown table.
//!
//! Invariants:
//! - Parsing then rendering an untouched file returns it byte for byte.
//! - Editing a song rewrites only the table. Text before and after it, extra columns, rows the
//!   app does not understand, and the original spelling of cells that did not change all stay.

use std::collections::HashSet;

use super::Problem;
use super::text::{single_line, title_key};
use crate::engine::{MAX_BEATS_PER_BAR, MAX_BPM, MIN_BPM};

pub const DEFAULT_BPM: f64 = 120.0;
pub const DEFAULT_BEATS: u32 = 4;

/// One song: a title and how to click it.
#[derive(Clone, Debug, PartialEq)]
pub struct Song {
    pub title: String,
    /// Tempo, 30–300. May have decimals; the UI shows it rounded.
    pub bpm: f64,
    /// Beats per bar, 1–99.
    pub beats: u32,
    pub notes: String,
}

impl Song {
    pub fn new(title: &str, bpm: f64, beats: u32) -> Self {
        Self {
            title: title.to_string(),
            bpm,
            beats,
            notes: String::new(),
        }
        .sanitized()
    }

    #[must_use]
    pub fn with_notes(mut self, notes: &str) -> Self {
        self.notes = single_line(notes);
        self
    }

    /// Titles and notes on one line; tempo and beats inside their ranges.
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            title: single_line(&self.title),
            bpm: if self.bpm.is_finite() {
                self.bpm.clamp(MIN_BPM, MAX_BPM)
            } else {
                DEFAULT_BPM
            },
            beats: self.beats.clamp(1, MAX_BEATS_PER_BAR),
            notes: single_line(&self.notes),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongsError {
    NotFound,
    /// Another song already has this title (ignoring case and surrounding spaces).
    DuplicateTitle,
    EmptyTitle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Align {
    None,
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Debug)]
struct Columns {
    song: usize,
    bpm: usize,
    beats: Option<usize>,
    notes: Option<usize>,
}

#[derive(Clone, Debug)]
struct SongRow {
    song: Song,
    /// What was read from the file; `None` for a song added in the app. Cells whose value did
    /// not change keep their original spelling.
    original: Option<Song>,
    cells: Vec<String>,
}

#[derive(Clone, Debug)]
enum Row {
    Song(SongRow),
    /// A row the app does not manage (no title, or a repeated title); kept as written.
    Opaque(Vec<String>),
}

#[derive(Clone, Debug)]
struct Table {
    columns: Vec<String>,
    aligns: Vec<Align>,
    cols: Columns,
    rows: Vec<Row>,
}

#[derive(Clone, Debug)]
pub struct SongsDoc {
    original: String,
    /// Text before the table, including its final line break.
    prefix: String,
    table: Option<Table>,
    /// Text after the table's last line (after the line break that ends it).
    suffix: String,
    dirty: bool,
}

impl SongsDoc {
    /// A new, empty songs file. Nothing is written until a song is added.
    pub fn new() -> Self {
        Self {
            original: String::new(),
            prefix: String::new(),
            table: None,
            suffix: String::new(),
            dirty: false,
        }
    }

    /// `text` should already be normalized (see [`super::text::normalize`]).
    pub fn parse(text: &str, problems: &mut Vec<Problem>) -> Self {
        let lines: Vec<&str> = text.split('\n').collect();
        let mut i = 0;
        let mut found = None;
        while i < lines.len() {
            if is_table_row(lines[i]) && i + 1 < lines.len() && is_separator(lines[i + 1]) {
                let end = (i + 2..lines.len())
                    .find(|&j| !is_table_row(lines[j]))
                    .unwrap_or(lines.len());
                if split_cells(lines[i]).len() == split_cells(lines[i + 1]).len() {
                    match parse_table(&lines[i..end], i + 1, problems) {
                        Some(table) => {
                            found = Some((i, end, table));
                            break;
                        }
                        None => problems.push(Problem::at(
                            i + 1,
                            "this table has no Song and BPM columns, so it is not the song list"
                                .to_string(),
                        )),
                    }
                }
                i = end;
            } else {
                i += 1;
            }
        }

        match found {
            Some((start, end, table)) => Self {
                original: text.to_string(),
                prefix: lines[..start].iter().map(|l| format!("{l}\n")).collect(),
                table: Some(table),
                suffix: if end < lines.len() {
                    lines[end..].join("\n")
                } else {
                    String::new()
                },
                dirty: false,
            },
            None => {
                if !text.trim().is_empty() {
                    problems.push(Problem::general(
                        "no table with Song and BPM columns was found; songs added in the app go in a new table at the end".to_string(),
                    ));
                }
                Self {
                    original: text.to_string(),
                    prefix: text.to_string(),
                    table: None,
                    suffix: String::new(),
                    dirty: false,
                }
            }
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn songs(&self) -> impl Iterator<Item = &Song> {
        self.table
            .iter()
            .flat_map(|t| t.rows.iter())
            .filter_map(|row| match row {
                Row::Song(r) => Some(&r.song),
                Row::Opaque(_) => None,
            })
    }

    pub fn find(&self, title: &str) -> Option<&Song> {
        let key = title_key(title);
        self.songs().find(|s| title_key(&s.title) == key)
    }

    /// Add a song at the end of the list.
    pub fn push(&mut self, song: Song) -> Result<(), SongsError> {
        let song = song.sanitized();
        if song.title.is_empty() {
            return Err(SongsError::EmptyTitle);
        }
        if self.find(&song.title).is_some() {
            return Err(SongsError::DuplicateTitle);
        }
        let table = self.ensure_table();
        table.rows.push(Row::Song(SongRow {
            song,
            original: None,
            cells: Vec::new(),
        }));
        table.ensure_columns();
        self.dirty = true;
        Ok(())
    }

    /// Replace the song titled `title` (renaming it if the new title differs).
    pub fn update(&mut self, title: &str, song: Song) -> Result<(), SongsError> {
        let song = song.sanitized();
        if song.title.is_empty() {
            return Err(SongsError::EmptyTitle);
        }
        let old_key = title_key(title);
        let new_key = title_key(&song.title);
        if new_key != old_key && self.find(&song.title).is_some() {
            return Err(SongsError::DuplicateTitle);
        }
        let row = self
            .table
            .as_mut()
            .and_then(|t| {
                t.rows.iter_mut().find_map(|row| match row {
                    Row::Song(r) if title_key(&r.song.title) == old_key => Some(r),
                    _ => None,
                })
            })
            .ok_or(SongsError::NotFound)?;
        if row.song != song {
            row.song = song;
            if let Some(table) = self.table.as_mut() {
                table.ensure_columns();
            }
            self.dirty = true;
        }
        Ok(())
    }

    pub fn remove(&mut self, title: &str) -> Option<Song> {
        let key = title_key(title);
        let table = self.table.as_mut()?;
        let index = table
            .rows
            .iter()
            .position(|row| matches!(row, Row::Song(r) if title_key(&r.song.title) == key))?;
        self.dirty = true;
        match table.rows.remove(index) {
            Row::Song(r) => Some(r.song),
            Row::Opaque(_) => None,
        }
    }

    /// The file's text: the original if nothing changed, otherwise with the table rewritten.
    pub fn render(&self) -> String {
        if !self.dirty {
            return self.original.clone();
        }
        let Some(table) = &self.table else {
            return self.original.clone();
        };
        let mut out = self.prefix.clone();
        out.push_str(&table.render().join("\n"));
        out.push('\n');
        out.push_str(&self.suffix);
        out
    }

    fn ensure_table(&mut self) -> &mut Table {
        if self.table.is_none() {
            let existing = self.prefix.trim_end();
            self.prefix = if existing.is_empty() {
                "# Songs\n\n".to_string()
            } else {
                format!("{existing}\n\n")
            };
            self.suffix = String::new();
            self.table = Some(Table {
                columns: vec!["Song".to_string(), "BPM".to_string()],
                aligns: vec![Align::None, Align::Right],
                cols: Columns {
                    song: 0,
                    bpm: 1,
                    beats: None,
                    notes: None,
                },
                rows: Vec::new(),
            });
        }
        self.table.as_mut().expect("just created")
    }
}

impl Default for SongsDoc {
    fn default() -> Self {
        Self::new()
    }
}

impl Table {
    /// Add a Beats and/or Notes column when some song needs one and the table has none.
    fn ensure_columns(&mut self) {
        let songs = || {
            self.rows.iter().filter_map(|r| match r {
                Row::Song(s) => Some(&s.song),
                Row::Opaque(_) => None,
            })
        };
        let needs_beats = self.cols.beats.is_none() && songs().any(|s| s.beats != DEFAULT_BEATS);
        let needs_notes = self.cols.notes.is_none() && songs().any(|s| !s.notes.is_empty());
        if needs_beats {
            self.cols.beats = Some(self.columns.len());
            self.columns.push("Beats".to_string());
            self.aligns.push(Align::Right);
        }
        if needs_notes {
            self.cols.notes = Some(self.columns.len());
            self.columns.push("Notes".to_string());
            self.aligns.push(Align::None);
        }
    }

    fn render(&self) -> Vec<String> {
        let n = self.columns.len();
        let rows: Vec<Vec<String>> = self
            .rows
            .iter()
            .map(|row| {
                let mut cells = match row {
                    Row::Opaque(cells) => cells.clone(),
                    Row::Song(r) => self.song_cells(r),
                };
                if cells.len() < n {
                    cells.resize(n, String::new());
                }
                cells
            })
            .collect();
        let width = |col: usize| {
            let header = self
                .columns
                .get(col)
                .map_or(0, |c| escape(c).chars().count());
            rows.iter()
                .filter_map(|r| r.get(col))
                .map(|c| escape(c).chars().count())
                .chain([header, 3])
                .max()
                .unwrap_or(3)
        };
        let total = rows.iter().map(Vec::len).max().unwrap_or(0).max(n);
        let widths: Vec<usize> = (0..total).map(width).collect();
        let aligns: Vec<Align> = (0..total)
            .map(|c| self.aligns.get(c).copied().unwrap_or(Align::None))
            .collect();

        let render_row = |cells: &[String]| {
            let padded: Vec<String> = (0..total)
                .map(|c| {
                    let text = escape(cells.get(c).map_or("", String::as_str));
                    let pad = widths[c].saturating_sub(text.chars().count());
                    match aligns[c] {
                        Align::Right => format!("{}{text}", " ".repeat(pad)),
                        Align::Center => {
                            format!("{}{text}{}", " ".repeat(pad / 2), " ".repeat(pad - pad / 2))
                        }
                        Align::Left | Align::None => format!("{text}{}", " ".repeat(pad)),
                    }
                })
                .collect();
            format!("| {} |", padded.join(" | "))
        };
        let header: Vec<String> = (0..total)
            .map(|c| self.columns.get(c).cloned().unwrap_or_default())
            .collect();
        let separator: Vec<String> = (0..total)
            .map(|c| {
                let w = widths[c] + 2;
                match aligns[c] {
                    Align::None => "-".repeat(w),
                    Align::Left => format!(":{}", "-".repeat(w - 1)),
                    Align::Right => format!("{}:", "-".repeat(w - 1)),
                    Align::Center => format!(":{}:", "-".repeat(w - 2)),
                }
            })
            .collect();

        let mut lines = vec![render_row(&header), format!("|{}|", separator.join("|"))];
        lines.extend(rows.iter().map(|r| render_row(r)));
        lines
    }

    /// The cells of a song row: its original cells, with only the changed values rewritten.
    fn song_cells(&self, row: &SongRow) -> Vec<String> {
        let mut cells = row.cells.clone();
        cells.resize(self.columns.len(), String::new());
        let original = row.original.as_ref();
        if original.is_none_or(|o| o.title != row.song.title) {
            cells[self.cols.song] = row.song.title.clone();
        }
        if original.is_none_or(|o| o.bpm != row.song.bpm) {
            cells[self.cols.bpm] = format_bpm(row.song.bpm);
        }
        if let Some(c) = self.cols.beats
            && original.is_none_or(|o| o.beats != row.song.beats)
        {
            cells[c] = row.song.beats.to_string();
        }
        if let Some(c) = self.cols.notes
            && original.is_none_or(|o| o.notes != row.song.notes)
        {
            cells[c] = row.song.notes.clone();
        }
        cells
    }
}

fn format_bpm(bpm: f64) -> String {
    if (bpm - bpm.round()).abs() < 1e-9 {
        format!("{}", bpm.round() as i64)
    } else {
        let text = format!("{bpm:.2}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn is_table_row(line: &str) -> bool {
    line.contains('|') && !line.trim().is_empty()
}

fn is_separator(line: &str) -> bool {
    if !line.contains('|') && !line.contains("--") {
        return false;
    }
    let cells = split_cells(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let core = c.trim_start_matches(':').trim_end_matches(':');
            !core.is_empty() && core.chars().all(|ch| ch == '-')
        })
}

/// Split a table line into trimmed cells; `\|` is a literal pipe.
fn split_cells(line: &str) -> Vec<String> {
    let trimmed = line.trim();
    let inner = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let inner = if inner.ends_with('|') && !inner.ends_with("\\|") {
        &inner[..inner.len() - 1]
    } else {
        inner
    };
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                current.push('|');
                chars.next();
            }
            '|' => {
                cells.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(c),
        }
    }
    cells.push(current.trim().to_string());
    cells
}

fn escape(cell: &str) -> String {
    cell.replace('|', "\\|")
}

fn parse_align(cell: &str) -> Align {
    match (cell.starts_with(':'), cell.ends_with(':')) {
        (true, true) => Align::Center,
        (true, false) => Align::Left,
        (false, true) => Align::Right,
        (false, false) => Align::None,
    }
}

/// Parse the table in `lines` (header, separator, rows) if it has Song and BPM columns.
/// `first_line` is the 1-based file line of the header, for messages.
fn parse_table(lines: &[&str], first_line: usize, problems: &mut Vec<Problem>) -> Option<Table> {
    let columns = split_cells(lines[0]);
    let find = |name: &str| {
        columns
            .iter()
            .position(|c| c.trim().eq_ignore_ascii_case(name))
    };
    let cols = Columns {
        song: find("song")?,
        bpm: find("bpm")?,
        beats: find("beats"),
        notes: find("notes"),
    };
    let aligns = split_cells(lines[1])
        .iter()
        .map(|c| parse_align(c))
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut rows = Vec::new();
    for (offset, line) in lines[2..].iter().enumerate() {
        let line_number = first_line + 2 + offset;
        let cells = split_cells(line);
        let cell = |i: usize| cells.get(i).map_or("", String::as_str);
        let title = cell(cols.song);
        if title.is_empty() {
            problems.push(Problem::at(
                line_number,
                "this row has no song title; it is ignored and kept as written".to_string(),
            ));
            rows.push(Row::Opaque(cells));
            continue;
        }
        if !seen.insert(title_key(title)) {
            problems.push(Problem::at(
                line_number,
                format!("the title \"{title}\" is already used by an earlier row; this row is ignored and kept as written"),
            ));
            rows.push(Row::Opaque(cells));
            continue;
        }

        let bpm = match cell(cols.bpm).parse::<f64>() {
            Ok(v) if v.is_finite() => {
                let clamped = v.clamp(MIN_BPM, MAX_BPM);
                if clamped != v {
                    problems.push(Problem::at(
                        line_number,
                        format!(
                            "BPM {v} is outside {MIN_BPM}–{MAX_BPM} and was clamped to {clamped}"
                        ),
                    ));
                }
                clamped
            }
            _ => {
                problems.push(Problem::at(
                    line_number,
                    format!(
                        "\"{}\" is not a tempo; {DEFAULT_BPM} is used",
                        cell(cols.bpm)
                    ),
                ));
                DEFAULT_BPM
            }
        };
        let beats_text = cols.beats.map_or("", cell);
        let beats = if beats_text.is_empty() {
            DEFAULT_BEATS
        } else {
            match beats_text.parse::<u32>() {
                Ok(v) => {
                    let clamped = v.clamp(1, MAX_BEATS_PER_BAR);
                    if clamped != v {
                        problems.push(Problem::at(
                            line_number,
                            format!("{v} beats is outside 1–{MAX_BEATS_PER_BAR} and was clamped to {clamped}"),
                        ));
                    }
                    clamped
                }
                Err(_) => {
                    problems.push(Problem::at(
                        line_number,
                        format!(
                            "\"{beats_text}\" is not a number of beats; {DEFAULT_BEATS} is used"
                        ),
                    ));
                    DEFAULT_BEATS
                }
            }
        };
        let song = Song {
            title: title.to_string(),
            bpm,
            beats,
            notes: cols.notes.map_or("", cell).to_string(),
        };
        rows.push(Row::Song(SongRow {
            original: Some(song.clone()),
            song,
            cells,
        }));
    }
    Some(Table {
        columns,
        aligns,
        cols,
        rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SAMPLE: &str = "# Songs\n\n| Song             | BPM | Beats | Notes      |\n|------------------|----:|------:|------------|\n| Hotel California |  75 |     4 | Bm, capo 7 |\n| Superstition     | 100 |     4 |            |\n| Waltz for Debby  | 132 |     3 |            |\n";

    fn parse(text: &str) -> (SongsDoc, Vec<Problem>) {
        let mut problems = Vec::new();
        let doc = SongsDoc::parse(text, &mut problems);
        (doc, problems)
    }

    fn titles(doc: &SongsDoc) -> Vec<String> {
        doc.songs().map(|s| s.title.clone()).collect()
    }

    #[test]
    fn reads_the_sample_table() {
        let (doc, problems) = parse(SAMPLE);
        assert!(problems.is_empty(), "{problems:?}");
        let songs: Vec<&Song> = doc.songs().collect();
        assert_eq!(songs.len(), 3);
        assert_eq!(
            songs[0],
            &Song {
                title: "Hotel California".into(),
                bpm: 75.0,
                beats: 4,
                notes: "Bm, capo 7".into()
            }
        );
        assert_eq!(
            (songs[2].title.as_str(), songs[2].bpm, songs[2].beats),
            ("Waltz for Debby", 132.0, 3)
        );
        assert_eq!(doc.find("  superSTITION ").map(|s| s.bpm), Some(100.0));
    }

    #[test]
    fn untouched_files_render_byte_for_byte() {
        for text in [
            "",
            "no table here\n",
            SAMPLE,
            "| Song | BPM |\n|-|-|\n| A | 90 |",
            "intro\n\n|Song|BPM|\n|:-|-:|\n|A|90|\n\nafter\n",
            "| a | b |\n|---|---|\n| 1 | 2 |\n\n| Song | BPM |\n|---|---|\n| X | 100 |\n",
        ] {
            let (doc, _) = parse(text);
            assert_eq!(doc.render(), text, "{text:?}");
        }
    }

    #[test]
    fn columns_are_found_by_name_in_any_order_and_extras_survive() {
        let text = "| Key | bpm | SONG | Notes | Capo |\n|---|---|---|---|---|\n| Bm | 75 | Hotel California | warm | 7 |\n";
        let (mut doc, problems) = parse(text);
        assert!(problems.is_empty());
        assert_eq!(doc.songs().next().unwrap().title, "Hotel California");
        doc.push(Song::new("Wonderwall", 87.0, 4)).unwrap();
        let out = doc.render();
        assert!(out.contains("Hotel California"));
        assert!(out.contains("| Bm"), "the Key column survives: {out}");
        assert!(out.contains("Capo"), "the Capo column survives: {out}");
        let (again, problems) = parse(&out);
        assert!(problems.is_empty(), "{problems:?}\n{out}");
        assert_eq!(titles(&again), ["Hotel California", "Wonderwall"]);
    }

    #[test]
    fn editing_keeps_text_around_the_table_and_untouched_cell_spelling() {
        let text = "# My songs\n\nIntro text.\n\n| Song | BPM | Notes |\n|---|---|---|\n| A | 97.50 | keep  |\n| B | 100 |  |\n\nOutro text.\n";
        let (mut doc, _) = parse(text);
        let b = doc.find("B").unwrap().clone();
        doc.update("B", Song { bpm: 110.0, ..b }).unwrap();
        let out = doc.render();
        assert!(out.starts_with("# My songs\n\nIntro text.\n\n"), "{out}");
        assert!(out.ends_with("\nOutro text.\n"), "{out}");
        assert!(
            out.contains("97.50"),
            "an untouched BPM cell keeps its spelling: {out}"
        );
        let (again, _) = parse(&out);
        assert_eq!(again.find("B").unwrap().bpm, 110.0);
        assert_eq!(again.find("A").unwrap().bpm, 97.5);
    }

    #[test]
    fn pipes_in_titles_and_notes_round_trip() {
        let mut doc = SongsDoc::new();
        doc.push(Song::new("Rock | Roll", 120.0, 4).with_notes("a | b | c"))
            .unwrap();
        let out = doc.render();
        assert!(out.contains("Rock \\| Roll"), "{out}");
        let (again, problems) = parse(&out);
        assert!(problems.is_empty(), "{problems:?}");
        let s = again.songs().next().unwrap();
        assert_eq!(
            (s.title.as_str(), s.notes.as_str()),
            ("Rock | Roll", "a | b | c")
        );
    }

    #[test]
    fn a_repeated_title_is_ignored_with_a_warning_but_kept_in_the_file() {
        let text = "| Song | BPM |\n|---|---|\n| A | 90 |\n| a  | 100 |\n| B | 80 |\n";
        let (mut doc, problems) = parse(text);
        assert_eq!(titles(&doc), ["A", "B"]);
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].line, Some(4));
        assert!(problems[0].message.contains("already used"));
        doc.push(Song::new("C", 70.0, 4)).unwrap();
        assert!(
            doc.render().contains("| a "),
            "the duplicate row stays: {}",
            doc.render()
        );
    }

    #[test]
    fn rows_without_a_title_are_ignored_and_kept() {
        let text = "| Song | BPM |\n|---|---|\n|  | 90 |\n| A | 80 |\n";
        let (mut doc, problems) = parse(text);
        assert_eq!(titles(&doc), ["A"]);
        assert_eq!(problems.len(), 1);
        doc.remove("A");
        assert!(doc.render().contains("90"), "{}", doc.render());
    }

    #[test]
    fn bad_values_fall_back_with_warnings() {
        let text = "| Song | BPM | Beats |\n|---|---|---|\n| A | fast | 4 |\n| B | 500 | many |\n| C | 10 | 200 |\n| D | 90.5 |  |\n";
        let (doc, problems) = parse(text);
        let by = |t: &str| doc.find(t).unwrap().clone();
        assert_eq!((by("A").bpm, by("A").beats), (DEFAULT_BPM, 4));
        assert_eq!((by("B").bpm, by("B").beats), (300.0, DEFAULT_BEATS));
        assert_eq!((by("C").bpm, by("C").beats), (30.0, 99));
        assert_eq!((by("D").bpm, by("D").beats), (90.5, DEFAULT_BEATS));
        assert_eq!(problems.len(), 5, "{problems:?}");
    }

    #[test]
    fn a_file_without_a_song_table_gets_one_added_after_existing_text() {
        let (mut doc, problems) = parse("Some notes about my songs.\n");
        assert_eq!(problems.len(), 1);
        doc.push(Song::new("A", 100.0, 4)).unwrap();
        let out = doc.render();
        assert!(
            out.starts_with("Some notes about my songs.\n\n| Song"),
            "{out}"
        );
        assert_eq!(titles(&parse(&out).0), ["A"]);
    }

    #[test]
    fn a_new_file_starts_with_a_heading() {
        let mut doc = SongsDoc::new();
        assert!(!doc.is_dirty());
        assert_eq!(doc.render(), "");
        doc.push(Song::new("A", 100.0, 4)).unwrap();
        assert!(
            doc.render().starts_with("# Songs\n\n| Song"),
            "{}",
            doc.render()
        );
    }

    #[test]
    fn another_kind_of_table_is_not_mistaken_for_the_song_list() {
        let text = "| Name | Age |\n|---|---|\n| X | 3 |\n";
        let (mut doc, problems) = parse(text);
        assert_eq!(doc.songs().count(), 0);
        assert!(
            problems
                .iter()
                .any(|p| p.message.contains("no Song and BPM"))
        );
        doc.push(Song::new("A", 100.0, 4)).unwrap();
        let out = doc.render();
        assert!(out.starts_with(text.trim_end()), "{out}");
        assert_eq!(titles(&parse(&out).0), ["A"]);
    }

    #[test]
    fn beats_and_notes_columns_are_added_only_when_needed() {
        let (mut doc, _) = parse("| Song | BPM |\n|---|---|\n| A | 90 |\n");
        doc.push(Song::new("B", 100.0, 4)).unwrap();
        assert!(!doc.render().contains("Beats") && !doc.render().contains("Notes"));
        doc.push(Song::new("C", 110.0, 3)).unwrap();
        assert!(doc.render().contains("Beats") && !doc.render().contains("Notes"));
        doc.push(Song::new("D", 120.0, 4).with_notes("intro 8 bars"))
            .unwrap();
        let out = doc.render();
        assert!(out.contains("Beats") && out.contains("Notes"), "{out}");
        let (again, problems) = parse(&out);
        assert!(problems.is_empty(), "{problems:?}\n{out}");
        let beats: Vec<u32> = again.songs().map(|s| s.beats).collect();
        assert_eq!(beats, [4, 4, 3, 4]);
        assert_eq!(again.find("D").unwrap().notes, "intro 8 bars");
    }

    #[test]
    fn titles_must_be_unique_and_non_empty() {
        let (mut doc, _) = parse(SAMPLE);
        assert_eq!(
            doc.push(Song::new(" hotel CALIFORNIA ", 80.0, 4)),
            Err(SongsError::DuplicateTitle)
        );
        assert_eq!(
            doc.push(Song::new("   ", 80.0, 4)),
            Err(SongsError::EmptyTitle)
        );
        assert_eq!(
            doc.update("Nope", Song::new("X", 80.0, 4)),
            Err(SongsError::NotFound)
        );
        assert_eq!(
            doc.update("Superstition", Song::new("hotel california", 80.0, 4)),
            Err(SongsError::DuplicateTitle)
        );
        assert!(!doc.is_dirty(), "failed edits change nothing");
        // Re-saving a song under its own title with different case is not a clash.
        assert_eq!(
            doc.update("Superstition", Song::new("SUPERSTITION", 100.0, 4)),
            Ok(())
        );
    }

    #[test]
    fn renaming_updates_the_title_cell_only() {
        let (mut doc, _) = parse(SAMPLE);
        let old = doc.find("Superstition").unwrap().clone();
        doc.update(
            "Superstition",
            Song {
                title: "Superstition (live)".into(),
                ..old
            },
        )
        .unwrap();
        let out = doc.render();
        assert!(out.contains("Superstition (live)"));
        assert_eq!(
            titles(&parse(&out).0),
            ["Hotel California", "Superstition (live)", "Waltz for Debby"]
        );
    }

    #[test]
    fn removing_a_song_removes_only_its_row() {
        let (mut doc, _) = parse(SAMPLE);
        assert_eq!(doc.remove("superstition").map(|s| s.bpm), Some(100.0));
        assert_eq!(doc.remove("superstition"), None);
        assert_eq!(titles(&doc), ["Hotel California", "Waltz for Debby"]);
        assert!(!doc.render().contains("Superstition"));
    }

    #[test]
    fn songs_are_made_safe_on_entry() {
        let s = Song::new("A\nB", f64::NAN, 0).with_notes("x\ny");
        assert_eq!(
            (s.title.as_str(), s.bpm, s.beats, s.notes.as_str()),
            ("A B", DEFAULT_BPM, 1, "x y")
        );
        let s = Song::new("A", 9_999.0, 1_000);
        assert_eq!((s.bpm, s.beats), (300.0, 99));
    }

    #[test]
    fn decimal_tempos_are_written_compactly() {
        assert_eq!(format_bpm(75.0), "75");
        assert_eq!(format_bpm(97.5), "97.5");
        assert_eq!(format_bpm(97.25), "97.25");
        assert_eq!(format_bpm(97.123456), "97.12");
    }

    #[test]
    fn alignment_markers_in_the_separator_are_kept() {
        let text = "| Song | BPM |\n|:--|--:|\n| A | 90 |\n";
        let (mut doc, _) = parse(text);
        doc.push(Song::new("B", 100.0, 4)).unwrap();
        let out = doc.render();
        let separator = out.lines().nth(1).unwrap();
        assert!(separator.contains(':'), "{out}");
        assert_eq!(parse(&out).0.songs().count(), 2);
    }

    proptest! {
        #[test]
        fn any_text_renders_back_exactly_and_never_panics(text in "[ -~\n|:\\\\é-]{0,300}") {
            let mut problems = Vec::new();
            let doc = SongsDoc::parse(&text, &mut problems);
            prop_assert_eq!(doc.render(), text);
        }

        #[test]
        fn songs_survive_a_write_and_read(
            raw in proptest::collection::vec(
                ("[A-Za-z0-9 |'éøå.,!-]{1,20}", 30u32..=300, 1u32..=99, "[A-Za-z0-9 |,.-]{0,15}"),
                0..8,
            )
        ) {
            let mut doc = SongsDoc::new();
            let mut expected: Vec<Song> = Vec::new();
            for (title, bpm, beats, notes) in raw {
                let song = Song::new(&title, f64::from(bpm), beats).with_notes(&notes);
                if song.title.is_empty() || expected.iter().any(|s| title_key(&s.title) == title_key(&song.title)) {
                    continue;
                }
                doc.push(song.clone()).unwrap();
                expected.push(song);
            }
            let text = doc.render();
            let mut problems = Vec::new();
            let again = SongsDoc::parse(&text, &mut problems);
            prop_assert!(problems.is_empty(), "{:?}\n{}", problems, text);
            let got: Vec<Song> = again.songs().cloned().collect();
            prop_assert_eq!(got, expected);
        }

        #[test]
        fn editing_one_song_never_changes_the_others(
            titles in proptest::collection::hash_set("[A-Z][a-z]{2,8}", 2..6),
            pick in 0usize..5,
            new_bpm in 30u32..=300,
        ) {
            let titles: Vec<String> = titles.into_iter().collect();
            let mut text = String::from("| Song | BPM |\n|---|---|\n");
            for (i, t) in titles.iter().enumerate() {
                text.push_str(&format!("| {t} |   {}  |\n", 60 + i));
            }
            let mut problems = Vec::new();
            let mut doc = SongsDoc::parse(&text, &mut problems);
            let target = &titles[pick % titles.len()];
            let before = doc.find(target).unwrap().clone();
            doc.update(target, Song { bpm: f64::from(new_bpm), ..before }).unwrap();
            let out = doc.render();
            let again = SongsDoc::parse(&out, &mut problems);
            for (i, t) in titles.iter().enumerate() {
                let expected = if t == target { f64::from(new_bpm) } else { f64::from(60 + i as u32) };
                prop_assert_eq!(again.find(t).unwrap().bpm, expected);
            }
        }
    }
}
