/**
 * Load a synthetic four-part barbershop set straight into the engine, for
 * testing the app without picking files. Debug-build only (see `debug_build`).
 *
 * The stems are generated as already-extracted mono voices (as if the isolated
 * channel had been pulled from four part-left files), so this exercises the
 * mixer, waveform, transport, tempo and export — not the decode/extract path.
 */
import { PARTS, type Part } from "../types";
import { setTrack } from "../mixer/store";
import type { MixEngine } from "./engine";

/** A short I–IV–V–I-ish progression in B♭, one note per voice per chord (Hz). */
const PROGRESSION: Record<Part, number>[] = [
  { tenor: 293.66, lead: 233.08, baritone: 174.61, bass: 116.54 }, // Bb
  { tenor: 392.0, lead: 311.13, baritone: 233.08, bass: 155.56 }, // Eb
  { tenor: 311.13, lead: 261.63, baritone: 220.0, bass: 174.61 }, // F7
  { tenor: 293.66, lead: 233.08, baritone: 174.61, bass: 116.54 }, // Bb
];
const SECONDS_PER_CHORD = 2;

/** Render one voice's line over the progression into a mono buffer. */
function renderVoice(part: Part, sampleRate: number): Float32Array<ArrayBuffer> {
  const chordFrames = Math.round(SECONDS_PER_CHORD * sampleRate);
  const data = new Float32Array(PROGRESSION.length * chordFrames);
  const attack = 0.02 * sampleRate;
  const release = 0.18 * sampleRate;

  PROGRESSION.forEach((chord, c) => {
    const hz = chord[part];
    const base = c * chordFrames;
    for (let i = 0; i < chordFrames; i++) {
      const t = i / sampleRate;
      const env =
        i < attack ? i / attack : i > chordFrames - release ? (chordFrames - i) / release : 1;
      // Sine plus a couple of harmonics, so it reads as a voice, not a beep.
      const s =
        Math.sin(2 * Math.PI * hz * t) +
        0.4 * Math.sin(2 * Math.PI * 2 * hz * t) +
        0.2 * Math.sin(2 * Math.PI * 3 * hz * t);
      data[base + i] = 0.16 * env * (s / 1.6);
    }
  });
  return data;
}

/**
 * Generate the four sample voices and load them as the current set. Returns the
 * parts loaded (always all four).
 */
export function loadSampleTracks(engine: MixEngine): Part[] {
  const sampleRate = engine.ctx.sampleRate;
  for (const part of PARTS) {
    const data = renderVoice(part, sampleRate);
    const buffer = engine.ctx.createBuffer(1, data.length, sampleRate);
    buffer.copyToChannel(data, 0);
    engine.setBuffer(part, buffer);
  }
  // Update the store after the buffers exist, so the waveform sees them.
  for (const part of PARTS) {
    const cap = part[0].toUpperCase() + part.slice(1);
    setTrack(part, {
      part,
      path: `sample://${part}`,
      name: `Sample — ${cap}`,
      channel: "left",
    });
  }
  return [...PARTS];
}
