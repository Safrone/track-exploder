/**
 * The waveform lanes: one per voice, with the level split into what the left
 * channel gets (drawn upward) and what the right gets (drawn downward), so pan
 * is readable straight off the picture.
 */
import { describe, it, expect } from "vitest";
import { PARTS, defaultPartMix, type Part, type PartMix } from "../types";
import type { MixerState } from "../mixer/store";
import { partLanes, type StemSource } from "./waveform";

const SAMPLE_RATE = 44_100;

class FakeBuffer {
  private readonly samples: Float32Array;
  constructor(
    readonly length: number,
    peak: number,
  ) {
    this.samples = new Float32Array(length).fill(peak);
  }
  get duration() {
    return this.length / SAMPLE_RATE;
  }
  getChannelData() {
    return this.samples;
  }
}

/** Stems for the given parts, each a flat tone at `peak`. */
function source(parts: Partial<Record<Part, number>>): StemSource {
  return {
    getBuffer: (part) =>
      parts[part] === undefined
        ? undefined
        : (new FakeBuffer(4096, parts[part]!) as unknown as AudioBuffer),
  };
}

function state(over: Partial<Record<Part, Partial<PartMix>>> = {}): MixerState {
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
  };
}

const all = source({ tenor: 0.5, lead: 0.5, baritone: 0.5, bass: 0.5 });
const lanesOf = (s: MixerState, src: StemSource = all) => {
  const byPart = Object.fromEntries(partLanes(src, s, 64).map((l) => [l.part, l]));
  return byPart as Record<Part, ReturnType<typeof partLanes>[number]>;
};

describe("partLanes", () => {
  it("gives a lane to each loaded voice, in score order", () => {
    const lanes = partLanes(source({ tenor: 0.5, bass: 0.4 }), state(), 64);
    expect(lanes.map((l) => l.part)).toEqual(["tenor", "bass"]);
  });

  it("splits a centred part evenly above and below the line", () => {
    const lead = lanesOf(state()).lead;
    expect(lead.left).toBeCloseTo(Math.SQRT1_2, 5);
    expect(lead.right).toBeCloseTo(Math.SQRT1_2, 5);
  });

  it("leans a hard-left part upward and a hard-right part downward", () => {
    const lanes = lanesOf(state({ tenor: { pan: -1 }, bass: { pan: 1 } }));
    expect(lanes.tenor.left).toBeCloseTo(1, 5);
    expect(lanes.tenor.right).toBeCloseTo(0, 5);
    expect(lanes.bass.left).toBeCloseTo(0, 5);
    expect(lanes.bass.right).toBeCloseTo(1, 5);
  });

  it("scales the lane with the part's gain", () => {
    const lanes = lanesOf(state({ lead: { gain: 0.5 } }));
    expect(lanes.lead.left).toBeCloseTo(Math.SQRT1_2 * 0.5, 5);
    expect(lanes.lead.right).toBeCloseTo(Math.SQRT1_2 * 0.5, 5);
  });

  it("leaves the other lanes alone when one part is turned up", () => {
    // Each lane is drawn against its own stem, so a hot part grows on its own
    // rather than squashing everybody else to fit a shared scale.
    const before = lanesOf(state());
    const after = lanesOf(state({ lead: { gain: 2 } }));
    for (const part of ["tenor", "baritone", "bass"] as const) {
      expect(after[part].left).toBeCloseTo(before[part].left, 5);
      expect(after[part].peak).toBeCloseTo(before[part].peak, 5);
    }
    expect(after.lead.left).toBeCloseTo(before.lead.left * 2, 5);
  });

  it("carries each stem's own peak as the lane's reference height", () => {
    const lanes = partLanes(source({ tenor: 0.5, bass: 0.25 }), state(), 64);
    expect(lanes[0].peak).toBeCloseTo(0.5, 5);
    expect(lanes[1].peak).toBeCloseTo(0.25, 5);
  });

  it("flags a muted part as silent but keeps its shape", () => {
    const lanes = lanesOf(state({ lead: { included: false } }));
    expect(lanes.lead.silent).toBe(true);
    // Still drawable — greyed out, not collapsed to a flat line.
    expect(lanes.lead.left).toBeGreaterThan(0);
    expect(Math.max(...lanes.lead.peaks)).toBeGreaterThan(0);
    expect(lanes.bass.silent).toBe(false);
  });

  it("treats a part turned all the way down as silent too", () => {
    expect(lanesOf(state({ bass: { gain: 0 } })).bass.silent).toBe(true);
  });
});
