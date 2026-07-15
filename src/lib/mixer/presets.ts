import { PARTS, type Part } from "../types";
import { mixer, type MixerState } from "./store";

/**
 * Common barbershop practice mixes. Labels are a function of the chosen focus
 * part so the buttons read e.g. "Lead missing" — making it clear the part
 * dropdown feeds these presets.
 */
export interface Preset {
  id: string;
  /** `part` is the capitalized focus part name (ignored by focus-less presets). */
  label: (part: string) => string;
  description: string;
  /** Whether the preset depends on the chosen focus part. */
  usesFocus: boolean;
  apply: (focus: Part) => Partial<Record<Part, { gain: number; pan: number; included: boolean }>>;
}

const QUIET = 0.35;

export const PRESETS: Preset[] = [
  {
    id: "full",
    label: () => "Full mix",
    description: "All four parts, centered and balanced.",
    usesFocus: false,
    apply: () =>
      Object.fromEntries(PARTS.map((p) => [p, { gain: 1, pan: 0, included: true }])),
  },
  {
    id: "solo",
    label: (part) => `${part} solo`,
    description: "Only the chosen part.",
    usesFocus: true,
    apply: (focus) =>
      Object.fromEntries(
        PARTS.map((p) => [p, { gain: 1, pan: 0, included: p === focus }]),
      ),
  },
  {
    id: "part-missing",
    label: (part) => `${part} missing`,
    description: "Everyone except the chosen part.",
    usesFocus: true,
    apply: (focus) =>
      Object.fromEntries(
        PARTS.map((p) => [p, { gain: 1, pan: 0, included: p !== focus }]),
      ),
  },
  {
    id: "predominant",
    label: (part) => `${part} predominant`,
    description: "Chosen part up front, the other three quieter.",
    usesFocus: true,
    apply: (focus) =>
      Object.fromEntries(
        PARTS.map((p) => [p, { gain: p === focus ? 1 : QUIET, pan: 0, included: true }]),
      ),
  },
  {
    id: "part-left",
    label: (part) => `${part} left`,
    description: "Chosen part hard left, the other three on the right.",
    usesFocus: true,
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
