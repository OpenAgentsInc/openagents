# Whole-container transport planning

RCM1 describes the resource payload. HTP1 adds the complete container needed
to obtain it, without changing either RCM1 or VPK1/VPK2. A 24-byte index span
inside a larger admitted pack still needs that whole pack's source and reader
reactor. `hpt_source` derives the plan from real bytes: complete pack admission,
exact entry-start/subspan bounds and the existing canonical resource codec
must agree. Raw sources fill their complete transfer. Unsupported models,
malformed containers and source mismatches have typed refusals. Reading a
companion is not proof of a later successful fetch.

`coax/web/tool.sh metaverse-engine/tools/stream-transport-manifest.coax SCENE_DIR NEW_DIR`
creates four companions over the original scene and its two-backend/two-tier
manifests. This fixture tool's container input span is explicitly 64 KiB;
the shared policy supports the existing 64 MiB pack bound. Original scene,
payload, container and RCM1 identities remain unchanged. `transport.json`
lists the new content hashes and original scene/manifest roots.

HTP1 has a 112-byte header: magic, u32 version 1, resource count and 96-byte
row size, then scene, RCM1 and cost-model SHA-256 identities. Each row has a
u32 raw flag, a zero reserved u32, container and content identities, then
u64 complete container size, source offset and source size. Digests keep
RCM1's little-endian numeric SHA-word convention. Every source row binds
the original RCM1 content and encoded size. Raw identity and complete extent
match the payload. The reader authenticates the whole companion before
parsing, validates the model and spans, and preserves prior output on refusal.
Wide wire values narrow only inside the bounded source domain.

The pinned model is
`HTP1/whole-vpk1-vpk2-1/raw-exact-1/hpk-initial8m-page64k-1`.
The reader reactor's actual observed heap base determines its aligned manual
arena using existing `hpk_allocation`. Planned reactor bytes are the greater
of its required page extent and the actual 8 MiB initial memory. Raw transfers
also own a separate exact byte array; its reservation rounds up to 64 KiB
pages. Successful byte allocation and reserved pages are distinct quantities.
These counts exclude opaque browser network, crypto, GC and driver memory.
The explicit `grow` observation prevents unnecessary zero-page growth.

`hpt_reserve` prepays the complete peak through the existing pack-storage
queue and exact slot/serial/epoch leases. Reactor and raw byte-array tickets
reserve atomically. A second-ticket refusal publishes neither reservation;
terminal serial retirement still propagates. The shared loader now uses this
boundary for every `loadBytes` call. Public `load({url,bytes})` additionally
plans a known complete VPK extent, then requires the header and stream to
match it. Admission precedes Wasm construction and fetch. Leases stay owned
through uncertain reads/cancellation, immutable borrowers, zeroing and final
operation disposal. The global 64-ticket/256 MiB page pool is unchanged.

An ordinary public pack without `bytes` keeps the existing bounded-header
path. Private broker/crypto/gzip loads keep their separately qualified
observed-extents path; a private load cannot silently use a public plan.
The additive `world/stream-walk-transport.coax` carrier now binds HTP1 in
the actual corridor. `stream-transport.coax` authenticates the original scene
before opening its companion. Each attempt binds the real loading owner,
generation, world epoch, wanted flag and every scene location field. Complete
fetched bytes must reproduce the admitted container, source span and canonical
resource before decoding. Finish/destruction and GPU rebinding refuse while
transport effects remain live. The SDK ends an attempt only after awaiting
the shared reader's retirement, cancellation and final borrowers. Successful
completion additionally requires source validation. Old keys cannot end a
replacement attempt. Final readiness remains the existing paid-step policy.

This carrier's actual retained configuration span limits complete containers
to 32 KiB, checked before effects; it does not change the shared 64 MiB model.
The HTP1 adapter copies the Coax-projected source span, removing the duplicate
JavaScript whole-hash/entry decision on this path. Earlier carriers retain
their qualified adapter and ABI. The new carrier supports automatic device
replacement; persistent whole-Wasm rescue remains the earlier separate profile.

