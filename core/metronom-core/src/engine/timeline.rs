//! A small lock-free log of recent beats, so the UI can show *what is being heard now*.
//!
//! The audio thread appends every beat it schedules; the UI thread reads the newest beat at or
//! before the frame currently reaching the speaker. Plain atomics: neither side ever waits.

use std::sync::atomic::{
    AtomicU64,
    Ordering::{Acquire, Relaxed, Release},
};

use super::Beat;

/// How many recent beats are kept: over ten seconds even at the fastest tempo (300 BPM), far
/// longer than the output latency the UI needs to look back.
const CAPACITY: usize = 64;

const VALID: u64 = 1 << 63;
const FRAME_BITS: u32 = 44;
const FRAME_MASK: u64 = (1 << FRAME_BITS) - 1;
const BEAT_MASK: u64 = 0x7F; // beats 0..=99 fit in 7 bits

/// One logged beat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// Frame index in the output stream at which the beat starts.
    pub frame: u64,
    /// Zero-based beat inside the bar; 0 is the accented first beat.
    pub beat: u32,
}

impl Entry {
    fn pack(self) -> u64 {
        VALID | (self.frame & FRAME_MASK) | ((u64::from(self.beat) & BEAT_MASK) << FRAME_BITS)
    }

    fn unpack(word: u64) -> Option<Self> {
        if word & VALID == 0 {
            return None;
        }
        Some(Self {
            frame: word & FRAME_MASK,
            beat: ((word >> FRAME_BITS) & BEAT_MASK) as u32,
        })
    }
}

pub struct Timeline {
    /// Number of beats ever pushed; the newest is in slot `(head - 1) % CAPACITY`.
    head: AtomicU64,
    slots: [AtomicU64; CAPACITY],
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            head: AtomicU64::new(0),
            slots: [const { AtomicU64::new(0) }; CAPACITY],
        }
    }

    /// Called by the audio thread for every beat it schedules.
    pub fn push(&self, beat: &Beat) {
        let entry = Entry {
            frame: beat.absolute_frame,
            beat: beat.beat,
        };
        let head = self.head.load(Relaxed);
        self.slots[(head % CAPACITY as u64) as usize].store(entry.pack(), Relaxed);
        // Publishing the new head last makes the slot visible to readers that see it.
        self.head.store(head + 1, Release);
    }

    /// The newest beat that starts at or before `frame`, or `None` if nothing has been
    /// scheduled yet (or the frame is before the first beat).
    pub fn latest_at_or_before(&self, frame: u64) -> Option<Entry> {
        let head = self.head.load(Acquire);
        let available = head.min(CAPACITY as u64);
        (0..available)
            .map(|back| head - 1 - back)
            .filter_map(|index| {
                Entry::unpack(self.slots[(index % CAPACITY as u64) as usize].load(Relaxed))
            })
            .find(|entry| entry.frame <= frame)
    }

    /// Forget everything. Only call while no audio thread is writing, for example between
    /// streams: a new stream counts its frames from zero again.
    pub fn clear(&self) {
        self.head.store(0, Release);
        for slot in &self.slots {
            slot.store(0, Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn beat(frame: u64, beat: u32) -> Beat {
        Beat {
            frame: 0,
            absolute_frame: frame,
            beat,
        }
    }

    #[test]
    fn an_empty_timeline_knows_nothing() {
        assert_eq!(Timeline::new().latest_at_or_before(1_000_000), None);
    }

    #[test]
    fn entries_round_trip_through_the_packed_word() {
        for beat in [0, 1, 15, 98, 99] {
            let entry = Entry {
                frame: 123_456_789_012,
                beat,
            };
            assert_eq!(Entry::unpack(entry.pack()), Some(entry));
        }
        assert_eq!(Entry::unpack(0), None, "an untouched slot is empty");
    }

    #[test]
    fn finds_the_newest_beat_at_or_before_a_frame() {
        let t = Timeline::new();
        t.push(&beat(0, 0));
        t.push(&beat(24_000, 1));
        t.push(&beat(48_000, 2));

        assert_eq!(t.latest_at_or_before(0).map(|e| e.beat), Some(0));
        assert_eq!(t.latest_at_or_before(23_999).map(|e| e.beat), Some(0));
        assert_eq!(t.latest_at_or_before(24_000).map(|e| e.beat), Some(1));
        assert_eq!(t.latest_at_or_before(47_999).map(|e| e.beat), Some(1));
        assert_eq!(
            t.latest_at_or_before(u64::MAX >> 30).map(|e| e.beat),
            Some(2)
        );
    }

    #[test]
    fn a_beat_scheduled_ahead_of_what_is_heard_is_not_returned_yet() {
        // The audio thread runs ahead of the speaker by the output latency.
        let t = Timeline::new();
        t.push(&beat(0, 0));
        t.push(&beat(24_000, 1));
        assert_eq!(t.latest_at_or_before(20_000).map(|e| e.beat), Some(0));
    }

    #[test]
    fn only_the_most_recent_beats_are_kept() {
        let t = Timeline::new();
        for i in 0..200u64 {
            t.push(&beat(i * 1_000, (i % 4) as u32));
        }
        // Frame 199_000 is the newest; the oldest retained beat is 64 back, at 136_000.
        assert_eq!(
            t.latest_at_or_before(199_500).map(|e| e.frame),
            Some(199_000)
        );
        assert_eq!(t.latest_at_or_before(100_000), None);
        assert_eq!(
            t.latest_at_or_before(136_000).map(|e| e.frame),
            Some(136_000)
        );
    }

    #[test]
    fn clear_forgets_a_previous_stream() {
        let t = Timeline::new();
        t.push(&beat(5_000_000, 3));
        t.clear();
        assert_eq!(t.latest_at_or_before(u64::MAX >> 30), None);
        t.push(&beat(0, 0));
        assert_eq!(t.latest_at_or_before(100).map(|e| e.beat), Some(0));
    }
}
