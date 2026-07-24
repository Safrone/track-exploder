/**
 * Bulk export: the stems that reach the renderer must be the *aligned* ones.
 *
 * A bulk run never touches the preview engine — it decodes each song fresh — so
 * it has to measure and correct every song on its own. Getting that wrong would
 * bake a publisher's timing skew into a whole album's worth of exports, silently.
 */
import { describe, it, expect, beforeEach, vi } from "vitest";
import { PARTS, defaultPartMix, type Alignment, type Part, type PartMix } from "../types";
import type { MixerState } from "../mixer/store";

const SAMPLE_RATE = 44_100;
const FRAMES = SAMPLE_RATE * 4;

// --- stand-ins for everything that needs a webview or the native side --------

class FakeBuffer {
  readonly numberOfChannels = 1;
  private readonly data: Float32Array;
  constructor(
    readonly length: number,
    readonly sampleRate: number,
  ) {
    this.data = new Float32Array(length).fill(0.25);
  }
  get duration() {
    return this.length / this.sampleRate;
  }
  getChannelData() {
    return this.data;
  }
  copyToChannel(source: Float32Array) {
    this.data.set(source.subarray(0, this.data.length));
  }
}

const ctx = {
  sampleRate: SAMPLE_RATE,
  createBuffer: (_c: number, length: number, rate: number) => new FakeBuffer(length, rate),
} as unknown as BaseAudioContext;

/** Corrections the (mocked) native analyzer reports, keyed by part. */
let corrections: Partial<Record<Part, number>> = {};
/** Stem lengths handed to the renderer, per rendered song. */
const rendered: Record<Part, number>[] = [];

vi.mock("./decode", () => ({
  decodeStem: async () => new FakeBuffer(FRAMES, SAMPLE_RATE),
  readAudioTags: async () => ({ album: "Test" }),
}));

vi.mock("./tauri", () => ({
  invokeEncodeMix: async () => new ArrayBuffer(8),
  invokeEmbedTags: async () => {},
}));

vi.mock("./files", () => ({
  writeBytes: async () => {},
  isRealPath: () => true,
  extOf: (p: string) => p.split(".").pop(),
}));

vi.mock("../mixer/exports", () => ({
  addExport: () => {},
  setLastExportDir: () => {},
}));

vi.mock("./export", () => ({
  renderMix: async (source: { getBuffer: (p: Part) => { length: number } | undefined }) => {
    rendered.push(
      Object.fromEntries(PARTS.map((p) => [p, source.getBuffer(p)?.length ?? 0])) as Record<
        Part,
        number
      >,
    );
    return { pcm: new Float32Array(8), channels: 2, sampleRate: SAMPLE_RATE };
  },
}));

vi.mock("./align", async (importOriginal) => {
  const real = await importOriginal<typeof import("./align")>();
  return {
    ...real,
    // Stand in for the native measurement; the splice itself stays real.
    analyzeAlignment: async () => ({
      alignment: Object.fromEntries(
        Object.entries(corrections).map(([part, frames]) => [
          part,
          {
            offsetFrames: -frames!,
            spliceAt: 1000,
            deltaFrames: frames!,
            confidence: 0.8,
            consistent: true,
            spreadFrames: 0,
            sampleRate: SAMPLE_RATE,
          } satisfies Alignment,
        ]),
      ),
      sampleRate: SAMPLE_RATE,
      unsteady: [],
      unmeasured: [],
    }),
  };
});

const { bulkExport } = await import("./bulk");

function song(name: string) {
  return {
    key: name.toLowerCase(),
    name,
    parts: Object.fromEntries(
      PARTS.map((p) => [p, `/songs/${name} - ${p}.mp3`]),
    ) as Record<Part, string>,
    missing: [],
    extra: [],
  };
}

function state(): MixerState {
  return {
    tracks: {},
    mix: Object.fromEntries(PARTS.map((p) => [p, defaultPartMix()])) as Record<Part, PartMix>,
    alignment: {},
    masterGain: 1,
    tempoEnabled: false,
    tempo: 1,
    output: "stereo",
    sourceChannel: "left",
  };
}

const options = (align?: boolean) => ({
  ctx,
  state: state(),
  format: "wav" as const,
  bitDepth: 24,
  outputDir: "/out",
  align,
});

beforeEach(() => {
  rendered.length = 0;
  corrections = { lead: -6688, baritone: -13507, bass: -11470 };
});

describe("bulk export alignment", () => {
  it("renders each song from its corrected stems", async () => {
    const result = await bulkExport([song("Medley")], options());

    expect(result).toEqual({ exported: 1, failed: 0 });
    expect(rendered).toHaveLength(1);
    expect(rendered[0]).toEqual({
      tenor: FRAMES,
      lead: FRAMES - 6688,
      baritone: FRAMES - 13507,
      bass: FRAMES - 11470,
    });
  });

  it("measures every song, not just the first", async () => {
    const stages: string[] = [];
    await bulkExport([song("One"), song("Two")], options(), (info) => {
      if (info.stage === "aligning") stages.push(info.group.name);
    });
    expect(stages).toEqual(["One", "Two"]);
    expect(rendered).toHaveLength(2);
    expect(rendered[1].baritone).toBe(FRAMES - 13507);
  });

  it("leaves the stems alone when alignment is switched off", async () => {
    await bulkExport([song("Medley")], options(false));
    expect(rendered[0]).toEqual({
      tenor: FRAMES,
      lead: FRAMES,
      baritone: FRAMES,
      bass: FRAMES,
    });
  });

  it("exports a set that needs no correction unchanged", async () => {
    corrections = {};
    await bulkExport([song("Aligned")], options());
    expect(rendered[0].lead).toBe(FRAMES);
  });
});
