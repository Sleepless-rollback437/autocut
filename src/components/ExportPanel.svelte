<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { editor } from "../lib/store.svelte";

  function suggestedName(ext: string): string {
    if (!editor.video) return `untitled_autocut.${ext}`;
    const path = editor.video.path;
    const slash = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
    const dot = path.lastIndexOf(".");
    const stem = path.slice(slash + 1, dot > slash ? dot : undefined);
    return `${stem}_autocut.${ext}`;
  }

  async function exportMp4() {
    const out = await save({
      defaultPath: suggestedName("mp4"),
      filters: [{ name: "MP4", extensions: ["mp4"] }],
    });
    if (out) editor.exportMp4(out);
  }

  async function exportFcpxml() {
    const out = await save({
      defaultPath: suggestedName("fcpxml"),
      filters: [{ name: "FCPXML", extensions: ["fcpxml"] }],
    });
    if (!out || !editor.video) return;
    const title = suggestedName("").replace(/\.$/, "");
    await editor.exportFcpxml(out, title);
  }

  let canExport = $derived(!!editor.cutlist && editor.jobStatus === "idle");
  // Disabled keeps are excluded from the export, so don't count them.
  let keptDuration = $derived(
    editor.cutlist
      ? editor.cutlist.intervals
          .filter((c) => c.kind === "keep" && !c.disabled)
          .reduce((a, c) => a + (c.end - c.start), 0)
      : 0,
  );
  let totalDuration = $derived(editor.video?.duration ?? 0);
  let keptPct = $derived(totalDuration > 0 ? (keptDuration / totalDuration) * 100 : 0);
</script>

<section class="card">
  <header class="card-head">
    <div>
      <h2>export</h2>
      <p class="card-sub">
        {#if editor.cutlist}
          <span class="mono">{keptDuration.toFixed(1)}s</span> kept
          <span class="muted-2">·</span>
          <span class="mono">{keptPct.toFixed(0)}%</span>
        {:else}
          run detection first
        {/if}
      </p>
    </div>
  </header>

  <div class="card-body">
    <div class="export-row">
      <button class="btn btn-primary" onclick={exportMp4} disabled={!canExport}>
        <span class="mono">▸</span> mp4
      </button>
      <button class="btn" onclick={exportFcpxml} disabled={!canExport}>
        <span class="mono">▸</span> fcpxml
      </button>
    </div>

    <p class="hint muted-2">
      mp4 re-encodes via ffmpeg · fcpxml preserves source timecode for davinci & premiere
    </p>

    {#if editor.jobStatus === "exporting" && editor.exportProgress}
      <div class="progress">
        <div class="progress-head">
          <span class="mono muted">{editor.exportProgress.message}</span>
          <span class="mono">{editor.exportProgress.pct.toFixed(0)}%</span>
        </div>
        <div class="bar">
          <div class="fill" style:width="{editor.exportProgress.pct}%"></div>
        </div>
        <button class="btn btn-danger btn-sm" onclick={() => editor.cancelExport()}>
          cancel
        </button>
      </div>
    {/if}

    {#if editor.error}
      <div class="error">
        <span class="dot dot-neg"></span>
        <span class="mono">{editor.error}</span>
      </div>
    {/if}
  </div>
</section>

<style>
  .export-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .hint {
    margin: 12px 0 0;
    font-size: 11px;
    line-height: 1.5;
  }
  .progress {
    margin-top: 14px;
    display: grid;
    gap: 6px;
  }
  .progress-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    font-size: 11px;
  }
  .progress-head .mono.muted {
    color: var(--muted-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 70%;
  }
  .progress-head .mono:last-child {
    color: var(--foreground);
  }
  .bar {
    height: 4px;
    background: var(--input);
    border-radius: 999px;
    overflow: hidden;
    border: 1px solid var(--border);
  }
  .fill {
    height: 100%;
    background: var(--accent);
    transition: width 240ms ease-out;
  }
  .progress .btn-danger {
    margin-top: 4px;
    justify-self: start;
  }
  .error {
    margin-top: 12px;
    display: flex;
    gap: 8px;
    align-items: flex-start;
    padding: 8px 10px;
    background: hsl(0 84% 65% / 0.06);
    border: 1px solid hsl(0 84% 65% / 0.25);
    border-radius: var(--radius-sm);
    font-size: 11px;
    color: var(--neg);
    word-break: break-word;
  }
  .error .dot { margin-top: 5px; flex-shrink: 0; }
</style>
