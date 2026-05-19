<script lang="ts">
  import Dropzone from "./components/Dropzone.svelte";
  import VideoPlayer from "./components/VideoPlayer.svelte";
  import Timeline from "./components/Timeline.svelte";
  import ParameterPanel from "./components/ParameterPanel.svelte";
  import ExportPanel from "./components/ExportPanel.svelte";
  import ManualCutPanel from "./components/ManualCutPanel.svelte";
  import ResizableSplit from "./components/ResizableSplit.svelte";
  import { editor } from "./lib/store.svelte";

  function basename(path: string): string {
    const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    return slash >= 0 ? path.slice(slash + 1) : path;
  }

  let path = $derived(editor.video?.path ?? editor.pendingPath ?? null);

  // Blur the workspace while the system is busy with something the user
  // shouldn't touch yet (loading metadata, detecting). Export keeps the UI
  // crisp because it has its own progress + cancel affordance.
  let isBusy = $derived(
    editor.jobStatus === "detecting" ||
      (editor.pendingPath !== null && editor.video === null),
  );
</script>

<main class="shell">
  <header class="topbar">
    <span class="brand">
      <span class="brand-dot"></span>
      autocut
    </span>

    {#if path}
      <span class="sep">/</span>
      <span class="filename mono">{basename(path)}</span>

      {#if editor.video}
        <div class="chip-row">
          <span><span class="chip-label">dur</span>{editor.video.duration.toFixed(2)}s</span>
          <span class="sep">·</span>
          <span><span class="chip-label">res</span>{editor.video.width}×{editor.video.height}</span>
          <span class="sep">·</span>
          <span><span class="chip-label">fps</span>{editor.video.fps.toFixed(3)}</span>
          {#if editor.video.start_timecode}
            <span class="sep">·</span>
            <span><span class="chip-label">tc</span>{editor.video.start_timecode}</span>
          {/if}
        </div>
      {:else}
        <span class="mono muted-2">loading metadata…</span>
      {/if}
    {/if}

    <span class="topbar-spacer"></span>
    {#if editor.video || editor.pendingPath}
      <button
        class="cancel-btn"
        onclick={() => editor.closeVideo()}
        title="Close this video and start over"
      >cancel</button>
    {/if}
  </header>

  {#if !editor.video && !editor.pendingPath}
    <section class="stage">
      <Dropzone />
    </section>
  {:else}
    <section class="workspace" class:busy={isBusy}>
      <ResizableSplit
        direction="horizontal"
        initial={0.2}
        min={0.12}
        max={0.32}
        storageKey="ac:split:left-col"
      >
        {#snippet a()}
          <div class="left-stack">
            <ParameterPanel />
            <ExportPanel />
          </div>
        {/snippet}
        {#snippet b()}
          <ResizableSplit
            direction="vertical"
            initial={0.65}
            min={0.3}
            max={0.85}
            storageKey="ac:split:right-rows"
          >
            {#snippet a()}
              <ResizableSplit
                direction="horizontal"
                initial={0.72}
                min={0.4}
                max={0.88}
                storageKey="ac:split:video-cuts"
              >
                {#snippet a()}
                  <div class="player-wrap">
                    <VideoPlayer />
                  </div>
                {/snippet}
                {#snippet b()}
                  <div class="pane-wrap">
                    <ManualCutPanel />
                  </div>
                {/snippet}
              </ResizableSplit>
            {/snippet}
            {#snippet b()}
              <div class="pane-wrap">
                <Timeline />
              </div>
            {/snippet}
          </ResizableSplit>
        {/snippet}
      </ResizableSplit>
    </section>
  {/if}
</main>

<style>
  .shell {
    display: grid;
    grid-template-rows: auto 1fr;
    height: 100vh;
    overflow: hidden;
  }

  .topbar .sep { color: var(--border-strong); font-size: 13px; }
  .topbar .filename { font-size: 13px; color: var(--foreground); }
  .topbar-spacer { flex: 1; }

  .cancel-btn {
    height: 26px;
    padding: 0 12px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--muted);
    cursor: pointer;
    font: inherit;
    font-size: 11px;
    letter-spacing: 0.04em;
    text-transform: lowercase;
    transition: background 120ms, color 120ms, border-color 120ms;
  }
  .cancel-btn:hover {
    background: hsl(0 84% 65% / 0.1);
    border-color: hsl(0 84% 65% / 0.4);
    color: var(--neg);
  }

  .stage {
    display: grid;
    /* Anchor the hero near the top instead of centering it in the entire
       viewport. On tall windows true-centering pushed the footer (brew
       links etc.) so far down that it felt disconnected from the hero. */
    place-items: start center;
    padding: max(56px, 8vh) 32px 32px;
    overflow-y: auto;
    background:
      radial-gradient(circle at 50% 0%, hsl(213 94% 68% / 0.05), transparent 50%),
      radial-gradient(circle at 90% 100%, hsl(280 70% 68% / 0.03), transparent 50%),
      var(--background);
  }

  .workspace {
    min-height: 0;
    overflow: hidden;
    padding: 10px;
    transition: filter 220ms ease, opacity 220ms ease;
  }
  .workspace.busy {
    filter: blur(2px) saturate(0.7) brightness(0.85);
    pointer-events: none;
  }

  .left-stack {
    display: grid;
    grid-template-rows: 1fr auto;
    gap: 12px;
    padding: 6px;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }
  .pane-wrap {
    padding: 6px;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    display: grid;
  }
  .player-wrap {
    padding: 6px;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
    display: grid;
  }
  .player-wrap :global(video) {
    border-radius: var(--radius-lg);
    border: 1px solid var(--border);
  }
</style>
