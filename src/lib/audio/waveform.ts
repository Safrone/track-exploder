import { PARTS } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";
import type { MixEngine } from "./engine";

/** Per-stem peak envelope, cached by AudioBuffer identity + bucket count. */
const cache = new WeakMap<AudioBuffer, { buckets: number; env: Float32Array }>();

/** Downsample a mono stem to `buckets` peak-amplitude values. */
export function stemPeaks(buffer: AudioBuffer, buckets: number): Float32Array {
  const cached = cache.get(buffer);
  if (cached && cached.buckets === buckets) return cached.env;

  const data = buffer.getChannelData(0);
  const env = new Float32Array(buckets);
  const step = data.length / buckets;
  for (let b = 0; b < buckets; b++) {
    const start = Math.floor(b * step);
    const end = Math.min(data.length, Math.floor((b + 1) * step));
    let peak = 0;
    for (let i = start; i < end; i++) {
      const a = Math.abs(data[i]);
      if (a > peak) peak = a;
    }
    env[b] = peak;
  }
  cache.set(buffer, { buckets, env });
  return env;
}

export interface StereoEnvelope {
  l: Float32Array;
  r: Float32Array;
}

/**
 * Combine the loaded stems into a stereo peak envelope for the current mix.
 * Cheap enough to recompute on every gain/pan/mute change: it works from the
 * precomputed per-stem envelopes, not the raw samples.
 *
 * Uses the same equal-power pan law as `StereoPannerNode` so the picture
 * matches what you hear.
 */
export function mixEnvelope(
  engine: MixEngine,
  state: MixerState,
  buckets: number,
): StereoEnvelope {
  const l = new Float32Array(buckets);
  const r = new Float32Array(buckets);

  for (const part of PARTS) {
    const buf = engine.getBuffer(part);
    if (!buf) continue;
    const gain = effectiveGain(state, part) * state.masterGain;
    if (gain === 0) continue;

    const pan = state.mix[part].pan;
    const x = ((pan + 1) * Math.PI) / 4; // -1..1 -> 0..π/2
    const gl = Math.cos(x) * gain;
    const gr = Math.sin(x) * gain;

    const env = stemPeaks(buf, buckets);
    for (let b = 0; b < buckets; b++) {
      l[b] += env[b] * gl;
      r[b] += env[b] * gr;
    }
  }
  return { l, r };
}
