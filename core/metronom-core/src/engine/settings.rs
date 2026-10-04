//! Settings shared between the UI and the audio thread.

use super::Timing;

/// The built-in click sounds. All are synthesized, so the app ships no audio files.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Click,
    Wood,
    Beep,
    Rim,
}

impl Sound {
    pub const ALL: [Sound; 4] = [Self::Click, Self::Wood, Self::Beep, Self::Rim];

    pub fn index(self) -> u32 {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0) as u32
    }

    /// Unknown indexes fall back to [`Sound::Click`].
    pub fn from_index(index: u32) -> Self {
        Self::ALL
            .get(index as usize)
            .copied()
            .unwrap_or(Self::Click)
    }
}

/// Everything the engine reads, captured once per audio callback.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub timing: Timing,
    pub sound: Sound,
    pub volume: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            timing: Timing::default(),
            sound: Sound::Click,
            volume: 0.8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sound_index_round_trips_and_unknown_falls_back() {
        for sound in Sound::ALL {
            assert_eq!(Sound::from_index(sound.index()), sound);
        }
        assert_eq!(Sound::from_index(99), Sound::Click);
    }

    #[test]
    fn defaults_are_a_click_at_120_bpm_in_four() {
        let s = Settings::default();
        assert_eq!(s.sound, Sound::Click);
        assert_eq!(s.timing, Timing::new(120.0, 4));
        assert!((s.volume - 0.8).abs() < 1e-6);
    }
}
