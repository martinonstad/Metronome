//! What the app UIs talk to: a UniFFI-exported `Metronome` plus the platform audio output.

uniffi::setup_scaffolding!();

mod audio;

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use audio::Output;
use metronom_core::engine::{self, Controls, Timeline};
use metronom_core::sync;
use metronom_core::tap::TapTempo;

/// The built-in click sounds. Mirrors `engine::Sound` for the app UIs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum Sound {
    Click,
    Wood,
    Beep,
    Rim,
}

impl From<engine::Sound> for Sound {
    fn from(sound: engine::Sound) -> Self {
        match sound {
            engine::Sound::Click => Self::Click,
            engine::Sound::Wood => Self::Wood,
            engine::Sound::Beep => Self::Beep,
            engine::Sound::Rim => Self::Rim,
        }
    }
}

impl From<Sound> for engine::Sound {
    fn from(sound: Sound) -> Self {
        match sound {
            Sound::Click => Self::Click,
            Sound::Wood => Self::Wood,
            Sound::Beep => Self::Beep,
            Sound::Rim => Self::Rim,
        }
    }
}

/// Field names must not be `message` (or `cause`): the generated Kotlin exception class already
/// inherits those from `Throwable` and would not compile.
#[derive(Debug, uniffi::Error)]
pub enum MetronomeError {
    Audio { detail: String },
}

impl fmt::Display for MetronomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Audio { detail } => write!(f, "audio error: {detail}"),
        }
    }
}

impl std::error::Error for MetronomeError {}

/// What the display should show right now, locked to the sound that is being heard.
#[derive(Clone, Copy, Debug, PartialEq, uniffi::Record)]
pub struct BeatState {
    /// Zero-based beat inside the bar; 0 is the accented first beat.
    pub beat: u32,
    /// Milliseconds since this beat became audible; the UI decays the flash over this.
    pub since_ms: f32,
}

#[derive(uniffi::Object)]
pub struct Metronome {
    controls: Arc<Controls>,
    /// Recent beats, written by the audio thread and read by the UI to time the beat flash.
    timeline: Arc<Timeline>,
    output: Mutex<Option<Output>>,
    tap_tempo: Mutex<TapTempo>,
}

