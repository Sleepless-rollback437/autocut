<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { open } from "@tauri-apps/plugin-dialog";
  import { editor } from "../lib/store.svelte";

  let hovering = $state(false);

  async function openExternal(url: string) {
    try {
      await invoke("plugin:shell|open", { path: url });
    } catch {
      /* fall through silently — link is plain text if the open fails */
    }
  }

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
            editor.loadVideo(p);
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
    if (typeof selected === "string") editor.loadVideo(selected);
  }
</script>

<div class="empty">
  <section class="hero card" class:hovering>
    <div class="hero-glyph" aria-hidden="true">
      <svg viewBox="0 0 64 64" width="44" height="44">
        <rect x="6" y="14" width="52" height="36" rx="4"
              fill="none" stroke="currentColor" stroke-width="1.2" />
        <line x1="6" y1="22" x2="58" y2="22"
              stroke="currentColor" stroke-width="1.2" opacity="0.5" />
        <line x1="6" y1="42" x2="58" y2="42"
              stroke="currentColor" stroke-width="1.2" opacity="0.5" />
        <line x1="32" y1="14" x2="32" y2="50"
              stroke="currentColor" stroke-width="1.4" stroke-dasharray="2 3" />
        <path d="M32 4 L32 12 M28 8 L32 12 L36 8"
              fill="none" stroke="currentColor" stroke-width="1.4"
              stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    </div>

    <h1 class="hero-title">drop a video</h1>

    <button class="btn btn-primary btn-lg" onclick={pickFile}>
      browse files
    </button>

    <div class="formats">
      <span class="mono">mp4</span>
      <span class="sep">·</span>
      <span class="mono">mov</span>
      <span class="sep">·</span>
      <span class="mono">m4v</span>
      <span class="sep">·</span>
      <span class="mono">mkv</span>
      <span class="sep">·</span>
      <span class="mono">webm</span>
      <span class="sep">·</span>
      <span class="mono">avi</span>
    </div>
  </section>

  <footer class="meta mono muted-2">
    <span><span class="brand-dot"></span> autocut</span>
    <span class="sep">·</span>
    <span>v0.2.0</span>
    <span class="sep">·</span>
    <button
      class="link"
      onclick={() => openExternal("https://github.com/cobanov/autocut")}
    >github.com/cobanov/autocut</button>
    <span class="sep">·</span>
    <span>built by mert cobanov</span>
  </footer>
</div>

<style>
  .empty {
    width: min(720px, 100%);
    display: grid;
    gap: 24px;
    padding: 12px;
  }

  /* ===== hero ===== */
  /* The hero is a drop target, so it advertises that via a dashed border
     and a tightly-spaced "dotted" inset rule. Stays refined: subtle until
     the user drags a file over the window, at which point both the outer
     dashed stroke and the inner radial glow lift to the accent color. */
  .hero {
    padding: 48px 40px 36px;
    text-align: center;
    display: grid;
    gap: 14px;
    place-items: center;
    background: linear-gradient(
        180deg,
        hsl(240 6% 8%) 0%,
        hsl(240 6% 6%) 100%
      );
    border-style: dashed;
    border-color: var(--border-strong);
    border-width: 1.5px;
    transition: border-color 200ms, background 200ms, transform 200ms;
  }
  .hero.hovering {
    border-color: var(--accent);
    background:
      radial-gradient(
        circle at 50% 0%,
        hsl(213 94% 68% / 0.14),
        transparent 60%
      ),
      hsl(240 6% 8%);
    transform: scale(1.005);
  }

  .hero-glyph {
    color: var(--muted);
    width: 64px;
    height: 64px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--surface-2);
    border: 1px solid var(--border);
    margin-bottom: 6px;
    transition: color 200ms, border-color 200ms;
  }
  .hero.hovering .hero-glyph {
    color: var(--accent);
    border-color: hsl(213 94% 68% / 0.4);
  }

  .hero-title {
    margin: 0;
    font-family: var(--font-mono);
    font-size: 28px;
    font-weight: 500;
    letter-spacing: -0.02em;
    line-height: 1.05;
  }
  .hero .btn-primary {
    margin-top: 6px;
    padding: 0 20px;
    min-width: 160px;
  }

  .formats {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
    padding: 4px 14px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 11px;
    color: var(--muted-2);
    letter-spacing: 0.04em;
  }
  .formats .sep { color: var(--border-strong); }

  /* ===== meta footer ===== */
  .meta {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    letter-spacing: 0.04em;
  }
  .meta .sep { color: var(--border-strong); }
  .meta .brand-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 6px hsl(213 94% 68% / 0.6);
    margin-right: 6px;
    vertical-align: middle;
  }
  .link {
    background: none;
    border: 0;
    padding: 0;
    font: inherit;
    color: inherit;
    cursor: pointer;
    text-decoration: none;
    letter-spacing: 0.04em;
    transition: color 120ms;
  }
  .link:hover {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  @media (max-width: 640px) {
    .hero { padding: 36px 24px 28px; }
    .hero-title { font-size: 24px; }
  }
</style>
