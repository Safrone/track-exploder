import { PARTS, type ExportFormat, type Part } from "../types";
import { guessPart, basename } from "./load";
import { decodeStem, readAudioTags } from "./decode";
import { renderMix, type RenderSource } from "./export";
import { invokeEncodeMix, invokeEmbedTags } from "./tauri";
import { writeBytes, isRealPath } from "./files";
import { commonSongBase, suggestBaseNameFor } from "../mixer/naming";
import { computeCommon } from "../mixer/tags";
import { addExport, setLastExportDir } from "../mixer/exports";
import type { MixerState } from "../mixer/store";

/** One detected song and the part files found for it. */
export interface SongGroup {
  key: string;
  /** Display base name (song title). */
  name: string;
  parts: Partial<Record<Part, string>>;
  missing: Part[];
  /** Files that matched the song but not a distinct part (dupes/unknowns). */
  extra: string[];
}

const PART_TOKEN = /\b(tenor|lead|baritone|bari|bass)\b/i;

function stripExt(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/**
 * A key shared by all parts of one song: the filename up to the part token.
 * Everything after (author, date, etc.) is ignored, since it can differ between
 * parts (e.g. a bass track re-rendered on a different day).
 */
function songKey(name: string): string {
  const bare = stripExt(name);
  const m = bare.match(PART_TOKEN);
  const prefix = m ? bare.slice(0, m.index) : bare;
  return prefix
    .replace(/[\s\-_–—]+$/, "")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();
}

export interface Grouping {
  groups: SongGroup[];
  /** Files with no recognizable part token. */
  ungrouped: string[];
}

/** Group selected files into songs by their shared name (part token removed). */
export function groupFiles(paths: string[]): Grouping {
  const byKey = new Map<string, string[]>();
  const ungrouped: string[] = [];

  for (const path of paths) {
    const name = basename(path);
    if (!PART_TOKEN.test(stripExt(name))) {
      ungrouped.push(path);
      continue;
    }
    const key = songKey(name);
    const list = byKey.get(key);
    if (list) list.push(path);
    else byKey.set(key, [path]);
  }

  const groups: SongGroup[] = [];
  for (const [key, groupPaths] of byKey) {
    const parts: Partial<Record<Part, string>> = {};
    const extra: string[] = [];
    for (const p of groupPaths) {
      const part = guessPart(basename(p));
      if (part && !parts[part]) parts[part] = p;
      else extra.push(p);
    }
    const missing = PARTS.filter((pt) => !parts[pt]);
    groups.push({ key, name: commonSongBase(groupPaths.map(basename)), parts, missing, extra });
  }
  groups.sort((a, b) => a.name.localeCompare(b.name));
  return { groups, ungrouped };
}

export type BulkStage = "decoding" | "rendering" | "encoding" | "done" | "error";
export type BulkProgress = (
  info: { index: number; total: number; group: SongGroup; stage: BulkStage; error?: string },
) => void;

export interface BulkOptions {
  ctx: BaseAudioContext;
  state: MixerState;
  format: ExportFormat;
  bitDepth: number;
  outputDir: string;
}

const EXT: Record<ExportFormat, string> = { wav: "wav", flac: "flac", mp3: "mp3" };

/** Export every complete group using the given (current) mix settings. */
export async function bulkExport(
  groups: SongGroup[],
  opts: BulkOptions,
  onProgress?: BulkProgress,
): Promise<{ exported: number; failed: number }> {
  const complete = groups.filter((g) => g.missing.length === 0);
  const total = complete.length;
  const tempo = opts.state.tempoEnabled ? opts.state.tempo : 1;
  const sep = opts.outputDir.includes("\\") ? "\\" : "/";
  setLastExportDir(opts.outputDir);

  let exported = 0;
  let failed = 0;
  for (let index = 0; index < complete.length; index++) {
    const group = complete[index];
    try {
      onProgress?.({ index, total, group, stage: "decoding" });
      const stems = new Map<Part, AudioBuffer>();
      const tagMap: Partial<Record<Part, Record<string, string>>> = {};
      for (const part of PARTS) {
        const path = group.parts[part]!;
        stems.set(part, await decodeStem(opts.ctx, path, opts.state.sourceChannel));
        try {
          tagMap[part] = await readAudioTags(path);
        } catch {
          /* tags best-effort */
        }
      }

      let duration = 0;
      for (const b of stems.values()) duration = Math.max(duration, b.duration);
      const source: RenderSource = { ctx: opts.ctx, duration, getBuffer: (p) => stems.get(p) };

      onProgress?.({ index, total, group, stage: "rendering" });
      const rendered = await renderMix(source, opts.state, opts.state.output);

      onProgress?.({ index, total, group, stage: "encoding" });
      const fileBase = suggestBaseNameFor(group.name, opts.state);
      const fileName = `${fileBase}.${EXT[opts.format]}`;
      const outPath = `${opts.outputDir}${sep}${fileName}`;
      const encoded = await invokeEncodeMix(rendered.pcm, {
        format: opts.format,
        channels: rendered.channels,
        sampleRate: rendered.sampleRate,
        bitDepth: opts.bitDepth,
        tempo,
      });
      await writeBytes(outPath, new Uint8Array(encoded));
      const tags = computeCommon(tagMap);
      if (isRealPath(outPath) && Object.keys(tags).length > 0) {
        try {
          await invokeEmbedTags(outPath, opts.format, tags);
        } catch {
          /* tags best-effort */
        }
      }

      addExport({ path: outPath, name: fileName, format: opts.format, at: Date.now() });
      exported++;
      onProgress?.({ index, total, group, stage: "done" });
    } catch (e) {
      failed++;
      onProgress?.({ index, total, group, stage: "error", error: String(e) });
    }
  }
  return { exported, failed };
}
