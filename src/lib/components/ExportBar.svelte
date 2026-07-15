<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import type { BitDepth, ExportFormat, OutputMode } from "../types";
  import { mixer, snapshot, patchState } from "../mixer/store";
  import { suggestBaseName } from "../mixer/naming";
  import { partTags, computeCommon } from "../mixer/tags";
  import {
    exportsList,
    addExport,
    getLastExportDir,
    setLastExportDir,
    splitDir,
  } from "../mixer/exports";
  import { getEngine } from "../audio/playback";
  import { renderMix } from "../audio/export";
  import { invokeExportMix } from "../audio/tauri";
  import ProgressBar from "./ProgressBar.svelte";

  let format = $state<ExportFormat>("wav");
  let bitDepth = $state<BitDepth>(24);
  let name = $state("");
  let nameEdited = $state(false);
  let busy = $state(false);
  let progress = $state(0);
  let message = $state("");

  const EXT: Record<ExportFormat, string> = { wav: "wav", flac: "flac", mp3: "mp3" };

  const commonTags = $derived(computeCommon($partTags));
  const tagKeys = $derived(Object.keys(commonTags));

  // Keep the suggested name in sync with sources + mix options until the user
  // types their own; then leave it alone.
  $effect(() => {
    const state = $mixer;
    if (!nameEdited) name = suggestBaseName(state);
  });

  function resetName() {
    nameEdited = false;
    name = suggestBaseName(snapshot());
  }

  function basename(path: string): string {
    return splitDir(path).dir ? path.slice(splitDir(path).dir.length + 1) : path;
  }

  async function doExport() {
    const state = snapshot();
    try {
      const fileName = `${(name.trim() || "mix")}.${EXT[format]}`;
      const lastDir = getLastExportDir();
      let defaultPath = fileName;
      if (lastDir) {
        const sep = lastDir.includes("\\") ? "\\" : "/";
        defaultPath = `${lastDir}${sep}${fileName}`;
      }

      const path = await save({
        title: "Export mix",
        defaultPath,
        filters: [{ name: format.toUpperCase(), extensions: [EXT[format]] }],
      });
      if (!path) {
        message = "";
        return;
      }

      busy = true;
      progress = 0.15;
      message = "Rendering…";
      const rendered = await renderMix(getEngine(), state, state.output);

      progress = 0.6;
      message = "Encoding…";
      await invokeExportMix(rendered.pcm, {
        path,
        format,
        channels: rendered.channels,
        sampleRate: rendered.sampleRate,
        bitDepth,
        tags: commonTags,
      });

      progress = 1;
      const { dir } = splitDir(path);
      if (dir) setLastExportDir(dir);
      addExport({ path, name: basename(path), format, at: Date.now() });
      message = "Exported ✓";
    } catch (err) {
      message = `Export failed: ${err}`;
    } finally {
      busy = false;
      progress = 0;
    }
  }

  async function openFile(path: string) {
    try {
      await openPath(path);
    } catch (err) {
      message = `Could not open: ${err}`;
    }
  }

  async function openFolder(path: string) {
    try {
      await revealItemInDir(path);
    } catch (err) {
      message = `Could not reveal: ${err}`;
    }
  }

  const recent = $derived([...$exportsList].reverse());
</script>

<div class="export">
  <label class="namefield">
    File name
    <div class="namerow">
      <input
        type="text"
        value={name}
        oninput={(e) => {
          nameEdited = true;
          name = e.currentTarget.value;
        }}
        placeholder="mix"
      />
      <span class="ext">.{EXT[format]}</span>
      <button class="reset" title="Regenerate suggested name" onclick={resetName}>↻</button>
    </div>
  </label>

  <div class="opts">
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

    <label>
      Output
      <select
        value={$mixer.output}
        onchange={(e) => patchState({ output: e.currentTarget.value as OutputMode })}
      >
        <option value="stereo">Stereo</option>
        <option value="mono">Mono</option>
      </select>
    </label>

    <button class="go" onclick={doExport} disabled={busy}>
      {busy ? "Working…" : "Export mix"}
    </button>

    {#if message && !busy}
      <span class="msg">{message}</span>
    {/if}
  </div>

  {#if busy}
    <ProgressBar value={progress} label={message} />
  {/if}

  {#if tagKeys.length > 0}
    <div class="tags">Keeping source tags: {tagKeys.join(", ")}</div>
  {/if}

  {#if recent.length > 0}
    <table class="exports">
      <thead>
        <tr>
          <th>Exported file</th>
          <th></th>
          <th></th>
        </tr>
      </thead>
      <tbody>
        {#each recent as rec (rec.path + rec.at)}
          <tr>
            <td class="fname" title={rec.path}>{rec.name}</td>
            <td><button class="link" onclick={() => openFile(rec.path)}>Open</button></td>
            <td>
              <button class="link" onclick={() => openFolder(rec.path)}>Open folder</button>
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .export {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }
  .namefield {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.25rem;
    width: 100%;
    font-size: 0.8rem;
    color: var(--text-dim);
  }
  .namerow {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    width: 100%;
  }
  .namerow input {
    flex: 1 1 auto;
    min-width: 0;
    width: 100%;
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 0.5rem 0.6rem;
    font-size: 0.95rem;
    text-align: left;
  }
  .ext {
    color: var(--text-dim);
    font-variant-numeric: tabular-nums;
  }
  .reset {
    background: var(--panel-2);
    border: 1px solid var(--border);
    color: var(--text-dim);
    border-radius: 6px;
    width: 32px;
    height: 32px;
    cursor: pointer;
    font-size: 1rem;
  }
  .reset:hover {
    color: var(--accent);
    border-color: var(--accent);
  }
  .opts {
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
  .tags {
    font-size: 0.78rem;
    color: var(--text-dim);
  }
  .exports {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.8rem;
  }
  .exports th {
    text-align: left;
    color: var(--text-dim);
    font-weight: 500;
    border-bottom: 1px solid var(--border);
    padding: 0.3rem 0.4rem;
  }
  .exports td {
    padding: 0.3rem 0.4rem;
    border-bottom: 1px solid var(--border);
  }
  .fname {
    max-width: 0;
    width: 100%;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .link {
    background: none;
    border: none;
    color: var(--accent);
    cursor: pointer;
    padding: 0;
    white-space: nowrap;
    font-size: 0.8rem;
  }
  .link:hover {
    text-decoration: underline;
  }
</style>
