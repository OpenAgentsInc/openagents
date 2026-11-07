# Pre-allocation native backing gate — #12990

This checkpoint starts at `ca4a3d3dff` in the reused worktree. The issue
remains open. It adds an SDK admission point before `vkAllocateMemory` and
a Coax budget over the existing inbox lease authority. The actual corridor
still uses the original factory facade; this record qualifies components,
not physical-budget adoption in that consumer.

## Measured requirements precede allocation

The additive checked factory functions require a `VksMemoryGate`. They create
an unbound buffer/image, query its actual `VkMemoryRequirements`, select a
supported memory type and call the gate exactly once before allocation.
The callback sees the actual required bytes/alignment/type mask and selected
type. Only success permits `vkAllocateMemory`; every other result, including
positive not-ready/timeout, unwinds the unbound object and returns unchanged.
A null callback refuses before any SDK effect.

The existing checked factory entry points delegate to the same implementation
with no callback and preserve their prior signatures and behavior. Neither
`VksMemoryInfo` nor resource layouts change. Failed output structs and info
remain untouched. The new callback does not publish the private handle.

Vulkan defines the requirement's size as the required allocation extent,
alignment as an offset constraint and type bits as the supported memory types.
A dedicated allocation is used at offset zero. See
[requirements](https://docs.vulkan.org/refpages/latest/refpages/source/VkMemoryRequirements.html),
[the object query](https://docs.vulkan.org/refpages/latest/refpages/source/vkGetBufferMemoryRequirements.html)
and [allocation semantics](https://docs.vulkan.org/refpages/latest/refpages/source/vkAllocateMemory.html).
These extents are not inferred from compressed bytes or logical texel costs.

## Coax authority and lifetime

`aethyr/host/backing-budget.coax` supplies eight declared lines: existing
resource categories 0–6 plus persistent renderer storage at 7. It reserves
the exact observed bytes through `HostInboxQueue`; there is no new allocator
or GPU handle registry. Per-allocation, category, aggregate and finite-ticket
pressure refuse before a new reservation is published. The bounded component
supports at most 8 MiB per allocation, 32 MiB aggregate and 64 live tickets.

Typed observation validation distinguishes size/alignment/mask/index failures
and a selected type absent from the mask. Current, per-line and aggregate
peaks retain their responsible leases. Equal peaks keep the earliest owner.
Canonical storage supplies release size/category; forged, duplicate and stale
leases cannot reduce another owner's counters. Reset requires an empty ledger
and advances the existing queue epoch. Epoch exhaustion retires without wrap.

The caller retains its admission even if a later factory phase fails. It
releases only after the returned failure has physically unwound, or after
successful final use and destruction. Pending GPU use remains the existing
submission/transfer owner's responsibility; this gate does not invent
completion or claim allocation success.
Budget counters and peaks measure reservations, including attempts whose
later SDK allocation fails. They do not replace the actual allocation trace.

Checks preceded behavior. The final root gate kills **148/148 mutants**, zero
survivors. The initial 133-site gate left six survivors. Typed validation
reasons distinguish different invalid boundaries, and peak attribution makes
equal-peak retention observable. The 137-site intermediate gate passed; the
added device-epoch reset checks pass the final 148-site gate. No equivalents
were waived and different runs are not summed.
A final serial audit also requires the inbox's terminal retired flag to
survive a refused reservation, while its current allocation remains owned.
The strengthened check and corrected queue copy pass the same 148-site gate;
the SDK runs use that final emission.

## Actual effects and independent model

The Coax native driver calls the SDK through an independent C wrapper oracle.
The callback invokes the real Coax authority; C only transports the SDK
observations and executes effects. A counted allocation wrapper requires the
exact live lease before forwarding `vkAllocateMemory`. It also compares
requirements against a separate actual query and checks selected memory flags.

Each backend passes **30 new cases**: category refusal, aggregate refusal,
success, raw callback outcomes 1/2/−13, eight injected SDK prefix failures and
a missing-callback refusal, for both buffers and images. Failed SDK outputs
are poisoned. Before allocation, the private object exists but no memory,
map or view exists. Refusal calls allocation zero times; accepted attempts
retain their leases through cleanup, then release once. The existing facade's
19 actual error/success cases also pass on each backend.

| Backend / 64-byte buffer or 4×4 RGBA8 three-mip image | Required bytes | Alignment | Type mask | Selected type |
| --- | ---: | ---: | ---: | ---: |
| RTX 4080 buffer | 64 | 16 | 31 | 2 |
| RTX 4080 image | 1,536 | 1,024 | 3 | 1 |
| Lavapipe buffer | 64 | 64 | 1 | 0 |
| Lavapipe image | 768 | 64 | 1 | 0 |

Hardware uses ordinary GCC output on NVIDIA 595.71.05. Lavapipe uses
ASan/UBSan/LSan with leak detection and abort/halt on errors. Both enable
the installed 1.4.341.0 Vulkan validation layer and report zero errors and
warnings. The previously documented NVIDIA sanitizer loader limitation is
reused, not retried. Prefix error inputs are explicit SDK-result injections,
not physical exhaustion or device resets.

The independent counted Vulkan model additionally passes **38 gated cases**:
24 poisoned SDK phase failures, twelve success/callback-outcome cases and
two unsupported memory profiles that never call admission. Its original
24 poisoned failures, unsupported profiles, success/descriptor checks and
invalid/nonempty-call preservation remain passing. The full counted run is
sanitized and requires zero private objects, memory, maps/views and violations.

The pre-change red executable bypasses the new gate through the old facade.
It attempts allocation despite a zero category cap and fails the observer.
The first current build found a stale snapshot header ahead of the checkout
on the include path; snapshots now live in a separate precursor directory.
That compiler failure is retained separately, not counted as runtime evidence.

## Reproduction, artifacts and remaining work

Emit `backing-sdk-native-check.coax` as a library with header `backing-sdk.h`.
Link its generated C, `backing-sdk-native-fixture.c`, existing `vkc`, `vks`,
`vks-resource` and tool/platform effects with Vulkan, math and pthreads. The
fixture wraps the twelve create/allocate/bind/map/view/destroy/free/unmap
entry points listed in its source. Use the installed Vulkan headers/loader,
explicit NVIDIA or Lavapipe ICD and validation layer; the software build adds
ASan/UBSan. The counted model emits `resource-sdk-check.coax` and links its
existing fixture and checked factory without a Vulkan driver. Commands are
bounded at 120 seconds for builds and 60/30 seconds for actual/model runs.

Private artifacts under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/backing-sdk-v1/` retain
generated C/header, final binaries, red/precursor binaries and source.
`backing-evidence` retains logs and current source snapshots. Source/artifact
manifests and compact raw case logs accompany this record. Tools and all six
book registries pass. No chapter status, issue closure or push occurs here.

Next is actual corridor adoption for streamed, staging, fixed-renderer,
replacement and readback allocations, with device-bound plan/receipt identity,
pressure/recovery histories and replay. Browser transport/container/transient
planning and the final consumer/residual-adapter/teaching audit remain required.
Opaque SDK/driver allocations remain separate; measured allocation extents
are not a claim about physical residency, RSS or guaranteed allocation success.
