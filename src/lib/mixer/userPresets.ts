import { writable, get } from "svelte/store";
import type { OutputMode, Part, PartMix } from "../types";
import { mixer } from "./store";

/** A user-saved mix: per-part settings plus master level and output mode. */
export interface UserPreset {
  id: string;
  name: string;
  mix: Record<Part, PartMix>;
  masterGain: number;
  output: OutputMode;
}

const KEY = "trackexploder.userPresets";

function load(): UserPreset[] {
  try {
    const raw = localStorage.getItem(KEY);
    return raw ? (JSON.parse(raw) as UserPreset[]) : [];
  } catch {
    return [];
  }
}

export const userPresets = writable<UserPreset[]>(load());

userPresets.subscribe((list) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(list));
  } catch {
    /* ignore */
  }
});

function newId(): string {
  return typeof crypto !== "undefined" && crypto.randomUUID
    ? crypto.randomUUID()
    : `p_${Date.now()}`;
}

/** Save the current mix (per-part + master + output) as a named preset. */
export function saveCurrentAsPreset(name: string): void {
  const s = get(mixer);
  const preset: UserPreset = {
    id: newId(),
    name,
    mix: structuredClone(s.mix),
    masterGain: s.masterGain,
    output: s.output,
  };
  userPresets.update((list) => [...list, preset]);
}

/** Apply a saved preset to the mixer. */
export function applyUserPreset(id: string): void {
  const preset = get(userPresets).find((p) => p.id === id);
  if (!preset) return;
  mixer.update((s) => ({
    ...s,
    mix: structuredClone(preset.mix),
    masterGain: preset.masterGain,
    output: preset.output,
  }));
}

export function deleteUserPreset(id: string): void {
  userPresets.update((list) => list.filter((p) => p.id !== id));
}
