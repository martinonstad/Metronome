//! `settings.md`: global settings, kept in the file's header.

use super::Problem;
use super::header::Document;
use crate::engine::Sound;

/// The version of the file format this build reads and writes.
pub const FORMAT: u32 = 2;

const TEMPLATE: &str = "---\nformat: 2\nsound: click\nvolume: 0.8\nkeep_screen_on: true\nvisual_offset_ms: 0\n---\n# Settings\nEdit the values above. Unknown keys are kept.\n";

#[derive(Clone, Debug, PartialEq)]
pub struct AppSettings {
    pub sound: Sound,
    /// 0.0–1.0.
    pub volume: f32,
    pub keep_screen_on: bool,
    /// Shifts the beat flash relative to the sound, −500–500 ms.
    pub visual_offset_ms: i32,
    /// File name (without `.md`) of the setlist to reopen on launch.
    pub last_setlist: Option<String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            sound: Sound::Click,
            volume: 0.8,
            keep_screen_on: true,
            visual_offset_ms: 0,
            last_setlist: None,
        }
    }
}

impl AppSettings {
    /// The same settings with every value inside its range.
    #[must_use]
    pub fn sanitized(self) -> Self {
        Self {
            volume: if self.volume.is_finite() {
                self.volume.clamp(0.0, 1.0)
            } else {
                0.8
            },
            visual_offset_ms: self.visual_offset_ms.clamp(-500, 500),
            last_setlist: self
                .last_setlist
                .map(|s| super::text::single_line(&s))
                .filter(|s| !s.is_empty()),
            ..self
        }
    }
}

fn sound_name(sound: Sound) -> &'static str {
    match sound {
        Sound::Click => "click",
        Sound::Wood => "wood",
        Sound::Beep => "beep",
        Sound::Rim => "rim",
    }
}

#[derive(Clone, Debug)]
pub struct SettingsDoc {
    original: String,
    doc: Document,
    current: AppSettings,
    newer_format: bool,
    dirty: bool,
}

impl SettingsDoc {
    /// Default settings. The file is only written when a setting is first changed.
    pub fn new() -> Self {
        Self {
            original: String::new(),
            doc: Document::parse(TEMPLATE, &mut Vec::new()),
            current: AppSettings::default(),
            newer_format: false,
            dirty: false,
        }
    }

    /// `text` should already be normalized (see [`super::text::normalize`]).
    pub fn parse(text: &str, problems: &mut Vec<Problem>) -> Self {
        let doc = Document::parse(text, problems);
        let mut current = AppSettings::default();
        let mut warn = |message: String| problems.push(Problem::general(message));

        if let Some(v) = doc.get("sound") {
            match Sound::ALL
                .iter()
                .find(|s| sound_name(**s).eq_ignore_ascii_case(v.trim()))
            {
                Some(&sound) => current.sound = sound,
                None => warn(format!("unknown sound \"{v}\"; the click is used")),
            }
        }
        if let Some(v) = doc.get("volume") {
            match v.trim().parse::<f32>() {
                Ok(x) if x.is_finite() => {
                    current.volume = x.clamp(0.0, 1.0);
                    if current.volume != x {
                        warn(format!("volume {x} is outside 0–1 and was clamped"));
                    }
                }
                _ => warn(format!("\"{v}\" is not a volume; 0.8 is used")),
            }
        }
        if let Some(v) = doc.get("keep_screen_on") {
            match v.trim().to_ascii_lowercase().as_str() {
                "true" => current.keep_screen_on = true,
                "false" => current.keep_screen_on = false,
                _ => warn(format!("\"{v}\" is not true or false; true is used")),
            }
        }
        if let Some(v) = doc.get("visual_offset_ms") {
            match v.trim().parse::<i32>() {
                Ok(x) => {
                    current.visual_offset_ms = x.clamp(-500, 500);
                    if current.visual_offset_ms != x {
                        warn(format!(
                            "visual offset {x} is outside −500–500 and was clamped"
                        ));
                    }
                }
                Err(_) => warn(format!(
                    "\"{v}\" is not a number of milliseconds; 0 is used"
                )),
            }
        }
        current.last_setlist = doc
            .get("last_setlist")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        let mut newer_format = false;
        if let Some(v) = doc.get("format") {
            match v.trim().parse::<u32>() {
                Ok(f) if f > FORMAT => {
                    newer_format = true;
                    warn(format!(
                        "this file is format {f}, newer than this app understands ({FORMAT}); it is read but not changed"
                    ));
                }
                Ok(_) => {}
                Err(_) => warn(format!("\"{v}\" is not a format number")),
            }
        }
        Self {
            original: text.to_string(),
            doc,
            current,
            newer_format,
            dirty: false,
        }
    }

