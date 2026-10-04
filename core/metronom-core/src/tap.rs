//! Tap tempo: the user taps a rhythm and the tempo follows.

use crate::engine::{MAX_BPM, MIN_BPM};

/// Taps further apart than this start a new measurement (2 s is 30 BPM, the slowest tempo).
const RESET_AFTER_NANOS: i64 = 2_000_000_000;
/// The tempo is the average over this many of the most recent taps.
const WINDOW: usize = 6;

#[derive(Default)]
pub struct TapTempo {
    taps: Vec<i64>,
}

impl TapTempo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tap at `now_nanos` (any monotonic clock). Returns the tempo once at least two
    /// taps are in the current measurement. The first tap after a pause, or one whose clock
    /// jumped backwards, starts a new measurement.
    pub fn tap(&mut self, now_nanos: i64) -> Option<f64> {
        if let Some(&last) = self.taps.last()
            && (now_nanos <= last || now_nanos - last > RESET_AFTER_NANOS)
        {
            self.taps.clear();
        }
        if self.taps.len() == WINDOW {
            self.taps.remove(0);
        }
        self.taps.push(now_nanos);

        let (first, last) = (*self.taps.first()?, *self.taps.last()?);
        let intervals = self.taps.len() - 1;
        if intervals == 0 {
            return None;
        }
        let average_nanos = (last - first) as f64 / intervals as f64;
        Some((60.0e9 / average_nanos).clamp(MIN_BPM, MAX_BPM))
    }

    pub fn reset(&mut self) {
        self.taps.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: i64 = 1_000_000;

    fn tap_all(tapper: &mut TapTempo, times_ms: &[i64]) -> Vec<Option<f64>> {
        times_ms.iter().map(|&t| tapper.tap(t * MS)).collect()
    }

    #[test]
    fn needs_two_taps_for_a_tempo() {
        let mut t = TapTempo::new();
        assert_eq!(t.tap(0), None);
        assert_eq!(t.tap(500 * MS), Some(120.0));
    }

    #[test]
    fn steady_taps_give_the_exact_tempo() {
        let mut t = TapTempo::new();
        let got = tap_all(&mut t, &[0, 500, 1_000, 1_500, 2_000]);
        assert_eq!(
            got,
            [None, Some(120.0), Some(120.0), Some(120.0), Some(120.0)]
        );
    }

    #[test]
    fn uneven_taps_are_averaged() {
        let mut t = TapTempo::new();
        // Intervals 480, 520, 490, 510 ms: mean 500 ms = 120 BPM.
        let last = tap_all(&mut t, &[0, 480, 1_000, 1_490, 2_000])
            .pop()
            .unwrap()
            .unwrap();
        assert!((last - 120.0).abs() < 1e-9, "{last}");
    }

    #[test]
    fn only_the_latest_taps_count_so_the_tempo_can_change() {
        let mut t = TapTempo::new();
        // Six taps 500 ms apart (120 BPM), then taps 400 ms apart (150 BPM).
        let slow = [0, 500, 1_000, 1_500, 2_000, 2_500];
        let fast = [2_900, 3_300, 3_700, 4_100, 4_500, 4_900];
        let got = tap_all(&mut t, &[slow.as_slice(), fast.as_slice()].concat());
        assert_eq!(got[5], Some(120.0));
        // Once the window holds only fast taps, the old rhythm no longer counts.
        assert_eq!(got[11], Some(150.0));
    }

    #[test]
    fn a_long_pause_starts_a_new_measurement() {
        let mut t = TapTempo::new();
        tap_all(&mut t, &[0, 500, 1_000]);
        assert_eq!(t.tap(10_000 * MS), None, "first tap after a pause");
        assert_eq!(t.tap(10_600 * MS), Some(100.0));
    }

    #[test]
    fn a_clock_that_goes_backwards_starts_over() {
        let mut t = TapTempo::new();
        t.tap(5_000 * MS);
        t.tap(5_500 * MS);
        assert_eq!(t.tap(1_000 * MS), None);
        assert_eq!(t.tap(1_500 * MS), Some(120.0));
        assert_eq!(
            t.tap(1_500 * MS),
            None,
            "a repeated timestamp is not a tempo"
        );
    }

    #[test]
    fn the_tempo_is_clamped_to_the_supported_range() {
        let mut t = TapTempo::new();
        assert_eq!(t.tap(0), None);
        assert_eq!(t.tap(10 * MS), Some(MAX_BPM)); // 6000 BPM asked for
        // 2 s between taps is exactly the slowest tempo; any longer is a new measurement.
        let mut slow = TapTempo::new();
        slow.tap(0);
        assert_eq!(slow.tap(2_000 * MS), Some(MIN_BPM));
        let mut too_slow = TapTempo::new();
        too_slow.tap(0);
        assert_eq!(too_slow.tap(2_001 * MS), None);
    }

    #[test]
    fn reset_forgets_the_taps() {
        let mut t = TapTempo::new();
        t.tap(0);
        t.tap(500 * MS);
        t.reset();
        assert_eq!(t.tap(1_000 * MS), None);
    }
}
