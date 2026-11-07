# Native Vulkan route and controlled replacement — November 7, 2026

**#12990 remains open.** The same engine-owned corridor now runs actual
native local-file loads, canonical decoders, Vulkan uploads/draws,
retirement and controlled physical device replacement. This closes the
previous absence of native GPU observations; it does not qualify automatic
loss handling or complete physical backend cost planning. Work starts from
local checkpoint `487260a112` in the reused worktree.

## Measured consumer

`aethyr/host/tests/stream-walk-native-gpu.coax` drives the existing
`SwWalk`, RCM1 admission, semantic queries, entities, navigation, resource
owners and draw list. Coax still makes every readiness, borrowing,
fixed-step and replacement decision. The C carrier only executes native
allocation and the existing `verse/gpu/vkc`/`vks` SDK effects. It does not
introduce another cell controller, codec, clock or resource allocator.

The original corridor-v2 scene root is
`ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810`.
Actual local sources are authenticated per load. The four original
manifests and their source identities remain unchanged. Vulkan companions
are selected explicitly by backend/tier, rather than treating the browser
zero-upload-scratch model as a native allocation plan.

Every run performs 32 stationary four-step warmup frames, controlled
replacement, 1,600 four-step frames toward the far end, 1,600 back, and
three single-step cleanup frames: **3,235 frames and 12,931 paid ticks**.
All 64 resources are visited in **114 attempts**, with **776 allocation/free
events**. Body/entity identities stay stable across sleeping/unloading;
the first cell's stored state advances after returning. Each final resource,
inbox, renderer, decoded image/audio and concrete SDK owner drains.

Normal hardware runs use coderos-4080's RTX 4080. Full sanitizer runs use
Lavapipe, two software workers, ASan/UBSan and leak detection. All four
runs enable Khronos Vulkan validation and report zero errors/warnings;
software timings are not hardware performance results. Raw successful
logs contain no sanitizer diagnostics.

## Transfers and recovery

The native carrier maps the resource's declared upload span as the actual
Vulkan staging buffer. Its original staging/inbox lease and a concrete
renderer buffer grant precede creation. Submission completes before
`sw_host_finish` releases the mapped allocation and its grant. The new
additive `vks_upload_buffer` entry consumes this same caller-owned buffer;
the older `vks_upload` wrapper keeps its previous allocate/copy/submit/free
behavior. The corresponding `vks_read_buffer` entry avoids an unobserved
extra capture transfer. Existing SDK/public host ABIs keep their meaning.

Persistent uniforms and the color/depth surface have separate original
renderer grants. Capture and replacement transfer storage also receive
temporary grants. Pipeline/descriptor caches close before their bound
resources; borrowed draw resources survive until the actual queue fence.

At paid tick 128, each run captures the complete 320×180 RGBA8 target,
canonical simulation SHA-256 and a row/length/source-byte digest. It closes
the actual old device and GPU objects, invokes the existing atomic GPU
handle replacement, opens a new device, recreates the surface and reuploads
all **10** retained visual owners. It then draws without another paid
frame, source read or resource load. All **230,400 readback bytes** match.
Canonical state and retained-source identity match too. The resumed route
runs on that replacement device, and both devices close finally.

The common recovery image SHA-256 across both tiers and both devices is
`2adea1077eb3dfd6778879847f89b6f5368a6538d63c17501a4408b92a85b316`.
Its raster is nonuniform and was visually inspected. This is full-image
replacement equality for this view, not independent shader numerical
qualification or browser/native image parity.

Both hardware tiers produce identical movement, accepted completion/event
records, active/drawn/updated/voice counts and actor identity checkpoints
at every corresponding frame. Their immutable manifest identities and
layout costs differ. Canonical state hashes bind those costs and are not
claimed identical across the two manifest profiles.

## Memory and timing scope

The original 32 KiB resource cap and seven named lines still admit the
logical CPU/GPU/scratch model. Physical allocation traces match every
source, decoded, 168-byte plan, decoder work and native transfer extent.
An independent oracle reconstructs each allocation lifetime, source SHA,
dependency publication, binary32 movement sample and complete readback.
Compact oracle files retain owner/category peaks and per-stage timing
quantiles. These are unpaced 320×180 headless observations including the
state observer/report construction, not interactive window/phone FPS.

