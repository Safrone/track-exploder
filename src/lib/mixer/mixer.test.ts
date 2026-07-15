import { describe, it, expect } from "vitest";
import { PARTS, defaultPartMix, type Part, type PartMix } from "../types";
import { effectiveGain, type MixerState } from "./store";
import { interleave } from "../audio/export";

function stateWith(overrides: Partial<Record<Part, Partial<PartMix>>>): MixerState {
  const mix = Object.fromEntries(
    PARTS.map((p) => [p, { ...defaultPartMix(), ...(overrides[p] ?? {}) }]),
  ) as Record<Part, PartMix>;
  return {
    tracks: {},
    mix,
    masterGain: 1,
    tempoEnabled: false,
    tempo: 1,
    output: "stereo",
    sourceChannel: "left",
  };
}

describe("effectiveGain", () => {
  it("returns the set gain when the part is audible", () => {
    const s = stateWith({ lead: { gain: 0.8 } });
    expect(effectiveGain(s, "lead")).toBe(0.8);
  });

  it("is 0 when the part is muted (not included)", () => {
    const s = stateWith({ bass: { included: false, gain: 1 } });
    expect(effectiveGain(s, "bass")).toBe(0);
  });
});

describe("interleave", () => {
  it("passes through a single channel unchanged", () => {
    const mono = new Float32Array([0.1, 0.2, 0.3]);
    expect(interleave([mono])).toBe(mono);
  });

  it("interleaves stereo frame by frame", () => {
    const l = new Float32Array([1, 3, 5]);
    const r = new Float32Array([2, 4, 6]);
    expect(Array.from(interleave([l, r]))).toEqual([1, 2, 3, 4, 5, 6]);
  });
});
