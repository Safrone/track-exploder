import { writable, derived, get } from "svelte/store";
import {
  PARTS,
  defaultPartMix,
  type Part,
  type PartMix,
  type SourceTrack,
  type OutputMode,
} from "../types";

export interface MixerState {
  /** Loaded source tracks keyed by part (undefined until loaded). */
  tracks: Partial<Record<Part, SourceTrack>>;
  /** Per-part mix settings. */
  mix: Record<Part, PartMix>;
  masterGain: number;
  /** Playback / export tempo (0.5 .. 1.5); 1 = original. */
  tempo: number;
  /** Preserve pitch when changing tempo (requires the stretch worklet). */
  preservePitch: boolean;
  output: OutputMode;
}

function initialMix(): Record<Part, PartMix> {
  return Object.fromEntries(PARTS.map((p) => [p, defaultPartMix()])) as Record<Part, PartMix>;
}

function initialState(): MixerState {
  return {
    tracks: {},
    mix: initialMix(),
    masterGain: 1,
    tempo: 1,
    preservePitch: true,
    output: "stereo",
  };
}

export const mixer = writable<MixerState>(initialState());

/** True once all four parts have a loaded source track. */
export const allLoaded = derived(mixer, ($m) => PARTS.every((p) => !!$m.tracks[p]));

/** Whether any part is currently soloed (affects effective gains). */
export function anySoloed(state: MixerState): boolean {
  return PARTS.some((p) => state.mix[p].soloed);
}

/**
 * Effective linear gain for a part, accounting for include/mute/solo.
 * Returns 0 when the part should be silent.
 */
export function effectiveGain(state: MixerState, part: Part): number {
  const m = state.mix[part];
  if (!m.included || m.muted) return 0;
  if (anySoloed(state) && !m.soloed) return 0;
  return m.gain;
}

// --- mutators ---------------------------------------------------------------

export function setPartMix(part: Part, patch: Partial<PartMix>): void {
  mixer.update((s) => ({ ...s, mix: { ...s.mix, [part]: { ...s.mix[part], ...patch } } }));
}

export function setTrack(part: Part, track: SourceTrack): void {
  mixer.update((s) => ({ ...s, tracks: { ...s.tracks, [part]: track } }));
}

export function patchState(patch: Partial<MixerState>): void {
  mixer.update((s) => ({ ...s, ...patch }));
}

export function snapshot(): MixerState {
  return get(mixer);
}
