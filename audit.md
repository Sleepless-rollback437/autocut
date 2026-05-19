# autocut audit notes

Snapshot taken on `0187cab` (v0.4.1). This file is a handoff for a reviewer
agent that will go in cold. Findings are grouped by impact, each with file
references in the form `path:line`. Tone is intentionally direct - this is
a list of things worth fixing, not a marketing document.

---

## 1. Critical: UI-lock and data-loss risks

### 1.1 Skip-jump stutter is inherent to single-`<video>` architecture
`src/components/VideoPlayer.svelte:81-145`

Setting `videoEl.currentTime = X` triggers a WebKit decoder seek that holds
the last decoded frame for 50-200ms until the new keyframe is decoded
forward. The current 80ms pre-roll lookahead (`SKIP_LOOKAHEAD = 0.08`)
narrows the gap but does not eliminate it. The user has reported this as a
visible "pause" between cuts. Real fixes:

- **Two stacked `<video>` elements** with crossfade-on-cut. Element A plays
  the current keep; element B pre-seeks to the next keep's start and
  decodes the first frame in the background; at the boundary swap opacity.
- **MediaSource Extensions (MSE)** stitched server-side or in-browser.
  Complex.
- **Pre-rendered preview**: write a temp MP4 via ffmpeg, play it back as a
  regular video. Costs disk + a 5-30s render but playback is truly seamless.

The two-video approach is the smallest delta; the rest of the pipeline does
not need to change.

### 1.2 `untrack()` in Timeline.svelte is a fragile load-bearing call
`src/components/Timeline.svelte:75-90`

The explicit-seek auto-pan effect tracks `editor.seekToken` only and reads
`viewStart`/`viewEnd` inside `untrack()`. Removing `untrack()` (e.g. in a
refactor or migration) silently reintroduces the v0.4.0 snap-back-during-pan
bug because Svelte 5 effects re-fire on any tracked dep change, including
the very fields the effect writes to. There is no test guarding this. Add a
playback regression test, or restructure so the dep graph naturally
prevents the loop (e.g. an imperative call from the seek code path instead
of a reactive effect).

### 1.3 Detection has no cancellation token
`src-tauri/src/commands.rs:130-152`, `src/lib/store.svelte.ts:198-225`

`detect_silence` runs on a Tauri blocking thread. The frontend debounces
slider changes by 300ms via `scheduleDetect`, but if a detect is already
in-flight when a new one is requested, the in-flight job runs to completion
even though its result is discarded. For a 60-minute video this wastes
~10-20 seconds of CPU per superseded request and stacks pressure on the
audio pipeline. Wire a `cancel: Arc<AtomicBool>` into the VAD loop similar
to `export_mp4::export`, expose it as `cancel_detect`, and call it from
`scheduleDetect` before re-arming. Same pattern applies to
`compute_waveform`.

### 1.4 Waveform extraction is uncancellable and races with `closeVideo()`
`src/lib/store.svelte.ts:127-145`

`fetchWaveform` is fire-and-forget. The result is guarded by a
`this.video?.path === path` check, so a stale result is dropped, but the
underlying ffmpeg subprocess keeps running until it finishes decoding the
entire audio track. If the user closes a 1-hour drone video to load a
different one, the previous waveform job continues consuming CPU for tens
of seconds. Add cancellation as in 1.3.

### 1.5 `editor.error` is a single string shared by every flow
`src/lib/store.svelte.ts:71`

Detection error, export error, load error, and waveform error all write to
the same field. If a detect fails and then an export fails, only the export
message is visible. ExportPanel renders the error UI any time `editor.error`
is non-empty, including for errors from a totally unrelated flow. Split
into `errors: { detect, export, load, waveform }` or attach a `source`
discriminator and route to the matching component.

### 1.6 `closeVideo()` does not wait for in-flight work
`src/lib/store.svelte.ts:296-310`

