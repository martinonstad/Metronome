//! `setlists/<name>.md`: a named list of songs, tagged with a band.
//!
//! Invariants:
//! - Parsing then rendering an untouched file returns it byte for byte.
//! - Editing the song list rewrites only the list (renumbered) and the blank lines around it.
//!   The header (unknown keys, comments), the heading and all notes are kept.
//! - The first list in the file is the song list; any later list is notes.

use super::Problem;
use super::header::Document;
use super::text::{single_line, title_key};

/// The setlist in a file: its name, band and the song titles in order.
#[derive(Clone, Debug)]
pub struct SetlistDoc {
    stem: String,
    original: String,
    doc: Document,
    /// Body lines before the song list (the heading, intro notes).
    before: Vec<String>,
    /// The song titles, as written.
    items: Vec<String>,
    /// Body lines after the song list (notes).
    after: Vec<String>,
    /// The list or header was edited: the file is rebuilt from the model.
    dirty: bool,
    /// The file must be written even if unedited (an imported file).
    needs_write: bool,
}

impl SetlistDoc {
    /// `stem` is the file name without `.md`; `text` should already be normalized.
    pub fn parse(stem: &str, text: &str, problems: &mut Vec<Problem>) -> Self {
        let doc = Document::parse(text, problems);
        let lines: Vec<&str> = doc.body.split('\n').collect();
        let fenced = fence_mask(&lines);
        let is_item = |i: usize| !fenced[i] && list_item(lines[i]).is_some();

        let (before, items, after) = match (0..lines.len()).find(|&i| is_item(i)) {
            None => (
                lines.iter().map(|l| (*l).to_string()).collect(),
                Vec::new(),
                Vec::new(),
            ),
            Some(start) => {
                let mut last = start;
                let mut k = start + 1;
                while k < lines.len() {
                    if is_item(k) {
                        last = k;
                        k += 1;
                    } else if lines[k].trim().is_empty() {
                        // Blank lines between items are fine; they end the list otherwise.
                        match (k + 1..lines.len()).find(|&j| !lines[j].trim().is_empty()) {
                            Some(next) if is_item(next) => k = next,
                            _ => break,
                        }
                    } else {
                        break;
                    }
                }
                (
                    lines[..start].iter().map(|l| (*l).to_string()).collect(),
                    (start..=last)
                        .filter(|&i| is_item(i))
                        .filter_map(|i| list_item(lines[i]).map(str::to_string))
                        .collect(),
                    lines[last + 1..].iter().map(|l| (*l).to_string()).collect(),
                )
            }
        };
        Self {
            stem: stem.to_string(),
            original: text.to_string(),
            doc,
            before,
            items,
            after,
            dirty: false,
            needs_write: false,
        }
    }

    /// A new, empty setlist. It is written the first time it is saved.
    pub fn new(stem: &str, name: &str, band: Option<&str>) -> Self {
        let mut doc = Document::parse("", &mut Vec::new());
        let mut setlist = Self {
            stem: stem.to_string(),
            original: String::new(),
            doc: doc.clone(),
            before: Vec::new(),
            items: Vec::new(),
            after: Vec::new(),
            dirty: true,
            needs_write: true,
        };
        setlist.set_name(name);
        setlist.set_band(band);
        doc = setlist.doc.clone();
        setlist.doc = doc;
        setlist
    }

    /// The file name without `.md`: the setlist's stable identity.
    pub fn stem(&self) -> &str {
        &self.stem
    }

    /// Does the file need to be written?
    pub fn is_dirty(&self) -> bool {
        self.dirty || self.needs_write
    }

    /// This setlist, unchanged, under another file name; it is written the next time the
    /// library is saved. An unedited file is written exactly as it was read.
    #[must_use]
    pub fn with_stem(&self, stem: &str) -> Self {
        let mut copy = self.clone();
        copy.stem = stem.to_string();
        copy.needs_write = true;
        copy
    }

