//! The optional `---` header at the top of a file: `key: value` lines that can be read and
//! edited without disturbing anything else in the file.
//!
//! Invariant: for any text, `Document::parse(text).render() == text`. Unknown keys, comments,
//! odd spacing and the delimiter lines all survive exactly as written; only the entries that
//! are explicitly changed are rewritten.

use super::Problem;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    key: String,
    value: String,
    comment: Option<String>,
    /// The original line, kept until the entry is changed.
    raw: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Line {
    Entry(Entry),
    /// A blank line, a comment, or something that is not `key: value`; kept as written.
    Other(String),
}

/// The `key: value` lines between the two `---` lines.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Header {
    lines: Vec<Line>,
}

impl Header {
    fn parse(lines: &[&str], first_line_number: usize, problems: &mut Vec<Problem>) -> Self {
        let mut parsed: Vec<Line> = Vec::with_capacity(lines.len());
        for (i, raw) in lines.iter().enumerate() {
            let line_number = first_line_number + i;
            match parse_entry(raw) {
                Some(mut entry) => {
                    let duplicate = parsed.iter().any(
                        |l| matches!(l, Line::Entry(e) if e.key.eq_ignore_ascii_case(&entry.key)),
                    );
                    if duplicate {
                        problems.push(Problem::at(
                            line_number,
                            format!("duplicate key `{}`: the last one wins", entry.key),
                        ));
                    }
                    entry.raw = Some((*raw).to_string());
                    parsed.push(Line::Entry(entry));
                }
                None => {
                    let trimmed = raw.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        problems.push(Problem::at(
                            line_number,
                            "this header line is not `key: value`; it is kept as written"
                                .to_string(),
                        ));
                    }
                    parsed.push(Line::Other((*raw).to_string()));
                }
            }
        }
        Self { lines: parsed }
    }

    /// The value of `key` (case-insensitive); the last occurrence wins.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().rev().find_map(|line| match line {
            Line::Entry(e) if e.key.eq_ignore_ascii_case(key) => Some(e.value.as_str()),
            _ => None,
        })
    }

    /// Set `key` to `value`. An existing entry keeps its position, spelling and trailing
    /// comment; a new one is added at the end. Setting the value it already has changes nothing.
    pub fn set(&mut self, key: &str, value: &str) {
        // Line breaks and other control characters would corrupt the file; spaces are kept.
        let value: String = value
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let existing = self.lines.iter_mut().rev().find_map(|line| match line {
            Line::Entry(e) if e.key.eq_ignore_ascii_case(key) => Some(e),
            _ => None,
        });
        match existing {
            Some(entry) if entry.value == value => {}
            Some(entry) => {
                entry.value = value;
                entry.raw = None;
            }
            None => self.lines.push(Line::Entry(Entry {
                key: key.to_string(),
                value,
                comment: None,
                raw: None,
            })),
        }
    }

    /// Remove every entry for `key`.
    pub fn remove(&mut self, key: &str) {
        self.lines
            .retain(|line| !matches!(line, Line::Entry(e) if e.key.eq_ignore_ascii_case(key)));
    }

    fn render_lines(&self) -> impl Iterator<Item = String> + '_ {
        self.lines.iter().map(|line| match line {
            Line::Other(raw) => raw.clone(),
            Line::Entry(e) => match &e.raw {
                Some(raw) => raw.clone(),
                None => {
                    let mut out = format!("{}: {}", e.key, quote_if_needed(&e.value));
                    if let Some(comment) = &e.comment {
                        out.push_str("  ");
                        out.push_str(comment);
                    }
                    out
                }
            },
        })
    }
}

fn parse_entry(line: &str) -> Option<Entry> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let (key, rest) = line.split_once(':')?;
    let key = key.trim();
    if key.is_empty()
        || !key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return None;
    }
    let rest = rest.trim_start();

    let (value, comment) = if rest.starts_with('"') {
        match parse_quoted(rest) {
            Some((value, after)) => {
                let after = after.trim();
                let comment = after.starts_with('#').then(|| after.to_string());
                (value, comment)
            }
            // An unterminated quote: take the rest of the line as it is.
            None => bare(rest),
        }
    } else {
        bare(rest)
    };
    Some(Entry {
        key: key.to_string(),
        value,
        comment,
        raw: None,
    })
}

/// A bare value ends at the first `#` that starts the value or follows whitespace.
fn bare(rest: &str) -> (String, Option<String>) {
    let cut = rest
        .char_indices()
        .find(|&(i, c)| c == '#' && (i == 0 || rest[..i].ends_with(char::is_whitespace)))
        .map(|(i, _)| i);
    match cut {
        Some(i) => (
            rest[..i].trim().to_string(),
            Some(rest[i..].trim_end().to_string()),
        ),
        None => (rest.trim().to_string(), None),
    }
}

