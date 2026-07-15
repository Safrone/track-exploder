<script lang="ts">
  import { position, duration } from "../audio/playback";
  import type { StereoEnvelope } from "../audio/waveform";

  interface Props {
    envelope: StereoEnvelope | null;
    onSeek?: (seconds: number) => void;
  }
  let { envelope, onSeek }: Props = $props();

  let wrap: HTMLDivElement | undefined = $state();
  let canvas: HTMLCanvasElement | undefined = $state();

  // Size the drawing buffer to the (fixed-height) box, accounting for HiDPI.
  function resize() {
    if (!wrap || !canvas) return;
    const dpr = window.devicePixelRatio || 1;
    const w = Math.max(1, Math.floor(wrap.clientWidth * dpr));
    const h = Math.max(1, Math.floor(wrap.clientHeight * dpr));
    if (canvas.width !== w) canvas.width = w;
    if (canvas.height !== h) canvas.height = h;
  }

  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    const laneH = h / 2;
    const centers = [laneH * 0.5, laneH * 1.5];
    const playedX = $duration > 0 ? ($position / $duration) * w : 0;

    if (envelope) {
      // Auto-scale to the current mix's peak so the picture re-fits as parts
      // are added/removed (the combined envelope sums each part's peaks).
      let peak = 0;
      for (let i = 0; i < envelope.l.length; i++) {
        if (envelope.l[i] > peak) peak = envelope.l[i];
        if (envelope.r[i] > peak) peak = envelope.r[i];
      }
      const scale = peak > 0 ? (laneH * 0.46) / peak : 0;

      const chans = [envelope.l, envelope.r];
      for (let c = 0; c < 2; c++) {
        const env = chans[c];
        const n = env.length;
        const cy = centers[c];
        for (let x = 0; x < w; x++) {
          const b = Math.floor((x / w) * n);
          const amp = (env[b] ?? 0) * scale;
          ctx.strokeStyle = x <= playedX ? "#34d399" : "#5eead4";
          ctx.beginPath();
          ctx.moveTo(x + 0.5, cy - amp);
          ctx.lineTo(x + 0.5, cy + amp);
          ctx.stroke();
        }
        ctx.strokeStyle = "#2a2f3a";
        ctx.beginPath();
        ctx.moveTo(0, cy);
        ctx.lineTo(w, cy);
        ctx.stroke();
      }
      ctx.strokeStyle = "#20242e";
      ctx.beginPath();
      ctx.moveTo(0, laneH);
      ctx.lineTo(w, laneH);
      ctx.stroke();

      const dpr = window.devicePixelRatio || 1;
      ctx.fillStyle = "#9aa3b2";
      ctx.font = `${11 * dpr}px system-ui, sans-serif`;
      ctx.fillText("L", 5 * dpr, 13 * dpr);
      ctx.fillText("R", 5 * dpr, laneH + 13 * dpr);
    }

    if ($duration > 0) {
      const x = ($position / $duration) * w;
      ctx.strokeStyle = "#f9fafb";
      ctx.beginPath();
      ctx.moveTo(x, 0);
      ctx.lineTo(x, h);
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

  // Redraw when the mix envelope or playhead changes.
  $effect(() => {
    void envelope;
    void $position;
    void $duration;
    resize();
    draw();
  });

  function handleClick(e: MouseEvent) {
    if (!canvas || $duration <= 0 || !onSeek) return;
    const rect = canvas.getBoundingClientRect();
    const frac = (e.clientX - rect.left) / rect.width;
    onSeek(frac * $duration);
  }
</script>

<div class="wave-wrap" bind:this={wrap}>
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
    height: 128px;
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