Sets `video = null` immediately. If a detection or export is mid-flight,
their callbacks still mutate `cutlist`, `error`, `exportProgress` etc.
afterwards. The race guard on waveform (1.4) is the only one in the file.
Either await/cancel each in-flight job in `closeVideo` or carry a
generation token (like `videoLoadId`) and have every callback abort if it
no longer matches.

### 1.7 No backpressure when user clicks Detect during an in-flight detect
`src/lib/store.svelte.ts:206-212`

`runDetectNow` checks `if (this.jobStatus === "exporting") return;` but
silently returns if status is already `"detecting"`. The click is dropped
with no UI feedback ("you're already detecting, please wait"). User can
spam the button and assume nothing is happening.

---

## 2. Logic bugs

### 2.1 `prevKeep()` traversal is brittle when playhead is inside a remove
`src/lib/store.svelte.ts:148-168`

The 1.0s "restart current keep vs go to previous" threshold is measured
relative to `keeps[curr].start`, where `curr` is the latest keep whose
`start <= t`. If `t` is inside a remove interval (between keeps), `curr`
points to the keep BEFORE the remove, and the calculation `t - keeps[curr].start`
can exceed the keep's own duration. Result: pressing Previous from inside a
remove sometimes jumps to "current" (which is actually behind the playhead)
instead of the obviously-previous keep. Reproduce: pause inside a long
remove segment, press the prev-keep button.

### 2.2 `nextKeep()` clamps to last keep's start at end of timeline
`src/lib/store.svelte.ts:170-181`

If currently inside the last keep and the user presses Next, the function
jumps to `keeps[keeps.length - 1].start` - the start of the same keep. That
restarts playback from the keep's beginning, which is surprising. Should
either no-op, jump to source duration, or seek to the keep's end.

### 2.3 `setKeepIntervals` merges disabled state aggressively
`src/lib/store.svelte.ts:330-348`

When two overlapping keeps merge, the resulting interval is marked
`disabled = last.disabled || k.disabled`. That means if one of the merged
keeps was disabled, the merged interval inherits disabled even though the
other was active. Manual cut edits that produce overlap (rare but possible
during drag) will silently disable previously-active content.

### 2.4 `resetParams()` runs detection unconditionally
`src/lib/store.svelte.ts:316-319`

```ts
resetParams() {
  this.params = { ...DEFAULTS };
  this.runDetectNow();
}
```

If the user hits "defaults" before ever running detect, this kicks off a
detect they didn't ask for. Should mirror `scheduleDetect`'s
"only after first manual detect" rule:

```ts
if (this.cutlist) this.runDetectNow();
```

### 2.5 `exportFcpxml` does not clear `lastExport` at start
`src/lib/store.svelte.ts:259-281`

`exportMp4` sets `lastExport = null` before running so the success row
clears. `exportFcpxml` does not. If a user exports MP4 successfully, then
exports FCPXML and it fails, the success row from the MP4 lingers and
might mislead the user about which file landed where.

### 2.6 `pendingPath` cleanup does not race-guard `loadVideo` reentrancy
`src/lib/store.svelte.ts:102-128`

If the user drops video A and then drops video B before `openVideo(A)` has
returned, `pendingPath = B`, then `openVideo(A)` succeeds and sets
`video = info_for_A`, but pendingPath now points to B. The video element
ends up playing A while metadata thinks B was loaded. Add a `loadId`
counter and check it before assigning.

### 2.7 `findUtteranceAt`-style overlap rules are not applied at cutlist
boundaries

The cutlist normalization (`store.svelte.ts:404-425`) collapses adjacent
removes with `Math.abs(last.end - next.start) < 1e-6`. That tolerance is
fine for clean detection output but not for hand-edited intervals where
users type "12.34" / "12.35" - a 10ms remove will not collapse with its
neighbor and become a 10ms ffmpeg `between(t,...)` filter atom. Output is
correct but the filter complexity grows unnecessarily.

