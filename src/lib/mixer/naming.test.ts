import { describe, it, expect } from "vitest";
import { PARTS, defaultPartMix, type Part, type PartMix } from "../types";
import type { MixerState } from "./store";
import { commonSongBase, describeMix, suggestBaseName } from "./naming";

function state(over: {
  mix?: Partial<Record<Part, Partial<PartMix>>>;
  tempo?: number;
  output?: "stereo" | "mono";
  names?: Partial<Record<Part, string>>;
}): MixerState {
  const mix = Object.fromEntries(
    PARTS.map((p) => [p, { ...defaultPartMix(), ...(over.mix?.[p] ?? {}) }]),
  ) as Record<Part, PartMix>;
  const tracks = Object.fromEntries(
    PARTS.filter((p) => over.names?.[p]).map((p) => [
      p,
      { part: p, path: `/x/${over.names![p]}`, name: over.names![p]!, channel: "left" as const },
    ]),
  );
  return {
    tracks,
    mix,
    masterGain: 1,
    tempo: over.tempo ?? 1,
    preservePitch: true,
    output: over.output ?? "stereo",
  };
}

const REAL_NAMES = {
  tenor: "01 Lets Burn Up the Town [C] - TENOR [Tim Waurick] [20250716].mp3",
  lead: "01 Lets Burn Up the Town [C] - LEAD [Tim Waurick] [20250716].mp3",
  baritone: "01 Lets Burn Up the Town [C] - BARI [Tim Waurick] [20250716].mp3",
  bass: "01 Lets Burn Up the Town [C] - BASS [Tim Waurick] [20250716].mp3",
};

describe("commonSongBase", () => {
  it("extracts the shared song title before the part token", () => {
    expect(commonSongBase(Object.values(REAL_NAMES))).toBe("01 Lets Burn Up the Town [C]");
  });

  it("falls back to 'mix' when no names", () => {
    expect(commonSongBase([])).toBe("mix");
  });
});

describe("describeMix", () => {
  it("names a solo", () => {
    expect(describeMix(state({ mix: { bass: { soloed: true } } }))).toBe("Bass solo");
  });

  it("names a part-off mix", () => {
    expect(describeMix(state({ mix: { lead: { included: false } } }))).toBe("Lead off");
  });

  it("detects a predominant part", () => {
    expect(
      describeMix(
        state({
          mix: {
            lead: { gain: 1 },
            tenor: { gain: 0.35 },
            baritone: { gain: 0.35 },
            bass: { gain: 0.35 },
          },
        }),
      ),
    ).toBe("Lead predominant");
  });

  it("calls the default full mix", () => {
    expect(describeMix(state({}))).toBe("full mix");
  });
});

describe("suggestBaseName", () => {
  it("combines song, descriptor, tempo and mono", () => {
    const s = state({
      names: REAL_NAMES,
      mix: { lead: { included: false } },
      tempo: 0.85,
      output: "mono",
    });
    expect(suggestBaseName(s)).toBe("01 Lets Burn Up the Town [C] - Lead off 85pct mono");
  });

  it("has no filesystem-invalid characters", () => {
    const s = suggestBaseName(state({ names: REAL_NAMES }));
    expect(s).not.toMatch(/[<>:"/\\|?*]/);
  });
});
