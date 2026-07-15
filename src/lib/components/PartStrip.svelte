<script lang="ts">
  import type { Channel, Part } from "../types";
  import { mixer, setPartMix } from "../mixer/store";

  interface Props {
    part: Part;
    onChannelChange: (part: Part, channel: Channel) => void;
  }
  let { part, onChannelChange }: Props = $props();

  const track = $derived($mixer.tracks[part]);
  const m = $derived($mixer.mix[part]);
</script>

<div class="strip" class:loaded={!!track}>
  <div class="head">
    <span class="name">{part}</span>
    {#if track}
      <span class="file" title={track.name}>{track.name}</span>
    {:else}
      <span class="file muted">not loaded</span>
    {/if}
  </div>

  <div class="row">
    <label class="chan">
      Ch
      <select
        value={track?.channel ?? "left"}
        disabled={!track}
        onchange={(e) => onChannelChange(part, e.currentTarget.value as Channel)}
      >
        <option value="left">Left</option>
        <option value="right">Right</option>
      </select>
    </label>

    <label class="toggle">
      <input
        type="checkbox"
        checked={m.included}
        onchange={(e) => setPartMix(part, { included: e.currentTarget.checked })}
      />
      on
    </label>

    <button
      class="chip"
      class:active={m.soloed}
      onclick={() => setPartMix(part, { soloed: !m.soloed })}>S</button
    >
    <button
      class="chip"
      class:active={m.muted}
      onclick={() => setPartMix(part, { muted: !m.muted })}>M</button
    >
  </div>

  <label class="slider">
    <span>Gain {(m.gain * 100).toFixed(0)}%</span>
    <input
      type="range"
      min="0"
      max="2"
      step="0.01"
      value={m.gain}
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
  .chan select {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.1rem 0.2rem;
  }
  .toggle {
    display: flex;
    align-items: center;
    gap: 0.2rem;
    font-size: 0.8rem;
    margin-left: auto;
  }
  .chip {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--text-dim);
    cursor: pointer;
    font-weight: 700;
  }
  .chip.active {
    background: var(--accent);
    color: #05221a;
    border-color: var(--accent);
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
