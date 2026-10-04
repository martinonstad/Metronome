//! Keeping the visual flash locked to the sound.
//!
//! The audio thread renders ahead of the speaker by the output latency. The platform reports
//! one anchor — "frame `F` of the stream reaches the speaker at time `T`" — and from it we work
//! out which frame is being heard *now*, then which beat that is.

use crate::engine::Timeline;

/// The frame currently reaching the speaker, given an anchor from the audio system: stream
/// frame `anchor_frame` is presented at `anchor_nanos`, and `now_nanos` is on the same clock.
/// Negative if the stream's first frames have not been presented yet.
pub fn heard_frame(anchor_frame: i64, anchor_nanos: i64, now_nanos: i64, sample_rate: f64) -> i64 {
    let elapsed_nanos = (now_nanos - anchor_nanos) as f64;
    anchor_frame + (elapsed_nanos * sample_rate / 1.0e9).round() as i64
}

/// What the display should show right now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BeatFlash {
    /// Zero-based beat inside the bar; 0 is the accented first beat.
    pub beat: u32,
    /// Milliseconds since that beat became audible. The UI turns this into a flash that
    /// decays over time.
    pub since_ms: f32,
}

/// The beat being heard at `heard_frame` and how long ago it started, or `None` before the
/// first beat is heard.
pub fn flash_at(timeline: &Timeline, heard_frame: i64, sample_rate: f64) -> Option<BeatFlash> {
    let heard = u64::try_from(heard_frame).ok()?;
    let entry = timeline.latest_at_or_before(heard)?;
    Some(BeatFlash {
        beat: entry.beat,
        since_ms: ((heard - entry.frame) as f64 * 1_000.0 / sample_rate) as f32,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::Beat;

    const SR: f64 = 48_000.0;

    fn push(timeline: &Timeline, frame: u64, beat: u32) {
        timeline.push(&Beat {
            frame: 0,
            absolute_frame: frame,
            beat,
        });
    }

    #[test]
    fn heard_frame_follows_the_clock_from_the_anchor() {
        // Frame 48 000 reaches the speaker at t = 1 s.
        assert_eq!(
            heard_frame(48_000, 1_000_000_000, 1_000_000_000, SR),
            48_000
        );
        assert_eq!(
            heard_frame(48_000, 1_000_000_000, 1_500_000_000, SR),
            72_000
        );
        // 20.833 ms later is 1000 frames further on.
        assert_eq!(
            heard_frame(48_000, 1_000_000_000, 1_020_833_333, SR),
            49_000
        );
    }

    #[test]
    fn heard_frame_can_look_back_before_the_anchor() {
        // A UI frame slightly before the anchor time hears an earlier frame.
        assert_eq!(heard_frame(48_000, 1_000_000_000, 900_000_000, SR), 43_200);
    }

    #[test]
    fn heard_frame_is_negative_before_the_stream_is_presented() {
        assert!(heard_frame(0, 1_000_000_000, 900_000_000, SR) < 0);
    }

    #[test]
    fn flash_reports_the_beat_being_heard_and_its_age() {
        let timeline = Timeline::new();
        push(&timeline, 0, 0);
        push(&timeline, 24_000, 1);

        // 10 ms after beat 1 became audible.
        let flash = flash_at(&timeline, 24_000 + 480, SR).unwrap();
        assert_eq!(flash.beat, 1);
        assert!((flash.since_ms - 10.0).abs() < 1e-3, "{}", flash.since_ms);

        // Just before beat 1: still beat 0, nearly 500 ms old.
        let flash = flash_at(&timeline, 23_999, SR).unwrap();
        assert_eq!(flash.beat, 0);
        assert!((flash.since_ms - 499.98).abs() < 0.05, "{}", flash.since_ms);
    }

    #[test]
    fn flash_ignores_beats_the_speaker_has_not_reached() {
        // The engine has already scheduled beat 1 (frame 24 000) but only frame 20 000 is audible.
        let timeline = Timeline::new();
        push(&timeline, 0, 0);
        push(&timeline, 24_000, 1);
        assert_eq!(flash_at(&timeline, 20_000, SR).unwrap().beat, 0);
    }

    #[test]
    fn flash_is_none_before_anything_is_heard() {
        let timeline = Timeline::new();
        assert_eq!(flash_at(&timeline, 1_000, SR), None, "nothing scheduled");
        push(&timeline, 10_000, 0);
        assert_eq!(
            flash_at(&timeline, 5_000, SR),
            None,
            "first beat not yet heard"
        );
        assert_eq!(
            flash_at(&timeline, -100, SR),
            None,
            "negative frames are not heard yet"
        );
    }

    #[test]
    fn the_whole_chain_gives_the_flash_the_ear_hears() {
        // The engine has written up to frame 100 000 while the speaker is at frame 96 160 (a
        // lag of 80 ms). A beat at frame 96 000 is therefore 3.3 ms old on screen, and the beat
        // already scheduled at 120 000 must not be shown yet.
        let timeline = Timeline::new();
        push(&timeline, 72_000, 2);
        push(&timeline, 96_000, 3);
        push(&timeline, 120_000, 0);
        let anchor_nanos = 5_000_000_000;
        let heard = heard_frame(96_160, anchor_nanos, anchor_nanos, SR);
        let flash = flash_at(&timeline, heard, SR).unwrap();
        assert_eq!(flash.beat, 3);
        assert!((flash.since_ms - 3.333).abs() < 0.01, "{}", flash.since_ms);
    }
}
