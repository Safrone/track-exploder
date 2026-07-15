<script lang="ts">
  interface Props {
    /** 0..1. Ignored when `indeterminate` is true. */
    value?: number;
    indeterminate?: boolean;
    label?: string;
  }
  let { value = 0, indeterminate = false, label = "" }: Props = $props();

  const pct = $derived(Math.round(Math.max(0, Math.min(1, value)) * 100));
</script>

<div class="wrap">
  {#if label}
    <div class="labelrow">
      <span class="label">{label}</span>
      {#if !indeterminate}<span class="pct">{pct}%</span>{/if}
    </div>
  {/if}
  <div
    class="track"
    class:indeterminate
    role="progressbar"
    aria-valuenow={indeterminate ? undefined : pct}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-label={label || "Progress"}
  >
    <div class="fill" style:width={indeterminate ? undefined : pct + "%"}></div>
  </div>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
    width: 100%;
  }
  .labelrow {
    display: flex;
    justify-content: space-between;
    font-size: 0.78rem;
    color: var(--text-dim);
  }
  .pct {
    font-variant-numeric: tabular-nums;
  }
  .track {
    position: relative;
    height: 8px;
    border-radius: 999px;
    background: var(--panel-2);
    border: 1px solid var(--border);
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--accent);
    border-radius: 999px;
    transition: width 0.18s ease;
  }
  .track.indeterminate .fill {
    width: 35%;
    position: absolute;
    animation: slide 1.1s ease-in-out infinite;
  }
  @keyframes slide {
    0% {
      left: -40%;
    }
    100% {
      left: 100%;
    }
  }
</style>
