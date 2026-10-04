//! Sample-accurate beat scheduling. Pure timing logic: no audio, no allocation.

pub const MIN_BPM: f64 = 30.0;
pub const MAX_BPM: f64 = 300.0;
pub const MAX_BEATS_PER_BAR: u32 = 99;

/// Everything the scheduler needs to place the next beat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    /// Beats per minute.
    pub bpm: f64,
    /// Beats per bar; the first beat of each bar is the accented one.
    pub beats_per_bar: u32,
    /// Changing this makes the next beat beat 1 of a new bar: a song change.
    pub bar_generation: u32,
}

impl Default for Timing {
    fn default() -> Self {
        Self::new(120.0, 4)
    }
}

impl Timing {
    pub fn new(bpm: f64, beats_per_bar: u32) -> Self {
        Self {
            bpm,
            beats_per_bar,
            bar_generation: 0,
        }
    }

    fn sanitized(self) -> Self {
        let bpm = if self.bpm.is_finite() {
            self.bpm.clamp(MIN_BPM, MAX_BPM)
        } else {
            120.0
        };
        Self {
            bpm,
            beats_per_bar: self.beats_per_bar.clamp(1, MAX_BEATS_PER_BAR),
            ..self
        }
    }
}

/// One beat placed inside the block that was just advanced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Beat {
    /// Frame offset inside the block passed to [`Scheduler::advance`].
    pub frame: usize,
    /// Frame index counted from the start of the stream.
    pub absolute_frame: u64,
    /// Zero-based beat inside the bar; 0 is the accented first beat.
    pub beat: u32,
}

pub struct Scheduler {
    sample_rate: f64,
    /// Frames advanced so far.
    position: u64,
    /// Exact, fractional frame position of the next beat. Kept as f64 so tempos that don't
    /// divide the sample rate evenly never accumulate rounding error.
    next_beat: f64,
    /// The beat index the next beat will have (before wrapping at the bar length).
    beat: u32,
    generation: u32,
}

impl Scheduler {
    pub fn new(sample_rate: f64) -> Self {
        Self {
            sample_rate,
            position: 0,
            next_beat: 0.0,
            beat: 0,
            generation: 0,
        }
    }

    /// Frames advanced so far. The UI derives the current beat from this.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Start a fresh bar on the very next frame.
    pub fn restart(&mut self) {
        self.next_beat = self.position as f64;
        self.beat = 0;
    }

