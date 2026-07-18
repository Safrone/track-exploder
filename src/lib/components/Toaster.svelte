<script lang="ts">
  import { toasts, dismiss } from "../toast";
  import { fly } from "svelte/transition";
</script>

<div class="toaster" aria-live="polite">
  {#each $toasts as t (t.id)}
    <div class="toast {t.kind}" role="status" transition:fly={{ y: 16, duration: 200 }}>
      <span class="msg">{t.message}</span>
      <button class="x" aria-label="Dismiss" onclick={() => dismiss(t.id)}>×</button>
    </div>
  {/each}
</div>

<style>
  .toaster {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    z-index: 80;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    max-width: min(360px, calc(100vw - 2rem));
  }
  .toast {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    background: var(--panel);
    border: 1px solid var(--border);
    border-left: 3px solid var(--text-dim);
    border-radius: 8px;
    padding: 0.6rem 0.7rem 0.6rem 0.8rem;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.35);
    font-size: 0.85rem;
    color: var(--text);
  }
  .toast.success {
    border-left-color: var(--accent);
  }
  .toast.error {
    border-left-color: #ef4444;
  }
  .toast.info {
    border-left-color: #60a5fa;
  }
  .msg {
    flex: 1 1 auto;
    min-width: 0;
  }
  .x {
    flex: 0 0 auto;
    background: none;
    border: none;
    color: var(--text-dim);
    font-size: 1.1rem;
    line-height: 1;
    cursor: pointer;
    padding: 0 0.15rem;
  }
  .x:hover {
    color: var(--text);
  }
</style>
