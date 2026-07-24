import { PARTS, type Alignment, type Part } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";
import { stretchStem } from "./stretchNative";
import { applyAlignment } from "./align";

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
  /** Stems as decoded, before any timing correction. */
  private readonly rawBuffers = new Map<Part, AudioBuffer>();
  /** Stems with the timing correction applied — what everything else reads. */
  private readonly buffers = new Map<Part, AudioBuffer>();
  private readonly alignment = new Map<Part, Alignment>();

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
  /** Progress of tempo pre-stretch: `{done, total}` while active, `null` when idle. */
  onStretching?: (progress: { done: number; total: number } | null) => void;

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
    this.rawBuffers.set(part, buffer);
    this.buffers.set(part, applyAlignment(this.ctx, buffer, this.alignment.get(part)));
    this.preparedTempo = null; // playback buffers are now stale
  }

  /**
   * Apply (or clear, with `undefined`) a part's timing correction. The stem is
   * re-spliced from the decoded original, so nudging never compounds.
   */
  setAlignment(part: Part, alignment: Alignment | undefined): void {
    if (this.updateAlignment(part, alignment) && this.suspendForChange()) {
      void this.play();
    }
  }

  /**
   * Re-splice one stem for a new correction. Returns whether anything changed —
   * the caller decides when to restart playback, because a restart has to happen
   * *after* every stem has been updated (see {@link applyMix}).
   */
  private updateAlignment(part: Part, alignment: Alignment | undefined): boolean {
    const previous = this.alignment.get(part);
    if (
      (previous?.deltaFrames ?? 0) === (alignment?.deltaFrames ?? 0) &&
      (previous?.spliceAt ?? 0) === (alignment?.spliceAt ?? 0)
    ) {
      return false;
    }
    if (alignment) this.alignment.set(part, alignment);
    else this.alignment.delete(part);

    const raw = this.rawBuffers.get(part);
    if (!raw) return false; // applied when the stem finishes decoding

    this.buffers.set(part, applyAlignment(this.ctx, raw, alignment));
    this.preparedTempo = null; // playback buffers are now stale
    return true;
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
    // Re-splice every changed stem *before* restarting playback. Restarting
    // per part would take the buffers as they are at that moment, so the parts
    // corrected later in the loop would carry on playing their old audio until
    // something else happened to restart it.
    let realigned = false;
    for (const part of PARTS) {
      realigned = this.updateAlignment(part, state.alignment[part]) || realigned;
    }
    if (realigned && this.suspendForChange()) void this.play();

    const now = this.ctx.currentTime;
    for (const part of PARTS) {
      this.partGain.get(part)!.gain.setTargetAtTime(effectiveGain(state, part), now, 0.01);
      this.partPan.get(part)!.pan.setTargetAtTime(state.mix[part].pan, now, 0.01);
    }
    this.masterGain.gain.setTargetAtTime(state.masterGain, now, 0.01);
    this.setStretchEnabled(state.tempoEnabled);
    this.setTempo(state.tempo);
  }

  /**
   * Stop the running sources but keep the playhead, so a change can be made and
   * playback picked up where it left off. Returns whether it was playing.
   */
  private suspendForChange(): boolean {
    if (!this.playing) return false;
    this.pausedAt = this.position();
    this.stopActive();
    this.playing = false;
    cancelAnimationFrame(this.rafId);
    return true;
  }

  private restartIfPlaying(mutate: () => void): void {
    const wasPlaying = this.suspendForChange();
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
    const entries = [...this.buffers];
    const total = entries.length;
    this.onStretching?.({ done: 0, total });
    this.preparing = (async () => {
      // Stretch every stem concurrently — each runs on its own Rust thread.
      let done = 0;
      const stretched = await Promise.all(
        entries.map(async ([part, buf]) => {
          const out = await stretchStem(this.ctx, buf.getChannelData(0), buf.sampleRate, target);
          this.onStretching?.({ done: ++done, total });
          return [part, out] as const;
        }),
      );
      this.playbackBuffers = new Map(stretched);
      this.preparedTempo = target;
    })();
    try {
      await this.preparing;
    } finally {
      this.preparing = null;
      this.onStretching?.(null);
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

    // At natural speed the stems are played as they stand, so a correction that
    // lands between here and `ensurePlayback` can't be missed. Only the stretched
    // path uses pre-built buffers — and any change invalidates those (see
    // `updateAlignment`), forcing a re-stretch.
    const playing = this.needsStretch ? this.playbackBuffers : this.buffers;

    this.sources.clear();
    for (const part of PARTS) {
      const buffer = playing.get(part);
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

  /**
   * Stop and send the playhead back to the start — what a freshly loaded set
   * wants, rather than dropping you two minutes into a song you just opened.
   */
  rewind(): void {
    this.pause();
    this.pausedAt = 0;
    this.onPosition?.(0, this.duration);
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
