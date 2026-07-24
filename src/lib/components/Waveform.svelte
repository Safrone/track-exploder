<script lang="ts">
  /**
   * One lane per voice. Within a lane the waveform grows **upward for the left
   * channel and downward for the right**, each side shaded a little differently
   * so the two are easy to tell apart, and panning is visible at a glance: a
   * part panned left leans up, panned right leans down, centred is symmetric.
   * Gain sets how tall the lane draws, and muted parts are greyed out — you can
   * still see what you've dropped out of the mix.
   *
   * Every lane is scaled against its own stem, never against the loudest one, so
   * turning one part up grows that lane alone and leaves the rest where they
   * were. A lane that would overflow is clipped at its own edge rather than
   * bleeding into its neighbour.
   */
  import { position, duration } from "../audio/playback";
  import { type PartLane } from "../audio/waveform";

  interface Props {
    lanes: PartLane[];
    onSeek?: (seconds: number) => void;
  }
  let { lanes, onSeek }: Props = $props();

  let wrap: HTMLDivElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();

  // Left sits above the line in the lighter shade, right below in the deeper one.
  const AUDIBLE = {
    played: { left: "#34d399", right: "#1c9c74" },
    ahead: { left: "#5eead4", right: "#35b9a7" },
  };
  const MUTED = {
    played: { left: "#4b5563", right: "#39404e" },
    ahead: { left: "#3d4451", right: "#2f3440" },
  };
  const LANE_PX = 42;
  /** Left column holding each lane's name and its L/R key. */
  const GUTTER_PX = 78;
  /** Share of a lane's half-height a part at unity gain, panned hard, fills. */
  const UNITY_FILL = 0.78;

  // Size the drawing buffer to the box, accounting for HiDPI.
  function resize() {
    if (!wrap || !canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.floor(wrap.clientWidth * dpr));
    const h = Math.max(1, Math.floor(wrap.clientHeight * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
  }

  /**
   * Stroke one channel's half of a lane over one stretch of time, as a single
   * path — cheap enough to redraw every frame. `direction` is -1 for the left
   * channel (above the centre line) and +1 for the right (below it).
   */
  function strokeHalf(
    ctx: CanvasRenderingContext2D,
    lane: PartLane,
    from: number,
    to: number,
    cy: number,
    scale: number,
    limit: number,
    gutter: number,
    plot: number,
    color: string,
    level: number,
    direction: -1 | 1,
  ) {
    if (to <= from) return;
    ctx.strokeStyle = color;
    ctx.beginPath();
    for (let x = Math.max(from, gutter); x < to; x++) {
      const bucket = Math.floor(((x - gutter) / plot) * lane.peaks.length);
      // Always leave a hairline (a silent stretch still reads as a lane), and
      // never draw past `limit` — the lane's own edge.
      const height = Math.min(Math.max((lane.peaks[bucket] ?? 0) * level * scale, 0.5), limit);
      ctx.moveTo(x + 0.5, cy);
      ctx.lineTo(x + 0.5, cy + direction * height);
    }
    ctx.stroke();
  }

  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const w = canvas.width;
    const h = canvas.height;
    const dpr = window.devicePixelRatio || 1;
    ctx.clearRect(0, 0, w, h);
    if (lanes.length === 0) return;

    const laneH = h / lanes.length;
    const gutter = GUTTER_PX * dpr;
    const plot = Math.max(1, w - gutter);
    const half = laneH / 2;
    const limit = half - 1.5 * dpr; // keep lanes out of each other's way
    const playedX = $duration > 0 ? gutter + Math.min($position / $duration, 1) * plot : gutter;

    lanes.forEach((lane, i) => {
      const cy = laneH * (i + 0.5);
      const colors = lane.silent ? MUTED : AUDIBLE;
      // Scaled against this stem alone: another part's fader can't resize it.
      const scale = lane.peak > 0 ? (half * UNITY_FILL) / lane.peak : 0;
      const played = Math.floor(playedX);
      const half_ = (from: number, to: number, shade: { left: string; right: string }) => {
        strokeHalf(ctx, lane, from, to, cy, scale, limit, gutter, plot, shade.left, lane.left, -1);
        strokeHalf(ctx, lane, from, to, cy, scale, limit, gutter, plot, shade.right, lane.right, 1);
      };
      half_(gutter, played, colors.played);
      half_(played, w, colors.ahead);

      // Centre line — the left/right divide for this voice.
      ctx.strokeStyle = "#2a2f3a";
      ctx.beginPath();
      ctx.moveTo(gutter, cy);
      ctx.lineTo(w, cy);
      ctx.stroke();

      if (i > 0) {
        ctx.strokeStyle = "#20242e";
        ctx.beginPath();
        ctx.moveTo(0, laneH * i);
        ctx.lineTo(w, laneH * i);
        ctx.stroke();
      }

      const name = lane.part[0].toUpperCase() + lane.part.slice(1);
      ctx.fillStyle = lane.silent ? "#6b7280" : "#c7cdd8";
      ctx.font = `${11 * dpr}px system-ui, sans-serif`;
      ctx.fillText(name, 8 * dpr, cy + (lane.silent ? 0 : 4 * dpr));
      if (lane.silent) {
        ctx.fillStyle = "#586074";
        ctx.font = `${9 * dpr}px system-ui, sans-serif`;
        ctx.fillText("muted", 8 * dpr, cy + 12 * dpr);
      }

      // Which way is which, on every lane and tinted to that lane's shades.
      ctx.font = `${9 * dpr}px system-ui, sans-serif`;
      ctx.fillStyle = colors.ahead.left;
      ctx.fillText("L", gutter - 12 * dpr, cy - 4 * dpr);
      ctx.fillStyle = colors.ahead.right;
      ctx.fillText("R", gutter - 12 * dpr, cy + 13 * dpr);
    });

    // The gutter's edge, and which way is up.
    ctx.strokeStyle = "#2a2f3a";
    ctx.beginPath();
    ctx.moveTo(gutter, 0);
    ctx.lineTo(gutter, h);
    ctx.stroke();
    if ($duration > 0) {
      ctx.strokeStyle = "#f9fafb";
      ctx.beginPath();
      ctx.moveTo(playedX, 0);
      ctx.lineTo(playedX, h);
      ctx.stroke();
    }
  }

  // Keep the canvas sized to its box (and redraw) on layout changes.
  $effect(() => {
    if (!wrap) return;
    const ro = new ResizeObserver(() => {
      resize();
      draw();
    });
    ro.observe(wrap);
    resize();
    draw();
    return () => ro.disconnect();
  });

  // Redraw when the mix, the stems or the playhead change.
  $effect(() => {
    void lanes;
    void $position;
    void $duration;
    resize();
    draw();
  });

  function handleClick(e: MouseEvent) {
    if (!canvas || $duration <= 0 || !onSeek) return;
    const rect = canvas.getBoundingClientRect();
    const frac = (e.clientX - rect.left - GUTTER_PX) / Math.max(1, rect.width - GUTTER_PX);
    onSeek(Math.min(Math.max(frac, 0), 1) * $duration);
  }
</script>

<div
  class="wave-wrap"
  style="height: {Math.max(1, lanes.length) * LANE_PX}px"
  bind:this={wrap}
>
  <canvas
    bind:this={canvas}
    class="waveform"
    onclick={handleClick}
    role="slider"
    aria-label="Seek"
    aria-valuenow={$position}
    aria-valuemin={0}
    aria-valuemax={$duration}
    tabindex="0"
  ></canvas>
</div>

<style>
  .wave-wrap {
    position: relative;
    width: 100%;
    border-radius: 8px;
    overflow: hidden;
    background: var(--panel-2);
    flex: 0 0 auto;
  }
  .waveform {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    cursor: pointer;
  }
</style>