    /// The first `# heading`, or the file name if there is none.
    pub fn name(&self) -> String {
        self.heading_position()
            .map(|(in_before, i)| {
                let line = if in_before {
                    &self.before[i]
                } else {
                    &self.after[i]
                };
                heading_text(line).unwrap_or_default().to_string()
            })
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| self.stem.clone())
    }

    /// The band or project (`band` in the header); `None` when absent or empty.
    pub fn band(&self) -> Option<&str> {
        self.doc
            .get("band")
            .map(str::trim)
            .filter(|b| !b.is_empty())
    }

    pub fn songs(&self) -> &[String] {
        &self.items
    }

    pub fn set_name(&mut self, name: &str) {
        let name = single_line(name);
        if name == self.name() && self.heading_position().is_some() {
            return;
        }
        let line = format!("# {name}");
        match self.heading_position() {
            Some((true, i)) => self.before[i] = line,
            Some((false, i)) => self.after[i] = line,
            None => {
                let blank_follows = self.before.first().is_none_or(|l| l.trim().is_empty());
                if !blank_follows {
                    self.before.insert(0, String::new());
                }
                self.before.insert(0, line);
            }
        }
        self.dirty = true;
    }

    /// `None` (or an empty name) removes the band.
    pub fn set_band(&mut self, band: Option<&str>) {
        let band = band.map(single_line).filter(|b| !b.is_empty());
        if band.as_deref() == self.band() {
            return;
        }
        match band {
            Some(band) => self.doc.header_mut().set("band", &band),
            None => {
                if self.doc.header().is_some() {
                    self.doc.header_mut().remove("band");
                }
            }
        }
        self.dirty = true;
    }

    pub fn push_song(&mut self, title: &str) {
        self.items.push(single_line(title));
        self.dirty = true;
    }

    /// Insert a song before position `index` (or at the end if `index` is past the end).
    pub fn insert_song(&mut self, index: usize, title: &str) {
        let index = index.min(self.items.len());
        self.items.insert(index, single_line(title));
        self.dirty = true;
    }

    pub fn remove_song(&mut self, index: usize) -> Option<String> {
        if index >= self.items.len() {
            return None;
        }
        self.dirty = true;
        Some(self.items.remove(index))
    }

    /// Move the song at `from` so that it ends up at position `to`. Returns false (and changes
    /// nothing) if either position is out of range.
    pub fn move_song(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() {
            return false;
        }
        if from != to {
            let item = self.items.remove(from);
            self.items.insert(to, item);
            self.dirty = true;
        }
        true
    }

    /// Positions of the songs titled `title` (ignoring case and surrounding spaces).
    pub fn positions_of(&self, title: &str) -> Vec<usize> {
        let key = title_key(title);
        (0..self.items.len())
            .filter(|&i| title_key(&self.items[i]) == key)
            .collect()
    }

    /// Rename every occurrence of a song; returns how many were changed.
    pub fn rename_song(&mut self, old: &str, new: &str) -> usize {
        let positions = self.positions_of(old);
        let new = single_line(new);
        for &i in &positions {
            self.items[i].clone_from(&new);
        }
        if !positions.is_empty() {
            self.dirty = true;
        }
        positions.len()
    }

    /// Remove every occurrence of a song; returns how many were removed.
    pub fn remove_title(&mut self, title: &str) -> usize {
        let key = title_key(title);
        let before = self.items.len();
        self.items.retain(|t| title_key(t) != key);
        let removed = before - self.items.len();
        if removed > 0 {
            self.dirty = true;
        }
        removed
    }

    /// A copy under a new file name and heading, as the app's "Copy" button makes.
    #[must_use]
    pub fn copy_as(&self, stem: &str, name: &str) -> Self {
        let mut copy = self.clone();
        copy.stem = stem.to_string();
        copy.original = String::new();
        copy.dirty = true;
        copy.needs_write = true;
        copy.set_name(name);
        copy
    }

    /// The file's text: the original if nothing changed, otherwise with the list rewritten.
    pub fn render(&self) -> String {
        if !self.dirty {
            return self.original.clone();
        }
        let mut doc = self.doc.clone();
        let mut lines: Vec<String> = trim_trailing_blank(&self.before);
        if !self.items.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.extend(
                self.items
                    .iter()
                    .enumerate()
                    .map(|(i, t)| format!("{}. {t}", i + 1)),
            );
        }
        let after = trim_leading_blank(&self.after);
        if after.iter().any(|l| !l.trim().is_empty()) {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            lines.extend(after);
        }
        let mut body = lines.join("\n");
        if !body.is_empty() && !body.ends_with('\n') {
            body.push('\n');
        }
        doc.body = body;
        doc.render()
    }

    /// Where the first `# heading` is: (in `before`?, index).
    fn heading_position(&self) -> Option<(bool, usize)> {
        let find = |lines: &[String]| {
            let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
            let fenced = fence_mask(&refs);
            (0..lines.len()).find(|&i| !fenced[i] && heading_text(&lines[i]).is_some())
        };
        find(&self.before)
            .map(|i| (true, i))
            .or_else(|| find(&self.after).map(|i| (false, i)))
    }
}

