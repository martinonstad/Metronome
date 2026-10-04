//! AAudio output through the NDK.
//!
//! The data callback runs on a real-time thread owned by AAudio: it must not allocate, lock,
//! or do I/O. It only reads atomics from `Controls` and renders into preallocated buffers.

#![allow(unsafe_code)]

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};

use metronom_core::engine::{Controls, Engine};
use ndk::audio::{
    AudioCallbackResult, AudioDirection, AudioError, AudioFormat, AudioPerformanceMode,
    AudioSharingMode, AudioStream, AudioStreamBuilder,
};

const CHANNELS: usize = 2;
const DEFAULT_SAMPLE_RATE: u32 = 48_000;
/// Frames rendered per engine call; a larger AAudio buffer is processed in several chunks.
const CHUNK_FRAMES: usize = 1024;

#[derive(Default)]
struct Health {
    callbacks: AtomicU64,
    disconnected: AtomicBool,
}

pub struct Output {
    stream: AudioStream,
    health: Arc<Health>,
}

// SAFETY: AAudio streams may be used from any thread; the NDK documents the stream API as
// thread-safe. `ndk` only lacks `Send` because the stream holds a raw handle. The callbacks
// are `Send` closures and are never touched from outside the audio thread.
unsafe impl Send for Output {}

fn describe(error: AudioError) -> String {
    error.to_text().into_owned()
}

impl Output {
    /// `sample_rate` should be the device's native rate so AAudio can use its low-latency
    /// path without resampling; 0 means 48 kHz.
    pub fn open(controls: Arc<Controls>, sample_rate: u32) -> Result<Self, String> {
        let rate = if sample_rate == 0 {
            DEFAULT_SAMPLE_RATE
        } else {
            sample_rate
        };
        let health = Arc::new(Health::default());
        let callback_health = Arc::clone(&health);
        let error_health = Arc::clone(&health);

        let mut engine = Engine::new(rate as f32);
        let mut chunk = [0.0f32; CHUNK_FRAMES];

        let stream = AudioStreamBuilder::new()
            .map_err(describe)?
            .direction(AudioDirection::Output)
            .performance_mode(AudioPerformanceMode::LowLatency)
            .sharing_mode(AudioSharingMode::Shared)
            .format(AudioFormat::PCM_Float)
            .channel_count(CHANNELS as i32)
            .sample_rate(rate as i32)
            .data_callback(Box::new(move |_stream, data, frames| {
                let frames = usize::try_from(frames).unwrap_or(0);
                // SAFETY: AAudio hands us `frames` frames of `CHANNELS` interleaved f32
                // samples (the format requested above). The buffer is valid and exclusively
                // ours until this callback returns.
                let out = unsafe {
                    std::slice::from_raw_parts_mut(data.cast::<f32>(), frames * CHANNELS)
                };
                let timing = controls.timing();
                let volume = controls.volume();
                for block in out.chunks_mut(CHUNK_FRAMES * CHANNELS) {
                    let n = block.len() / CHANNELS;
                    engine.render(&mut chunk[..n], timing, volume);
                    let frames = block.as_chunks_mut::<CHANNELS>().0.iter_mut();
                    for (frame, sample) in frames.zip(&chunk[..n]) {
                        frame.fill(*sample);
                    }
                }
                callback_health.callbacks.fetch_add(1, Relaxed);
                AudioCallbackResult::Continue
            }))
            .error_callback(Box::new(move |_stream, _error| {
                // Headphones unplugged, device changed, ...: the stream is unusable. The
                // owner notices via `is_disconnected` and reopens.
                error_health.disconnected.store(true, Relaxed);
            }))
            .open_stream()
            .map_err(describe)?;

        // Two bursts is the usual low-latency starting point; `xruns` in the diagnostics
        // shows whether it needs to grow on a given device.
        let burst = stream.frames_per_burst();
        let _ = stream.set_buffer_size_in_frames(burst * 2);
        stream.request_start().map_err(describe)?;

        Ok(Self { stream, health })
    }

    pub fn is_disconnected(&self) -> bool {
        self.health.disconnected.load(Relaxed)
    }

    pub fn diagnostics(&self) -> String {
        let s = &self.stream;
        format!(
            "{:?}, {:?}, {} Hz, burst {}, buffer {}/{} frames, xruns {}, callbacks {}, {:?}",
            s.performance_mode(),
            s.sharing_mode(),
            s.sample_rate(),
            s.frames_per_burst(),
            s.buffer_size_in_frames(),
            s.buffer_capacity_in_frames(),
            s.x_run_count(),
            self.health.callbacks.load(Relaxed),
            s.state(),
        )
    }
}
