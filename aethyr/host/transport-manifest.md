# Whole-container transport planning

RCM1 describes the resource payload. HTP1 adds the complete container needed
to obtain it, without changing either RCM1 or VPK1/VPK2. A 24-byte index span
inside a larger admitted pack still needs that whole pack's source and reader
reactor. `hpt_source` derives the plan from real bytes: complete pack admission,
exact entry-start/subspan bounds and the existing canonical resource codec
must agree. Raw sources fill their complete transfer. Unsupported models,
malformed containers and source mismatches have typed refusals. Reading a
companion is not proof of a later successful fetch.

`coax/web/tool.sh aethyr/tools/stream-transport-manifest.coax SCENE_DIR NEW_DIR`
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
The corridor benefits from prepaid raw metadata/collision/body transfers,
but its packed-resource adapter has not yet bound the HTP1 index. Final
per-owner transport traces, remaining browser transient planning and the
whole-issue consumer/teaching audit remain open. The
[qualification record](../../docs/verification/aethyr-streaming-12990/walk-transport-planning.md)
separates these claims.
