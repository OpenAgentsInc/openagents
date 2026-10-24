# Browser compositor tracing prototype

This test-only tool adds a source-attributed browser compositor observation. It
does not change the existing GPU-fence gates, measure physical scanout, or qualify
all Aethyrforge workloads. It currently supports the editor's preview worker,
Chrome 155.0.8059.85 on macOS, and one unchanged static view per run.

## Run the probe

Start the selected local editor with `VITE_C_TOWN_BENCHMARK=1`. Use one isolated,
visible Chrome with no other test scene rendering. The usual headless launcher
can collect diagnostics, but headless or unknown browser modes cannot pass the
headed compositor check.

Run `node tools/compositor-probe.mjs` through the isolated browser launcher with:

- `AF_COMPOSITOR_URL`: the selected local editor URL.
- `AF_CAPTURE_DIR`: an evidence directory outside the checkout.
- `AF_TRACE_PROCESSOR`: the official Perfetto `trace_processor_shell` binary.
- `AF_COMPOSITOR_WINDOW_MS`: 500–60000 milliseconds; the diagnostic default is 5000.
- `AF_RUNTIME_WARMUP_MS`: 500–30000 milliseconds; the default is 5000.
- `AF_COMPOSITOR_REPEATS`: 1–8 groups; the default is one ABBA group.
- `AF_COMPOSITOR_VIEWPORT`: the default is `1280x720`.
- `AF_COMPOSITOR_SHOT`: one of the sixteen Medieval Town reference IDs; Town
  defaults to `MT-11`. The probe waits for the successful camera/scene receipt.
- `AF_RUNTIME_TARGET_HZ`: `60` (default) or `display`. Both trace cohorts use the
  same target. Render-attempt samples are distinct from raw and intentionally
  paced callbacks.
- `AF_PERFORMANCE_MODE`: `diagnostic` or `qualification`. Qualification requires
  at least 30 seconds per traced window and two passing traced windows.

`OPENAGENTS_CHROME_PORT` or `OPENAGENTS_CHROME_WS` comes from the launcher.
`AF_COMPOSITOR_INSTRUMENTED=1` enables the common GPU profiler in both cohorts;
its default is off. The tracing order is off/on/on/off, then on/off/off/on. Each
window has its own warmup. No screenshots, video, or JavaScript stack sampling run
during the window. All GPU resources close before Perfetto processing starts.

The tool writes `compositor-probe.json`, bounded raw proto traces, decoded JSON,
and the original application frame samples. Files and helper binaries are hashed.
The report retains browser mode, trace loss, exact order, source/shader identity,
worker-clock boundaries, old fence diagnostics, and separate collection/cleanup
results. A failed collection or cleanup cannot become successful on reanalysis.

To parse retained evidence without opening a browser, set
`AF_COMPOSITOR_REANALYZE` to its report path and choose a new private
`AF_CAPTURE_DIR`. Keep the original files. Missing or ambiguous trace evidence
fails attribution; it never falls back to global display-frame counts.

## Evidence accepted by the parser

1. CDP's DOM backend node and the traced layer's `owner_node` identify the canvas.
   Its current `SurfaceLayer` range identifies one exact `FrameSinkId`. A second
   layer trace must confirm the same process and surface after the windows.
2. `STEP_RECEIVE_COMPOSITOR_FRAME` supplies unique canvas surface-frame IDs.
   `STEP_SURFACE_AGGREGATION` joins these IDs to display-frame IDs.
3. A successful `PipelineReporter` with no missing content supplies the feedback
   endpoint for that display ID. `Display::FrameDisplayed` must confirm it. An
   explicitly ID-linked asynchronous `Graphics.Pipeline.DrawAndSwap` endpoint is
   also supported; nearby CPU swap slices are never accepted as presentation.
4. Only the first display of a fresh canvas update counts. UI-only displays and
   repeated presentations of an old canvas image cannot fill a missed frame.

Strict fresh-frame count, coverage, tail interval, stall, and application-count
checks apply. The one-frame count allowance and two-frame application boundary
allowance remain explicit in the report. Synthetic tests cover stale canvases,
UI-only frames, repeated presentations, missing IDs, lost traces, conflicting
ownership, incomplete feedback, and cleanup failures.

## Observed result on 2026-10-23

The visible `24d` MT-11 run completed all four target-60 windows and closed its
GPU resources. The traced DOM canvas and layer mapped to the same embedded
surface before and after the run. Both explicit worker-clock boundary marks were
present. The two tracing-on windows each recorded 300 accepted application
submissions in about 5 seconds.

This did not establish fresh-image presentation. The offscreen canvas submitted
surface updates without corresponding successful UI `PipelineReporter` records;
the emitted reporters said `STATE_NO_UPDATE_DESIRED` and lacked display IDs. The
asynchronous `DrawAndSwap` spans carried opaque track IDs rather than explicit
display IDs. The current parser cannot join these spans to each fresh canvas
image, so the compositor verdict remains unavailable and failed. Global display
counts cannot substitute for this missing link.

The retained traces also contained a rising service-wide discarded-chunk counter.
CDP and per-buffer loss fields reported no loss, but the prototype retains its
conservative loss rejection. The 16 MiB CSV-field bound now permits the real layer
snapshot while the existing total-output and row limits remain enforced. The
original evidence and failed verdict remain available for further analysis.

## Source contract and limitations

The implementation uses the exact Chromium tag rather than old frame-viewer
examples. Chrome's macOS feedback timestamp is derived from CVDisplayLink and a
latch heuristic. It is a browser display-time estimate, not a measurement of the
screen's physical scanout. Queue completion remains useful as a separate latency
and GPU backlog diagnostic.

- [CDP tracing stream, format, and loss fields](https://chromedevtools.github.io/devtools-protocol/tot/Tracing/)
- [Exact display feedback and FrameDisplayed events](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/components/viz/service/display/display.cc)
- [Frame reporter state, IDs, and feedback endpoint](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/cc/metrics/compositor_frame_reporter.cc)
- [Canvas surface-frame receive IDs](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/components/viz/service/frame_sinks/compositor_frame_sink_support.cc)
- [Surface-to-display aggregation IDs](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/components/viz/service/display/surface_aggregator.cc)
- [DOM layer owner](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/cc/layers/layer_impl.cc) and [surface range](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/cc/layers/surface_layer_impl.cc)
- [macOS display-time prediction](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/gpu/ipc/service/image_transport_surface_overlay_mac.mm) and [feedback construction](https://chromium.googlesource.com/chromium/src/+/refs/tags/155.0.8059.85/ui/accelerated_widget_mac/ca_layer_tree_coordinator.mm)
- [Official Perfetto trace processor](https://perfetto.dev/docs/quickstart/trace-analysis) and [Chrome graphics pipeline tables](https://perfetto.dev/docs/analysis/stdlib-docs)
- [WebGPU queue completion contract](https://gpuweb.github.io/gpuweb/#dom-gpuqueue-onsubmittedworkdone)

These joins are a prototype until real retained traces demonstrate that this
Chrome build emits every required field for the offscreen WebGPU canvas. Missing
fields remain unavailable. Headed runs must still cover the complete workload
matrix before a product-level pacing claim is justified.
