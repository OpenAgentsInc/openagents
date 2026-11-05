# Canonical Worklet allocation and acknowledged retirement

The v1 browser adapter now uses its shared Coax PCM owner for canonical
WAV planning scratch, compressed transfer/input storage and retained decoded
output. It reserves each measured extent before the corresponding copy or
decode allocation. The Worklet independently validates the same WAV plan
before decoding. Its successful install acknowledgement releases scratch
and transfer reservations; retained PCM remains owned through node cleanup.
Known Coax refusal releases temporary reservations. An uncertain failed
install retains its reservations until cleanup is acknowledged.

Retirement fences new commands, disconnects the node, clears the actual
Coax PCM borrowed views, frees all measured assets/workspace and replies
from that processor. Only that acknowledgement releases retained master
reservations. The product then closes its concrete AudioContext before
installing a replacement engine. A failed cleanup blocks replacement and
reports the error. Queue release closures capture only their scalar lease;
delivery closures capture only the admitted independent line and owner.

The real host receipt observes `[8,8,0,8,0,8,0,1,0,1]`: eight retained words
in master/node, no pending offer, then both counts zero, closed context,
and replacement with zero words and no old context. Its check-first
authority kills 18/18 root mutants without equivalents.

The separate actual SDK receipt passes all 20 refusal/cancellation fields.
A held 8,388,608-word reservation refuses canonical planning before PCM is
created; the Worklet stays at zero words and the offer lease is returned.
Releasing it admits the complete asset. Cleanup returns master/node to
zero. A second fresh context holds a real deferred decode: cleanup occurs
while its offer remains owned, then completing that decode returns -1,
releases the offer and cannot install into the retired processor. The
28-mutant observer has zero survivors or equivalents. The existing real
32 MiB allocation component remains separate physical-allocation evidence.

Actual muted gesture/suspend/resume/retirement and host.js replacement
regressions also pass. The unchanged shared WAV/DSP sources and earlier
48-asset synth-bank/sample gates are reused; no sample bound or alignment
changed. A first product invocation supplied a directory lacking the
fixture files and stopped before launch. An earlier diagnostic stalled
while reopening a processor in the same suspended context; the retained
trace is not a pass. Actual replacement creates a fresh context, which
the passing cancellation workload uses. An initial product-memory poll
also timed out; its raw pending observation remains retained.
The regression harness now bounds each asynchronous poll to one second
within its unchanged 15-second deadline, so one pending SDK query cannot
consume the entire polling loop. The final product receipt passes with
that bounded observation; no product deadline was relaxed.

The 64,095-byte reactor SHA256 is
`c0e43dc678737b15b9a268a91351183563334b6fda996107674d34f6d1dbff7d`.
Its actual generated C joins 18 pure Coax source files with zero differences
between the genuine historical c224 preparation and this product snapshot.
The original admitted compiler is unchanged SHA256 `598ddc2358d7fdefaead3a2add7947448f54d6935238acbbf4c0254e521a5c0e`;
this does not qualify a current compiler release. The ordinary size budget
passes. Exact C/header/object, source/SDK, Wasm, observer and receipt hashes
are frozen under
`~/.openagents/aethyrforge/host-contract-12869/browser-worklet-memory/`.

These observations qualify the canonical WAV path and cleanup boundary.
Everglade's ordinary default is the picked Ogg/Opus profile, whose codec
preflight, decoded output and complete retained-memory workload remain
open. Browser socket pressure, ordinary Vulkan integration, current primary
packaging and graphics admission remain separate exits. Both host issues
stay open and the desktop default remains web.
