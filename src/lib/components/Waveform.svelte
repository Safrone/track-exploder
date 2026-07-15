<script lang="ts">
  import { position, duration } from "../audio/playback";
  import type { StereoEnvelope } from "../audio/waveform";

  interface Props {
    envelope: StereoEnvelope | null;
    onSeek?: (seconds: number) => void;
  }
  let { envelope, onSeek }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();

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
      const chans = [envelope.l, envelope.r];
      for (let c = 0; c < 2; c++) {
        const env = chans[c];
        const n = env.length;
        const cy = centers[c];
        for (let x = 0; x < w; x++) {
          const b = Math.floor((x / w) * n);
          const amp = Math.min(env[b] ?? 0, 1) * (laneH * 0.46);
          ctx.strokeStyle = x <= playedX ? "#34d399" : "#5eead4";
          ctx.beginPath();
          ctx.moveTo(x + 0.5, cy - amp);
          ctx.lineTo(x + 0.5, cy + amp);
          ctx.stroke();
        }
        // Channel centre line.
        ctx.strokeStyle = "#2a2f3a";
        ctx.beginPath();
        ctx.moveTo(0, cy);
        ctx.lineTo(w, cy);
        ctx.stroke();
      }
      // Divider between L and R lanes.
      ctx.strokeStyle = "#20242e";
      ctx.beginPath();
      ctx.moveTo(0, laneH);
      ctx.lineTo(w, laneH);
      ctx.stroke();

      // Lane labels.
      ctx.fillStyle = "#9aa3b2";
      ctx.font = "11px system-ui, sans-serif";
      ctx.fillText("L", 5, 13);
      ctx.fillText("R", 5, laneH + 13);
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

  $effect(() => {
    // Redraw when the mix envelope or the playhead changes.
    void envelope;
    void $position;
    void $duration;
    draw();
  });

  function handleClick(e: MouseEvent) {
    if (!canvas || $duration <= 0 || !onSeek) return;
    const rect = canvas.getBoundingClientRect();
    const frac = (e.clientX - rect.left) / rect.width;
    onSeek(frac * $duration);
  }
</script>

<canvas
  bind:this={canvas}
  width="1000"
  height="160"
  class="waveform"
  onclick={handleClick}
  role="slider"
  aria-label="Seek"
  aria-valuenow={$position}
  aria-valuemin={0}
  aria-valuemax={$duration}
  tabindex="0"
></canvas>

<style>
  .waveform {
    width: 100%;
    height: 160px;
    display: block;
    background: var(--panel-2);
    border-radius: 8px;
    cursor: pointer;
  }
</style>
