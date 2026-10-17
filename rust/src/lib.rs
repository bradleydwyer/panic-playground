//! Portable, deterministic Panic rendering and tracker playback.
//! Original executable images supply immutable data tables; no x86 is executed.
pub mod data;
pub mod engine;
pub mod music;
pub mod scenes;
pub mod vga;

#[cfg(feature = "hires")]
pub mod hd;

#[cfg(test)]
mod test_support;
