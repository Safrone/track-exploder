//! Track Exploder — Tauri app crate.
//!
//! Thin native layer over [`audio_core`]. All commands are **byte-in / byte-out**
//! and never touch the filesystem directly — the web layer reads and writes
//! files (via the fs/dialog plugins), so the same code path works on desktop
//! (paths) and on mobile (content:// URIs).
//!
//! * [`decode_stem`] — decode file bytes, return one isolated channel as PCM.
//! * [`encode_mix`] — stretch (if slowed) + encode a rendered mix, return bytes.
//! * [`stretch_stem`] — pitch-preserving time-stretch of one mono stem.
//! * [`read_tags`] — read tags from file bytes.
//! * [`embed_tags`] — embed tags into an already-written file (desktop paths only).

use audio_core::{
    decode_bytes, encode_interleaved, extract_channel, BitDepth, Channel, ExportFormat,
};
use serde::Deserialize;
use tauri::ipc::{InvokeBody, Request, Response};

// --- helpers ----------------------------------------------------------------

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

fn raw_body<'a>(request: &'a Request<'_>) -> Result<&'a [u8], String> {
    match request.body() {
        InvokeBody::Raw(b) => Ok(b),
        _ => Err("expected a raw ArrayBuffer body".into()),
    }
}

fn header<'a>(request: &'a Request<'_>, name: &str) -> Option<&'a str> {
    request.headers().get(name).and_then(|v| v.to_str().ok())
}

fn bytes_to_f32(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() % 4 != 0 {
        return Err("PCM byte length is not a multiple of 4".into());
    }
    Ok(bytes
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// Prefix a header (sample_rate, frames) onto f32 samples: `[sr u32][n u32][f32…]`.
fn pcm_response(sample_rate: u32, samples: Vec<f32>) -> Response {
    let mut out = Vec::with_capacity(8 + samples.len() * 4);
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    Response::new(out)
}

// --- commands ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DecodeMeta {
    channel: String,
    #[serde(default)]
    ext: Option<String>,
}

/// Decode audio file bytes (raw body) and return one isolated channel as
/// `[sample_rate u32 LE][frames u32 LE][samples f32 LE …]`.
#[tauri::command]
async fn decode_stem(request: Request<'_>) -> Result<Response, String> {
    let meta: DecodeMeta =
        serde_json::from_str(header(&request, "x-decode-meta").ok_or("missing x-decode-meta")?)
            .map_err(|e| format!("bad decode meta: {e}"))?;
    let channel = parse_channel(&meta.channel)?;
    let bytes = raw_body(&request)?.to_vec();
    let ext = meta.ext;

    let decoded = tauri::async_runtime::spawn_blocking(move || decode_bytes(bytes, ext.as_deref()))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let sr = decoded.sample_rate;
    let samples = extract_channel(&decoded, channel);
    Ok(pcm_response(sr, samples))
}

#[derive(Debug, Deserialize)]
struct EncodeMeta {
    format: String,
    channels: u16,
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
    #[serde(rename = "bitDepth")]
    bit_depth: u32,
    #[serde(default = "default_tempo")]
    tempo: f32,
}

/// Stretch (if `tempo != 1`) and encode a rendered interleaved-f32 mix (raw body).
/// Returns the encoded file bytes for the web layer to write.
#[tauri::command]
async fn encode_mix(request: Request<'_>) -> Result<Response, String> {
    let meta: EncodeMeta =
        serde_json::from_str(header(&request, "x-encode-meta").ok_or("missing x-encode-meta")?)
            .map_err(|e| format!("bad encode meta: {e}"))?;
    let samples = bytes_to_f32(raw_body(&request)?)?;

    let format = parse_format(&meta.format)?;
    let bit_depth = match meta.bit_depth {
        16 => BitDepth::Sixteen,
        _ => BitDepth::TwentyFour,
    };
    let (channels, sample_rate, tempo) = (meta.channels, meta.sample_rate, meta.tempo);

    let encoded = tauri::async_runtime::spawn_blocking(move || {
        let samples = if (tempo - 1.0).abs() > 1e-4 {
            audio_core::time_stretch(&samples, channels, sample_rate, tempo)
        } else {
            samples
        };
        encode_interleaved(&samples, channels, sample_rate, format, bit_depth)
    })
    .await
    .map_err(|e| format!("encode task failed: {e}"))?
    .map_err(|e| e.to_string())?;

    Ok(Response::new(encoded))
}

#[derive(Debug, Deserialize)]
struct StretchMeta {
    #[serde(rename = "sampleRate")]
    sample_rate: u32,
    tempo: f32,
}

/// Pitch-preserving time-stretch of one mono stem (raw f32 body). Returns
/// `[sample_rate u32 LE][frames u32 LE][samples f32 LE …]`.
#[tauri::command]
async fn stretch_stem(request: Request<'_>) -> Result<Response, String> {
    let meta: StretchMeta =
        serde_json::from_str(header(&request, "x-stretch-meta").ok_or("missing x-stretch-meta")?)
            .map_err(|e| format!("bad stretch meta: {e}"))?;
    let input = bytes_to_f32(raw_body(&request)?)?;
    let (sr, tempo) = (meta.sample_rate, meta.tempo);

    let out = tauri::async_runtime::spawn_blocking(move || {
        audio_core::time_stretch(&input, 1, sr, tempo)
    })
    .await
    .map_err(|e| format!("stretch task failed: {e}"))?;
    Ok(pcm_response(sr, out))
}

/// Read a normalized set of tags from audio file bytes (raw body).
#[tauri::command]
async fn read_tags(request: Request<'_>) -> Result<audio_core::Tags, String> {
    let ext = header(&request, "x-ext").map(String::from);
    let bytes = raw_body(&request)?.to_vec();
    tauri::async_runtime::spawn_blocking(move || audio_core::read_tags_bytes(bytes, ext.as_deref()))
        .await
        .map_err(|e| format!("read_tags task failed: {e}"))?
        .map_err(|e| e.to_string())
}

/// Embed tags into an already-written file. Desktop only (real filesystem path);
/// skipped by the web layer for content:// URIs.
#[tauri::command]
async fn embed_tags(path: String, format: String, tags: audio_core::Tags) -> Result<(), String> {
    let format = parse_format(&format)?;
    tauri::async_runtime::spawn_blocking(move || {
        audio_core::write_tags(std::path::Path::new(&path), format, &tags)
    })
    .await
    .map_err(|e| format!("embed_tags task failed: {e}"))?
    .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            decode_stem,
            encode_mix,
            stretch_stem,
            read_tags,
            embed_tags
        ])
        .run(tauri::generate_context!())
        .expect("error while running Track Exploder");
}
