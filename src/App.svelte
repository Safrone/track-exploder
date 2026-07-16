<script lang="ts">
  import { PARTS, type Channel, type Part } from "./lib/types";
  import { mixer, allLoaded, snapshot, patchState } from "./lib/mixer/store";
  import { getEngine, currentEngine } from "./lib/audio/playback";
  import {
    pickAndLoad,
    pickOneAudioFile,
    loadPart,
    reExtractAll,
    displayName,
    basename,
  } from "./lib/audio/load";
  import { mixEnvelope, type StereoEnvelope } from "./lib/audio/waveform";
  import { isTauri } from "./lib/audio/tauri";
  import { readAudioTags } from "./lib/audio/decode";
  import { clearTags, setPartTags } from "./lib/mixer/tags";
  import PartStrip from "./lib/components/PartStrip.svelte";
  import Transport from "./lib/components/Transport.svelte";
  import PresetBar from "./lib/components/PresetBar.svelte";
  import ExportBar from "./lib/components/ExportBar.svelte";
  import Waveform from "./lib/components/Waveform.svelte";
  import ProgressBar from "./lib/components/ProgressBar.svelte";
  import RecentExports from "./lib/components/RecentExports.svelte";
  import About from "./lib/components/About.svelte";
  import { exportsList } from "./lib/mixer/exports";

  const WAVE_BUCKETS = 1000;

  let loading = $state(false);
  let status = $state("");
  let loadProgress = $state<{ done: number; total: number; part: Part } | null>(null);
  let showAbout = $state(false);
  let envelope = $state<StereoEnvelope | null>(null);
  const tauri = isTauri();

  const hasTracks = $derived(PARTS.some((p) => !!$mixer.tracks[p]));

  // Push mixer changes into the engine and regenerate the stereo waveform
  // whenever the mix or the loaded stems change.
  $effect(() => {
    const state = $mixer;
    const engine = currentEngine();
    if (!engine) return;
    engine.applyMix(state);
    envelope = PARTS.some((p) => engine.hasBuffer(p))
      ? mixEnvelope(engine, state, WAVE_BUCKETS)
      : null;
  });

  // Explicit per-part load — pick one file and assign it to `part`. Works on
  // mobile where the file picker returns opaque content:// URIs (no filename to
  // auto-detect the part from).
  async function onLoadPart(part: Part) {
    if (!tauri) {
      status = "File loading needs the app (not a plain browser).";
      return;
    }
    const path = await pickOneAudioFile();
    if (!path) return;
    loading = true;
    status = `Loading ${part}…`;
    try {
      const name = await displayName(path);
      await loadPart(getEngine(), part, path, snapshot().sourceChannel, name);
      currentEngine()?.applyMix(snapshot());
      try {
        setPartTags(part, await readAudioTags(path));
      } catch {
        /* tags best-effort */
      }
      status = `Loaded ${part}`;
    } catch (err) {
      status = `Load failed: ${err}`;
    } finally {
      loading = false;
    }
  }

  async function onLoad() {
    if (!tauri) {
      status = "File loading needs the desktop app (run `npm run tauri dev`).";
      return;
    }
    loading = true;
    status = "Decoding…";
    clearTags();
    try {
      const report = await pickAndLoad(
        getEngine(),
        (done, total, part) => {
          loadProgress = { done, total, part };
        },
        snapshot().sourceChannel,
      );
      currentEngine()?.applyMix(snapshot());

      // Read tags from each loaded source file (for common-tag passthrough).
      const tracks = snapshot().tracks;
      await Promise.all(
        report.loaded.map(async (part) => {
          const path = tracks[part]?.path;
          if (!path) return;
          try {
            setPartTags(part, await readAudioTags(path));
          } catch {
            /* tags are best-effort */
          }
        }),
      );

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
      loadProgress = null;
    }
  }

  async function setSourceChannel(channel: Channel) {
    if (channel === $mixer.sourceChannel) return;
    patchState({ sourceChannel: channel });

    const engine = currentEngine();
    const tracks = snapshot().tracks;
    if (!engine || !PARTS.some((p) => tracks[p])) return;

    loading = true;
    status = "Re-extracting…";
    try {
      await reExtractAll(engine, tracks, channel, (done, total, part) => {
        loadProgress = { done, total, part };
      });
      engine.applyMix(snapshot());
      status = `Isolated part read from ${channel} channel`;
    } catch (err) {
      status = `Re-extract failed: ${err}`;
    } finally {
      loading = false;
      loadProgress = null;
    }
  }
