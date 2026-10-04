//! Small text helpers shared by the file parsers.

/// Line endings become `\n` and a leading byte-order mark is dropped, so files saved by editors
/// on other platforms read the same as ours.
pub fn normalize(text: &str) -> String {
    text.strip_prefix('\u{feff}')
        .unwrap_or(text)
        .replace("\r\n", "\n")
}

/// The key two titles are compared by: surrounding spaces and letter case do not matter.
pub fn title_key(title: &str) -> String {
    title.trim().to_lowercase()
}

/// One line of text: no line breaks or other control characters, surrounding spaces removed.
pub fn single_line(text: &str) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect::<String>()
        .trim()
        .to_string()
}

/// A file-name-safe version of a name: lowercase ASCII, accents folded (`é` → `e`, `ø` → `o`),
/// every run of other characters turned into `-`. Never empty.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for c in name.chars().flat_map(char::to_lowercase) {
        let folded = fold(c);
        if folded.is_empty() {
            pending_dash = !slug.is_empty();
            continue;
        }
        if pending_dash {
            slug.push('-');
            pending_dash = false;
        }
        slug.push_str(folded);
    }
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

/// ASCII letters and digits as themselves, common accented letters folded, anything else empty.
fn fold(c: char) -> &'static str {
    // Strings for the ASCII range are produced from a static table to stay `&'static`.
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    const LETTERS: [&str; 26] = [
        "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r",
        "s", "t", "u", "v", "w", "x", "y", "z",
    ];
    match c {
        '0'..='9' => DIGITS[(c as u8 - b'0') as usize],
        'a'..='z' => LETTERS[(c as u8 - b'a') as usize],
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ą' => "a",
        'æ' => "ae",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' | 'ě' => "e",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ı' => "i",
        'ł' | 'ľ' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'œ' => "oe",
        'ř' => "r",
        'ś' | 'š' | 'ş' => "s",
        'ß' => "ss",
        'ť' | 'ţ' => "t",
        'þ' => "th",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        _ => "",
    }
}

/// A slug that does not clash with any of `taken`: `name`, then `name-2`, `name-3`, …
pub fn unique_slug(base: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(base) {
        return base.to_string();
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|candidate| !taken(candidate))
        .expect("an unused suffix always exists")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_endings_and_bom_are_normalized() {
        assert_eq!(normalize("a\r\nb\r\n"), "a\nb\n");
        assert_eq!(normalize("\u{feff}# Songs\n"), "# Songs\n");
        assert_eq!(normalize("plain\n"), "plain\n");
    }

    #[test]
    fn titles_compare_ignoring_case_and_surrounding_spaces() {
        assert_eq!(
            title_key("  Hotel California "),
            title_key("hotel california")
        );
        assert_ne!(
            title_key("Hotel California"),
            title_key("Hotel  California")
        );
        assert_eq!(title_key("ÅSE"), title_key("åse"));
    }

    #[test]
    fn single_line_removes_breaks_and_controls() {
        assert_eq!(single_line("  a\nb\tc \r"), "a b c");
        assert_eq!(single_line("plain"), "plain");
    }

    #[test]
    fn slugs_are_lowercase_ascii_with_accents_folded() {
        assert_eq!(slugify("Friday Gig"), "friday-gig");
        assert_eq!(slugify("Café evening"), "cafe-evening");
        assert_eq!(
            slugify("Blåbærsyltetøy & Rømmegrøt!"),
            "blabaersyltetoy-rommegrot"
        );
        assert_eq!(slugify("  --Wedding   set-- "), "wedding-set");
        assert_eq!(slugify("Straße"), "strasse");
    }

    #[test]
    fn a_name_with_nothing_usable_still_gets_a_slug() {
        assert_eq!(slugify(""), "untitled");
        assert_eq!(slugify("!!!"), "untitled");
        assert_eq!(slugify("日本語"), "untitled");
    }

    #[test]
    fn slugs_never_contain_path_characters() {
        for name in ["../../etc/passwd", "a/b\\c", "CON:", "x\0y", "a.b.md"] {
            let slug = slugify(name);
            assert!(
                slug.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'),
                "{name:?} -> {slug:?}"
            );
            assert!(!slug.starts_with('-') && !slug.ends_with('-'));
        }
    }

    #[test]
    fn unique_slug_adds_a_numeric_suffix() {
        let taken = ["friday-gig", "friday-gig-2"];
        assert_eq!(
            unique_slug("friday-gig", |s| taken.contains(&s)),
            "friday-gig-3"
        );
        assert_eq!(unique_slug("other", |s| taken.contains(&s)), "other");
    }
}
