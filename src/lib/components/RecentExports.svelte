<script lang="ts">
  import { openPath, revealItemInDir } from "@tauri-apps/plugin-opener";
  import { exportsList, clearExports } from "../mixer/exports";
  import { isDesktop } from "../platform";

  const desktop = isDesktop();
  let err = $state("");
  const recent = $derived([...$exportsList].reverse());

  async function openFile(path: string) {
    try {
      await openPath(path);
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
          {#if desktop}
            <td><button class="link" onclick={() => openFile(rec.path)}>Open</button></td>
            <td><button class="link" onclick={() => openFolder(rec.path)}>Open folder</button></td>
          {:else}
            <td colspan="2"></td>
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
