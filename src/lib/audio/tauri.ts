/**
 * Thin wrappers around Tauri IPC. All audio commands are byte-in / byte-out —
 * the web layer reads/writes files (see `files.ts`) so the same code path works
 * on desktop (paths) and mobile (content:// URIs).
 */
import { invoke } from "@tauri-apps/api/core";
import { writeTempPcm, removeTemp } from "./files";

/** True when running inside a Tauri webview (desktop or mobile). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Decode one isolated channel from the file at `path` (Rust reads it via the fs
 * plugin — works with desktop paths and Android content:// URIs). Returns:
 * `[sampleRate:u32 LE][frames:u32 LE][samples:f32 LE ...]`.
 */
export async function invokeDecodeStem(
  path: string,
  channel: "left" | "right",
  ext?: string,
): Promise<ArrayBuffer> {
  return (await invoke("decode_stem", { path, channel, ext })) as ArrayBuffer;
}

/** Stretch (if needed) + encode a rendered mix; returns the encoded file bytes. */
export async function invokeEncodeMix(
  pcm: Float32Array,
  meta: {
    format: string;
    channels: number;
    sampleRate: number;
    bitDepth: number;
    tempo?: number;
  },
): Promise<ArrayBuffer> {
  const path = await writeTempPcm(new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength));
  try {
    return (await invoke("encode_mix", { path, ...meta })) as ArrayBuffer;
  } finally {
    await removeTemp(path);
  }
}

/** Embed tags into an already-written file (desktop paths only). */
export async function invokeEmbedTags(
  path: string,
  format: string,
  tags: Record<string, string>,
): Promise<void> {
  await invoke("embed_tags", { path, format, tags });
}

/**
 * Resolve a content:// URI's real display name (Android ContentResolver).
 * Returns null off-Android or when the name can't be resolved.
 */
export async function invokeContentName(uri: string): Promise<string | null> {
  return (await invoke("content_name", { uri })) as string | null;
}

export interface AlignmentResult {
  offsetFrames: number;
  spliceAt: number;
  deltaFrames: number;
  confidence: number;
  consistent: boolean;
  spreadFrames: number;
  sampleRate: number;
}

/**
 * Measure how far apart a set's part tracks play. Rust reads and decodes the
 * files itself — only the small result crosses IPC.
 */
export async function invokeAnalyzeAlignment(
  files: { path: string; ext?: string }[],
): Promise<AlignmentResult[]> {
  return (await invoke("analyze_alignment", { files })) as AlignmentResult[];
}

/**
 * Open a `content://` URI (or path) in the system viewer on Android via a native
 * ACTION_VIEW intent. Desktop uses the opener plugin instead — the plugin's
 * `open_path` can't handle Android content URIs.
 */
export async function invokeOpenUri(uri: string, mime?: string): Promise<void> {
  await invoke("open_uri", { uri, mime });
}

/** Whether MP3 export is compiled into this build (the optional `mp3` feature). */
export async function invokeMp3Enabled(): Promise<boolean> {
  return (await invoke("mp3_enabled")) as boolean;
}

/** Whether this is a debug build (gates developer-only UI). */
export async function invokeDebugBuild(): Promise<boolean> {
  return (await invoke("debug_build")) as boolean;
}

/** Read a normalized set of tags (album, artist, title, …) from a file path/URI. */
export async function invokeReadTags(
  path: string,
  ext?: string,
): Promise<Record<string, string>> {
  return (await invoke("read_tags", { path, ext })) as Record<string, string>;
}

/**
 * Pitch-preserving time-stretch of one mono stem. Returns raw bytes:
 * `[sampleRate:u32 LE][frames:u32 LE][samples:f32 LE ...]`.
 */
export async function invokeStretchStem(
  samples: Float32Array,
  sampleRate: number,
  tempo: number,
): Promise<ArrayBuffer> {
  const path = await writeTempPcm(
    new Uint8Array(samples.buffer, samples.byteOffset, samples.byteLength),
  );
  try {
    return (await invoke("stretch_stem", { path, sampleRate, tempo })) as ArrayBuffer;
  } finally {
    await removeTemp(path);
  }
}
