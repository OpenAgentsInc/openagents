# Native streaming observation profile

`tests/stream-walk-native-gpu.coax` runs the engine-owned corridor through
the existing walker, resource owners, canonical codecs and draw list.
`tests/stream-walk-vulkan.c` executes headless Vulkan effects with the
existing `verse/gpu/vkc` and `vks` SDK. It creates no window, input capture
or audio output. This is a bounded native observation carrier, separate
from the ordinary realm host and the published host-v1 ABI.

The Coax driver authenticates the pinned scene, Vulkan RCM1 companion and
actual local container/source bytes. Each normal load reads its source
after admission; the world is not eagerly made resident. Native HTTP and
asynchronous file workers are outside this profile. The browser consumer
retains its separate actual HTTP observations.

## Ownership and effects

Coax controls selection, the fixed-step clock, load/retirement commands,
render borrows, fixed grants and replacement order. The SDK binds admitted
48-byte vertices, u32 indices, decoded RGBA8 mip chains and the Coax shader.
It holds every render borrow through the actual submission fence, then
returns it. Cached pipeline descriptors close before a bound resource.

In this carrier only, `SW_VULKAN` makes the existing SDK allocation field 4
a mapped `VkcBuffer`. Its resource staging lease and a concrete renderer
buffer grant both precede creation. The owned `VksTransfer` consumes that
same allocation, without another transfer copy. Coax polls its submission;
finish/destroy cannot release scratch while the transfer is owned. After
retirement, `sw_host_finish` releases it and the fixed grant releases afterward.
The ordinary malloc/free and browser carrier retain their prior path.

Streamed buffer/image creation uses the additive checked
`verse/gpu/vks-resource` factories. They return SDK errors, unwind partial
creation and preserve the destination until success. Staging and retained
resource factories are adopted; surface/pipeline/readback factories remain
separate adapters. The
[factory record](../../docs/verification/aethyr-streaming-12990/walk-native-factories.md)
qualifies SDK failure prefixes and actual successful corridor use. The
[owned upload record](../../docs/verification/aethyr-streaming-12990/walk-native-transfers.md)
adds buffer/image pending views and held-observation early-release guards.
The [integrated SDK record](../../docs/verification/aethyr-streaming-12990/walk-native-sdk-failures.md)
qualifies scoped staging/destination/submission errors and upload-poll stops,
including the unchanged ten-second deadline, on both tiers/backends.
The SDK's new admitted-factory entry points can call the shared
[backing budget](backing-budget.md) with actual requirements before allocation.
The current v7 carrier [adopts that boundary](native-backing.md) for streamed,
staging, fixed-renderer, replacement and readback allocations. Original v2–v6
observations retain their historical logical-only scope. New v7 receipts bind
those observation contracts to a mandatory physical backing companion.

Persistent uniforms and the 320×180 color/depth attachments have separate
existing renderer grants. Readback and replacement transfer buffers each
receive a temporary grant before allocation. `vks_read_buffer` reads into
the already owned mapped allocation. Source/decoded bytes remain retained
under their original resource tickets.

The additive presentation carrier also owns a 98,304-byte CPU HUD vertex
span, the same-sized GPU buffer, a 256-byte uniform buffer and a linear
FP16 color target. World and premultiplied HUD draw into FP16; a separate
Coax fullscreen pass samples it once into the original UNORM display.
The surface's six fixed grants (0, 1, 2, 6, 7, 8) remain live until physical
teardown finishes. The SDK observes missing grants at close; final-zero
counts alone cannot prove this ordering. Area retirement never destroys
the HUD pipeline or buffers.

The observer records dynamic allocation/free events and per-stage timing,
logical GPU bytes, actual `VkMemoryRequirements.size` totals/peak, and
fixed core/SDK metadata sizes. Explicit Vulkan backing is measured; it is
not called opaque driver memory. Internal pipeline/driver allocations,
allocator overhead, process RSS and the observer's fixed stack/report
storage are separate. RCM1 categories still describe logical payload bytes;
the additional v7 backing budget admits actual Vulkan extents under its own
named/category/aggregate limits. The two byte domains are not interchangeable.

## Controlled replacement

After 32 warmup frames the driver captures all 230,400 RGBA8 bytes, the
canonical simulation SHA-256 and a row/length/retained-source digest. It
drains physical effects, closes all old pipelines/resources and the old
device, invokes the existing atomic GPU-handle replacement, opens a new
device, and rebuilds from the retained CPU spans. No source read or paid
simulation frame occurs between the compared captures. Sources, queued
logical ownership and canonical state remain in their original carrier.