/// Parse `"…"` at the start of `s`, understanding `\"` and `\\`; returns the value and what
/// follows the closing quote.
fn parse_quoted(s: &str) -> Option<(String, &str)> {
    let mut value = String::new();
    let mut chars = s[1..].char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, escaped @ ('"' | '\\'))) => value.push(escaped),
                Some((_, other)) => {
                    value.push('\\');
                    value.push(other);
                }
                None => value.push('\\'),
            },
            '"' => return Some((value, &s[1 + i + 1..])),
            _ => value.push(c),
        }
    }
    None
}

fn quote_if_needed(value: &str) -> String {
    let needs_quotes = value.contains('#') || value.starts_with('"') || value != value.trim();
    if !needs_quotes {
        return value.to_string();
    }
    let mut out = String::from("\"");
    for c in value.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// A file split into its optional header and everything after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Document {
    header: Option<Header>,
    open: String,
    close: String,
    close_has_newline: bool,
    /// Everything after the header, exactly as written.
    pub body: String,
}

impl Document {
    /// `text` should already be normalized (see [`super::text::normalize`]).
    pub fn parse(text: &str, problems: &mut Vec<Problem>) -> Self {
        let lines: Vec<&str> = text.split('\n').collect();
        if lines.first().is_some_and(|l| l.trim_end() == "---") {
            if let Some(close) = (1..lines.len()).find(|&j| lines[j].trim_end() == "---") {
                let header = Header::parse(&lines[1..close], 2, problems);
                let close_has_newline = close + 1 < lines.len();
                let body = if close_has_newline {
                    lines[close + 1..].join("\n")
                } else {
                    String::new()
                };
                return Self {
                    header: Some(header),
                    open: lines[0].to_string(),
                    close: lines[close].to_string(),
                    close_has_newline,
                    body,
                };
            }
            problems.push(Problem::at(
                1,
                "the header starting on this line is never closed with `---`; the whole file is treated as text".to_string(),
            ));
        }
        Self {
            header: None,
            open: "---".to_string(),
            close: "---".to_string(),
            close_has_newline: true,
            body: text.to_string(),
        }
    }

    /// The header, if the file has one.
    pub fn header(&self) -> Option<&Header> {
        self.header.as_ref()
    }

    /// The header, creating an empty one first if the file has none.
    pub fn header_mut(&mut self) -> &mut Header {
        self.header.get_or_insert_with(Header::default)
    }

