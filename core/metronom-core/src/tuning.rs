//! Output-buffer tuning: start with a comfortable buffer and grow it when the device underruns.
//!
//! A metronome is not interactive, so a few extra milliseconds of buffer are inaudible (the
//! visual flash compensates using the real output latency), while an underrun is an audible
//! glitch. The policy therefore favours stability, and only ever grows the buffer.

/// Decides when the audio buffer should grow. Pure logic: the platform layer reads the stream's
/// underrun count and applies the size this returns.
pub struct BufferTuner {
    burst: i32,
    max_size: i32,
    seen_xruns: i32,
}

impl BufferTuner {
    /// `burst` is the hardware burst in frames, `capacity` the largest buffer the stream can
    /// hold, and `max_bursts` how far the tuner may grow the buffer (in bursts).
    pub fn new(burst: i32, capacity: i32, max_bursts: i32) -> Self {
        let burst = burst.max(1);
        Self {
            burst,
            max_size: capacity.min(burst.saturating_mul(max_bursts)).max(burst),
            seen_xruns: 0,
        }
    }

    /// Buffer size to start with: `bursts` bursts, but never more than the tuner's maximum.
    pub fn initial_size(&self, bursts: i32) -> i32 {
        self.burst
            .saturating_mul(bursts)
            .clamp(self.burst, self.max_size)
    }

    /// Call once per audio callback with the stream's underrun count. When new underruns have
    /// appeared since the last call, returns the larger buffer size to request. `current_size`
    /// is only evaluated in that case, so it may be a (slightly) costly stream query.
    ///
    /// Negative counts are AAudio error codes and are ignored.
    pub fn observe(&mut self, xruns: i32, current_size: impl FnOnce() -> i32) -> Option<i32> {
        if xruns <= self.seen_xruns {
            return None;
        }
        self.seen_xruns = xruns;
        let current = current_size();
        if current >= self.max_size {
            return None;
        }
        Some(current.saturating_add(self.burst).min(self.max_size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tuner() -> BufferTuner {
        // A Pixel-like stream: 96-frame bursts, 1536-frame capacity, grow up to 12 bursts.
        BufferTuner::new(96, 1536, 12)
    }

    #[test]
    fn never_grows_without_underruns() {
        let mut t = tuner();
        for _ in 0..10_000 {
            assert_eq!(t.observe(0, || 384), None);
        }
    }

    #[test]
    fn grows_by_one_burst_when_an_underrun_appears() {
        let mut t = tuner();
        assert_eq!(t.observe(1, || 384), Some(480));
    }

    #[test]
    fn the_same_count_is_not_counted_twice() {
        let mut t = tuner();
        assert_eq!(t.observe(4, || 384), Some(480));
        assert_eq!(t.observe(4, || 480), None);
        assert_eq!(t.observe(4, || 480), None);
        assert_eq!(t.observe(5, || 480), Some(576));
    }

    #[test]
    fn a_burst_of_underruns_grows_one_step_at_a_time() {
        let mut t = tuner();
        assert_eq!(t.observe(50, || 384), Some(480));
        assert_eq!(t.observe(50, || 480), None);
    }

    #[test]
    fn stops_at_the_maximum_and_at_capacity() {
        let mut t = tuner();
        assert_eq!(t.observe(1, || 1_056), Some(1_152)); // 11 -> 12 bursts
        assert_eq!(t.observe(2, || 1_152), None); // already at the 12-burst maximum

        let mut small = BufferTuner::new(96, 500, 12); // capacity limits before max_bursts does
        assert_eq!(small.observe(1, || 480), Some(500));
        assert_eq!(small.observe(2, || 500), None);
    }

    #[test]
    fn error_codes_are_ignored() {
        let mut t = tuner();
        assert_eq!(t.observe(-899, || 384), None);
        assert_eq!(t.observe(-1, || 384), None);
    }

    #[test]
    fn current_size_is_only_queried_when_needed() {
        let mut t = tuner();
        assert_eq!(t.observe(0, || panic!("queried without underruns")), None);
    }

    #[test]
    fn initial_size_is_in_bursts_and_bounded() {
        let t = tuner();
        assert_eq!(t.initial_size(4), 384);
        assert_eq!(t.initial_size(0), 96);
        assert_eq!(t.initial_size(1_000), 1_152);
    }

    #[test]
    fn degenerate_input_does_not_panic() {
        let mut t = BufferTuner::new(0, 0, 0);
        assert_eq!(t.initial_size(4), 1);
        let _ = t.observe(1, || i32::MAX);
        let _ = BufferTuner::new(i32::MAX, i32::MAX, i32::MAX).initial_size(i32::MAX);
    }
}
