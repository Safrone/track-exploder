import { PARTS, type Part } from "../types";
import type { MixerState } from "../mixer/store";

/** Anything holding the loaded stems (the preview engine, in practice). */
export interface StemSource {
  getBuffer(part: Part): AudioBuffer | undefined;
}

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

/** One voice's lane in the waveform view. */
export interface PartLane {
  part: Part;
  /** Peak envelope of the stem as recorded, before gain and pan. */
  peaks: Float32Array;
  /** The loudest of those peaks — the lane's own reference for fitting the box. */
  peak: number;
  /** Level going to the left channel — drawn upward (gain × pan). */
  left: number;
  /** …and to the right — drawn downward. */
  right: number;
  /** True when the part won't be heard: muted, or turned all the way down. */
  silent: boolean;
  /** Length of the stem in seconds (they differ once a part has been shifted). */
  duration: number;
}

/**
 * A lane per loaded voice, with its level split into what the left and right
 * channels get.
 *
 * The split uses the same equal-power pan law as `StereoPannerNode`, so a part
 * panned hard left draws only upward, hard right only downward, and centred
 * draws evenly either side — and gain scales the lane, so you can see the
 * balance you've set as well as hear it.
 *
 * Each lane carries its own [`peak`](PartLane.peak) and the view scales against
 * that, so turning one part up grows *that* lane and leaves the others exactly
 * where they were. The master gain is left out for the same reason: it moves
 * every part together and would only make the picture jump.
 */
export function partLanes(source: StemSource, state: MixerState, buckets: number): PartLane[] {
  const lanes: PartLane[] = [];

  for (const part of PARTS) {
    const buffer = source.getBuffer(part);
    if (!buffer) continue;

    const mix = state.mix[part];
    const angle = ((mix.pan + 1) * Math.PI) / 4; // -1..1 -> 0..π/2
    const peaks = stemPeaks(buffer, buckets);
    lanes.push({
      part,
      peaks,
      peak: peaks.reduce((m, p) => Math.max(m, p), 0),
      left: Math.cos(angle) * mix.gain,
      right: Math.sin(angle) * mix.gain,
      silent: !mix.included || mix.gain === 0,
      duration: buffer.duration,
    });
  }
  return lanes;
}
