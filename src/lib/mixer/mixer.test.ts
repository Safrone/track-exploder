import { describe, it, expect, beforeEach } from "vitest";
import { PARTS, defaultPartMix, type Part, type PartMix } from "../types";
import {
  effectiveGain,
  setTrack,
  setPartMix,
  setAlignment,
  clearTrack,
  clearAllTracks,
  patchState,
  snapshot,
  type MixerState,
} from "./store";
import { interleave } from "../audio/export";

function stateWith(overrides: Partial<Record<Part, Partial<PartMix>>>): MixerState {
  const mix = Object.fromEntries(
    PARTS.map((p) => [p, { ...defaultPartMix(), ...(overrides[p] ?? {}) }]),
  ) as Record<Part, PartMix>;
  return {
    tracks: {},
    mix,
    alignment: {},
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

describe("clearing tracks", () => {
  function load(part: Part) {
    setTrack(part, { part, path: `/tracks/${part}.wav`, name: `${part}.wav`, channel: "left" });
    setAlignment(part, {
      offsetFrames: 0,
      spliceAt: 1000,
      deltaFrames: -441,
      confidence: 0.9,
      consistent: true,
      spreadFrames: 0,
      sampleRate: 44_100,
    });
  }

  beforeEach(() => {
    clearAllTracks();
    patchState({ sourceChannel: "left", output: "stereo" });
    for (const part of PARTS) load(part);
  });

  it("drops one part and its timing correction, leaving the rest loaded", () => {
    clearTrack("lead");
    const s = snapshot();
    expect(s.tracks.lead).toBeUndefined();
    expect(s.alignment.lead).toBeUndefined();
    expect(PARTS.filter((p) => s.tracks[p])).toEqual(["tenor", "baritone", "bass"]);
    expect(s.alignment.bass?.deltaFrames).toBe(-441);
  });

  it("drops every part at once", () => {
    clearAllTracks();
    const s = snapshot();
    expect(PARTS.filter((p) => s.tracks[p])).toEqual([]);
    expect(s.alignment).toEqual({});
  });

  it("puts the mix knobs back to their defaults", () => {
    setPartMix("bass", { gain: 0.5, included: false, pan: -0.8 });
    patchState({ masterGain: 0.4, tempoEnabled: true, tempo: 0.8 });
    clearAllTracks();

    const s = snapshot();
    expect(s.mix.bass).toEqual(defaultPartMix());
    expect(s.masterGain).toBe(1);
    expect(s.tempoEnabled).toBe(false);
    expect(s.tempo).toBe(1);
  });

  it("keeps the source channel and output mode, which describe the files, not the mix", () => {
    patchState({ sourceChannel: "right", output: "mono" });
    clearAllTracks();
    expect(snapshot()).toMatchObject({ sourceChannel: "right", output: "mono" });
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
