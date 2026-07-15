<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import type { BitDepth, ExportFormat } from "../types";
  import { snapshot } from "../mixer/store";
  import { getEngine } from "../audio/playback";
  import { renderMix } from "../audio/export";
  import { invokeExportMix } from "../audio/tauri";

  let format = $state<ExportFormat>("wav");
  let bitDepth = $state<BitDepth>(24);
  let busy = $state(false);
  let message = $state("");

  const EXT: Record<ExportFormat, string> = { wav: "wav", flac: "flac", mp3: "mp3" };

  async function doExport() {
    const state = snapshot();
    busy = true;
    message = "Rendering…";
    try {
      const path = await save({
        title: "Export mix",
        defaultPath: `mix.${EXT[format]}`,
        filters: [{ name: format.toUpperCase(), extensions: [EXT[format]] }],
      });
      if (!path) {
        busy = false;
        message = "";
        return;
      }

      const rendered = await renderMix(getEngine(), state, state.output);
      message = "Encoding…";
      await invokeExportMix(rendered.pcm, {
        path,
        format,
        channels: rendered.channels,
        sampleRate: rendered.sampleRate,
        bitDepth,
      });
      message = "Exported ✓";
    } catch (err) {
      message = `Export failed: ${err}`;
    } finally {
      busy = false;
    }
  }
</script>

<div class="export">
  <label>
    Format
    <select bind:value={format}>
      <option value="wav">WAV</option>
      <option value="flac">FLAC</option>
      <option value="mp3">MP3</option>
    </select>
  </label>

  <label class:disabled={format === "mp3"}>
    Depth
    <select bind:value={bitDepth} disabled={format === "mp3"}>
      <option value={16}>16-bit</option>
      <option value={24}>24-bit</option>
    </select>
  </label>

  <button class="go" onclick={doExport} disabled={busy}>
    {busy ? "Working…" : "Export mix"}
  </button>

  {#if message}
    <span class="msg">{message}</span>
  {/if}
</div>

<style>
  .export {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex-wrap: wrap;
  }
  label {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    font-size: 0.8rem;
    color: var(--text-dim);
  }
  label.disabled {
    opacity: 0.5;
  }
  select {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.2rem 0.35rem;
  }
  .go {
    background: var(--accent);
    color: #05221a;
    border: none;
    border-radius: 8px;
    padding: 0.45rem 1rem;
    font-weight: 600;
    cursor: pointer;
  }
  .go:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .msg {
    font-size: 0.8rem;
    color: var(--text-dim);
  }
</style>
