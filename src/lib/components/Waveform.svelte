<script lang="ts">
  import { position, duration } from "../audio/playback";

  interface Props {
    buffer: AudioBuffer | undefined;
    onSeek?: (seconds: number) => void;
  }
  let { buffer, onSeek }: Props = $props();

  let canvas: HTMLCanvasElement | undefined = $state();

  // Downsample the buffer to per-pixel min/max peaks and draw.
  function draw() {
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);

    const style = getComputedStyle(canvas);
    const wave = style.getPropertyValue("--wave").trim() || "#6ee7b7";
    const played = style.getPropertyValue("--wave-played").trim() || "#34d399";

    if (buffer) {
      const data = buffer.getChannelData(0);
      const step = Math.max(1, Math.floor(data.length / w));
      const playedX = $duration > 0 ? ($position / $duration) * w : 0;
      for (let x = 0; x < w; x++) {
        let min = 1;
        let max = -1;
        for (let i = 0; i < step; i++) {
          const v = data[x * step + i] ?? 0;
          if (v < min) min = v;
          if (v > max) max = v;
        }
        ctx.strokeStyle = x <= playedX ? played : wave;
        ctx.beginPath();
        ctx.moveTo(x + 0.5, ((1 - max) * h) / 2);
        ctx.lineTo(x + 0.5, ((1 - min) * h) / 2);
        ctx.stroke();
      }
    }

    // Playhead line
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
    // Redraw whenever the buffer or playhead changes.
    void buffer;
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
  height="120"
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
    height: 120px;
    display: block;
    background: var(--panel-2);
    border-radius: 8px;
    cursor: pointer;
    --wave: #5eead4;
    --wave-played: #34d399;
  }
</style>
