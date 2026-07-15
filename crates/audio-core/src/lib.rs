//! Pure-Rust audio DSP for Track Exploder.
//!
//! This crate has **no GUI or Tauri dependency** so it can be unit-tested in
//! isolation (`cargo test -p audio-core`) without any webview/system libraries.
//!
//! The pipeline it supports:
//! 1. [`decode::decode_file`] — decode any supported file to per-channel PCM.
//! 2. [`extract::extract_channel`] — pull the isolated voice from one channel.
//! 3. [`encode::encode_interleaved`] — encode a rendered mix to WAV/FLAC/MP3.

pub mod decode;
pub mod encode;
pub mod extract;
pub mod metadata;

pub use decode::{decode_file, DecodeError, DecodedAudio};
pub use encode::{encode_interleaved, BitDepth, EncodeError, ExportFormat};
pub use extract::{extract_channel, Channel};
pub use metadata::{read_tags, write_tags, TagError, Tags};
