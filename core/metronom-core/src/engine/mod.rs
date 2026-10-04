//! Metronome engine: scheduling plus sound, rendered into a caller-provided buffer.

use std::sync::Arc;

mod controls;
mod scheduler;
mod settings;
mod synth;
mod timeline;

pub use controls::Controls;
pub use scheduler::{Beat, MAX_BEATS_PER_BAR, MAX_BPM, MIN_BPM, Scheduler, Timing};
pub use settings::{Settings, Sound};
pub use synth::{Strength, Synth};
pub use timeline::{Entry, Timeline};

/// Renders a metronome as mono `f32` audio. `render` does no allocation and takes no locks,
/// so it is safe to call from a real-time audio callback.
pub struct Engine {
    scheduler: Scheduler,
    synth: Synth,
    timeline: Arc<Timeline>,
}

impl Engine {
    pub fn new(sample_rate: f32) -> Self {
        Self::with_timeline(sample_rate, Arc::new(Timeline::new()))
    }

    /// Log every beat into `timeline`, which the UI reads to show what is being heard.
    pub fn with_timeline(sample_rate: f32, timeline: Arc<Timeline>) -> Self {
        Self {
            scheduler: Scheduler::new(f64::from(sample_rate)),
            synth: Synth::new(sample_rate),
            timeline,
        }
    }

    /// Frames rendered so far.
    pub fn position(&self) -> u64 {
        self.scheduler.position()
    }

    /// Start a fresh bar on the next frame.
    pub fn restart(&mut self) {
        self.scheduler.restart();
    }

