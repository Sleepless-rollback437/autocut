<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { editor } from "../lib/store.svelte";

  let videoEl: HTMLVideoElement | null = $state(null);

  let path = $derived(editor.pendingPath ?? editor.video?.path ?? null);
  let src = $derived(path ? convertFileSrc(path) : null);

  $effect(() => {
    editor.seekToken;
    if (videoEl) videoEl.currentTime = editor.seekTarget;
  });

  // UI-driven play/pause: any caller can bump playToggleToken to toggle.
  $effect(() => {
    editor.playToggleToken;
    if (!videoEl) return;
    if (editor.playToggleToken === 0) return; // skip initial run
    if (videoEl.paused || videoEl.ended) {
      videoEl.play().catch(() => {});
    } else {
      videoEl.pause();
    }
  });

  // Mirror underlying paused state into the store so UI glyphs match reality.
  $effect(() => {
    if (!videoEl) return;
    const v = videoEl;
    const onPlay = () => (editor.isPlaying = true);
    const onPause = () => (editor.isPlaying = false);
    v.addEventListener("play", onPlay);
    v.addEventListener("playing", onPlay);
    v.addEventListener("pause", onPause);
    v.addEventListener("ended", onPause);
    return () => {
      v.removeEventListener("play", onPlay);
      v.removeEventListener("playing", onPlay);
      v.removeEventListener("pause", onPause);
      v.removeEventListener("ended", onPause);
    };
  });

  // Spacebar play/pause anywhere in the app, except while typing in inputs.
  $effect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.code !== "Space" && e.key !== " ") return;
      const ae = document.activeElement as HTMLElement | null;
      const tag = ae?.tagName;
      if (
        tag === "INPUT" ||
        tag === "TEXTAREA" ||
        tag === "SELECT" ||
        ae?.isContentEditable
      ) {
        return;
      }
      if (!videoEl) return;
      e.preventDefault();
      if (videoEl.paused || videoEl.ended) {
        videoEl.play().catch(() => {});
      } else {
        videoEl.pause();
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  });

  // Frame-rate-precision skip. `ontimeupdate` only fires ~4×/sec which is
  // long enough to be visible as a stutter when a remove segment is short.
  // requestVideoFrameCallback fires per decoded frame, so the jump lands
  // within one frame of entering the remove region. Fallback to rAF for
  // browsers without rVFC (Firefox at time of writing).
  $effect(() => {
    if (!videoEl) return;
    const v = videoEl;
    let cancelled = false;
    let handle = 0;

    function tick() {
      if (cancelled) return;
      const t = v.currentTime;
      editor.currentTime = t;
      if (editor.skipRemoved && editor.cutlist) {
        for (const c of editor.cutlist.intervals) {
          const isOmitted = c.kind === "remove" || (c.kind === "keep" && c.disabled);
          if (isOmitted && t >= c.start && t < c.end - 0.02) {
            v.currentTime = c.end;
            break;
          }
        }
      }
      schedule();
    }

    function schedule() {
      if (cancelled) return;
      // While paused, fall back to a single timeupdate-driven refresh so we
      // don't spin a rAF loop indefinitely.
      if (v.paused || v.ended) {
        handle = window.setTimeout(tick, 100);
        return;
      }
      const anyV = v as unknown as {
        requestVideoFrameCallback?: (cb: () => void) => number;
      };
      if (anyV.requestVideoFrameCallback) {
        handle = anyV.requestVideoFrameCallback(tick);
      } else {
        handle = requestAnimationFrame(tick);
      }
    }

    function start() { cancelled = false; schedule(); }
    function stop() { cancelled = true; }

    v.addEventListener("play", start);
    v.addEventListener("playing", start);
    v.addEventListener("pause", () => { editor.currentTime = v.currentTime; });
    v.addEventListener("seeked", () => { editor.currentTime = v.currentTime; });
    schedule();

    return () => {
      stop();
      v.removeEventListener("play", start);
      v.removeEventListener("playing", start);
    };
  });
</script>

{#if src}
  <video
    bind:this={videoEl}
    {src}
    controls
    preload="metadata"
  >
    <track kind="captions" />
  </video>
{:else}
  <div class="empty mono muted-2">no video loaded</div>
{/if}

<style>
  video {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: #000;
    display: block;
  }
  .empty {
    display: grid;
    place-items: center;
    height: 100%;
    font-size: 12px;
  }
</style>
