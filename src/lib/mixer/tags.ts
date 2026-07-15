import { writable, get } from "svelte/store";
import { PARTS, type Part } from "../types";

export type Tags = Record<string, string>;

/** Tags read from each loaded source file. */
export const partTags = writable<Partial<Record<Part, Tags>>>({});

export function setPartTags(part: Part, tags: Tags): void {
  partTags.update((m) => ({ ...m, [part]: tags }));
}

export function clearTags(): void {
  partTags.set({});
}

/**
 * Tags shared (identical value) across every loaded part — e.g. album, date,
 * genre, title. Per-part tags like `artist` (which encodes the voice) drop out.
 */
export function computeCommon(map: Partial<Record<Part, Tags>>): Tags {
  const parts = PARTS.filter((p) => map[p]);
  if (parts.length === 0) return {};
  const first = map[parts[0]]!;
  const out: Tags = {};
  for (const [k, v] of Object.entries(first)) {
    if (parts.every((p) => map[p]![k] === v)) out[k] = v;
  }
  return out;
}

/** Convenience: common tags from the current store state. */
export function commonTags(): Tags {
  return computeCommon(get(partTags));
}
