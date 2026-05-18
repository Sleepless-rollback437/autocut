<script lang="ts">
  import { editor } from "../lib/store.svelte";

  function bump() {
    editor.scheduleDetect();
  }

  let hasCutlist = $derived(!!editor.cutlist);
  let busy = $derived(editor.jobStatus !== "idle");
</script>

<section class="card">
  <header class="card-head">
    <div>
      <h2>detection</h2>
      <p class="card-sub">silero v5 · 16khz</p>
    </div>
    <div class="card-head-actions">
      {#if hasCutlist}
        <button class="btn btn-ghost btn-sm" onclick={() => editor.resetParams()}>
          reset
        </button>
      {/if}
    </div>
  </header>

  <div class="card-body">
    <button
      class="btn btn-primary btn-block btn-lg"
      onclick={() => editor.runDetectNow()}
      disabled={busy}
    >
      {#if editor.jobStatus === "detecting"}
        analyzing…
      {:else if hasCutlist}
        re-detect
      {:else}
        detect silences
      {/if}
    </button>

    <div class="fields">
      <div class="field">
        <div class="field-label">
          <span>threshold</span>
          <span class="val">{editor.params.threshold.toFixed(2)}</span>
        </div>
        <input
          type="range"
          min="0.1"
          max="0.95"
          step="0.05"
          bind:value={editor.params.threshold}
          oninput={bump}
        />
      </div>

      <div class="field">
        <div class="field-label">
          <span>pad</span>
          <span class="val">{editor.params.pad.toFixed(2)}s</span>
        </div>
        <input
          type="range"
          min="0"
          max="2"
          step="0.05"
          bind:value={editor.params.pad}
          oninput={bump}
        />
      </div>

      <div class="field">
        <div class="field-label">
          <span>min silence</span>
          <span class="val">{editor.params.min_silence_ms}ms</span>
        </div>
        <input
          type="range"
          min="100"
          max="2000"
          step="50"
          bind:value={editor.params.min_silence_ms}
          oninput={bump}
        />
      </div>

      <div class="field">
        <div class="field-label">
          <span>min speech</span>
          <span class="val">{editor.params.min_speech_ms}ms</span>
        </div>
        <input
          type="range"
          min="100"
          max="2000"
          step="50"
          bind:value={editor.params.min_speech_ms}
          oninput={bump}
        />
      </div>
    </div>

    <div class="divider"></div>

    <label class="toggle">
      <input
        type="checkbox"
        bind:checked={editor.usePreviewRange}
        oninput={bump}
      />
      <span>preview range only</span>
    </label>

    {#if editor.usePreviewRange && editor.video}
      <div class="range-grid">
        <div class="field">
          <div class="field-label"><span>start</span></div>
          <input
            type="number"
            min="0"
            max={editor.video.duration}
            step="0.1"
            bind:value={editor.previewRange[0]}
            oninput={bump}
          />
        </div>
        <div class="field">
          <div class="field-label"><span>end</span></div>
          <input
            type="number"
            min="0"
            max={editor.video.duration}
            step="0.1"
            bind:value={editor.previewRange[1]}
            oninput={bump}
          />
        </div>
      </div>
    {/if}

  </div>
</section>

<style>
  .fields {
    display: grid;
    gap: 14px;
    margin-top: 14px;
  }
  .divider {
    border-top: 1px solid var(--border);
    margin: 14px 0;
  }
  .range-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin: 10px 0;
  }
</style>
