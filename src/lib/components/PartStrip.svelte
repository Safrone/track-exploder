<script lang="ts">
  import type { Part } from "../types";
  import { mixer, setPartMix, setAlignment } from "../mixer/store";
  import { rangeGesture } from "../actions";
  import { currentEngine } from "../audio/playback";
  import { noAlignment, nudge, shiftMs } from "../audio/align";

  interface Props {
    part: Part;
    onLoadPart: (part: Part) => void;
  }
  let { part, onLoadPart }: Props = $props();

  const track = $derived($mixer.tracks[part]);
  const m = $derived($mixer.mix[part]);
  // Thumb position 0..100 for the pan fill (which grows from the centre out).
  const panPct = $derived(((m.pan + 1) / 2) * 100);

  const alignment = $derived($mixer.alignment[part]);
  const shift = $derived(shiftMs(alignment));
  const timing = $derived(
    shift === 0
      ? "as recorded"
      : `${shift > 0 ? "+" : "−"}${Math.abs(shift).toFixed(shift % 1 ? 1 : 0)} ms`,
  );

  /** Move this part earlier (negative) or later, in milliseconds. */
  function nudgeBy(ms: number) {
    const sampleRate =
      alignment?.sampleRate || currentEngine()?.getBuffer(part)?.sampleRate || 44100;
    setAlignment(part, nudge(alignment ?? noAlignment(sampleRate), ms));
  }
</script>

