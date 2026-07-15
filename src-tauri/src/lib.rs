//! Track Exploder — Tauri app crate.
//!
//! This is a thin native layer over [`audio_core`]. It exposes two commands to
//! the web UI:
//!
//! * [`decode_stem`] — decode a file and return one isolated channel as raw
//!   little-endian bytes (a compact header followed by `f32` samples). Returning
//!   raw bytes avoids the cost of JSON-serializing millions of samples.
//! * [`export_mix`] — receive a rendered interleaved-`f32` mix as a raw request
//!   body plus a JSON metadata header, encode it, and write it to disk.

use std::path::PathBuf;

use audio_core::{
    decode_file, encode_interleaved, extract_channel, BitDepth, Channel, ExportFormat,
};
use serde::Deserialize;
use tauri::ipc::{InvokeBody, Request, Response};

/// Metadata sent alongside the raw PCM body of an export request (JSON in the
/// `x-export-meta` header).
#[derive(Debug, Deserialize)]
struct ExportMeta {
    /// Absolute destination path chosen via the frontend save dialog.
    path: String,
    /// "wav" | "flac" | "mp3".
    format: String,
    /// 1 (mono) or 2 (stereo).
    channels: u16,
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
    /// 16 or 24 (ignored for mp3).
    #[serde(rename = "bitDepth")]
    bit_depth: u32,
}

fn parse_channel(s: &str) -> Result<Channel, String> {
    match s.to_ascii_lowercase().as_str() {
        "left" | "l" => Ok(Channel::Left),
        "right" | "r" => Ok(Channel::Right),
        other => Err(format!("unknown channel: {other}")),
    }
}

fn parse_format(s: &str) -> Result<ExportFormat, String> {
    match s.to_ascii_lowercase().as_str() {
        "wav" => Ok(ExportFormat::Wav),
        "flac" => Ok(ExportFormat::Flac),
        #[cfg(feature = "mp3")]
        "mp3" => Ok(ExportFormat::Mp3),
        #[cfg(not(feature = "mp3"))]
        "mp3" => Err("MP3 export not enabled in this build (rebuild with --features mp3)".into()),
        other => Err(format!("unknown format: {other}")),
    }
}

/// Decode `path` and return the isolated `channel` as raw bytes:
/// `[sample_rate: u32 LE][frames: u32 LE][samples: f32 LE ...]`.
#[tauri::command]
async fn decode_stem(path: String, channel: String) -> Result<Response, String> {
    let channel = parse_channel(&channel)?;
    let path = PathBuf::from(path);

    // Decoding is CPU-bound; keep it off the main thread.
    let decoded = tauri::async_runtime::spawn_blocking(move || decode_file(&path))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let samples = extract_channel(&decoded, channel);

    let mut out = Vec::with_capacity(8 + samples.len() * 4);
    out.extend_from_slice(&decoded.sample_rate.to_le_bytes());
    out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    Ok(Response::new(out))
}

/// Encode a rendered mix (raw interleaved-`f32` request body) and write it to disk.
#[tauri::command]
async fn export_mix(request: Request<'_>) -> Result<(), String> {
    let meta_raw = request
        .headers()
        .get("x-export-meta")
        .ok_or("missing x-export-meta header")?
        .to_str()
        .map_err(|e| format!("bad header encoding: {e}"))?;
    let meta: ExportMeta =
        serde_json::from_str(meta_raw).map_err(|e| format!("bad export meta: {e}"))?;

    let bytes = match request.body() {
        InvokeBody::Raw(b) => b,
        _ => return Err("export_mix expects a raw ArrayBuffer body".into()),
    };
    if bytes.len() % 4 != 0 {
        return Err("PCM byte length is not a multiple of 4".into());
    }

    let samples: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let format = parse_format(&meta.format)?;
    let bit_depth = match meta.bit_depth {
        16 => BitDepth::Sixteen,
        _ => BitDepth::TwentyFour,
    };

    let encoded = encode_interleaved(&samples, meta.channels, meta.sample_rate, format, bit_depth)
        .map_err(|e| e.to_string())?;

    std::fs::write(&meta.path, encoded).map_err(|e| format!("write failed: {e}"))?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![decode_stem, export_mix])
        .run(tauri::generate_context!())
        .expect("error while running Track Exploder");
}