    pub fn settings(&self) -> &AppSettings {
        &self.current
    }

    /// A file written by a newer version of the app is never overwritten.
    pub fn is_read_only(&self) -> bool {
        self.newer_format
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Change the settings. Only the values that differ are written; unknown keys, comments and
    /// the rest of the file stay as they are. Returns false (and changes nothing) for a file
    /// from a newer version.
    pub fn update(&mut self, new: AppSettings) -> bool {
        if self.newer_format {
            return false;
        }
        let new = new.sanitized();
        if new == self.current {
            return true;
        }
        let old = std::mem::replace(&mut self.current, new.clone());
        let header = self.doc.header_mut();
        if new.sound != old.sound {
            header.set("sound", sound_name(new.sound));
        }
        if new.volume != old.volume {
            header.set("volume", &new.volume.to_string());
        }
        if new.keep_screen_on != old.keep_screen_on {
            header.set("keep_screen_on", &new.keep_screen_on.to_string());
        }
        if new.visual_offset_ms != old.visual_offset_ms {
            header.set("visual_offset_ms", &new.visual_offset_ms.to_string());
        }
        if new.last_setlist != old.last_setlist {
            match &new.last_setlist {
                Some(s) => header.set("last_setlist", s),
                None => header.remove("last_setlist"),
            }
        }
        self.dirty = true;
        true
    }

    /// Replace the whole document with an imported one (see the archive module).
    pub fn replace_with(&mut self, other: &SettingsDoc) -> bool {
        if self.newer_format || other.newer_format {
            return false;
        }
        self.doc = other.doc.clone();
        self.current = other.current.clone();
        self.dirty = true;
        true
    }

    pub fn render(&self) -> String {
        if self.dirty {
            self.doc.render()
        } else {
            self.original.clone()
        }
    }
}

impl Default for SettingsDoc {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> (SettingsDoc, Vec<Problem>) {
        let mut problems = Vec::new();
        let doc = SettingsDoc::parse(text, &mut problems);
        (doc, problems)
    }

    #[test]
    fn the_template_parses_to_the_defaults() {
        let (doc, problems) = parse(TEMPLATE);
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(doc.settings(), &AppSettings::default());
        assert_eq!(SettingsDoc::new().settings(), &AppSettings::default());
    }

    #[test]
    fn reads_every_key() {
        let (doc, problems) = parse(
            "---\nformat: 2\nsound: Wood\nvolume: 0.25\nkeep_screen_on: FALSE\nvisual_offset_ms: -120\nlast_setlist: friday-gig\n---\n",
        );
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(
            doc.settings(),
            &AppSettings {
                sound: Sound::Wood,
                volume: 0.25,
                keep_screen_on: false,
                visual_offset_ms: -120,
                last_setlist: Some("friday-gig".to_string()),
            }
        );
    }

    #[test]
    fn a_file_without_a_header_means_defaults() {
        let (doc, problems) = parse("# My settings\n");
        assert!(problems.is_empty());
        assert_eq!(doc.settings(), &AppSettings::default());
    }

