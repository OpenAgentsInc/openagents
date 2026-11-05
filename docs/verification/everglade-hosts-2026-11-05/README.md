# Everglade: Chrome, WKWebView and native Vulkan (#12864)

Measured on 2026-11-05. The frozen program is `e3a0896e1e` plus this
benchmark instrumentation and its fixed camera. `artifacts.sha256` identifies
the actual Wasm, generated native C, SDK images, pack and shaders. This record
does not substitute an older native Everglade for the desktop's Everglade 5E.
The two generated web shader modules differ only in their provenance comment.

## Workload and machines

All four cases replay the same 2,264 room messages (74.860 seconds), captured
from the first of **sixteen actual bot connections** to one native networked
room. The observer consumes no seventeenth seat. The fixture contains the
development world's public crate edits and five development API responses,
with no credentials. Client intents cannot change this frozen room. Native
and browser hosts also replay the same HTTP status/body bytes.

The raid has a thirty-second announcement. Admission parses its `g` phase
records and refuses a fixture unless the entire measured **40–70 second**
interval is active siege (phase 7 or 8). The authority helper killed all 23
mutants. The captured siege first enters phase 7 at 36.110 seconds. The bot
whose connection is replayed eventually falls unconscious; the separate
benchmark camera keeps looking at the battle, regardless of that avatar.

Each renderer uses 1280×720 pixels, DPR 1, HIGH quality, painterly style,
fixed daylight 0.4, a 60 Hz callback ceiling and the same fixed South Gate
camera: eye `(18,18,-86)`, look-at `(0,1.4,-54)`. Normal camera presets are
unchanged. Screenshots accompany the samples. Native Vulkan uses the legacy
pass host with the 5E producer, **not** the future shared Metal host. The
native executable links the same musl math as Wasm, with contraction and
fast math disabled. There is no Worker, live room simulation, model, voice
capture or provider session in the renderer measurements. Native room tick
costs and drift are separately qualified in
[`../aethyrforge-offline-2026-11-05/`](../aethyrforge-offline-2026-11-05/).

| Machine | Configuration |
| --- | --- |
| Mac | M5 Max, 18 CPU cores, 40 GPU cores, 128 GB; macOS 26.4 (25E246); built-in 3456×2234, maximum 120 Hz |
| Mac Chrome | Google Chrome 156.0.8078.80; fresh temporary profile; headless, muted; ANGLE Metal |
| WKWebView | WebKit 21624.1.16.11.4; a visible foreground Aethyrforge window; muted; fresh nonpersistent store |
| coderos-4080 | i7-14700K, 28 logical CPUs, 128 GB; NixOS Linux 6.18.49; RTX 4080 16 GB, NVIDIA 595.71.05 |
| Linux Chrome | ungoogled Chromium 152.0.7977.82; fresh temporary profile; headless, muted; ANGLE Vulkan |
| Native | GCC 15.2.0, Vulkan, visible window on existing `wayland-1`; no VT switch |

The installed Aethyrforge was closed during the final matrix. Other agents
and the owner's browser were left alone. On coderos, an idle Elephant
allocation occupied about 12.9 GiB VRAM; GPU utilization before the matrix was
0%, P8, about 30 W, 35°C. This is a desktop-session observation, not a claim
of an otherwise empty laboratory machine. Host engines and graphics drivers
are intentionally the installed versions, not matched browser versions.

## Observations

The tables below are filled from the committed raw samples and the Coax
reducer. Durations are milliseconds; percentiles are nearest rank. The
unmodified rendering path's cadence, stage costs, CPU and memory come from
the **base** run. GPU spans come from a separate **gpu** run. Both replay
the same interval; their cadence difference exposes timestamp overhead.