impl Metronome {
    fn output(&self) -> MutexGuard<'_, Option<Output>> {
        self.output.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[uniffi::export]
impl Metronome {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            controls: Arc::new(Controls::default()),
            timeline: Arc::new(Timeline::new()),
            output: Mutex::new(None),
            tap_tempo: Mutex::new(TapTempo::new()),
        })
    }

    /// Start playing. `sample_rate` should be the device's native output rate; 0 means 48 kHz.
    /// Calling this while already playing does nothing; after the audio device went away it
    /// reopens the stream.
    pub fn start(&self, sample_rate: u32) -> Result<(), MetronomeError> {
        let mut output = self.output();
        if output.as_ref().is_some_and(|o| !o.is_disconnected()) {
            return Ok(());
        }
        *output = None; // close a dead stream before opening a new one
        // A new stream counts its frames from zero again, so forget the old stream's beats.
        self.timeline.clear();
        let opened = Output::open(
            Arc::clone(&self.controls),
            Arc::clone(&self.timeline),
            sample_rate,
        )
        .map_err(|detail| MetronomeError::Audio { detail })?;
        *output = Some(opened);
        Ok(())
    }

    pub fn stop(&self) {
        *self.output() = None;
    }

    pub fn is_running(&self) -> bool {
        self.output().as_ref().is_some_and(|o| !o.is_disconnected())
    }

    /// Tempo in beats per minute (30–300).
    pub fn bpm(&self) -> f64 {
        self.controls.bpm()
    }

    /// Out-of-range tempos are clamped to 30–300.
    pub fn set_bpm(&self, bpm: f64) {
        self.controls.set_bpm(bpm);
    }

    /// Beats per bar (1–99). The first beat of each bar is the accented one.
    pub fn beats_per_bar(&self) -> u32 {
        self.controls.beats_per_bar()
    }

    pub fn set_beats_per_bar(&self, beats: u32) {
        self.controls.set_beats_per_bar(beats);
    }

    /// Make the next beat beat 1 of a new bar. Call this together with `set_bpm` and
    /// `set_beats_per_bar` when switching songs: the new song then starts on its accented first
    /// beat, with the new tempo applying from the next beat.
    pub fn restart_bar(&self) {
        self.controls.restart_bar();
    }

    pub fn sound(&self) -> Sound {
        self.controls.sound().into()
    }

    pub fn set_sound(&self, sound: Sound) {
        self.controls.set_sound(sound.into());
    }

    pub fn volume(&self) -> f32 {
        self.controls.volume()
    }

    pub fn set_volume(&self, volume: f32) {
        self.controls.set_volume(volume);
    }

    /// The beat being heard at `now_nanos` and how long ago it started, or `None` when nothing
    /// is playing yet. `now_nanos` must come from the monotonic clock the audio system uses
    /// (`System.nanoTime()` on Android; a Compose frame time is on that clock too). Call it once
    /// per display frame.
    pub fn visual_state(&self, now_nanos: i64) -> Option<BeatState> {
        let output = self.output();
        let output = output.as_ref()?;
        let heard = output.heard_frame(now_nanos)?;
        let flash = sync::flash_at(&self.timeline, heard, output.sample_rate())?;
        Some(BeatState {
            beat: flash.beat,
            since_ms: flash.since_ms,
        })
    }

    /// How far the sound being heard lags the audio the engine has already written, in
    /// milliseconds (the output latency), or `None` when it cannot be measured.
    pub fn output_latency_ms(&self, now_nanos: i64) -> Option<f32> {
        let output = self.output();
        let output = output.as_ref()?;
        let heard = output.heard_frame(now_nanos)?;
        let lag_frames = output.frames_written() - heard;
        Some((lag_frames as f64 * 1_000.0 / output.sample_rate()) as f32)
    }

    /// Register a tap on the tap-tempo button. Returns the new tempo once at least two taps are
    /// in the current measurement, and applies it.
    pub fn tap(&self, now_nanos: i64) -> Option<f64> {
        let bpm = self
            .tap_tempo
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .tap(now_nanos)?;
        self.controls.set_bpm(bpm);
        Some(self.controls.bpm())
    }

    /// One line describing the audio stream, for the on-device timing check.
    pub fn diagnostics(&self) -> String {
        match self.output().as_ref() {
            Some(output) => output.diagnostics(),
            None => "stopped".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_the_exported_object() {
        let m = Metronome::new();
        m.set_bpm(93.0);
        m.set_beats_per_bar(7);
        m.set_volume(0.25);
        m.set_sound(Sound::Wood);
        assert_eq!((m.bpm(), m.beats_per_bar(), m.volume()), (93.0, 7, 0.25));
        assert_eq!(m.sound(), Sound::Wood);
    }

    #[test]
    fn tempo_and_bar_length_are_clamped() {
        let m = Metronome::new();
        m.set_bpm(5.0);
        assert_eq!(m.bpm(), 30.0);
        m.set_bpm(900.0);
        assert_eq!(m.bpm(), 300.0);
        m.set_beats_per_bar(0);
        assert_eq!(m.beats_per_bar(), 1);
        m.set_beats_per_bar(500);
        assert_eq!(m.beats_per_bar(), 99);
    }

    #[test]
    fn switching_song_changes_tempo_and_bar_without_touching_the_sound() {
        let m = Metronome::new();
        m.set_sound(Sound::Beep);
        m.set_bpm(75.0);
        m.set_beats_per_bar(4);
        m.restart_bar();
        m.set_bpm(132.0);
        m.set_beats_per_bar(3);
        assert_eq!(
            (m.bpm(), m.beats_per_bar(), m.sound()),
            (132.0, 3, Sound::Beep)
        );
    }

    #[test]
    fn the_mirror_enum_converts_both_ways() {
        for sound in [Sound::Click, Sound::Wood, Sound::Beep, Sound::Rim] {
            assert_eq!(Sound::from(engine::Sound::from(sound)), sound);
        }
        assert_eq!(
            engine::Sound::ALL.len(),
            4,
            "every engine sound needs a mirror variant"
        );
    }

    #[test]
    fn tapping_sets_the_tempo_after_two_taps() {
        let m = Metronome::new();
        assert_eq!(m.tap(0), None);
        assert_eq!(m.tap(500_000_000), Some(120.0));
        assert_eq!(m.tap(1_000_000_000), Some(120.0));
        // Taps at 0, 0.5, 1.0 and 1.6 s: three intervals averaging 0.5333 s = 112.5 BPM, and
        // the tap overrides whatever tempo was set in between.
        m.set_bpm(90.0);
        let bpm = m.tap(1_600_000_000).unwrap();
        assert!((bpm - 112.5).abs() < 1e-9, "{bpm}");
        assert!((m.bpm() - 112.5).abs() < 1e-9);
    }

    #[test]
    fn nothing_to_show_while_stopped() {
        let m = Metronome::new();
        assert_eq!(m.visual_state(1_000_000_000), None);
        assert_eq!(m.output_latency_ms(1_000_000_000), None);
    }

    #[test]
    fn stopped_by_default_and_stop_is_idempotent() {
        let m = Metronome::new();
        assert!(!m.is_running());
        assert_eq!(m.diagnostics(), "stopped");
        m.stop();
        m.stop();
    }

    #[test]
    #[cfg(not(target_os = "android"))]
    fn starting_without_an_audio_backend_reports_an_error() {
        let m = Metronome::new();
        assert!(matches!(m.start(48_000), Err(MetronomeError::Audio { .. })));
        assert!(!m.is_running());
    }
}
