# World residency and resource ownership

`cells.coax` is the historical #12847 controller: 64 m cells, separate
resident/simulate/draw states, hysteresis, persistent sleeping bodies,
edge admission, floating origin and recorded completion envelopes.
`owners.coax` is the separate distributed cell-owner contract. Neither is
replaced by the resource-cost work.

The #12990 components currently provide:

| Source | Boundary |
| --- | --- |
| `resource-cost.coax` | SHA-256 identities and source-derived encoded, decoded, GPU, metadata and scratch costs; actual canonical mip/WAV/collision/body decoders |
| `resource-manifest.coax` | Versioned RCM1 companion bytes, a pinned cost-model revision, complete expected identity, canonical dependency digests and bounded earlier-owner references |
| `resource-admission.coax` | Seven named budget lines around the existing inbox leases and renderer ledger; atomic multi-ticket admission, explicit completion settlement, final retirement and first-peak owner records |
| `resource-stream.coax` | Concrete dependency owners, typed claim/borrow references, completion generations, wanted closure and atomic fixed-tick readiness publication |

These files are source parts so a composed host has one owner of the
existing unmoduled image decoder. The companion does not change VPK1/VPK2
or any published renderer/host ABI. RCM1 has at most 64 resources and eight
dependencies per resource. Entries name earlier owners in strictly
increasing order. Identical content with a different category, image side
or backend is a different concrete layout owner; identical content/spec
pairs must share one entry. Every dependency also carries its full source
SHA-256. A trusted world root names the complete manifest SHA-256.
Digest fields serialize `AssetId`'s eight numeric SHA-256 words as
little-endian u32 values; conventional hex identity still uses `asset_hex`.

The manifest builder derives plans from actual bytes. Reading a trusted
manifest validates its encoding and closure; readiness additionally needs
`resource_matches`, successful real decoding, completion of platform
effects and the stream's tick/epoch/wanted gate. A successful image plan
alone does not authenticate later compressed image data.

Admission charges encoded bytes, retained decoded bytes, one physical
168-byte `ResourcePlan` and logical GPU bytes to the resource's category.
Decoder work and an explicit native upload span charge staging. The host
must retain a separate plan object if it uses this allocation profile;
the manifest and fixed controller/host storage are additional owners.
The retained plan object follows its generated ABI alignment; the cost's
alignment field describes the resource payload.
Browser queue writes borrow the retained source span. The proposed Vulkan
profile owns a transfer span until submission completion. Opaque SDK,
driver, Wasm arena and allocator overhead are separate observations.

Physical allocations and GPU creation follow a successful grant. The
original 64-ticket/32 MiB inbox and 512-slot/1 GiB renderer ceilings remain
in force. A cancellation cannot settle scratch. Only actual effect
completion permits settlement, and final borrowers plus physical
destruction precede retirement. Duplicate lease copies cannot release
retained CPU bytes twice. A lost GPU handle cannot release a replacement.

Each concrete parent retains one edge to each dependency, regardless of
how many root claims use that parent. The edge survives cancellation and
retirement through the parent's actual physical destruction. Current demand
is computed separately from live root claims; an old upload can retain a
dependency without wanting it published. Loading owners retain their full
reservations until actual effect completion. A successful decode remains
unpublished until a later fixed-step publication can also admit every
dependency. Insufficient output capacity leaves the prior state and events
unchanged. Destruction receipts, rather than cancellation signals, release
retained tickets.

The stream has 64 claims, 64 immutable borrows and at most four unfinished
loads. It retains its expected manifest identity and owner epoch throughout
cleanup. Closing rejects new demand but still accepts the old owner's
matching completion and final-borrow receipts. Reused slots and retried
loads require new serials. Render borrows use a separate serial counter and
a different reference type, so display frequency cannot change logical load
generations. Exhausted counters refuse instead of wrapping.

The platform must queue completion facts. Its fixed-step coordinator calls
the controller, records every applied result including failures and cleanup,
then publishes readiness. Host callbacks cannot mutate the simulation or
replace an old stream's backing state. The retained manifest is immutable
until the stream finishes; queue/registry arguments remain the concrete
owners supplied to admission. These are caller obligations to be qualified
by actual host adoption.

The actual multi-cell consumer, platform asynchronous-effect integration,
streaming profiles, route/replay integration and native/browser GPU
allocation/recovery measurements remain #12990 work. The current
[verification record](../../docs/verification/aethyr-streaming-12990/README.md)
qualifies these components and a small native/Wasm codec fixture.
