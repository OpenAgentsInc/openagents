# Bitcoin indexed store — 2026-11-03

Epic #12704's native storage foundation, after P2P fix `65ad8ab5e4`.
The packed table remains available. The optional `kvskip.coax` arena uses
ordered, SipHash-selected relative links for lookup and updates. Its
256-byte header and unreachable old nodes are private memory state. No KV
v1 durable identity, frame limit, masking rule or byte representation changes.
Batch preflight includes every put's tower; deletes need no allocation.

Final mutation gates retain every imported baseline:

| Module | Killed | Survived |
| --- | ---: | ---: |
| `kvskip.coax` | 260 | 0 |
| `kvstore.coax` | 730 | 0 |
| Total | 990 | 0 |

No mutants are marked equivalent. Expanded store checks cover exact buffer
boundaries, largest key/value extents, zero-success status, overlapping
moves, complete header bytes, checksum extents, empty commits, all C result
fields and fresh-arena loading. The C iterator's first cursor reports its
actual arena key/value offsets. Both representations share the same v1
encoder and ordered snapshot format.

Independent qualification:

- `kvskip.py`, debug/release/ASan/UBSan: 5,000 updates, 15,000 immediate
  seeks, 201 full scans, 100 copies into dirty stage memory, and real mapped
  offsets above four GiB match Python's ordered dictionary.
- `kvskip-store.py`, each mode: exact independent v1 frame/snapshot bytes
  and 1,096 full, torn and damaged file replays. Debug/release additionally
  compare native commit/get/seek/iteration with the original table after
  every batch. The sanitizer process reopens actual fixture files.
- Original `kvstore.py`: 5,136 crash points, four compactions, 2,100 damaged
  logs, 360 commits and 1,200 table operations pass in both native profiles.
- `addrindex.py --arena`, debug/release: each matches 630 complete address
  comparisons and 5,413 reorg-frame cuts. Prepared state, full chronological
  replay and compacted replay agree as logical key/value maps; physical
  allocation addresses are deliberately not a durable identity.

Compiler: coax0 0.48.0, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
Fixed Coax arrays have a one-GiB bootstrap bound. A full-chain host therefore
provides larger caller-owned byte views; the native arena tests exercise
their 64-bit offsets. Product logic remains Coax; allocation and file effects
remain platform effects.

```sh
coax/bin/coax mutate coax/lib/bitcoin/kvskip.coax --root-only --jobs 8
coax/bin/coax mutate coax/lib/bitcoin/kvstore.coax --root-only --jobs 8
python3 coax/tests/bitcoin/kvskip.py --coax coax/bin/coax --mode sanitized
python3 coax/tests/bitcoin/kvskip-store.py --coax coax/bin/coax --mode sanitized
COAX_TEST_COMPILER=coax/bin/coax python3 coax/tests/bitcoin/kvstore.py
python3 coax/tests/bitcoin/addrindex.py --coax coax/bin/coax --mode debug --arena
python3 coax/tests/bitcoin/addrindex.py --coax coax/bin/coax --mode release --arena
```

This record does not claim full signet serving. Incremental log loading,
block-file rollover and the own-node/Core/Electrs acceptance comparison are
the remaining integration work.
