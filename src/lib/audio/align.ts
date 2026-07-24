/**
 * Track alignment.
 *
 * Publishers build a learning track as *spoken title → pitch pipe → a gap → the
 * song*, and the four part files don't always get the song pasted in at the same
 * spot — so the parts can play tens (occasionally hundreds) of milliseconds
 * apart even though their intros line up. The measurement runs natively (see the
 * `analyze_alignment` command); this module applies the result to a stem and
 * lets the user nudge it afterwards.
 *
 * The edit is made *inside the lead-in gap* rather than by sliding the whole
 * file, so the spoken title and the pitch pipe stay in line with the other parts.
 */
import { PARTS, type Alignment, type Part, type SourceTrack } from "../types";
import { invokeAnalyzeAlignment } from "./tauri";
import { extOf } from "./files";

/** No change: the stem plays as decoded. */
export function noAlignment(sampleRate: number): Alignment {
  return {
    offsetFrames: 0,
    spliceAt: 0,
    deltaFrames: 0,
    confidence: 1,
    consistent: true,
    spreadFrames: 0,
    sampleRate,
  };
}

export function isAligned(a: Alignment | undefined): boolean {
  return !a || a.deltaFrames === 0;
}

/** How far this stem is moved, in milliseconds (negative = earlier). */
export function shiftMs(a: Alignment | undefined): number {
  if (!a || !a.sampleRate) return 0;
  return (a.deltaFrames / a.sampleRate) * 1000;
}

/**
 * Cut `-delta` frames out of `samples` at `at`, or splice in that many frames of
 * silence when `delta` is positive.
 */
export function spliceSamples(
  samples: Float32Array<ArrayBuffer>,
  at: number,
  delta: number,
): Float32Array<ArrayBuffer> {
  if (!delta) return samples;
  const cut = Math.max(0, Math.min(Math.round(at), samples.length));
  const frames = Math.max(1, samples.length + delta);
  const out = new Float32Array(frames);

  out.set(samples.subarray(0, Math.min(cut, frames)));
  if (delta > 0) {
    // Silence goes in at `cut`; everything after it moves later.
    out.set(samples.subarray(cut, cut + Math.max(0, frames - cut - delta)), cut + delta);
  } else {
    out.set(samples.subarray(cut - delta, cut - delta + Math.max(0, frames - cut)), cut);
  }
  return out;
}

/**
 * A stem with its timing correction applied. Returns the original buffer when
 * there's nothing to do.
 */
export function applyAlignment(
  ctx: BaseAudioContext,
  buffer: AudioBuffer,
  alignment: Alignment | undefined,
): AudioBuffer {
  if (!alignment?.deltaFrames) return buffer;
  const out = spliceSamples(buffer.getChannelData(0), alignment.spliceAt, alignment.deltaFrames);
  const shifted = ctx.createBuffer(1, out.length, buffer.sampleRate);
  shifted.copyToChannel(out, 0);
  return shifted;
}

/** Move a stem by `ms` relative to its current alignment (negative = earlier). */
export function nudge(alignment: Alignment, ms: number): Alignment {
  const frames = Math.round((ms / 1000) * alignment.sampleRate);
  return { ...alignment, deltaFrames: alignment.deltaFrames + frames };
}

export interface AlignmentReport {
  alignment: Partial<Record<Part, Alignment>>;
  sampleRate: number;
  /** Parts whose measurement wandered through the song — a single edit is approximate. */
  unsteady: Part[];
  /** Parts the measurement couldn't pin down at all. */
  unmeasured: Part[];
}

/**
 * Measure a loaded set and return the correction for each part. Aligns to
 * whichever part's song starts earliest.
 */
export async function analyzeAlignment(
  tracks: Partial<Record<Part, SourceTrack>>,
): Promise<AlignmentReport> {
  const parts = PARTS.filter((p) => tracks[p]);
  if (parts.length < 2) {
    return { alignment: {}, sampleRate: 0, unsteady: [], unmeasured: [] };
  }

  const results = await invokeAnalyzeAlignment(
    parts.map((p) => ({ path: tracks[p]!.path, ext: extOf(tracks[p]!.path) })),
  );

  const alignment: Partial<Record<Part, Alignment>> = {};
  const unsteady: Part[] = [];
  const unmeasured: Part[] = [];
  parts.forEach((part, i) => {
    const r = results[i];
    alignment[part] = {
      offsetFrames: r.offsetFrames,
      spliceAt: r.spliceAt,
      deltaFrames: r.deltaFrames,
      confidence: r.confidence,
      consistent: r.consistent,
      spreadFrames: r.spreadFrames,
      sampleRate: r.sampleRate,
    };
    if (r.confidence <= 0.35) unmeasured.push(part);
    else if (!r.consistent) unsteady.push(part);
  });

  return { alignment, sampleRate: results[0]?.sampleRate ?? 0, unsteady, unmeasured };
}
