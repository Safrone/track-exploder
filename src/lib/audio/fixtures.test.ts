/**
 * Frontend tests driven by the **real** generated learning-track set: a
 * circle-of-fifths barbershop progression with rests, written to disk by
 * `cargo run -p audio-core --example generate_fixtures`.
 *
 * These cover the parts of the pipeline that live in TypeScript — part
 * detection, song grouping, export naming, tag merging and the mix math — using
 * the same files the Rust suite decodes, instead of hand-made buffers.
 */
import { it, expect } from "vitest";
import {
  decodeWav,
  describeWithFixtures,
  fixturePath,
  manifest,
  peak,
  steadyRms,
} from "../test/fixtures";
import { PARTS, defaultPartMix, type OutputMode, type Part, type PartMix } from "../types";
import { basename, guessPart, guessPartFromTags } from "./load";
import { extOf } from "./files";
import { groupFiles } from "./bulk";
import { spliceSamples } from "./align";
import { interleave } from "./export";
import { effectiveGain, type MixerState } from "../mixer/store";
import { commonSongBase, suggestBaseName } from "../mixer/naming";
import { computeCommon } from "../mixer/tags";

/** Non-null inside `describeWithFixtures` — the suite is skipped otherwise. */
const set = manifest!;

function partLeft(part: Part): string {
  return fixturePath(set.files.partLeft[part]);
}

interface StateOverrides {
  mix?: Partial<Record<Part, Partial<PartMix>>>;
  tempoEnabled?: boolean;
  tempo?: number;
  output?: OutputMode;
}

