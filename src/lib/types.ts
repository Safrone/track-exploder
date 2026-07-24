/** The four barbershop voice parts, in conventional score order. */
export const PARTS = ["tenor", "lead", "baritone", "bass"] as const;
export type Part = (typeof PARTS)[number];

/** Which stereo channel of a source file holds the isolated part. */
export type Channel = "left" | "right";

/** One source track loaded from disk, before/after extraction. */
export interface SourceTrack {
  part: Part;
  /** Absolute path on disk (from the native open dialog). */
  path: string;
  /** File name for display. */
  name: string;
  /** Which channel to extract the isolated part from. */
  channel: Channel;
}

/**
 * How one part's stem is shifted so the set plays together.
 *
 * The edit is a splice: cut `-deltaFrames` frames at `spliceAt` (or insert that
 * many frames of silence when positive). It's placed in the silent gap of the
 * lead-in, so the spoken title and pitch pipe stay aligned with the other parts.
 */
export interface Alignment {
  /** How much later this part's song started than the earliest one, in frames. */
  offsetFrames: number;
  spliceAt: number;
  deltaFrames: number;
  /** Correlation behind the measurement, 0..1. */
  confidence: number;
  /** Whether the offset held steady through the song. */
  consistent: boolean;
  /** How much the measurement varied between windows, in frames. */
  spreadFrames: number;
  /** Sample rate the frame counts are in (the source file's, not the mixer's). */
  sampleRate: number;
}

/** Per-part mixer settings. */
export interface PartMix {
  /** Audible in the output. `false` = muted (the single per-part on/off). */
  included: boolean;
  /** Linear gain (0..~2). 1 = unity. */
  gain: number;
  /** Stereo pan, -1 (hard left) .. +1 (hard right). */
  pan: number;
}

export type OutputMode = "stereo" | "mono";
export type ExportFormat = "wav" | "flac" | "mp3";
export type BitDepth = 16 | 24;

export function defaultPartMix(): PartMix {
  return { included: true, gain: 1, pan: 0 };
}