The complete image must match byte-for-byte and contain more than one
color. Every granted visual owner must upload again. A continued draw and
the optional full 768 m out-and-back route then use the replacement device.
Final cleanup requires all resource, inbox, decoded image/audio, renderer
and concrete SDK owners to reach zero. The additive
[result-driven draw profile](../../docs/verification/aethyr-streaming-12990/walk-native-loss.md)
now handles reported draw loss automatically and preserves submitted owners
through a timeout. Scoped upload errors/deadline stops now have separate v6
histories. The wider device/factory failure surface and native rescue remain
unqualified exits.

`stream-native-conformance.coax` admits sixteen independently supplied
observation words: version; nonempty initial GPU set; equal full image;
nonuniform raster; equal canonical state; retained CPU count/source identity
and no new reads; no paid frame; old device closed; all grants reuploaded;
epoch advanced; resumed draw; validation errors; validation warnings; final
SDK objects; final ledger objects/words; complete drain with validation
enabled. An additional checked observation requires zero fixed-grant
teardown violations. Its complete root mutation gate kills 25/25 without
equivalents. HUD receipts use `aethyr.stream-walk-native-gpu.v2`, preserve
the sixteen original verdict words, and append GPU observation word 19
for these violations. They also bind all three SPIR-V identities and the
explicit display profile. The oracle continues to accept historical v1
receipts with their original nineteen-word GPU observations.

## Run and inspect

```sh
coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_BUILD [--sanitize]
NEW_BUILD/stream-native SCENE_DIR SCENE_SHA256 NEW_BUILD/stream-draw.spv NEW_OUTPUT TIER --route
python3 aethyr/tests/streaming_native_oracle.py SCENE_DIR NEW_OUTPUT NEW_BUILD/stream-draw.spv --fault-checks
```

The installed Vulkan SDK must be discoverable through pkg-config; the
receipt requires `VK_LAYER_KHRONOS_validation`. Each command has a bounded
deadline. The build uses the installed C compiler and Coax's own SPIR-V
emitter. No external shader compiler or source-asset tool runs.

The independent oracle reconstructs allocation lifetimes/costs, dependency
publication, exact binary32 walking, source identity and every recovery
readback byte. It rejects 22 altered v1 histories and 30 altered v2
histories, including early HUD release, changed shader/profile identity,
wrong draw counts, changed HUD bytes and false teardown claims. It retains paid-step
canonical hashes. The [native-history CPU replay](../../docs/verification/aethyr-streaming-12990/walk-native-cpu-replay.md)
now consumes actual native v1/v2 histories using their original backend-1
RCM1 companion and compares every paid step, record and final actor value.
Its v3 output is distinct from the existing browser-input v2 replay.
This does not independently reimplement the simulation serializer or infer shader numerical
reference qualification from a nonuniform image.

Current hardware/software scope and exact pins are in the
[native receipt record](../../docs/verification/aethyr-streaming-12990/walk-native-gpu.md).
The [persistent HUD record](../../docs/verification/aethyr-streaming-12990/walk-persistent-hud.md)
adds full-pixel reference comparison, two browser rasters, unchanged
canonical histories and explicit display-storage ownership.

## Owned submissions and reported draw loss

The native carrier now uses the additive `verse/gpu/vks-submit.{h,c}` API
for its draw submission. A successful begin means work was accepted, not
that it finished. The command pool and fence survive pending or unknown
errors. Retirement refuses while pending and rejects a different device.
Coax maps result/in-flight observations through `render/submission-policy.coax`;
`render/loss-policy.coax` owns rebuild counts, the absolute recovery deadline,
explicit failure/timeout stops and paid-frame permission. Unknown failures
do not invent a loss event or reset its rate window.

Surface/pipeline/readback SDK helpers still have legacy fatal paths outside
the checked draw/upload boundaries; ordinary host-v1 adapters are not migrated. Physical device-fault
qualification is distinct from the actual Vulkan rendering and SDK-result
injection used here. See the [record](../../docs/verification/aethyr-streaming-12990/walk-native-loss.md)
for the exact supported surface and remaining limits.

