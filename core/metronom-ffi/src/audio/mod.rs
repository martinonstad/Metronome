//! Platform audio output. Every platform provides an `Output` with the same small surface:
//! `open`, `is_disconnected` and `diagnostics`; dropping it stops and closes the stream.

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::Output;

#[cfg(not(target_os = "android"))]
mod unsupported;
#[cfg(not(target_os = "android"))]
pub use unsupported::Output;
