import { PARTS, type Part } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";
import { stretchStem } from "./stretchNative";

/**
 * Live preview engine (Web Audio).
 *
 * Graph per part: AudioBufferSourceNode → GainNode → StereoPannerNode → mixBus
 * → masterGain → destination. Gain/pan/mute stay live (they sit after the source).
 *
 * Pitch-preserving tempo is done by pre-stretching each stem in Rust (Signalsmith)
 * and playing the stretched buffer at natural rate — the JS AudioWorklet build is
 * unreliable in the webview, so we don't use it. The playhead is tracked in
 * *source* seconds; when stretched, the source position advances at `tempo` per
 * real second and the stretched-buffer offset is `sourceOffset / tempo`.
 */
export class MixEngine {
  readonly ctx: AudioContext;
  private readonly masterGain: GainNode;
  private readonly mixBus: GainNode;
  private readonly partGain = new Map<Part, GainNode>();
  private readonly partPan = new Map<Part, StereoPannerNode>();
  private readonly buffers = new Map<Part, AudioBuffer>();

  /** Buffers actually played (stretched, or the originals when at natural speed). */
  private playbackBuffers = new Map<Part, AudioBuffer>();
  /** Tempo the playbackBuffers were built for (null = not built). */
  private preparedTempo: number | null = null;
  private preparing: Promise<void> | null = null;

  private readonly sources = new Map<Part, AudioBufferSourceNode>();

  playing = false;
  private stretchEnabled = false;
  private tempo = 1;
  private startedAtCtx = 0;
  /** Playhead in *source* seconds when last paused/seeked. */
  private pausedAt = 0;
  private rafId = 0;

  onPosition?: (seconds: number, duration: number) => void;
  onEnded?: () => void;
  onStretching?: (active: boolean) => void;

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
    this.preparedTempo = null; // playback buffers are now stale
  }

  hasBuffer(part: Part): boolean {
    return this.buffers.has(part);
  }

  getBuffer(part: Part): AudioBuffer | undefined {
    return this.buffers.get(part);
  }

  /** Longest loaded stem, in source seconds. */
  get duration(): number {
    let max = 0;
    for (const b of this.buffers.values()) max = Math.max(max, b.duration);
    return max;
  }

  /** Effective playback rate (source-seconds per real second). */
  private get rate(): number {
    return this.stretchEnabled ? this.tempo : 1;
  }

  private get needsStretch(): boolean {
    return this.stretchEnabled && Math.abs(this.tempo - 1) > 1e-4;
  }

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

  private restartIfPlaying(mutate: () => void): void {
    const wasPlaying = this.playing;
    if (wasPlaying) {
      this.pausedAt = this.position();
      this.stopActive();
      this.playing = false;
      cancelAnimationFrame(this.rafId);
    }
    mutate();
    if (wasPlaying) void this.play();
  }

  private setStretchEnabled(enabled: boolean): void {
    if (enabled === this.stretchEnabled) return;
    this.restartIfPlaying(() => {
      this.stretchEnabled = enabled;
    });
  }

  setTempo(tempo: number): void {
    if (tempo === this.tempo) return;
    this.restartIfPlaying(() => {
      this.tempo = tempo;
      this.preparedTempo = null; // stretched buffers stale
    });
  }

  position(): number {
    if (!this.playing) return this.pausedAt;
    const elapsed = (this.ctx.currentTime - this.startedAtCtx) * this.rate;
    return Math.min(Math.max(this.pausedAt + elapsed, 0), this.duration);
  }

  /** Ensure playbackBuffers match the current tempo (stretch in Rust if needed). */
  private async ensurePlayback(): Promise<void> {
    if (!this.needsStretch) {
      this.playbackBuffers = new Map(this.buffers);
      this.preparedTempo = 1;
      return;
    }
    if (this.preparedTempo === this.tempo && this.playbackBuffers.size === this.buffers.size) {
      return;
    }
    if (this.preparing) await this.preparing;
    if (this.preparedTempo === this.tempo) return;

    const target = this.tempo;
    this.onStretching?.(true);
    this.preparing = (async () => {
      const next = new Map<Part, AudioBuffer>();
      for (const [part, buf] of this.buffers) {
        next.set(part, await stretchStem(this.ctx, buf.getChannelData(0), buf.sampleRate, target));
      }
      this.playbackBuffers = next;
      this.preparedTempo = target;
    })();
    try {
      await this.preparing;
    } finally {
      this.preparing = null;
      this.onStretching?.(false);
    }
  }

  async play(): Promise<void> {
    if (this.playing) return;
    if (this.ctx.state === "suspended") await this.ctx.resume();
    await this.ensurePlayback();

    const rate = this.rate;
    const offset = this.pausedAt >= this.duration ? 0 : this.pausedAt;
    this.pausedAt = offset;
    const when = this.ctx.currentTime + 0.03;

    this.sources.clear();
    for (const part of PARTS) {
      const buffer = this.playbackBuffers.get(part);
      if (!buffer) continue;
      const src = this.ctx.createBufferSource();
      src.buffer = buffer;
      src.connect(this.partGain.get(part)!);
      // Stretched buffer runs at natural rate; map source offset into its timeline.
      src.start(when, offset / rate);
      this.sources.set(part, src);
    }
    if (this.sources.size === 0) return;

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
