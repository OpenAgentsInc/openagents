# 63. The coins cache and crash-safe flushes

Chapter 58 validates and prepares one block. Chapter 60 prepares a whole
reorganization. Appending and syncing after every block is simple, but many
successive changes cancel: a new output is spent, a tip changes again, or a
reorganization removes undo that was created privately. A bounded write-back
cache can retain those changes and commit their final effect together.

The implementation is [coinscache.coax](../../coax/lib/bitcoin/coinscache.coax),
over [kvwriteback.coax](../../coax/lib/bitcoin/kvwriteback.coax). It caches the
complete validated transition: coins, inverse records, Core undo chunks,
receipts and `V`. Keeping only coins in the cache would let their durable
cursor or undo disagree after a crash.

## Keep a stable base and a private delta

The base is one immutable, checked validated-v1 table. The delta contains
only keys whose final values differ from that base. Each entry has a put
tag followed by its bytes, or a one-byte deletion tag. Clean reads go to the
base; dirty reads consult the delta first. Empty values remain distinct
from absent keys.

A put equal to the base removes a cached change. Deleting a key absent from
the base also removes its cached change. This gives an important property:
an output created and spent entirely in the cache requires no coin write.
The same rule applies to temporary undo records removed before a flush.

An **existing** base coin behaves differently. Spending it, re-adding it
during a reorganization, then spending it again must retain a deletion. The
base still has the coin. Losing that deletion would restore money that has
already been spent. The cache derives this distinction from actual base
presence, rather than letting a caller guess freshness.