## Transport observation v1

This complete-history format is retained. The additive v2 profile below
separates an exhausted diagnostic recording from product effect lifetime.

`pack-host.js` accepts an optional synchronous `observe` callback on known
public/raw operations. It reports plans after original-ticket admission and
before construction/fetch, actual reader growth/raw-array allocation, and
zeroing plus final ownership release. Callback/constructor failures unwind
the actual owners. It does not expose mutable source buffers. Unknown-size
and private operations retain their earlier observed-extents boundary.

The corridor keeps at most 2,048 records in a prepaid 393,216-byte
`BigInt64Array`. `aethyr.stream-transport-observation.v1` exports 24 signed
decimal words per event. Words 0–3 are kind, resource row, generation and
world epoch. Rows −1/−2/−3 name original scene/RCM1/HTP1 bootstrap readers.

| Kind | Words after the key |
| --- | --- |
| 1 begin | 4–7: complete container bytes, source offset, source bytes, raw flag |
| 2 plan | 4–9: reactor, external raw bytes, reserved page bytes, observed heap base, initial memory, grow flag |
| 3 allocation | 4–5: added reactor bytes, added external raw bytes |
| 4 checked source | 4: complete received container bytes |
| 5 ownership release | 4–5: zeroed reactor and external raw bytes |
| 6 joined end | 4: SDK outcome (0 success, 1 failure) |
| 7 constructor refusal | 4–5: zeroed partial reader bytes (if memory construction succeeded), external bytes; original tickets returned |

For plan/release/refusal, word 10 counts tickets, 11–16 carry up to two
original slot/serial/epoch triples, and 17–20 carry the shared storage queue's
count/bytes/capacity/per-allocation limit. Remaining words are zero. Trace
capacity exhaustion refuses; it never truncates a successful observation.
The companion root and separately measured carrier static size accompany
the trace. An independent oracle reconstructs source identities, page plans,
all leases, overlapping allocations, source/end ordering and final cleanup.

Ownership release means explicit engine references and reservations ended.
It does not assert that browser GC or the OS physically returned backing
pages. SDK/driver/GC internals and serialized diagnostic object graphs remain
separate. The [adoption record](../../docs/verification/aethyr-streaming-12990/walk-transport-binding.md)
and earlier [planning record](../../docs/verification/aethyr-streaming-12990/walk-transport-planning.md)
state their respective scopes.

## Transport observation v2 and capture

`diagnostic-trace.coax` admits 1–2,048 records, default 2,048, using the
same 24-word events. Its plan gives the exact array bytes and existing
64 KiB page reservation. `aethyr.stream-transport-observation.v2` adds
`traceReservedBytes`, `complete`, `exhausted` and decimal `dropped` fields.
A full trace returns no index and counts refused observations without
throwing through a resource callback. Product work continues through join,
completion, retirement and final cleanup. Closing an exhausted recorder
succeeds, but never marks its history complete. Counters saturate rather
than wrap. Historical v1 data remains accepted as a complete-history format.

The oracle rejects incomplete v2 data by default. Its explicit `--incomplete`
mode reconstructs only the recorded prefix and checks final SDK/engine
cleanup separately; it does not invent unrecorded release history. Actual
32- and 2,048-event SDK pressure probes hold old effects across the limit
and observe their ownership until cancellation joins.

`capture-cost.coax` separately admits padded readback, raw copy, base64 extent
and logical UTF-16 temporary headroom. The existing renderer/storage owners
reserve before GPU buffer creation. Actual copy/text extents pass the Coax
gate; failed encoding zeroes the copy while original owners remain held
through the GPU completion observation and final release. Captures report their output as
caller-owned diagnostic text. The [adapter/memory inventory](stream-adoption.md)
names static compiler input and SDK/GC/driver storage separately.
