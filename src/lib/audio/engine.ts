import { PARTS, type Part } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";
import { createStretch, type StretchNode } from "./stretch";

/**
 * Live preview engine built on the Web Audio API with pitch-preserving tempo.
 *
 * Each stem is played by a Signalsmith Stretch node (scheduled mode) so tempo
 * changes preserve pitch. Graph per part:
 *   StretchNode → GainNode → StereoPannerNode → mixBus → masterGain → destination
 * Gain/pan/mute stay live because they sit after the per-stem stretcher.
 *
 * The playhead advances at `tempo` source-seconds per real second (rate = tempo,
 * where rate < 1 is slower/longer).
 */
export class MixEngine {
  readonly ctx: AudioContext;
  private readonly masterGain: GainNode;
  private readonly mixBus: GainNode;
  private readonly partGain = new Map<Part, GainNode>();
  private readonly partPan = new Map<Part, StereoPannerNode>();
  private readonly buffers = new Map<Part, AudioBuffer>();
  private readonly stretch = new Map<Part, StretchNode>();
  private prepared = false;
  private preparing: Promise<void> | null = null;

  playing = false;
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
    // Stretchers must be rebuilt with the new sample data.
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

  /** Apply gain/pan/master/tempo from mixer state. Safe to call any time. */
  applyMix(state: MixerState): void {
    const now = this.ctx.currentTime;
    for (const part of PARTS) {
      this.partGain.get(part)!.gain.setTargetAtTime(effectiveGain(state, part), now, 0.01);
      this.partPan.get(part)!.pan.setTargetAtTime(state.mix[part].pan, now, 0.01);
    }
    this.masterGain.gain.setTargetAtTime(state.masterGain, now, 0.01);
    this.setTempo(state.tempo);
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
        // Copy so the stem AudioBuffer keeps its data (used for waveform/export).
        node.addBuffers([buffer.getChannelData(0).slice()]);
        node.connect(this.partGain.get(part)!);
        this.stretch.set(part, node);
      }
      this.prepared = true;
      this.preparing = null;
    })();
    return this.preparing;
  }

  /** Change tempo, keeping the playhead continuous. rate<1 = slower. */
  setTempo(tempo: number): void {
    if (tempo === this.tempo) return;
    if (this.playing) {
      this.pausedAt = this.position();
      this.startedAtCtx = this.ctx.currentTime;
    }
    this.tempo = tempo;
    if (this.playing) {
      for (const s of this.stretch.values()) s.schedule({ rate: tempo });
    }
  }

  /** Current playhead in source seconds. */
  position(): number {
    if (!this.playing) return this.pausedAt;
    const elapsed = (this.ctx.currentTime - this.startedAtCtx) * this.tempo;
    return Math.min(Math.max(this.pausedAt + elapsed, 0), this.duration);
  }

  async play(): Promise<void> {
    if (this.playing) return;
    if (this.ctx.state === "suspended") await this.ctx.resume();
    await this.ensurePrepared();
    if (this.stretch.size === 0) return;

    const offset = this.pausedAt >= this.duration ? 0 : this.pausedAt;
    this.pausedAt = offset;
    const when = this.ctx.currentTime + 0.03;

    for (const node of this.stretch.values()) {
      node.start(when, offset, undefined, this.tempo, 0);
    }
    this.startedAtCtx = when;
    this.playing = true;
    this.tick();
  }

  pause(): void {
    if (!this.playing) return;
    this.pausedAt = this.position();
    this.stopNodes();
    this.playing = false;
    cancelAnimationFrame(this.rafId);
  }

  seek(seconds: number): void {
    const clamped = Math.max(0, Math.min(seconds, this.duration));
    if (this.playing) {
      this.stopNodes();
      this.pausedAt = clamped;
      this.playing = false;
      void this.play();
    } else {
      this.pausedAt = clamped;
      this.onPosition?.(clamped, this.duration);
    }
  }

  private stopNodes(): void {
    const now = this.ctx.currentTime;
    for (const node of this.stretch.values()) {
      try {
        node.stop(now);
      } catch {
        /* already stopped */
      }
    }
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
