import { PARTS, type Part } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";
import { createStretch, type StretchNode } from "./stretch";

/**
 * Live preview engine built on the Web Audio API.
 *
 * Graph per part: <source> → GainNode → StereoPannerNode → mixBus → masterGain
 * → destination. Gain/pan/mute stay live because they sit after the source.
 *
 * Two source types:
 *  - **plain** (default): AudioBufferSourceNode at natural rate. No stretch
 *    processing is created at all — used whenever tempo is disabled.
 *  - **stretch**: one Signalsmith Stretch node per stem (scheduled mode) for
 *    pitch-preserving tempo. Only built the first time tempo is enabled.
 *
 * The playhead advances at the active rate (tempo when stretching, else 1).
 */
export class MixEngine {
  readonly ctx: AudioContext;
  private readonly masterGain: GainNode;
  private readonly mixBus: GainNode;
  private readonly partGain = new Map<Part, GainNode>();
  private readonly partPan = new Map<Part, StereoPannerNode>();
  private readonly buffers = new Map<Part, AudioBuffer>();

  private readonly sources = new Map<Part, AudioBufferSourceNode>();
  private readonly stretch = new Map<Part, StretchNode>();
  private prepared = false;
  private preparing: Promise<void> | null = null;

  playing = false;
  private stretchEnabled = false;
  private tempo = 1;
  private startedAtCtx = 0;
  /** Playhead in *source* seconds when last paused/seeked. */
  private pausedAt = 0;
  private rafId = 0;

  onPosition?: (seconds: number, duration: number) => void;
  onEnded?: () => void;

  constructor() {
    this.ctx = new AudioContext();
    this.masterGain = this.ctx.createGain();
    this.mixBus = this.ctx.createGain();
    this.mixBus.connect(this.masterGain);
    this.masterGain.connect(this.ctx.destination);

    for (const part of PARTS) {
      const g = this.ctx.createGain();
      const p = this.ctx.createStereoPanner();
      g.connect(p);
      p.connect(this.mixBus);
      this.partGain.set(part, g);
      this.partPan.set(part, p);
    }
  }

  setBuffer(part: Part, buffer: AudioBuffer): void {
    this.buffers.set(part, buffer);
    this.prepared = false;
  }

  hasBuffer(part: Part): boolean {
    return this.buffers.has(part);
  }

  getBuffer(part: Part): AudioBuffer | undefined {
    return this.buffers.get(part);
  }

  /** Longest loaded stem, in seconds. */
  get duration(): number {
    let max = 0;
    for (const b of this.buffers.values()) max = Math.max(max, b.duration);
    return max;
  }

  /** Effective playback rate (only stretch mode applies tempo). */
  private get rate(): number {
    return this.stretchEnabled ? this.tempo : 1;
  }

  /** Apply gain/pan/master/tempo from mixer state. Safe to call any time. */
  applyMix(state: MixerState): void {
    const now = this.ctx.currentTime;
    for (const part of PARTS) {
      this.partGain.get(part)!.gain.setTargetAtTime(effectiveGain(state, part), now, 0.01);
      this.partPan.get(part)!.pan.setTargetAtTime(state.mix[part].pan, now, 0.01);
    }
    this.masterGain.gain.setTargetAtTime(state.masterGain, now, 0.01);
    this.setStretchEnabled(state.tempoEnabled);
    this.setTempo(state.tempo);
  }

  /** Toggle pitch-preserving tempo processing. Restarts playback if needed. */
  private setStretchEnabled(enabled: boolean): void {
    if (enabled === this.stretchEnabled) return;
    const wasPlaying = this.playing;
    if (wasPlaying) {
      this.pausedAt = this.position();
      this.stopActive();
      this.playing = false;
      cancelAnimationFrame(this.rafId);
    }
    this.stretchEnabled = enabled;
    if (wasPlaying) void this.play();
  }

