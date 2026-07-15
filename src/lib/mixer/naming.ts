import { PARTS, type Part } from "../types";
import type { MixerState } from "./store";

function stripExt(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

function cap(s: string): string {
  return s.length ? s[0].toUpperCase() + s.slice(1) : s;
}

// Characters that are invalid in filenames on Windows (and best avoided
// elsewhere). Spaces, hyphens, and brackets are intentionally kept.
const INVALID = new Set(["<", ">", ":", '"', "/", "\\", "|", "?", "*"]);

function sanitize(s: string): string {
  let out = "";
  for (const ch of s) {
    if (ch.charCodeAt(0) < 0x20) continue; // control chars
    if (INVALID.has(ch)) continue;
    out += ch;
  }
  return out.replace(/\s+/g, " ").trim();
}

/**
 * Derive a song base name from the loaded track filenames. Since the four files
 * differ only in their part token (TENOR/LEAD/BARI/BASS), the longest common
 * prefix is the song title up to that token.
 */
export function commonSongBase(names: string[]): string {
  const bare = names.map(stripExt).filter(Boolean);
  if (bare.length === 0) return "mix";
  if (bare.length === 1) return sanitize(bare[0]) || "mix";

  let prefix = bare[0];
  for (const s of bare) {
    while (prefix && !s.startsWith(prefix)) prefix = prefix.slice(0, -1);
  }
  // Trim a trailing separator left where the part token was removed.
  const base = sanitize(prefix.replace(/[\s\-_–—]+$/, ""));
  return base.length >= 3 ? base : sanitize(bare[0]) || "mix";
}

/** The parts that are actually audible (not muted). */
function activeParts(state: MixerState): Part[] {
  return PARTS.filter((p) => state.mix[p].included);
}

/** A short human description of the current mix (e.g. "Lead off", "Bass solo"). */
export function describeMix(state: MixerState): string {
  const active = activeParts(state);

  if (active.length === 0) return "silent";
  if (active.length === 1) return `${cap(active[0])} solo`;

  if (active.length === PARTS.length) {
    // Detect a single predominant part (one loud, the rest clearly quieter).
    const loud = PARTS.filter((p) => state.mix[p].gain >= 0.9);
    const quiet = PARTS.filter((p) => state.mix[p].gain <= 0.6);
    if (loud.length === 1 && quiet.length === PARTS.length - 1) {
      return `${cap(loud[0])} predominant`;
    }
    return "full mix";
  }

  if (active.length === PARTS.length - 1) {
    const missing = PARTS.find((p) => !active.includes(p));
    if (missing) return `no ${cap(missing)}`;
  }

  return active.map(cap).join("-");
}

/** The mix-descriptor suffix, e.g. " - Lead missing 85pct mono". */
export function mixSuffix(state: MixerState): string {
  const desc = describeMix(state);
  const t = state.tempoEnabled ? state.tempo : 1;
  const tempo = t !== 1 ? ` ${Math.round(t * 100)}pct` : "";
  const mono = state.output === "mono" ? " mono" : "";
  return ` - ${desc}${tempo}${mono}`;
}

/** Suggested base filename for an explicit song base + the current mix. */
export function suggestBaseNameFor(songBase: string, state: MixerState): string {
  return sanitize(`${songBase}${mixSuffix(state)}`);
}

/** Suggested export base filename (no extension), reflecting sources + options. */
export function suggestBaseName(state: MixerState): string {
  const names = PARTS.map((p) => state.tracks[p]?.name).filter(
    (n): n is string => !!n,
  );
  return suggestBaseNameFor(commonSongBase(names), state);
}
