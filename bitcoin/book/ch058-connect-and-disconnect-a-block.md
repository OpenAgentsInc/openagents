# 58. Connect and disconnect a block

Chapter 57 decides whether a block obeys consensus against its parent's
coins. This chapter makes that decision durable and reversible. The
implementation is [chainstate.coax](../../coax/lib/bitcoin/chainstate.coax),
with [chainstate-store.coax](../../coax/lib/bitcoin/chainstate-store.coax) for
the profile, chunked undo and disconnects, and [blockundo.coax](../../coax/lib/bitcoin/blockundo.coax) for Core's undo
coin serialization. It uses chapter 59's admitted header tree and chapter
62's atomic store. Chapter 63 adds a cache; no cache is needed to understand
or run this connector.

## Validate, prepare, then publish

`cs_init_vartime` accepts an empty table and initializes the actual genesis
of the selected network. Its metadata key is `V`, version 1. It cannot
import coins, adopt a later tip or migrate an explorer light snapshot.
`cs_tip_vartime` refuses any table containing the light profile's `L` key,
even an empty value. The production path starts from genesis and applies
each block through `cs_connect_vartime`.

A connect requires exactly the validated tip's child in the admitted header
tree. It copies the published table to disjoint scratch and runs chapter
57's complete validator, including every script. A refusal returns no
publishable batch; the original table is unchanged. Capacity and local
context failures do not assert consensus invalidity.

After validation passes, the connector discards the first scratch view and
replays the accepted immutable body against the original coins. This replay
records each input's full coin **before** spending it, adds spendable outputs
in transaction order, and records every inverse. It builds the Core undo,
its chunks, the inverse receipt and the final tip in one KV batch. The caller
appends and syncs the complete frame before publishing scratch. The function
never moves the header tree's active tip or raises its validity flags.

<!-- source: coax/lib/bitcoin/chainstate-store.coax types -->
```jai
CS_OK :: 0; CS_CONTEXT :: 1; CS_CAPACITY :: 2; CS_CONSENSUS :: 3; CS_UNDO :: 4;
CS_META :: [1] u8.['V']; CS_VERSION :: 1; CS_UNDO_CHUNK :: 65536;
CsResult :: struct {
  code: int; vcode: int; detail: int; tx: int; input: int;
  used: int; n: int; undo_n: int;
}
// V: version/network/height/hash (44 bytes). C: chapter 57 coin memory format.
// U: complete ordered inverses, including BIP30 overwritten coinbases.
// H: version/height/parent/inverse count/digest/weight/undo size/checksum (116).
// O || blockhash || LE32(chunk): exact serialized CBlockUndo, in 64KiB chunks.
// All these keys share ONE atomic KV batch. L is never accepted or upgraded.
```

The `C` records are chapter 57's coin memory format. `U` keeps chapter 58's
complete inverse representation described below. The validated `H` receipt
extends the light receipt with the serialized undo length and Core's
`SHA256d(parent_hash || undo)` checksum. `O` stores the exact Core undo bytes
in 65,536-byte chunks, so undo larger than a KV value still shares the atomic
commit. A resource refusal leaves no batch to publish.

`cs_disconnect_vartime` first checks the validated tip, receipt version and
height, every undo chunk's exact extent, the parent-bound Core checksum, and
the digest of the entire ordered inverse set. It then reverses the changes,
checking every current coin's expected afterimage before restoring its old
bytes. Finally it removes the detached block's receipt, chunks and inverses
and sets the parent's tip in the same batch. Damage refuses without changing
the published snapshot. These hashes detect damage, not an attacker who can
replace the complete store and recompute its hashes.

## Core's undo bytes

