<script lang="ts">
  import { mixer, patchState } from "../mixer/store";
  import { getEngine, position, duration, isPlaying, isStretching } from "../audio/playback";
  import { resetOnDblClick } from "../actions";

  // Local slider value while dragging; committed to the store on release so the
  // stems are only re-stretched once (not on every input tick).
  let dragTempo = $state($mixer.tempo);
  $effect(() => {
    dragTempo = $mixer.tempo;
  });

  function fmt(sec: number): string {
    if (!isFinite(sec)) return "0:00";
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function toggle() {
    const engine = getEngine();
    if ($isPlaying) {
      engine.pause();
      isPlaying.set(false);
    } else {
      await engine.play();
      isPlaying.set(true);
    }
  }

  function seek(e: Event) {
    getEngine().seek(+(e.target as HTMLInputElement).value);
  }
</script>

<div class="transport">
  <button class="play" onclick={toggle} aria-label={$isPlaying ? "Pause" : "Play"}>
    {$isPlaying ? "❚❚" : "►"}
  </button>

  <span class="time">{fmt($position)}</span>
  <input
    class="scrub"
    type="range"
    min="0"
    max={$duration || 0}
    step="0.01"
    value={$position}
    oninput={seek}
  />
  <span class="time">{fmt($duration)}</span>

  <div class="tempo">
    <label class="tempo-head" title="Pitch-preserving time-stretch. Off = no extra processing.">
      <input
        type="checkbox"
        checked={$mixer.tempoEnabled}
        onchange={(e) => patchState({ tempoEnabled: e.currentTarget.checked })}
      />
      Tempo{$mixer.tempoEnabled
        ? ` ${Math.round(dragTempo * 100)}%${$isStretching ? " · preparing…" : ""}`
        : ""}
    </label>
    <input
      type="range"
      min="0.5"
      max="1.5"
      step="0.05"
      value={dragTempo}
      disabled={!$mixer.tempoEnabled || $isStretching}
      use:resetOnDblClick={1}
      oninput={(e) => (dragTempo = +e.currentTarget.value)}
      onchange={(e) => patchState({ tempo: +e.currentTarget.value })}
    />
  </div>

  <label class="master">
    <span>Master {Math.round($mixer.masterGain * 100)}%</span>
    <input
      type="range"
      min="0"
      max="1.5"
      step="0.01"
      value={$mixer.masterGain}
      use:resetOnDblClick={1}
      oninput={(e) => patchState({ masterGain: +e.currentTarget.value })}
    />
  </label>
</div>

<style>
  .transport {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.6rem 0.9rem;
    flex-wrap: wrap;
  }
  .play {
    width: 44px;
    height: 44px;
    border-radius: 50%;
    border: none;
    background: var(--accent);
    color: #05221a;
    font-size: 1.1rem;
    cursor: pointer;
  }
  .time {
    font-variant-numeric: tabular-nums;
    color: var(--text-dim);
    min-width: 3ch;
  }
  .scrub {
    flex: 1 1 200px;
    accent-color: var(--accent);
  }
  .tempo,
  .master {
    display: flex;
    flex-direction: column;
    font-size: 0.72rem;
    color: var(--text-dim);
    gap: 0.15rem;
    min-width: 130px;
  }
  .tempo input[type="range"],
  .master input {
    accent-color: var(--accent);
  }
  .tempo-head {
    display: flex;
    align-items: center;
    gap: 0.3rem;
  }
  .tempo input[type="range"]:disabled {
    opacity: 0.4;
    cursor: default;
  }
</style>
