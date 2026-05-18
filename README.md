# autocut

A small desktop app for removing silent gaps from videos. Drop a video in,
tweak a few sliders, refine the cuts, then export an MP4 or an FCPXML for
DaVinci Resolve / Premiere Pro.

Built with Rust (Tauri 2) + Svelte 5. Cross-platform: macOS, Linux, Windows.

![autocut screenshot](docs/screenshot.jpeg)

## What it does

- Drag-and-drop a video file (or pick one) - preview opens instantly
- Detect speech with silero-vad (bundled ONNX, no Python)
- Two-tier timeline with audio waveform: zoomable detail bar over a navigator
- Live-tune `threshold`, `pad`, `min silence`, `min speech` (debounced rerun)
- Manual cut editing: drag the green edge handles on the timeline, edit
  in/out timestamps in the cuts panel, or toggle cuts off temporarily
  (purple) without losing them until export
- Bidirectional hover sync between the timeline and the cuts list
- Preview window: process only `[t_start, t_end]` while iterating on a long
  video, then export the full thing
- Playback skips removed segments automatically (toggleable). Spacebar
  play/pause, transport buttons jump between cuts
- Resizable workspace with persistent split positions
- Export:
  - **MP4** via ffmpeg `filter_complex select` (libx264 + aac, re-encode)
  - **FCPXML 1.11** with source-timecode alignment so DaVinci/Premiere will
    auto-link the clip (no "media offline" relink dialog)

## Quick start (dev)

Requirements: Rust 1.77+, Node 20+, pnpm. `curl`, `tar`, and `unzip` for the
first build (used by `build.rs` to fetch ffmpeg).

```sh
pnpm install
pnpm tauri dev
```

The first build downloads ffmpeg + ffprobe (~150 MB combined) into
`src-tauri/binaries/`. Set `AUTOCUT_SKIP_FFMPEG_DOWNLOAD=1` to skip (e.g. for
offline cargo check; the bundle phase will then fail loudly).

## Production build

```sh
pnpm tauri build
```

Output sits under `src-tauri/target/release/bundle/`:
- macOS: `.app` and `.dmg`
- Linux: `.AppImage` and `.deb`
- Windows: `.msi` and `.exe`

## Layout

```
src-tauri/                  Rust + Tauri backend
  build.rs                  Downloads ffmpeg/ffprobe at build time
  binaries/                 ffmpeg-<triple>, ffprobe-<triple> (gitignored)
  src/
    cutlist.rs              Cut, CutList, pad-and-merge inversion
    timecode.rs             SMPTE parse, drop-frame, FCPXML rational render
    probe.rs                ffprobe wrapper (duration, fps, source TC)
    audio.rs                ffmpeg -> 16kHz f32 PCM in memory
    vad.rs                  silero-vad wrapper, hysteresis, segment grouping
    waveform.rs             Downsampled amplitude envelope
    export_fcpxml.rs        FCPXML 1.11 emitter
    export_mp4.rs           ffmpeg filter_complex select + progress
    commands.rs             Tauri command surface
    binaries.rs             Sidecar path resolver (dev vs bundled)

src/                        Svelte 5 frontend
  App.svelte                Topbar + resizable workspace
  components/
    Dropzone.svelte         Empty-state hero with drag/drop target
    VideoPlayer.svelte      <video> + skip-removed playback, space toggle
    Timeline.svelte         Waveform, two-tier zoom, edge-drag cuts
    ManualCutPanel.svelte   Inline-editable keep intervals
    ParameterPanel.svelte   Custom Slider component, debounced auto-rerun
    ExportPanel.svelte
    Slider.svelte           Custom horizontal slider with shift-fine, kbd
    ResizableSplit.svelte   Reusable horizontal/vertical split with persist
  lib/
    api.ts                  invoke() wrappers
    store.svelte.ts         Runes-based editor store
    types.ts                Mirrors Rust types
```

## Notes

- The FCPXML exporter intentionally hand-writes XML rather than using
  opentimelineio. Resolve binds an asset to its source media via THREE checks
  (path, embedded source TC, frame-rate format). If the MP4 carries
  `15:33:27;24` but the FCPXML says the asset starts at `0s`, the relink
  dialog fires even when the path is correct. We carry the source TC through.
- silero VAD runs with hysteresis (start at `threshold`, continue while
  above `threshold - 0.15`). Without it, marginal-probability frames in the
  middle of an utterance flicker on/off and produce false silences.
- Lossless smart-cut is deferred. The MP4 path re-encodes; for serious editing
  use the FCPXML hand-off.
- Single concurrent export. Cancellation kills the ffmpeg child process.

## Testing

```sh
cargo test --manifest-path src-tauri/Cargo.toml
pnpm check
```

There's also a CLI smoke harness for the end-to-end pipeline:

```sh
cargo run --manifest-path src-tauri/Cargo.toml --bin smoke --release -- \
  /path/to/video.mp4
```
