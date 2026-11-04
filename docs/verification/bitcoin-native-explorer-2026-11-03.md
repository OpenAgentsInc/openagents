# Native explorer integration — 2026-11-03

Epic #12704, following indexed-store checkpoint `897b6f08ed`. This record
qualifies the native integration on independent regtest fixtures. It does
not claim the separate full signet/Core/Electrs acceptance comparison.

The native host allocates caller-owned arena views and performs file/time
effects. Coax owns header admission, bounded log replay, block-file catalog,
positional XOR, private light/transaction/address preparation, durable
publication and HTTP responses. The service imports raw Core block files;
it never asks Core or Electrs to answer an explorer query.

Private preparation trades an entire-arena copy for exclusive ownership. A
refusal invalidates the workspace and requires reopening the durable log.
The serialized event loop exposes a new cursor only after its state frame
is synced. The public staged transition keeps its original contract.

Journal segments keep KV v1 bytes and sequence continuity. Only a final
torn frame can be trimmed. Only an exact partial canonical header in a new
final segment can be rebuilt. Existing segments can reside in either owned
storage directory; duplicate names refuse. New segments use available
capacity. The target segment size is soft for one indivisible frame.
Header feeds are replaced atomically, and the node retains its own admitted
archive so an update cannot erase the previous branch's headers.

Final root mutation gates, with imported baseline checks retained:

| Module | Killed | Survived |
| --- | ---: | ---: |
| `kvstore.coax` | 733 | 0 |
| `kvstream.coax` | 49 | 0 |
| `addrindex-private.coax` | 76 | 0 |
| Total | 858 | 0 |

No mutants are marked equivalent. Sequence exhaustion now refuses before
unsigned addition could wrap or trap. The unchanged arena's 260-mutant
result is recorded at `897b6f08ed` rather than repeated.

Independent native evidence:

- `kvstream.py`, checked ASan/UBSan: 1,220 actual full, torn, corrupt,
  zero-padded and exhausted-sequence files match the independent v1 replay
  model and exact canonical snapshot bytes. Reader files remain unchanged.
- `explorer-node.py`, release and checked ASan/UBSan: 105 independent HTTP
  comparisons per profile. Tests use multiple positionally XOR-masked block
  files with physical zero padding, rotating journals, relocation between
  directories, durable restarts, both branch changes, exclusive writer
  refusal, cross-segment sequence corruption, interior tears, partial new
  headers and final-frame recovery.
- In both profiles, one native writer process and one open headless browser
  pass height/hash/txid/address searches and balance changes
  `1,999,998,000 → 0 → 1,999,998,000` through both reorgs. The competing feed
  deliberately omits the old branch; the admitted archive preserves it.
- The extracted teaching API still passes all 75 independent HTTP/RPC
  response comparisons. Header export's original-identity Coax check passes.

The full signet import began from our P2P-presynced 329,328-header chain,
matching Core's tip
`00000013da59fb6c020d38710e9de1eeb8d9a9830b37acb89c152b7cf79ff569`.
Its first bounded native import exits 75 at a synced height 231,735 after
thirteen minutes. This is progress evidence, not final acceptance.

Compiler: coax0 0.48.0, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
Build and repeatable commands are in [the explorer README](../../bitcoin/explorer/README.md).

The Bitcoin registry and all 861 exact source listings pass. The tooling
inventory covers 1,759 tracked files and reports 12 unrelated missing reason
headers, including the newly present upstream `coax/web/api/test_room_events.py`.
Both new Python independent oracles carry Firm reason 2. The full book's
previously recorded capability-evidence mismatch remains outside this change.