Core expresses this distinction with its dirty/fresh coin states in
[coins.h](https://github.com/bitcoin/bitcoin/blob/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/coins.h)
and [coins.cpp](https://github.com/bitcoin/bitcoin/blob/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/coins.cpp).
Here the delta also covers non-coin records, so it records the final
difference from one stable base. Nested cache layers and clean-entry
eviction are outside this profile.

## Bound the memory before accepting work

<!-- source: coax/lib/bitcoin/coinscache.coax types -->
```jai
CoinsCache :: struct {
  ready: bool; pending: bool; used: int; limit: int; generation: u64;
  base: LcTip; tip: LcTip;
}
CcResult :: struct { code: int; used: int; n: int; generation: u64; }
// Resident cache memory is this state plus both delta arenas and the tagged
// value workspace. DB, validation and frame workspaces have separate explicit
// bounds. No cache procedure allocates another arena or evicts dirty entries.
```

`cc_init_vartime` binds the base's exact network, height and hash, requires
the `V` profile and checks the supplied memory budget. The accounted memory
includes the state, the published and staging delta arenas, and the tagged
value workspace. It uses subtraction to check their sum without overflow.
Every arena has an explicit caller-owned extent. Validation, database and
frame workspaces have their own bounds; they are separate from this cache
budget.

`cc_accept_vartime` accepts only a complete successful chapter-58 or
chapter-60 transition body prepared against this cache's exact materialized
view. It is not a raw-block validator. The real caller must obtain that body
from the full connector. It stages the coalesced delta, materializes the
result, checks its validated profile and network, then replaces the private
cache. Any refusal leaves the previous cache cursor and delta unchanged.

If an edit needs more space, it refuses before exceeding the configured
arenas. The host can flush the previous accepted cache and retry the new
accepted transition against its semantically identical new base. A single
transition that still does not fit remains a capacity refusal. Dirty entries
are never silently evicted.

## Freeze, append, sync, acknowledge

`cc_view_vartime` materializes the base plus delta for the next block's
validator. The cache's private tip can advance while the active index and
serving snapshot remain at the durable base.

`cc_flush_vartime` builds one coalesced, unsealed chapter-62 KV body and its
complete replacement table. An empty cache produces no frame. A dirty
cache enters a pending phase that refuses further accepted edits or another
flush. The host then:

1. Seals the body with the store's next sequence number.
2. Appends the frame and completes storage sync.
3. Publishes the checked replacement base and calls `cc_ack_vartime` with
   the returned generation.
4. Publishes the active index through chapter 60's cursor binding.

Acknowledgement requires the pending generation and exact private tip. It
then clears the delta, binds the new base and advances the generation.
A stale acknowledgement cannot clear a later pending flush.

`cc_abort_before_append` cancels a known pre-append preparation and retains
the dirty cache. Once a write or sync has an ambiguous outcome, close and
recover the store instead. The file adapter follows that rule; it does not
guess whether a failed storage operation committed.

The [worked example](../examples/coins-cache.coax) uses an already accepted
storage fixture at height 100. It prepares height 101, freezes the cache,
rejects acknowledgement of the old cursor, and models the accepted durable
result. Its model acknowledgement performs no disk I/O. The real
[file adapter](../examples/live/cached-chain.coax) appends and syncs before
acknowledgement.

## Restart from the durable cursor

Unflushed work is private and disposable. Before using it, the host must
retain immutable durable raw blocks and their admitted headers. After a
restart, open the checked KV journal, recover only its reported torn tail,
load its `V` cursor, and replay the archived blocks through the full
validator. Never infer a later tip from the size of an archive.

Every flush contains its final coins, undo and cursor in one atomic frame.
Each physical prefix therefore recovers either the complete old base or the
complete new base. All cuts that recover the old base share the same replay
starting point. Replaying the archived unflushed blocks reconstructs the
same state and MuHash.

The teaching file command extends an existing validated store:

```shell
coax/web/tool.sh bitcoin/examples/live/validated-chain.coax init regtest state.kv
coax/web/tool.sh bitcoin/examples/live/cached-chain.coax regtest state.kv headers.bin 2000000000 25 1000000 block1.bin block2.bin
```

It requires exclusive ownership and immutable durable input files. It commits
every requested group, on cache pressure, and at successful completion.
A late invalid block discards the current private group and retains earlier
flushed groups. Repeating the same input list skips blocks already on the
checked cursor's ancestry, so a command can resume after recovery.

The file profile has 262,144-byte delta arenas, a 65,537-byte tagged-record
workspace and a 144-byte native cache state: 589,969 resident cache bytes.
A lower configured limit refuses. Its database, frame and validation buffers
are separately fixed; this command is a bounded teaching adapter. It is a
linear connector. The native oracle additionally exercises complete cached
reorganizations in both directions.

## Check the boundary

The independent [oracle](../../coax/tests/bitcoin/coinscache.py) validates
the regtest prefix from genesis against an isolated Bitcoin Core 32.1.0 node,
then compares every coin and full-set MuHash at A102, B103 and A104. Its
131,217-byte cache performs seven durable flushes. A 256-byte delta refuses
without changing its cursor. Both packed and indexed stores pass.

It also discards the unflushed A101/A102 cache, reopens the actual file into
a fresh header index, replays both archived blocks and restores the exact
private state. The three final flushes export 7,582 byte cuts. Disjoint
digest-bound receipts cover every cut, with only complete old or new states
visible. A separate journey runs the real Coax file adapter through budget
refusal, failed private groups, earlier durable retention, resume and tail
recovery.

```shell
coax/bin/coax test coax/lib/bitcoin/coinscache.coax
coax/bin/coax mutate coax/lib/bitcoin/kvwriteback.coax --root-only --jobs 128 --shard 1/1
coax/bin/coax mutate coax/lib/bitcoin/coinscache.coax --root-only --jobs 128 --shard 1/1
```

All 304 root mutations are killed with zero survivors. The example matches
in debug, release and under address/undefined-behavior sanitizers. The
[verification record](../../docs/verification/bitcoin-ch63-2026-11-04.md)
retains exact scope, source digests and receipts. This native CPU profile
does not import a mainnet snapshot, qualify a mainnet sync or grant money
admission; chapters 80, 93 and 118 still gate those decisions.

Next: pruning, which must use the durable cursor before deleting archives.
