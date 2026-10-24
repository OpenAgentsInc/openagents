# Runtime ABI v1

Status: proposed normative baseline, not an implemented interface. This document
specifies the wasm32 boundary used by the trusted preview worker. All byte
counts are binary units. [Runtime](runtime.md) defines scheduling;
[contracts](contracts.md) defines authored and generation identities.

## Module and exports

The baseline module exports one nonshared `WebAssembly.Memory`, imports no
WASI, filesystem, network, clock, or browser functions, and accepts one live
session. Host input supplies time and randomness seeds. Its exported functions
use wasm `i32` parameters/results, interpreted as unsigned integers.

The portable engine identity is `af.kernel.1`, matching release manifests.
It maps to this binary ABI's major/minor 1.0 and `af_abi_version` value
`0x00010000`; `engineAbi` fields contain `af.kernel.1`, not the string `1.0`.
The engine artifact digest additionally pins the exact implementation build.
An ABI-major change needs a new portable identity; compatible minor capability
versions remain explicit in the module and adapter's supported-version table.

| Export | Signature and meaning |
| --- | --- |
| `af_abi_version` | `() -> u32`; returns `0x00010000` for major 1, minor 0 |
| `af_slab_alloc` | `(bytes) -> handle`; zero on rejection or allocation failure |
| `af_slab_ptr` | `(handle) -> byteOffset`; zero for an invalid handle |
| `af_slab_len` | `(handle) -> allocatedLength`; zero for an invalid handle |
| `af_slab_release` | `(handle) -> status`; invalid handles never free another allocation |
| `af_call` | `(requestHandle) -> responseHandle`; synchronous, bounded batch execution |

The first 64 memory bytes are reserved, so a valid slab pointer is nonzero.
Allocation rounds capacity to eight-byte alignment; logical payload length
remains explicit. Handles contain a 16-bit slot and 16-bit nonzero generation.
Slots start at 1; generation wrap retires the slot. Slab, session, candidate,
and entity handles are separate namespaces. No handle is a security capability.