| Base run | Mac Chrome | Mac WKWebView | Linux Chrome | Linux native Vulkan |
| --- | ---: | ---: | ---: | ---: |
| Submitted frames in 30 s | 1,785 | 1,784 | 1,788 | 1,790 |
| Frame interval p50 / p95 / p99 | 16.900 / 20.005 / 21.995 | 16.680 / 18.880 / 20.300 | 16.600 / 19.285 / 20.505 | 16.737 / 16.800 / 16.813 |
| Aggregate process CPU, % one core | 49.48 | 36.12 | 32.42 | 14.34 |
| Derived mean aggregate CPU per frame | 8.32 | 6.07 | 5.44 | 2.40 |
| Software input completion p50 / p95 / p99 | 22.905 / 193.245 / 193.245 | 23.120 / 130.520 / 130.520 | 16.430 / 24.965 / 24.965 | 8.161 / 10.962 / 10.962 |
| Startup to first ready frame, seconds | 0.768 | 1.046 | 1.939 | 0.233 |
| RSS, MiB (scopes below) | 1,769 | 588 | 1,464 | 258 |

Derived aggregate CPU/frame divides the measured process CPU rate by the
observed submission rate. It includes browser/host threads beyond the frame
thread. Browser process counter collection extends slightly past the last
snapshot; WK's six five-second samples took 30.9 s including bridge/scheduling
overhead. Frame/GPU percentiles are filtered to fixture seconds 40–70.

Per-frame stage observations, p50 / p95 / p99:

| Scope | Mac Chrome | Mac WKWebView | Linux Chrome | Linux native Vulkan |
| --- | ---: | ---: | ---: | ---: |
| Controller + river, wall ms | 0.360 / 2.425 / 3.795 | 0.900 / 1.440 / 1.640 | 0.240 / 2.575 / 3.100 | 1.452 / 2.145 / 2.499 |
| Scene + HUD construction, wall ms | 1.015 / 1.915 / 2.570 | 1.180 / 1.580 / 1.920 | 1.310 / 1.860 / 2.215 | 1.108 / 1.586 / 1.799 |
| Tail, wall ms | 0.020 / 0.035 / 0.050 | 0.020 / 0.040 / 0.060 | 0.030 / 0.045 / 0.070 | 0.014 / 0.021 / 0.024 |
| Host encode, browser wall / native thread CPU ms | 0.625 / 0.940 / 1.420 | 0.160 / 0.220 / 0.240 | 0.445 / 0.645 / 0.750 | 0.141 / 0.217 / 0.266 |
| Full frame, wall ms | 2.905 / 4.740 / 6.675 | 2.220 / 3.140 / 3.560 | 3.420 / 5.015 / 5.960 | 2.683 / 3.815 / 4.148 |
| Full frame thread CPU ms | unavailable | unavailable | unavailable | 2.813 / 4.009 / 4.322 |

These are separate percentiles, not additive budgets. Fixed-step work can
alternate between frames, so the median need not equal the mean. Native
frame-thread samples sum to 4.238 CPU seconds; the process counter reports
4.297 seconds over the same approximately thirty-second interval.

| Timestamp-enabled run | Mac Chrome | Mac WKWebView | Linux Chrome | Linux native Vulkan |
| --- | ---: | ---: | ---: | ---: |
| Submitted frames in 30 s | 1,167 | 1,276 | 1,787 | 1,791 |
| Frame interval p50 / p95 / p99 | 17.995 / 57.230 / 66.915 | 16.940 / 64.000 / 78.620 | 16.590 / 18.770 / 20.270 | 16.731 / 16.806 / 16.816 |
| GPU span p50 / p95 / p99 | 11.923 / 53.797 / 62.200 | 12.439 / 54.850 / 65.699 | 6.487 / 6.609 / 6.656 | 5.897 / 7.057 / 7.371 |

At this resolution all four ordinary paths sustain about 60 submissions/s.
On this Linux machine, native uses about 56% less aggregate process CPU than
Chrome and has a substantially tighter frame-interval tail. Warm native
startup and its software input-completion proxy are also lower. The Mac base
results do **not** establish that WKWebView is slower than Chrome. Both Mac
timestamp-enabled runs exhibit large stalls absent from their controls;
their GPU figures cannot be treated as unperturbed GPU costs. Run order was
not counterbalanced, and WK reported nominal/fair thermal state (0/1), with
low-power mode false. These results support continuing the native path, not
a claim about full Retina resolution, optical latency or a future Metal host.

## What the counters mean

