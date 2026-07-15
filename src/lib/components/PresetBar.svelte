<script lang="ts">
  import { PARTS, type Part } from "../types";
  import { PRESETS, applyPreset } from "../mixer/presets";

  let focus = $state<Part>("lead");
</script>

<div class="presets">
  <label class="focus">
    Focus part
    <select bind:value={focus}>
      {#each PARTS as p (p)}
        <option value={p}>{p}</option>
      {/each}
    </select>
  </label>

  <div class="buttons">
    {#each PRESETS as preset (preset.id)}
      <button title={preset.description} onclick={() => applyPreset(preset.id, focus)}>
        {preset.label}
      </button>
    {/each}
  </div>
</div>

<style>
  .presets {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  .focus {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.8rem;
    color: var(--text-dim);
    text-transform: capitalize;
  }
  .focus select {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.2rem 0.35rem;
    text-transform: capitalize;
  }
  .buttons {
    display: flex;
    gap: 0.4rem;
    flex-wrap: wrap;
  }
  .buttons button {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.35rem 0.8rem;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .buttons button:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