The host exclusively writes a new request slab until `af_call`, then borrows
it read-only for that call. Responses are host-readable immutable slabs. The
host releases both after use; double release returns `invalid_handle`. Slabs
cannot alias the mutable kernel heap. JavaScript never holds a Rust object
pointer. Memory growth happens only outside calls and while no view is borrowed;
the host reacquires `memory.buffer` and all views afterward. The
[Wasm JavaScript memory interface](https://webassembly.github.io/spec/js-api/index.html#memories)
defines the underlying buffer behavior.

## Envelope

Every request and response starts with this 64-byte little-endian header.
Readers use checked byte arithmetic; native-endian typed-array casts require
an endian check or a converting adapter.

| Offset | Type | Field |
| --- | --- | --- |
| 0 | `u32` | Magic `0x31424641`, bytes `AFB1` |
| 4, 6 | `u16`, `u16` | ABI major, minor |
| 8, 10 | `u16`, `u16` | Operation, response status; request status is zero |
| 12 | `u32` | Flags; zero in v1 |
| 16, 20 | `u32`, `u32` | Session handle, reserved zero |
| 24, 32 | `u64`, `u64` | Request sequence, tick |
| 40, 44 | `u32`, `u32` | Candidate/active generation handle, payload byte length |
| 48, 52 | `u32`, `u32` | Payload offset, row count |
| 56, 58, 60 | `u16`, `u16`, `u32` | Row stride, payload format, reserved zero |

Payload offset is 64. Logical length equals offset plus payload length and
fits the slab; remaining alignment padding is zero. Unknown flags, nonzero
reserved fields, unsupported formats, overflow, and nonzero trailing data fail
before execution. `u64` counters cross
JavaScript as BigInt or paired `u32` words, never unsafe Number conversions.
Canonical UUIDs become their 16 RFC-order bytes; hashes become 32 raw bytes.

## Operations and payloads

Operation numbers are fixed: `1 create`, `2 stageChunk`, `3 prepareGeneration`,
`4 activateGeneration`, `5 submitInputs`, `6 advanceTick`, `7 readSnapshot`,
`8 restoreCheckpoint`, `9 dropCandidate`, `10 destroy`, `11 readCheckpoint`,
and `12 submitScriptProposals`. Unknown operations fail closed. The caller
loads compiled artifacts as chunks, prepares their complete generation manifest,
then activates under the migration policy in [contracts](contracts.md).
Checkpoint restore also stages complete chunks before an atomic restore.

Format 1 is bounded UTF-8 JCS metadata with zero row count/stride and these
closed operation-specific bodies. UUIDs/digests use [contracts](contracts.md).

| Operation | Required body fields |
| --- | --- |
| create | `clientSessionId` UUID, `profile` (`desktop-v1`/`mobile-v1`), `seed` (32 lowercase hex digits), `engineDigest` |
| prepareGeneration | `generationId` UUID, `worldDigest`, `targetProfile`, `engineAbi` (`af.kernel.1`), `artifacts` (1–4,096 unique digests) |
| activateGeneration | `placementPolicy` (`preserve`/`hold`/`explicit-relocate`), `relocations` (0–64 UUID/global XYZ records); header identifies prepared generation |
| restoreCheckpoint | `checkpointDigest`, `engineDigest`, `worldDigest`; referenced complete staged checkpoint |
| submitScriptProposals | `proposals` (0–1,024 records: `moduleDigest`, `instanceId` UUID, `invocationId` UUID, `stateDigest`, `actions` with 0–64 registered runtime actions) |

Other operations have no metadata body except checkpoint reads: chunk/input/snapshot
formats carry their own data, and drop/destroy use header handles. Format 0 means
an empty payload with zero row count and stride. Snapshot read requests use
format 3 or 4 with an empty payload. Checkpoint reads return a verified internal
checkpoint blob in format 5 chunks selected by byte offset; its format 1 request
contains `offset`/`length` `u32` values. Unknown fields fail. These payloads never
execute source code. Checkpoints include complete script state and RNG state.
Proposals for a tick cross in one batch. Their state blobs are staged immutable
bytes, checked against the instance's schema and limits; an arbitrary hash does
not establish valid state. Well-formed batches return per-instance outcomes.
At `advanceTick`, each valid invocation commits its state/actions atomically;
one invalid invocation cannot discard unrelated valid ones. A malformed batch
rejects the entire request. Inputs and proposals target the next tick.

The following fixed row formats are concrete v1 contracts. Offsets are from
each row; unspecified bytes are reserved zero. Arrays satisfy
`rowCount × stride = payloadLength`, except the declared snapshot prefix.

| Format | Stride | Layout |
| --- | --- | --- |
| 2, player input | 48 | UUID at 0; input sequence `u32` at 16; buttons `u32` at 20; move X/Z, yaw, pitch `f32` at 24/28/32/36; reserved at 40–47 |
| 3, world pose | 80 | UUID at 0; component mask/flags `u32` at 16/20; global XYZ `f64` at 24/32/40; quaternion XYZW `f32` at 48/52/56/60; local linear velocity XYZ `f32` at 64/68/72; reserved at 76 |
| 4, render transform | 64 | UUID at 0; dense handle/flags `u32` at 16/20; local XYZ `f32` at 24/28/32; quaternion XYZW at 36/40/44/48; scale XYZ at 52/56/60 |
| 5, artifact chunk | Variable | 48-byte prefix: artifact hash at 0; total length/offset/chunk length/reserved `u32` at 32/36/40/44, then exact chunk bytes |

Pose and render payloads prepend 32 bytes: origin XYZ `f64`, origin sequence
`u32`, and reserved `u32`. Input values are finite; movement is within [-1,1],
pitch within [-π/2,π/2], yaw normalized to [-π,π), and unknown button bits fail.
Quaternions and scales satisfy [world validation](world-model.md). Chunk writes
reject overlap with different bytes, out-of-range offsets, and incorrect final
hashes. Identical retransmitted chunks are harmless. At most two prepared
candidates coexist with the active generation.

## Order, limits, and errors

Create uses sequence 1 and session 0; its UUID makes identical retries return
the current session. Subsequent requests use the next sequence. The module
retains 64 request hashes and outcomes: an exact retry reports the original
status/tick without repeating effects; a changed request conflicts. A decoded
next-sequence request consumes that sequence even on a typed domain rejection;
malformed headers and invalid sessions do not. Older
sequences fail as expired. Generation handles must match each operation's
expected active or prepared identity. `advanceTick` advances exactly one tick;
its tick must equal current tick plus one.

Replay records are compact metadata, at most 128 bytes each (8 KiB for 64
records): request sequence/hash, operation, original status, applied tick,
generation, and bounded scalar result identities. They retain neither request
nor response slabs, snapshot arrays, or checkpoint bytes. A duplicate response
reports `duplicate` with the original status/tick and available result identity;
it does not promise the original bulk payload. Read operations use a new
sequence to acquire data again and report its actual tick/generation. The host
releases original response slabs independently of replay-record retention.
All replay metadata counts toward the module-memory ceiling.

Initial ceilings are 256 live slabs, 4 MiB per request/result, 16,384 resident
entities, 2,048 active dynamic bodies, 64 characters, 1,024 queued input rows,
and 1,024 emitted events per tick. Module memory has a 256 MiB desktop or 64 MiB
mobile maximum; live slabs have aggregate ceilings of 32/8 MiB respectively.
These allocations count inside [performance](performance.md) budgets. Limits
are safety ceilings, not guaranteed simultaneous performance. Calibration may
lower profile limits; increasing them requires a versioned qualified profile.

Status values are `0 ok`, `1 invalid_handle`, `2 malformed`, `3 incompatible_abi`,
`4 resource_limit`, `5 stale_generation`, `6 sequence_conflict`,
`7 expired_sequence`, `8 sequence_gap`, `9 invalid_value`,
`10 missing_dependency`, `11 migration_required`, `12 duplicate`, and
`13 internal_failure`. Responses reserve required output capacity before
mutating. If even a bounded error response cannot allocate, `af_call` returns
zero without mutation. A trap invalidates the instance; recovery uses its last
accepted checkpoint rather than retrying inside corrupted state.

## Compatibility evidence

Major changes require a new module/adapter and explicit checkpoint migration.
Minor versions cannot reinterpret existing layouts or accept unknown fields
silently. Golden byte fixtures cover every format, endian conversion, maximal
counts, invalid arithmetic, recycled handles, sequence retries, and origin shifts.
Upgrade fixtures run old host/new module and new host/old module, requiring
explicit acceptance or `incompatible_abi`. Browser tests transfer ArrayBuffers,
reconstruct checked slabs on the receiving side, and never transfer the kernel's
memory buffer. **ABI-01:** these fixtures and fuzzed malformed calls pass before
the ABI profile is enabled. **ABI-02:** traps, allocation failure, and duplicate
requests demonstrate no partial accepted mutation or silent replay.