Frame cadence is the interval between **actual queue submissions**, not the
animation callback rate. The native column uses actual frame-begin times.
GPU admission can skip a callback while an unfinished frame is pending;
counting those callbacks as rendered frames would hide a slowdown.

The browser stage counters use `performance.now`; the native producer uses
`CLOCK_MONOTONIC`. Controller/river, scene/HUD construction and tail are
wall-clock scopes in the same C face. Scene/HUD includes the town updates
performed there; it is not a pure drawing or pure simulation counter.
Browser host encoding is wall time. Native host encoding and full frame
thread CPU use `CLOCK_THREAD_CPUTIME_ID`. Aggregate native CPU comes from
`getrusage`; Chrome process CPU comes from CDP `SystemInfo.getProcessInfo`;
WK app/web/GPU CPU uses `proc_pid_rusage`, with Mach timebase conversion.
These scopes are labeled rather than treating browser wall time as CPU time.

GPU samples bracket the real render passes with timestamp queries. Browser
span is earliest pass beginning through latest pass end; overlapping pass
durations must **not** be summed as frame time (the raw sum is retained only
as a diagnostic). Vulkan brackets its complete render command buffer. Browser
timestamp support/activation, canvas size, room seat, Worker count and errors
are retained in each snapshot. The reducer refuses an unseated browser,
nonzero Workers, errors, a resized canvas or incomplete frame/input samples.
The timestamp API itself can change scheduling; the separate controls are
part of the result, not an optional check.

Latency is a **software input-to-GPU-completion callback proxy**. Six synthetic
W presses begin at fixture seconds 40,45,50,55,60,65. Each is timed until the
first subsequent submitted frame's completion callback/fence. Browser values
include callback/IPC scheduling. This measures neither an electrical input
device nor scanout/pixel response. With six samples, p95/p99 are both the
maximum; they are not stable tail estimates. A literal physical
input-to-photon measurement needs an external sensor or high-speed camera.

Startup uses process launch to first scene-ready frame: Chrome launch plus
page observer readiness, WK's existing first-render readiness, and the native
first completed GPU frame. These are software readiness boundaries, with
warm OS/driver caches and fresh browser data stores. Native cold shader-cache
startup was separately observed at about 21.9 seconds during preparation;
that exploratory observation is not mixed into the warm matrix percentiles.
Memory is end-of-run summed process RSS for Chrome and WK app/web/GPU, and
peak process RSS for native. Shared pages can be counted more than once;
these are not unique physical allocations or a GPU memory budget.

## Rerun

With the prepared SDK images and shader directory, the complete eight-run
matrix (four hosts × timestamps off/on), fixture staging, screenshots and
sample reduction is one command, from a worktree on this Mac:

```sh
COAX_CHECK_JOBS=2 COAX_JOBS=2 coax/web/tool.sh everglade-matrix \
  --prepared "$HOME/.openagents/aethyrforge/benchmark-12864" \
  --app "$HOME/.openagents/aethyrforge/benchmark-12864/Aethyrforge.app" \
  --out "$HOME/.openagents/aethyrforge/benchmark-12864/results"
```

It takes about ten minutes and has a fourteen-minute deadline. It never
opens a browser window or plays sound. It opens the required native SDK
windows, replays the committed anonymous fixture, and uses only the
benchmark's private world folders. `--remote`, `--remote-root` and
`--remote-worktree` select an equivalent prepared Linux session. The defaults
are coderos-4080, `~/.openagents/aethyrforge/benchmark-12864`, and the separate
`~/oa-forge-render-benchmark` checkout. No main checkout is changed.

Preparation uses the **frozen program**, not whatever game/wire changes
have since landed on main. `producer.patch.gz` records the complete source
instrumentation against `e3a0896e1e`, before rebasing this measurement record.
In a fresh disposable worktree at that commit, decompress/apply that patch and copy this
record's `fixture/` directory to its matching repository-relative path.
This is especially important for later build-identity admission changes:
do not feed an old welcome to a different program and call it the same scene.
The durable prepared images on these two machines retain the measured program.

Preparation, in those separate frozen-source worktrees:

1. Build the desktop shell normally with `apps/aethyrforge-desktop/build.sh`.
   The app bundles the observer; no installed app is overwritten by the
   matrix. Compile the browser producer normally with `coax/web/wasm.sh` if
   it was not already built by the shell.
2. Generate the **5E** native producer into a fresh directory with
   `verse/native/build.sh everglade5e PREPARED/producer`. Do not substitute
   `realm.sh`'s older `everglade` producer. Generate the shader spec without
   rebuilding that older Wasm:

   ```sh
   coax/web/tool.sh verse/gpu/everglade-pass.coax coderos-4080 live \
     --wasm apps/openagents-web/build/everglade5e.wasm \
     --emit PREPARED/spec
   ```

3. Stage `verse/gpu/{vk-realm.c,vk-spec.c,vk-spec.h,vks.c,vks.h,vkc.c,vkc.h,
   vkp.c,vkp.h,vkd.c,vkd.h,vkw.h,vkdesk.h,vkhands.h}`, `verse/kms/*.h`, the
   generated `os/verse-panel.coax` and `os/verse-hands.coax` C/headers, the
   pack, and committed `bench.facts` beside that producer. Copy producer,
   spec and the same Wasm/web assets/host scripts to the prepared Linux
   worktree/session. The production `realm.sh` staging is a reference for
   these platform effects; the benchmark does not take a screen/VT.
4. In that Linux worktree, build the staged effects with the existing Nix
   SDK (the tool compiles serially and requires musl math):

   ```sh
   nix-shell -p gcc pkg-config libdrm vulkan-headers vulkan-loader \
     wayland wayland-scanner wayland-protocols --run \
     'coax/web/tool.sh coax/web/everglade-native-build.coax ABS_PRODUCER ABS_WORKTREE/verse/native/musl-math.sh'
   ```

The reducer alone is `coax/web/tool.sh everglade-benchmark-stats INPUT.json
OUTPUT.json`. Summary durations are integer microseconds; raw files retain
the original timestamps. To capture a **new** workload rather than replay
the qualified one, `everglade-benchmark record --out DIR` starts the loopback
room and sixteen bots, then validates/packs the capture. Such a capture is a
new experiment and must not be mixed into this record.

## Checks

- Fixture authority: all 23 mutants killed, zero survivors; no equivalents
  marked. Capture checks: 195 passed. Driver/dependency checks: 83 passed.
  Reducer/dependency checks: 81 passed, plus the later current-fence refusal
  check. Native SDK builder: 10 passed. Fixed camera: its focused check passed.
- The complete matrix command produced all eight raw reports in under ten
  minutes. Subsequent validation found that an available Vulkan query could
  still be from the preceding frame before the next command buffer executed
  its reset. That instrumented native report was rejected. The host now waits
  for **this submission's fence** before reading timestamps/completion. Only
  the affected native GPU case was rebuilt/replaced; unchanged passing
  controls were retained. Both native executable digests are identified.
- The reducer refuses the actual rejected report because its claimed input
  completion predates that frame's GPU span. The replacement passes that
  check. Native screenshot readback occurs once in the GPU case only; browser
  screenshots are taken after the measured interval. Native's compositor
  resized its visible surface to 1271×1428, while the render target remained
  1280×720; its screenshot includes that presentation scaling.
- SDK builds and the visible muted WK probes exercised timestamp activation,
  real submit/completion transport and the fixed camera before the final runs.
  All browser windows retained seat zero, zero Workers, fixed canvas size and
  no reported JavaScript/GPU errors. Linux Chrome selected NVIDIA Lovelace,
  not a software adapter.
- `tools-check` reports 18 pre-existing files without Firm-reason headers;
  none is part of this change. The new browser observer and Swift SDK effects
  carry their required headers. Raw check logs are in `checks/`.

API references: [WebGPU timestamp writes](https://gpuweb.github.io/types/interfaces/GPURenderPassTimestampWrites.html),
[Vulkan timestamps and timestampPeriod](https://registry.khronos.org/vulkan/specs/latest/pdf/vkspec.pdf),
and [WebKit's guarded diagnostic process identifier](https://github.com/WebKit/WebKit/blob/main/Source/WebKit/UIProcess/API/Cocoa/WKWebViewPrivate.h).