    #[test]
    fn bad_values_fall_back_with_warnings() {
        let (doc, problems) = parse(
            "---\nsound: gong\nvolume: loud\nkeep_screen_on: maybe\nvisual_offset_ms: soon\n---\n",
        );
        assert_eq!(doc.settings(), &AppSettings::default());
        assert_eq!(problems.len(), 4, "{problems:?}");
    }

    #[test]
    fn out_of_range_values_are_clamped_with_warnings() {
        let (doc, problems) = parse("---\nvolume: 3\nvisual_offset_ms: 9000\n---\n");
        assert_eq!(
            (doc.settings().volume, doc.settings().visual_offset_ms),
            (1.0, 500)
        );
        assert_eq!(problems.len(), 2);
    }

    #[test]
    fn changing_one_setting_writes_only_that_key() {
        let text = "---\nformat: 2\nsound: click   # my favourite\nvolume: 0.8\nmystery: keep me\n---\n# Settings\nNotes.\n";
        let (mut doc, _) = parse(text);
        let mut s = doc.settings().clone();
        s.volume = 0.5;
        assert!(doc.update(s));
        assert_eq!(
            doc.render(),
            "---\nformat: 2\nsound: click   # my favourite\nvolume: 0.5\nmystery: keep me\n---\n# Settings\nNotes.\n"
        );
    }

    #[test]
    fn changing_a_setting_that_is_absent_adds_it() {
        let (mut doc, _) = parse("---\nsound: click\n---\n");
        let mut s = doc.settings().clone();
        s.last_setlist = Some("wedding-set".to_string());
        s.sound = Sound::Rim;
        doc.update(s);
        let out = doc.render();
        assert!(
            out.contains("sound: rim\n") && out.contains("last_setlist: wedding-set\n"),
            "{out}"
        );
        let (again, problems) = parse(&out);
        assert!(problems.is_empty());
        assert_eq!(
            again.settings().last_setlist.as_deref(),
            Some("wedding-set")
        );
        // Clearing it removes the key again.
        let mut s = again.settings().clone();
        s.last_setlist = None;
        let mut again = again;
        again.update(s);
        assert!(!again.render().contains("last_setlist"));
    }

    #[test]
    fn an_unchanged_update_leaves_the_file_alone() {
        let (mut doc, _) = parse(TEMPLATE);
        let s = doc.settings().clone();
        assert!(doc.update(s));
        assert!(!doc.is_dirty());
        assert_eq!(doc.render(), TEMPLATE);
    }

    #[test]
    fn a_new_file_is_written_from_the_template_with_the_change_applied() {
        let mut doc = SettingsDoc::new();
        assert_eq!(doc.render(), "", "nothing exists until something changes");
        let mut s = doc.settings().clone();
        s.sound = Sound::Beep;
        doc.update(s);
        let out = doc.render();
        assert!(
            out.starts_with("---\nformat: 2\nsound: beep\nvolume: 0.8\n"),
            "{out}"
        );
        assert!(out.contains("# Settings"));
    }

    #[test]
    fn updates_are_made_safe() {
        let (mut doc, _) = parse(TEMPLATE);
        let mut s = doc.settings().clone();
        s.volume = f32::NAN;
        s.visual_offset_ms = 99_999;
        s.last_setlist = Some("  ".to_string());
        doc.update(s);
        assert_eq!(
            (doc.settings().volume, doc.settings().visual_offset_ms),
            (0.8, 500)
        );
        assert_eq!(doc.settings().last_setlist, None);
    }

    #[test]
    fn a_file_from_a_newer_version_is_never_changed() {
        let text = "---\nformat: 3\nsound: wood\nfuture_key: x\n---\n";
        let (mut doc, problems) = parse(text);
        assert!(doc.is_read_only());
        assert!(problems.iter().any(|p| p.message.contains("newer")));
        assert_eq!(doc.settings().sound, Sound::Wood, "it is still read");
        let mut s = doc.settings().clone();
        s.sound = Sound::Click;
        assert!(!doc.update(s));
        assert_eq!(doc.render(), text);
    }
}