### 2.8 `private progressUnlisten` is dead state
`src/lib/store.svelte.ts:86, 240, 251`

The field is assigned and never read elsewhere. The actual unlisten
function is captured in a local `const unlisten` inside `exportMp4` and
called in `finally`. Remove the field.

---

## 3. Performance hotspots

### 3.1 Audio PCM is extracted twice per video
`src-tauri/src/audio.rs:14-59`, called from both `detect_silence` and
`compute_waveform`.

The same audio stream is re-decoded by two separate ffmpeg subprocesses
each time a video is opened. For a multi-GB 4K clip this is 5-30 seconds
of redundant ffmpeg startup + decode. Cache the f32 PCM in `AppState`
keyed by `(path, mtime)`; both detection and waveform consume it. Memory
budget is ~10MB per 10 minutes at 16kHz mono.

### 3.2 Detection re-decodes full PCM on every slider tweak
`src-tauri/src/commands.rs:130-152`

Each rerun of `detect_silence` calls `extract_pcm` again, re-decoding the
audio. After the first run, only the VAD parameters change - the PCM is
identical. Cache as in 3.1 and detection becomes essentially free
(<100ms for VAD on cached samples).

### 3.3 `requestVideoFrameCallback` tick scans intervals linearly
`src/components/VideoPlayer.svelte:90-103`

At 60fps the tick fires 60×/sec; each tick scans all cutlist intervals to
find the relevant remove. For 1000+ interval videos this is 60k iterations
per second. Pre-compute "next omitted boundary after t" via binary search
or a cursor maintained across ticks. Practically not a bottleneck for
typical inputs but a sharp edge.

### 3.4 `buildWavePath` rebuilds on every viewStart/viewEnd change
`src/components/Timeline.svelte:308-328`

After PR #2 the bin count is capped at 2000 points, so this is acceptable.
But `$derived.by` re-runs whenever any dep changes, including
viewStart/viewEnd, currentTime indirectly via auto-pan, etc. Profile a
60-minute video to confirm the cap is enough, otherwise memoize on
`(viewStart, viewEnd, waveform)` triple.

### 3.5 `extract_pcm` reads stdout fully into memory
`src-tauri/src/audio.rs:46`

A 1-hour 16kHz mono PCM is ~115MB. Currently allocated as one `Vec<u8>`
plus the converted `Vec<f32>` (~460MB). For very long videos this is
memory pressure that could OOM on 8GB machines. Stream into a
pre-allocated buffer sized from the probe-known duration.

### 3.6 `nav-waveform` SVG path duplicates the bar's waveform calculation
`src/components/Timeline.svelte:344-352`

`navWavePath` is a separate `$derived.by` that always renders the full
waveform at the same downsampled resolution. It changes only when the
waveform itself changes (rare). Memoize once on `editor.waveform`.

### 3.7 Tauri `asset://` protocol may not support efficient byte-range seeks
`src/components/VideoPlayer.svelte:7`, `convertFileSrc(path)`

