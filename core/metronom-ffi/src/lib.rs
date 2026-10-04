//! What the app UIs talk to: a UniFFI-exported `Metronome` plus the platform audio output.

uniffi::setup_scaffolding!();

mod audio;

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use audio::Output;
use metronom_core::engine::Controls;

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

#[derive(uniffi::Object)]
pub struct Metronome {
    controls: Arc<Controls>,
    output: Mutex<Option<Output>>,
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
            output: Mutex::new(None),
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
        let opened = Output::open(Arc::clone(&self.controls), sample_rate)
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

    pub fn bpm(&self) -> f64 {
        self.controls.bpm()
    }

    pub fn set_bpm(&self, bpm: f64) {
        self.controls.set_bpm(bpm);
    }

    pub fn beats_per_bar(&self) -> u32 {
        self.controls.beats_per_bar()
    }

    pub fn set_beats_per_bar(&self, beats: u32) {
        self.controls.set_beats_per_bar(beats);
    }

    pub fn volume(&self) -> f32 {
        self.controls.volume()
    }

    pub fn set_volume(&self, volume: f32) {
        self.controls.set_volume(volume);
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
        assert_eq!((m.bpm(), m.beats_per_bar(), m.volume()), (93.0, 7, 0.25));
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
