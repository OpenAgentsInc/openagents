# Render lab

Date: 2026-10-24, updated 2026-10-26 (#12600: window presentation, Dawn,
heavy scene). Status: lab results for decisions
[0006](../decisions-history/0006-cross-platform-engine-core.md),
[0007](../decisions-history/0007-gpu-driven-rendering.md), and
[0008](../decisions-history/0008-shader-toolchain.md). Nothing here changes a decision.

The render lab (`crates/render-lab`) is one Rust renderer over the
`webgpu.h`-shaped `af-gpu` crate (`crates/gpu`). It switches at runtime between
three visibility paths (CPU-batched draws, GPU culling with indirect draws, and
a visibility buffer), three anti-aliasing modes (none, 4x MSAA, TAA), occlusion
culling, LOD, and crowd tiers, and reports one `Stats` document per frame
(`crates/render-lab/src/lab.rs`). The browser build runs it at
`render-lab.html` (`apps/render-lab`).

Labels used below:

- **Measured** is a number from a recorded run on the named machine.
- **Inferred** is a conclusion drawn from measured numbers or code without a
  run that isolates it.

## Native backend

`af-gpu`'s `native` feature implements the whole `af-gpu` surface on a native
`webgpu.h` (`crates/gpu/src/native.rs`), with bindings generated from the
implementation's header by `crates/gpu/build.rs`. Both wgpu-native and Dawn
are linked and run (see [Dawn](#dawn)); the Dawn differences (device polling,
instance setup) sit behind the `af_dawn` cfg.

- Without a window, frames render into an offscreen texture
  (`Gpu::surface_texture`, `Lab::capture`). With a window (`SurfaceSource`:
  Metal layer, Xlib, Wayland, or Win32) frames render into the surface's
  texture and are presented (see [Window presentation](#window-presentation)).
  `NativeOptions::present_mode` and `Gpu::set_present_mode` pick `fifo`,
  `mailbox`, or `immediate`, falling back to `fifo` where the surface lacks
  the mode.
- wgpu-native is created with automatic timestamp normalization, so resolved
  timestamps are nanoseconds as in browsers and Dawn.
- `AF_WGPU_LOG=warn|info|debug|trace` forwards wgpu-native's log to stderr,
  and with either implementation prints uncaptured device errors.
  `AF_GPU_DISABLE_FEATURES=timestamp-query,...` hides optional features from
  the device for A/B runs.
- The C ABI (`crates/render-lab/src/capi.rs`) exposes
  `af_lab_native_{create,create_window,configure,resize,orbit,frame,stats,wait_idle,capture,destroy,last_error}`.
  `frame` returns the same stats JSON the web lab reports. `create_window`
  takes the host's window handles (Metal layer, Xlib, Wayland, or Win32) and
  a `present-mode` in its options JSON.

Build and run (the script downloads the pinned wgpu-native v29.0.1.1 release and
checks its SHA-256, or builds a checkout with `--source`):

```sh
eval "$(tools/build-webgpu-native.sh --out ../webgpu-native)"   # or: eval "$(tools/fetch-dawn.sh --out ../webgpu-dawn)"
cargo build --release -p af-render-lab --features native --bin render-lab-native
target/release/render-lab-native --width 1920 --height 1080 --frames 300 --warmup 60 \
  --suite compare|heavy|all [--backend vulkan|metal] [--capture DIR] > results.jsonl
# In a window (feature `window`: winit, plus a CAMetalLayer on macOS):
cargo build --release -p af-render-lab --features window --bin render-lab-window
target/release/render-lab-window --suite all --frames 600 --warmup 120 \
  --present fifo,immediate > window.jsonl
```

`tools/build-webgpu-native.sh` falls back to Python's `zipfile` where `unzip`
is missing (NixOS). The suites are defined once in
`crates/render-lab/src/bench.rs` for both runners.

Each output line holds the case name, a summary, and the final `Stats`. The
summary fields are:

- `frame_ms`: wall time for the measured frames, including a final wait for
  the GPU, divided by the frame count. Frames are pipelined with no vsync, so
  this is the throughput frame time, max(CPU, GPU) in steady state.
- `gpu_total_ms`: median of the lab's timestamp total (first pass start to last
  pass end in one frame).
- `cpu_frame_ms`: median CPU time inside `Lab::frame`. It includes time the
  driver blocks in submit or poll, so on a GPU-bound case it can approach
  `frame_ms`.
- `cpu_cull_ms` and `main_encode_ms`: the CPU path's culling and its main-pass
  draw encoding.

### Driver fault fixed by the native run

**Measured:** on the RTX 4080 (Vulkan, driver 595.71.05) the first native runs
lost the device within a few frames, with kernel `NVRM: Xid 32` faults. Runs
with `AF_GPU_DISABLE_FEATURES=timestamp-query` did not fault. The lab resolved
all 24 timestamp queries every frame, including queries no pass of the current
path wrote. WebGPU defines those as zero; the native driver faulted instead.
The lab now resolves only the slots a frame wrote, each pair into its own
256-byte block, and clears the readback so unwritten slots read as zero. No Xid
appeared in later runs.

## Native results

Setup (**measured** 2026-10-24): default lab config (50,000 instances, 16
variants, 20,000 crowd agents, TAA, LOD and occlusion on, street camera) with
the scene time fixed at 12 s; 1920x1080 offscreen RGBA8; 60 warm-up frames and
300 measured frames per case; one process per machine running every case.

- RTX 4080: CoderOS (NixOS), Intel Core i7-14700K, wgpu-native v29.0.1.1 on
  Vulkan, NVIDIA driver 595.71.05. Other processes held GPU memory
  (inference servers, a browser) but the GPU was idle (0% utilization) before
  the runs.
- M5 Max: MacBook Pro on AC power, macOS 26.4, wgpu-native v29.0.1.1 on Metal.

Two runs per machine; ranges show both runs (the overview rows for the CPU and
visibility-buffer paths are single runs). Times are milliseconds.

| Case | 4080 frame | 4080 GPU | 4080 CPU | M5 frame | M5 GPU | M5 CPU | Draws | Visible | Triangles |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| CPU-batched, TAA | 2.48–2.52 | 0.78 | 2.45 | 2.95–3.00 | 1.59 | 2.76–2.84 | 6,773 | 24,183 | 9.60 M |
| GPU-driven forward, TAA | 0.25–0.26 | 0.22 | 0.22–0.23 | 0.47–0.51 | 0.33–0.35 | 0.35–0.40 | 909 | 266 | 0.14 M |
| Visibility buffer, TAA | 0.29–0.32 | 0.27–0.29 | 0.15–0.16 | 0.62–0.75 | 0.41–0.51 | 0.45–0.58 | 6 | 266 | 0.14 M |
| CPU-batched, MSAA 4x | 2.48–2.50 | 0.80 | 2.45–2.47 | 3.09–3.42 | 1.74–1.98 | 2.94–3.29 | 6,773 | 24,183 | 9.60 M |
| GPU-driven forward, MSAA 4x | 0.31 | 0.29 | 0.30 | 0.90–1.08 | 0.74–0.82 | 0.56–0.72 | 909 | 266 | 0.14 M |
| GPU-driven forward, no AA | 0.24 | 0.14 | 0.22 | 0.40–0.41 | 0.30–0.45 | 0.29–0.31 | 909 | 266 | 0.14 M |
| Visibility buffer, no AA | 0.22–0.23 | 0.20 | 0.12–0.13 | 0.44–0.63 | 0.40 | 0.27–0.38 | 6 | 266 | 0.14 M |
| GPU-driven forward, TAA, no occlusion | 0.73 | 0.71 | 0.72 | 1.41–1.92 | 1.34–1.93 | 0.44–0.55 | 457 | 24,183 | 9.60 M |
| Overview, CPU-batched, TAA | 2.53 | 0.81 | 2.46 | 2.95 | 1.53 | 2.77 | 6,779 | 24,184 | 9.39 M |
| Overview, visibility buffer, TAA | 0.83 | 0.86 | 0.31 | 1.96 | 1.35 | 1.12 | 6 | 14,716 | 5.33 M |
| Overview, GPU-driven, crowd tiers off | 1.06 | 1.03–1.04 | 1.05 | 1.92–2.70 | 2.28–2.31 | 0.51–0.57 | 907 | 14,421 | 5.17 M |
| Overview, GPU-driven, crowd tiers on | 0.56 | 0.54 | 0.55 | 1.47–1.77 | 1.01–1.38 | 0.35–0.38 | 909 | 14,757 | 5.36 M |

Draws, visible instances, and triangles are from the 4080's final frame; the
M5 matches on the street camera and differs by under 3% on the overview, where
occlusion depends on the previous frame's depth.

CPU-path detail (**measured**, medians):

| Machine | CPU cull | Main-pass encode (6,773 draws) | Per draw |
| --- | --- | --- | --- |
| RTX 4080 host (i7-14700K) | 1.24 ms | 0.31–0.34 ms | about 46–50 ns |
| M5 Max | 1.06–1.22 ms | 0.11–0.13 ms | about 16–19 ns |

Crowd tiers on the overview camera (**measured**, final frame): with tiers off,
2,998 (4080) and 3,112 (M5) visible agents are fully skinned and the crowd pass
takes 0.49 ms and 0.72 ms of GPU time. With tiers on, 61 agents use the mid
tier and 3,227 and 3,094 use far impostors; the crowd pass takes 0.008 ms and
0.033 ms.

### What the numbers show

- **Measured:** the CPU-batched path is CPU-bound on both machines: about
  2.5 ms (4080 host) and 3 ms (M5) per frame against 0.8 ms and 1.6 ms of GPU
  time. CPU culling and bucketing (1.1–1.2 ms) costs more than encoding the
  6,773 draws (0.1–0.3 ms).
- **Measured:** at equal visibility (no occlusion, 24,183 instances, 9.6 M
  triangles) GPU-driven forward costs 0.71 ms of GPU time on the 4080 against
  0.78 ms for the CPU path, with 457 draws instead of 6,773, and frames finish
  in 0.73 ms against 2.5 ms.
- **Measured:** occlusion culling removes 23,917 of 24,183 instances on the
  street camera and cuts GPU-driven frames from 0.73 ms to 0.25 ms (4080) and
  from 1.4–1.9 ms to 0.5 ms (M5).
- **Measured:** on the street camera the visibility buffer costs 0.05–0.07 ms
  (4080) and 0.08–0.16 ms (M5) more GPU time than GPU-driven forward, and less
  CPU time (0.12–0.16 ms on the 4080, six draws). Its compute resolve costs
  0.05 ms (4080) and 0.12 ms (M5).
- **Measured:** MSAA 4x adds 0.07 ms on the 4080 and 0.4 ms on the M5 over TAA
  for the GPU-driven path. TAA's resolve pass costs 0.05 ms and 0.12 ms.
- **Measured:** crowd tiers halve the overview frame on the 4080 (1.06 ms to
  0.56 ms) and cut the M5's GPU time from 2.3 ms to 1.0–1.4 ms.
- **Measured:** on the 4080, `cpu_frame_ms` tracks `frame_ms` for GPU-bound
  cases. **Inferred:** the NVIDIA driver blocks submission once a few frames
  are queued, so that CPU number includes waiting, not only encoding. On the
  M5 the CPU number stays below the frame time.
- **Measured:** M5 GPU medians vary by up to 35% between runs and the p95 is
  up to three times the median; the 4080 varies by under 5%. **Inferred:** the
  Apple GPU changes power state at these small loads. On some M5 rows the
  per-frame GPU span exceeds the throughput frame time. **Inferred:**
  consecutive frames overlap on the GPU, so a single frame's span can be longer
  than the interval between frames.
- **Inferred:** at 1920x1080 this scene is light for both GPUs (well under
  1 ms per frame on the GPU-driven paths). Differences between techniques of
  under about 0.1 ms here should be re-checked at a heavier load (more
  instances, higher resolution) before they decide anything.

### Compared with the web lab

No browser render-lab benchmark numbers were recorded in this repository before
this run, so the native table has no web counterpart yet. The only web figure
is the encoding cost in the [GPU Native research](../../imported/2026-10-24-gpu-native.md):
on the M5 Max in Chrome, about 1,500 draws cost 0.22–0.66 ms through Rust-to-JS
imports (about 150–440 ns per draw) and 0.10–0.15 ms from TypeScript (about
70–100 ns per draw), single samples.

- **Inferred:** native Rust encoding on the same M5 (about 16–19 ns per draw,
  measured above) is roughly 4–6x cheaper per draw than TypeScript calling
  WebGPU and 8–27x cheaper than Rust through JS imports. The web sample used a
  different draw count and configuration, so this is an order-of-magnitude
  comparison, not a matched one.
- Matched browser runs of these cases on both machines are in
  [Heavy scene](#heavy-scene) (#12600), with the street and overview cases
  for scale; `tools/measure-render-lab.mjs` runs all of them.

### Batched web calls

Per ADR 0004, the web backend batches pass commands (#12592). Rust writes each
render or compute pass's commands as `u32` words into Wasm memory
(`crates/aethyr/gpu/src/batch.rs`), and a single `replay` import
(`apps/aethyr/apps/render-lab/gpu-batch.ts`) issues them when the pass ends.
That replaces one import call per command. Batching is on by default. The
lab's **Rust GPU calls** control, or `calls: "per-call"` in its configuration,
turns it off for comparison, and `af_gpu::set_batched_calls` does the same for
other callers.

`apps/aethyr/tools/measure-render-lab-encode.mjs` compares the main-pass
encode of the CPU path at about 1,500 draws in three ways: Rust per call, Rust
batched, and TypeScript direct. It reports p50 and p95 and checks the issue's
gate, which requires batched to be within 0.1 ms of TypeScript at p95. Build
and serve the lab, then run the tool in a browser that has a real GPU adapter
(`AF_CDP` connects to a running Chrome).

On the RTX 4080 (Chrome 152, 1,460 draws), the first batched version was no
faster than one call per command: 0.815 ms at p95 batched, 0.805 ms per call,
and 0.560 ms from TypeScript. The cost was in the work on either side of the
crossing, not the crossing itself. The lab now reports `main_replay_ms` (the
JavaScript replay inside the encode) and `main_log_ms` (the Rust log build
that the TypeScript timer leaves out), so the encode splits into its Rust and
JavaScript parts. The changes that followed:

- The replay switches on literal opcodes, with draw commands first, and
  indexes the handle table directly.
- It passes an offset or size as its low word when the high word is zero.
  `lo + hi * 2^32` is always a double, which V8 allocates on every call.
- Rust records into one buffer per thread, borrowed once per command and
  inlined at each call site.
- A per-pass state cache (`af_gpu::batch::Recorder`) skips a pipeline, bind
  group, or buffer bind that repeats the one already set. It forgets at the
  end of each pass and on every release. The per-call path makes every bind.

SwiftShader in a container, 1,495 draws, 10 s × 5 rounds (CPU timings only,
not a GPU result):

| Path | Before p50 | Before p95 | After p50 | After p95 |
| --- | --- | --- | --- | --- |
| Rust per call | 0.290 ms | 0.640 ms | 0.290 ms | 0.630 ms |
| Rust batched | 0.305 ms | 0.580 ms | 0.275 ms | 0.545 ms |
| TypeScript direct | 0.210 ms | 0.360 ms | 0.215 ms | 0.365 ms |

After the changes, the Rust part of the batched encode is 0.050 ms at p50
and 0.085 ms at p95. The replay costs 0.020 ms more than the TypeScript loop
at p50, which is the opcode dispatch. The rest of the p95 gap is the replay's
wider tail. These runs also show wider whole-frame tails in both Rust modes,
so the gate needs the 4080 run again.

#### Real-GPU results after the changes

**Measured** on 2026-10-25 from a fresh clone of `main` at `90ca907caa6`
(which contains `fbfcd67f4da` and `ca4a5be18c8`), with
`tools/measure-render-lab-encode.mjs`, `AF_ROUNDS=5`, 10 s per sample,
1920x1080, 7,454 instances calibrated to 1,460 CPU-path draws. Each figure is
the median over the five rounds of that round's p50 or p95. The Wasm module
was built with rustc 1.98.1 and wasm-opt 132 on each machine. Neither run
reported page or lab errors.

**M5 Max** (macOS 26.4, Google Chrome 155.0.8059.85, headless, launched by the
tool with a temporary profile; WebGPU adapter `apple` / `metal-3`, not a
fallback adapter; about 400 frames per second, uncapped):

| Path | Encode p50 | Encode p95 | Per draw (p50) | `main_replay_ms` p50 / p95 | `main_log_ms` p50 / p95 | CPU frame p50 |
| --- | --- | --- | --- | --- | --- | --- |
| Rust per call | 0.115 ms | 0.600 ms | 79 ns | 0 / 0 | 0 / 0 | 0.260 ms |
| Rust batched | 0.110 ms | 0.590 ms | 75 ns | 0.080 / 0.560 ms | 0 / 0 | 0.245 ms |
| TypeScript direct | 0.075 ms | 0.595 ms | 51 ns | 0 / 0 | 0 / 0.005 ms | 0.220 ms |

Batched minus TypeScript at p95 is **−0.005 ms**, so the gate (≤ 0.1 ms) is
**met**. Per round, the gap at p95 was −0.020, 0.000, −0.005, −0.005 and
0.000 ms.

**RTX 4080** (CoderOS, i7-14700K, NVIDIA driver 595.71.05, the CoderOS GPU
browser, ungoogled Chromium 152.0.7977.82 over CDP; adapter `nvidia` /
`lovelace`; about 62.5 frames per second, since that browser keeps its vsync;
4.4 GB of 16 GB VRAM free, the rest held by local model servers):

| Path | Encode p50 | Encode p95 | Per draw (p50) | `main_replay_ms` p50 / p95 | `main_log_ms` p50 / p95 | CPU frame p50 |
| --- | --- | --- | --- | --- | --- | --- |
| Rust per call | 0.610 ms | 0.915 ms | 418 ns | 0 / 0.005 ms | 0 / 0 | 2.155 ms |
| Rust batched | 0.635 ms | 0.855 ms | 435 ns | 0.510 / 0.690 ms | 0 / 0 | 2.180 ms |
| TypeScript direct | 0.450 ms | 0.585 ms | 308 ns | 0 / 0 | 0.015 / 0.025 ms | 2.080 ms |

Batched minus TypeScript at p95 is **0.270 ms** here, so the 4080 still
misses the 0.1 ms margin. A first run in the same browser gave the same
figures (0.265 ms).

What the two runs show:

- **Measured:** on the M5 the three paths share one p95 tail of about
  0.6 ms, while their p50s differ by at most 0.04 ms. Batched replay alone
  reaches 0.56 ms at p95, so the tail is inside the WebGPU calls, which the
  TypeScript loop makes too.
- **Inferred:** that shared tail is Chrome's own cost, such as the WebGPU
  client flushing its command buffer to the GPU process, not the Wasm
  crossing.
- **Measured:** on the 4080 the JavaScript replay alone (0.510 ms p50,
  0.690 ms p95) costs more than the TypeScript loop that makes the same calls
  (0.450 / 0.585 ms). The Rust encode adds about 0.125 ms at p50.
- **Inferred:** the replay's opcode dispatch and word decoding cost more
  relative to the WebGPU calls on this host and browser than on the M5.
- **Measured:** this 4080 host ran slower than in the first 4080 run on
  `ce78251bc7d` (TypeScript p50 0.450 against 0.200 ms). The browser, its
  frame rate cap, and the host's load (load average about 3.5) may all
  differ between the two runs, so compare figures within one run only.

## Follow-up runs (#12600)

**Measured** on 2026-10-26 from fresh clones of `main` at `79196508036` with
the change that landed as `0280998ed70` applied. Both machines ran
the native runners at 1920x1080 with a fixed scene time of 12 s, 60 warm-up
and 300 measured frames offscreen, 120 warm-up and 600 measured frames in a
window. Times are milliseconds; ranges are two runs, single values are one.

- **M5 Max:** MacBook Pro on AC power, macOS 26.4 (25E246), built-in Liquid
  Retina XDR display at 120 Hz, Rust 1.98.1. Another agent was compiling C++
  on the machine at times; no other GPU workload was running.
- **RTX 4080:** CoderOS (NixOS), i7-14700K, NVIDIA driver 595.71.05, Vulkan,
  Rust 1.98.1. The host is shared: local model servers held about 10.5 GB of
  the 16 GB, other agents' browsers used the GPU on and off (3–26%
  utilization sampled just before runs), and the load average was 4.5–8.6.
  Figures on this host vary more than on the M5, and CPU-bound cases most.

### Window presentation

`render-lab-window` opens a winit window, builds the `SurfaceSource` from its
handles (a `CAMetalLayer` made with `raw-window-metal` on macOS, the Wayland
`wl_surface` on Linux), and renders every case into the surface. It measures
the interval between consecutive presented frames. The **M5** ran it in the
owner's logged-in GUI session, launched over ssh: a real window on the
built-in display, 1920x1080 physical pixels (scale 2), 120 Hz. The **4080**
has no display attached, so it ran under its own `cage` 0.3.0 compositor
with the wlroots headless backend (`WLR_BACKENDS=headless`,
`WLR_RENDERER=gles2` on the 4080, a private `XDG_RUNTIME_DIR`), the same
pattern as the shared GPU browser on port 9340, which was not touched. Its
output was set to 1920x1080 at 60 Hz with `wlr-randr`. That is real Vulkan
WSI presentation over Wayland to a compositor that composites on the GPU, but
no display scans the frames out; the compositor's frame clock paces them.

Present modes offered: `fifo` and `immediate` on the M5 (Metal), `fifo` and
`mailbox` on the 4080 (Wayland). wgpu-native, both machines (**measured**;
the M5 FIFO and immediate columns show two runs separated by `;`):

| Case | M5 offscreen frame | M5 FIFO p50 / p95 / p99 · fps | M5 immediate fps (p50) | 4080 offscreen frame | 4080 FIFO p50 / p95 / p99 · fps | 4080 mailbox fps (p50) |
| --- | --- | --- | --- | --- | --- | --- |
| `cpu-forward-taa` | 2.10–2.16 | 8.33 / 8.78 / 9.75 · 120 ; 8.32 / 8.75 / 10.16 · 119 | 180 (8.01) ; 149 (8.26) | 2.58–2.61 | 16.02 / 16.13 / 16.30 · 62 | 250 (3.63) |
| `gpu-forward-taa` | 0.34 | 8.32 / 8.64 / 8.92 · 120 ; 8.32 / 8.79 / 9.14 · 120 | 238 (1.77) ; 120 (8.32) | 0.27–0.31 | 16.02 / 16.08 / 16.18 · 62 | 1692 (0.57) |
| `visbuffer-taa` | 0.41–0.43 | 8.33 / 8.59 / 8.84 · 120 ; 8.33 / 8.66 / 8.81 · 120 | 260 (1.58) ; 120 (8.33) | 0.33 | 16.02 / 16.06 / 16.07 · 62 | 1499 (0.66) |
| `cpu-forward-msaa4` | 2.19–2.20 | 8.32 / 9.31 / 10.60 · 120 ; 8.33 / 8.81 / 11.22 · 119 | 193 (2.62) ; 120 (8.33) | 2.54–2.75 | 16.02 / 16.07 / 16.12 · 62 | 234 (3.91) |
| `gpu-forward-msaa4` | 0.63 | 8.33 / 8.74 / 9.63 · 117 ; 8.33 / 8.71 / 8.86 · 120 | 222 (2.27) ; 120 (8.33) | 0.35–0.36 | 16.03 / 16.17 / 16.34 · 62 | 537 (1.64) |
| `gpu-forward-no-aa` | 0.25 | 8.32 / 8.66 / 8.90 · 120 ; 8.32 / 8.65 / 8.88 · 120 | 225 (2.28) ; 120 (8.33) | 0.24 | 16.03 / 16.15 / 16.24 · 62 | 695 (1.33) |
| `visbuffer-no-aa` | 0.31–0.32 | 8.33 / 8.59 / 8.74 · 120 ; 8.33 / 8.68 / 8.92 · 120 | 218 (2.37) ; 120 (8.33) | 0.25 | 16.03 / 16.16 / 16.27 · 62 | 601 (1.46) |
| `gpu-forward-taa-no-occlusion` | 1.16–1.23 | 8.33 / 8.59 / 8.78 · 120 ; 8.33 / 8.65 / 8.81 · 120 | 222 (2.17) ; 120 (8.33) | 0.81–0.84 | 16.03 / 16.18 / 16.33 · 62 | 434 (2.04) |
| `cpu-overview-taa` | 2.15–2.32 | 8.32 / 8.79 / 9.98 · 120 ; 8.33 / 8.74 / 10.61 · 119 | 183 (7.74) ; 120 (8.32) | 2.51–2.53 | 16.02 / 16.16 / 16.26 · 62 | 204 (4.48) |
| `visbuffer-overview-taa` | 1.79–1.96 | 8.33 / 8.71 / 9.10 · 120 ; 8.32 / 8.75 / 8.94 · 120 | 346 (1.63) ; 120 (8.33) | 0.88–0.89 | 16.04 / 16.17 / 16.36 · 62 | 408 (2.39) |
| `gpu-overview-crowd-tiers-off` | 1.56–1.80 | 8.33 / 8.68 / 8.86 · 120 ; 8.33 / 8.74 / 8.92 · 120 | 198 (7.99) ; 120 (8.32) | 1.16 | 16.03 / 16.14 / 16.26 · 62 | 342 (2.98) |
| `gpu-overview-crowd-tiers-on` | 1.21–1.37 | 8.32 / 8.58 / 8.76 · 120 ; 8.33 / 8.79 / 8.91 · 120 | 256 (1.64) ; 120 (8.33) | 0.62–0.64 | 16.04 / 16.16 / 16.21 · 62 | 554 (1.78) |
| `heavy-cpu-taa` | 9.67–9.81 | 9.28 / 9.56 / 9.64 · 108 ; 9.40 / 9.66 / 10.67 · 106 | 107 (9.34) ; 105 (9.48) | 14.71–14.89 | 20.79 / 28.78 / 33.44 · 47 | 51 (19.71) |
| `heavy-gpu-taa` | 1.93–1.99 | 8.33 / 8.75 / 9.05 · 120 ; 8.33 / 8.94 / 9.17 · 120 | 192 (7.81) ; 120 (8.33) | 1.13–1.22 | 16.03 / 16.17 / 16.34 · 62 | 524 (1.81) |
| `heavy-visbuffer-taa` | 2.59–2.86 | 8.33 / 8.69 / 8.82 · 120 ; 8.34 / 9.10 / 9.23 · 120 | 192 (2.58) ; 120 (8.32) | 1.59–1.69 | 16.03 / 16.21 / 16.34 · 62 | 441 (2.10) |
| `heavy-gpu-no-occlusion` | 2.98–3.09 | 8.33 / 8.95 / 9.16 · 120 ; 8.35 / 9.10 / 9.23 · 120 | 164 (8.09) ; 120 (8.33) | 2.12–2.30 | 16.02 / 16.09 / 16.22 · 62 | 377 (2.57) |
| `heavy-visbuffer-no-occlusion` | 4.60–4.65 | 8.31 / 8.96 / 9.21 · 120 ; 8.34 / 9.17 / 9.28 · 120 | 141 (8.25) ; 120 (8.33) | 3.32–3.33 | 16.03 / 16.16 / 16.25 · 62 | 261 (3.80) |
| `heavy-gpu-no-lod` | 5.61–5.72 | 8.32 / 8.89 / 9.11 · 120 ; 8.33 / 8.58 / 8.73 · 120 | 132 (8.25) ; 120 (8.32) | 3.77–3.79 | 16.02 / 16.07 / 16.14 · 62 | 233 (4.16) |
| `heavy-visbuffer-no-lod` | 8.26–8.30 | 8.33 / 8.47 / 8.54 · 120 ; 8.34 / 8.62 / 16.67 · 116 | 121 (8.30) ; 117 (8.33) | 6.26–6.31 | 16.02 / 16.09 / 16.22 · 62 | 149 (6.67) |
| `heavy-gpu-crowd-tiers-off` | 3.26–3.48 | 8.33 / 8.98 / 9.22 · 120 ; 8.32 / 8.98 / 9.14 · 120 | 182 (3.43) ; 120 (8.33) | 2.36–2.40 | 16.02 / 16.22 / 17.03 · 62 | 314 (3.08) |

- **Measured:** with FIFO on the M5 every case except `heavy-cpu-taa` holds
  the 120 Hz cadence: 8.33 ms at p50, at most 9.3 ms at p95, and at most
  11.2 ms at p99, with 0–5 of 600 intervals above 1.5 times the median. One
  of the two runs of `heavy-visbuffer-no-lod` (8.3 ms of GPU work, at the
  edge of the 8.33 ms budget) missed 21 refreshes: p99 16.67 ms, 116 fps. `heavy-cpu-taa` is CPU-bound at
  9.3–9.5 ms and runs at 106–108 fps with a steady interval.
- **Measured:** with FIFO on the 4080 every case except `heavy-cpu-taa`
  holds the headless output's cadence of 16.02 ms (62.4 fps, not 60: the
  wlroots headless frame timer; the 9340 browser also ran at 62.5 fps), with
  p99 at most 17.0 ms. `heavy-cpu-taa` is CPU-bound at 19–21 ms there, slower
  than its 14.7 ms offscreen on the same loaded host.
- **Measured:** mailbox on the 4080 runs uncapped. Light cases run at
  0.57–0.66 ms per presented frame against 0.27–0.33 ms offscreen; heavy
  cases add 0.3–0.7 ms to the offscreen frame. **Inferred:** acquiring and
  presenting a surface texture through the compositor costs about 0.3 ms a
  frame on this host.
- **Measured:** `immediate` on the M5 is not a stable uncapped mode. In the
  first run intervals split between 1.6–3 ms and 8–9 ms; in the second run
  most cases stayed at 120 fps. **Inferred:** the window server still
  composites the layer at the display rate and the drawable pool decides
  how far the app can run ahead. Offscreen `frame_ms` stays the throughput
  figure; the window numbers are the pacing figure.
- **Measured:** GPU timestamps under FIFO are higher than offscreen on the
  M5 (for example `gpu-forward-taa` 2.4 ms against 0.3 ms). **Inferred:** the
  Apple GPU lowers its clocks when frames are paced to the display; the
  offscreen run keeps it busy.
- **Dawn in a window (measured):** the same runner linked to Dawn presented
  on both machines (Metal layer on the M5, Wayland under cage on the 4080).
  FIFO pacing matched wgpu-native: 8.31–8.35 ms p50 on the M5 (except
  `heavy-cpu-taa` at 9.76 ms and `heavy-visbuffer-no-lod`, p95 16.7 ms,
  102 fps), 16.02–16.03 ms on the 4080 with p99 up to 18.2 ms. Dawn's
  `immediate` on the M5 was bimodal with p95 near 16.7 ms; its 4080 mailbox
  ran 49–868 fps.
- The first frame on the M5 can find the window not yet visible: wgpu-native
  returns its `Occluded` status and the lab renders that frame offscreen
  (reported as `surface texture unavailable (occluded)`). It happened once in
  the first run, during warm-up, and not in the second.

### Dawn

`tools/fetch-dawn.sh` downloads Dawn's own prebuilt release, pinned to
`v20261023.194543` (Dawn commit `1716e88cc9e58398fab5d4473f7f2e858667b677`),
checks the archive's SHA-256 before extracting it, and refuses unknown
platforms rather than downloading unpinned. The pinned digests are the ones
GitHub publishes for the release assets and were re-checked by download:

| Asset | SHA-256 |
| --- | --- |
| `Dawn-1716e88…-macos-latest-Release.tar.gz` (macOS arm64) | `8f013482cad9e634995538089c8354c2a70b7a669a3952cd03ab60d297a55f9a` |
| `Dawn-1716e88…-ubuntu-latest-Release.tar.gz` (Linux x86_64) | `bef250fdfbd078ffc22ffc9a756b66b190e2b04121c0f6df216e7905395bc45b` |
| `Dawn-1716e88…-macos-15-intel-Release.tar.gz` (macOS x86_64) | `a22c3faa2777e01dee29aa5f969da80cd608d31fdaa57e95b532495838cb8048` |

The release ships `libwebgpu_dawn.a` (in `lib/` on macOS, `lib64/` on
Linux). `af-gpu`'s build script links it statically with `libc++` and the
Metal, QuartzCore, IOSurface, IOKit, Cocoa, Foundation, and CoreFoundation
frameworks on macOS, and with `libstdc++`, `libdl`, `libpthread`, `libm`, and
`librt` on Linux. The bindings generate from Dawn's `webgpu.h` with no source
changes beyond what the `af_dawn` cfg already held; the NixOS build needed
`BINDGEN_EXTRA_CLANG_ARGS` with the clang and glibc include directories,
which wgpu-native's header needs there too.

Every case renders on both implementations with no device errors, and the
captured frames match: across all 20 cases on both machines the mean absolute
difference is at most 0.002 of 255 per channel and no sampled channel differs
by more than 16 (**measured**, `--capture` PPMs compared per pixel).
Offscreen, two runs each (**measured**):

| Case | M5 wgpu-native frame | M5 Dawn frame | M5 wgpu-native GPU | M5 Dawn GPU | 4080 wgpu-native frame | 4080 Dawn frame | 4080 wgpu-native GPU | 4080 Dawn GPU |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `cpu-forward-taa` | 2.10–2.16 | 2.71–2.78 | 1.31 | 1.31–1.77 | 2.58–2.61 | 2.92 | 0.71–0.79 | 0.72 |
| `gpu-forward-taa` | 0.34 | 0.36 | 0.32 | 0.26 | 0.27–0.31 | 0.29–0.30 | 0.20–0.21 | 0.20 |
| `visbuffer-taa` | 0.41–0.43 | 0.42–0.43 | 0.40 | 0.26 | 0.33 | 0.82–1.03 | 0.27–0.28 | 0.26 |
| `cpu-forward-msaa4` | 2.19–2.20 | 2.69–2.72 | 1.43–1.46 | 1.44–1.51 | 2.54–2.75 | 2.91–3.05 | 0.81–0.82 | 0.85 |
| `gpu-forward-msaa4` | 0.63 | 0.43–0.45 | 0.59–0.60 | 0.33 | 0.35–0.36 | 0.34 | 0.28–0.29 | 0.26 |
| `gpu-forward-no-aa` | 0.25 | 0.28 | 0.20–0.22 | 0.13 | 0.24 | 0.28 | 0.14 | 0.13 |
| `visbuffer-no-aa` | 0.31–0.32 | 0.33–0.36 | 0.29 | 0.20–0.26 | 0.25 | 0.77–0.89 | 0.20–0.21 | 0.20 |
| `gpu-forward-taa-no-occlusion` | 1.16–1.23 | 1.14–1.17 | 1.09–1.23 | 1.05–1.11 | 0.81–0.84 | 0.79–0.80 | 0.72 | 0.72 |
| `cpu-overview-taa` | 2.15–2.32 | 2.70–2.73 | 1.28–1.34 | 1.25 | 2.51–2.53 | 3.05 | 0.73–0.74 | 0.72 |
| `visbuffer-overview-taa` | 1.79–1.96 | 1.34–1.35 | 1.32–1.34 | 1.18–1.25 | 0.88–0.89 | 1.06–1.17 | 0.78–0.91 | 0.79 |
| `gpu-overview-crowd-tiers-off` | 1.56–1.80 | 1.48–1.52 | 1.50–1.58 | 1.31–1.38 | 1.16 | 1.08–1.19 | 1.04–1.07 | 1.05 |
| `gpu-overview-crowd-tiers-on` | 1.21–1.37 | 0.92–0.97 | 0.91–1.02 | 0.85 | 0.62–0.64 | 0.57–0.68 | 0.54–0.55 | 0.52 |
| `heavy-cpu-taa` | 9.67–9.81 | 10.08–10.65 | 4.97–5.18 | 5.96–6.09 | 14.71–14.89 | 15.24–15.45 | 2.13–3.24 | 2.23–2.56 |
| `heavy-gpu-taa` | 1.93–1.99 | 1.87–1.88 | 2.15–2.21 | 1.70–1.84 | 1.13–1.22 | 1.21–1.55 | 0.98–1.10 | 0.98–1.11 |
| `heavy-visbuffer-taa` | 2.59–2.86 | 2.41–2.42 | 2.34–2.37 | 2.36–2.56 | 1.59–1.69 | 1.59–1.84 | 1.44–1.60 | 1.38–1.70 |
| `heavy-gpu-no-occlusion` | 2.98–3.09 | 2.97–3.03 | 2.93–3.43 | 2.88–2.95 | 2.12–2.30 | 2.10–2.70 | 2.05–2.10 | 1.97–2.10 |
| `heavy-visbuffer-no-occlusion` | 4.60–4.65 | 4.58–4.59 | 4.37–4.48 | 4.33–4.46 | 3.32–3.33 | 3.33–4.37 | 3.23–3.42 | 3.21–4.59 |
| `heavy-gpu-no-lod` | 5.61–5.72 | 5.56–5.57 | 5.49–5.96 | 5.44–5.51 | 3.77–3.79 | 3.75–4.36 | 3.63–3.76 | 3.54–4.19 |
| `heavy-visbuffer-no-lod` | 8.26–8.30 | 8.19–8.23 | 8.29–8.34 | 8.19–8.32 | 6.26–6.31 | 5.97–6.47 | 6.06–6.24 | 5.83–6.49 |
| `heavy-gpu-crowd-tiers-off` | 3.26–3.48 | 3.20–3.21 | 3.23–3.28 | 3.01–3.08 | 2.36–2.40 | 2.20–2.48 | 2.24–2.30 | 2.16–2.29 |

CPU time inside `Lab::frame` (median):

| Case | M5 wgpu-native CPU | M5 Dawn CPU | 4080 wgpu-native CPU | 4080 Dawn CPU |
| --- | --- | --- | --- | --- |
| `cpu-forward-taa` | 2.09–2.14 | 2.69–2.77 | 2.47–2.49 | 2.88 |
| `gpu-forward-taa` | 0.30–0.31 | 0.24–0.25 | 0.24–0.25 | 0.28–0.29 |
| `visbuffer-taa` | 0.36 | 0.31 | 0.14–0.16 | 0.39–0.71 |
| `cpu-forward-msaa4` | 2.15–2.17 | 2.67–2.71 | 2.46–2.50 | 2.87–2.98 |
| `gpu-forward-msaa4` | 0.40–0.41 | 0.35 | 0.30–0.31 | 0.29–0.30 |
| `gpu-forward-no-aa` | 0.24–0.25 | 0.22 | 0.22–0.23 | 0.27 |
| `visbuffer-no-aa` | 0.21–0.22 | 0.24 | 0.12–0.13 | 0.42–0.64 |
| `gpu-forward-taa-no-occlusion` | 0.20–0.22 | 1.07 | 0.74 | 0.72–0.73 |
| `cpu-overview-taa` | 2.14–2.30 | 2.68–2.72 | 2.47–2.49 | 2.88–2.94 |
| `visbuffer-overview-taa` | 0.57–0.62 | 1.18 | 0.23 | 0.92–1.05 |
| `gpu-overview-crowd-tiers-off` | 0.32–0.34 | 1.35–1.36 | 1.07 | 1.05–1.08 |
| `gpu-overview-crowd-tiers-on` | 0.27 | 0.82–0.83 | 0.56 | 0.55 |
| `heavy-cpu-taa` | 9.37–9.39 | 10.04–10.68 | 14.56–14.80 | 15.15–15.19 |
| `heavy-gpu-taa` | 0.61–0.62 | 1.70 | 1.02–1.12 | 1.04–1.19 |
| `heavy-visbuffer-taa` | 2.28 | 2.22–2.23 | 0.14–0.19 | 0.58–0.59 |
| `heavy-gpu-no-occlusion` | 0.22 | 2.88–2.90 | 2.07–2.11 | 1.94–2.09 |
| `heavy-visbuffer-no-occlusion` | 0.13–0.14 | 4.35–4.38 | 0.08–0.09 | 0.25–0.30 |
| `heavy-gpu-no-lod` | 0.44–0.46 | 5.37–5.39 | 3.66–3.67 | 3.63–4.07 |
| `heavy-visbuffer-no-lod` | 7.72–7.91 | 7.93–8.12 | 0.14 | 0.32–0.52 |
| `heavy-gpu-crowd-tiers-off` | 0.44–0.45 | 3.01 | 2.29–2.33 | 2.18–2.35 |

- **Measured:** GPU time is the same on both implementations within run
  noise wherever the GPU-driven paths carry load (overview and heavy
  cases). On the M5 Dawn's GPU spans are lower on several light street
  cases (for example `gpu-forward-msaa4` 0.33 against 0.59–0.60 ms).
- **Measured:** Dawn costs more CPU per draw. The CPU path's main-pass encode
  of 6,773 draws takes 0.47–0.48 ms with Dawn against 0.10 ms with
  wgpu-native on the M5 (about 70 against 15 ns per draw), and 0.96 against
  0.34 ms on the 4080 (about 140 against 50 ns). The 13,537-draw heavy CPU
  case doubles both. That makes Dawn's CPU-path frames 0.4–0.7 ms slower.
- **Measured:** on the 4080 Dawn also spends more CPU on the
  visibility-buffer path's light cases (0.39–0.71 ms against 0.14–0.16 ms
  for `visbuffer-taa`), so those frames are CPU-bound at 0.8–1.0 ms against
  0.33 ms. **Inferred:** Dawn's per-command validation and submission work,
  which the visibility buffer's few draws don't amortize at this light load.
- **Measured:** on the M5 Dawn's `cpu_frame_ms` approaches the frame time on
  GPU-bound cases while wgpu-native's stays low. **Inferred:** Dawn blocks in
  submission once frames are queued, as the NVIDIA driver does under both
  implementations; it says nothing about encoding cost.
- Dawn qualifies on both desktop targets with these numbers. ADR 0004 names
  Dawn first with wgpu-native as the fallback per target; on these two
  machines wgpu-native has the lower CPU cost and equal GPU time.

### Heavy scene

The `heavy` suite (`crates/render-lab/src/bench.rs`) is a city-scale scene:
500,000 static instances (ten times the street scene) with 32 mesh variants,
100,000 crowd agents (five times), seen from the overview camera. Its cases
run the three paths and turn occlusion, LOD, and crowd tiers off one at a
time. `tools/measure-render-lab.mjs` runs the same cases (and the `compare`
suite) in the web lab at 1920x1080 in a browser with vsync and the frame-rate
cap off, and reports the frame interval, CPU frame, and GPU timestamp totals.

Browsers (**measured**): on the M5, Google Chrome 156.0.8078.48, headless,
launched by the tool, adapter `apple` / `metal-3`. On the 4080, ungoogled
Chromium 152.0.7977.82 started in its own `cage` (headless wlroots, ANGLE on
Vulkan, `--disable-gpu-vsync --disable-frame-rate-limit`, debugging port
9352), adapter `nvidia` / `lovelace`. Neither browser reported page, lab, or
device errors. The page was served from a static build of `main` plus this
change with the cross-origin isolation headers of `vite preview`; the Wasm
module was built with rustc 1.98.1 and wasm-opt 132 on the M5 and used on
both machines. Web GPU figures are the median and p95 of the lab's timestamp
total over 10 s; native figures are as in the tables above. The street and
overview rows are for scale:

| Case | Visible | Triangles | M5 native GPU (wgpu-native · Dawn) | M5 native frame | M5 web GPU p50 / p95 | M5 web interval p50 | 4080 native GPU (wgpu-native · Dawn) | 4080 native frame | 4080 web GPU p50 / p95 | 4080 web interval p50 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `gpu-forward-taa` | 266 | 0.1 M | 0.32 · 0.26 | 0.34 | 0.25 / 0.54 | 0.14 | 0.20–0.21 · 0.20 | 0.27–0.31 | 0.23 / 0.25 | 0.60 |
| `visbuffer-taa` | 266 | 0.1 M | 0.40 · 0.26 | 0.41–0.43 | 0.33 / 0.35 | 0.19 | 0.27–0.28 · 0.26 | 0.33 | 0.30 / 0.43 | 1.33 |
| `gpu-overview-crowd-tiers-on` | 14,670 | 5.3 M | 0.91–1.02 · 0.85 | 1.21–1.37 | 1.04 / 1.46 | 0.97 | 0.54–0.55 · 0.52 | 0.62–0.64 | 0.54 / 0.55 | 0.46 |
| `visbuffer-overview-taa` | 14,757 | 5.4 M | 1.32–1.34 · 1.18–1.25 | 1.79–1.96 | 1.28 / 1.72 | 1.31 | 0.78–0.91 · 0.79 | 0.88–0.89 | 0.79 / 0.97 | 1.31 |
| `heavy-cpu-taa` | 71,250 | 27.2 M | 4.97–5.18 · 5.96–6.09 | 9.67–9.81 | 6.30 / 6.77 | 14.33 | 2.13–3.24 · 2.23–2.56 | 14.71–14.89 | 2.46 / 4.35 | 16.35 |
| `heavy-gpu-taa` | 36,172 | 11.2 M | 2.15–2.21 · 1.70–1.84 | 1.93–1.99 | 1.74 / 2.16 | 1.71 | 0.98–1.10 · 0.98–1.11 | 1.13–1.22 | 0.98 / 1.59 | 0.78 |
| `heavy-visbuffer-taa` | 35,947 | 11.1 M | 2.34–2.37 · 2.36–2.56 | 2.59–2.86 | 2.52 / 2.86 | 2.45 | 1.44–1.60 · 1.38–1.70 | 1.59–1.69 | 1.45 / 2.04 | 1.39 |
| `heavy-gpu-no-occlusion` | 71,250 | 27.2 M | 2.93–3.43 · 2.88–2.95 | 2.98–3.09 | 3.07 / 3.91 | 3.04 | 2.05–2.10 · 1.97–2.10 | 2.12–2.30 | 1.95 / 2.38 | 1.68 |
| `heavy-visbuffer-no-occlusion` | 71,250 | 27.2 M | 4.37–4.48 · 4.33–4.46 | 4.60–4.65 | 4.76 / 5.32 | 4.71 | 3.23–3.42 · 3.21–4.59 | 3.32–3.33 | 3.07 / 3.83 | 3.15 |
| `heavy-gpu-no-lod` | 32,742 | 59.5 M | 5.49–5.96 · 5.44–5.51 | 5.61–5.72 | 5.72 / 6.68 | 5.69 | 3.63–3.76 · 3.54–4.19 | 3.77–3.79 | 3.86 / 4.60 | 3.49 |
| `heavy-visbuffer-no-lod` | 32,652 | 59.2 M | 8.29–8.34 · 8.19–8.32 | 8.26–8.30 | 9.07 / 9.33 | 8.91 | 6.06–6.24 · 5.83–6.49 | 6.26–6.31 | 6.26 / 6.40 | 6.26 |
| `heavy-gpu-crowd-tiers-off` | 35,635 | 11.0 M | 3.23–3.28 · 3.01–3.08 | 3.26–3.48 | 3.52 / 4.02 | 3.35 | 2.24–2.30 · 2.16–2.29 | 2.36–2.40 | 2.51 / 2.99 | 2.14 |

- **Measured:** the heavy scene loads the GPU-driven paths about 5 to 25
  times more than the street scene: 1.0–1.1 ms (4080) and 1.7–2.2 ms (M5)
  for GPU-driven forward with occlusion and LOD, against 0.20 and 0.26–0.32
  ms on the street, and up to 5.8–6.5 ms (4080) and 8.2–9.1 ms (M5) for the
  visibility buffer with LOD off (59 M triangles).
- **Measured:** the CPU path is CPU-bound at 9.7–9.8 ms (M5) and
  14.7–14.9 ms (4080) natively, 14.3 and 16.3 ms in the browser, against
  GPU-driven frames of 1.9–2.0 and 1.1–1.2 ms natively. CPU culling of the
  500,000 instances takes 6.6 ms (M5) and 11.8 ms (4080) of that, and
  encoding the 13,537 draws 0.24 and 0.76 ms with wgpu-native.
- **Measured:** occlusion culling halves the visible set (71,250 to
  36,172 instances, 27.2 M to 11.2 M triangles) and cuts GPU-driven forward
  from 2.1 to 1.1 ms (4080) and from 2.9–3.4 to 2.2 ms (M5). LOD is worth
  more: off, the same view costs 3.6–3.8 ms (4080) and 5.5–6.0 ms (M5) for
  59 M triangles. Crowd tiers save 1.2 ms (4080) and 1.0 ms (M5).
- **Measured:** at this scale the visibility buffer is slower than
  GPU-driven forward on both machines: 1.4–1.6 against 1.0–1.1 ms (4080),
  2.3–2.6 against 1.7–2.2 ms (M5), and 5.8–6.5 against 3.5–4.2 ms (4080)
  with LOD off. In the browser's per-pass timings its ID raster
  (`raster_1`) is almost all of it: 1.05 of 1.45 ms (4080) and 1.94 of
  2.52 ms (M5); the material resolve is 0.05–0.08 ms. **Inferred:** with the
  lab's simple materials the visibility buffer saves little shading, and
  rasterizing 180,000–890,000 clusters through one indirect draw costs more
  than forward's per-mesh indirect draws. Its case needs heavier materials
  or more overdraw than this scene has before it decides anything (ADR 0011
  keeps a technique only where it measurably reduces GPU time).
- **Measured:** web GPU time matches native within run noise on the 4080
  for every heavy case (for example 0.98 against 0.98–1.10 ms for GPU-driven
  forward, 6.26 against 6.06–6.24 ms for the visibility buffer with LOD
  off). On the M5 the browser is within about 10% (1.74 against
  1.70–2.21 ms; 9.07 against 8.19–8.34 ms). **Inferred:** once the work is
  on the GPU, Chrome's WebGPU adds little; the web and native gaps are on
  the CPU side (the CPU path, and frame pacing).
- On light cases the browser's frame interval can be shorter than the GPU
  time (for example 0.14 ms against 0.25 ms on the M5). **Inferred:** with
  vsync off, `requestAnimationFrame` runs ahead of the GPU and the interval
  is not throughput there; compare GPU timings for those rows.

### 4080 batched-call gap (not a gate)

**Measured** with `tools/measure-render-lab-encode.mjs` (`AF_ROUNDS=5`, 10 s
per sample, 7,454 instances, 1,460 draws) in the 4080 browser above, with
vsync off: Rust per call 0.205 / 1.100 ms (p50 / p95), Rust batched 0.215 /
1.055 ms, TypeScript 0.145 / 1.050 ms; replay alone 0.170 ms at p50. The p95
gap is 0.005 ms, but all three paths share a p95 tail near 1 ms in this
uncapped, shared-host run, so the tail no longer separates them. At p50 the
batched encode is 0.070 ms slower than TypeScript; the replay loop alone is
0.025 ms slower than the TypeScript loop that makes the same calls (0.060 ms
in the earlier vsync-capped run). No code changed: the replay already switches on literal
opcodes over a packed handle table, and the remaining cost is the Rust
recording plus the decode. **Inferred:** the 0.270 ms p95 gap measured on
2026-10-25 came from that browser's vsync-paced frame loop and host load
more than from the replay itself.

## Open items

- The 4080's window runs present to a headless compositor, not a display.
  A run on a monitor attached to that machine (or another NVIDIA desktop)
  would add scan-out pacing; the M5 numbers are on a real display.
- At the heavy scene's scale the visibility buffer loses to GPU-driven
  forward on both GPUs, because the lab's materials are cheap. A scene with
  production-weight materials or heavy overdraw is needed before it decides
  anything (ADR 0011).
- Dawn on iOS and Android, ADR 0004's other qualification targets, is not
  covered here.
- The lab still has no automated native image check beyond
  `every_path_renders_natively` (`crates/render-lab/src/capi.rs`), which skips
  where no adapter exists, as in the GPU-less build container. The Dawn and
  wgpu-native captures above were compared by hand.