| Device / tier | CPU resource peak | Explicit Vulkan peak | Replacement ms | Frame ms p50 / p95 / p99 |
| --- | ---: | ---: | ---: | --- |
| RTX 4080 / 0 | 14,139 | 894,672 | 126.14 | 1.110 / 1.283 / 2.491 |
| RTX 4080 / 1 | 14,232 | 896,720 | 158.72 | 1.117 / 1.660 / 2.550 |
| Lavapipe + sanitizers / 0 | 14,139 | 699,080 | 56.17 | 10.420 / 11.075 / 18.815 |
| Lavapipe + sanitizers / 1 | 14,232 | 700,104 | 57.65 | 10.377 / 11.025 / 18.813 |

Bytes are reported separately by owner/category in the compact oracles.
Transient CPU staging peaks at 1,728 bytes in each run; fixed renderer and
capture storage contributes to the explicit Vulkan column, not the 32 KiB
logical dynamic-resource line.

The carrier additionally records exact explicit Vulkan allocation extents
from `VkMemoryRequirements.size`, including load transfers, fixed targets,
replacement transfers and capture buffers. Those extents are larger than
logical texels/buffer payloads. They are measured engine-requested backing,
**not opaque SDK storage** and **not proof that physical category budgets
equal the current RCM1 logical costs**. Backend cost/pool reconciliation
remains required before claiming physical native budget qualification.

Native fixed core/face metadata is 775,477 bytes; Vulkan carrier metadata is
21,466 bytes. Shader source/word storage, the file-read buffer, diagnostic
report/readback arrays and native tool stack are separate fixed observer
storage. Driver/pipeline internals, malloc overhead, physical memory commit
and process RSS are not estimated as zero. The Vulkan SDK allocation peak
includes its explicit temporary transfers; the precursor `*-current`
records without the final staging renderer grant are historical evidence,
not the final source join.

## Verification and provenance

`stream-native-conformance.coax` has a complete root mutation result of
**23 killed, zero survivors**, with no equivalent annotations. Unchanged
coordinator 958/958, canonical serializer 185/185, replacement 63/63 and
recorded-source 76/76 results are reused. The native independent oracle
also refuses 22 modified histories per run: image corruption/truncation,
state/source changes, false CPU/GPU counts, allocation extent/generation
and lifetime changes, missing completions, wrong ready keys, identity
changes, omitted steps, changed motion/ticks, final owner counts, schema
and invalid timing. Its first generation-negative attempt raised KeyError;
explicit lifetime membership now produces the required assertion refusal.
The earlier failed harness logs remain retained.
The four final oracles reject 88 altered histories in total.

Coax 0.52.0 emits the actual native C/header and SPIR-V; GCC 15.2.0 compiles
the SDK. Shader SHA-256 is
`6f24ad86c28c902f456d761778d7b0f0f62cac382e5221b2d1f257e5d1502410`.
The unchanged product passes `spirv-val --target-env vulkan1.1`. Exact
source, compiler, SDK, executed binary, scene, raw receipt and image pins
accompany this record. The source fence is checked after execution.

Artifacts live under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`:

- `native-gpu-final/` and `native-gpu-final-san/`: generated C/header,
  actual executable and shader.
- `native-gpu-{low,high}-final{,-san}/`: raw JSON, complete before/after
  RGBA8 readbacks and retained execution/independent-oracle logs.
- Earlier `native-gpu-v1`, `*-route-v1`, `*-current` and warmup directories
  preserve development observations; they are not substituted for final
  source-matched results.

All commands are bounded below 15 minutes: builder subprocesses at 180 s,
hardware routes at 180 s, sanitizer routes at 240 s, mutation at 890 s.
The [carrier guide](../../../aethyr/host/stream-native.md) gives invocation
and observation fields; the runtime record pins the installed SDK paths and
environment. The tools reason gate passes with 1,856 tracked tools and zero
new files without a reason. All book registries pass with unchanged counts;
the known unrelated chapter-87 listing drift is not rerun or called a pass.

## Remaining issue requirements

Automatic/repeated device loss, the deadline and failed-replacement rescue
are unfinished. So are stale/superseded callback and additional allocator/
SDK failure histories, actual category-pressure refusal with existing
resident content, persistent HUD adoption, shader reference/mutation
qualification, complete transport/transient/backend cost planning, and
the final consumer/residual-adapter audit. Native local-file effects are
synchronous; they do not qualify native HTTP/asynchronous workers or an
ordinary native realm/window/audio integration. This native GPU history's
paid-step marks are recorded but not yet independently replayed; prior
browser-history CPU replay retains its separate input/backend scope.
No issue or book chapter is promoted by this stage.
