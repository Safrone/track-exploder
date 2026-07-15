<script lang="ts">
  import type { Part } from "../types";
  import { mixer, setPartMix } from "../mixer/store";
  import { resetOnDblClick } from "../actions";

  interface Props {
    part: Part;
  }
  let { part }: Props = $props();

  const track = $derived($mixer.tracks[part]);
  const m = $derived($mixer.mix[part]);
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
      use:resetOnDblClick={1}
      oninput={(e) => setPartMix(part, { gain: +e.currentTarget.value })}
    />
  </label>

  <label class="slider">
    <span>Pan {m.pan === 0 ? "C" : m.pan < 0 ? `L${Math.round(-m.pan * 100)}` : `R${Math.round(m.pan * 100)}`}</span>
    <input
      type="range"
      min="-1"
      max="1"
      step="0.02"
      value={m.pan}
      use:resetOnDblClick={0}
      oninput={(e) => setPartMix(part, { pan: +e.currentTarget.value })}
    />
  </label>
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
  .slider input {
    width: 100%;
    accent-color: var(--accent);
  }
</style>
