//! Platform-independent core of the Metronom app.
//!
//! Nothing in this crate talks to the OS: the engine renders audio into a buffer it is handed,
//! so it can be tested on the host without a phone.

#![forbid(unsafe_code)]

pub mod engine;
pub mod tuning;