</script>

<main>
  <header>
    <div class="brand">
      <img class="logo" src="/logo.svg" alt="Track Exploder logo" />
      <div>
        <h1>Track Exploder</h1>
        <p>Isolate & remix barbershop part tracks</p>
      </div>
    </div>
    <div class="actions">
      <button class="about" onclick={() => (showAbout = true)}>About</button>
      <button class="load" onclick={onLoad} disabled={loading}>
        {loading ? "Loading…" : "Load part tracks"}
      </button>
    </div>
  </header>

  <About open={showAbout} onClose={() => (showAbout = false)} />

  {#if !tauri}
    <div class="banner">
      Running in a plain browser — loading and exporting files require the desktop
      app. Start it with <code>npm run tauri dev</code>.
    </div>
  {/if}

  {#if loadProgress}
    <ProgressBar
      value={loadProgress.total ? loadProgress.done / loadProgress.total : 0}
      label={`Decoding ${loadProgress.part} · ${loadProgress.done}/${loadProgress.total}`}
    />
  {:else if status}
    <div class="status">{status}</div>
  {/if}

  {#if hasTracks}
    <div class="sourcechan">
      <span class="lbl">Isolated part is on:</span>
      <div class="seg">
        <button
          class:active={$mixer.sourceChannel === "left"}
          disabled={loading}
          onclick={() => setSourceChannel("left")}>Left</button
        >
        <button
          class:active={$mixer.sourceChannel === "right"}
          disabled={loading}
          onclick={() => setSourceChannel("right")}>Right</button
        >
      </div>
      <span class="hint">All files in a learning-track set share the same side.</span>
    </div>
  {/if}

  <div class="mixhead">
    <h2>Output mix</h2>
    <p>
      These controls define exactly what you'll hear in the preview and get in the
      exported file — switch each part on/off, set its level, and pan it.
    </p>
  </div>

  <section class="strips">
    {#each PARTS as part (part)}
      <PartStrip {part} {onLoadPart} />
    {/each}
  </section>

  {#if $allLoaded || envelope}
    <section class="stack">
      <Waveform {envelope} onSeek={(s) => currentEngine()?.seek(s)} />
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
    {#if $exportsList.length > 0}
      <div class="panel">
        <h2>Recent exports</h2>
        <RecentExports />
      </div>
    {/if}
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
    width: 42px;
    height: 42px;
    border-radius: 9px;
    display: block;
    flex: 0 0 auto;
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
  .actions {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  .about {
    background: transparent;
    color: var(--text-dim);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.6rem 1rem;
    cursor: pointer;
    font-size: 0.9rem;
  }
  .about:hover {
    color: var(--accent);
    border-color: var(--accent);
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
  .mixhead h2 {
    margin: 0 0 0.15rem;
    font-size: 1rem;
  }
  .mixhead p {
    margin: 0;
    color: var(--text-dim);
    font-size: 0.82rem;
    max-width: 68ch;
  }
  .sourcechan {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    flex-wrap: wrap;
    background: var(--panel);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 0.55rem 0.85rem;
    font-size: 0.85rem;
  }
  .sourcechan .lbl {
    color: var(--text);
  }
  .sourcechan .hint {
    color: var(--text-dim);
    font-size: 0.78rem;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
  }
  .seg button {
    background: var(--panel-2);
    color: var(--text-dim);
    border: none;
    padding: 0.3rem 0.9rem;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .seg button + button {
    border-left: 1px solid var(--border);
  }
  .seg button.active {
    background: var(--accent);
    color: #05221a;
    font-weight: 600;
  }
  .seg button:disabled {
    opacity: 0.6;
    cursor: default;
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