function stateFor(over: StateOverrides): MixerState {
  const mix = Object.fromEntries(
    PARTS.map((p) => [p, { ...defaultPartMix(), ...(over.mix?.[p] ?? {}) }]),
  ) as Record<Part, PartMix>;
  const tracks = Object.fromEntries(
    PARTS.map((p) => [
      p,
      {
        part: p,
        path: partLeft(p),
        name: basename(set.files.partLeft[p]),
        channel: "left" as const,
      },
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

describeWithFixtures("part detection on real learning-track files", () => {
  it("reads the part out of every vendor-style filename", () => {
    for (const part of PARTS) {
      for (const group of [set.files.partLeft, set.files.partRight, set.files.flac]) {
        expect(guessPart(basename(group[part]))).toBe(part);
      }
    }
  });

  it("falls back to the artist tag when the name doesn't help", () => {
    for (const part of PARTS) {
      expect(guessPartFromTags(set.tags[part])).toBe(part);
      // The tag really does carry the voice, not the song.
      expect(guessPart(set.tags[part].title)).toBeNull();
    }
  });

  it("sees the container extension", () => {
    expect(extOf(partLeft("lead"))).toBe("wav");
    expect(extOf(fixturePath(set.files.flac.lead))).toBe("flac");
  });
});

describeWithFixtures("grouping a folder of real files", () => {
  it("collects the four parts into one complete song", () => {
    const { groups, ungrouped } = groupFiles(PARTS.map(partLeft));
    expect(ungrouped).toEqual([]);
    expect(groups).toHaveLength(1);
    expect(groups[0].name).toBe(set.songBase);
    expect(groups[0].missing).toEqual([]);
    for (const part of PARTS) {
      expect(groups[0].parts[part]).toBe(partLeft(part));
    }
  });

  it("treats a second copy of the same song as duplicates, not a new song", () => {
    const both = [...PARTS.map(partLeft), ...PARTS.map((p) => fixturePath(set.files.partRight[p]))];
    const { groups } = groupFiles(both);
    expect(groups).toHaveLength(1);
    expect(groups[0].missing).toEqual([]);
    expect(groups[0].extra).toHaveLength(4);
  });
});

describeWithFixtures("naming an export from a real set", () => {
  it("recovers the song title from the four filenames", () => {
    expect(commonSongBase(PARTS.map((p) => basename(set.files.partLeft[p])))).toBe(set.songBase);
  });

  it("describes a slowed-down, part-missing practice mix", () => {
    const state = stateFor({
      mix: { lead: { included: false } },
      tempoEnabled: true,
      tempo: 0.85,
      output: "mono",
    });
    expect(suggestBaseName(state)).toBe(`${set.songBase} - no Lead 85pct mono`);
  });
});

describeWithFixtures("tags carried across the four sources", () => {
  it("keeps what the parts share and drops the per-voice artist", () => {
    const common = computeCommon(set.tags);
    expect(common.album).toBe(set.tags.lead.album);
    expect(common.title).toBe(set.song);
    expect(common.genre).toBe(set.tags.lead.genre);
    expect(common.artist).toBeUndefined();
  });
});

describeWithFixtures("the audio itself", () => {
  it("decodes four aligned stereo tracks", () => {
    for (const part of PARTS) {
      const audio = decodeWav(partLeft(part));
      expect(audio.channels).toBe(2);
      expect(audio.sampleRate).toBe(set.sampleRate);
      expect(audio.frames).toBe(set.frames);
    }
  });

  it("is silent exactly where the part rests, and sings everywhere else", () => {
    for (const part of PARTS) {
      const isolated = decodeWav(partLeft(part)).planar[0];

      for (const [start, end] of set.rests[part]) {
        expect(peak(isolated, start, end)).toBe(0);
      }
      for (const event of set.events) {
        if (!event.notes[part]) continue;
        expect(steadyRms(isolated, event.startFrame, event.endFrame)).toBeGreaterThan(0.02);
      }
    }
  });

  it("goes quiet on both channels when the whole quartet rests", () => {
    for (const part of PARTS) {
      const audio = decodeWav(partLeft(part));
      for (const [start, end] of set.allRest) {
        expect(peak(audio.planar[0], start, end)).toBe(0);
        expect(peak(audio.planar[1], start, end)).toBe(0);
      }
    }
  });

  it("mixes the extracted voices back into the published part-predominant side", () => {
    // "Sing the lead yourself": mute the lead, keep the other three at unity.
    // That is exactly what the lead file already carries on its right channel.
    const state = stateFor({ mix: { lead: { included: false } } });
    const mixed = new Float32Array(set.frames);
    for (const part of PARTS) {
      const gain = effectiveGain(state, part);
      if (gain === 0) continue;
      const isolated = decodeWav(partLeft(part)).planar[0];
      for (let i = 0; i < mixed.length; i++) mixed[i] += isolated[i] * gain;
    }

    const published = decodeWav(partLeft("lead")).planar[1];
    let worst = 0;
    for (let i = 0; i < mixed.length; i++) worst = Math.max(worst, Math.abs(mixed[i] - published[i]));
    expect(worst).toBeLessThan(1e-5);
  });

  it("interleaves real stereo frames in order", () => {
    const audio = decodeWav(partLeft("bass"));
    const [left, right] = audio.planar;
    const frames = 1000;
    const out = interleave([left.subarray(0, frames), right.subarray(0, frames)]);
    expect(out.length).toBe(frames * 2);
    for (const i of [0, 1, 500, 999]) {
      expect(out[i * 2]).toBe(left[i]);
      expect(out[i * 2 + 1]).toBe(right[i]);
    }
  });

  it("re-aligns a misaligned set by splicing the surplus silence out", () => {
    // The `misaligned/` files are shaped like a publisher's: spoken title, pitch
    // pipe, a gap — with the song pasted in late in three of the four. Cutting
    // that surplus out of the gap is exactly what the app does with the offsets
    // the native analyzer reports.
    for (const part of PARTS) {
      const skewed = decodeWav(fixturePath(set.files.misaligned[part]));
      const extra = set.misalignedExtraFrames[part];
      const aligned = decodeWav(partLeft(part)).planar[0];

      // Cut inside the gap, a moment before the song starts.
      const spliceAt = skewed.frames - set.frames - extra - 100;
      const fixed = spliceSamples(skewed.planar[0], spliceAt, -extra);

      expect(fixed.length).toBe(skewed.frames - extra);
      // Everything from the song onwards now matches the aligned release.
      const songStart = fixed.length - set.frames;
      let worst = 0;
      for (let i = 0; i < set.frames; i++) {
        worst = Math.max(worst, Math.abs(fixed[songStart + i] - aligned[i]));
      }
      expect(worst).toBeLessThan(2e-4);
    }
  });

  it("leaves a stem untouched when there is nothing to correct", () => {
    const samples = decodeWav(partLeft("bass")).planar[0];
    expect(spliceSamples(samples, 1000, 0)).toBe(samples);
  });

  it("finds the same voice on the other channel of the part-right set", () => {
    for (const part of PARTS) {
      const left = decodeWav(partLeft(part)).planar[0];
      const swapped = decodeWav(fixturePath(set.files.partRight[part])).planar;

      let sameSide = 0;
      let otherSide = 0;
      for (let i = 0; i < left.length; i++) {
        sameSide = Math.max(sameSide, Math.abs(left[i] - swapped[0][i]));
        otherSide = Math.max(otherSide, Math.abs(left[i] - swapped[1][i]));
      }
      // The isolated voice moved to the right; the left now holds the other three.
      expect(otherSide).toBeLessThan(1e-4);
      expect(sameSide).toBeGreaterThan(0.05);
    }
  });
});