The runtime parser additionally accepts `--auto-loss`, `--auto-repeat`,
`--auto-restart`, `--auto-timeout`, `--auto-late`, `--auto-fail`, `--auto-oom` and `--auto-route`
in the optional final argument. The two route tiers keep the original scene,
source and backend-1 manifest identities. These observations use schema
`aethyr.stream-walk-native-gpu.v3`; historical controlled v1/v2 observations
retain their own reader paths. CPU replay admits all three named native
schemas, preserving its existing v3 output. Zero-step interruption frames
remain inputs to replay and do not invent paid-step marks.

`--auto-late` delivers a completed successful SDK observation after ten
seconds. The caller checks the deadline after that return as well as while
pending, so late completion cannot admit another paid frame. This bounds
acceptance; it does not preempt a synchronous SDK call while it is blocked.

## Texture category pressure

`--category-pressure` adds a decoder-derived 8×8 texture request after the
normal warmup. The request calls the existing admission function with the
live corridor's budget, inbox and renderer registry. Its separate one-entry
RCM1 companion declares 939 retained bytes and 638 scratch bytes. Staging and
aggregate room remain available; the 512-byte texture line refuses before
any candidate resource allocation or publication.

The qualifier compares the complete existing image and ownership before and
after refusal, continues eight paid movement steps, then exercises controlled
device replacement and full cleanup. Native observation v4 records this
additional boundary. CPU replay requires its pressure metadata, reissues the
same source-derived request between warmup and movement, and matches every
paid-step digest including the counted refusal. Earlier observation schemas
remain readable. The [record](../../docs/verification/aethyr-streaming-12990/walk-category-pressure.md)
names the exact measured tiers and limits.

## Failed allocation, source and decode effects

Normal load effects now report a typed completion even when allocation,
source read/copy or decoding fails. Partial physical allocations retain their
attempt identity and image/audio reservations until shared retirement. The
owner cannot retry until destruction is accepted. Later allocation phases
are skipped after the first failure. Temporary transfer grants release only
after the completed synchronous effect and its scratch cleanup.

`native-load-policy.coax` preserves the first failed phase. The additive
native `sw_host_finish_code` carries existing `ResourceCostStatus` values;
the earlier boolean SDK-failure facade keeps its original meaning. The
qualifier modes `--load-alloc-0` through `--load-alloc-4`, `--load-source` and
`--load-decode` exercise explicit allocator/source/scratch inputs, successful
retirement/retry, rendering and controlled recovery. Native observation v5
requires their bound effect record. CPU replay reproduces the same partial
allocation prefix, source outcome or decoder refusal before accepting the
typed completion. See the [record](../../docs/verification/aethyr-streaming-12990/walk-native-load-failures.md).

Streamed Vulkan resource factories and upload submissions return SDK errors
and retain pending owners. The separate v6 qualification below covers their
scoped error inputs. Surface/pipeline/readback adapters remain legacy.

## Integrated SDK error and upload-stop inputs

`native-sdk-fault.coax` admits `--sdk-{buffer|image}-stage-{1..4}`,
`--sdk-{buffer|image}-create-{1..4}`, `--sdk-{buffer|image}-submit-{1..6}`
and `--sdk-{buffer|image}-poll-{1..3}`. The independent linker wrapper binds
the first matching streamed attempt's row/generation/epoch. Prefix inputs
execute earlier real Vulkan calls, poison failed outputs and require complete
private retirement. Staging errors finish as capacity 3; destination/submit
errors finish as decode 5, with retirement before a newer retry.

Poll inputs report an unknown error, device loss after actual fence success,
or not-ready for 10.1 seconds before observing the real fence. They stop paid
frames, retain pending owners/grants until terminal observation, and close
without publishing ready content. These are explicit SDK observations, not
physical OOM/reset or physical GPU-latency claims. The v6 `sdkFault` companion
binds scope, private-child counts, real/reported results, stop snapshots,
source hashes and timing. CPU replay reproduces allocation/codec prefixes
and every paid step without executing GPU effects or wall time. All 136
two-tier hardware/software histories and healthy-route regressions pass;
the [record](../../docs/verification/aethyr-streaming-12990/walk-native-sdk-failures.md)
pins exact results and limitations. The separate [browser callback record](../../docs/verification/aethyr-streaming-12990/walk-callback-retirement.md)
qualifies held unwanted responses and old callback redelivery against newer
generations, plus exact accepted-history CPU replay. The native local-file
profile remains synchronous. The [retained adapter inventory](stream-adoption.md)
states its input-window, SDK and unsupported transport scopes alongside the
later browser container/capture qualification.
