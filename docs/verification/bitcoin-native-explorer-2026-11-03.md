# Native explorer integration — 2026-11-03

Epic #12704, following indexed-store checkpoint `897b6f08ed`. This record
qualifies the native integration on independent regtest fixtures. The full
signet/Core/Electrs acceptance results are linked at the end of this record.

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

The listening node also retains its arena between bounded import slices.
The independent native journey forces a pause, checks HTTP 503 while the
preferred chain is incomplete, updates the local continuation file and
checks the completed cursor. Both release and checked ASan/UBSan profiles
also notice a header feed replaced while import is paused, without another
file update after the initial ready cursor. The forward batch limit is 64
blocks; whole reorgs keep their independent 4,096-block workspace bound.

Restart replay now advances one complete frame per bounded read and retains
its private arena through continuation. The independent 1,220-file checked
ASan/UBSan oracle still matches exact v1 snapshots. Native release and
checked ASan/UBSan pass 105 HTTP comparisons each with forced replay pauses,
503 until complete, continuation, missing interior segments and unavailable
archive links, in addition to the existing recovery and reorg journey.

The first full signet run reached a synced height 326,651 before the VM's
global OOM killer ended the process. RAM-backed raw files and journals
competed with its growing arena. All durable journals were preserved. The
qualification host moved closed journals and immutable raw-file prefixes
to disk-backed archives, checking complete SHA-256 identities before
publication. Read-only mounted copies were compared again, including reads
through the remote filesystem. A bounded temporary cache accelerated replay;
each cached journal also passed its complete original SHA-256 before use,
and completed cache files were released. This is qualification infrastructure,
not a new journal format or a product dependency on an RPC backend.

The replacement process from `4f25b20dc0` replayed the retained prefix and
finished the frozen signet import at 23:35 CST on November 3. Height 329,338
is synced at sequence 23,932, journal segment 69, with an arena extent of
100,078,620,649 bytes. The Linux release binary's SHA-256 is
`6b43cfea7af814c4afd4377b59e56b7773d7921668f3377fe0c17cb92a7accb7`.
Its cgroup reports zero high/max/OOM events. Core and Electrs are stopped.
The independent frozen-reference and browser results belong in the
[reference record](bitcoin-explorer-reference-2026-11-03.md).

The full-chain browser check exposed a height-lookup timeout after the API
comparison passed. Lookup now seeks only through `J` block receipts; its
128-mutant root gate has zero survivors. A complete restart also defers
raw-file cataloging until another block needs admission, while reading the
XOR key at startup and preserving every query body's existing framing,
header, Merkle and witness checks. The independent native oracle fails on
the previous eager-catalog startup and now passes 108 HTTP comparisons in
release and checked ASan/UBSan, including missing-body refusal and recovery.
Both current profiles pass the 133 Core/Electrs reorg responses and browser
journey; the teaching API's original 75 sanitized responses also pass.

The updated Linux release binary's SHA-256 is
`a90ee16a40da86d623175c222a18d7f25cf7b8224760d1bc5d6482867a76917b`
(GCC 15.2.0). Its complete restart finished at 00:59 CST on November 4,
retaining the same durable sequence 23,932 and preferred height 329,338.
The cold browser journey passed typed height, hash, txid and address searches
before the 63-response Core/Electrs reference comparison passed again.
Desktop/mobile and public mempool captures passed visual review. The
[retained result](bitcoin-explorer-signet-329338/result.json) identifies the
producer, actual served Wasm, memory bounds and evidence. This completes
the full-signet fast-path gate; the original 329,328 import paragraph above
remains historical progress on the superseded chain.
