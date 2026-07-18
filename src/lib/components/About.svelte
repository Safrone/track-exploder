<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";

  interface Props {
    open: boolean;
    onClose: () => void;
  }
  let { open, onClose }: Props = $props();

  const VERSION = "1.0.0";
  const REPO_URL: string = "https://github.com/Safrone/track-exploder";
  const AUTHOR = "Eric Blum";
  const CONTACT: string = "eblumster@gmail.com"; // empty hides the row
  const KOFI_URL: string = "https://ko-fi.com/safrone"; // empty hides the row

  const YEAR = 2026;

  async function link(url: string) {
    try {
      await openUrl(url);
    } catch {
      /* only works in the desktop app */
    }
  }

  function onKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div class="overlay" role="dialog" aria-modal="true">
    <div class="modal">
      <header>
        <div class="title">
          <img src="/logo.svg" alt="" class="logo" />
          <div>
            <h3>Track Exploder</h3>
            <span class="ver">Version {VERSION}</span>
          </div>
        </div>
        <button class="x" onclick={onClose} aria-label="Close">×</button>
      </header>

      <p class="lead">
        Extract the individual voice parts from barbershop “part-predominant”
        learning tracks and remix them — solo or drop any part, rebalance,
        re-pan, change tempo without changing pitch, and export.
      </p>

      <div class="section">
        <h4>Built with</h4>
        <ul>
          <li>Tauri + Svelte + TypeScript</li>
          <li>Rust — Symphonia (decoding), Signalsmith Stretch (time-stretch)</li>
          <li>Web Audio API (live preview &amp; mixing)</li>
        </ul>
      </div>

      <div class="section">
        <h4>License &amp; credits</h4>
        <p>
          Released under the MIT License. Time-stretching by
          <button class="inline-link" onclick={() => link("https://signalsmith-audio.co.uk/code/stretch/")}
            >Signalsmith Stretch</button
          > (MIT). Audio decoding by
          <button class="inline-link" onclick={() => link("https://github.com/pdeljanov/Symphonia")}
            >Symphonia</button
          >.
        </p>
      </div>

      {#if REPO_URL || CONTACT || KOFI_URL}
        <div class="links">
          {#if REPO_URL}
            <button class="link-btn" onclick={() => link(REPO_URL)}>GitHub repository</button>
          {/if}
          {#if KOFI_URL}
            <button class="link-btn" onclick={() => link(KOFI_URL)}>Support on Ko-fi</button>
          {/if}
          {#if CONTACT}
            <button class="link-btn" onclick={() => link(`mailto:${CONTACT}`)}>Contact</button>
          {/if}
        </div>
      {/if}

      <footer>
        © {YEAR}
        {#if KOFI_URL}
          <button class="inline-link" onclick={() => link(KOFI_URL)}>{AUTHOR}</button>
        {:else}
          {AUTHOR}
        {/if}. MIT-licensed open source.
      </footer>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 1.5rem;
    z-index: 60;
  }
  .modal {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    width: min(500px, 100%);
    max-height: 85vh;
    overflow: auto;
    padding: 1.1rem 1.2rem;
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }
  .logo {
    width: 44px;
    height: 44px;
    border-radius: 10px;
  }
  h3 {
    margin: 0;
    font-size: 1.15rem;
  }
  .ver {
    font-size: 0.8rem;
    color: var(--text-dim);
  }
  .x {
    background: none;
    border: none;
    color: var(--text-dim);
    font-size: 1.4rem;
    cursor: pointer;
    line-height: 1;
  }
  .lead {
    margin: 0;
    font-size: 0.9rem;
    color: var(--text);
  }
  .section h4 {
    margin: 0 0 0.3rem;
    font-size: 0.78rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-dim);
  }
  .section p {
    margin: 0;
    font-size: 0.85rem;
    color: var(--text);
  }
  .section ul {
    margin: 0;
    padding-left: 1.1rem;
    font-size: 0.85rem;
    color: var(--text);
  }
  .section li {
    margin: 0.1rem 0;
  }
  .links {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .link-btn {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.4rem 0.85rem;
    cursor: pointer;
    font-size: 0.82rem;
  }
  .link-btn:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  .inline-link {
    background: none;
    border: none;
    padding: 0;
    color: var(--accent);
    cursor: pointer;
    font: inherit;
  }
  .inline-link:hover {
    text-decoration: underline;
  }
  footer {
    font-size: 0.76rem;
    color: var(--text-dim);
    border-top: 1px solid var(--border);
    padding-top: 0.7rem;
  }
</style>
