//! Sample-accurate beat scheduling. Pure timing logic: no audio, no allocation.

pub const MIN_BPM: f64 = 20.0;
pub const MAX_BPM: f64 = 400.0;

/// Everything the scheduler needs to place the next pulse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// Pulses per minute. A pulse is one note of the time signature's denominator.
    pub bpm: f64,
    /// Pulses per bar (the time signature's numerator).
    pub beats_per_bar: u32,
}

impl Timing {
    fn sanitized(self) -> Self {
        let bpm = if self.bpm.is_finite() {
            self.bpm.clamp(MIN_BPM, MAX_BPM)
        } else {
            120.0
        };
        Self {
            bpm,
            beats_per_bar: self.beats_per_bar.max(1),
        }
    }
}

/// One pulse placed inside the block that was just advanced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pulse {
    /// Frame offset inside the block passed to [`Scheduler::advance`].
    pub frame: usize,
    /// Frame index counted from the start of the stream.
    pub absolute_frame: u64,
    /// Zero-based beat inside the bar; 0 is the downbeat.
    pub beat: u32,
}

pub struct Scheduler {
    sample_rate: f64,
    /// Frames advanced so far.
    position: u64,
    /// Exact, fractional frame position of the next pulse. Kept as f64 so tempos that don't
    /// divide the sample rate evenly never accumulate rounding error.
    next_pulse: f64,
    beat: u32,
}

impl Scheduler {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            position: 0,
            next_pulse: 0.0,
            beat: 0,
        }
    }

    /// Frames advanced so far. The UI derives the current beat from this.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Start a fresh bar on the very next frame.
    pub fn restart(&mut self) {
        self.next_pulse = self.position as f64;
        self.beat = 0;
    }

    /// Advance `frames` frames, calling `on_pulse` for every pulse that starts inside them.
    ///
    /// Tempo and time signature are read when a pulse fires, so changes take effect from the
    /// next pulse onward and never produce a short or doubled beat.
    pub fn advance(&mut self, frames: usize, timing: Timing, mut on_pulse: impl FnMut(Pulse)) {
        let timing = timing.sanitized();
        let end = self.position + frames as u64;
        while self.next_pulse < end as f64 {
            // The pulse starts on the first whole frame at or after its exact time.
            let at = (self.next_pulse.ceil() as u64).max(self.position);
            on_pulse(Pulse {
                frame: (at - self.position) as usize,
                absolute_frame: at,
                beat: self.beat,
            });
            self.next_pulse += self.sample_rate * 60.0 / timing.bpm;
            self.beat = (self.beat + 1) % timing.beats_per_bar;
        }
        self.position = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;

    fn timing(bpm: f64, beats_per_bar: u32) -> Timing {
        Timing { bpm, beats_per_bar }
    }

    fn collect(block: usize, total: usize, timing: Timing) -> Vec<Pulse> {
        let mut s = Scheduler::new(SR);
        let mut pulses = Vec::new();
        let mut done = 0;
        while done < total {
            let n = block.min(total - done);
            s.advance(n, timing, |p| {
                pulses.push(Pulse {
                    frame: p.frame,
                    ..p
                })
            });
            done += n;
        }
        pulses
    }

    #[test]
    fn pulses_land_on_exact_frames_at_120_bpm() {
        let frames: Vec<u64> = collect(480, 96_000, timing(120.0, 4))
            .iter()
            .map(|p| p.absolute_frame)
            .collect();
        assert_eq!(frames, [0, 24_000, 48_000, 72_000]);
    }

    #[test]
    fn placement_does_not_depend_on_block_size() {
        let reference: Vec<u64> = collect(4096, 480_000, timing(137.0, 7))
            .iter()
            .map(|p| p.absolute_frame)
            .collect();
        for block in [1, 7, 64, 480, 513, 1024] {
            let got: Vec<u64> = collect(block, 480_000, timing(137.0, 7))
                .iter()
                .map(|p| p.absolute_frame)
                .collect();
            assert_eq!(got, reference, "block size {block}");
        }
    }

    #[test]
    fn frame_offset_matches_absolute_frame_within_block() {
        let mut s = Scheduler::new(SR);
        let t = timing(120.0, 4);
        s.advance(10_000, t, |_| {});
        let mut seen = Vec::new();
        s.advance(20_000, t, |p| seen.push(p));
        assert_eq!(
            seen,
            [Pulse {
                frame: 14_000,
                absolute_frame: 24_000,
                beat: 1
            }]
        );
    }

    #[test]
    fn no_drift_over_one_hour_at_awkward_tempo() {
        let bpm = 97.0;
        let period = SR * 60.0 / bpm;
        let total = (3600.0 * SR) as usize;
        let mut s = Scheduler::new(SR);
        let mut k = 0u64;
        let mut worst = 0.0f64;
        let mut done = 0;
        while done < total {
            let n = 512.min(total - done);
            s.advance(n, timing(bpm, 4), |p| {
                let exact = k as f64 * period;
                let err = p.absolute_frame as f64 - exact;
                worst = worst.max(err.abs());
                assert!(
                    err > -1e-3 && err < 1.0 + 1e-3,
                    "pulse {k}: off by {err} frames"
                );
                k += 1;
            });
            done += n;
        }
        assert!(k > 5_000, "expected thousands of pulses, got {k}");
        assert!(worst < 1.0 + 1e-3);
    }

    #[test]
    fn beats_cycle_through_the_bar() {
        let beats: Vec<u32> = collect(512, 24_000 * 9, timing(120.0, 3))
            .iter()
            .map(|p| p.beat)
            .collect();
        assert_eq!(beats, [0, 1, 2, 0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn tempo_change_lands_on_the_next_pulse() {
        let mut s = Scheduler::new(SR);
        let mut frames = Vec::new();
        s.advance(30_000, timing(120.0, 4), |p| frames.push(p.absolute_frame)); // 0, 24_000
        s.advance(100_000, timing(60.0, 4), |p| frames.push(p.absolute_frame));
        // The pulse at 24_000 already scheduled its successor at the old tempo (48_000);
        // everything after that uses the new 60 BPM period of 48_000 frames.
        assert_eq!(frames, [0, 24_000, 48_000, 96_000]);
    }

    #[test]
    fn restart_begins_a_new_bar_immediately() {
        let mut s = Scheduler::new(SR);
        let t = timing(120.0, 4);
        s.advance(30_000, t, |_| {});
        s.restart();
        let mut first = None;
        s.advance(1_000, t, |p| first = first.or(Some(p)));
        assert_eq!(
            first,
            Some(Pulse {
                frame: 0,
                absolute_frame: 30_000,
                beat: 0
            })
        );
    }

    #[test]
    fn out_of_range_input_is_clamped_not_fatal() {
        // NaN falls back to 120 BPM and a zero-beat bar to one beat: 2 pulses in one second.
        assert_eq!(collect(480, 48_000, timing(f64::NAN, 0)).len(), 2);
        // An absurd tempo is capped at MAX_BPM (400 BPM = at most 7 pulses in one second).
        assert_eq!(collect(480, 48_000, timing(1.0e9, 4)).len(), 7);
    }
}
