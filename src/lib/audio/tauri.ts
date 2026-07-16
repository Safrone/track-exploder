/**
 * Thin wrappers around Tauri IPC. All audio commands are byte-in / byte-out —
 * the web layer reads/writes files (see `files.ts`) so the same code path works
 * on desktop (paths) and mobile (content:// URIs).
 */
import { invoke } from "@tauri-apps/api/core";

/** True when running inside a Tauri webview (desktop or mobile). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Decode one isolated channel from audio file bytes. Returns raw response:
 * `[sampleRate:u32 LE][frames:u32 LE][samples:f32 LE ...]`.
 */
export async function invokeDecodeStem(
  bytes: Uint8Array,
  channel: "left" | "right",
  ext?: string,
): Promise<ArrayBuffer> {
  return (await invoke("decode_stem", bytes, {
    headers: { "x-decode-meta": JSON.stringify({ channel, ext }) },
  })) as ArrayBuffer;
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
  return (await invoke("encode_mix", pcm.buffer as ArrayBuffer, {
    headers: { "x-encode-meta": JSON.stringify(meta) },
  })) as ArrayBuffer;
}

/** Embed tags into an already-written file (desktop paths only). */
export async function invokeEmbedTags(
  path: string,
  format: string,
  tags: Record<string, string>,
): Promise<void> {
  await invoke("embed_tags", { path, format, tags });
}

/** Read a normalized set of tags (album, artist, title, …) from file bytes. */
export async function invokeReadTags(
  bytes: Uint8Array,
  ext?: string,
): Promise<Record<string, string>> {
  return (await invoke("read_tags", bytes, {
    headers: ext ? { "x-ext": ext } : {},
  })) as Record<string, string>;
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
  return (await invoke("stretch_stem", samples.buffer as ArrayBuffer, {
    headers: { "x-stretch-meta": JSON.stringify({ sampleRate, tempo }) },
  })) as ArrayBuffer;
}
