/**
 * Thin wrappers around Tauri IPC with graceful fallbacks so the UI can still
 * load in a plain browser (`npm run dev` without a webview) for layout work.
 */
import { invoke } from "@tauri-apps/api/core";

/** True when running inside a Tauri webview (desktop or mobile). */
export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/**
 * Decode one isolated channel of a file. Returns the raw response bytes:
 * `[sampleRate:u32 LE][frames:u32 LE][samples:f32 LE ...]`.
 */
export async function invokeDecodeStem(
  path: string,
  channel: "left" | "right",
): Promise<ArrayBuffer> {
  // A command returning `tauri::ipc::Response` resolves to an ArrayBuffer.
  return (await invoke("decode_stem", { path, channel })) as ArrayBuffer;
}

/**
 * Send a rendered interleaved-f32 mix to the native side to encode and write.
 */
export async function invokeExportMix(
  pcm: Float32Array,
  meta: {
    path: string;
    format: string;
    channels: number;
    sampleRate: number;
    bitDepth: number;
    tags?: Record<string, string>;
    tempo?: number;
  },
): Promise<void> {
  await invoke("export_mix", pcm.buffer as ArrayBuffer, {
    headers: { "x-export-meta": JSON.stringify(meta) },
  });
}

/** Read a normalized set of tags (album, artist, title, …) from a source file. */
export async function invokeReadTags(path: string): Promise<Record<string, string>> {
  return (await invoke("read_tags", { path })) as Record<string, string>;
}
