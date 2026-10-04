//! Metronome engine: scheduling plus sound, rendered into a caller-provided buffer.

mod controls;
mod scheduler;
mod synth;

pub use controls::{Controls, MAX_BEATS_PER_BAR};
pub use scheduler::{MAX_BPM, MIN_BPM, Pulse, Scheduler, Timing};
pub use synth::{Accent, Synth};

/// Renders a metronome as mono `f32` audio. `render` does no allocation and takes no locks,
/// so it is safe to call from a real-time audio callback.
pub struct Engine {
    scheduler: Scheduler,
    synth: Synth,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            scheduler: Scheduler::new(f64::from(sample_rate)),
            synth: Synth::new(sample_rate),
        }
    }

    /// Frames rendered so far; the UI derives the current beat from this.
    pub fn position(&self) -> u64 {
        self.scheduler.position()
    }

    pub fn restart(&mut self) {
        self.scheduler.restart();
    }

    /// Fill `out` with the next `out.len()` frames.
    pub fn render(&mut self, out: &mut [f32], timing: Timing, volume: f32) {
        out.fill(0.0);
        let synth = &mut self.synth;
        let mut cursor = 0;
        self.scheduler.advance(out.len(), timing, |pulse| {
            synth.render(&mut out[cursor..pulse.frame]);
            cursor = pulse.frame;
            synth.trigger(if pulse.beat == 0 {
                Accent::Strong
            } else {
                Accent::Normal
            });
        });
        synth.render(&mut out[cursor..]);
        for sample in out.iter_mut() {
            *sample *= volume;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;
    const TIMING: Timing = Timing {
        bpm: 120.0,
        beats_per_bar: 4,
    };

    fn render_in_blocks(block: usize, total: usize) -> Vec<f32> {
        let mut engine = Engine::new(SR);
        let mut out = vec![0.0; total];
        for chunk in out.chunks_mut(block) {
            engine.render(chunk, TIMING, 1.0);
        }
        out
    }

    fn peak(slice: &[f32]) -> f32 {
        slice.iter().fold(0.0, |m, s| m.max(s.abs()))
    }

    #[test]
    fn audio_is_identical_whatever_the_block_size() {
        let reference = render_in_blocks(4096, 96_000);
        for block in [1, 64, 480, 511, 1024] {
            assert_eq!(
                render_in_blocks(block, 96_000),
                reference,
                "block size {block}"
            );
        }
    }

    #[test]
    fn click_starts_exactly_on_the_pulse_frame() {
        let out = render_in_blocks(512, 48_000);
        assert_eq!(out[0], 0.0, "the sine starts at zero phase");
        assert!(
            out[1..40].iter().any(|s| s.abs() > 0.01),
            "downbeat click audible right away"
        );
        assert!(
            out[24_000 - 1].abs() < 1e-3,
            "silent just before the next pulse"
        );
        assert!(
            peak(&out[24_000..24_100]) > 0.01,
            "second click begins at frame 24000"
        );
    }

    #[test]
    fn silence_between_clicks() {
        let out = render_in_blocks(512, 48_000);
        assert!(peak(&out[3_000..24_000]) == 0.0);
    }

    #[test]
    fn downbeat_is_louder_than_other_beats() {
        let out = render_in_blocks(512, 48_000);
        assert!(peak(&out[0..2_000]) > peak(&out[24_000..26_000]));
    }

    #[test]
    fn output_stays_within_range_and_volume_scales_it() {
        let loud = render_in_blocks(512, 96_000);
        assert!(peak(&loud) <= 1.0);
        let mut engine = Engine::new(SR);
        let mut quiet = vec![0.0; 96_000];
        engine.render(&mut quiet, TIMING, 0.5);
        assert!((peak(&quiet) - 0.5 * peak(&loud)).abs() < 1e-4);
    }
}
