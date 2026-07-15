import type { Channel } from "../types";
import { invokeDecodeStem } from "./tauri";

/**
 * Decode one isolated part from a source file into a mono {@link AudioBuffer}.
 *
 * The native `decode_stem` command returns a compact binary blob rather than a
 * JSON array so that multi-million-sample stems transfer efficiently.
 */
export async function decodeStem(
  ctx: BaseAudioContext,
  path: string,
  channel: Channel,
): Promise<AudioBuffer> {
  const raw = await invokeDecodeStem(path, channel);
  const view = new DataView(raw);
  const sampleRate = view.getUint32(0, true);
  const frames = view.getUint32(4, true);
  // Samples start at byte offset 8; offset is 4-byte aligned so Float32Array is safe.
  const samples = new Float32Array(raw, 8, frames);

  const buffer = ctx.createBuffer(1, frames, sampleRate);
  buffer.copyToChannel(samples, 0);
  return buffer;
}