    /// Advance `frames` frames, calling `on_beat` for every beat that starts inside them.
    ///
    /// The tempo that applies to the gap *after* a beat is the one in force when that beat
    /// starts, so a tempo change never shortens or doubles a beat: it takes effect from the
    /// next beat on. A change of `bar_generation` makes the next beat beat 1 of a new bar.
    pub fn advance(&mut self, frames: usize, timing: Timing, mut on_beat: impl FnMut(Beat)) {
        let timing = timing.sanitized();
        let end = self.position + frames as u64;
        while self.next_beat < end as f64 {
            if timing.bar_generation != self.generation {
                self.generation = timing.bar_generation;
                self.beat = 0;
            } else if self.beat >= timing.beats_per_bar {
                // The bar ended, or the bar was shortened mid-way: never emit a beat outside it.
                self.beat = 0;
            }
            // The beat starts on the first whole frame at or after its exact time.
            let at = (self.next_beat.ceil() as u64).max(self.position);
            on_beat(Beat {
                frame: (at - self.position) as usize,
                absolute_frame: at,
                beat: self.beat,
            });
            self.next_beat += self.sample_rate * 60.0 / timing.bpm;
            self.beat += 1;
        }
        self.position = end;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48_000.0;

    fn collect(block: usize, total: usize, timing: Timing) -> Vec<Beat> {
        let mut s = Scheduler::new(SR);
        let mut beats = Vec::new();
        let mut done = 0;
        while done < total {
            let n = block.min(total - done);
            s.advance(n, timing, |b| beats.push(b));
            done += n;
        }
        beats
    }

    fn frames(beats: &[Beat]) -> Vec<u64> {
        beats.iter().map(|b| b.absolute_frame).collect()
    }

    #[test]
    fn beats_land_on_exact_frames_at_120_bpm() {
        let got = frames(&collect(480, 96_000, Timing::new(120.0, 4)));
        assert_eq!(got, [0, 24_000, 48_000, 72_000]);
    }

    #[test]
    fn placement_does_not_depend_on_block_size() {
        // `frame` is the offset inside a block, so it legitimately depends on the block size;
        // where in the stream a beat lands, and which beat it is, must not.
        let position = |beats: Vec<Beat>| -> Vec<(u64, u32)> {
            beats.iter().map(|b| (b.absolute_frame, b.beat)).collect()
        };
        for timing in [
            Timing::new(137.0, 7),
            Timing::new(97.0, 4),
            Timing::new(250.0, 99),
        ] {
            let reference = position(collect(4096, 480_000, timing));
            for block in [1, 7, 64, 480, 513, 1024] {
                assert_eq!(
                    position(collect(block, 480_000, timing)),
                    reference,
                    "block size {block}, {timing:?}"
                );
            }
        }
    }

    #[test]
    fn frame_offset_matches_absolute_frame_within_block() {
        let mut s = Scheduler::new(SR);
        let t = Timing::new(120.0, 4);
        s.advance(10_000, t, |_| {});
        let mut seen = Vec::new();
        s.advance(20_000, t, |b| seen.push(b));
        assert_eq!(
            seen,
            [Beat {
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
        let mut done = 0;
        while done < total {
            let n = 512.min(total - done);
            s.advance(n, Timing::new(bpm, 4), |b| {
                let err = b.absolute_frame as f64 - k as f64 * period;
                assert!(
                    err > -1e-3 && err < 1.0 + 1e-3,
                    "beat {k}: off by {err} frames"
                );
                k += 1;
            });
            done += n;
        }
        assert!(k > 5_000, "expected thousands of beats, got {k}");
    }

    #[test]
    fn beats_cycle_through_the_bar() {
        let beats: Vec<u32> = collect(512, 24_000 * 9, Timing::new(120.0, 3))
            .iter()
            .map(|b| b.beat)
            .collect();
        assert_eq!(beats, [0, 1, 2, 0, 1, 2, 0, 1, 2]);
    }

    #[test]
    fn a_bar_can_be_ninety_nine_beats_long() {
        let beats: Vec<u32> = collect(4096, 24_000 * 101, Timing::new(120.0, 99))
            .iter()
            .map(|b| b.beat)
            .collect();
        assert_eq!(beats.len(), 101);
        assert_eq!(beats[98], 98);
        assert_eq!(beats[99], 0, "the bar wraps after 99 beats");
        assert_eq!(beats[100], 1);
    }

    #[test]
    fn tempo_change_lands_on_the_next_beat() {
        let mut s = Scheduler::new(SR);
        let mut got = Vec::new();
        s.advance(30_000, Timing::new(120.0, 4), |b| {
            got.push(b.absolute_frame)
        }); // 0, 24_000
        s.advance(100_000, Timing::new(60.0, 4), |b| {
            got.push(b.absolute_frame)
        });
        // The beat at 24_000 already planned its successor at the old tempo (48_000);
        // every gap after the beat at 48_000 uses the new 60 BPM period of 48_000 frames.
        assert_eq!(got, [0, 24_000, 48_000, 96_000]);
    }

    #[test]
    fn a_new_bar_generation_makes_the_next_beat_beat_one() {
        let mut s = Scheduler::new(SR);
        let mut got = Vec::new();
        // Beats 0, 1, 2 of a 4-beat bar at 120 BPM: frames 0, 24_000, 48_000.
        s.advance(60_000, Timing::new(120.0, 4), |b| {
            got.push((b.absolute_frame, b.beat))
        });
        // A song change: new tempo (60 BPM), 3 beats per bar, new generation.
        let song2 = Timing {
            bar_generation: 1,
            ..Timing::new(60.0, 3)
        };
        s.advance(200_000, song2, |b| got.push((b.absolute_frame, b.beat)));
        // The next beat on the old grid (72_000) becomes beat 1 of the new song and the gap
        // after it is the new tempo (48_000 frames); then 3 beats per bar.
        assert_eq!(
            got,
            [
                (0, 0),
                (24_000, 1),
                (48_000, 2),
                (72_000, 0),
                (120_000, 1),
                (168_000, 2),
                (216_000, 0)
            ]
        );
    }

    #[test]
    fn shrinking_the_bar_mid_way_never_emits_an_out_of_range_beat() {
        let mut s = Scheduler::new(SR);
        let mut beats = Vec::new();
        s.advance(24_000 * 3 + 1, Timing::new(120.0, 4), |b| {
            beats.push(b.beat)
        }); // beats 0..=3
        s.advance(24_000 * 6, Timing::new(120.0, 3), |b| beats.push(b.beat));
        assert_eq!(&beats[..4], [0, 1, 2, 3]);
        assert!(beats.iter().skip(4).all(|&beat| beat < 3), "{beats:?}");
    }

    #[test]
    fn restart_begins_a_new_bar_immediately() {
        let mut s = Scheduler::new(SR);
        let t = Timing::new(120.0, 4);
        s.advance(30_000 + 24_000 * 4, t, |_| {});
        s.restart();
        let mut first = None;
        s.advance(1_000, t, |b| first = first.or(Some(b)));
        assert_eq!(
            first,
            Some(Beat {
                frame: 0,
                absolute_frame: 30_000 + 24_000 * 4, // where `advance` left off
                beat: 0
            })
        );
    }

    #[test]
    fn out_of_range_input_is_clamped_not_fatal() {
        // NaN falls back to 120 BPM and a zero-beat bar to one beat: 2 beats in one second.
        assert_eq!(collect(480, 48_000, Timing::new(f64::NAN, 0)).len(), 2);
        // An absurd tempo is capped at MAX_BPM (300 BPM = at most 5 beats in one second).
        assert_eq!(collect(480, 48_000, Timing::new(1.0e9, 4)).len(), 5);
        // A tempo below the minimum is raised to MIN_BPM (30 BPM = 2 s = 96 000 frames per beat).
        assert_eq!(collect(480, 96_001, Timing::new(1.0, 4)).len(), 2);
        // An absurd bar length is capped.
        let beats = collect(480, 24_000 * 120, Timing::new(120.0, 1_000));
        assert!(beats.iter().all(|b| b.beat < MAX_BEATS_PER_BAR));
    }
}
