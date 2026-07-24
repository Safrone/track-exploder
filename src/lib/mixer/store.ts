import { writable, derived, get } from "svelte/store";
import {
  PARTS,
  defaultPartMix,
  type Alignment,
  type Part,
  type PartMix,
  type SourceTrack,
  type OutputMode,
  type Channel,
} from "../types";

export interface MixerState {
  /** Loaded source tracks keyed by part (undefined until loaded). */
  tracks: Partial<Record<Part, SourceTrack>>;
  /** Per-part mix settings. */
  mix: Record<Part, PartMix>;
  /** Per-part timing correction (empty until measured or nudged). */
  alignment: Partial<Record<Part, Alignment>>;
  masterGain: number;
  /** Whether pitch-preserving tempo processing is enabled (off = no stretch). */
  tempoEnabled: boolean;
  /** Playback / export tempo (0.5 .. 1.5); 1 = original. Applied only when enabled. */
  tempo: number;
  output: OutputMode;
  /** Which channel the isolated part sits on — shared by all files in a set. */
  sourceChannel: Channel;
}

function initialMix(): Record<Part, PartMix> {
  return Object.fromEntries(PARTS.map((p) => [p, defaultPartMix()])) as Record<Part, PartMix>;
}

function initialState(): MixerState {
  return {
    tracks: {},
    mix: initialMix(),
    alignment: {},
    masterGain: 1,
    tempoEnabled: false,
    tempo: 1,
    output: "stereo",
    sourceChannel: "left",
  };
}

export const mixer = writable<MixerState>(initialState());

/** True once all four parts have a loaded source track. */
export const allLoaded = derived(mixer, ($m) => PARTS.every((p) => !!$m.tracks[p]));

/**
 * Effective linear gain for a part. Returns 0 when the part is muted.
 */
export function effectiveGain(state: MixerState, part: Part): number {
  const m = state.mix[part];
  return m.included ? m.gain : 0;
}

// --- mutators ---------------------------------------------------------------

export function setPartMix(part: Part, patch: Partial<PartMix>): void {
  mixer.update((s) => ({ ...s, mix: { ...s.mix, [part]: { ...s.mix[part], ...patch } } }));
}

export function setTrack(part: Part, track: SourceTrack): void {
  mixer.update((s) => ({ ...s, tracks: { ...s.tracks, [part]: track } }));
}

/** Set (or clear, with `undefined`) one part's timing correction. */
export function setAlignment(part: Part, alignment: Alignment | undefined): void {
  mixer.update((s) => {
    const next = { ...s.alignment };
    if (alignment) next[part] = alignment;
    else delete next[part];
    return { ...s, alignment: next };
  });
}

/** Drop every timing correction (parts play exactly as decoded). */
export function clearAlignment(): void {
  mixer.update((s) => ({ ...s, alignment: {} }));
}

export function patchState(patch: Partial<MixerState>): void {
  mixer.update((s) => ({ ...s, ...patch }));
}

export function snapshot(): MixerState {
  return get(mixer);
}
