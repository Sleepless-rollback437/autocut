<script lang="ts">
  type Props = {
    value: number;
    min: number;
    max: number;
    step: number;
    label: string;
    format?: (v: number) => string;
    oninput?: () => void;
  };

  let {
    value = $bindable(),
    min,
    max,
    step,
    label,
    format,
    oninput,
  }: Props = $props();

  let trackEl: HTMLDivElement | null = $state(null);
  let rootEl: HTMLDivElement | null = $state(null);
  let dragging = $state(false);

  let pct = $derived.by(() => {
    const range = max - min;
    if (range <= 0) return 0;
    return Math.max(0, Math.min(100, ((value - min) / range) * 100));
  });

  let formatted = $derived(format ? format(value) : value.toString());

  function quantize(raw: number, fine: boolean): number {
    // Shift = 10x finer step for precise tuning. Numerical guard against
    // float drift so values like 0.30 don't end up as 0.29999...
    const s = fine ? step / 10 : step;
    const snapped = Math.round((raw - min) / s) * s + min;
    return Math.max(min, Math.min(max, parseFloat(snapped.toFixed(6))));
  }

  function setFromClientX(clientX: number, fine: boolean) {
    if (!trackEl) return;
    const rect = trackEl.getBoundingClientRect();
    const p = (clientX - rect.left) / rect.width;
    const raw = min + Math.max(0, Math.min(1, p)) * (max - min);
    const next = quantize(raw, fine);
    if (next !== value) {
      value = next;
      oninput?.();
    }
  }

  function onPointerDown(e: PointerEvent) {
    if (!trackEl) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging = true;
    setFromClientX(e.clientX, e.shiftKey);
    rootEl?.focus();

    const onMove = (ev: PointerEvent) => setFromClientX(ev.clientX, ev.shiftKey);
    const onUp = () => {
      dragging = false;
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function onKeyDown(e: KeyboardEvent) {
    let dir = 0;
    if (e.key === "ArrowLeft" || e.key === "ArrowDown") dir = -1;
    else if (e.key === "ArrowRight" || e.key === "ArrowUp") dir = 1;
    else if (e.key === "Home") {
      value = min;
      oninput?.();
      e.preventDefault();
      return;
    } else if (e.key === "End") {
      value = max;
      oninput?.();
      e.preventDefault();
      return;
    }
    if (!dir) return;
    const s = e.shiftKey ? step / 10 : step;
    const next = quantize(value + dir * s, e.shiftKey);
    if (next !== value) {
      value = next;
      oninput?.();
    }
    e.preventDefault();
  }
</script>

<div
  bind:this={rootEl}
  class="slider"
  class:dragging
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-valuetext={formatted}
  onkeydown={onKeyDown}
>
  <div
    bind:this={trackEl}
    class="track"
    onpointerdown={onPointerDown}
    role="presentation"
  >
    <div class="fill" style:width="{pct}%"></div>
    <div class="edge" style:left="{pct}%"></div>
    <span class="label mono">{label}</span>
    <span class="value mono">{formatted}</span>
  </div>
</div>

<style>
  .slider {
    outline: none;
    border-radius: var(--radius);
  }
  .slider:focus-visible .track {
    box-shadow: 0 0 0 2px hsl(0 0% 98% / 0.18);
  }

  /* Unified pill-shaped control: label sits on the left, value on the right,
     fill rises from 0% to the current value behind both. A thin edge line
     marks the current-value boundary and lifts to full opacity on
     hover/drag for precise scrubbing feedback. */
  .track {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 38px;
    padding: 0 14px;
    background: var(--input);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    cursor: ew-resize;
    overflow: hidden;
    touch-action: none;
    transition: border-color 140ms, background 140ms;
    user-select: none;
  }
  .slider:hover .track,
  .slider.dragging .track {
    border-color: var(--border-strong);
  }

  .fill {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    background: linear-gradient(
      90deg,
      hsl(0 0% 98% / 0.04) 0%,
      hsl(0 0% 98% / 0.14) 100%
    );
    transition: background 160ms;
    pointer-events: none;
  }
  .slider:hover .fill,
  .slider.dragging .fill {
    background: linear-gradient(
      90deg,
      hsl(0 0% 98% / 0.08) 0%,
      hsl(0 0% 98% / 0.24) 100%
    );
  }

  /* Leading edge of the fill: 1px white line. Subtle when idle, fully
     visible on interaction. */
  .edge {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--foreground);
    transform: translateX(-0.5px);
    opacity: 0.3;
    transition: opacity 140ms;
    pointer-events: none;
  }
  .slider:hover .edge,
  .slider.dragging .edge {
    opacity: 1;
  }

  .label {
    position: relative;
    z-index: 1;
    font-size: 10px;
    color: var(--muted-2);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    pointer-events: none;
    transition: color 140ms;
  }
  .slider:hover .label,
  .slider.dragging .label {
    color: var(--muted);
  }

  .value {
    position: relative;
    z-index: 1;
    font-size: 13px;
    color: var(--foreground);
    font-variant-numeric: tabular-nums;
    font-weight: 500;
    pointer-events: none;
  }
</style>
