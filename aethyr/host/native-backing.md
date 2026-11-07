# Device-bound native backing plans

The bounded stream carrier now adopts `native-backing.coax` over the existing
`backing-budget.coax`/`HostInboxQueue` authority. It plans every explicit
Vulkan allocation before resource loading. This supplements immutable RCM1
logical payload costs; it does not rewrite their revision or claim physical
VRAM residency, RSS, opaque driver storage or guaranteed allocation success.

`nb_begin` binds the authenticated native manifest and five device words:
vendor, device ID, API version, driver version and allocation-count limit.
It derives staging/destination descriptors for each GPU resource and seven
fixed roles: scene uniforms, HUD vertices/uniforms, color/HDR/depth targets
and readback. Each descriptor has an immutable content identity or a fixed
renderer role. There are 135 possible plans, with only valid roles queried.

The C face creates an unbound private SDK object for each descriptor and
uses the admitted factory to observe size, alignment, supported memory types
and selected type. Its planning callback returns `VK_NOT_READY`, so the
private object is destroyed without allocating memory. `nb_ready` refuses
until every expected role has exactly one valid plan. The actual factory
must subsequently reproduce the exact requirements before `nb_hold` reserves
bytes. Device replacement closes the old ledger, advances its existing
epoch and queries a new device-bound plan before re-upload.

The profile declares 2,097,152 aggregate bytes, 1,048,576 per allocation and
eight caps `[262144,65536,0,16384,0,0,524288,1572864]`. Categories 0–6 keep
their resource meanings; 7 is persistent renderer storage. Category 6 also
owns staging/readback. These are explicit allocation extents, independently
of the existing resource/scratch/presentation grants. Sixty-four concrete
leases bound concurrency. Every allocation requires a live matching lease;
only actual SDK success increments allocated bytes. Free is observed after
`vkFreeMemory`, then the reservation may be released. SDK prefix failure
retains its reservation until the admitted factory has returned after cleanup.

Parts are initial staging (0), retained destination (1), replacement staging
(2), private pressure clone (3), and fixed role (4). A clone requires the
matching live original index/generation/world epoch; another resource cannot
stand in for it. Equal allocation peaks retain their first responsible lease.

The new observation envelope is `aethyr.stream-walk-native-gpu.v7`. Its
`observationSchema` names the existing v2–v6 gameplay/recovery/failure
contract, and its mandatory `backing` is `aethyr.stream-native-backing.v1`.
Old observations preserve their identities and cannot silently carry this
new companion. The complete receipt hash binds its device observations,
plans, ownership history and original world history together.

`devices` has 18 words: epoch, the five device words, pressure flag, aggregate
cap, per-allocation cap, eight category caps, valid plan count. `plans` has
one array per device; each 13-word row is index, row, normalized part,
category, storage kind (buffer/RGBA8/RGBA16f/D32), logical bytes, width,
height, mip levels, required bytes, alignment, type mask and selected type.

Each 20-word `events` row holds tag, tick, plan index, resource/fixed row,
generation, world epoch, concrete part, category, logical bytes, required
bytes, alignment, mask, selected type, lease slot/serial/epoch, status,
post-event reserved bytes, allocated bytes and live lease count. Tags are
reserve (0), allocate (1), free (2), release (3), refuse (4), device open (5)
and close (6). Refusals have the canonical invalid lease `[-1,0,0]`.

`summary` has phase, reserved bytes, allocated bytes, live leases,
reservation peak, allocation peak, refusals, observer violations, next epoch,
ledger static bytes, observer static bytes, event count and device count.
The LP64 observer explicitly costs 803,432 static bytes, the authority 25,272,
and the memory-to-lease effects map plus active pointer 2,056. These are
included in the reported 854,086 SDK/carrier static bytes. The 4,096-event,
eight-device bounds refuse overflow rather than omitting history.

CPU replay authenticates the original manifest, derives the same descriptors,
admits the recorded device observations and replays every reservation and
allocation/free/release transition through the authority. It also reproduces
each accepted world step with real CPU codecs and allocator effects. It
does not query or simulate a GPU or reproduce observed wall times. The
independent Python oracle reconstructs descriptors, ownership, category and
aggregate totals, and delegates the existing source/motion/image checks to
their original observation-specific oracles.

The integrated qualification and remaining issue scope are in the
[native backing record](../../docs/verification/aethyr-streaming-12990/walk-native-backing.md).