  /** Change tempo, keeping the playhead continuous. rate<1 = slower. */
  setTempo(tempo: number): void {
    if (tempo === this.tempo) return;
    if (this.playing && this.stretchEnabled) {
      this.pausedAt = this.position();
      this.startedAtCtx = this.ctx.currentTime;
    }
    this.tempo = tempo;
    if (this.playing && this.stretchEnabled) {
      for (const s of this.stretch.values()) s.schedule({ rate: tempo });
    }
  }

  /** Current playhead in source seconds. */
  position(): number {
    if (!this.playing) return this.pausedAt;
    const elapsed = (this.ctx.currentTime - this.startedAtCtx) * this.rate;
    return Math.min(Math.max(this.pausedAt + elapsed, 0), this.duration);
  }

  /** (Re)build the per-stem stretch nodes from the current buffers. */
  private async ensurePrepared(): Promise<void> {
    if (this.prepared) return;
    if (this.preparing) return this.preparing;

    this.preparing = (async () => {
      const old = [...this.stretch.values()];
      this.stretch.clear();
      for (const s of old) {
        try {
          s.stop();
        } catch {
          /* not started */
        }
        s.disconnect();
      }

      for (const part of PARTS) {
        const buffer = this.buffers.get(part);
        if (!buffer) continue;
        const node = await createStretch(this.ctx);
        node.addBuffers([buffer.getChannelData(0).slice()]);
        node.connect(this.partGain.get(part)!);
        this.stretch.set(part, node);
      }
      this.prepared = true;
      this.preparing = null;
    })();
    return this.preparing;
  }

  async play(): Promise<void> {
    if (this.playing) return;
    if (this.ctx.state === "suspended") await this.ctx.resume();

    const offset = this.pausedAt >= this.duration ? 0 : this.pausedAt;
    this.pausedAt = offset;
    const when = this.ctx.currentTime + 0.03;

    if (this.stretchEnabled) {
      await this.ensurePrepared();
      if (this.stretch.size === 0) return;
      for (const node of this.stretch.values()) {
        node.start(when, offset, undefined, this.tempo, 0);
      }
    } else {
      this.sources.clear();
      for (const part of PARTS) {
        const buffer = this.buffers.get(part);
        if (!buffer) continue;
        const src = this.ctx.createBufferSource();
        src.buffer = buffer;
        src.connect(this.partGain.get(part)!);
        src.start(when, offset);
        this.sources.set(part, src);
      }
      if (this.sources.size === 0) return;
    }

    this.startedAtCtx = when;
    this.playing = true;
    this.tick();
  }

  pause(): void {
    if (!this.playing) return;
    this.pausedAt = this.position();
    this.stopActive();
    this.playing = false;
    cancelAnimationFrame(this.rafId);
  }

  seek(seconds: number): void {
    const clamped = Math.max(0, Math.min(seconds, this.duration));
    if (this.playing) {
      this.stopActive();
      this.pausedAt = clamped;
      this.playing = false;
      void this.play();
    } else {
      this.pausedAt = clamped;
      this.onPosition?.(clamped, this.duration);
    }
  }

  private stopActive(): void {
    const now = this.ctx.currentTime;
    for (const node of this.stretch.values()) {
      try {
        node.stop(now);
      } catch {
        /* already stopped */
      }
    }
    for (const src of this.sources.values()) {
      try {
        src.stop();
      } catch {
        /* already stopped */
      }
      src.disconnect();
    }
    this.sources.clear();
  }

  private tick = (): void => {
    if (!this.playing) return;
    const pos = this.position();
    this.onPosition?.(pos, this.duration);
    if (pos >= this.duration) {
      this.pause();
      this.pausedAt = 0;
      this.onEnded?.();
      return;
    }
    this.rafId = requestAnimationFrame(this.tick);
  };

  async close(): Promise<void> {
    this.pause();
    await this.ctx.close();
  }
}
