# 58. Connect and disconnect a block

> **Status: partly implemented.** The explorer light profile is runnable on
native CPU. It tracks outputs and undo on an admitted proof-of-work header
tree, with atomic storage batches. It does not execute scripts, check every
consensus rule or replace chapter 57's block validator. Its state is explicitly
the light profile, never `BLOCK_VALID_CHAIN` or `BLOCK_VALID_SCRIPTS`.

Chapter 59 supplies the header tree. Chapter 61 keeps downloaded block bytes.
Chapter 62 supplies ordered storage and atomic batches. This chapter builds
the first light connection path for [the explorer epic](https://github.com/OpenAgentsInc/openagents/issues/12704).
Chapter 60 uses it to switch branches. The full validated path still needs
chapter 57 and the later coins-cache integration in chapter 63.

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

Full chapter-58 promotion still requires chapter 57's consensus validation,
the Core-compatible undo body for the validated node, historical validation
cases, and the complete validated connect/disconnect qualification. The
explorer's light state remains separate throughout that work.

Next: [59. The block index and the most-work chain](ch059-the-block-index-and-the-most-work-chain.md),
then [60. Reorganizations and invalid chains](ch060-reorganizations-and-invalid-chains.md).
