import { PARTS, type Part } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";

/**
 * Live preview engine built on the Web Audio API.
 *
 * Graph (per part): AudioBufferSourceNode → GainNode → StereoPannerNode → mixBus
 * → masterGain → destination. Sources are recreated on each `play()` because
 * `AudioBufferSourceNode`s are single-use.
 *
 * Tempo: for now the preview changes tempo via `playbackRate` (which also shifts
 * pitch). Pitch-preserving tempo (Signalsmith Stretch AudioWorklet on the mix
 * bus) is the planned upgrade — see `stretch.ts`. The playhead math is identical
 * either way because the source advances at `tempo` source-seconds per real
 * second in both models.
 */
export class MixEngine {
  readonly ctx: AudioContext;
  private readonly masterGain: GainNode;
  private readonly mixBus: GainNode;
  private readonly partGain = new Map<Part, GainNode>();
  private readonly partPan = new Map<Part, StereoPannerNode>();
  private readonly buffers = new Map<Part, AudioBuffer>();
  private sources = new Map<Part, AudioBufferSourceNode>();

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
      const g = this.partGain.get(part)!;
      const p = this.partPan.get(part)!;
      g.gain.setTargetAtTime(effectiveGain(state, part), now, 0.01);
      p.pan.setTargetAtTime(state.mix[part].pan, now, 0.01);
    }
    this.masterGain.gain.setTargetAtTime(state.masterGain, now, 0.01);
    this.setTempo(state.tempo);
  }

  /** Change tempo, keeping the playhead continuous. */
  setTempo(tempo: number): void {
    if (tempo === this.tempo) return;
    if (this.playing) {
      // Rebase time so the playhead doesn't jump.
      this.pausedAt = this.position();
      this.startedAtCtx = this.ctx.currentTime;
    }
    this.tempo = tempo;
    for (const src of this.sources.values()) {
      src.playbackRate.value = tempo;
    }
  }

  /** Current playhead in source seconds. */
  position(): number {
    if (!this.playing) return this.pausedAt;
    const elapsed = (this.ctx.currentTime - this.startedAtCtx) * this.tempo;
    return Math.min(this.pausedAt + elapsed, this.duration);
  }

  async play(): Promise<void> {
    if (this.playing) return;
    if (this.ctx.state === "suspended") await this.ctx.resume();

    const offset = this.pausedAt >= this.duration ? 0 : this.pausedAt;
    this.pausedAt = offset;
    this.sources = new Map();
    for (const part of PARTS) {
      const buffer = this.buffers.get(part);
      if (!buffer) continue;
      const src = this.ctx.createBufferSource();
      src.buffer = buffer;
      src.playbackRate.value = this.tempo;
      src.connect(this.partGain.get(part)!);
      src.start(0, offset);
      this.sources.set(part, src);
    }
    this.startedAtCtx = this.ctx.currentTime;
    this.playing = true;
    this.tick();
  }

  pause(): void {
    if (!this.playing) return;
    this.pausedAt = this.position();
    this.stopSources();
    this.playing = false;
    cancelAnimationFrame(this.rafId);
  }

  seek(seconds: number): void {
    const clamped = Math.max(0, Math.min(seconds, this.duration));
    if (this.playing) {
      this.stopSources();
      this.pausedAt = clamped;
      this.playing = false;
      void this.play();
    } else {
      this.pausedAt = clamped;
      this.onPosition?.(clamped, this.duration);
    }
  }

  private stopSources(): void {
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
