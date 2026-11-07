# Stream coordinator and controlled GPU replacement — November 7, 2026

This stage's frame-level replay is superseded by the separate
[canonical paid-step replay record](walk-state-replay.md). Historical
measurements and mutation results below retain their original scope.

**#12990 remains open.** The coordinator and replacement ownership boundary
now pass their complete root mutation gates. Actual browser probes cover a
controlled device replacement with retained sources and an upload validation
failure attributed to the exact resource attempt. This is not automatic
unsolicited-loss recovery, failed-replacement rescue or native GPU recovery.
Those, complete per-step replay, persistent HUD adoption and the remaining
host failure matrix still prevent completion and publication.

## Checked boundaries

| Gate | Result |
| --- | --- |
| `stream-walk.coax` loaded checks | 639 passed, zero failures |
| Coordinator root mutation | 945 killed, zero survivors |
| `stream-walk-recovery.coax` root mutation | 63 killed, zero survivors |
| SDK source/reason gate | 1,852 tracked tools, zero new without a reason |
| Current low/high full routes | Both pass actual HTTP, Coax codecs, WebGPU, readback and cleanup |
| Native CPU replay | Both current histories pass the existing instrumented native binaries, with leak detection |
| Independent current route oracles | Positions, identities, physical allocations, accepted events, cleanup and three altered histories per native replay pass |
| Explicit low/high device replacement | Both retain exact GPU readback bytes and cell state, with zero fetch requests |
| GPU validation-error ownership | The failed index owner never becomes ready; a later generation becomes ready after physical retirement |

`walk-coordinator-mutation.log` and `walk-device-mutation.log` retain the
complete gate results. The earlier 378-, 145- and 9-survivor results remain
historical failures. No mutant-equivalence annotations or waivers were used.
Imported checks remain enabled. Passing unchanged component gates from the
preceding commits are reused rather than rerun.

New checks cover media hysteresis at claim slot zero, nearest-voice ranking,
geometry/floor dependencies independently of lighting, exact final counter
values, ordinary-play failure retirement, every camera placement selector,
codec output refusal, and shape metadata/lane initialization. A four-cell
fixture uses actual body/collision admission and paid tick cadence, sleeps
every actor, edits its canonical saved state while asleep, and verifies
that waking uses that stored edit rather than a stale active cache. The
first tick reaching the active-actor peak is retained for diagnostics.

The consumer constructs its desired resource-root table before releasing
unwanted roots and acquiring replacements. The dependency's ready boundary
already gates collision readiness for a placement. Resource-role dispatch
and cell indexing avoid redundant threshold/offset calculations. Original
allocation tickets, resource epochs, load generations and borrower ownership
remain the authorities.

## Retained-owner replacement

`aethyr/world/stream-walk-recovery.coax` is an additive carrier over the same
coordinator. After the SDK stops frames, drains old effects and immutable
borrows, and destroys old objects, it stages a replacement renderer ledger
and GPU handles. Loading, loaded and ready visual owners receive new handle
generations; retiring resources and CPU-only placements receive none.
Capacity refusal publishes no staged prefix. Live borrowers, a mismatched
manifest, closing and exhausted epochs refuse. CPU leases, source identities,
load keys, cell stores, actor state and queued facts remain retained.

The SDK adapter uses the same buffer/texture upload routine for ordinary
loading and replacement. It destroys the actual old device and awaits its
`lost` promise, then requests a fresh adapter/device, rebuilds fixed storage
and pipelines, and uploads retained encoded geometry and decoded full mips.
It does not instantiate a new Wasm module or fetch source content again.
Frames resume after replacement without advancing the clock during rebuild.

The first hardware probe failed because Dawn marks a used adapter consumed;
requesting another device from that object was refused. The failed receipt
is `stream-walk-recover-first.json`. The implementation now requests a fresh
adapter. Explicit replacements on the current serialized-SDK path took
89.9 ms at low tier and 103.8 ms at high tier on the RTX 4080. Both rebuilt
ten dynamic visual resources. Before/after arrays of all 28 reported owner,
clock and allocation values match exactly; all twelve cell records match.
The mapped 640x360 color-target bytes have equal SHA-256:
`3a99a96b1955ba1f494682210adc5edc36e9259621534109be30686680448770`.
No source request occurs while fetch is deliberately unavailable. Subsequent
walking advances, and final CPU/audio/image/inbox/GPU owners drain to zero.

The probe enforces the existing ten-second observation limit. The explicit
SDK method does not yet provide automatic unsolicited-loss scheduling,
repeat-loss admission or failed-replacement rescue; no such runtime claim
follows from the successful controlled replacements.

## Exact-owner upload failure

The probe makes the actual 24-byte floor-index write exceed its GPU buffer.
Dawn returns a real validation error. An initial generic probe only checked
that some resource failed; its passing `stream-walk-upload-failure.json`
was insufficient. Tightening it to require owner 1 exposed an SDK race:
concurrent uploads pushed overlapping device-wide error scopes, and the
texture completion popped the index upload's scope. The texture was charged
with the error while the index could be published. The precise red receipt
is `stream-walk-upload-owner-before.json`.

