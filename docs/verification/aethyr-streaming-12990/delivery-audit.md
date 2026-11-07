# #12990 delivery audit

This is the requirement-by-requirement scope audit for the shared
resource-cost/streaming pipeline. The first adopted consumer is the original
CC0 corridor: twelve 64 m cells, 768 m out and back, 64 resource rows and two
visual tiers. The complete world has 62,106 retained model bytes against a
32,768-byte aggregate; this is actual residency pressure rather than a world
that already fits its cap. Browser HTTP/WebGPU and native local-file/Vulkan
adapters use the same Coax resource owners. Actual native CPU codecs replay
each recorded accepted history; they do not emulate GPU effects.

The commit containing this audit builds on the local checkpoints through
`ebc40a8e47`; each linked record pins its own source/build/content identities.
Current source/artifact SHA inventories accompany the
[diagnostic/capture record](walk-diagnostic-capture.md). Historical failures
remain recorded. Passing unchanged boundaries are reused, not rerun merely
to claim a fresher timestamp.

The [rebased integration](rebased-integration.md) supersedes final build
identity and route measurements after adopting current main's walking
half-turn rule. Earlier policy/failure gates retain their original source
identities; the rebase map and preserved source bundle connect those local
checkpoint hashes to the published history.

| Issue requirement | Delivered boundary and evidence |
| --- | --- |
| Versioned, content-addressed resource/dependency costs | `world/resource-cost.coax` uses real image/mip, WAV, collision, geometry and actor codecs. RCM1 binds model revision, complete source SHA, dependency digests, type/category/backend/tier/alignment, encoded/decoded/GPU/object/scratch costs. Unknown model/format and noncanonical or corrupt costs refuse with typed results. [Format, small native/Wasm oracle and mutation evidence](README.md#format-and-accounting). VPK1/VPK2 identities and published ABIs are preserved. |
| Shared dependencies and concurrent scratch | Existing inbox leases/renderer ledger sit behind `resource-admission.coax`; `resource-stream.coax` adds concrete retained owners, claims and immutable borrowers. Failed/cancelled/stale effects retain their original reservations until joined retirement. [Owner journeys](README.md), [actual callback retirement](walk-callback-retirement.md), [native owned transfers](walk-native-transfers.md) and [HTP1 observations](walk-transport-binding.md) cover release order and actual concurrent backing. |
| Seven named categories and atomic category refusal | Geometry, textures, collision/navigation, baked lighting, audio, actors and staging preserve aggregate/ticket/epoch authorities. [Category-pressure receipt](walk-category-pressure.md) refuses the additional texture while aggregate room remains, retains existing pixels/owners and replays the counted refusal. [Native physical backing](walk-native-backing.md) separately reserves actual allocation requirements before allocation. |
| Named interest/pool/work profiles | `stream-profile.coax` declares 32/64/96 m draw/simulate/resident rings, 8 m hysteresis, one new cell per paid step and four unfinished resource attempts. Texture/light/audio radii are 96/80/64 m; far simulation updates every fourth paid tick. Four streamed actors, one persistent actor, two collected voices and zero unused animation/sensor/effect/dynamic-light slots have explicit limits. [Profile and fallback rules](README.md#named-cell-and-interest-profiles), route activity/refusal/pressure counters and [persistent HUD](walk-persistent-hud.md) record actual use. |
| Bounded neighbor prefetch and persistent services | Placement hints activate within 8 m of a neighbor edge under 4 KiB/one-root quota. They grant no spawn/simulation authority. Shared dependency closure orders wake/retirement; player/camera/HUD outlive area content. Texture, lighting and audio requests have independent owners and collection radii. [World module inventory](../../../aethyr/world/README.md) and [HUD/replacement receipts](walk-persistent-hud.md). |
| Fixed-step async acceptance and same-history replay | Digest, generation, world epoch, actual owner and wanted closure gate readiness at paid-step boundaries. [Canonical state replay](walk-state-replay.md), [browser/native CPU replay](walk-native-cpu-replay.md) and [retained receive sources](walk-receive-fault-replay.md) compare every declared-profile state after each paid tick. The new complete routes add 25,618 matched paid steps. Different live completion schedules are explicitly not promised to agree. No client-render-readiness authority is added to shared gameplay. |
| Actual consumer, navigation and stable identity | `stream-walk.coax` uses the existing cells, controller, semantic query/navigation boundary from #12989 and stable sleeping entity state. Actual two-tier browser/native routes traverse both directions, cross cell/LOD edges and retain all twelve final actor identities. [Consumer and profile](../../../aethyr/world/README.md), [native adoption](walk-native-gpu.md), [tier comparison](diagnostic-capture-tiers.json). |
| Allocation trace agrees with all supported fixture layouts | Independent oracles reconstruct each exact RCM1 allocation, mip/tier layout, owner/category peak, source/decode/upload stage and concurrent scratch. Current browser routes each contain 680 actual allocation/free events and 117 whole-transport plans/releases. Native v7 adds 40,072 backing events over its 188-case matrix. Logical payload, arenas/static storage, actual physical requirements and opaque SDK/driver storage are reported separately. [Current browser](walk-diagnostic-capture.md), [native physical](walk-native-backing.md) and [adapter/memory inventory](../../../aethyr/host/stream-adoption.md). |
| Malformed, failed, cancelled, superseded and reordered loads cannot publish prefixes/leak | [Retained corrupt/oversize/cancel/reorder histories](walk-receive-fault-replay.md), [failed uploads](walk-upload-failure-replay.md), [callback supersession/redelivery](walk-callback-retirement.md), [native SDK/factory failures](walk-native-sdk-failures.md) and [HTP1 whole-source/constructor cases](walk-transport-binding.md) verify original owners, failed-generation non-readiness, joined cleanup and retry. Historical negative inputs remain part of those pinned gates. |
| Repeated route remains bounded; visual tiers preserve gameplay | Both current routes contain 3,206 frames / 114 attempts and match gameplay frames, content requests and final cell states; distinct image/GPU layouts are measured. Retained peaks are 18,171/18,299 bytes under 32,768. Hysteresis churn and work/pool limits are checked from actual route observations. [Tier oracle](diagnostic-capture-tiers.json), [route allocation/peak/timing oracles](walk-diagnostic-capture.md). |
| Native/browser load/unload/device recovery retains identity | Both adapters retain source/CPU owners, retire actual old-device effects and restore unchanged full pixels/cells with continued movement. Browser replacement makes zero source requests. Native RTX and sanitized Lavapipe receipts preserve their own manifests/device requirements. [Native backing/recovery](walk-native-backing.md), [automatic browser loss](walk-automatic-loss.md), [current browser recovery](walk-diagnostic-capture.md). Physical OOM/reset and durable native rescue are separate unqualified profiles. |
| Timings, bytes, counts, pressure and measured scope | Per-owner/category CPU peaks, physical backing/reservation peaks, four loading-stage durations, frame p50/p95/p99, active/drawn/updated counts and load/refusal counters are retained in raw receipts and compact independent results. [Current browser measurements](walk-diagnostic-capture.md), [native measurements](walk-native-backing.md). Concurrent correctness runs are not quiet-machine 60 FPS qualification. |
| Coax checks, bounded target gates and published status | All new runtime policies and tools are Coax; browser effects and independent Python oracles retain their stated firm reasons. Current new authority gates kill 124/124 root mutants, no survivors/equivalent waivers; earlier unchanged gates retain their zero-survivor receipts. Actual muted headless browser and ASan/UBSan/LSan CPU/native gates are bounded below 15 minutes. Chapters 16/38/53 and source registry describe actual evidence without promoting unfinished teaching. Broader existing book-gate failures remain separately recorded at `ebc40a8e47`. |
| Actual adoption or named residual adapters | Duplicate browser hash/span policy is removed on the HTP1 path; the native v7 carrier requires the shared backing authority. The [remaining-adapter inventory](../../../aethyr/host/stream-adoption.md) explicitly names legacy faces/general realm host, synchronous native source windows, private/unknown transport, separate rescue and unused/audio profiles. The issue requires one adopted consumer, not automatic migration of every realm. |

The browser trace now survives actual 2,048-event exhaustion and held effect
cancellation; incomplete history is never represented as complete evidence.
Capture copy/text peaks reserve before GPU allocation and retain owners
through the completion observation even on encoding failure. Static compiler
input extents are measured and byte inputs zeroed. Those final auxiliary
observations complete the inventory without claiming measured SDK/GC/driver
storage or RSS.

The named residuals bound this first delivered profile. Range-addressable
compressed delivery (#13001), general geometry/Hi-Z work (#12998), broader
realm migration and the full teaching epic (#12610) remain separate work.
