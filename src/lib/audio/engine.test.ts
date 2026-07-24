/**
 * Preview-engine wiring, driven through a stand-in AudioContext.
 *
 * The engine is where a timing correction actually reaches your ears: the store
 * holds the numbers, but `applyMix` has to re-splice the stems *and* the started
 * sources have to be the corrected ones. These tests replay the app's real
 * sequence — load, auto-align, play — and check what the sources get handed.
 */
import { describe, it, expect, beforeEach } from "vitest";
import { PARTS, defaultPartMix, type Alignment, type Part, type PartMix } from "../types";
import type { MixerState } from "../mixer/store";
import { MixEngine } from "./engine";

// --- a minimal Web Audio stand-in -------------------------------------------

class FakeBuffer {
  readonly numberOfChannels = 1;
  private readonly data: Float32Array;
  constructor(
    readonly length: number,
    readonly sampleRate: number,
    fill = 0,
  ) {
    this.data = new Float32Array(length).fill(fill);
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

interface StartedSource {
  buffer: FakeBuffer;
  when: number;
  offset: number;
}

class FakeContext {
  currentTime = 0;
  state = "running";
  destination = { connect() {}, disconnect() {} };
  readonly started: StartedSource[] = [];

  async resume() {
    this.state = "running";
  }
  createGain() {
    return { gain: { value: 1, setTargetAtTime() {} }, connect() {}, disconnect() {} };
  }
  createStereoPanner() {
    return { pan: { value: 0, setTargetAtTime() {} }, connect() {}, disconnect() {} };
  }
  createBuffer(_channels: number, length: number, sampleRate: number) {
    return new FakeBuffer(length, sampleRate);
  }
  createBufferSource() {
    const ctx = this;
    return {
      buffer: null as FakeBuffer | null,
      connect() {},
      disconnect() {},
      stop() {},
      start(when: number, offset: number) {
        ctx.started.push({ buffer: this.buffer!, when, offset });
      },
    };
  }
}

const SAMPLE_RATE = 44_100;
const FRAMES = SAMPLE_RATE * 4;

function state(alignment: Partial<Record<Part, Alignment>> = {}): MixerState {
  return {
    tracks: {},
    mix: Object.fromEntries(PARTS.map((p) => [p, defaultPartMix()])) as Record<Part, PartMix>,
    alignment,
    masterGain: 1,
    tempoEnabled: false,
    tempo: 1,
    output: "stereo",
    sourceChannel: "left",
  };
}

function correction(deltaFrames: number, spliceAt = 1000): Alignment {
  return {
    offsetFrames: -deltaFrames,
    spliceAt,
    deltaFrames,
    confidence: 0.8,
    consistent: true,
    spreadFrames: 0,
    sampleRate: SAMPLE_RATE,
  };
}

/** Let the engine's fire-and-forget restart finish. */
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

let engine: MixEngine;
let ctx: FakeContext;

beforeEach(() => {
  (globalThis as unknown as { AudioContext: unknown }).AudioContext = FakeContext;
  // The engine drives its playhead off rAF; in node it just needs to exist.
  (globalThis as unknown as { requestAnimationFrame: unknown }).requestAnimationFrame = () => 0;
  (globalThis as unknown as { cancelAnimationFrame: unknown }).cancelAnimationFrame = () => {};
  engine = new MixEngine();
  ctx = engine.ctx as unknown as FakeContext;
  for (const part of PARTS) {
    engine.setBuffer(part, new FakeBuffer(FRAMES, SAMPLE_RATE, 0.5) as unknown as AudioBuffer);
  }
  // The app pushes the mixer state in after every load; at this point nothing
  // is aligned yet.
  engine.applyMix(state());
});

describe("timing corrections", () => {
  it("re-splices the stem when a correction arrives", () => {
    engine.applyMix(state({ lead: correction(-6688) }));

    expect(engine.getBuffer("lead")!.length).toBe(FRAMES - 6688);
    expect(engine.getBuffer("bass")!.length).toBe(FRAMES);
  });

  it("plays the corrected stems, not the ones decoded from disk", async () => {
    engine.applyMix(state({ lead: correction(-6688), baritone: correction(-13507) }));
    await engine.play();

    const lengths = ctx.started.map((s) => s.buffer.length).sort((a, b) => a - b);
    expect(lengths).toEqual([FRAMES - 13507, FRAMES - 6688, FRAMES, FRAMES]);
  });

  it("restarts with every corrected stem when alignment lands mid-playback", async () => {
    // Auto-align finishes a couple of seconds after loading, so it often lands
    // while the track is already playing. Every corrected part has to be picked
    // up by the restart — not just the first one, and not only once some later
    // nudge happens to restart playback again.
    await engine.play();
    ctx.started.length = 0;

    engine.applyMix(
      state({
        lead: correction(-6688),
        baritone: correction(-13507),
        bass: correction(-11470),
      }),
    );
    await tick();

    expect(ctx.started).toHaveLength(PARTS.length);
    const lengths = ctx.started.map((s) => s.buffer.length).sort((a, b) => a - b);
    expect(lengths).toEqual([FRAMES - 13507, FRAMES - 11470, FRAMES - 6688, FRAMES]);
  });

  it("keeps playing the corrected stems when a later nudge touches one part", async () => {
    engine.applyMix(state({ lead: correction(-6688) }));
    await engine.play();
    ctx.started.length = 0;

    // Nudging the bass must not undo the lead's correction.
    engine.applyMix(state({ lead: correction(-6688), bass: correction(-441) }));
    await tick();

    expect(ctx.started).toHaveLength(PARTS.length);
    const lengths = ctx.started.map((s) => s.buffer.length).sort((a, b) => a - b);
    expect(lengths).toEqual([FRAMES - 6688, FRAMES - 441, FRAMES, FRAMES]);
  });

  it("rewinds to the start, so a new set doesn't open mid-song", async () => {
    await engine.play();
    engine.seek(2.5); // the fake stems are 4 s long
    expect(engine.position()).toBe(2.5);

    engine.rewind();
    expect(engine.position()).toBe(0);
    expect(engine.playing).toBe(false);

    ctx.started.length = 0;
    await engine.play();
    expect(ctx.started.every((s) => s.offset === 0)).toBe(true);
  });

  it("reports the new duration when it rewinds", () => {
    let seen: { at: number; of: number } | null = null;
    engine.onPosition = (at, of) => (seen = { at, of });
    engine.rewind();
    expect(seen).toEqual({ at: 0, of: FRAMES / SAMPLE_RATE });
  });

  it("re-splices from the decoded original, so nudges don't compound", () => {
    engine.applyMix(state({ lead: correction(-6688) }));
    engine.applyMix(state({ lead: correction(-7129) })); // nudged 10 ms further
    expect(engine.getBuffer("lead")!.length).toBe(FRAMES - 7129);
  });

  it("goes back to the untouched stem when the correction is cleared", () => {
    engine.applyMix(state({ lead: correction(-6688) }));
    engine.applyMix(state());
    expect(engine.getBuffer("lead")!.length).toBe(FRAMES);
  });

  it("applies a correction that arrives before the audio finishes decoding", () => {
    const fresh = new MixEngine();
    fresh.applyMix(state({ lead: correction(-6688) }));
    fresh.setBuffer("lead", new FakeBuffer(FRAMES, SAMPLE_RATE, 0.5) as unknown as AudioBuffer);
    expect(fresh.getBuffer("lead")!.length).toBe(FRAMES - 6688);
  });
});
