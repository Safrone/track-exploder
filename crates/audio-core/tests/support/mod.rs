//! Shared test support.
//!
//! * [`score`] — the synthetic circle-of-fifths barbershop arrangement (with rests).
//! * [`fixtures`] — renders that score into real media files on disk.
//! * [`analysis`] — small measurement helpers (RMS, peak, pitch detection).
//!
//! The `generate_fixtures` example includes this same module, so the files the
//! tests use and the ones you can load into the app by hand are identical.

pub mod analysis;
pub mod fixtures;
pub mod score;