<div class="strip" class:loaded={!!track} class:muted={!!track && !m.included}>
  <div class="head">
    <span class="name">{part}</span>
    {#if track}
      <span class="file" title={track.name}>{track.name}</span>
    {:else}
      <span class="file muted">not loaded</span>
    {/if}
  </div>

  <div class="row">
    <button class="loadbtn" onclick={() => onLoadPart(part)}>
      {track ? "Replace" : "Load file"}
    </button>
    <button
      class="mute"
      class:active={!m.included}
      aria-pressed={!m.included}
      onclick={() => setPartMix(part, { included: !m.included })}
    >
      {m.included ? "Mute" : "Muted"}
    </button>
  </div>

  <label class="slider">
    <span>Gain {(m.gain * 100).toFixed(0)}%</span>
    <input
      type="range"
      min="0"
      max="2"
      step="0.01"
      value={m.gain}
      use:rangeGesture={{ resetValue: 1 }}
      oninput={(e) => setPartMix(part, { gain: +e.currentTarget.value })}
    />
  </label>

  <label class="slider">
    <span
      >Pan {m.pan === 0
        ? "Center"
        : m.pan < 0
          ? `Left ${Math.round(-m.pan * 100)}%`
          : `Right ${Math.round(m.pan * 100)}%`}</span
    >
    <input
      class="pan"
      type="range"
      min="-1"
      max="1"
      step="0.02"
      value={m.pan}
      style="--lo:{Math.min(50, panPct)}%; --hi:{Math.max(50, panPct)}%"
      use:rangeGesture={{ resetValue: 0 }}
      oninput={(e) => setPartMix(part, { pan: +e.currentTarget.value })}
    />
  </label>

  {#if track}
    <div class="timing" class:shifted={shift !== 0}>
      <span class="lbl" title="Timing relative to the rest of the set">
        Timing {timing}
        {#if alignment && !alignment.consistent && alignment.deltaFrames !== 0}
          <span
            class="warn"
            title="The offset drifts through the song, so one shift can only get close."
            >±{Math.round((alignment.spreadFrames / alignment.sampleRate) * 1000)} ms</span
          >
        {/if}
      </span>
      <div class="nudge">
        <button title="10 ms earlier" onclick={() => nudgeBy(-10)}>−10</button>
        <button title="1 ms earlier" onclick={() => nudgeBy(-1)}>−1</button>
        <button title="1 ms later" onclick={() => nudgeBy(1)}>+1</button>
        <button title="10 ms later" onclick={() => nudgeBy(10)}>+10</button>
        <button
          class="reset"
          disabled={shift === 0}
          title="Play this part exactly as recorded"
          onclick={() => setAlignment(part, undefined)}>⟲</button
        >
      </div>
    </div>
  {/if}
</div>

<style>
  .strip {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    opacity: 0.6;
    /* Prevent a long file name from stretching the grid column. */
    min-width: 0;
  }
  .strip.loaded {
    opacity: 1;
  }
  .strip.muted .name {
    color: var(--text-dim);
  }
  .strip.muted {
    border-color: #5b2626;
  }
  .head {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
  }
  .name {
    text-transform: capitalize;
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .file {
    font-size: 0.72rem;
    color: var(--text-dim);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .file.muted {
    font-style: italic;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .loadbtn {
    padding: 0.25rem 0.6rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--text);
    cursor: pointer;
    font-size: 0.78rem;
  }
  .loadbtn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .mute {
    margin-left: auto;
    padding: 0.25rem 0.7rem;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--text-dim);
    cursor: pointer;
    font-size: 0.8rem;
    font-weight: 600;
  }
  .mute:hover {
    color: var(--text);
  }
  .mute.active {
    background: #7f1d1d;
    border-color: #b91c1c;
    color: #fecaca;
  }
  .slider {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.75rem;
    color: var(--text-dim);
  }
  .timing {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
    font-size: 0.72rem;
    color: var(--text-dim);
    border-top: 1px solid var(--border);
    padding-top: 0.4rem;
  }
  .timing.shifted .lbl {
    color: var(--accent);
  }
  .timing .warn {
    color: #fbbf24;
    margin-left: 0.3rem;
  }
  .nudge {
    display: flex;
    gap: 0.2rem;
  }
  .nudge button {
    flex: 1;
    padding: 0.15rem 0;
    border-radius: 5px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--text-dim);
    cursor: pointer;
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
  }
  .nudge button:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  .nudge button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .nudge .reset {
    flex: 0 0 1.6rem;
  }
  .slider input {
    width: 100%;
    accent-color: var(--accent);
  }
  /* Pan: fill grows from the centre out toward the thumb (not from the left). */
  .slider input.pan {
    -webkit-appearance: none;
    appearance: none;
    height: 6px;
    background: transparent;
    border-radius: 999px;
  }
  .pan::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 999px;
    background: linear-gradient(
      90deg,
      var(--panel-2) 0 var(--lo, 50%),
      var(--accent) var(--lo, 50%) var(--hi, 50%),
      var(--panel-2) var(--hi, 50%) 100%
    );
  }
  .pan::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 13px;
    height: 13px;
    margin-top: -3.5px;
    border-radius: 50%;
    background: var(--text);
    border: 2px solid var(--panel);
    cursor: pointer;
  }

  /* Touch devices: bigger tracks, thumbs, and hit areas for fingers. */
  @media (pointer: coarse) {
    .slider {
      font-size: 0.85rem;
      gap: 0.35rem;
    }
    .slider input {
      height: 30px; /* enlarges the touch target */
    }
    /* Gain (native range): style the thumb so it's finger-sized. */
    .slider input:not(.pan) {
      -webkit-appearance: none;
      appearance: none;
      background: transparent;
    }
    .slider input:not(.pan)::-webkit-slider-runnable-track {
      height: 8px;
      border-radius: 999px;
      background: var(--panel-2);
    }
    .slider input:not(.pan)::-webkit-slider-thumb {
      -webkit-appearance: none;
      appearance: none;
      width: 26px;
      height: 26px;
      margin-top: -9px;
      border-radius: 50%;
      background: var(--accent);
      border: 2px solid var(--panel);
    }
    /* Pan: taller track + bigger thumb. */
    .pan {
      height: 10px;
    }
    .pan::-webkit-slider-runnable-track {
      height: 10px;
    }
    .pan::-webkit-slider-thumb {
      width: 26px;
      height: 26px;
      margin-top: -8px;
    }
    .loadbtn,
    .mute {
      padding: 0.45rem 0.8rem;
      font-size: 0.85rem;
    }
  }
</style>
