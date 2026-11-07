# Final integration on current main — #12990

The 25 local delivery checkpoints through `1d5538d827` were rebased cleanly
onto `2b6ab2dd16`, producing code checkpoint `3373eee106`. Both sides' changes
are retained. Main's shared `walk.coax` now breaks a near-half-circle turn
tie consistently; its CDP launcher/process ownership also changed. These
intersections justify fresh current-artifact travel/replay observations.
Unchanged cost, ownership, failure and mutation gates keep their earlier
qualified builds and are not represented as rerun results.

Before publication, main advanced once more to `3fc28b2f30` for an unrelated
experimental-meadow asset showcase. All 26 delivery commits rebased cleanly
again; `rebased-final-range-diff.log` records this second mapping. Its thirteen
changed paths do not intersect the qualified streaming sources or their
dependencies. The tested source bytes and artifacts remain unchanged, so no
passing gate was repeated for that asset-only advance.

`rebased-range-diff.log` records the old/new checkpoint mapping. The original
unpublished commit objects remain in
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/streaming-pre-rebase.bundle`,
SHA `7296d0ef6a19e0dc615aebb506b09b07c448a724c0ace55909aa5ef9ab34a119`.
This preserves the source identities referenced by older receipts even
though rebase changed their public commit hashes. The companion source and
artifact SHA inventories pin the rebased inputs and retained outputs.

## Browser

The rebuilt world is **156,253 bytes**, SHA
`fd86e77a33a6202948aa7ddf67c7b9e7c6d994e32476645efa76d71a76065ca4`.
Scene, RCM1/HTP1 roots, reader Wasm, shader bytes and configuration retain
their [diagnostic/capture identities](walk-diagnostic-capture.md).
`rebased-stream-site-v1` serves this artifact with the final capture adapter.

Both visual tiers again complete **3,206 frames / 114 attempts**, 680
payload allocation/free events, 740 complete transport records and 117
plans/releases. Independent allocation/transport/tier oracles pass. They
preserve identical gameplay frames, content requests and final actor cells;
visual layout costs remain 2,484/2,532 final dynamic GPU bytes. Each transport
oracle refuses ten altered histories. Current ASan/UBSan/LSan CPU replay
matches **12,809 paid steps** and refuses six altered histories per tier.

| Tier | Frame p50 / p95 / p99 (ms) | Payload CPU peak | Sanitized CPU replay peak |
| --- | --- | ---: | ---: |
| 0 | 2.645 / 5.520 / 9.680 | 14,132 B | 14,819 B |
| 1 | 3.110 / 5.580 / 9.115 | 14,212 B | 14,883 B |

The final seven-fact capture fixture passes against this rebuilt world,
including held GPU completion, failed encoding, pre-allocation text-budget
pressure, identical retry pixels and zeroed compiler inputs. Its independent
oracle recomputes exact extents/reservations and observes the new 156,253-byte
input. Diagnostic/capture policy source is unchanged from its 124/124 killed
mutation gates. The earlier maximum-capacity diagnostic probe retains its
explicit earlier build scope.

## Native

`rebased-native-v2` builds the actual v7 Vulkan carrier against current
source. Both `rebased-native-qualified-{low,high}` runs complete the full
automatic-recovery route with **114 attempts / 12,931 paid ticks**, three
actual SDK-result-driven replacements, four device plans and zero final
owners. Each compares **691,200 image channels** across its three replacements.
Vulkan validation is enabled with zero warnings/errors. The injected result
provenance remains explicit; this is not physical GPU-reset qualification.

| Tier | Frame p50 / p95 / p99 (ms) | Actual Vulkan peak | Backing reservations / allocations / frees / releases |
| --- | --- | ---: | --- |
| 0 | 2.182 / 2.642 / 4.739 | 1,649,392 B | 192 / 192 / 192 / 192 |
| 1 | 2.182 / 2.723 / 4.671 | 1,651,952 B | 192 / 192 / 192 / 192 |

The independent native/backing oracles check all **776 backing events** per
tier, every source/descriptor/device requirement, actual allocation lifetime,
category/aggregate peak, source/pixel retention and final drain. Core/native
SDK static inventories are 874,009/854,086 bytes, separate from payloads and
physical allocation peaks. Sanitized CPU replay uses current Coax source and
compares the native accepted-history canonical observations after every paid
step; the accompanying CPU oracle independently binds that transcript and
rejects twelve altered histories per tier.

These runs extend the [188-case native matrix](walk-native-backing.md) with
current-main route integration. They do not relabel the older Lavapipe/failure
artifacts as rebuilt results. All frame timings here are concurrent
correctness measurements, not a quiet-machine live-FPS benchmark.

## Reproduction and retained failures

Artifacts share the existing November 7 root. `rebased-cpu-v1/replay` is the
new instrumented CPU executable; its browser and native reports retain their
own input hashes. The native build uses the already installed Vulkan loader
development package via `PKG_CONFIG_PATH`; qualification sets both
`VK_INSTANCE_LAYERS=VK_LAYER_KHRONOS_validation` and `VK_LAYER_PATH` to the
installed validation manifest directory. No package or system rebuild occurred.

The first build lacked that pkg-config path and stopped before linking.
The first two native routes completed effects but correctly refused their
qualification because the layer manifest was not discoverable; these are
failed observations, not passing results. The corrected manifest-path runs
above pass. An initial CPU-oracle invocation put its report away from the
required sibling `replay` executable; reports were copied next to the existing
executable before completing that oracle. Positive replay itself was not
rerun. These setup failures and logs remain in `rebased-evidence`.

All six book registries pass with unchanged statuses; the tools gate passes
**1,881 tracked tools**, zero missing firm reasons. The pre-existing broader
book-gate failures in the historical record remain unclaimed. This final
record changes no additional runtime behavior. The [delivery audit](delivery-audit.md)
and [remaining-adapter inventory](../../../aethyr/host/stream-adoption.md)
define the issue's bounded delivered scope and residuals.
