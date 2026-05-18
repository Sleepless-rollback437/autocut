<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open } from "@tauri-apps/plugin-dialog";
  import { editor } from "../lib/store.svelte";

  let hovering = $state(false);

  onMount(() => {
    let unlisten: (() => void) | null = null;
    (async () => {
      const webview = getCurrentWebview();
      unlisten = await webview.onDragDropEvent((event) => {
        if (event.payload.type === "over") {
          hovering = true;
        } else if (event.payload.type === "leave") {
          hovering = false;
        } else if (event.payload.type === "drop") {
          hovering = false;
          const p = event.payload.paths[0];
          if (p && /\.(mp4|mov|m4v|mkv|webm|avi)$/i.test(p)) {
            editor.setVideoFile(p);
          }
        }
      });
    })();
    return () => unlisten?.();
  });

  async function pickFile() {
    const selected = await open({
      multiple: false,
      directory: false,
      filters: [
        {
          name: "Video",
          extensions: ["mp4", "mov", "m4v", "mkv", "webm", "avi"],
        },
      ],
    });
    if (typeof selected === "string") editor.setVideoFile(selected);
  }
</script>

<div class="zone" class:hovering>
  <div class="glyph mono">▾</div>
  <h1>drop a video</h1>
  <p class="muted">silero detects speech · ffmpeg cuts · davinci-friendly fcpxml</p>

  <div class="formats mono muted-2">
    .mp4 · .mov · .mkv · .webm · .avi
  </div>

  <button class="btn btn-ghost" onclick={pickFile}>or pick a file</button>

  <div class="hint mono muted-2">
    <span class="kbd">⌘ V</span> paste a path soon — for now use drag-drop
  </div>
</div>

<style>
  .zone {
    width: min(560px, 100%);
    padding: 56px 40px;
    border: 1.5px dashed var(--border-strong);
    border-radius: var(--radius-lg);
    display: grid;
    place-items: center;
    gap: 14px;
    background: var(--surface);
    transition: border-color 160ms, background 160ms, transform 160ms;
  }
  .zone.hovering {
    border-color: var(--accent);
    background:
      radial-gradient(circle at center, hsl(213 94% 68% / 0.06), transparent 70%),
      var(--surface);
    transform: scale(1.01);
  }

  .glyph {
    font-size: 36px;
    color: var(--muted-2);
    line-height: 1;
    animation: bob 2.4s ease-in-out infinite;
  }
  @keyframes bob {
    0%, 100% { transform: translateY(0); }
    50% { transform: translateY(4px); }
  }

  h1 {
    margin: 0;
    font-size: 22px;
    font-weight: 500;
    letter-spacing: -0.01em;
  }
  p { margin: 0; font-size: 13px; }

  .formats {
    font-size: 11px;
    letter-spacing: 0.06em;
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--surface-2);
  }

  .hint {
    margin-top: 6px;
    font-size: 11px;
  }
  .kbd {
    display: inline-block;
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--surface-2);
    color: var(--muted);
    font-size: 10px;
    margin-right: 4px;
  }
</style>
