declare module "signalsmith-stretch" {
  /** The AudioWorkletNode returned by the factory (scheduled/live stretch). */
  export interface SignalsmithStretchNode extends AudioWorkletNode {
    /** Load channel sample data for scheduled playback. Omit `transfer` to copy. */
    addBuffers(channels: Float32Array[], transfer?: ArrayBufferLike[]): void;
    /** Drop loaded buffers up to `toSeconds` (or all). */
    dropBuffers(toSeconds?: number): void;
    /** Begin scheduled playback. rate<1 = slower/longer, semitones shifts pitch. */
    start(
      when?: number,
      offset?: number,
      duration?: number,
      rate?: number,
      semitones?: number,
    ): void;
    stop(when?: number): void;
    /** Adjust playback mapping/params (e.g. live rate change). */
    schedule(
      config: {
        input?: number;
        output?: number;
        rate?: number;
        semitones?: number;
      },
      adjustPrevious?: boolean,
    ): void;
    configure(config: Record<string, unknown>): void;
  }

  /** Create a stretch node bound to the given context (async: loads WASM + worklet). */
  export default function SignalsmithStretch(
    context: BaseAudioContext,
  ): Promise<SignalsmithStretchNode>;
}
