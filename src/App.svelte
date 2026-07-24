<script lang="ts">
  import { PARTS, type Channel, type Part } from "./lib/types";
  import {
    mixer,
    allLoaded,
    snapshot,
    patchState,
    setAlignment,
    clearAlignment,
  } from "./lib/mixer/store";
  import { getEngine, currentEngine, rewindPlayback } from "./lib/audio/playback";
  import {
    pickAndLoad,
    pickOneAudioFile,
    loadPart,
    reExtractAll,
    displayName,
    basename,
  } from "./lib/audio/load";
  import { analyzeAlignment, shiftMs } from "./lib/audio/align";
  import { partLanes, type PartLane } from "./lib/audio/waveform";
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
  import Toaster from "./lib/components/Toaster.svelte";
  import { toast } from "./lib/toast";
  import { exportsList } from "./lib/mixer/exports";

  const WAVE_BUCKETS = 1000;

  let loading = $state(false);
  let aligning = $state(false);
  let loadProgress = $state<{ done: number; total: number; part: Part } | null>(null);
  let showAbout = $state(false);
  let lanes = $state<PartLane[]>([]);
  const tauri = isTauri();

  const hasTracks = $derived(PARTS.some((p) => !!$mixer.tracks[p]));
  const shifted = $derived(PARTS.filter((p) => $mixer.alignment[p]?.deltaFrames));

  // Push mixer changes into the engine and rebuild the per-part waveform lanes
  // whenever the mix or the loaded stems change.
  $effect(() => {
    const state = $mixer;
    const engine = currentEngine();
    if (!engine) return;
    engine.applyMix(state);
    lanes = partLanes(engine, state, WAVE_BUCKETS);
  });

  // Explicit per-part load — pick one file and assign it to `part`. Works on
  // mobile where the file picker returns opaque content:// URIs (no filename to
  // auto-detect the part from).
  async function onLoadPart(part: Part) {
    if (!tauri) {
      toast("File loading needs the app (not a plain browser).", "error");
      return;
    }
    const path = await pickOneAudioFile();
    if (!path) return;
    loading = true;
    try {
      const name = await displayName(path);
      await loadPart(getEngine(), part, path, snapshot().sourceChannel, name);
      currentEngine()?.applyMix(snapshot());
      rewindPlayback(); // the stem changed under the playhead
      try {
        setPartTags(part, await readAudioTags(path));
      } catch {
        /* tags best-effort */
      }
      toast(`Loaded ${part}`, "success");
    } catch (err) {
      toast(`Load failed: ${err}`, "error");
    } finally {
      loading = false;
    }
  }

  async function onLoad() {
    if (!tauri) {
      toast("File loading needs the desktop app (run `npm run tauri dev`).", "error");
      return;
    }
    loading = true;
    clearTags();
    clearAlignment(); // offsets belong to the set being replaced
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

      if (report.loaded.length > 0) {
        rewindPlayback(); // start the new song from the top
        toast(`Loaded: ${report.loaded.join(", ")}`, "success");
        if (report.unassigned.length) {
          toast(`Couldn't match: ${report.unassigned.map(basename).join(", ")}`, "info");
        }
        // Publishers' part files are often a few milliseconds (sometimes much
        // more) out of step with each other; check as soon as they're loaded and
        // only speak up when there's something to fix.
        if (report.loaded.length > 1) void autoAlign(false);
      } else {
        toast("No parts matched. Name files with tenor/lead/bari/bass.", "error");
      }
    } catch (err) {
      toast(`Load failed: ${err}`, "error");
    } finally {
      loading = false;
      loadProgress = null;
    }
  }

  /**
   * Measure how far apart the loaded parts play and line them up.
   *
   * Publishers paste the song in after the spoken title and pitch pipe, and not
   * always at the same spot, so parts can be tens (occasionally hundreds) of
   * milliseconds out. The correction lands in the silent lead-in gap, which keeps
   * the spoken intro aligned; every part strip shows its shift and can be nudged.
   */
  async function autoAlign(announceWhenAligned = true) {
    const tracks = snapshot().tracks;
    if (PARTS.filter((p) => tracks[p]).length < 2) return;

    aligning = true;
    try {
      const report = await analyzeAlignment(tracks);
      const moved: string[] = [];
      for (const part of PARTS) {
        const alignment = report.alignment[part];
        if (!alignment) continue;
        setAlignment(part, alignment.deltaFrames ? alignment : undefined);
        const ms = shiftMs(alignment);
        if (ms !== 0) moved.push(`${part} ${ms > 0 ? "+" : "−"}${Math.abs(ms).toFixed(0)} ms`);
      }
      currentEngine()?.applyMix(snapshot());

      if (moved.length) {
        toast(`Aligned: ${moved.join(", ")}`, "success");
        if (report.unsteady.length) {
          toast(
            `${report.unsteady.join(", ")} drift through the song — nudge to taste.`,
            "info",
          );
        }
      } else if (announceWhenAligned) {
        toast("Parts are already in sync.", "info");
      }
      if (report.unmeasured.length) {
        toast(`Couldn't measure: ${report.unmeasured.join(", ")}`, "info");
      }
    } catch (err) {
      toast(`Alignment failed: ${err}`, "error");
    } finally {
      aligning = false;
    }
  }

  function resetAlignment() {
    clearAlignment();
    currentEngine()?.applyMix(snapshot());
    toast("Parts play exactly as recorded.", "info");
  }

  async function setSourceChannel(channel: Channel) {
    if (channel === $mixer.sourceChannel) return;
    patchState({ sourceChannel: channel });

    const engine = currentEngine();
    const tracks = snapshot().tracks;
    if (!engine || !PARTS.some((p) => tracks[p])) return;

    loading = true;
    try {
      await reExtractAll(engine, tracks, channel, (done, total, part) => {
        loadProgress = { done, total, part };
      });
      engine.applyMix(snapshot());
      toast(`Isolated part read from ${channel} channel`, "success");
    } catch (err) {
      toast(`Re-extract failed: ${err}`, "error");
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
  <Toaster />

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

    <div class="sourcechan">
      <span class="lbl">Track timing:</span>
      <button class="alignbtn" disabled={aligning || loading} onclick={() => autoAlign()}>
        {aligning ? "Measuring…" : "Auto-align"}
      </button>
      <button
        class="alignbtn"
        disabled={aligning || loading || shifted.length === 0}
        onclick={resetAlignment}>Reset</button
      >
      <span class="hint">
        {#if shifted.length}
          Shifted: {shifted.join(", ")} — the correction sits in the silent gap after
          the pitch pipe, so the spoken intro stays in line.
        {:else}
          Part files aren't always pasted in at the same spot; this lines them up.
        {/if}
      </span>
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

  {#if $allLoaded || lanes.length > 0}
    <section class="stack">
      <Waveform {lanes} onSeek={(s) => currentEngine()?.seek(s)} />
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
  .banner code {
    font-size: 0.85rem;
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
  .alignbtn {
    padding: 0.3rem 0.9rem;
    border-radius: 8px;
    border: 1px solid var(--border);
    background: var(--panel-2);
    color: var(--text);
    cursor: pointer;
    font-size: 0.85rem;
  }
  .alignbtn:hover:not(:disabled) {
    border-color: var(--accent);
    color: var(--accent);
  }
  .alignbtn:disabled {
    opacity: 0.5;
    cursor: default;
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
  @media (max-width: 480px) {
    /* One full-width strip per row so the sliders have room on a phone. */
    .strips {
      grid-template-columns: 1fr;
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
