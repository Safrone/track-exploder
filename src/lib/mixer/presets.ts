import { PARTS, type Part } from "../types";
import { mixer, type MixerState } from "./store";

/**
 * Common barbershop practice mixes. Each preset maps to a full per-part
 * configuration applied on top of the current tempo/output settings.
 */
export interface Preset {
  id: string;
  label: string;
  description: string;
  /** Build the per-part settings; `focus` is the chosen part (if any). */
  apply: (focus: Part) => Partial<Record<Part, { gain: number; pan: number; included: boolean }>>;
}

const QUIET = 0.35;

export const PRESETS: Preset[] = [
  {
    id: "full",
    label: "Full mix",
    description: "All four parts, centered and balanced.",
    apply: () =>
      Object.fromEntries(PARTS.map((p) => [p, { gain: 1, pan: 0, included: true }])),
  },
  {
    id: "solo",
    label: "Solo part",
    description: "Only the chosen part.",
    apply: (focus) =>
      Object.fromEntries(
        PARTS.map((p) => [p, { gain: 1, pan: 0, included: p === focus }]),
      ),
  },
  {
    id: "part-off",
    label: "Part off (sing along)",
    description: "Everyone except the chosen part — you sing the missing voice.",
    apply: (focus) =>
      Object.fromEntries(
        PARTS.map((p) => [p, { gain: 1, pan: 0, included: p !== focus }]),
      ),
  },
  {
    id: "predominant",
    label: "Part predominant",
    description: "Chosen part up front, the other three quieter.",
    apply: (focus) =>
      Object.fromEntries(
        PARTS.map((p) => [p, { gain: p === focus ? 1 : QUIET, pan: 0, included: true }]),
      ),
  },
  {
    id: "learning-layout",
    label: "Learning-track layout",
    description: "Chosen part hard left, the other three on the right.",
    apply: (focus) =>
      Object.fromEntries(
        PARTS.map((p) => [
          p,
          { gain: 1, pan: p === focus ? -1 : 1, included: true },
        ]),
      ),
  },
];

/** Apply a preset (by id) with the given focus part to the mixer store. */
export function applyPreset(id: string, focus: Part): void {
  const preset = PRESETS.find((p) => p.id === id);
  if (!preset) return;
  const partial = preset.apply(focus);
  mixer.update((s: MixerState) => {
    const mix = { ...s.mix };
    for (const p of PARTS) {
      const cfg = partial[p];
      if (cfg) {
        mix[p] = { ...mix[p], ...cfg };
      }
    }
    return { ...s, mix };
  });
}
