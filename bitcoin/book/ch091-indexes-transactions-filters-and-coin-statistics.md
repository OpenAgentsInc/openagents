# 91. Indexes: transactions, filters and coin statistics

> **Status: partly implemented.** The transaction index is runnable on
native CPU, including durable file lookup and light-profile reorganizations.
The filter index, historical coin-statistics index, node-wide background
index management and chapter 90's RPC surface remain planned.

An unspent-output set cannot answer every transaction query. It deliberately
forgets spent outputs and never needs to retain every original transaction.
A transaction index records where the original bytes live, including fully
spent transactions and transactions from branches that have become stale.

This chapter implements the transaction-index step of
[#12704](https://github.com/OpenAgentsInc/openagents/issues/12704). Its source
is [txindex.coax](../../coax/lib/bitcoin/txindex.coax),
[txindex-store.coax](../../coax/lib/bitcoin/txindex-store.coax) and
[txindex-files.coax](../../coax/lib/bitcoin/txindex-files.coax).
The [verification record](../../docs/verification/bitcoin-txindex-2026-11-03.md)
gives the exact checked versions and qualified scope.

## A position points at a transaction, not an output

Chapter 61's block record has an eight-byte framing header followed by the
serialized block. Its data position points at the first byte of the block's
80-byte header. The transaction index adds an offset measured from that
data position:

```text
file offset of transaction = block data position + transaction offset
```

The first transaction starts at `80 + encoded_size(transaction_count)`.
That is 81 for a small block, but 83 for 253 transactions. Each following
offset advances by the preceding transaction's complete serialized size,
including its witness. The key remains its **txid**, computed from the
stripped serialization. A wtxid is not an alias for a txid.

An entry also records the transaction's exact serialized length, block hash,
height, file number and the block's first-indexed sequence. Positions use
checked uint32 file extents; an invalid extent refuses before publication.
The library accepts caller-sized views and multiple file numbers. The
teaching file adapter explicitly supports one unmasked block file, file 0.

## Active and stale are separate answers

The book targets Core revision `ddea9196`, whose
[transaction index](../../coax/tests/vectors/upstream/bitcoin-core/txindex.cpp)
keeps stale entries, excludes genesis, preserves a block's original sequence
on replay, and prefers an active-chain candidate before later-indexed stale
candidates. Its [reorg test](../../coax/tests/vectors/upstream/bitcoin-core/txindex_tests.cpp)
mines the same transaction into two branches and returns to the older one.
Our independent oracle exercises that ordering too.

`txi_find_vartime` returns a `TxiHit` with an explicit `active` flag.
An active match is confirmed in the snapshot being read. A stale match has
zero confirmations. Passing `only_active=true` excludes stale matches.
Disconnecting a block leaves its archival transaction positions intact;
reconnecting it reuses those positions and its sequence without duplicates.
The genesis coinbase is omitted, as in Core's transaction index.

Suppose a transaction occurs in A101, then in B101. After B becomes active,
B101 wins. After returning to A, A101 wins even though its sequence is older.
If the transaction occurs only in A, an archival query still finds it while
B is active, but an active-only query misses.

This implementation has its own versioned KV schema. It uses full txid keys,
unlike pinned Core's [hashed bucket format](../../coax/tests/vectors/upstream/bitcoin-core/txindex_key.h).
It neither reads Core's index database nor implements its legacy fallback.

## Keep the index and chain cursor together

The new keys coexist with chapter 58's `L`, `C`, `U` and `H` records:

| Key | Value in index schema version 1 |
| --- | --- |
| `I` | Version, network, synchronized height, next block sequence, synchronized tip hash: 48 bytes |
| `J || blockhash` | Height, transaction count, sequence, file number, block data position and block length: six LE32 fields |
| `T || txid || blockhash` | Transaction offset and length: two LE32 fields |

The key's two full hashes keep distinct occurrences of a txid. The existing
`H` records identify blocks in the active light chain. Candidate selection
uses that membership, then the immutable first-indexed sequence.

`txi_init_vartime` creates a fresh indexed state at genesis. An existing
light-only database does not become indexed by adding a flag: its history
must be replayed from available block bodies into a new indexed state.
Every query checks that `I` matches `L`, including the network and full tip
hash. An absent or stale cursor refuses instead of presenting incomplete
results as a synchronized index.

`txi_reorg_vartime` first prepares chapter 60's complete detach/attach path
in private scratch. It then indexes each newly encountered block, verifies
receipts on replay, and updates `I` in that **same** batch. A late missing
input, damaged receipt, exhausted sequence or workspace failure returns no
publishable batch. The source snapshot remains unchanged. `txi_disconnect_vartime`
likewise changes the light tip and index cursor together.

Neither operation promotes shared header-index validity. The underlying
profile remains proof-of-work-only, without script/full consensus validation.

## Publish bytes before publishing their positions

The [Coax file adapter](../examples/live/tx-index.coax) follows this order:

1. Admit candidate headers and prepare the whole branch and index batch.
2. Append any new chapter-61 block records and sync them.
3. Append and sync the one KV frame containing coins, inverses, positions
   and both cursors.
4. Acknowledge the new snapshot.

A crash between steps 2 and 3 leaves unreferenced block records. Those
records are harmless and are retained. A committed index never precedes
the block bytes it points at. Reprocessing an already indexed block uses
its original immutable file location.

Recovery replays the KV prefix, checks block-file framing, and audits every
receipt for the supplied file against the complete physical prefix and
its header hash. It refuses to cut a tail containing a committed position.
Only an unreferenced torn block tail and the KV reader's reported torn
state tail may be truncated. Chapter 62's final-frame checksum ambiguity
still applies: the last damaged frame can be an interrupted append; a
checksum failure before later frames is not tail recovery.

Keep exclusive ownership of both files, and keep all inputs and stored
block bytes immutable during a command. Multi-writer locking, file rollover,
pruning, moving an existing receipt, asynchronous backfill and serving
concurrent reader snapshots are later node work.

## Read and check the candidate

Lookup metadata is a position hint. The file adapter reads the record
framing, block header and exact transaction span from that position.
`txi_check_tx_vartime` requires a complete parse, the expected block-header
hash and the requested full txid before printing raw bytes. It reports
parser capacity failures separately from malformed bytes and identity
mismatches. The returned bytes include witnesses.

A txid commits to stripped bytes, so this identity check alone does not
authenticate witness-only changes. Witness commitments were checked when
the light connector admitted the immutable block body. Revalidating a
modified block body is a separate operation, not part of each point lookup.

```shell
coax/web/tool.sh bitcoin/examples/live/tx-index.coax init regtest ./index.kv
coax/web/tool.sh bitcoin/examples/live/tx-index.coax apply regtest ./index.kv ./blk00000.dat ./headers.bin 2000000000 ./block1.bin ./block2.bin
coax/web/tool.sh bitcoin/examples/live/tx-index.coax get regtest ./index.kv ./blk00000.dat TXID_HEX
coax/web/tool.sh bitcoin/examples/live/tx-index.coax get regtest ./index.kv ./blk00000.dat TXID_HEX --active
coax/web/tool.sh bitcoin/examples/live/tx-index.coax disconnect regtest ./index.kv
coax/web/tool.sh bitcoin/examples/live/tx-index.coax recover regtest ./index.kv ./blk00000.dat
```

`TXID_HEX` uses the usual displayed hash order. Header input is concatenated
80-byte headers in parent-first order. `2000000000` is the fixture's explicit
Unix-time observation; supply current time for live data. The block list
contains exactly the fork's children through the requested target.

## Run the trace

```shell
coax/bin/coax run bitcoin/examples/tx-index.coax
coax/bin/coax mutate coax/lib/bitcoin/txindex.coax --library --root-only --jobs 4
python3 -I coax/tests/bitcoin/txindex.py --coax coax/bin/coax --mode debug
python3 -I coax/tests/bitcoin/txindex.py --coax coax/bin/coax --mode release
```

The [example](../examples/tx-index.coax) indexes 100 mined blocks, follows A,
refuses a late missing input, switches to B, finds A's now-stale spend, and
returns to A without changing its original position. Exact stdout is checked
in debug and release.

The independent oracle adds a shared txid in both forks, a witness spend,
and a block containing 253 transactions. It compares every stored index
record and UTXO byte with its own model, reads actual file spans, and checks
every physical cut of the reorg KV frame. The real file-tool trace covers
orphan storage, recovery, read-time corruption refusals and an old light-only
database, with ASan/UBSan qualification. No upstream scripts are executed.

The remaining chapter includes BIP-157 filter serving and its index, a
historical coin-statistics index, complete node backfill/pruning policy and
RPC parity with a running Bitcoin Core node. This transaction portion does
not claim those results.

Next: 92. Notifications for wallets and Lightning (planned).