    /// The value of `key` in the header, if both exist.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.header.as_ref()?.get(key)
    }

    pub fn render(&self) -> String {
        let mut out = String::new();
        if let Some(header) = &self.header {
            out.push_str(&self.open);
            out.push('\n');
            for line in header.render_lines() {
                out.push_str(&line);
                out.push('\n');
            }
            out.push_str(&self.close);
            if self.close_has_newline {
                out.push('\n');
            }
        }
        out.push_str(&self.body);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn parse(text: &str) -> (Document, Vec<Problem>) {
        let mut problems = Vec::new();
        let doc = Document::parse(text, &mut problems);
        (doc, problems)
    }

    #[test]
    fn reads_bare_quoted_and_commented_values() {
        let (doc, problems) = parse(
            "---\nsound: wood\nvolume: 0.8   # quiet\nband: \"The # Band\"\nname: \"a \\\"quoted\\\" \\\\ name\"\nempty:\n---\nbody\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(doc.get("sound"), Some("wood"));
        assert_eq!(doc.get("volume"), Some("0.8"));
        assert_eq!(doc.get("band"), Some("The # Band"));
        assert_eq!(doc.get("name"), Some("a \"quoted\" \\ name"));
        assert_eq!(doc.get("empty"), Some(""));
        assert_eq!(doc.get("missing"), None);
        assert_eq!(doc.body, "body\n");
    }

    #[test]
    fn keys_are_case_insensitive_and_the_last_duplicate_wins() {
        let (doc, problems) = parse("---\nBand: First\nband: Second\n---\n");
        assert_eq!(doc.get("BAND"), Some("Second"));
        assert_eq!(problems.len(), 1);
        assert!(problems[0].message.contains("duplicate"));
        assert_eq!(problems[0].line, Some(3));
    }

    #[test]
    fn a_file_without_a_header_is_all_body() {
        let (doc, problems) = parse("# Songs\n\ntext\n");
        assert!(problems.is_empty());
        assert!(doc.header().is_none());
        assert_eq!(doc.body, "# Songs\n\ntext\n");
        assert_eq!(doc.get("anything"), None);
    }

    #[test]
    fn an_unclosed_header_is_treated_as_text_with_a_warning() {
        let (doc, problems) = parse("---\nband: X\n# Friday\n");
        assert!(doc.header().is_none());
        assert_eq!(doc.body, "---\nband: X\n# Friday\n");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].message.contains("never closed"));
    }

    #[test]
    fn an_empty_header_is_fine() {
        let (doc, problems) = parse("---\n---\n# Friday\n");
        assert!(problems.is_empty());
        assert!(doc.header().is_some());
        assert_eq!(doc.body, "# Friday\n");
    }

    #[test]
    fn lines_that_are_not_entries_are_kept_and_reported() {
        let text = "---\n# a comment\n\nthis is not an entry\nok: 1\n---\n";
        let (doc, problems) = parse(text);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert_eq!(problems[0].line, Some(4));
        assert_eq!(doc.render(), text);
    }

    #[test]
    fn untouched_files_render_byte_for_byte() {
        for text in [
            "",
            "plain text",
            "---\n---",
            "---\na: 1\n---",
            "--- \nkey:   spaced   \n---  \nbody",
            "---\nunknown_key: value # note\n# comment\n---\n# Title\n\n1. A\n",
            "---\nunterminated\n",
        ] {
            let (doc, _) = parse(text);
            assert_eq!(doc.render(), text, "{text:?}");
        }
    }

    #[test]
    fn setting_a_value_keeps_position_spelling_comment_and_everything_else() {
        let (mut doc, _) = parse("---\n# top\nBand: Old   # keep me\nother: x\n---\nbody\n");
        doc.header_mut().set("band", "New");
        // The unchanged entry's comment is kept, and the unchanged lines stay byte-exact.
        assert_eq!(doc.get("band"), Some("New"));
        let rendered = doc.render();
        assert!(
            rendered.starts_with("---\n# top\nBand: New  # keep me\nother: x\n---\nbody\n"),
            "{rendered}"
        );
    }

    #[test]
    fn setting_the_same_value_changes_nothing() {
        let text = "---\nband:    The Band   # note\n---\n";
        let (mut doc, _) = parse(text);
        doc.header_mut().set("band", "The Band");
        assert_eq!(doc.render(), text);
    }

    #[test]
    fn a_new_key_is_appended_and_a_missing_header_is_created() {
        let (mut doc, _) = parse("# Friday Gig\n");
        doc.header_mut().set("band", "The Band");
        assert_eq!(doc.render(), "---\nband: The Band\n---\n# Friday Gig\n");
        doc.header_mut().set("x", "1");
        assert_eq!(
            doc.render(),
            "---\nband: The Band\nx: 1\n---\n# Friday Gig\n"
        );
    }

    #[test]
    fn values_that_need_quoting_round_trip() {
        for value in [
            "The # Band",
            "  padded  ",
            "say \"hi\"",
            "back\\slash",
            "\"starts with quote",
            "plain",
        ] {
            let (mut doc, _) = parse("---\n---\n");
            doc.header_mut().set("band", value);
            let (again, problems) = parse(&doc.render());
            assert!(problems.is_empty(), "{value:?}: {problems:?}");
            assert_eq!(
                again.get("band"),
                Some(value),
                "{value:?} -> {}",
                doc.render()
            );
        }
    }

    #[test]
    fn a_value_cannot_inject_extra_lines() {
        let (mut doc, _) = parse("---\n---\n");
        doc.header_mut().set("band", "A\nevil: 1\n---\nrest");
        let rendered = doc.render();
        assert_eq!(
            rendered.lines().filter(|l| *l == "---").count(),
            2,
            "{rendered}"
        );
        assert_eq!(parse(&rendered).0.get("evil"), None);
    }

    #[test]
    fn removing_a_key_removes_every_occurrence() {
        let (mut doc, _) = parse("---\na: 1\nb: 2\nA: 3\n---\n");
        doc.header_mut().remove("a");
        assert_eq!(doc.render(), "---\nb: 2\n---\n");
    }

    proptest! {
        #[test]
        fn any_text_renders_back_exactly(text in "[ -~\n#:\"\\\\é]{0,200}") {
            let mut problems = Vec::new();
            let doc = Document::parse(&text, &mut problems);
            prop_assert_eq!(doc.render(), text);
        }

        #[test]
        fn setting_one_key_leaves_every_other_line_alone(
            value in "[ -~]{0,20}",
            others in proptest::collection::vec("[a-z]{1,6}: [a-z0-9 ]{0,8}", 0..5),
        ) {
            let text = format!("---\nband: old\n{}\n---\nbody\n", others.join("\n"));
            let mut problems = Vec::new();
            let mut doc = Document::parse(&text, &mut problems);
            doc.header_mut().set("band", &value);
            let rendered = doc.render();
            for other in &others {
                prop_assert!(rendered.contains(other.as_str()), "{} missing from {}", other, rendered);
            }
            prop_assert!(rendered.ends_with("---\nbody\n"));
        }
    }
}
