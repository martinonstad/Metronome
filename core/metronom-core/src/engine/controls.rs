//! Settings the UI changes while the metronome is playing.
//!
//! Plain atomics: the audio thread only ever loads, so it never waits on the UI.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

use super::{MAX_BPM, MIN_BPM, Timing};

pub const MAX_BEATS_PER_BAR: u32 = 16;

pub struct Controls {
    bpm: AtomicU64,
    beats_per_bar: AtomicU32,
    volume: AtomicU32,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            bpm: AtomicU64::new(120.0f64.to_bits()),
            beats_per_bar: AtomicU32::new(4),
            volume: AtomicU32::new(0.8f32.to_bits()),
        }
    }
}

impl Controls {
    pub fn bpm(&self) -> f64 {
        f64::from_bits(self.bpm.load(Relaxed))
    }

    /// Out-of-range tempos are clamped; NaN is ignored.
    pub fn set_bpm(&self, bpm: f64) {
        if bpm.is_finite() {
            self.bpm
                .store(bpm.clamp(MIN_BPM, MAX_BPM).to_bits(), Relaxed);
        }
    }

    pub fn beats_per_bar(&self) -> u32 {
        self.beats_per_bar.load(Relaxed)
    }

    pub fn set_beats_per_bar(&self, beats: u32) {
        self.beats_per_bar
            .store(beats.clamp(1, MAX_BEATS_PER_BAR), Relaxed);
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

    pub fn timing(&self) -> Timing {
        Timing {
            bpm: self.bpm(),
            beats_per_bar: self.beats_per_bar(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_120_bpm_in_four() {
        let c = Controls::default();
        assert_eq!(
            c.timing(),
            Timing {
                bpm: 120.0,
                beats_per_bar: 4
            }
        );
        assert!((c.volume() - 0.8).abs() < 1e-6);
    }

    #[test]
    fn tempo_is_clamped_and_nan_is_ignored() {
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
    fn beats_per_bar_stays_between_one_and_sixteen() {
        let c = Controls::default();
        c.set_beats_per_bar(0);
        assert_eq!(c.beats_per_bar(), 1);
        c.set_beats_per_bar(99);
        assert_eq!(c.beats_per_bar(), MAX_BEATS_PER_BAR);
        c.set_beats_per_bar(7);
        assert_eq!(c.beats_per_bar(), 7);
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
}