The pinned [Core undo formatter](https://github.com/bitcoin/bitcoin/blob/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/undo.h)
serializes a block's non-coinbase undo vector with CompactSize. Each
transaction's input vector also begins with CompactSize. Each input stores
`VARINT(height * 2 + coinbase)`, then a legacy zero byte when height is
nonzero, then chapter 56's compressed amount and script. That dummy is the
difference from `Coin::Serialize`. The reader accepts historical nonzero
VARINT version dummies too.

A mature height-1 coinbase worth 5,000,000,000 satoshis with script `OP_1`
serializes as `03 00 32 07 51`: metadata 3, the dummy 0, compressed amount
0x32, raw-script code 7, then 0x51. A block with only its coinbase has no
transaction undo entries, so its entire `CBlockUndo` body is `00`.

The writer always retains full height and coinbase metadata. Reading an old
height-zero undo coin does not recover missing metadata: importing such a
legacy record would require Core's alternate-output lookup. This state
version consumes its own complete records and does not import arbitrary
Core chainstate or old undo files. Chapter 61 can frame these exact bodies
in Core's block/undo file format; the teaching state stores them atomically
as KV chunks.

Core's undo contains inputs, not overwritten BIP-30 coinbases. Our separate
complete inverses preserve those old coins too. They restore the exact
pre-block snapshot at both historical repeat exceptions. That is stronger
than claiming a clean Core disconnect across the old overwrite bug; the
live Core disconnect comparison below uses ordinary regtest blocks.

## Run the smallest durable transition

The [worked example](../examples/validated-chain.coax) admits regtest's
genesis and first mined block through the header loader. It initializes
`validated-v1`, connects the block, prints its 5,000,000,000-satoshi coinbase
and `00` undo, then disconnects to the exact genesis state. Its checked
transcript is [validated-chain.out](../fixtures/validated-chain.out).

```shell
coax/bin/coax run bitcoin/examples/validated-chain.coax
coax/bin/coax run bitcoin/examples/validated-chain.coax --release
coax/web/tool.sh bitcoin/examples/live/validated-chain.coax init regtest /tmp/validated.kv
```

The [file tool](../examples/live/validated-chain.coax) supports `init`, `tip`,
`recover`, `disconnect`, and `apply CHAIN STATE HEADERS NOW BLOCK...`.
Headers are immutable concatenated 80-byte headers beginning at genesis.
Each block is a separate durable commit; a later refusal preserves any
earlier commits from that command. Keep one exclusive owner of the state
file through replay, preparation and append. A torn tail requires explicit
recovery; malformed committed frames refuse without truncation. The tool
has explicit teaching limits: 4,096 headers, 16 MiB tables and batches,
64 MiB log input, a four-million-byte block and 16 MiB undo. The library
accepts caller-sized views; chapter 63 owns cache/flush policy, and chapter
80 owns full sync.

## Independent qualification

[chainstate.py](../../coax/tests/bitcoin/chainstate.py) builds the mature
regtest journey below independently. A live, isolated Core 32.1 has no wallet
or peers. Both nodes start at genesis and connect 102 blocks. At heights
100, 101 and 102, their entire UTXO set's MuHash matches. Both disconnect
102 and 101 and match again. Core's actual XOR-masked `rev` file yields 102
undo bodies, each byte-for-byte identical to the Coax serializer. Both packed
and indexed in-memory tables run this journey. Every one of the 1,335 cuts
of the mature-spend frame exposes exactly the old or complete new snapshot.
The real Coax file tool independently repeats the coin journey, refuses an
invalid child without appending, disconnects twice and recovers a torn tail.

The pinned mainnet range 170,060–170,063 contains 50 transactions. It connects
with every script checked, then disconnects in reverse order to the exact
original table and MuHash. The archival parent context supplies the exact
coins the range touches; untouched global mainnet coins are outside this
fixture. This is not a claim to a global mainnet UTXO hash or sync from
genesis. The codec and inverse model preserve metadata and unrelated records;
chapter 80 still owes the full mainnet journey.

ASan/UBSan checks cover all 35 recorded Core block verdicts and 16 historical
blocks, including both BIP-30 repeats, activation and hash exceptions,
testnet4 and signet. Successful cases compare exact post-block coins, Core
undo bytes and exact disconnects. The [verification record](../../docs/verification/bitcoin-ch58-2026-11-04.md)
records mutation qualification and commands. The full node's money admission
still waits for chapters 80, 93 and 118.

## The existing explorer light path

The original `lightchain.coax` path below remains explicitly proof-of-work-only.
It does not execute scripts, check every consensus rule or receive a profile
upgrade from the new connector. The light and validated metadata and receipts
are distinct. Its earlier qualification remains evidence for unchanged code.

## What connecting changes

An outpoint is a transaction's 32-byte txid in wire order and a four-byte
little-endian output index. A coin adds its amount, creation height,
coinbase flag and scriptPubKey. To connect transactions in block order:

1. Remove each non-coinbase input's existing coin, recording its old bytes.
2. Insert each spendable output, recording whether it replaced an old coin.
3. Save all inverses, their integrity digest, and the new tip in the same
   storage batch.

A later transaction in the block can spend an earlier transaction's output.
You therefore look up inputs in the private working table after earlier
transactions have changed it. A missing input refuses the whole preparation.
Disconnect applies inverses in the opposite order, so an output created and
spent in one block disappears again rather than surviving its disconnect.

The implementation skips outputs whose script starts with `OP_RETURN` or
exceeds 10,000 bytes, matching Core's `IsUnspendable` storage rule. The genesis
coinbase never enters the spendable set. An occupied outpoint refuses unless
the exact height and hash identify one of chapter 55's historical BIP-30
repeat blocks. Even those overwrites keep the prior coin in this profile's
own inverse format, so disconnect can restore the exact earlier snapshot.

This is output tracking. Amount ranges, basic transaction shape, block weight,
Merkle mutation and the witness commitment are checked, but script validity,
maturity, timelocks, complete fee/subsidy rules and signet's challenge remain
outside this light profile. A block can have proof of work and still fail
those later validation rules.

## Work through a mature spend

The independently generated regtest fixture mines 100 blocks after genesis.
Each coinbase has one spendable 5,000,000,000-satoshi output and a zero-value
`OP_RETURN` output. The tip at height 100 therefore has 100 coins worth
500,000,000,000 satoshis. Genesis adds none.

Branch A then does this:

| Block | Change | Coins after the block |
| --- | --- | ---: |
| 101 | Add the coinbase; spend block 1's mature coin into two outputs; spend one of those into another output in the same block | 102 |
| 102 | Add the coinbase; spend the last transaction's output from 101 | 103 |

The three non-coinbase transactions leave fees of 1,000, 2,000 and 1,000
satoshis. The coinbases claim only their subsidies, so those 4,000 satoshis
leave the UTXO set. The result is
`500000000000 + 2 * 5000000000 - 4000 = 509999996000` satoshis.

Undo block 102, then 101. The set returns to the height-100 coins, including
the original height and coinbase flag on block 1's coin. Chapter 56's MuHash
includes those metadata bytes, so an amount-only restoration is insufficient.

## The stored profile

[lightstore.coax](../../coax/lib/bitcoin/lightstore.coax) owns a versioned
schema over the chapter-62 table. It is not Core's chainstate or `CBlockUndo`
serialization. Do not mix these records with a validated chainstate.

| Key | Value |
| --- | --- |
| `L` | Version 1, network, height and tip hash; this schema denotes the proof-of-work-only explorer profile |
| `C || txid || index_le32` | Amount as LE64, height as LE32, one coinbase byte, then the script bytes |
| `U || blockhash || ordinal_le32` | One inverse, including old coin bytes and a digest of the expected new coin |
| `H || blockhash` | Version, height, parent hash, inverse count, the parent/block-bound inverse digest and the block's weight |

An inverse records old-present and new-present flags, the outpoint, the old
record's length and bytes, and `SHA256d(new_coin)`. Before restoring a coin,
disconnect checks that its current bytes match the expected afterimage.
Before applying any inverse, it verifies the digest of the complete ordered
inverse set, including lengths and the parent and block hashes. Changed old
bytes, changed keys, missing inverses and wrong parent bindings refuse.
These digests detect damage; an attacker who can rewrite storage can
recompute them.

[lightpolicy.coax](../../coax/lib/bitcoin/lightpolicy.coax) owns the extent,
transaction-shape, amount, weight and unspendable-script predicates. Their
boundary checks run independently of block traversal. The stored weight is
also compared with each legacy fixture's serialized size, and the file tool
accepts a block at 4,000,000 weight units while refusing one at 4,000,004.

`lc_init_vartime` prepares an empty set at the network's genesis hash.
`lc_disconnect_vartime` prepares one disconnect. Chapter 60's
`lc_reorg_vartime` prepares both ordinary extension and complete branch
changes. Every public preparation returns `LcResult`: `code`, the private
table's `used` bytes, the batch length `n`, and disconnect/connect counts.
A refusal has no publishable batch. Inputs and output workspaces must be
live and disjoint, and the packed source table must come from the checked
KV reader or an earlier accepted preparation.

## Durability comes before publication

Preparation never changes the published table or the block index. It copies
the table to scratch, applies transitions there and builds one KV batch.
Even if the last transaction fails after many earlier changes, the live
snapshot is unchanged.

The [file tool](../examples/live/light-chain.coax) seals that batch as one
chapter-62 frame, appends it and syncs storage before acknowledging the tip.
Coins, undo and tip share one commit. A later invocation replays the log;
a torn tail must be explicitly recovered before further use. A malformed
committed frame is refused without truncation.

Keep one exclusive owner of the state file through load, preparation and
append. Header and block inputs remain immutable during the command. The
tool does not provide a multi-writer transaction or a concurrent-reader
server. Its fixed table, log, header and parser capacities are explicit in
the source; a larger node supplies appropriately sized views and chapter
63's cache/flush policy.

For example, with a binary file of admitted candidate headers in parent-first
order and raw block bodies read from chapter 61:

```shell
coax/web/tool.sh bitcoin/examples/live/light-chain.coax init regtest ./light.kv
coax/web/tool.sh bitcoin/examples/live/light-chain.coax apply regtest ./light.kv ./headers.bin 2000000000 ./block1.bin ./block2.bin
coax/web/tool.sh bitcoin/examples/live/light-chain.coax tip regtest ./light.kv
coax/web/tool.sh bitcoin/examples/live/light-chain.coax disconnect regtest ./light.kv
coax/web/tool.sh bitcoin/examples/live/light-chain.coax recover regtest ./light.kv
```

`2000000000` is the fixture's explicit Unix-time observation. Pass your
current time when admitting live headers. `apply` verifies each header with
chapter 59, then takes the last block's hash as the requested target. The
block list must contain exactly the fork's children through that target.

The tool prints `profile pow-only` with every reported tip. It does not mark
light data as validated. A refused preparation leaves the log byte-identical.
If an append fails, reopen and recover before retrying: a completed frame
can have reached storage even when its caller did not receive success.

Large batches use [kvhash.coax](../../coax/lib/bitcoin/kvhash.coax)'s streamed
SHA-256d checksum. This preserves the existing log format and allows frames
above the one-shot SHA input limit; a 5,000-output test block exercises a
batch larger than 1 MiB through real append and reopen.

## Run and deliberately break it

The [verification record](../../docs/verification/bitcoin-light-connect-2026-11-03.md)
states the checked source versions, commands, results and profile limits.

```shell
coax/bin/coax run bitcoin/examples/light-chain.coax
```

The [example](../examples/light-chain.coax) prints the exact common, A and B
sets with their MuHashes. Its missing-input branch has valid mined headers
and matching Merkle roots. Preparation disconnects A and starts connecting B,
then finds the missing coin in a later block and returns:

```text
late missing input: refusal 4, live tip 102
```

The program asserts that the earlier live snapshot remains byte-identical.
Its exit status is 0 because the refusal is expected. The exact full stdout
and exit status are checked in debug and release, with an additional
ASan/UBSan run of the file adapter.

## Verification and remaining work

The [independent oracle](../../coax/tests/bitcoin/lightchain.py) builds the
regtest blocks with Python's hashes and a separate transaction writer, then
models coins with a dictionary. It checks every connected snapshot against
the native Coax table and independently computes MuHash3072 from each coin's
`TxOutSer`. Its own ChaCha20 and big-integer modular multiplication share no
transition code with Coax. No upstream script is executed.

The oracle also writes and syncs real logs, reopens them, and checks every
physical cut of the reorg's frame. You get the whole earlier tip or the
whole new tip, never a coin prefix with the wrong tip. The file-tool journey
checks extension, reorg, refused branches, recovery and large frames.

This earlier explorer qualification applies to the unchanged light profile.
The validated path above supplies complete consensus and Core undo without
upgrading those snapshots.

Next: [59. The block index and the most-work chain](ch059-the-block-index-and-the-most-work-chain.md),
then [60. Reorganizations and invalid chains](ch060-reorganizations-and-invalid-chains.md).
