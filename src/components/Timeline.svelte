<script lang="ts">
  import { onMount } from "svelte";
  import { editor } from "../lib/store.svelte";

  const MIN_VIEW_SPAN = 1.5;
  const HANDLE_FRACTION = 0.15;
  const MIN_KEEP_SECONDS = 0.05;

  let barEl: HTMLDivElement | null = $state(null);
  let navEl: HTMLDivElement | null = $state(null);

  let viewStart = $state(0);
  let viewEnd = $state(0);

  let duration = $derived(editor.video?.duration ?? 0);

  $effect(() => {
    if (editor.video) {
      viewStart = 0;
      viewEnd = editor.video.duration;
    }
  });

  let viewSpan = $derived(Math.max(0.001, viewEnd - viewStart));
  let isZoomed = $derived(duration > 0 && viewSpan < duration - 0.001);

  function clampWindow(start: number, end: number): [number, number] {
    const span = Math.max(MIN_VIEW_SPAN, Math.min(duration, end - start));
    let s = Math.max(0, Math.min(duration - span, start));
    const e = Math.min(duration, s + span);
    if (e > duration) s = duration - span;
    return [Math.max(0, s), Math.min(duration, s + span)];
  }

  onMount(() => {
    const el = barEl;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      if (!duration) return;
      if (Math.abs(e.deltaX) > Math.abs(e.deltaY) || e.shiftKey) {
        const panDelta = (e.shiftKey ? e.deltaY : e.deltaX) * (viewSpan / 600);
        const [s, en] = clampWindow(viewStart + panDelta, viewEnd + panDelta);
        viewStart = s;
        viewEnd = en;
        e.preventDefault();
        return;
      }
      const rect = el.getBoundingClientRect();
      const x = (e.clientX - rect.left) / rect.width;
      const cursorTime = viewStart + x * viewSpan;
      const factor = e.deltaY > 0 ? 1.25 : 0.8;
      const newSpan = Math.max(MIN_VIEW_SPAN, Math.min(duration, viewSpan * factor));
      const [s, en] = clampWindow(
        cursorTime - x * newSpan,
        cursorTime + (1 - x) * newSpan,
      );
      viewStart = s;
      viewEnd = en;
      e.preventDefault();
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  });

  type NavDragMode = "pan" | "resize-l" | "resize-r";
  function startNavDrag(e: PointerEvent, mode: NavDragMode) {
    if (!duration || !navEl) return;
    e.preventDefault();
    e.stopPropagation();
    const track = navEl.getBoundingClientRect();
    const initialStart = viewStart;
    const initialEnd = viewEnd;
    const initialX = e.clientX;
    const onMove = (ev: PointerEvent) => {
      const ds = ((ev.clientX - initialX) / track.width) * duration;
      if (mode === "pan") {
        const [s, en] = clampWindow(initialStart + ds, initialEnd + ds);
        viewStart = s;
        viewEnd = en;
      } else if (mode === "resize-l") {
        viewStart = Math.min(
          initialEnd - MIN_VIEW_SPAN,
          Math.max(0, initialStart + ds),
        );
        viewEnd = initialEnd;
      } else {
        viewStart = initialStart;
        viewEnd = Math.max(
          initialStart + MIN_VIEW_SPAN,
          Math.min(duration, initialEnd + ds),
        );
      }
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function onNavTrackPointer(e: PointerEvent) {
    if (!duration || !navEl) return;
    if (e.target !== navEl) return;
    const rect = navEl.getBoundingClientRect();
    const t = ((e.clientX - rect.left) / rect.width) * duration;
    const halfSpan = viewSpan / 2;
    const [s, en] = clampWindow(t - halfSpan, t + halfSpan);
    viewStart = s;
    viewEnd = en;
  }

  function onWindowPointerDown(e: PointerEvent) {
    const rect = (e.currentTarget as HTMLDivElement).getBoundingClientRect();
    const x = e.clientX - rect.left;
    const handle = Math.min(14, rect.width * HANDLE_FRACTION);
    if (x < handle) startNavDrag(e, "resize-l");
    else if (x > rect.width - handle) startNavDrag(e, "resize-r");
    else startNavDrag(e, "pan");
  }

  function fmt(seconds: number): string {
    const m = Math.floor(seconds / 60);
    const s = (seconds % 60).toFixed(1);
    return `${m}:${s.padStart(4, "0")}`;
  }

  function onBarClick(e: MouseEvent) {
    // If a keep-edge drag just finished we get a click event afterward; the
    // drag handler stops propagation so we never reach this. Click on empty
    // bar area still seeks.
    if (!barEl || !duration) return;
    const rect = barEl.getBoundingClientRect();
    const x = (e.clientX - rect.left) / rect.width;
    const t = viewStart + Math.max(0, Math.min(1, x)) * viewSpan;
    editor.requestSeek(t);
  }

  // ---- keep-edge dragging ----

  type EdgeDragKind = "in" | "out";
  let dragHint = $state<{ x: number; t: number; label: string } | null>(null);

  function pxToTime(clientX: number): number {
    if (!barEl) return 0;
    const rect = barEl.getBoundingClientRect();
    const x = Math.max(0, Math.min(rect.width, clientX - rect.left));
    return viewStart + (x / rect.width) * viewSpan;
  }

  function startEdgeDrag(e: PointerEvent, keepIndex: number, kind: EdgeDragKind) {
    e.preventDefault();
    e.stopPropagation();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const keeps = editor.keepIntervals();
    const prevEnd = keepIndex > 0 ? keeps[keepIndex - 1].end : 0;
    const nextStart =
      keepIndex < keeps.length - 1 ? keeps[keepIndex + 1].start : duration;
    const own = keeps[keepIndex];

    const onMove = (ev: PointerEvent) => {
      const t = pxToTime(ev.clientX);
      if (kind === "in") {
        const next = Math.max(prevEnd, Math.min(own.end - MIN_KEEP_SECONDS, t));
        editor.updateKeep(keepIndex, next, own.end);
        dragHint = { x: ev.clientX, t: next, label: "in" };
      } else {
        const next = Math.min(nextStart, Math.max(own.start + MIN_KEEP_SECONDS, t));
        editor.updateKeep(keepIndex, own.start, next);
        dragHint = { x: ev.clientX, t: next, label: "out" };
      }
    };
    const onUp = () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      dragHint = null;
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  // ---- derived display data ----

  let intervals = $derived(editor.cutlist?.intervals ?? []);
  let segWithKeepIndex = $derived.by(() => {
    let k = 0;
    return intervals.map((c) => ({
      cut: c,
      keepIndex: c.kind === "keep" ? k++ : -1,
    }));
  });
  let visibleSegments = $derived(
    segWithKeepIndex.filter(
      (s) => s.cut.end > viewStart && s.cut.start < viewEnd,
    ),
  );
  // Disabled keeps count toward "removed" since they won't appear in the
  // exported output. Only active keeps contribute to "kept".
  let keptDuration = $derived(
    intervals
      .filter((c) => c.kind === "keep" && !c.disabled)
      .reduce((a, c) => a + (c.end - c.start), 0),
  );
  let removedDuration = $derived(
    intervals
      .filter((c) => c.kind === "remove" || (c.kind === "keep" && c.disabled))
      .reduce((a, c) => a + (c.end - c.start), 0),
  );
  let playheadInView = $derived(
    editor.currentTime >= viewStart && editor.currentTime <= viewEnd,
  );
  let playheadPct = $derived(((editor.currentTime - viewStart) / viewSpan) * 100);
  let previewVisible = $derived(
    editor.usePreviewRange &&
      editor.previewRange[1] > viewStart &&
      editor.previewRange[0] < viewEnd,
  );

  // ---- waveform path (visible slice) ----

  function buildWavePath(peaks: number[]): string {
    if (peaks.length < 2) return "";
    // Compress softer parts so the timeline never looks flat. sqrt scaling
    // matches what most NLEs do visually.
    const maxAmp = Math.max(0.01, ...peaks);
    const normalize = (p: number) => Math.sqrt(Math.max(0, p) / maxAmp);
    let path = "";
    for (let i = 0; i < peaks.length; i++) {
      const x = (i / (peaks.length - 1)) * 100;
      const h = Math.max(0.4, normalize(peaks[i]) * 48);
      path += `${i === 0 ? "M" : " L"}${x.toFixed(3)},${(50 - h).toFixed(3)}`;
    }
    for (let i = peaks.length - 1; i >= 0; i--) {
      const x = (i / (peaks.length - 1)) * 100;
      const h = Math.max(0.4, normalize(peaks[i]) * 48);
      path += ` L${x.toFixed(3)},${(50 + h).toFixed(3)}`;
    }
    path += " Z";
    return path;
  }

  let wavePath = $derived.by(() => {
    if (!editor.waveform || !editor.waveform.length || !duration) return "";
    const total = editor.waveform.length;
    const start = Math.max(
      0,
      Math.min(total - 1, Math.floor((viewStart / duration) * total)),
    );
    const end = Math.max(
      start + 1,
      Math.min(total, Math.ceil((viewEnd / duration) * total)),
    );
    return buildWavePath(editor.waveform.slice(start, end));
  });

  let navWavePath = $derived.by(() => {
    if (!editor.waveform || !editor.waveform.length) return "";
    return buildWavePath(editor.waveform);
  });
</script>

{#if editor.video}
  <section class="card timeline-card">
    <header class="card-head">
      <div>
        <h2>timeline</h2>
        <p class="card-sub">
          {#if editor.cutlist}
            <span class="mono">
              <span class="dot dot-pos"></span> kept {fmt(keptDuration)}
            </span>
            <span class="muted-2">·</span>
            <span class="mono">
              <span class="dot dot-neg"></span> removed {fmt(removedDuration)}
            </span>
          {:else}
            scroll to zoom · drag window below to pan · detect to populate
          {/if}
        </p>
      </div>
      <div class="card-head-actions">
        <div class="transport">
          <button
            class="tbtn"
            onclick={() => editor.prevKeep()}
            disabled={!editor.cutlist}
            title="Previous cut (jump to start of previous keep)"
            aria-label="Previous cut"
          >⏮</button>
          <button
            class="tbtn play"
            onclick={() => editor.togglePlay()}
            disabled={!editor.video}
            title={editor.isPlaying ? "Pause (space)" : "Play (space)"}
            aria-label={editor.isPlaying ? "Pause" : "Play"}
          >{editor.isPlaying ? "⏸" : "▶"}</button>
          <button
            class="tbtn"
            onclick={() => editor.nextKeep()}
            disabled={!editor.cutlist}
            title="Next cut (jump to start of next keep)"
            aria-label="Next cut"
          >⏭</button>
        </div>

        <label class="skip-switch">
          <input type="checkbox" bind:checked={editor.skipRemoved} />
          <span class="mono">{editor.skipRemoved ? "skipping cuts" : "playing all"}</span>
        </label>

        <span class="mono muted-2 time">
          {fmt(editor.currentTime)} / {fmt(duration)}
        </span>
        {#if isZoomed}
          <button
            class="btn btn-ghost btn-sm"
            onclick={() => { viewStart = 0; viewEnd = duration; }}
          >
            {(duration / viewSpan).toFixed(1)}× fit
          </button>
        {/if}
      </div>
    </header>

    <div class="tl-body">
      <div bind:this={barEl} class="bar" onclick={onBarClick} role="presentation">
        {#if wavePath}
          <svg
            class="waveform"
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <path d={wavePath} />
          </svg>
        {/if}
        {#each visibleSegments as s (s.cut.start + "-" + s.cut.end + "-" + s.cut.kind)}
          {@const c = s.cut}
          {@const isKeep = c.kind === "keep"}
          {@const isDisabled = isKeep && c.disabled === true}
          {@const isHovered = isKeep && editor.hoveredKeepIndex === s.keepIndex}
          <div
            class="seg {c.kind}"
            class:disabled={isDisabled}
            class:hovered={isHovered}
            style:left="{((c.start - viewStart) / viewSpan) * 100}%"
            style:width="{((c.end - c.start) / viewSpan) * 100}%"
            title="{isDisabled ? 'disabled' : c.kind} {fmt(c.start)} → {fmt(c.end)}"
            onmouseenter={isKeep ? () => editor.setHoveredKeep(s.keepIndex) : undefined}
            onmouseleave={isKeep ? () => editor.setHoveredKeep(null) : undefined}
            role="presentation"
          >
            {#if isKeep}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
              <div
                class="edge edge-in"
                onpointerdown={(e) => startEdgeDrag(e, s.keepIndex, "in")}
                onclick={(e) => e.stopPropagation()}
                title="Drag to adjust in-point"
                role="separator"
                aria-label="In point handle"
              ></div>
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
              <div
                class="edge edge-out"
                onpointerdown={(e) => startEdgeDrag(e, s.keepIndex, "out")}
                onclick={(e) => e.stopPropagation()}
                title="Drag to adjust out-point"
                role="separator"
                aria-label="Out point handle"
              ></div>
            {/if}
          </div>
        {/each}

        {#if previewVisible}
          <div
            class="preview-band"
            style:left="{((editor.previewRange[0] - viewStart) / viewSpan) * 100}%"
            style:width="{((editor.previewRange[1] - editor.previewRange[0]) / viewSpan) * 100}%"
          ></div>
        {/if}
        {#if playheadInView}
          <div class="playhead" style:left="{playheadPct}%"></div>
        {/if}
        {#if dragHint}
          <div
            class="hint"
            style:left="{((dragHint.t - viewStart) / viewSpan) * 100}%"
          >
            <span class="mono">{dragHint.label} {fmt(dragHint.t)}</span>
          </div>
        {/if}
      </div>

      <div bind:this={navEl} class="nav" onpointerdown={onNavTrackPointer} role="presentation">
        {#if navWavePath}
          <svg
            class="nav-waveform"
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            aria-hidden="true"
          >
            <path d={navWavePath} />
          </svg>
        {/if}
        {#each intervals as c (c.start + "n-" + c.end)}
          <div
            class="nav-seg {c.kind}"
            class:disabled={c.kind === "keep" && c.disabled === true}
            style:left="{(c.start / duration) * 100}%"
            style:width="{((c.end - c.start) / duration) * 100}%"
          ></div>
        {/each}
        <div
          class="nav-playhead"
          style:left="{(editor.currentTime / duration) * 100}%"
        ></div>
        <div
          class="nav-window"
          style:left="{(viewStart / duration) * 100}%"
          style:width="{(viewSpan / duration) * 100}%"
          onpointerdown={onWindowPointerDown}
          role="presentation"
        >
          <div class="nav-handle left"></div>
          <div class="nav-handle right"></div>
        </div>
      </div>
    </div>
  </section>
{/if}

<style>
  .time { font-size: 11px; }

  .transport {
    display: inline-flex;
    align-items: stretch;
    gap: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    background: var(--surface-2);
  }
  .tbtn {
    width: 28px;
    height: 26px;
    background: transparent;
    border: 0;
    color: var(--muted);
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
    padding: 0;
    transition: background 120ms, color 120ms;
    font-family: ui-monospace, monospace;
  }
  .tbtn + .tbtn { border-left: 1px solid var(--border); }
  .tbtn:hover:not(:disabled) {
    background: var(--elevated);
    color: var(--foreground);
  }
  .tbtn:disabled { opacity: 0.35; cursor: not-allowed; }
  .tbtn.play { font-size: 12px; color: var(--foreground); }

  .skip-switch {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 11px;
    color: var(--muted);
    cursor: pointer;
    user-select: none;
  }
  .skip-switch input[type="checkbox"] {
    appearance: none;
    -webkit-appearance: none;
    position: relative;
    width: 30px;
    height: 17px;
    border-radius: 999px;
    background: var(--input);
    border: 1px solid var(--border);
    cursor: pointer;
    transition: background 120ms, border-color 120ms;
    flex-shrink: 0;
  }
  .skip-switch input[type="checkbox"]::after {
    content: "";
    position: absolute;
    top: 1px;
    left: 1px;
    width: 13px;
    height: 13px;
    border-radius: 50%;
    background: var(--muted);
    transition: transform 140ms ease, background 140ms;
  }
  .skip-switch input[type="checkbox"]:checked {
    background: var(--pos);
    border-color: var(--pos);
  }
  .skip-switch input[type="checkbox"]:checked::after {
    transform: translateX(13px);
    background: var(--primary-fg);
  }
  .skip-switch input[type="checkbox"]:checked + .mono {
    color: var(--pos);
  }

  .waveform, .nav-waveform {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
  .waveform path {
    fill: hsl(0 0% 90% / 0.18);
  }
  .nav-waveform path {
    fill: hsl(0 0% 90% / 0.22);
  }
  .tl-body {
    padding: 12px 14px 14px;
    display: grid;
    gap: 8px;
  }
  .bar {
    position: relative;
    width: 100%;
    height: 56px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    cursor: crosshair;
  }
  .seg {
    position: absolute;
    top: 0;
    bottom: 0;
    transition: filter 120ms, box-shadow 120ms;
  }
  .seg.keep {
    background: hsl(142 71% 55% / 0.32);
    border-left: 1px solid hsl(142 71% 55% / 0.6);
    border-right: 1px solid hsl(142 71% 55% / 0.6);
  }
  .seg.keep.hovered {
    background: hsl(142 71% 55% / 0.55);
    box-shadow: inset 0 0 0 1px hsl(142 71% 55% / 0.9);
    filter: brightness(1.1);
    z-index: 2;
  }
  .seg.keep.disabled {
    background: hsl(280 70% 68% / 0.22);
    background-image: repeating-linear-gradient(
      45deg,
      transparent 0px,
      transparent 6px,
      hsl(280 70% 68% / 0.18) 6px,
      hsl(280 70% 68% / 0.18) 7px
    );
    border-left-color: hsl(280 70% 68% / 0.6);
    border-right-color: hsl(280 70% 68% / 0.6);
  }
  .seg.keep.disabled.hovered {
    background: hsl(280 70% 68% / 0.42);
    box-shadow: inset 0 0 0 1px hsl(280 70% 68% / 0.9);
    filter: brightness(1.1);
  }
  .seg.keep.disabled .edge::after {
    background: hsl(280 70% 68% / 0.9);
  }
  .seg.keep.disabled .edge:hover {
    background: hsl(280 70% 68% / 0.2);
  }
  .seg.remove {
    background: hsl(0 84% 65% / 0.18);
    background-image: repeating-linear-gradient(
      135deg,
      transparent 0px,
      transparent 6px,
      hsl(0 84% 65% / 0.18) 6px,
      hsl(0 84% 65% / 0.18) 7px
    );
  }

  .edge {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 8px;
    cursor: ew-resize;
    z-index: 3;
    background: transparent;
    transition: background 120ms;
  }
  .edge::after {
    content: "";
    position: absolute;
    top: 4px;
    bottom: 4px;
    left: 50%;
    width: 2px;
    transform: translateX(-1px);
    background: hsl(142 71% 55% / 0.9);
    border-radius: 1px;
    opacity: 0;
    transition: opacity 120ms;
  }
  .edge-in { left: -4px; }
  .edge-out { right: -4px; }
  .seg.keep:hover .edge::after,
  .seg.keep.hovered .edge::after,
  .edge:hover::after {
    opacity: 1;
  }
  .edge:hover {
    background: hsl(142 71% 55% / 0.2);
  }

  .playhead {
    position: absolute;
    top: -2px;
    bottom: -2px;
    width: 2px;
    background: var(--foreground);
    box-shadow: 0 0 6px hsl(0 0% 100% / 0.6);
    pointer-events: none;
    z-index: 4;
  }
  .preview-band {
    position: absolute;
    top: 0;
    bottom: 0;
    background: hsl(213 94% 68% / 0.06);
    border-left: 1px dashed var(--accent);
    border-right: 1px dashed var(--accent);
    pointer-events: none;
  }
  .hint {
    position: absolute;
    top: -22px;
    transform: translateX(-50%);
    padding: 2px 8px;
    background: var(--elevated);
    border: 1px solid var(--border-strong);
    border-radius: 4px;
    font-size: 10px;
    font-family: var(--font-mono);
    color: var(--foreground);
    white-space: nowrap;
    pointer-events: none;
    z-index: 5;
    box-shadow: 0 4px 12px hsl(0 0% 0% / 0.4);
  }
  .hint::after {
    content: "";
    position: absolute;
    top: 100%;
    left: 50%;
    transform: translateX(-50%);
    border: 4px solid transparent;
    border-top-color: var(--border-strong);
  }

  .nav {
    position: relative;
    width: 100%;
    height: 20px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    overflow: hidden;
    cursor: pointer;
  }
  .nav-seg {
    position: absolute;
    top: 0;
    bottom: 0;
  }
  .nav-seg.keep { background: hsl(142 71% 55% / 0.25); }
  .nav-seg.keep.disabled { background: hsl(280 70% 68% / 0.3); }
  .nav-seg.remove { background: hsl(0 84% 65% / 0.3); }
  .nav-playhead {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--foreground);
    pointer-events: none;
  }
  .nav-window {
    position: absolute;
    top: -1px;
    bottom: -1px;
    border: 1px solid var(--accent);
    background: hsl(213 94% 68% / 0.08);
    border-radius: 3px;
    cursor: grab;
  }
  .nav-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 4px;
    background: var(--accent);
    opacity: 0.7;
  }
  .nav-handle.left { left: 0; cursor: ew-resize; border-radius: 2px 0 0 2px; }
  .nav-handle.right { right: 0; cursor: ew-resize; border-radius: 0 2px 2px 0; }

  .card-sub .dot { margin-right: 4px; }
</style>
