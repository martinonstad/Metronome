//! Stand-in for platforms without an audio backend yet, so the workspace still builds and
//! tests on the development machine.

use std::sync::Arc;

use metronom_core::engine::Controls;

pub struct Output;

impl Output {
    pub fn open(_controls: Arc<Controls>, _sample_rate: u32) -> Result<Self, String> {
        Err("audio output is only implemented for Android so far".to_string())
    }

    pub fn is_disconnected(&self) -> bool {
        true
    }

    pub fn diagnostics(&self) -> String {
        "no audio backend on this platform".to_string()
    }
}
