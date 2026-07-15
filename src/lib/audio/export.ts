import { PARTS, type OutputMode } from "../types";
import { effectiveGain, type MixerState } from "../mixer/store";
import type { MixEngine } from "./engine";
import { createStretch } from "./stretch";

/**
 * Interleave planar channel data into a single Float32Array.
 * For "mono" output the (already single) channel is returned as-is.
 */
export function interleave(channels: Float32Array[]): Float32Array {
  if (channels.length === 1) return channels[0];
  const frames = channels[0].length;
  const out = new Float32Array(frames * channels.length);
  for (let i = 0; i < frames; i++) {
    for (let c = 0; c < channels.length; c++) {
      out[i * channels.length + c] = channels[c][i];
    }
  }
  return out;
}

export interface RenderedMix {
  pcm: Float32Array;
  channels: number;
  sampleRate: number;
}

/**
 * Render the current mix offline (the exact permutation shown in the UI) into
 * interleaved f32 PCM ready to hand to the native encoder.
 *
 * At 100% tempo the stems play through plain buffer sources (bit-for-bit the
 * mix). When slowed, each stem goes through a Signalsmith Stretch node so the
 * export is pitch-preserved, matching the preview.
 */
export async function renderMix(
  engine: MixEngine,
  state: MixerState,
  mode: OutputMode,
): Promise<RenderedMix> {
  const sampleRate = engine.ctx.sampleRate;
  const tempo = state.tempo;
  const stretched = tempo !== 1;
  const durationSeconds = engine.duration / tempo;
  // Give the stretch algorithm a little tail to flush.
  const tail = stretched ? 0.5 : 0;
  const length = Math.max(1, Math.ceil((durationSeconds + tail) * sampleRate));

  const offline = new OfflineAudioContext(2, length, sampleRate);
  const master = offline.createGain();
  master.gain.value = state.masterGain;
  master.connect(offline.destination);

  for (const part of PARTS) {
    const buffer = engine.getBuffer(part);
    const gainValue = effectiveGain(state, part);
    if (!buffer || gainValue === 0) continue;

    const gain = offline.createGain();
    gain.gain.value = gainValue;
    const pan = offline.createStereoPanner();
    pan.pan.value = state.mix[part].pan;
    gain.connect(pan);
    pan.connect(master);

    if (stretched) {
      const node = await createStretch(offline);
      node.addBuffers([buffer.getChannelData(0).slice()]);
      node.connect(gain);
      node.start(0, 0, undefined, tempo, 0);
    } else {
      const src = offline.createBufferSource();
      src.buffer = buffer;
      src.connect(gain);
      src.start(0);
    }
  }

  const rendered = await offline.startRendering();

  if (mode === "mono") {
    const l = rendered.getChannelData(0);
    const r = rendered.numberOfChannels > 1 ? rendered.getChannelData(1) : l;
    const mono = new Float32Array(l.length);
    for (let i = 0; i < l.length; i++) mono[i] = (l[i] + r[i]) * 0.5;
    return { pcm: mono, channels: 1, sampleRate };
  }

  const channels = [rendered.getChannelData(0), rendered.getChannelData(1)];
  return { pcm: interleave(channels), channels: 2, sampleRate };
}
