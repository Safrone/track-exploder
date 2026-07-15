import { writable } from "svelte/store";
import { MixEngine } from "./engine";

/** Playhead position in source seconds. */
export const position = writable(0);
/** Longest loaded stem duration, in seconds. */
export const duration = writable(0);
export const isPlaying = writable(false);

let engine: MixEngine | null = null;

/** Get (creating on first use) the shared preview engine. Call from a user gesture. */
export function getEngine(): MixEngine {
  if (!engine) {
    engine = new MixEngine();
    engine.onPosition = (pos, dur) => {
      position.set(pos);
      duration.set(dur);
    };
    engine.onEnded = () => {
      isPlaying.set(false);
      position.set(0);
    };
  }
  return engine;
}

/** The engine if it already exists, else null (does not create an AudioContext). */
export function currentEngine(): MixEngine | null {
  return engine;
}
