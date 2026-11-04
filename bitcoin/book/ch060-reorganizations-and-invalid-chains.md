# 60. Reorganizations and invalid chains


Two valid blocks can extend the same parent. Chapter 59 keeps both branches
and selects by work, transaction-data arrival order and its final tie-break.
This chapter moves chapter 58's **validated** coins between those branches,
with every script checked, complete undo and one durable commit for the path.
The implementation is [chainreorg.coax](../../coax/lib/bitcoin/chainreorg.coax).
Its batch concatenator is [chainbatch.coax](../../coax/lib/bitcoin/chainbatch.coax).

## Walk backward, then forward

The [runnable example](../examples/validated-reorg.coax) uses independently
mined regtest blocks:

```text
                    A101 -- A102 -- A103 -- A104
                   /
common height 100
                   \
                    B101 -- B102 -- B103 -- B104! -- B105
```

A arrives first. B102 ties A102 in work, so A retains priority. B103 has more
work. Switching to B103 disconnects A102 and A101, then connects B101, B102
and B103. All coins, inverse records, Core undo chunks and the final `V`
cursor share one chapter-62 batch.

B104 spends a missing coin. Attempting the highest-work B105 discovers this
only after privately detaching A and validating B101–B103. The complete
attempt refuses: the public coins and active index still describe A102.
B104 and its known descendants become failed in the header index. Chapter
59's selection then falls back to the valid B103 prefix. A new preparation
can commit that prefix. Finally A104 overtakes B103, requiring three
disconnects and four connects. Both directions execute the complete chapter
57 validator and chapter 58 connector.

## Prepare and commit have different effects

`cr_reorg_vartime` requires the admitted header tree, a validated `V` table,
transaction-data receipts and immutable archived bodies. The current cursor
must match its header's network and height. The target must have complete
transaction-data ancestry and beat the current entry's comparator, except
that a failed active branch can rewind to a valid ancestor. The supplied
bodies must describe exactly the fork's child through the target, in order.

The target may be a preferred prefix of a distant candidate. This permits
bounded initial-download progress; the node's selection loop must finish
before declaring itself caught up or admitting money decisions.

The function alternates two private coin workspaces while disconnecting and
connecting. Each checked transition produces an unsealed KV body with
sequence zero. `cb_join_vartime` checks both bodies and their extents before
appending their operations; a refusal leaves its destination unchanged.
Only the final combined body is sealed with the next storage sequence.

<!-- source: coax/lib/bitcoin/chainreorg.coax types -->
```jai
CR_NOT_BETTER :: 5;
CrBlock :: struct { index: int; bytes: [] u8; }
CrWork :: struct { validate: ValidationWork; path: [] int; }
CrResult :: struct {
  code: int; vcode: int; detail: int; tx: int; input: int;
  used: int; n: int; disconnected: int; connected: int; block: int;
}
// block is a diagnostic index, never an invalidity proof by itself. Only the
// cache predicate below authorizes marking a branch; local/transport failures
// preserve its validity. Success has block=-1. Publication is a separate step.
```

A refusal has no batch or new used extent. Discard all private scratch.
Successful preparation returns the complete replacement table and aggregate
body. Under exclusive ownership, append and sync its frame, then publish the
new coin snapshot and call `cr_publish_vartime`. That function binds the
network, height and hash to the durable validated cursor before setting the
active index. A loaded `V` cursor certifies its already validated ancestry;
the caller must never substitute an unauthenticated imported table.

Validation caches may advance during preparation. A successfully checked
prefix can retain `BLOCK_VALID_SCRIPTS` even when a later block fails. That
flag proves completed checks; it does not assert active membership or durable
coin publication. Block-data and physical undo ownership flags belong to the
archive adapter. Preparation never moves the active index.

## Cache invalidity only with a stable consensus verdict

`cr_cacheable` admits the contextual transaction, coinbase, BIP-30, script,
weight and sigop failures returned by the complete validator. Chapter 59's
`bi_invalid_block_found` marks the failing block and known descendants, then
selection excludes their branch. This follows the pinned Core
[validation flow](https://github.com/bitcoin/bitcoin/blob/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/validation.cpp).

Malformed copies, Merkle failures, witness malleation and signet preflight
failures do not poison an otherwise usable header. Neither do local context,
capacity or damaged-undo failures. An unknown error remains retryable.
The transaction receipt precondition matters: its body has already passed
`CheckBlock`. A changed immutable archive violates that caller contract;
the connector still refuses the changed body before publishing anything.

For example, changing A103's final byte preserves its header but breaks its
transaction Merkle root. The attempt refuses without marking A103 failed.
Supplying the original body then succeeds. A valid header must remain usable
after a peer supplied a damaged copy of its body.

Administrative invalidation is distinct from a consensus verdict. The model
also marks the active A branch failed and prepares a detach-only rewind to
the valid common ancestor. It still commits coins before moving the active
index. Chapter 90 builds the administrative RPC and reconsideration policy.

## Recovery and comparison

The independent [oracle](../../coax/tests/bitcoin/chainreorg.py) starts both
implementations from genesis and submits the same fork to an isolated,
network-disabled Bitcoin Core 32.1.0 node. It compares every native coin
record and full-set MuHash at A102, B103 and A104; Core independently produces
the same invalid verdict and permits the corrected Merkle-body retry.
It cuts each aggregate frame at every byte, checks that recovery exposes
only the old or complete new table, then reloads the committed journal into
a fresh header index and publishes precisely its validated cursor.

The model checks additionally cover exact unwind, damaged undo after a
private detach, missing data, all workspaces, publication bindings and
invalid-descendant fallback. The example runs as native C in debug, release
and with address/undefined-behavior sanitizers. Authority mutation receipts
and detailed scope are in the
[verification record](../../docs/verification/bitcoin-ch60-2026-11-04.md).

```shell
coax/bin/coax test coax/lib/bitcoin/chainreorg.coax
coax/bin/coax mutate coax/lib/bitcoin/chainreorg.coax --root-only --jobs 256 --shard 1/1
python3 coax/tests/bitcoin/chainreorg.py --shared coax/bin/chainreorg-native.dylib --core-dir coax/bin/validation-core --cut-dir /tmp/reorg-cuts
python3 coax/tests/bitcoin/chainreorg.py --shared coax/bin/chainreorg-native.dylib --cuts /tmp/reorg-cuts/105.json --shard 1/8
coax/bin/coax run bitcoin/examples/validated-reorg.coax
```

Run all eight cut shards for each exported target, 102, 105 and 109. Each
receipt names the exact case digest, frame digest, shard and cut count.

The earlier [light example](../examples/light-chain.coax) and
[file adapter](../examples/live/light-chain.coax) remain explicitly
proof-of-work-only. Their `L` table cannot become this chapter's `V` table.
The validated path is qualified on native CPU; this chapter does not claim
a mainnet sync, an unbounded archive, serving policy or money admission.
Chapters 80, 93 and 118 still gate those decisions.

Next: [61. Block files and undo files](ch061-block-files-and-undo-files.md).
