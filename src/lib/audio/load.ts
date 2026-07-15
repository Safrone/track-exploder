import { open } from "@tauri-apps/plugin-dialog";
import { PARTS, type Channel, type Part } from "../types";
import { setTrack } from "../mixer/store";
import { decodeStem } from "./decode";
import type { MixEngine } from "./engine";

const PART_KEYWORDS: Record<Part, string[]> = {
  tenor: ["tenor", "ten"],
  lead: ["lead", "melody"],
  baritone: ["baritone", "bari", "bar"],
  bass: ["bass"],
};

export function basename(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

/** Guess which part a filename belongs to, or null if unclear. */
export function guessPart(fileName: string): Part | null {
  const lower = fileName.toLowerCase();
  for (const part of PARTS) {
    if (PART_KEYWORDS[part].some((kw) => new RegExp(`\\b${kw}\\b`).test(lower))) {
      return part;
    }
  }
  return null;
}

/** Decode one file into the engine for a given part + channel, updating the store. */
export async function loadPart(
  engine: MixEngine,
  part: Part,
  path: string,
  channel: Channel,
): Promise<void> {
  const buffer = await decodeStem(engine.ctx, path, channel);
  engine.setBuffer(part, buffer);
  setTrack(part, { part, path, name: basename(path), channel });
}

/** Open a native file picker and return the selected absolute paths. */
export async function pickAudioFiles(): Promise<string[]> {
  const selected = await open({
    multiple: true,
    title: "Select the four part tracks",
    filters: [
      { name: "Audio", extensions: ["mp3", "wav", "flac", "m4a", "aac", "ogg", "opus"] },
    ],
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export interface LoadReport {
  loaded: Part[];
  unassigned: string[];
}

/** Progress callback: `done` of `total` files decoded; `part` is the one just finished. */
export type LoadProgress = (done: number, total: number, part: Part) => void;

/**
 * Pick files, guess each part from its filename, and decode the matched ones
 * (default channel: left). Reports progress as each file finishes decoding.
 * Returns which parts loaded and any files that couldn't be auto-assigned.
 */
export async function pickAndLoad(
  engine: MixEngine,
  onProgress?: LoadProgress,
): Promise<LoadReport> {
  const paths = await pickAudioFiles();

  // Resolve part assignments up front so we know the total to decode.
  const assignments: { part: Part; path: string }[] = [];
  const unassigned: string[] = [];
  const taken = new Set<Part>();
  for (const path of paths) {
    const guess = guessPart(basename(path));
    if (guess && !taken.has(guess)) {
      taken.add(guess);
      assignments.push({ part: guess, path });
    } else {
      unassigned.push(path);
    }
  }

  const loaded: Part[] = [];
  const total = assignments.length;
  for (let i = 0; i < total; i++) {
    const { part, path } = assignments[i];
    onProgress?.(i, total, part);
    await loadPart(engine, part, path, "left");
    loaded.push(part);
    onProgress?.(i + 1, total, part);
  }
  return { loaded, unassigned };
}
