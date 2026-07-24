/**
 * The offline mixdown — where a part's pan and gain actually reach the exported
 * file. This used to lean on `OfflineAudioContext`'s `StereoPannerNode`, which
 * WebKitGTK renders as a passthrough, so exports came out centred. The mixdown
 * is now plain arithmetic; these tests pin the pan/gain math down.
 */
import { describe, it, expect } from "vitest";
import { PARTS, defaultPartMix, type Part, type PartMix } from "../types";
import type { MixerState } from "../mixer/store";
import { interleave, renderMix, type RenderSource } from "./export";

const SAMPLE_RATE = 48_000;

/** A source whose stems are flat DC tones, so a channel's value is just its gain. */
function sourceOf(parts: Partial<Record<Part, number>>, length = 100): RenderSource {
  const buffers = Object.fromEntries(
    Object.entries(parts).map(([part, level]) => {
      const data = new Float32Array(length).fill(level!);
      return [part, { length, duration: length / SAMPLE_RATE, getChannelData: () => data }];
    }),
  ) as Record<Part, { length: number; duration: number; getChannelData: () => Float32Array }>;
  return {
    ctx: { sampleRate: SAMPLE_RATE } as BaseAudioContext,
    duration: length / SAMPLE_RATE,
    getBuffer: (p) => buffers[p] as unknown as AudioBuffer,
  };
}

function state(over: Partial<Record<Part, Partial<PartMix>>> = {}, extra: Partial<MixerState> = {}): MixerState {
  return {
    tracks: {},
    mix: Object.fromEntries(
      PARTS.map((p) => [p, { ...defaultPartMix(), ...(over[p] ?? {}) }]),
    ) as Record<Part, PartMix>,
    alignment: {},
    masterGain: 1,
    tempoEnabled: false,
    tempo: 1,
    output: "stereo",
    sourceChannel: "left",
    ...extra,
  };
}

/** First-frame [L, R] of a stereo render. */
function frame0(pcm: Float32Array): [number, number] {
  return [pcm[0], pcm[1]];
}

describe("renderMix panning", () => {
  it("sends a hard-left part to the left channel and a hard-right part to the right", async () => {
    // The reported mix: lead panned left, tenor panned right, the rest muted.
    const s = state({
      lead: { pan: -1 },
      tenor: { pan: 1 },
      baritone: { included: false },
      bass: { included: false },
    });
    const { pcm, channels } = await renderMix(sourceOf({ lead: 0.5, tenor: 0.5 }), s, "stereo");
    expect(channels).toBe(2);
    const [left, right] = frame0(pcm);
    expect(left).toBeCloseTo(0.5, 5); // lead only
    expect(right).toBeCloseTo(0.5, 5); // tenor only
  });

  it("keeps a hard-panned part out of the opposite channel entirely", async () => {
    const s = state({
      lead: { pan: -1 },
      tenor: { pan: 1 },
      baritone: { included: false },
      bass: { included: false },
    });
    // Distinct levels so we can see which voice landed where.
    const { pcm } = await renderMix(sourceOf({ lead: 0.4, tenor: 0.9 }), s, "stereo");
    const [left, right] = frame0(pcm);
    expect(left).toBeCloseTo(0.4, 5);
    expect(right).toBeCloseTo(0.9, 5);
  });

  it("splits a centred part equally with equal power", async () => {
    const s = state({ tenor: { included: false }, baritone: { included: false }, bass: { included: false } });
    const { pcm } = await renderMix(sourceOf({ lead: 1 }), s, "stereo");
    const [left, right] = frame0(pcm);
    expect(left).toBeCloseTo(Math.SQRT1_2, 5);
    expect(right).toBeCloseTo(Math.SQRT1_2, 5);
  });

  it("leaves muted parts out of the mix", async () => {
    const s = state({
      lead: { pan: -1 },
      tenor: { included: false },
      baritone: { included: false },
      bass: { included: false },
    });
    const { pcm } = await renderMix(sourceOf({ lead: 0.5, tenor: 0.5 }), s, "stereo");
    const [, right] = frame0(pcm);
    expect(right).toBe(0); // tenor was muted, nothing on the right
  });

  it("scales the whole mix by the master gain", async () => {
    const s = state({ lead: { pan: -1 } }, { masterGain: 0.5 });
    const { pcm } = await renderMix(sourceOf({ lead: 1 }), s, "stereo");
    expect(pcm[0]).toBeCloseTo(0.5, 5);
  });
});

describe("renderMix output modes", () => {
  it("sums to a single channel for mono output", async () => {
    const s = state({ lead: { pan: -1 }, tenor: { pan: 1 }, baritone: { included: false }, bass: { included: false } });
    const { pcm, channels } = await renderMix(sourceOf({ lead: 0.5, tenor: 0.5 }), s, "mono");
    expect(channels).toBe(1);
    // (left + right) / 2 = (0.5 + 0.5) / 2
    expect(pcm[0]).toBeCloseTo(0.5, 5);
  });

  it("mixes over the longest stem when parts differ in length (post-alignment)", async () => {
    const src = sourceOf({ lead: 0.5 }, 100);
    // A bass that runs longer than the lead, as an aligned stem can.
    const long = new Float32Array(160).fill(0.5);
    const base = src.getBuffer;
    src.getBuffer = (p) =>
      p === "bass"
        ? ({ length: 160, duration: 160 / SAMPLE_RATE, getChannelData: () => long } as unknown as AudioBuffer)
        : base(p);
    const s = state({ tenor: { included: false }, baritone: { included: false } });
    const { pcm } = await renderMix(src, s, "stereo");
    expect(pcm.length).toBe(160 * 2); // covered the longer stem
    // Past the lead's end, only the bass remains.
    expect(pcm[150 * 2]).toBeCloseTo(0.5 * Math.SQRT1_2, 5);
  });
});

describe("interleave", () => {
  it("passes a single channel through unchanged", () => {
    const mono = new Float32Array([1, 2, 3]);
    expect(interleave([mono])).toBe(mono);
  });

  it("interleaves L and R frame by frame", () => {
    const l = new Float32Array([1, 3, 5]);
    const r = new Float32Array([2, 4, 6]);
    expect(Array.from(interleave([l, r]))).toEqual([1, 2, 3, 4, 5, 6]);
  });
});
