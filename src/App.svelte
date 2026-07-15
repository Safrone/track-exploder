<script lang="ts">
  import { PARTS, type Channel, type Part } from "./lib/types";
  import { mixer, allLoaded, snapshot } from "./lib/mixer/store";
  import { getEngine, currentEngine } from "./lib/audio/playback";
  import { pickAndLoad, loadPart, basename } from "./lib/audio/load";
  import { isTauri } from "./lib/audio/tauri";
  import PartStrip from "./lib/components/PartStrip.svelte";
  import Transport from "./lib/components/Transport.svelte";
  import PresetBar from "./lib/components/PresetBar.svelte";
  import ExportBar from "./lib/components/ExportBar.svelte";
  import Waveform from "./lib/components/Waveform.svelte";

  let loading = $state(false);
  let status = $state("");
  let waveBuffer = $state<AudioBuffer | undefined>(undefined);
  const tauri = isTauri();

  // Push mixer changes into the audio engine (only once it exists).
  $effect(() => {
    const state = $mixer;
    currentEngine()?.applyMix(state);
  });

  function refreshWave() {
    const engine = currentEngine();
    if (!engine) return;
    waveBuffer =
      engine.getBuffer("lead") ??
      PARTS.map((p) => engine.getBuffer(p)).find((b) => b) ??
      undefined;
  }

  async function onLoad() {
    if (!tauri) {
      status = "File loading needs the desktop app (run `npm run tauri dev`).";
      return;
    }
    loading = true;
    status = "Decoding…";
    try {
      const report = await pickAndLoad(getEngine());
      currentEngine()?.applyMix(snapshot());
      refreshWave();
      status =
        report.loaded.length > 0
          ? `Loaded: ${report.loaded.join(", ")}` +
            (report.unassigned.length
              ? ` · couldn't match: ${report.unassigned.map(basename).join(", ")}`
              : "")
          : "No parts matched. Name files with tenor/lead/bari/bass.";
    } catch (err) {
      status = `Load failed: ${err}`;
    } finally {
      loading = false;
    }
  }

  async function onChannelChange(part: Part, channel: Channel) {
    const engine = currentEngine();
    const track = $mixer.tracks[part];
    if (!engine || !track) return;
    status = `Re-extracting ${part} (${channel})…`;
    try {
      await loadPart(engine, part, track.path, channel);
      engine.applyMix(snapshot());
      refreshWave();
      status = `${part}: using ${channel} channel`;
    } catch (err) {
      status = `Re-extract failed: ${err}`;
    }
  }
</script>

<main>
  <header>
    <div class="brand">
      <span class="logo">◧</span>
      <div>
        <h1>Track Exploder</h1>
        <p>Isolate & remix barbershop part tracks</p>
      </div>
    </div>
    <button class="load" onclick={onLoad} disabled={loading}>
      {loading ? "Loading…" : "Load part tracks"}
    </button>
  </header>

  {#if !tauri}
    <div class="banner">
      Running in a plain browser — loading and exporting files require the desktop
      app. Start it with <code>npm run tauri dev</code>.
    </div>
  {/if}

  {#if status}
    <div class="status">{status}</div>
  {/if}

  <section class="strips">
    {#each PARTS as part (part)}
      <PartStrip {part} {onChannelChange} />
    {/each}
  </section>

  {#if $allLoaded || waveBuffer}
    <section class="stack">
      <Waveform buffer={waveBuffer} onSeek={(s) => currentEngine()?.seek(s)} />
      <Transport />
      <div class="panel">
        <h2>Presets</h2>
        <PresetBar />
      </div>
      <div class="panel">
        <h2>Export</h2>
        <ExportBar />
      </div>
    </section>
  {:else}
    <section class="empty">
      <p>
        Load the four part tracks for a song (tenor, lead, baritone, bass). Each
        file's isolated voice is pulled from its panned channel; then build any
        mix, preview, and export.
      </p>
    </section>
  {/if}
</main>

<style>
  main {
    max-width: 1120px;
    margin: 0 auto;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .logo {
    font-size: 2rem;
    color: var(--accent);
  }
  h1 {
    font-size: 1.35rem;
    margin: 0;
  }
  .brand p {
    margin: 0;
    color: var(--text-dim);
    font-size: 0.85rem;
  }
  .load {
    background: var(--accent);
    color: #05221a;
    border: none;
    border-radius: 10px;
    padding: 0.6rem 1.1rem;
    font-weight: 600;
    cursor: pointer;
  }
  .load:disabled {
    opacity: 0.6;
  }
  .banner {
    background: #3b2f14;
    border: 1px solid #6b551f;
    color: #f5e6c0;
    border-radius: 8px;
    padding: 0.6rem 0.85rem;
    font-size: 0.85rem;
  }
  .banner code,
  .status {
    font-size: 0.85rem;
  }
  .status {
    color: var(--text-dim);
  }
  .strips {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.75rem;
  }
  @media (max-width: 720px) {
    .strips {
      grid-template-columns: repeat(2, 1fr);
    }
  }
  .stack {
    display: flex;
    flex-direction: column;
    gap: 0.85rem;
  }
  .panel {
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.85rem;
  }
  .panel h2 {
    margin: 0 0 0.6rem;
    font-size: 0.9rem;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .empty {
    color: var(--text-dim);
    background: var(--panel);
    border: 1px dashed var(--border);
    border-radius: 10px;
    padding: 1.5rem;
    text-align: center;
  }
</style>
