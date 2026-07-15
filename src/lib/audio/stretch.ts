import SignalsmithStretch, { type SignalsmithStretchNode } from "signalsmith-stretch";

export type StretchNode = SignalsmithStretchNode;

/**
 * Create a Signalsmith Stretch node (pitch-preserving time-stretch) bound to a
 * context. The WASM + AudioWorklet are bundled inside the package, so no extra
 * asset files need serving.
 */
export async function createStretch(ctx: BaseAudioContext): Promise<StretchNode> {
  return SignalsmithStretch(ctx);
}
