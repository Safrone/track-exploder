import { PARTS, type Part } from "../types";
import type { MixerState } from "./store";

/** Words that name each part in a learning track's filename or tags. */
export const PART_KEYWORDS: Record<Part, string[]> = {
  tenor: ["tenor", "ten"],
  lead: ["lead", "melody"],
  baritone: ["baritone", "bari", "bar"],
  bass: ["bass"],
};

// Any part keyword as a whole word: "TENOR", "(Bass)", "[bari]".
const PART_TOKEN = new RegExp(
  `\\b(?:${PARTS.flatMap((p) => PART_KEYWORDS[p]).join("|")})\\b`,
  "gi",
);

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
 * The song title in one part file's name: everything before its part token. It's
 * the last token that counts, since a title can name a part too ("Lead Me Home -
 * BASS"). A name with no token, or one that starts with it, is kept whole.
 */
function titleBeforePart(name: string): string {
  let cut = -1;
  for (const m of name.matchAll(PART_TOKEN)) cut = m.index;
  return cut > 0 ? name.slice(0, cut) : name;
}

/**
 * Derive a song base name from the loaded track filenames: the title before the
 * part token (TENOR, "(Bass)", ...), shared across the files. What follows the
 * token (arranger, date, "Left Pitched Down") is dropped, along with the
 * separator and any opening bracket left dangling where the token was cut.
 */
export function commonSongBase(names: string[]): string {
  const bare = names.map(stripExt).filter(Boolean);
  if (bare.length === 0) return "mix";

  const titles = bare.map(titleBeforePart);
  let prefix = titles[0];
  for (const s of titles) {
    while (prefix && !s.startsWith(prefix)) prefix = prefix.slice(0, -1);
  }
  const base = sanitize(prefix.replace(/[\s\-_–—([{]+$/, ""));
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