GPU uploads, drawing and readback now use one device-effect sequence. Each
scope finishes and pops before the next owner pushes one. Source transport
and decoding remain asynchronous under the original four-load bound, and
their actual scratch owners remain retained until each effect finishes.
The corrected `stream-walk-upload-owner-after.json` identifies owner 1's
failed attempt, observes its physical retirement, then observes a higher
generation of the same source become ready. The failed generation never
becomes ready. The independent allocation/event oracle checks all 108
physical events, nineteen attempts, exact modeled allocation extents,
source hashes, and the destruction-before-replacement timeline. Actual
requested CPU storage peaks at 12,908 bytes; all nineteen attempts retire.
The retained pack host ends with its existing 8 MiB master and no readers
or source operations. This qualifies one real upload validation failure,
not every possible GPU/SDK failure.

## Current full routes and replay

Both serialized-SDK tiers repeat the 3,206-frame, 12,806-paid-tick route and
return to z=18.3974609375 after reaching z=767.9873046875. Gameplay frame
tuples and cell states match between tiers. Each has 114 attempts and 114
eventual physical retirements. Requested CPU peaks remain 14,132/14,212
bytes; declared dynamic resident peaks remain 18,171/18,299 bytes.

Measured frame-work p50/p95/p99 are 1.4/4.0/6.9 ms at low tier and
1.5/4.0/7.7 ms at high tier. These two headless routes ran concurrently;
they are accelerated-input work measurements, not uncontended RAF or
mobile-device qualifications. SDK/driver backing, allocator overhead and
process RSS remain separate from logical GPU texels and requested payloads.

The previously built instrumented native observers execute these new input
histories without rebuilding unchanged native source. Both compare every
record word, frame position, active/drawn/updated/resident count, and final
cell identity/counter, including cleanup. Both pass with leak detection.
Independent reserialization yields low/high observation digests
`8d72bd4a2abd99c004245571477dc55c4c716902c01f21bc24998d9919c7555b`
and `9294a01934217a00090282a51377e39f80d92d0d52cc9486958ae38337c2b184`.
These cover the stated observations; a complete canonical state digest
after every individual paid step remains outstanding.

## Identities and reproduction

The CC0 corridor scene/manifest identities and hardware/compiler profile
are unchanged from the preceding record. Source pins are in
`walk-boundary-source-sha256.txt`. All large artifacts live below
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`.

| Artifact | SHA-256 |
| --- | --- |
| `stream-walk-recovery.wasm`, 121,571 bytes | `3e7c21d9681991310445b31cf6c4e330ef96fb45a08270c2296da0171e6afad3` |
| `stream-walk-browser-serial.json` | `a93feba35a6f8ac0b9252c183180cc40717f5394001a86c9a0f5f30a9be29c6f` |
| `stream-walk-browser-serial-high.json` | `7e353022a19462d7a2f9c92c726f10fc478bedf5eb84aedc9dc51287dc2ce63b` |
| `stream-walk-recover-serial.json` | `0ea7c99dabb413ec054de69307452bb97bc9537a2cbcd55363c64960ca63b86b` |
| `stream-walk-recover-serial-high.json` | `274b9cea7aa6b3ea260ab48547f06acbe0a854eb3f925c1db2cdb61ae94a5550` |
| `stream-walk-upload-owner-after.json` | `4fb77afca52824b99f5e5680c4cfdf3c7548fa0d2b858fc5d579e658c9a78cbe` |

Route companions retain PNG readback, physical allocation events and per-load
stage timings. Native `replay-serial.json` files live beside their unchanged
binaries in `native-replay-qualified-san/` and
`native-replay-qualified-high-san/`. Compact independent oracle results are
retained here as `walk-serial-low-oracle.json`, `walk-serial-high-oracle.json`,
`walk-serial-tier-comparison.json` and `walk-upload-owner-oracle.json`.

```sh
coax/bin/coax mutate aethyr/world/stream-walk.coax --library --root-only --jobs 8
coax/bin/coax mutate aethyr/world/stream-walk-recovery.coax --library --root-only --jobs 8
coax/web/wasm.sh aethyr/world/stream-walk-recovery.coax aethyr/host/tests/stream-walk-recovery-face.c OUTPUT.wasm
coax/web/tool.sh aethyr/tools/stream-walk-browser.coax BASE_URL SCENE_JSON OUTPUT.json --recover
coax/web/tool.sh aethyr/tools/stream-walk-browser.coax BASE_URL SCENE_JSON OUTPUT.json --recover-high
coax/web/tool.sh aethyr/tools/stream-walk-browser.coax BASE_URL SCENE_JSON OUTPUT.json --upload-failure
python3 aethyr/tests/streaming_walk_oracle.py --upload-failure SCENE_DIR OUTPUT.json
```

The separate stalled-complete-prefix probe also passes on the updated
coordinator before SDK serialization (`stream-walk-stall-qualified.json`).
Automatic/native recovery, hostile source completion/supersession replay,
complete per-step state recording, the persistent HUD and final consumer/
teaching inventories remain required. The fixture shader's numerical and
mutation coverage is separate from the two authority gates above. No issue
is closed by this record.
