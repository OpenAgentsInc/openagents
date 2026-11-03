# 125. Address history, balances and spends

> **Status: implemented.** Native CPU, explorer proof-of-work-only profile.

An address encodes a scriptPubKey. Chapter 29 turns the text into those exact
bytes; this chapter indexes the bytes, so alternate address spellings do not
split a balance. The same query accepts scripts that have no address, such
as the `OP_TRUE` in the small worked example below. It does not infer an
address from a public key hidden inside a script.

This is the address-index step of [#12704](https://github.com/OpenAgentsInc/openagents/issues/12704),
added after the original 124-chapter plan. Bitcoin Core's transaction index
does not provide it. The implementation is [addrindex.coax](../../coax/lib/bitcoin/addrindex.coax),
[addrindex-stage.coax](../../coax/lib/bitcoin/addrindex-stage.coax) and
[addrindex-store.coax](../../coax/lib/bitcoin/addrindex-store.coax),
on chapter 91's transaction index and chapters 58/60's light chainstate.

## Index output occurrences and spend occurrences

The new schema is version 1, separate from Core's databases:

| Key | Value |
| --- | --- |
| `E` | An exact copy of the synchronized transaction-index cursor |
| `F / SHA256(script) / outpoint / blockhash` | Creation height, transaction ordinal, satoshis and full script |
| `G / outpoint / creation-height / spending-blockhash` | Spending txid, input number, height and transaction ordinal |

All numeric fields use little endian. The hash groups matching scripts,
while the full script comparison prevents a hash collision from silently
combining unrelated outputs. An outpoint is the full txid followed by its
LE32 output number. Block hashes distinguish the same transaction mined on
two branches. Creation height distinguishes historical duplicate txids.

The light transition has already checked transaction framing, witness and
Merkle commitments, and input availability. Each removed coin has an ordered
inverse record containing its original creation height. We use that receipt
to bind the spend to the right occurrence, including a coin created earlier
in the same block. No script is executed by this profile.

Unspendable outputs (`OP_RETURN`, or scripts over 10,000 bytes) are absent,
as they are from the light UTXO set. Addressable outputs are retained even
when their value is zero. Balance includes confirmed unspent coins, including
immature coinbase outputs; it is not a wallet's spendable balance.

## One chain cursor

`axi_init_vartime` starts a new database at genesis. Existing transaction-only
databases must be replayed into a new database: adding an empty index at a
nonzero tip would claim a complete history it did not have.

`axi_reorg_vartime` prepares coins, inverses, transaction positions, output
and spend occurrences, and all three cursors in one atomic KV batch. Every
failure returns no publishable batch and leaves the live source unchanged.
`axi_disconnect_vartime` shares that rule. The durable
[file adapter](../examples/live/address-index.coax) syncs block bytes before
the one state frame, using the shared [file implementation](../platform/index-file.coax).
It requires exclusive file ownership and immutable block bodies.

Archived occurrences survive disconnect. Queries use the light chain's
active block membership rather than separately maintained active flags.
Replaying a block must reproduce each archived value byte for byte. A stale
or absent cursor refuses the query, including after someone bypasses the
address-aware transition and changes only the transaction index.

## Query the snapshot

`axi_query_vartime` returns a balance, deduplicated confirmed transaction
occurrences, and output rows. Each row has an explicit status: 0 unspent,
1 spent (with txid, input number, height and block hash), or 2 replaced by a
historical duplicate txid. A reorg removes both incoming and outgoing
transactions from the address history when their blocks leave the chain.
The query requires caller-sized arrays and returns `LC_CAPACITY` instead of
a truncated history; arrays are scratch unless the result code is `LC_OK`.
Rows follow stored key order, not a promise of newest-first sorting.

The [worked program](../examples/address-index.coax) connects one mined
regtest block, queries its output script, then disconnects it:

```text
profile pow-only; scripts not validated
height 1: balance 5000000000 sat, 1 transaction, 1 output
output status 0: unspent
zero output capacity: refusal 2
disconnect: balance 0 sat, 0 transactions, 0 outputs
```

For a durable database, run:

```shell
coax/web/tool.sh bitcoin/examples/live/address-index.coax init regtest STATE
coax/web/tool.sh bitcoin/examples/live/address-index.coax apply regtest STATE BLOCKFILE HEADERS NOW BLOCK...
coax/web/tool.sh bitcoin/examples/live/address-index.coax address regtest STATE ADDRESS
```

The shared adapter also provides `tip`, `get`, `disconnect` and `recover`,
with chapter 91's argument order. It supports one unmasked block file and
bounded in-memory snapshots; file rollover and a full-node service are
integration work, not implied by this chapter's library profile.

## Check against an independent replay

[addrindex.py](../../coax/tests/bitcoin/addrindex.py) parses raw transactions
independently into coins, output histories and spend links. It compares
native debug and release results after each mature-prefix block, same-block
spends, multiple outputs to one address, zero-valued outputs, a shared txid
mined into competing branches, both reorg directions, and disconnect.
Every physical cut of the reorg's state frame must expose the previous or
the complete new address snapshot. Coax checks cover exact workspace edges,
stale cursors and malformed archive records. See the
[verification record](../../docs/verification/bitcoin-address-index-2026-11-03.md).
