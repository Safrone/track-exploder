import { PARTS, type OutputMode, type Part } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";

/** Anything that can supply stems to render (the live engine, or a bulk job). */
export interface RenderSource {
  ctx: BaseAudioContext;
  duration: number;
  getBuffer(part: Part): AudioBuffer | undefined;
}

/**
 * Interleave planar channel data into a single Float32Array.
 * For "mono" output the (already single) channel is returned as-is.
 */
export function interleave(channels: Float32Array[]): Float32Array {
  if (channels.length === 1) return channels[0];
  const frames = channels[0].length;
  const out = new Float32Array(frames * channels.length);
  for (let i = 0; i < frames; i++) {
    for (let c = 0; c < channels.length; c++) {
      out[i * channels.length + c] = channels[c][i];
    }
  }
  return out;
}

export interface RenderedMix {
  pcm: Float32Array;
  channels: number;
  sampleRate: number;
}

/**
 * Render the current mix (the exact permutation shown in the UI) into
 * interleaved f32 PCM at natural speed. Any tempo change is applied afterward on
 * the native side (Signalsmith Stretch in Rust) — the browser's
 * OfflineAudioContext + AudioWorklet path deadlocks, so we don't stretch here.
 *
 * The mixdown is done in plain arithmetic rather than through an
 * `OfflineAudioContext`: WebKitGTK (the Linux/macOS webview) implements
 * `StereoPannerNode` in a realtime context but treats it as a passthrough when
 * rendering offline, so an exported mix came out with every part centred no
 * matter how it was panned. Doing the pan math here — the same equal-power law
 * `StereoPannerNode` uses for a mono source — makes the export match the preview
 * on every platform.
 */
export async function renderMix(
  source: RenderSource,
  state: MixerState,
  mode: OutputMode,
): Promise<RenderedMix> {
  const sampleRate = source.ctx.sampleRate;

  // Gather the audible parts with their left/right gains. Equal-power pan
  // (StereoPannerNode's law): a centred part sits at cos(45°)=sin(45°)≈0.707 in
  // each channel, hard-left at 1/0, hard-right at 0/1.
  const voices: { data: Float32Array; gl: number; gr: number }[] = [];
  let length = Math.max(1, Math.ceil(source.duration * sampleRate));
  for (const part of PARTS) {
    const buffer = source.getBuffer(part);
    const gain = effectiveGain(state, part) * state.masterGain;
    if (!buffer || gain === 0) continue;
    const angle = ((state.mix[part].pan + 1) * Math.PI) / 4; // -1..1 -> 0..π/2
    const data = buffer.getChannelData(0);
    length = Math.max(length, data.length);
    voices.push({ data, gl: Math.cos(angle) * gain, gr: Math.sin(angle) * gain });
  }

  const l = new Float32Array(length);
  const r = new Float32Array(length);
  for (const v of voices) {
    const n = v.data.length;
    for (let i = 0; i < n; i++) {
      l[i] += v.data[i] * v.gl;
      r[i] += v.data[i] * v.gr;
    }
  }

  if (mode === "mono") {
    const mono = new Float32Array(length);
    for (let i = 0; i < length; i++) mono[i] = (l[i] + r[i]) * 0.5;
    return { pcm: mono, channels: 1, sampleRate };
  }
  return { pcm: interleave([l, r]), channels: 2, sampleRate };
}
