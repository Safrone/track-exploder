import { invokeStretchStem } from "./tauri";

/**
 * Pitch-preserving time-stretch of a mono stem via the native (Rust) Signalsmith
 * implementation, returning a new AudioBuffer. `tempo < 1` = slower/longer.
 */
export async function stretchStem(
  ctx: BaseAudioContext,
  mono: Float32Array,
  sampleRate: number,
  tempo: number,
): Promise<AudioBuffer> {
  // Copy to a tight buffer so we send exactly this channel's samples.
  const raw = await invokeStretchStem(mono.slice(), sampleRate, tempo);
  const view = new DataView(raw);
  const sr = view.getUint32(0, true);
  const frames = view.getUint32(4, true);
  const samples = new Float32Array(raw, 8, frames);

  const buffer = ctx.createBuffer(1, frames, sr);
  buffer.copyToChannel(samples, 0);
  return buffer;
}
