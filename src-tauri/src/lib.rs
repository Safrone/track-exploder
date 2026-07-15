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
    /// Common tags from the source files to embed in the output.
    #[serde(default)]
    tags: audio_core::Tags,
    /// Playback tempo (<1 = slower). Pitch-preserving stretch applied if != 1.
    #[serde(default = "default_tempo")]
    tempo: f32,
}

fn default_tempo() -> f32 {
    1.0
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
    let channels = meta.channels;
    let sample_rate = meta.sample_rate;
    let tempo = meta.tempo;

    // Stretch (if slowed) + encode are CPU-bound; keep them off the async thread.
    let encoded = tauri::async_runtime::spawn_blocking(move || {
        let samples = if (tempo - 1.0).abs() > 1e-4 {
            audio_core::time_stretch(&samples, channels, sample_rate, tempo)
        } else {
            samples
        };
        encode_interleaved(&samples, channels, sample_rate, format, bit_depth)
    })
    .await
    .map_err(|e| format!("export task failed: {e}"))?
    .map_err(|e| e.to_string())?;

    std::fs::write(&meta.path, encoded).map_err(|e| format!("write failed: {e}"))?;

    // Embed the common source tags (best-effort per format).
    audio_core::write_tags(std::path::Path::new(&meta.path), format, &meta.tags)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct StretchMeta {
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
    /// Playback tempo (<1 = slower).
    tempo: f32,
}

/// Pitch-preserving time-stretch of one mono stem (raw f32 request body).
/// Returns `[sample_rate: u32 LE][frames: u32 LE][samples: f32 LE ...]`.
#[tauri::command]
async fn stretch_stem(request: Request<'_>) -> Result<Response, String> {
    let meta_raw = request
        .headers()
        .get("x-stretch-meta")
        .ok_or("missing x-stretch-meta header")?
        .to_str()
        .map_err(|e| format!("bad header encoding: {e}"))?;
    let meta: StretchMeta =
        serde_json::from_str(meta_raw).map_err(|e| format!("bad stretch meta: {e}"))?;

    let bytes = match request.body() {
        InvokeBody::Raw(b) => b,
        _ => return Err("stretch_stem expects a raw ArrayBuffer body".into()),
    };
    if bytes.len() % 4 != 0 {
        return Err("PCM byte length is not a multiple of 4".into());
    }
    let input: Vec<f32> = bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();

    let sr = meta.sample_rate;
    let tempo = meta.tempo;
    let out = tauri::async_runtime::spawn_blocking(move || {
        audio_core::time_stretch(&input, 1, sr, tempo)
    })
    .await
    .map_err(|e| format!("stretch task failed: {e}"))?;

    let mut resp = Vec::with_capacity(8 + out.len() * 4);
    resp.extend_from_slice(&sr.to_le_bytes());
    resp.extend_from_slice(&(out.len() as u32).to_le_bytes());
    for s in out {
        resp.extend_from_slice(&s.to_le_bytes());
    }
    Ok(Response::new(resp))
}

/// Read a normalized set of tags from an input file.
#[tauri::command]
async fn read_tags(path: String) -> Result<audio_core::Tags, String> {
    let path = PathBuf::from(path);
    tauri::async_runtime::spawn_blocking(move || audio_core::read_tags(&path))
        .await
        .map_err(|e| format!("read_tags task failed: {e}"))?
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            decode_stem,
            export_mix,
            read_tags,
            stretch_stem
        ])
        .run(tauri::generate_context!())
        .expect("error while running Track Exploder");
}
