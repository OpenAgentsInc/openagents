# Checked native resource factories — #12990

This checkpoint starts at `85095c5a7e` in the reused
`/home/christopherdavid/oa-open-issues` worktree. The issue remains open.
It adds checked buffer/image creation to the actual native corridor carrier.
It qualifies failure cleanup at the SDK factory boundary and successful
consumer adoption. It does not yet qualify an integrated factory-failure
history, pending uploads, surface/pipeline creation or physical cost admission.

## Effect boundary and adoption

The additive `verse/gpu/vks-resource.{h,c}` returns `VkResult` from buffer
creation, allocation, binding, mapping, image creation and view creation.
No failed call exits. Each output handle stays private until every required
SDK effect succeeds. An error unwinds only successfully created objects,
destroys the buffer/image before freeing its allocation, and preserves the
caller's empty destination and optional memory-info output. An occupied
destination or invalid call refuses before SDK effects. Existing fatal
`vkc_buffer`/`vks_image_levels` interfaces remain unchanged.

`VksMemoryInfo` returns the actual requirement size/alignment/type mask and
selected memory type on success. Whole allocations use Vulkan 1.1 dedicated
allocation records, including buffer/image identity. This is an observed
layout, not pre-allocation budget admission. The native device already
requires Vulkan 1.1. See [allocation requirements](https://docs.vulkan.org/refpages/latest/refpages/source/vkAllocateMemory.html)
and [buffer binding requirements](https://docs.vulkan.org/refpages/latest/refpages/source/vkBindBufferMemory.html).

`stream-walk-vulkan.c` now calls the checked factories for mapped resource
staging, replacement staging, retained mesh/index buffers and RGBA8 mip
images. Returned failure uses the existing allocation/load failure path.
Source validation precedes replacement staging, and a failed destination
factory frees any replacement staging. The owner is marked live only after
upload succeeds. No Coax admission, replay, schema or cost policy changes.

The existing legacy upload functions still wait through their fatal SDK
submission helper. Their timeout/error behavior is an explicit remaining
adapter, not qualified by the factory results here. Caller-owned resource
lifetimes, pending-use protection and category admission remain Coax's job.

## Checks before implementation

`resource-sdk-check.coax` drives the actual factory against the independent
counted `resource-sdk-fixture.c` Vulkan model. Its first failure against the
old factory exits 1 at `vkCreateBuffer`, rather than returning the expected
error. That red log and executable are retained.

The final counted check passes under ASan/UBSan/LSan:

- Two resource kinds × four SDK phases × three error inputs = 24 failures.
  Failed creation/allocation/map/view outputs contain poison handles or pointers.
  No poison enters the destination or cleanup, and every created prefix retires.
- Two incompatible memory profiles refuse without allocation or publication.
- Success preserves the exact resource descriptor, selected memory type and
  requirements. Buffer clearing covers exactly the admitted bytes, with an
  unchanged tail. A second creation cannot overwrite the existing owner.
- Sixteen invalid calls and thirteen individually occupied destination fields
  refuse before effects and preserve both outputs. Cleanup counters reach zero.

`resource-sdk-native-check.coax` plus the independent link-wrapper oracle
`resource-sdk-native-fixture.c` qualifies real driver-created prefixes. Each
explicit failure input is returned before its selected SDK phase; preceding
successful effects execute on the actual device. The checks compare the
successful backing size/alignment/type mask with a fresh driver query and
observe exactly ordered destruction of the created prefix. Nineteen cases
pass per device: sixteen host/device allocation-error inputs, two successful
creations, and a map-failed input. These inputs do not establish physical
OS/GPU memory exhaustion or physical device loss.

| SDK execution | Result |
| --- | --- |
| RTX 4080, NVIDIA 595.71.05, ordinary build | 19 cases, zero validation errors/warnings |
| Lavapipe, LLVM 21.1.8, ASan/UBSan/LSan | 19 cases, zero validation errors/warnings and clean sanitizers |

The instrumented NVIDIA probe cannot initialize the ICD: loader debug reports
that it cannot obtain `vkCreateInstance` from `vk_icdGetInstanceProcAddr`.
The ordinary build initializes and passes. Both failed logs are retained;
hardware sanitizer coverage is not claimed. No driver or system change was made.

## Actual consumer regressions

Both hardware tiers run `--auto-route` through repeated cell boundaries and
three reported-loss replacements. Both sanitizer/software tiers run
`--auto-loss`. The unchanged independent loss oracle checks source/manifests,
every allocation and lifecycle event, binary32 walking, category/owner peaks,
full image equality after replacement and final zero owners.

| Profile | Paid steps | Attempts | CPU peak | Explicit Vulkan peak | Frame p50 / p95 / p99 ms |
| --- | ---: | ---: | ---: | ---: | --- |
| RTX 4080 tier 0 route | 12,931 | 114 | 14,139 | 1,649,392 | 2.182 / 2.691 / 3.673 |
| RTX 4080 tier 1 route | 12,931 | 114 | 14,232 | 1,651,952 | 2.176 / 2.493 / 3.723 |
| Lavapipe sanitizer tier 0 loss | 139 | 18 | 13,168 | 1,258,440 | 11.317 / 25.521 / 26.681 |
| Lavapipe sanitizer tier 1 loss | 139 | 18 | 13,232 | 1,259,464 | 11.301 / 25.302 / 26.561 |

All four positive histories pass, with 72 altered GPU receipts rejected and
1,843,200 image channels compared across their eight replacements. The CPU
replayer built at `85095c5a7e` is unchanged and reused: all 26,140 paid steps
match, and 48 altered histories refuse. GPU effects are not replayed. The
first CPU-oracle invocation lacked its required sibling `replay` executable;
adding a symlink to that existing qualified binary allowed the negative
checks to execute. This was an invocation correction, not a source change.

Core/SDK statics remain 874,009 / 22,794 bytes. These observations do not
include all process/driver memory. Logical RCM1 GPU categories still differ
from measured physical Vulkan requirements. Software timings are sanitizer
observations, not a hardware frame-rate claim.

## Evidence and remaining work

Compact `factory-*-oracle.json` and `factory-*-cpu-oracle.json` files beside
this record pin runtime/replay results. `factory-source-sha256.txt` and
`factory-artifact-sha256.txt` identify source, compiler, generated C, native
executables, original sources, images and raw receipts. Private artifacts
are under `~/.openagents/aethyrforge/streaming-12990/2026-11-07/`:
`factory-sdk-v1`, `factory-native-v1`, `factory-native-san-v1`, the four
`factory-route-*`/`factory-loss-*` observations and `factory-evidence`.

The counted driver emits with `coax/bin/coax c
aethyr/host/tests/resource-sdk-check.coax --library`. Compile that output
with its fixture, `vks-resource.c` and `tool-native.c`, installed Vulkan
headers, `-I verse/gpu -I coax/platform`, `-lm -lpthread` and sanitizers.
The real-device driver additionally links `vkc.c` and Vulkan, wrapping
CreateBuffer, CreateImage, AllocateMemory, BindBufferMemory, BindImageMemory,
MapMemory, CreateImageView, DestroyBuffer, DestroyImage, DestroyImageView,
FreeMemory and UnmapMemory through the fixture. Exact sources and logs are
retained. Native consumer build/oracle commands follow the preceding native
loss record, using the updated build tool. Every command is below fifteen minutes.

No runtime Coax policy changed; its prior zero-survivor evidence at
`85095c5a7e` is reused. The tools check and book registry checks cover this
checkpoint; no chapter status is promoted. Browser sources and replay
sources are unchanged. No browser regression is inferred from native checks.

Remaining #12990 work includes integrated SDK failure receipts, owned
upload completion through timeout/error, actual stale/superseded callbacks,
complete physical/transport/transient cost planning, and final consumer,
supported-profile and teaching audits. Native persistent rescue and genuine
GPU-reset qualification are broader unqualified profiles. This checkpoint
does not push or close the issue.
