<script lang="ts">
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { exportsList, clearExports } from "../mixer/exports";
  import { invokeOpenUri } from "../audio/tauri";
  import { isDesktop } from "../platform";

  const desktop = isDesktop();
  let err = $state("");
  const recent = $derived([...$exportsList].reverse());

  // The SAF content provider often reports exports as application/octet-stream,
  // which matches no player; tell the view intent the real type from the format.
  const MIME: Record<string, string> = {
    wav: "audio/wav",
    flac: "audio/flac",
    mp3: "audio/mpeg",
  };

  async function openFile(rec: { path: string; format: string }) {
    try {
      // The opener plugin can't open a content:// URI on Android, so hand it to
      // a native view intent there; desktop uses the plugin.
      if (desktop) await openPath(rec.path);
      else await invokeOpenUri(rec.path, MIME[rec.format] ?? "audio/*");
    } catch (e) {
      err = `Could not open: ${e}`;
    }
  }

  async function openFolder(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      err = `Could not reveal: ${e}`;
    }
  }
</script>

{#if recent.length > 0}
  <table class="exports">
    <thead>
      <tr>
        <th>Exported file</th>
        <th></th>
        <th class="clearcol"><button class="clear" onclick={clearExports}>Clear list</button></th>
      </tr>
    </thead>
    <tbody>
      {#each recent as rec (rec.path + rec.at)}
        <tr>
          <td class="fname" title={rec.path}>{rec.name}</td>
          <td><button class="link" onclick={() => openFile(rec)}>Open</button></td>
          {#if desktop}
            <td><button class="link" onclick={() => openFolder(rec.path)}>Open folder</button></td>
          {:else}
            <td></td>
          {/if}
        </tr>
      {/each}
    </tbody>
  </table>
  {#if err}<div class="err">{err}</div>{/if}
{/if}

<style>
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
  .clearcol {
    text-align: right;
    white-space: nowrap;
  }
  .clear {
    background: none;
    border: none;
    color: var(--text-dim);
    cursor: pointer;
    font-size: 0.75rem;
    font-weight: 400;
    padding: 0;
  }
  .clear:hover {
    color: #fca5a5;
    text-decoration: underline;
  }
  .err {
    font-size: 0.78rem;
    color: #fca5a5;
    margin-top: 0.3rem;
  }
</style>
