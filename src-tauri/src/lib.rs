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
use tauri::ipc::Response;

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

/// Read a file's bytes via the fs plugin — resolves real paths on desktop and
/// content:// URIs on Android (avoids transferring the file over IPC).
fn read_file_bytes(app: &tauri::AppHandle, path: &str) -> Result<Vec<u8>, String> {
    use tauri_plugin_fs::{FilePath, FsExt};
    let fp: FilePath = path.parse().map_err(|e| format!("bad path: {e}"))?;
    app.fs().read(fp).map_err(|e| format!("read failed: {e}"))
}

fn bytes_to_f32(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if !bytes.len().is_multiple_of(4) {
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

/// Read the file at `path` (or content:// URI), decode it, and return one
/// isolated channel as `[sample_rate u32 LE][frames u32 LE][samples f32 LE …]`.
#[tauri::command]
async fn decode_stem(
    app: tauri::AppHandle,
    path: String,
    channel: String,
    ext: Option<String>,
) -> Result<Response, String> {
    let channel = parse_channel(&channel)?;
    let bytes = read_file_bytes(&app, &path)?;

    let decoded = tauri::async_runtime::spawn_blocking(move || decode_bytes(bytes, ext.as_deref()))
        .await
        .map_err(|e| format!("decode task failed: {e}"))?
        .map_err(|e| e.to_string())?;

    let sr = decoded.sample_rate;
    let samples = extract_channel(&decoded, channel);
    Ok(pcm_response(sr, samples))
}

/// Stretch (if `tempo != 1`) and encode a rendered interleaved-f32 mix. The web
/// layer writes the raw PCM to a temp file (`path`) and passes it here; we read
/// it via the fs plugin (avoids a large raw IPC body, which Android rejects).
/// Returns the encoded file bytes for the web layer to write.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn encode_mix(
    app: tauri::AppHandle,
    path: String,
    format: String,
    channels: u16,
    #[allow(non_snake_case)] sampleRate: u32,
    #[allow(non_snake_case)] bitDepth: u32,
    tempo: Option<f32>,
) -> Result<Response, String> {
    let samples = bytes_to_f32(&read_file_bytes(&app, &path)?)?;

    let format = parse_format(&format)?;
    let bit_depth = match bitDepth {
        16 => BitDepth::Sixteen,
        _ => BitDepth::TwentyFour,
    };
    let (sample_rate, tempo) = (sampleRate, tempo.unwrap_or_else(default_tempo));

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

/// Pitch-preserving time-stretch of one mono stem. The web layer writes the raw
/// f32 samples to a temp file (`path`) and passes it here. Returns
/// `[sample_rate u32 LE][frames u32 LE][samples f32 LE …]`.
#[tauri::command]
async fn stretch_stem(
    app: tauri::AppHandle,
    path: String,
    #[allow(non_snake_case)] sampleRate: u32,
    tempo: f32,
) -> Result<Response, String> {
    let input = bytes_to_f32(&read_file_bytes(&app, &path)?)?;
    let sr = sampleRate;

    let out = tauri::async_runtime::spawn_blocking(move || {
        audio_core::time_stretch(&input, 1, sr, tempo)
    })
    .await
    .map_err(|e| format!("stretch task failed: {e}"))?;
    Ok(pcm_response(sr, out))
}

/// Read a normalized set of tags from the file at `path` (or content:// URI).
#[tauri::command]
async fn read_tags(
    app: tauri::AppHandle,
    path: String,
    ext: Option<String>,
) -> Result<audio_core::Tags, String> {
    let bytes = read_file_bytes(&app, &path)?;
    tauri::async_runtime::spawn_blocking(move || audio_core::read_tags_bytes(bytes, ext.as_deref()))
        .await
        .map_err(|e| format!("read_tags task failed: {e}"))?
        .map_err(|e| e.to_string())
}

/// Resolve a file's real display name (e.g. `01 Song - TENOR.mp3`) from an
/// opaque Android `content://` URI, by querying the `ContentResolver` for
/// `OpenableColumns.DISPLAY_NAME`. Returns `None` when unavailable (and always
/// `None` off Android, where paths already carry a filename).
#[tauri::command]
async fn content_name(_uri: String) -> Result<Option<String>, String> {
    #[cfg(target_os = "android")]
    {
        let (tx, rx) = std::sync::mpsc::channel::<Option<String>>();
        wry::prelude::dispatch(move |env, activity, _webview| {
            let _ = tx.send(android_display_name(env, activity, &_uri));
        });
        tauri::async_runtime::spawn_blocking(move || rx.recv().unwrap_or(None))
            .await
            .map_err(|e| format!("content_name task failed: {e}"))
    }
    #[cfg(not(target_os = "android"))]
    {
        Ok(None)
    }
}

/// Query the Android `ContentResolver` for a content URI's `DISPLAY_NAME`.
/// Runs on the UI thread (via `wry::prelude::dispatch`) with a live JNI env.
#[cfg(target_os = "android")]
fn android_display_name(
    env: &mut jni::JNIEnv,
    activity: &jni::objects::JObject,
    uri: &str,
) -> Option<String> {
    use jni::objects::{JObject, JString};

    let juri = env.new_string(uri).ok()?;
    let uri_class = env.find_class("android/net/Uri").ok()?;
    let uri_obj = env
        .call_static_method(
            uri_class,
            "parse",
            "(Ljava/lang/String;)Landroid/net/Uri;",
            &[(&juri).into()],
        )
        .ok()?
        .l()
        .ok()?;

    let resolver = env
        .call_method(
            activity,
            "getContentResolver",
            "()Landroid/content/ContentResolver;",
            &[],
        )
        .ok()?
        .l()
        .ok()?;

    let null = JObject::null();
    let cursor = env
        .call_method(
            &resolver,
            "query",
            "(Landroid/net/Uri;[Ljava/lang/String;Ljava/lang/String;[Ljava/lang/String;Ljava/lang/String;)Landroid/database/Cursor;",
            &[(&uri_obj).into(), (&null).into(), (&null).into(), (&null).into(), (&null).into()],
        )
        .ok()?
        .l()
        .ok()?;
    if cursor.is_null() {
        return None;
    }

    let col = env.new_string("_display_name").ok()?;
    let idx = env
        .call_method(
            &cursor,
            "getColumnIndex",
            "(Ljava/lang/String;)I",
            &[(&col).into()],
        )
        .ok()?
        .i()
        .ok()?;

    let mut name = None;
    let has_row = env
        .call_method(&cursor, "moveToFirst", "()Z", &[])
        .ok()
        .and_then(|v| v.z().ok())
        .unwrap_or(false);
    if idx >= 0 && has_row {
        if let Ok(v) = env.call_method(&cursor, "getString", "(I)Ljava/lang/String;", &[idx.into()])
        {
            if let Ok(jstr) = v.l() {
                if let Ok(s) = env.get_string(&JString::from(jstr)) {
                    name = Some(s.into());
                }
            }
        }
    }
    let _ = env.call_method(&cursor, "close", "()V", &[]);
    name
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
            content_name,
            embed_tags
        ])
        .run(tauri::generate_context!())
        .expect("error while running Track Exploder");
}
