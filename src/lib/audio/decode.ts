import type { Channel } from "../types";
import { invokeDecodeStem, invokeReadTags } from "./tauri";
import { extOf } from "./files";

/**
 * Decode one isolated part from a source file into a mono {@link AudioBuffer}.
 *
 * The native `decode_stem` command reads the file itself (via the fs plugin —
 * works with desktop paths and mobile content:// URIs) and returns a compact
 * binary blob rather than JSON so multi-million-sample stems transfer
 * efficiently.
 */
export async function decodeStem(
  ctx: BaseAudioContext,
  path: string,
  channel: Channel,
): Promise<AudioBuffer> {
  const raw = await invokeDecodeStem(path, channel, extOf(path));
  const view = new DataView(raw);
  const sampleRate = view.getUint32(0, true);
  const frames = view.getUint32(4, true);
  // Samples start at byte offset 8; offset is 4-byte aligned so Float32Array is safe.
  const samples = new Float32Array(raw, 8, frames);

  const buffer = ctx.createBuffer(1, frames, sampleRate);
  buffer.copyToChannel(samples, 0);
  return buffer;
}

/** Read a source file's tags (Rust reads the file and parses them). */
export async function readAudioTags(path: string): Promise<Record<string, string>> {
  return invokeReadTags(path, extOf(path));
}
