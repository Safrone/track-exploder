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

/** Per-part mixer settings. */
export interface PartMix {
  /** Included in the output at all. */
  included: boolean;
  /** Linear gain (0..~2). 1 = unity. */
  gain: number;
  /** Stereo pan, -1 (hard left) .. +1 (hard right). */
  pan: number;
  muted: boolean;
  soloed: boolean;
}

export type OutputMode = "stereo" | "mono";
export type ExportFormat = "wav" | "flac" | "mp3";
export type BitDepth = 16 | 24;

export function defaultPartMix(): PartMix {
  return { included: true, gain: 1, pan: 0, muted: false, soloed: false };
}
