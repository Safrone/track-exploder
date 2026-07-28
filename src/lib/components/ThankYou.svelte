<script lang="ts">
  import { AUTHOR, KOFI_URL, openExternal } from "../links";

  interface Props {
    open: boolean;
    onClose: () => void;
  }
  let { open, onClose }: Props = $props();

  function onKey(e: KeyboardEvent) {
    if (open && e.key === "Escape") onClose();
  }
</script>

<svelte:window onkeydown={onKey} />

{#if open}
  <div class="overlay" role="dialog" aria-modal="true" aria-labelledby="thanks-title">
    <div class="modal">
      <header>
        <div class="title">
          <img src="/logo.svg" alt="" class="logo" />
          <h3 id="thanks-title">Thank you for using Track Exploder!</h3>
        </div>
        <button class="x" onclick={onClose} aria-label="Close">×</button>
      </header>

      <p>
        It's free and open source, built by one person in his spare time. If it's
        enhancing your barbershop experience, a small tip on Ko-fi helps me keep fixing
        bugs here and making more things like this.
      </p>

      <div class="row">
        {#if KOFI_URL}
          <button class="kofi" onclick={() => openExternal(KOFI_URL)}>
            ☕ Support on Ko-fi
          </button>
        {/if}
        <button class="later" onclick={onClose}>Maybe later</button>
      </div>

      <footer>Either way, happy singing. — {AUTHOR}</footer>
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
    z-index: 70;
  }
  .modal {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 12px;
    width: min(420px, 100%);
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
    gap: 0.5rem;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 0.7rem;
  }
  .logo {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    flex: 0 0 auto;
  }
  h3 {
    margin: 0;
    font-size: 1.05rem;
  }
  .x {
    background: none;
    border: none;
    color: var(--text-dim);
    font-size: 1.4rem;
    cursor: pointer;
    line-height: 1;
  }
  p {
    margin: 0;
    font-size: 0.88rem;
    color: var(--text);
  }
  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }
  .kofi {
    background: var(--accent);
    color: #05221a;
    border: none;
    border-radius: 8px;
    padding: 0.5rem 1rem;
    font-weight: 600;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .later {
    background: var(--panel-2);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 0.5rem 0.9rem;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .later:hover {
    border-color: var(--accent);
    color: var(--accent);
  }
  footer {
    font-size: 0.76rem;
    color: var(--text-dim);
    border-top: 1px solid var(--border);
    padding-top: 0.7rem;
  }
</style>
