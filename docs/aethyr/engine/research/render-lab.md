# Render lab

Date: 2026-10-24. Status: lab results for decisions
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
implementation's header by `crates/gpu/build.rs`. wgpu-native is the tested
implementation; the Dawn differences that are cheap to keep (device polling,
instance setup) sit behind the `af_dawn` cfg and are not tested.

- Without a window, frames render into an offscreen texture
  (`Gpu::surface_texture`, `Lab::capture`). A window surface (`SurfaceSource`:
  Metal layer, Xlib, Wayland, or Win32) is implemented but not yet exercised.
- wgpu-native is created with automatic timestamp normalization, so resolved
  timestamps are nanoseconds as in browsers and Dawn.
- `AF_WGPU_LOG=warn|info|debug|trace` forwards wgpu-native's log to stderr.
  `AF_GPU_DISABLE_FEATURES=timestamp-query,...` hides optional features from
  the device for A/B runs.
- The C ABI (`crates/render-lab/src/capi.rs`) exposes
  `af_lab_native_{create,configure,resize,orbit,frame,stats,wait_idle,capture,destroy,last_error}`.
  `frame` returns the same stats JSON the web lab reports.

Build and run (the script downloads the pinned wgpu-native v29.0.1.1 release and
checks its SHA-256, or builds a checkout with `--source`):

```sh
eval "$(tools/build-webgpu-native.sh --out ../webgpu-native)"
cargo build --release -p af-render-lab --features native --bin render-lab-native
target/release/render-lab-native --width 1920 --height 1080 --frames 300 --warmup 60 \
  --suite compare [--backend vulkan|metal] [--capture DIR] > results.jsonl
```

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
- Next step: a scripted browser run of the lab page's 10-second measurement
  with the same cases (`--suite compare` configurations, 1920x1080,
  scene time 12 s) on the M5 in Chrome, and on the 4080 in a browser with a
  real GPU adapter (headless Linux Chrome on CoderOS falls back to SwiftShader,
  per the GPU Native research), and add a matched table here.

## Open items

- Window presentation through `SurfaceSource` is written but not run.
- Dawn: the `af_dawn` paths compile against Dawn's header only in principle;
  no Dawn build was linked.
- The lab still has no automated native image check beyond
  `every_path_renders_natively` (`crates/render-lab/src/capi.rs`), which skips
  where no adapter exists, as in the GPU-less build container.