fn trim_trailing_blank(lines: &[String]) -> Vec<String> {
    let end = lines
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(0, |i| i + 1);
    lines[..end].to_vec()
}

fn trim_leading_blank(lines: &[String]) -> Vec<String> {
    let start = lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(lines.len());
    lines[start..].to_vec()
}

/// For each line: is it inside (or part of) a fenced code block?
fn fence_mask(lines: &[&str]) -> Vec<bool> {
    let mut inside = false;
    lines
        .iter()
        .map(|line| {
            let fence =
                line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~");
            if fence {
                inside = !inside;
                return true;
            }
            inside
        })
        .collect()
}

/// The text of a level-1 heading (`# Title`), if the line is one.
fn heading_text(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("# ")?;
    // An ATX heading may close with more `#`.
    Some(rest.trim().trim_end_matches('#').trim_end())
}

/// The song title in a list item (`1. Title`, `2) Title`, `- Title`, `* Title`), if the line is
/// one. Items indented four or more spaces are continuation lines, not songs.
fn list_item(line: &str) -> Option<&str> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = line.trim_start_matches(' ');
    let text = if let Some(t) = ["- ", "* ", "+ "].iter().find_map(|m| rest.strip_prefix(m)) {
        t
    } else {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        if digits == 0 || digits > 9 {
            return None;
        }
        let after = &rest[digits..];
        after
            .strip_prefix(". ")
            .or_else(|| after.strip_prefix(") "))?
    };
    let text = text.trim();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const SAMPLE: &str = "---\nband: The Band\n---\n# Friday Gig\n\n1. Hotel California\n2. Superstition\n3. Waltz for Debby\n\nSoundcheck at 18:00.\n";

    fn parse(stem: &str, text: &str) -> (SetlistDoc, Vec<Problem>) {
        let mut problems = Vec::new();
        let doc = SetlistDoc::parse(stem, text, &mut problems);
        (doc, problems)
    }

    #[test]
    fn reads_the_sample_setlist() {
        let (s, problems) = parse("friday-gig", SAMPLE);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(s.stem(), "friday-gig");
        assert_eq!(s.name(), "Friday Gig");
        assert_eq!(s.band(), Some("The Band"));
        assert_eq!(
            s.songs(),
            ["Hotel California", "Superstition", "Waltz for Debby"]
        );
    }

    #[test]
    fn untouched_files_render_byte_for_byte() {
        for text in [
            "",
            SAMPLE,
            "# Only a title",
            "no heading\n- A\n- B\n",
            "---\nband: X\nunknown: y # c\n---\n\n# T\n\n- A\n\n- B\n\nnotes\n\n- later list\n",
            "```\n1. not a song\n```\n1. real\n",
        ] {
            let (s, _) = parse("x", text);
            assert_eq!(s.render(), text, "{text:?}");
        }
    }

    #[test]
    fn name_falls_back_to_the_file_name_and_band_to_none() {
        let (s, _) = parse("wedding-set", "1. A\n2. B\n");
        assert_eq!(s.name(), "wedding-set");
        assert_eq!(s.band(), None);
        let (s, _) = parse("x", "---\nband:   \n---\n# T\n");
        assert_eq!(s.band(), None);
        let (s, _) = parse("x", "## Not level one\n# Real ##\n");
        assert_eq!(s.name(), "Real");
    }

    #[test]
    fn list_markers_and_indentation() {
        let (s, _) = parse(
            "x",
            "# T\n1. One\n2) Two\n- Three\n* Four\n+ Five\n   6. Six\n",
        );
        assert_eq!(s.songs(), ["One", "Two", "Three", "Four", "Five", "Six"]);
        let (s, _) = parse("x", "- A\n    - nested note\n- B\n");
        assert_eq!(s.songs(), ["A"], "a continuation line ends the list");
        let (s, _) = parse("x", "- [ ] task\n-nospace\n1.nospace\n- \n");
        assert_eq!(s.songs(), ["[ ] task"]);
    }

    #[test]
    fn the_first_list_is_the_song_list_and_blank_lines_between_items_are_fine() {
        let (s, _) = parse(
            "x",
            "# T\n\n1. A\n\n2. B\n\nNotes:\n- bring cables\n- spare strings\n",
        );
        assert_eq!(s.songs(), ["A", "B"]);
    }

    #[test]
    fn lists_inside_code_fences_are_not_songs() {
        let (s, _) = parse("x", "# T\n```\n1. Fake\n```\n1. Real\n");
        assert_eq!(s.songs(), ["Real"]);
    }

    #[test]
    fn editing_the_list_keeps_header_heading_and_notes() {
        let (mut s, _) = parse(
            "friday-gig",
            "---\nband: The Band\nunknown: keep # me\n---\n# Friday Gig\n\nIntro text.\n\n- A\n- B\n\nSoundcheck at 18:00.\n",
        );
        s.push_song("C");
        let out = s.render();
        assert_eq!(
            out,
            "---\nband: The Band\nunknown: keep # me\n---\n# Friday Gig\n\nIntro text.\n\n1. A\n2. B\n3. C\n\nSoundcheck at 18:00.\n"
        );
        let (again, problems) = parse("friday-gig", &out);
        assert!(problems.is_empty());
        assert_eq!(again.songs(), ["A", "B", "C"]);
    }

    #[test]
    fn reordering_removing_and_inserting() {
        let (mut s, _) = parse("x", SAMPLE);
        assert!(s.move_song(0, 2));
        assert_eq!(
            s.songs(),
            ["Superstition", "Waltz for Debby", "Hotel California"]
        );
        assert!(s.move_song(2, 0));
        assert_eq!(
            s.songs(),
            ["Hotel California", "Superstition", "Waltz for Debby"]
        );
        assert!(!s.move_song(0, 3));
        assert!(!s.move_song(3, 0));
        assert_eq!(s.remove_song(1).as_deref(), Some("Superstition"));
        assert_eq!(s.remove_song(5), None);
        s.insert_song(1, "New");
        s.insert_song(99, "Last");
        assert_eq!(
            s.songs(),
            ["Hotel California", "New", "Waltz for Debby", "Last"]
        );
        let out = s.render();
        assert!(
            out.contains("1. Hotel California\n2. New\n3. Waltz for Debby\n4. Last\n"),
            "{out}"
        );
    }

    #[test]
    fn moving_to_the_same_place_changes_nothing() {
        let (mut s, _) = parse("x", "- A\n- B\n");
        assert!(s.move_song(1, 1));
        assert!(!s.is_dirty());
        assert_eq!(s.render(), "- A\n- B\n");
    }

    #[test]
    fn renaming_and_removing_a_song_everywhere_in_the_list() {
        let (mut s, _) = parse("x", "1. A\n2. b\n3. B \n4. C\n");
        assert_eq!(s.positions_of(" B"), [1, 2]);
        assert_eq!(s.rename_song("b", "Beta"), 2);
        assert_eq!(s.songs(), ["A", "Beta", "Beta", "C"]);
        assert_eq!(s.remove_title("BETA"), 2);
        assert_eq!(s.songs(), ["A", "C"]);
        assert_eq!(s.remove_title("nothing"), 0);
        assert_eq!(s.rename_song("nothing", "x"), 0);
    }

    #[test]
    fn emptying_the_list_leaves_the_notes() {
        let (mut s, _) = parse("x", "# T\n\n1. A\n\nNotes\n");
        s.remove_song(0);
        assert_eq!(s.render(), "# T\n\nNotes\n");
    }

    #[test]
    fn name_and_band_can_be_changed() {
        let (mut s, _) = parse("x", SAMPLE);
        s.set_name("Saturday Gig");
        s.set_band(Some("The Duo"));
        let out = s.render();
        assert!(
            out.contains("# Saturday Gig\n") && !out.contains("# Friday Gig"),
            "{out}"
        );
        assert!(out.contains("band: The Duo\n"), "{out}");
        let (again, _) = parse("x", &out);
        assert_eq!(
            (again.name().as_str(), again.band()),
            ("Saturday Gig", Some("The Duo"))
        );
        let (mut s, _) = parse("x", SAMPLE);
        s.set_band(None);
        assert_eq!(parse("x", &s.render()).0.band(), None);
    }

    #[test]
    fn a_heading_is_added_when_there_is_none() {
        let (mut s, _) = parse("x", "- A\n");
        s.set_name("Gig");
        assert_eq!(s.render(), "# Gig\n\n1. A\n");
    }

    #[test]
    fn setting_the_same_name_and_band_changes_nothing() {
        let (mut s, _) = parse("x", SAMPLE);
        s.set_name("Friday Gig");
        s.set_band(Some("The Band"));
        assert!(!s.is_dirty());
        assert_eq!(s.render(), SAMPLE);
    }

    #[test]
    fn a_new_setlist_is_a_heading_and_an_optional_band() {
        let mut s = SetlistDoc::new("friday-gig", "Friday Gig", Some("The Band"));
        assert!(s.is_dirty());
        assert_eq!(s.render(), "---\nband: The Band\n---\n# Friday Gig\n");
        s.push_song("A");
        assert_eq!(
            s.render(),
            "---\nband: The Band\n---\n# Friday Gig\n\n1. A\n"
        );
        let plain = SetlistDoc::new("x", "Plain", None);
        assert_eq!(plain.render(), "# Plain\n");
    }

    #[test]
    fn a_copy_has_its_own_file_name_and_heading() {
        let (s, _) = parse("friday-gig", SAMPLE);
        let copy = s.copy_as("friday-gig-copy", "Friday Gig (copy)");
        assert_eq!(copy.stem(), "friday-gig-copy");
        assert_eq!(copy.name(), "Friday Gig (copy)");
        assert_eq!(copy.songs(), s.songs());
        assert_eq!(copy.band(), s.band());
        assert!(copy.is_dirty());
        assert_eq!(s.name(), "Friday Gig", "the original is untouched");
    }

    #[test]
    fn an_unedited_setlist_moved_to_a_new_file_name_is_written_as_it_was_read() {
        let (s, _) = parse("old", SAMPLE);
        let moved = s.with_stem("new");
        assert_eq!(moved.stem(), "new");
        assert!(moved.is_dirty());
        assert_eq!(moved.render(), SAMPLE);
    }

    #[test]
    fn titles_are_single_line() {
        let mut s = SetlistDoc::new("x", "A\nB", None);
        s.push_song("Line one\nline two");
        let out = s.render();
        assert_eq!(out, "# A B\n\n1. Line one line two\n");
    }

    proptest! {
        #[test]
        fn any_text_renders_back_exactly_and_never_panics(text in "[ -~\n#:`\\-é*+.)]{0,300}") {
            let mut problems = Vec::new();
            let s = SetlistDoc::parse("x", &text, &mut problems);
            let _ = (s.name(), s.band(), s.songs().len());
            prop_assert_eq!(s.render(), text);
        }

        #[test]
        fn edits_survive_a_write_and_read(
            start in proptest::collection::vec("[A-Za-z0-9 ']{1,12}", 0..6),
            ops in proptest::collection::vec((0u8..4, 0usize..8, "[A-Za-z0-9]{1,8}"), 0..12),
        ) {
            let mut s = SetlistDoc::new("x", "Gig", Some("Band"));
            let mut model: Vec<String> = Vec::new();
            for t in &start {
                let t = t.trim().to_string();
                if t.is_empty() { continue; }
                s.push_song(&t);
                model.push(t);
            }
            for (op, i, title) in ops {
                match op {
                    0 => { s.push_song(&title); model.push(title); }
                    1 => { if i < model.len() { s.remove_song(i); model.remove(i); } }
                    2 => { if i < model.len() { let to = (i + 1) % model.len(); s.move_song(i, to); let x = model.remove(i); model.insert(to, x); } }
                    _ => { let idx = i.min(model.len()); s.insert_song(idx, &title); model.insert(idx, title); }
                }
            }
            let text = s.render();
            let mut problems = Vec::new();
            let again = SetlistDoc::parse("x", &text, &mut problems);
            prop_assert!(problems.is_empty(), "{:?}\n{}", problems, text);
            prop_assert_eq!(again.songs(), model.as_slice());
            prop_assert_eq!(again.name(), "Gig");
            prop_assert_eq!(again.band(), Some("Band"));
        }
    }
}
