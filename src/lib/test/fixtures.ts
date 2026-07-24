/**
 * Access to the generated audio fixtures (test-only).
 *
 * The fixture set is a real circle-of-fifths barbershop learning-track set —
 * four part-predominant stereo files plus a manifest — written by the Rust
 * generator so both test suites work off the very same media:
 *
 * ```bash
 * cargo run -p audio-core --example generate_fixtures
 * ```
 *
 * If the files are missing we try to generate them; when Rust isn't installed
 * the fixture-backed suites skip rather than fail (see {@link describeWithFixtures}).
 */
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe } from "vitest";
import type { Part } from "../types";

export interface FixtureManifest {
  version: number;
  song: string;
  songBase: string;
  /** Key of the arrangement, e.g. "Bb". */
  key: string;
  sampleRate: number;
  bpm: number;
  frames: number;
  durationSecs: number;
  parts: Part[];
  files: {
    partLeft: Record<Part, string>;
    partRight: Record<Part, string>;
    flac: Record<Part, string>;
    mp3: Partial<Record<Part, string>>;
    reference: Record<Part, string>;
    /** Publisher-shaped copies whose songs are pasted in late. */
    misaligned: Record<Part, string>;
  };
  /** Surplus silence each `misaligned/` file carries, in frames. */
  misalignedExtraFrames: Record<Part, number>;
  tags: Record<Part, Record<string, string>>;
  events: {
    /** Chord name, e.g. "G7" (or "(breath)" for a rest). */
    label: string;
    /** Syllable sung on this chord; null for a rest or a swipe (held word). */
    lyric: string | null;
    /** Chord root pitch class; null for a rest. */
    root: string | null;
    /** True when the chord changes inside a held word — a barbershop swipe. */
    swipe: boolean;
    beats: number;
    startFrame: number;
    endFrame: number;
    notes: Partial<Record<Part, string>>;
    /** Justly-tuned frequency each voice sings. */
    hz: Partial<Record<Part, number>>;
  }[];
  /** Frame spans `[start, end)` where each part rests. */
  rests: Record<Part, [number, number][]>;
  /** Frame spans where every part rests. */
  allRest: [number, number][];
}

const repoRoot = resolve(dirname(fileURLToPath(import.meta.url)), "../../..");

export const fixturesRoot = process.env.TRACK_EXPLODER_FIXTURES ?? join(repoRoot, "samples/fixtures");

function generate(): void {
  execFileSync(
    "cargo",
    ["run", "-q", "-p", "audio-core", "--example", "generate_fixtures", "--", fixturesRoot],
    { cwd: repoRoot, stdio: "inherit" },
  );
}

function load(): FixtureManifest | null {
  const manifest = join(fixturesRoot, "fixtures.json");
  if (!existsSync(manifest)) {
    try {
      generate();
    } catch (e) {
      console.warn(
        `[fixtures] no audio fixtures and could not run the generator (${e}); ` +
          "skipping the fixture-backed tests. Run `cargo run -p audio-core " +
          "--example generate_fixtures` to enable them.",
      );
      return null;
    }
  }
  return JSON.parse(readFileSync(manifest, "utf8")) as FixtureManifest;
}

export const manifest: FixtureManifest | null = load();

/** `describe`, skipped when the fixtures could not be generated. */
export const describeWithFixtures = describe.skipIf(!manifest);

/** Absolute path of a file listed in the manifest. */
export function fixturePath(relative: string): string {
  return join(fixturesRoot, relative);
}

export interface DecodedWav {
  sampleRate: number;
  channels: number;
  frames: number;
  /** `planar[ch]` holds channel `ch`, normalized to roughly -1..1. */
  planar: Float32Array<ArrayBuffer>[];
}

/**
 * Minimal PCM WAV reader (16- and 24-bit), enough to read the fixtures without
 * pulling a decoder into the frontend's dependency tree. Sample scaling matches
 * Symphonia's (divide by 2^(bits-1)), so values line up with what the app sees.
 */
export function decodeWav(path: string): DecodedWav {
  const buf = readFileSync(path);
  if (buf.toString("ascii", 0, 4) !== "RIFF" || buf.toString("ascii", 8, 12) !== "WAVE") {
    throw new Error(`${path} is not a RIFF/WAVE file`);
  }

  let channels = 0;
  let sampleRate = 0;
  let bits = 0;
  let data: Buffer | null = null;

  let at = 12;
  while (at + 8 <= buf.length) {
    const id = buf.toString("ascii", at, at + 4);
    const size = buf.readUInt32LE(at + 4);
    const body = buf.subarray(at + 8, at + 8 + size);
    if (id === "fmt ") {
      channels = body.readUInt16LE(2);
      sampleRate = body.readUInt32LE(4);
      bits = body.readUInt16LE(14);
    } else if (id === "data") {
      data = body;
    }
    at += 8 + size + (size % 2); // chunks are word-aligned
  }

  if (!data || !channels || !bits) throw new Error(`${path} has no PCM data`);
  if (bits !== 16 && bits !== 24) throw new Error(`${path}: unsupported bit depth ${bits}`);

  const bytes = bits / 8;
  const frames = Math.floor(data.length / (bytes * channels));
  const scale = 1 / 2 ** (bits - 1);
  const planar = Array.from({ length: channels }, () => new Float32Array(frames));

  for (let i = 0; i < frames; i++) {
    for (let c = 0; c < channels; c++) {
      const offset = (i * channels + c) * bytes;
      const value =
        bits === 16 ? data.readInt16LE(offset) : (data.readIntLE(offset, 3) as number);
      planar[c][i] = value * scale;
    }
  }

  return { sampleRate, channels, frames, planar };
}

/** Largest absolute sample in a span. */
export function peak(samples: Float32Array, start = 0, end = samples.length): number {
  let max = 0;
  for (let i = start; i < Math.min(end, samples.length); i++) max = Math.max(max, Math.abs(samples[i]));
  return max;
}

/** RMS level over a span, ignoring the first and last fifth (note attack/release). */
export function steadyRms(samples: Float32Array, start: number, end: number): number {
  const margin = Math.floor((end - start) / 5);
  const from = start + margin;
  const to = Math.min(end - margin, samples.length);
  let sum = 0;
  for (let i = from; i < to; i++) sum += samples[i] * samples[i];
  return to > from ? Math.sqrt(sum / (to - from)) : 0;
}
