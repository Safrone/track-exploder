<script lang="ts">
  import { PARTS, type Part } from "../types";
  import { PRESETS, applyPreset } from "../mixer/presets";
  import {
    userPresets,
    saveCurrentAsPreset,
    applyUserPreset,
    deleteUserPreset,
  } from "../mixer/userPresets";

  let focus = $state<Part>("lead");
  let presetName = $state("");

  const cap = (s: string) => (s.length ? s[0].toUpperCase() + s.slice(1) : s);

  function save() {
    const name = presetName.trim();
    if (!name) return;
    saveCurrentAsPreset(name);
    presetName = "";
  }
</script>

<div class="presets">
  <label class="focus">
    Part
    <select bind:value={focus}>
      {#each PARTS as p (p)}
        <option value={p}>{p}</option>
      {/each}
    </select>
  </label>

  <div class="buttons">
    {#each PRESETS as preset (preset.id)}
      <button title={preset.description} onclick={() => applyPreset(preset.id, focus)}>
        {preset.label(cap(focus))}
      </button>
    {/each}
  </div>
</div>

<div class="userpresets">
  <div class="saverow">
    <input
      type="text"
      placeholder="Name this mix…"
      bind:value={presetName}
      onkeydown={(e) => e.key === "Enter" && save()}
    />
    <button class="save" onclick={save} disabled={!presetName.trim()}>Save current mix</button>
  </div>

  {#if $userPresets.length > 0}
    <div class="buttons">
      {#each $userPresets as p (p.id)}
        <span class="chip">
          <button class="apply" title="Apply this saved mix" onclick={() => applyUserPreset(p.id)}>
            {p.name}
          </button>
          <button class="del" title="Delete preset" onclick={() => deleteUserPreset(p.id)}
            >×</button
          >
        </span>
      {/each}
    </div>
  {/if}
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
  .userpresets {
    margin-top: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.6rem;
    border-top: 1px solid var(--border);
    padding-top: 0.75rem;
  }
  .saverow {
    display: flex;
    gap: 0.4rem;
    align-items: center;
  }
  .saverow input {
    flex: 0 1 220px;
    min-width: 0;
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.35rem 0.55rem;
    font-size: 0.85rem;
  }
  .save {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.35rem 0.8rem;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .save:disabled {
    opacity: 0.5;
    cursor: default;
  }
  .chip {
    display: inline-flex;
    align-items: stretch;
    border: 1px solid var(--border);
    border-radius: 999px;
    overflow: hidden;
  }
  .chip .apply {
    background: var(--panel-2);
    color: var(--text);
    border: none;
    padding: 0.35rem 0.7rem;
    cursor: pointer;
    font-size: 0.8rem;
  }
  .chip .apply:hover {
    color: var(--accent);
  }
  .chip .del {
    background: var(--panel-2);
    color: var(--text-dim);
    border: none;
    border-left: 1px solid var(--border);
    padding: 0 0.5rem;
    cursor: pointer;
    font-size: 0.95rem;
  }
  .chip .del:hover {
    color: #fca5a5;
  }
</style>
