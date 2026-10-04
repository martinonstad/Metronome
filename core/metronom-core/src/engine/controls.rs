//! Settings the UI changes while the metronome is playing.
//!
//! Plain atomics: the audio thread only ever loads, so it never waits on the UI.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

use super::{MAX_BEATS_PER_BAR, MAX_BPM, MIN_BPM, Settings, Sound, Timing};

pub struct Controls {
    bpm: AtomicU64,
    beats_per_bar: AtomicU32,
    bar_generation: AtomicU32,
    sound: AtomicU32,
    volume: AtomicU32,
}

impl Default for Controls {
    fn default() -> Self {
        let defaults = Settings::default();
        Self {
            bpm: AtomicU64::new(defaults.timing.bpm.to_bits()),
            beats_per_bar: AtomicU32::new(defaults.timing.beats_per_bar),
            bar_generation: AtomicU32::new(0),
            sound: AtomicU32::new(defaults.sound.index()),
            volume: AtomicU32::new(defaults.volume.to_bits()),
        }
    }
}

impl Controls {
    /// Everything the engine needs, read once per audio callback.
    pub fn settings(&self) -> Settings {
        Settings {
            timing: self.timing(),
            sound: self.sound(),
            volume: self.volume(),
        }
    }

    pub fn timing(&self) -> Timing {
        Timing {
            bpm: self.bpm(),
            beats_per_bar: self.beats_per_bar(),
            bar_generation: self.bar_generation.load(Relaxed),
        }
    }

    pub fn bpm(&self) -> f64 {
        f64::from_bits(self.bpm.load(Relaxed))
    }

    /// Out-of-range tempos are clamped to 30–300; NaN is ignored.
    pub fn set_bpm(&self, bpm: f64) {
        if bpm.is_finite() {
            self.bpm
                .store(bpm.clamp(MIN_BPM, MAX_BPM).to_bits(), Relaxed);
        }
    }

    pub fn beats_per_bar(&self) -> u32 {
        self.beats_per_bar.load(Relaxed)
    }

    /// Beats per bar, clamped to 1–99. The first beat of each bar is accented.
    pub fn set_beats_per_bar(&self, beats: u32) {
        self.beats_per_bar
            .store(beats.clamp(1, MAX_BEATS_PER_BAR), Relaxed);
    }

    /// Make the next beat beat 1 of a new bar. Call this when switching to another song, so the
    /// new song starts on its accented first beat.
    pub fn restart_bar(&self) {
        self.bar_generation.fetch_add(1, Relaxed);
    }

    pub fn sound(&self) -> Sound {
        Sound::from_index(self.sound.load(Relaxed))
    }

    pub fn set_sound(&self, sound: Sound) {
        self.sound.store(sound.index(), Relaxed);
    }

    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Relaxed))
    }

    /// Clamped to 0.0..=1.0; NaN is ignored.
    pub fn set_volume(&self, volume: f32) {
        if volume.is_finite() {
            self.volume.store(volume.clamp(0.0, 1.0).to_bits(), Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_120_bpm_in_four() {
        let c = Controls::default();
        assert_eq!(c.timing(), Timing::new(120.0, 4));
        assert_eq!(c.sound(), Sound::Click);
        assert_eq!(c.settings(), Settings::default());
    }

    #[test]
    fn tempo_is_clamped_to_30_300_and_nan_is_ignored() {
        let c = Controls::default();
        c.set_bpm(5.0);
        assert_eq!(c.bpm(), MIN_BPM);
        c.set_bpm(9_000.0);
        assert_eq!(c.bpm(), MAX_BPM);
        c.set_bpm(93.5);
        c.set_bpm(f64::NAN);
        assert_eq!(c.bpm(), 93.5);
    }

    #[test]
    fn beats_per_bar_stays_between_one_and_ninety_nine() {
        let c = Controls::default();
        c.set_beats_per_bar(0);
        assert_eq!(c.beats_per_bar(), 1);
        c.set_beats_per_bar(500);
        assert_eq!(c.beats_per_bar(), MAX_BEATS_PER_BAR);
        c.set_beats_per_bar(7);
        assert_eq!(c.beats_per_bar(), 7);
    }

    #[test]
    fn restarting_the_bar_is_visible_to_the_engine() {
        let c = Controls::default();
        let before = c.timing().bar_generation;
        c.restart_bar();
        c.restart_bar();
        assert_eq!(c.timing().bar_generation, before.wrapping_add(2));
        // It changes nothing else.
        assert_eq!((c.bpm(), c.beats_per_bar()), (120.0, 4));
    }

    #[test]
    fn the_sound_can_be_chosen() {
        let c = Controls::default();
        c.set_sound(Sound::Rim);
        assert_eq!(c.sound(), Sound::Rim);
        assert_eq!(c.settings().sound, Sound::Rim);
    }

    #[test]
    fn volume_is_clamped_and_nan_is_ignored() {
        let c = Controls::default();
        c.set_volume(3.0);
        assert_eq!(c.volume(), 1.0);
        c.set_volume(-1.0);
        assert_eq!(c.volume(), 0.0);
        c.set_volume(f32::NAN);
        assert_eq!(c.volume(), 0.0);
    }

    #[test]
    fn the_snapshot_reflects_every_setting() {
        let c = Controls::default();
        c.set_bpm(90.0);
        c.set_beats_per_bar(7);
        c.set_sound(Sound::Wood);
        c.set_volume(0.5);
        let s = c.settings();
        assert_eq!(s.timing, Timing::new(90.0, 7));
        assert_eq!((s.sound, s.volume), (Sound::Wood, 0.5));
    }
}