    /// Fill `out` with the next `out.len()` frames. The first beat of every bar is the strong
    /// click, all others are normal.
    pub fn render(&mut self, out: &mut [f32], settings: &Settings) {
        out.fill(0.0);
        let Settings {
            timing,
            sound,
            volume,
        } = *settings;
        let synth = &mut self.synth;
        let timeline = &self.timeline;
        let mut cursor = 0;
        self.scheduler.advance(out.len(), timing, |beat| {
            synth.render(&mut out[cursor..beat.frame]);
            cursor = beat.frame;
            timeline.push(&beat);
            let strength = if beat.beat == 0 {
                Strength::Strong
            } else {
                Strength::Normal
            };
            synth.trigger(sound, strength);
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

    fn settings() -> Settings {
        Settings::default() // 120 BPM, 4 beats: a click every 24 000 frames
    }

    fn render_in_blocks(block: usize, total: usize, settings: &Settings) -> Vec<f32> {
        let mut engine = Engine::new(SR);
        let mut out = vec![0.0; total];
        for chunk in out.chunks_mut(block) {
            engine.render(chunk, settings);
        }
        out
    }

    fn peak(slice: &[f32]) -> f32 {
        slice.iter().fold(0.0, |m, s| m.max(s.abs()))
    }

    #[test]
    fn audio_is_identical_whatever_the_block_size() {
        let busy = Settings {
            timing: Timing::new(133.0, 5),
            sound: Sound::Rim,
            ..settings()
        };
        for settings in [settings(), busy] {
            let reference = render_in_blocks(4096, 144_000, &settings);
            for block in [1, 64, 480, 511, 1024] {
                assert_eq!(
                    render_in_blocks(block, 144_000, &settings),
                    reference,
                    "block size {block}"
                );
            }
        }
    }

    #[test]
    fn click_starts_exactly_on_the_beat_frame() {
        let out = render_in_blocks(512, 48_000, &settings());
        assert_eq!(out[0], 0.0, "the sine starts at zero phase");
        assert!(
            out[1..40].iter().any(|s| s.abs() > 0.01),
            "downbeat audible right away"
        );
        assert!(
            out[24_000 - 1].abs() < 1e-3,
            "silent just before the next beat"
        );
        assert!(
            peak(&out[24_000..24_100]) > 0.01,
            "second click begins at frame 24000"
        );
    }

    #[test]
    fn silence_between_clicks() {
        let out = render_in_blocks(512, 48_000, &settings());
        assert!(peak(&out[3_000..24_000]) == 0.0);
    }

    #[test]
    fn the_first_beat_of_the_bar_is_louder_than_the_others() {
        let out = render_in_blocks(512, 48_000, &settings());
        assert!(peak(&out[0..2_000]) > peak(&out[24_000..26_000]));
    }

    #[test]
    fn output_stays_within_range_and_volume_scales_it() {
        let loud = render_in_blocks(
            512,
            96_000,
            &Settings {
                volume: 1.0,
                ..settings()
            },
        );
        assert!(peak(&loud) <= 1.0);
        let quiet = render_in_blocks(
            512,
            96_000,
            &Settings {
                volume: 0.5,
                ..settings()
            },
        );
        assert!((peak(&quiet) - 0.5 * peak(&loud)).abs() < 1e-4);
        let silent = render_in_blocks(
            512,
            96_000,
            &Settings {
                volume: 0.0,
                ..settings()
            },
        );
        assert_eq!(peak(&silent), 0.0);
    }

    #[test]
    fn each_sound_plays_and_they_differ() {
        let renders: Vec<Vec<f32>> = Sound::ALL
            .iter()
            .map(|&sound| {
                render_in_blocks(
                    512,
                    24_000,
                    &Settings {
                        sound,
                        ..settings()
                    },
                )
            })
            .collect();
        for (i, a) in renders.iter().enumerate() {
            assert!(peak(a) > 0.1, "{:?} is audible", Sound::ALL[i]);
            for b in &renders[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn a_song_change_starts_the_new_song_on_a_strong_beat() {
        let mut engine = Engine::new(SR);
        let mut song = settings(); // 120 BPM, 4 beats
        let mut first = vec![0.0; 60_000]; // beats at 0, 24_000, 48_000 (beat 3 of the bar)
        engine.render(&mut first, &song);
        song.timing.bar_generation += 1;
        let mut second = vec![0.0; 48_000];
        engine.render(&mut second, &song);
        // Without the change, the beat at 72_000 would be beat 4 of the bar (normal). With it,
        // it is beat 1 of the new song: as loud as the very first beat.
        let normal = peak(&first[48_000..50_000]);
        let new_downbeat = peak(&second[12_000..14_000]); // frame 72_000
        let first_beat = peak(&first[0..2_000]);
        assert!(new_downbeat > normal * 1.2, "{new_downbeat} vs {normal}");
        assert!((new_downbeat - first_beat).abs() < 1e-3);
    }

    #[test]
    fn every_beat_is_logged_for_the_ui() {
        let timeline = Arc::new(Timeline::new());
        let mut engine = Engine::with_timeline(SR, Arc::clone(&timeline));
        for chunk in vec![0.0; 100_000].chunks_mut(777) {
            engine.render(chunk, &settings());
        }
        // 120 BPM: a beat every 24 000 frames, beats 0, 1, 2, 3, 0.
        let at = |frame| timeline.latest_at_or_before(frame).unwrap();
        assert_eq!((at(0).frame, at(0).beat), (0, 0));
        assert_eq!((at(24_000).frame, at(24_000).beat), (24_000, 1));
        assert_eq!((at(72_000).frame, at(72_000).beat), (72_000, 3));
        assert_eq!((at(99_999).frame, at(99_999).beat), (96_000, 0));
    }

    #[test]
    fn restart_begins_a_fresh_bar() {
        let mut engine = Engine::new(SR);
        let mut out = vec![0.0; 30_000];
        engine.render(&mut out, &settings());
        engine.restart();
        let mut again = vec![0.0; 4_000];
        engine.render(&mut again, &settings());
        assert!(
            peak(&again[1..40]) > 0.01,
            "the downbeat sounds immediately after a restart"
        );
        assert_eq!(engine.position(), 34_000);
    }
}