When the `<video>` element seeks, WebKit issues range requests against the
asset URL. Tauri 2's custom asset protocol handler implementation
determines whether ranges are honored efficiently. Compare seek behavior
between autocut (asset://) and video-edit (HTTP byte-range via FastAPI) -
if the latter is materially faster, consider implementing a local HTTP
server or wiring Tauri's `streamProtocol` API with explicit Range support.

---

## 4. Workarounds and tech debt

### 4.1 `pnpm-workspace.yaml` with `packages: []` is purely for CI
`pnpm-workspace.yaml:10`

Not a real workspace. The empty list satisfies `pnpm/action-setup`'s
`pnpm store path` invocation. Document or migrate to a project-level
`.pnpmrc` if pnpm publishes one for non-workspace settings.

### 4.2 `pnpm.onlyBuiltDependencies` in package.json is redundant
`package.json:24-30`

Duplicates the entry in `pnpm-workspace.yaml`. PR #2 kept it for pnpm 10
backward compat. Drop it once pnpm 10 isn't a target.

### 4.3 ResizeObserver hack for container queries
`src/components/ManualCutPanel.svelte:16-25`

Comment notes Chromium webview's first-paint has unset width so container
queries break. As WebView2/WKWebView mature this becomes redundant. Watch
release notes; remove the JS path when the native CSS path is reliable.

### 4.4 Sidecar binary resolution checks 5 paths per command
`src-tauri/src/binaries.rs:54-97`

macOS Tauri places sidecars in `Contents/MacOS` rather than
`Contents/Resources`, so the resource-dir lookup misses them. The fix is a
5-candidate priority list checked on every command (`ffmpeg_path`,
`ffprobe_path`). Cache the resolved path in `AppState` after first
successful resolve.

### 4.5 `SKIP_LOOKAHEAD = 0.08` is empirically tuned
`src/components/VideoPlayer.svelte:75`

Magic constant with no test. If WebKit changes its decoder/seek pipeline,
this value silently degrades skip quality. Make it adaptive (measure
seek-to-frame latency on first seek and adjust) or at least gate behind a
runtime feature detect.

### 4.6 `lastSkipTarget` reset tied to `seeked` event timing
`src/components/VideoPlayer.svelte:117, 132-135`

The guard prevents re-firing the same skip. Relies on `seeked` firing
reliably. If a user manually drags the native video control's scrubber to
mid-remove, the `seeked` event resets the guard, then the next tick
performs the skip, then resets again - oscillation possible at boundaries.

### 4.7 FCPXML file URI encoding is hand-rolled
`src-tauri/src/export_fcpxml.rs:89-112`

Custom byte-level percent-encoder instead of `url::Url` or `urlencoding`
crate. Works but reinvents a well-tested wheel. Adds maintenance surface.

### 4.8 `current_target_triple()` empty-string fallback
`src-tauri/src/binaries.rs:46`

```rust
} else {
    ""
}
```

If a platform doesn't match any branch, the fallback produces broken paths
like `ffmpeg-`. Promote to a compile_error! so unsupported platforms fail
at build time, not at runtime with a confusing "could not locate ffmpeg"
message.

### 4.9 Auto-generated `pnpm-workspace.yaml` stub from `pnpm install`
The user's `pnpm install` sometimes regenerates the file with an
`allowBuilds` stub at the top. Manual `git checkout` is required. Either
pin pnpm to a version that doesn't do this or document the workaround in
DEVELOPING.md.

---

## 5. Dead, redundant, or unreachable code

### 5.1 `FcpxmlParams.asset_path: Option<&Path>` is always `Some(...)` in production
`src-tauri/src/export_fcpxml.rs:21, 42`

Default `"source.mp4"` branch is never hit because `commands::export_fcpxml`
always sets `asset_path` (commands.rs:268). Either drop the Option and make
it required, or document the test-only purpose.

### 5.2 `ms_to_chunks` `.max(0.0)` on non-negative input
`src-tauri/src/vad.rs:88`

The expression `((ms as f64 / 1000.0) / CHUNK_SECONDS).ceil()` can't be
negative for `u32 ms`. Drop the `max(0.0)`.

### 5.3 `editor.error` is checked in both ExportPanel and Dropzone
`src/components/Dropzone.svelte:125`, `src/components/ExportPanel.svelte:172`

Both branches assume the error originates in their respective flow. Per
1.5, they actually share state.

### 5.4 `closeVideo` does not reset `exportOptions`
`src/lib/store.svelte.ts:296-310`

When the user closes one video and loads another, their previous
quality/resolution choice persists - probably desired behavior, worth
verifying as a product decision.

### 5.5 `editor.lastExport = null` clear pattern is duplicated
`src/lib/store.svelte.ts:240, 267 (after fix in 2.5)`, plus the close
handler. Extract a method or accept the duplication.

### 5.6 `Cargo.lock.bak` in `.gitignore`
`.gitignore:4`

Was this ever generated? grep'ing the repo finds no producer. Safe to drop
the rule if no longer relevant.

### 5.7 `tauri-plugin-shell` is used for one operation
`src-tauri/Cargo.toml:21`, `src/components/Dropzone.svelte:11`

Only for `plugin:shell|open` on the github link. The plugin's other surface
is dead. Could be replaced with `tauri::webview::WebviewWindow::eval` or
`tauri-plugin-opener` if the shell plugin's bundle weight matters.

---

## 6. UX and improvement opportunities

### 6.1 No keyboard shortcuts beyond Spacebar
- Arrow keys for frame stepping (currently no-op on video)
- `J/K/L` for industry-standard transport
- `,`/`.` for prev/next cut
- `Esc` to cancel an export
- `Cmd-Z` for cut-edit undo (huge gap)

### 6.2 No undo/redo for manual edits
Disabling a keep, dragging edge handles, adding cuts via the `+` button -
none are reversible. A simple linear undo stack on `cutlist` mutations
would handle 90% of "oops" cases.

### 6.3 No detection progress
The Detect button shows "analyzing…" for the full 5-30s on long videos
with no progress indication. Wire up a progress event from Rust:
percentage = (samples_consumed / total_samples).

### 6.4 No waveform progress
Same problem - the waveform appears suddenly mid-session. A loading state
on the timeline would set expectations.

### 6.5 No "merge adjacent keeps" affordance
User can only adjust in/out points. Merging two keeps requires dragging
the right edge of one over the next, which is fiddly. Add a context menu
or a "merge" button.

### 6.6 Export cancel button is inside ExportPanel
If the user scrolls the left column, the cancel button can be off-screen
during a long export. Move to a globally visible position, or display a
modal during export.

### 6.7 Cuts panel auto-scroll on click changed to instant (PR #2)
Verify with the user that the loss of smooth scroll is acceptable. The PR
made this change for Windows perf, but on macOS the smooth scroll worked
fine.

### 6.8 No "open output folder" affordance until after export
`reveal_in_finder` is only surfaced via the post-export success row. Add a
shortcut or menu item that opens the output dir directly.

### 6.9 Diagnostic info copy is minimal
`src/components/ExportPanel.svelte:42-69`, `src/components/Dropzone.svelte:21-49`

Includes app version, OS, sidecar paths. Missing: video duration, fps,
cutlist length, kept duration, current export settings. Pad it out so a
single paste is enough to debug remotely.

### 6.10 No internationalization
All strings in English. Project author is Turkish; adding a translation
layer is straightforward if multi-language is a future goal.

### 6.11 `detection params` slider tooltip says "shift for fine adjustment"
`src/components/ParameterPanel.svelte:80-83`

Only documented in a tiny tip. Users don't read tips. Consider a "fine"
toggle next to each slider, or surface the step value visibly.

---

## 7. Security and privacy

### 7.1 `reveal_in_finder` validates only existence
`src-tauri/src/commands.rs:227-256`

Accepts arbitrary path strings from the frontend, opens whatever is
there. Frontend currently only passes export paths but a future caller
could pass anything. Threat is low (user-initiated) but tighten to known
output dirs or a registered allowlist.

### 7.2 Diagnostic info includes filesystem paths
`src-tauri/src/commands.rs:96-108`, surfaced via copy-details

ffmpeg/ffprobe paths leak `/Users/<username>/...` or the equivalent on
other OSes. For a bug-report flow this is probably acceptable, but users
should know what they're sharing. Add a "redact username" toggle or note
the contents in the UI before copying.

### 7.3 No CSP set
`src-tauri/tauri.conf.json:16-19`

`"csp": null` is explicitly set. For an offline-only app this is mostly
fine but eliminates a defense layer. If the app ever loads remote
resources (analytics, autoupdate, telemetry) it becomes a real risk.

### 7.4 `assetProtocol.scope: ["**/*"]`
`src-tauri/tauri.conf.json:18`

The asset protocol is open to every path. Allows loading any file the
process can read via `convertFileSrc`. For a single-user desktop app fine,
but tighten to user-selected files only if multi-tenant ever becomes a
concern.

---

## 8. Testing gaps

### 8.1 No Svelte component tests
Rust has 24 unit tests across `cutlist`, `vad`, `timecode`, `export_mp4`,
`export_fcpxml`. Frontend has zero. Critical reactive flows -
auto-pan-on-play, skip-jump, cutlist normalization on export - are
unverified.

### 8.2 No integration test that exercises the full pipeline
Probe -> PCM -> VAD -> CutList -> ffmpeg cut export. Could be done with a
short fixture video; would catch the kind of cross-module regression that
unit tests miss.

### 8.3 No CI for macOS
Only Windows has a build workflow. A `tauri build` on macOS in CI would
have caught the icon RGBA check earlier in this session.

### 8.4 No tests for skip-jump regression
The pre-roll lookahead constant and the lastSkipTarget guard have no
tests. Easy to break by accident.

---

## 9. Quick-win list (in order of effort)

1. Drop dead `progressUnlisten` field (1 line).
2. Fix `resetParams` to no-op when no cutlist yet (2.4, 2 lines).
3. Clear `lastExport` in `exportFcpxml` (2.5, 1 line).
4. Fix `nextKeep`/`prevKeep` edge cases (2.1, 2.2).
5. Cache PCM extraction across detect + waveform (3.1, 3.2). Single biggest
   perf win.
6. Add `cancel_detect` + `cancel_waveform` commands (1.3, 1.4).
7. Split `editor.error` per flow (1.5).
8. Add a `loadId` race guard (1.6, 2.6).
9. Skim 6.x for low-effort UX touches.
10. Investigate the asset:// vs HTTP byte-range hypothesis for skip-jump
    (3.7, 1.1).

---

## 10. Out-of-scope for this audit

- Notarization / signing for macOS Gatekeeper
- Apple developer ID setup
- Windows code signing for SmartScreen
- Autoupdate plumbing (Tauri has built-in support; not wired)
- Telemetry / crash reporting (intentionally absent for privacy)

These are product decisions, not bugs.

---

## Appendix: file map

```
src-tauri/src/
  audio.rs           PCM extraction via ffmpeg
  binaries.rs        sidecar binary resolution
  commands.rs        Tauri command surface (frontend RPC)
  cutlist.rs         Cut/CutList data model + VAD -> intervals
  export_fcpxml.rs   FCPXML 1.11 renderer for DaVinci/Premiere
  export_mp4.rs      ffmpeg select-filter-based MP4 cut
  lib.rs             Tauri app entrypoint
  main.rs            cargo main
  probe.rs           ffprobe wrapper -> VideoInfo
  timecode.rs        SMPTE timecode parsing + FCPXML rational format
  vad.rs             Silero VAD wrapper
  waveform.rs        PCM -> downsampled amplitude bins

src/components/
  Dropzone.svelte         file-pick + drag-drop landing
  ExportPanel.svelte      quality/resolution presets + export buttons
  ManualCutPanel.svelte   keep-interval list with toggles
  ParameterPanel.svelte   detection sliders + Detect button
  ResizableSplit.svelte   draggable split panes
  Slider.svelte           keyboard-accessible custom slider
  Timeline.svelte         waveform + cut overlay + transport + zoom
  VideoPlayer.svelte      HTML5 video + skip-jump logic

src/lib/
  api.ts                  Tauri invoke wrappers
  store.svelte.ts         EditorStore singleton (svelte 5 $state)
  types.ts                Cut, CutList, DetectParams, ExportOptions
```
