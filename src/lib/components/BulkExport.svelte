<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { PARTS, type BitDepth, type ExportFormat } from "../types";
  import { mixer, snapshot } from "../mixer/store";
  import { suggestBaseNameFor } from "../mixer/naming";
  import { getEngine } from "../audio/playback";
  import { groupFiles, bulkExport, type SongGroup, type BulkStage } from "../audio/bulk";
  import ProgressBar from "./ProgressBar.svelte";

  interface Props {
    format: ExportFormat;
    bitDepth: BitDepth;
  }
  let { format, bitDepth }: Props = $props();

  let show = $state(false);
  let stage = $state<"confirm" | "running" | "done">("confirm");
  let groups = $state<SongGroup[]>([]);
  let ungrouped = $state<string[]>([]);
  let outputDir = $state<string | null>(null);
  let prog = $state<{ index: number; total: number; name: string; stage: BulkStage } | null>(null);
  let result = $state<{ exported: number; failed: number } | null>(null);

  const completeCount = $derived(groups.filter((g) => g.missing.length === 0).length);
  const cap = (s: string) => (s.length ? s[0].toUpperCase() + s.slice(1) : s);

  function outName(g: SongGroup): string {
    return `${suggestBaseNameFor(g.name, $mixer)}.${format}`;
  }

  async function pick() {
    const selected = await open({
      multiple: true,
      title: "Select the part files for one or more songs",
      filters: [{ name: "Audio", extensions: ["mp3", "wav", "flac", "m4a", "aac", "ogg", "opus"] }],
    });
    if (!selected) return;
    const paths = Array.isArray(selected) ? selected : [selected];
    const g = groupFiles(paths);
    groups = g.groups;
    ungrouped = g.ungrouped;
    result = null;
    stage = "confirm";
    show = true;
  }

  async function chooseFolder() {
    const d = await open({ directory: true, title: "Choose output folder" });
    if (d) outputDir = Array.isArray(d) ? d[0] : d;
  }

  async function run() {
    if (!outputDir) return;
    stage = "running";
    prog = null;
    result = await bulkExport(
      groups,
      { ctx: getEngine().ctx, state: snapshot(), format, bitDepth, outputDir },
      (info) => {
        prog = { index: info.index, total: info.total, name: info.group.name, stage: info.stage };
      },
    );
    stage = "done";
  }

  function close() {
    show = false;
  }
</script>

<button class="bulk-btn" onclick={pick}>Bulk export…</button>

{#if show}
  <div class="overlay" role="dialog" aria-modal="true">
    <div class="modal">
      <header>
        <h3>Bulk export</h3>
        <button class="x" onclick={close} aria-label="Close">×</button>
      </header>

      {#if stage === "confirm"}
        <p class="intro">
          Each detected song will be exported with the <strong>current mix settings</strong>
          ({format.toUpperCase()}). Review the grouping below.
        </p>

        <div class="tablewrap">
          <table>
            <thead>
              <tr>
                <th>Song</th>
                <th>Parts</th>
                <th>Output file</th>
              </tr>
            </thead>
            <tbody>
              {#each groups as g (g.key)}
                <tr class:incomplete={g.missing.length > 0}>
                  <td class="song">{g.name}</td>
                  <td class="parts">
                    {#each PARTS as p (p)}
                      <span class="pchip" class:have={!!g.parts[p]} title={cap(p)}>
                        {cap(p)[0]}
                      </span>
                    {/each}
                  </td>
                  <td class="out">
                    {#if g.missing.length > 0}
                      <span class="skip">skipped — missing {g.missing.map(cap).join(", ")}</span>
                    {:else}
                      {outName(g)}
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </div>

        {#if ungrouped.length > 0}
          <p class="note">
            {ungrouped.length} file(s) had no recognizable part name and were ignored.
          </p>
        {/if}

        <div class="folder">
          <button class="secondary" onclick={chooseFolder}>Choose output folder…</button>
          {#if outputDir}<span class="path" title={outputDir}>{outputDir}</span>{/if}
        </div>

        <footer>
          <button class="secondary" onclick={close}>Cancel</button>
          <button class="go" disabled={!outputDir || completeCount === 0} onclick={run}>
            Export {completeCount} song{completeCount === 1 ? "" : "s"}
          </button>
        </footer>
      {:else if stage === "running"}
        <div class="running">
          <p>
            Exporting {prog ? prog.index + 1 : 0} / {prog?.total ?? completeCount}
            {#if prog}— <strong>{prog.name}</strong> ({prog.stage}…){/if}
          </p>
          <ProgressBar value={prog && prog.total ? prog.index / prog.total : 0} />
        </div>
      {:else}
        <div class="done">
          <p>Exported {result?.exported ?? 0} song(s){result?.failed ? `, ${result.failed} failed` : ""}.</p>
          <footer>
            <button class="go" onclick={close}>Done</button>
          </footer>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .bulk-btn {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.45rem 0.9rem;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .bulk-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    z-index: 50;
  }
  .modal {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    width: min(760px, 100%);
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    padding: 1rem 1.1rem;
    gap: 0.75rem;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h3 {
    margin: 0;
  }
  .x {
    background: none;
    border: none;
    color: var(--text-dim);
    font-size: 1.4rem;
    cursor: pointer;
    line-height: 1;
  }
  .intro,
  .note {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text-dim);
  }
  .tablewrap {
    overflow: auto;
    border: 1px solid var(--border);
    border-radius: 8px;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.82rem;
  }
  th {
    text-align: left;
    color: var(--text-dim);
    font-weight: 500;
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--border);
    position: sticky;
    top: 0;
    background: var(--panel);
  }
  td {
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid var(--border);
    vertical-align: middle;
  }
  tr.incomplete {
    opacity: 0.6;
  }
  .song {
    white-space: nowrap;
  }
  .parts {
    display: flex;
    gap: 0.2rem;
  }
  .pchip {
    display: inline-flex;
    width: 18px;
    height: 18px;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    font-size: 0.68rem;
    font-weight: 700;
    background: #4b1d1d;
    color: #fca5a5;
  }
  .pchip.have {
    background: #14432f;
    color: var(--accent);
  }
  .out {
    color: var(--text);
    word-break: break-word;
  }
  .skip {
    color: #fca5a5;
    font-size: 0.78rem;
  }
  .folder {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .path {
    font-size: 0.78rem;
    color: var(--text-dim);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 0.6rem;
    margin-top: 0.25rem;
  }
  .secondary {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.45rem 0.9rem;
    cursor: pointer;
    font-size: 0.85rem;
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
    opacity: 0.5;
    cursor: default;
  }
  .running p,
  .done p {
    font-size: 0.9rem;
  }
</style>
