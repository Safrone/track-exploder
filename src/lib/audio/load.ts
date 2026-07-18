import { open } from "@tauri-apps/plugin-dialog";
import { PARTS, type Channel, type Part, type SourceTrack } from "../types";
import { setTrack } from "../mixer/store";
import { decodeStem, readAudioTags } from "./decode";
import { invokeContentName } from "./tauri";
import { isRealPath } from "./files";
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

/** Guess which part a piece of text (filename, tag value) refers to, or null. */
export function guessPart(text: string): Part | null {
  const lower = text.toLowerCase();
  for (const part of PARTS) {
    if (PART_KEYWORDS[part].some((kw) => new RegExp(`\\b${kw}\\b`).test(lower))) {
      return part;
    }
  }
  return null;
}

/**
 * The file's real display name. For desktop paths that's the basename; for an
 * Android content:// URI (whose "basename" is an opaque token like `msf%3A…`)
 * we ask the ContentResolver for the actual filename.
 */
export async function displayName(path: string): Promise<string> {
  if (isRealPath(path)) return basename(path);
  try {
    const name = await invokeContentName(path);
    if (name) return name;
  } catch {
    /* fall back to the raw basename */
  }
  return basename(path);
}

/**
 * Guess the part from a file's metadata tags. A safety net for oddly-named files
 * where the filename alone doesn't resolve; vendor learning tracks put the part
 * in the `artist` tag (e.g. "TENOR", "BARI").
 */
export function guessPartFromTags(tags: Record<string, string>): Part | null {
  return guessPart(tags.artist ?? "") ?? guessPart(tags.comment ?? "");
}

/**
 * Resolve a picked file's display name and part. Detection is filename-first
 * (using the real name, resolved from the URI on Android); tags are only a
 * fallback when the name doesn't clearly identify the part.
 */
export async function resolvePart(path: string): Promise<{ part: Part | null; name: string }> {
  const name = await displayName(path);
  let part = guessPart(name);
  if (!part) {
    try {
      part = guessPartFromTags(await readAudioTags(path));
    } catch {
      /* tags are best-effort */
    }
  }
  return { part, name };
}

/** Decode one file into the engine for a given part + channel, updating the store. */
export async function loadPart(
  engine: MixEngine,
  part: Part,
  path: string,
  channel: Channel,
  name?: string,
): Promise<void> {
  const buffer = await decodeStem(engine.ctx, path, channel);
  engine.setBuffer(part, buffer);
  setTrack(part, { part, path, name: name ?? (await displayName(path)), channel });
}

const AUDIO_FILTERS = [
  { name: "Audio", extensions: ["mp3", "wav", "flac", "m4a", "aac", "ogg", "opus"] },
];

/** Open a single-file picker (used for explicit per-part loading). */
export async function pickOneAudioFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    title: "Select the file for this part",
    filters: AUDIO_FILTERS,
  });
  if (!selected) return null;
  return Array.isArray(selected) ? (selected[0] ?? null) : selected;
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
 * Decode a set of part/file assignments into the engine concurrently (each stem
 * decodes on its own Rust thread), reporting progress as each finishes.
 */
async function loadAll(
  engine: MixEngine,
  items: { part: Part; path: string; name?: string }[],
  channel: Channel,
  onProgress?: LoadProgress,
): Promise<void> {
  const total = items.length;
  let done = 0;
  if (total) onProgress?.(0, total, items[0].part);
  await Promise.all(
    items.map(async ({ part, path, name }) => {
      await loadPart(engine, part, path, channel, name);
      onProgress?.(++done, total, part);
    }),
  );
}

/**
 * Pick files, guess each part (by filename, falling back to metadata tags for
 * Android content URIs), and decode the matched ones (default channel: left).
 * Part resolution and decoding both run concurrently across files. Returns which
 * parts loaded and any files that couldn't be auto-assigned.
 */
export async function pickAndLoad(
  engine: MixEngine,
  onProgress?: LoadProgress,
  channel: Channel = "left",
): Promise<LoadReport> {
  const paths = await pickAudioFiles();

  // Resolve every file's part concurrently, then assign deterministically in the
  // picked order (first file wins a part; a duplicate part goes to unassigned).
  const resolved = await Promise.all(
    paths.map(async (path) => ({ path, ...(await resolvePart(path)) })),
  );

  const assignments: { part: Part; path: string; name: string }[] = [];
  const unassigned: string[] = [];
  const taken = new Set<Part>();
  for (const { part, path, name } of resolved) {
    if (part && !taken.has(part)) {
      taken.add(part);
      assignments.push({ part, path, name });
    } else {
      unassigned.push(name);
    }
  }

  await loadAll(engine, assignments, channel, onProgress);
  return { loaded: assignments.map((a) => a.part), unassigned };
}

/** Re-extract every already-loaded track from the given channel (with progress). */
export async function reExtractAll(
  engine: MixEngine,
  tracks: Partial<Record<Part, SourceTrack>>,
  channel: Channel,
  onProgress?: LoadProgress,
): Promise<void> {
  const items = PARTS.filter((p) => tracks[p]).map((part) => ({
    part,
    path: tracks[part]!.path,
    name: tracks[part]!.name,
  }));
  await loadAll(engine, items, channel, onProgress);
}
