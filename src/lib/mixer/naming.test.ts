import { describe, it, expect } from "vitest";
import { PARTS, defaultPartMix, type Part, type PartMix } from "../types";
import type { MixerState } from "./store";
import { commonSongBase, describeMix, suggestBaseName } from "./naming";

function state(over: {
  mix?: Partial<Record<Part, Partial<PartMix>>>;
  tempo?: number;
  tempoEnabled?: boolean;
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
    alignment: {},
    masterGain: 1,
    tempoEnabled: over.tempoEnabled ?? false,
    tempo: over.tempo ?? 1,
    output: over.output ?? "stereo",
    sourceChannel: "left",
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

  const BRACKETED = ["Tenor", "Lead", "Bari", "Bass"].map(
    (p) => `Look For The Light - (${p}) Left Pitched Down.mp3`,
  );

  it("drops the bracket that opened the part token", () => {
    expect(commonSongBase(BRACKETED)).toBe("Look For The Light");
  });

  it("doesn't keep half a part name when only some parts are loaded", () => {
    // BARI and BASS share "Ba", which the old common-prefix rule kept.
    expect(commonSongBase(BRACKETED.slice(2))).toBe("Look For The Light");
  });

  it("finds the title in a single file's name", () => {
    expect(commonSongBase([BRACKETED[3]])).toBe("Look For The Light");
    expect(commonSongBase([REAL_NAMES.bass])).toBe("01 Lets Burn Up the Town [C]");
  });

  it("cuts at the part token, not a part word in the title", () => {
    const names = ["TENOR", "LEAD", "BARI", "BASS"].map((p) => `Lead Me Home - ${p}.mp3`);
    expect(commonSongBase(names)).toBe("Lead Me Home");
  });

  it("keeps a name with no part token whole", () => {
    expect(commonSongBase(["Look For The Light.mp3"])).toBe("Look For The Light");
  });
});

describe("describeMix", () => {
  it("names a solo", () => {
    expect(
      describeMix(
        state({
          mix: {
            tenor: { included: false },
            lead: { included: false },
            baritone: { included: false },
          },
        }),
      ),
    ).toBe("Bass solo");
  });

  it("names a part-off mix", () => {
    expect(describeMix(state({ mix: { lead: { included: false } } }))).toBe("no Lead");
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
      tempoEnabled: true,
      tempo: 0.85,
      output: "mono",
    });
    expect(suggestBaseName(s)).toBe("01 Lets Burn Up the Town [C] - no Lead 85pct mono");
  });

  it("has no filesystem-invalid characters", () => {
    const s = suggestBaseName(state({ names: REAL_NAMES }));
    expect(s).not.toMatch(/[<>:"/\\|?*]/);
  });
});
