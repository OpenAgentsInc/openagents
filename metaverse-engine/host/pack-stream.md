# Version 1 pack stream component

`pack-stream.coax` supplies bounded stream and allocation admission around the
existing `metaverse-engine/editor/pack.coax` whole VPK1 reader. It does not publish a partial
pack or replace `pack_read`'s complete layout/catalog/licence check.

`hpk_stream_open(expected,encoded)` opens a 64 MiB maximum stream. Encoded
private input requires an exact nonzero expected length; ordinary VPK1 input
may begin with expected zero. `hpk_stream_offer(state,prefix,chunk_bytes)`
admits an observed chunk before retaining it. Prefix contains only the bytes
needed to complete the first twelve header bytes, even if that header spans
several SDK chunks. The returned `at`, `bytes` and `total` are the admitted
copy offset, length and known allocation extent. Once known, the stated VPK1
length must equal any supplied expected length. An invalid header, overflow,
negative extent or missing prefix poisons the stream. `hpk_stream_finish`
requires the exact complete size. Complete encoded input still needs actual
decrypt/decompress/digest and complete `pack_read` admission.

`hpk_allocation(observed_heap_base,bytes)` supplies a sixteen-byte-aligned
manual arena and exact 64 KiB page count under the unchanged 64 MiB Wasm
maximum. The host observes its real memory extent and grows/copies only the
admitted span. Physical allocation failure remains a refusal.

`hpk_storage_open/reserve/offer` reuses `HostInboxQueue` exact ownership, but
its **internal units are 64 KiB pages**, not socket payload bytes. A ticket
admits at most 64 MiB, and all live tickets together admit 256 MiB/64 tickets.
Release uses the unchanged exact `hiq_release` lease; `hpk_storage_offer`
projects actual byte units for adapters/receipts. Charge every separately
retained source, SDK crypto output and actual Wasm allocation before retaining
it. Retirement cancels effects first and keeps uncertain in-flight ownership
until actual completion; releasing a ticket does not claim an OS/GC RSS drop.

The shared browser now prepays known public/raw transfers through
[transport costs and original page leases](transport-manifest.md), before
constructing an operation reactor or fetching. Reactor and raw-array leases
reserve atomically and remain held through actual cancellation and borrowers.
Unknown-size public and private crypto/gzip paths retain the existing bounded
observed-extents behavior and their separate qualification.

The 64 MiB per-pack extent matches the current native physical pack reader.
Current authored Everglade and fixture packs are 9,463,680 and 1,267,160 bytes.
The aggregate bound is explicit v1 application storage, independent of GPU
admission and hidden network/SDK buffers. Browser/native adoption, actual
stream/cancellation/private-gzip and whole-pack publication receipts remain
separate exits. This component changes no defaults or compiler/build policy.
