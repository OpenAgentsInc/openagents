# 60. Reorganizations and invalid chains

> **Status: partly implemented.** The native explorer light profile disconnects
to a fork, connects the replacement branch, and prepares the whole change as
one storage batch. Missing inputs, malformed bodies, damaged undo and resource
failures preserve the published snapshot. Full consensus validation, invalid
chain propagation and validated-node recovery remain future work.

Chapter 59 chooses a preferred header branch by work, arrival order and its
final tie-break. Chapter 58's light path supplies reversible coin transitions.
This chapter joins them without claiming that a proof-of-work branch has
passed script validation.

## Work through the fork

The [runnable example](../examples/light-chain.coax) shares a 100-block
regtest prefix, then creates these branches:

```text
                    A101 -- A102
                   /
common height 100
                   \
                    B101 -- B102 -- B103
```

A is connected first. B103 has one more block's work than A102. The common
ancestor is at height 100, so switching to B requires two disconnects and
three connects:

```text
A102 -> A101 -> common -> B101 -> B102 -> B103
```

The published A tip has 103 coins and 509,999,996,000 satoshis. B103 has 103
coins and 514,999,998,000 satoshis. The equal counts do not make these the
same set: B spends the shared mature coin differently, creates different
coinbase outputs, and leaves different script and creation metadata. Their
MuHashes differ.

To test the inverse, the example explicitly disconnects B103. Now A102 and
B102 have equal work. A's earlier entry wins the pinned index comparator's
tie-break. Disconnect B102 and B101, then reconnect A101 and A102. The entire
stored table, including its tip and inverse records, returns byte for byte
to the saved A snapshot. Normal selection refuses the weaker A branch while
B103 remains connected; explicit disconnect is a separate operation.

## Prepare before publishing

[lc_reorg_vartime](../../coax/lib/bitcoin/lightchain.coax) reads the tip from
the versioned light table and locates it in the admitted header tree. It
checks the network and height, verifies that the target is preferred, and
finds the common ancestor with chapter 59's skip pointers.

The caller supplies block bodies in forward order from the fork through the
target. Their index positions must match that exact path. Each body must
hash to its admitted header, link to the current private tip, parse fully,
meet the weight bound, and match its Merkle and witness commitments. The
connection layer then tracks its inputs and outputs.

Only private scratch changes while the path runs. A missing coin in B104
can be discovered after A has been disconnected and B101–B103 have been
prepared. That failure still returns no publishable batch and leaves A's
live snapshot unchanged. A capacity refusal has the same publication rule.

On success the batch includes every disconnected inverse removal, every
replacement coin, every new inverse and the final tip. Chapter 58's file
adapter appends and syncs one frame before reporting success. Reopening
after a torn append exposes the previous committed branch. Reopening after
a complete frame exposes the whole replacement branch.

The block index remains read-only during preparation. This code does not
call `bi_set_tip` or raise `BLOCK_VALID_CHAIN` or `BLOCK_VALID_SCRIPTS`.
The light-profile tip in storage is the authoritative explorer cursor.
A future serving host must publish its reader snapshot only after the
same durable commit; moving the header index first is insufficient.

## A tracking failure is not a consensus-invalid proof

The light path refuses a branch it cannot consistently track. It does not
mark a block consensus-invalid in the shared header tree. A missing input
can mean incomplete local state, and a storage or parser capacity failure
says nothing about consensus validity. Full chapter-57 validation will
provide the verdict that the validated node can propagate through chapter
59's invalid-chain machinery.

Likewise, a successful light reorg says nothing about script signatures,
coinbase maturity, all timelocks or the remaining block rules. Its reported
profile remains `pow-only`, and the book's Core-verdict requirement for
money decisions stays in force.

## Check the boundary

The [verification record](../../docs/verification/bitcoin-light-connect-2026-11-03.md)
records the qualified source versions and exact scopes.

```shell
coax/bin/coax test coax/lib/bitcoin/lightchain.coax --library
coax/bin/coax mutate coax/lib/bitcoin/lightchain.coax --library --root-only --jobs 2 --shard 1/4
python3 -I coax/tests/bitcoin/lightchain.py --coax coax/bin/coax
```

Run mutation shards 1 through 4 separately and merge their records as the
compiler's usage describes. The transition checks exercise the real header
admission path, mature spends, intra-block spends, both fork directions,
late failure, malformed bodies, damaged inverse records and workspace limits.

The independent native oracle compares every UTXO record and MuHash in
debug and release. It writes a durable real-file trace and tests all 2,713
physical cuts of the reorg frame. The file-tool journey additionally checks
explicit tail recovery and a batch above 1 MiB. No live network, public node
or complete consensus-validation qualification is claimed.

The remaining full profile includes consensus-invalid branches, integration
with the validated block connector and coins cache, and differential
reorganization traces against a running Core node.

Next: [61. Block files and undo files](ch061-block-files-and-undo-files.md).
