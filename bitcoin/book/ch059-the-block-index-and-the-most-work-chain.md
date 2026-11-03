# 59. The block index and the most-work chain

A node hears about blocks in any order. Headers come first, from many
peers, and some of them build on each other while others fork. Block data
comes later, and not always in order either. The node must keep every
header it has accepted, know how much work lies behind each one, and
always follow the chain with the most work. Bitcoin Core keeps this in its
block index: one `CBlockIndex` per header, joined into a tree rooted at
genesis.

This chapter writes that index. Each entry has a height, the chainwork up
to it, status flags, a parent and a skip pointer that finds any ancestor in
O(log n) steps. A header is admitted only after chapter 47's proof-of-work
check and chapter 51's header rules. The tip is the candidate with the
most work, with Core's tie-breaks, and a block found invalid takes every
descendant down with it. The example admits mainnet's first 2,017 headers,
mines regtest forks and reorganizes between them, and every line it prints
agrees with a Python transliteration of Bitcoin Core.

## What this chapter teaches

- What an entry of the block index holds, and what its status flags mean.
- How skip pointers make finding an ancestor take O(log n) steps, and how
  `LastCommonAncestor`, `FindFork` and the block locator use them.
- What `AcceptBlockHeader` checks before a header joins the index, and in
  which order.
- How Core picks the tip: most work, then the block whose data arrived
  first, then the lower pointer.
- Why a block that arrives before its parent waits, and why a header with
  no data never becomes the tip.
- How one invalid block marks all its descendants, and what the best
  header and the best invalid block are.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [blockindex.coax](../../coax/lib/bitcoin/blockindex.coax) | `bi_is_valid`, `bi_raise_validity` | `CBlockIndex::IsValid` and `RaiseValidity` |
| | `bi_skip_height`, `bi_ancestor_walk`, `bi_ancestor`, `bi_build_skip` | `GetSkipHeight`, `GetAncestor` and `BuildSkip` |
| | `bi_last_common_ancestor`, `bi_find_fork`, `bi_locator` | `LastCommonAncestor`, `CChain::FindFork` and `LocatorEntries` |
| | `bi_init`, `bi_find`, `bi_add_vartime` | the index, lookup by hash and `AddToBlockIndex` |
| | `bi_next_bits_vartime`, `bi_median_time_past` | `GetNextWorkRequired` and `GetMedianTimePast` over the index |
| | `bi_accept_header_vartime` | `AcceptBlockHeader` |
| | `bi_worse` | `CBlockIndexWorkComparator` |
| | `bi_received_block_vartime`, `bi_try_add_candidate` | `ReceivedBlockTransactions` and `TryAddBlockIndexCandidate` |
| | `bi_find_most_work_chain`, `bi_activate_best_chain` | `FindMostWorkChain`, and the tip moving to it |
| | `bi_invalid_block_found`, `bi_invalid_chain_found`, `bi_invalidate_block` | `InvalidBlockFound`, `InvalidChainFound` with `SetBlockFailureFlags`, and `InvalidateBlock` |
| | `bi_difficulty` | the RPCs' `GetDifficulty` |
| [header.coax](../../coax/lib/bitcoin/header.coax) | `header_check_vartime` | chapter 51's `ContextualCheckBlockHeader` |
| [retarget.coax](../../coax/lib/bitcoin/retarget.coax) | `retarget_calculate_vartime` | chapter 48's `CalculateNextWorkRequired` |
| [pow.coax](../../coax/lib/bitcoin/pow.coax) | `pow_check_vartime`, `pow_work_vartime` | chapter 47's proof of work and `GetBlockProof` |

## Status flags

Core records how far it has checked each block in `nStatus`. The low
three bits are a level: each level implies the ones below it. The other
bits are flags:

<!-- source: coax/lib/bitcoin/blockindex.coax status -->
```jai
// BlockStatus (chain.h). The low three bits are a level, each implying
// the ones below it; the others are flags.
BLOCK_VALID_UNKNOWN :: 0;
BLOCK_VALID_RESERVED :: 1;       // was BLOCK_VALID_HEADER
BLOCK_VALID_TREE :: 2;           // parents found, nBits, time and version checked
BLOCK_VALID_TRANSACTIONS :: 3;   // CheckBlock passed: the data arrived and is well formed
BLOCK_VALID_CHAIN :: 4;          // ConnectBlock's spending rules passed
BLOCK_VALID_SCRIPTS :: 5;        // and every script
BLOCK_VALID_MASK :: 7;
BLOCK_HAVE_DATA :: 8;            // the block is in a block file
BLOCK_HAVE_UNDO :: 16;           // its undo data is in an undo file
BLOCK_FAILED_VALID :: 32;        // it, or an ancestor, failed
BLOCK_FAILED_CHILD :: 64;        // unused: once set on descendants of a failed block
BLOCK_OPT_WITNESS :: 128;        // the data arrived with witnesses
SEQ_ID_BEST_CHAIN_FROM_DISK :: 0;
SEQ_ID_INIT_FROM_DISK :: 1;
// IsValid: not failed, and at least this level.
bi_is_valid :: (status: u32, level: u32) -> bool #export_c {
  if status & BLOCK_FAILED_VALID != 0 { return false; }
  return status & BLOCK_VALID_MASK >= level;
}
// RaiseValidity: raise the level, never lower it, and never on a failed
// entry. Returns whether it changed.
bi_raise_validity :: (status: [] u32, level: u32) -> bool #export_c {
  if status[0] & BLOCK_FAILED_VALID != 0 { return false; }
  if status[0] & BLOCK_VALID_MASK >= level { return false; }
  status[0] = status[0] - (status[0] & BLOCK_VALID_MASK) + level;
  return true;
}
```

A header that passes `AcceptBlockHeader` is at `BLOCK_VALID_TREE`. Once
its block arrives and passes `CheckBlock`, it rises to
`BLOCK_VALID_TRANSACTIONS` and gets `BLOCK_HAVE_DATA`. Chapter 58's
`ConnectBlock` raises it to `BLOCK_VALID_SCRIPTS`. `BLOCK_FAILED_VALID`
overrides every level: `IsValid` is false for a failed entry, and
`RaiseValidity` leaves it alone.

## An entry

Core's `CBlockIndex` lives in a hash map and points at its parent and its
skip ancestor. Here the entries live in two arrays the caller owns, and
point at each other by position:

<!-- source: coax/lib/bitcoin/blockindex.coax entry -->
```jai
// One CBlockIndex, in two parts. BiLink is its place in the tree: its
// height, and its parent and skip pointer as positions in the same array,
// -1 for none; the skip-list code reads only these. BiEntry, at the same
// position in a second array, is the rest. arrived orders the blocks
// whose data came before their parent's, as Core's m_blocks_unlinked
// multimap does; 0 when there is none.
BiLink :: struct { height: int; prev: int; skip: int; }
BiEntry :: struct {
  hash: [32] u8;
  work: U256;               // nChainWork
  status: u32;              // nStatus
  seq: int;                 // nSequenceId
  ntx: int; chain_tx: int;  // nTx and m_chain_tx_count
  version: int; time: u32; time_max: u32; bits: u32; nonce: u32;
  merkle_root: [32] u8;
  arrived: int;
  candidate: bool;          // in setBlockIndexCandidates
}
```

The split keeps the tree small. `BiLink` is 24 bytes, so the example can
build 300,000 of them on the stack, as Core's `skiplist_test` does. The
skip-list procedures read only links.

## Skip pointers

Finding the ancestor of a block at some height is everywhere in Core: the
locator, the fork point, the retarget's first block, checking that one
block descends from another. Following parents takes as many steps as
the heights differ. Each entry therefore also keeps a skip pointer, built
when it is added, to an ancestor at `GetSkipHeight` of its height:

<!-- source: coax/lib/bitcoin/blockindex.coax skip -->
```jai
// Core's GetSkipHeight: the height a skip pointer jumps back to. Even
// heights clear their lowest set bit; odd ones clear the two lowest set
// bits of height - 1 and add one back.
bi_invert_lowest_one :: (n: int) -> int { return n & (n - 1); }
bi_skip_height :: (height: int) -> int #export_c {
  if height < 2 { return 0; }
  if height & 1 != 0 { return bi_invert_lowest_one(bi_invert_lowest_one(height - 1)) + 1; }
  return bi_invert_lowest_one(height);
}
// GetAncestor: walk back from entry i to height, taking the skip pointer
// unless the parent's skip pointer gets closer. Returns the entry and the
// number of steps; -1 when height is above i or negative.
BiWalk :: struct { index: int; steps: int; }
bi_ancestor_walk :: (l: [] BiLink, i: int, height: int) -> BiWalk #export_c {
  w := BiWalk.{index=-1};
  if i < 0 || height > l[i].height || height < 0 { return w; }
  w.index = i;
  at := l[i].height;
  while at > height {
    skip := bi_skip_height(at);
    skip_prev := bi_skip_height(at - 1);
    if l[w.index].skip >= 0 && (skip == height || (skip > height && !(skip_prev < skip - 2 && skip_prev >= height))) {
      w.index = l[w.index].skip;
      at = skip;
    } else {
      #assert l[w.index].prev >= 0;
      w.index = l[w.index].prev;
      at -= 1;
    }
    w.steps += 1;
  }
  return w;
}
bi_ancestor :: (l: [] BiLink, i: int, height: int) -> int #export_c {
  return bi_ancestor_walk(l, i, height).index;
}
// BuildSkip: the skip pointer of a new entry whose parent is set.
bi_build_skip :: (l: [] BiLink, i: int) {
  l[i].skip = ifx l[i].prev >= 0 then bi_ancestor(l, l[i].prev, bi_skip_height(l[i].height)) else -1;
}
// LastCommonAncestor: bring both to one height, then follow skip pointers
// while they differ and parents once they agree. Equal heights have equal
// skip heights, so the two walks stay level.
bi_last_common_ancestor :: (l: [] BiLink, pa: int, pb: int) -> int #export_c {
  a := pa; b := pb;
  if l[a].height > l[b].height { a = bi_ancestor(l, a, l[b].height); }
  else if l[b].height > l[a].height { b = bi_ancestor(l, b, l[a].height); }
  while a != b {
    while l[a].skip != l[b].skip { a = l[a].skip; b = l[b].skip; }
    a = l[a].prev; b = l[b].prev;
  }
  return a;
}
```

An even height clears its lowest set bit, so 12 skips to 8 and 8 to 0.
An odd height skips to the even height below it with its two lowest set
bits cleared, plus one, so 15 skips to 9. `GetAncestor` takes the skip
pointer unless the parent's skip pointer gets closer to the target. Core's
comment says this needs at most 110 steps to go back up to 2^18 blocks:

```text
skip pointers
  GetSkipHeight of 1 to 20: 0 0 1 0 1 4 1 0 1 8 1 8 1 12 9 0 1 16 1 16
  300000 entries, steps from 299999 to 0: 7 1: 6 150000: 58 262143: 7 299998: 1
  from 262143 (2^18 - 1) to any lower height: at most 105 steps, to 2; one parent at a time it is up to 262143
```

`LastCommonAncestor` relies on one property of `GetSkipHeight`: two
entries at the same height have skip pointers to the same height. Once
both walks are level, they move together until their skip pointers meet,
then step back by parents until the entries meet.

## The locator

A node asks a peer for headers with a locator: a short list of hashes
from its tip back to genesis. The peer finds the first one it knows and
sends what follows. `LocatorEntries` takes the tip, the 10 blocks before
it, then steps that double, and genesis last:

<!-- source: coax/lib/bitcoin/blockindex.coax chain -->
```jai
// CChain: the active chain as an array by height.
bi_tip :: (t: [] BlockTree) -> int #export_c { return ifx t[0].height >= 0 then t[0].chain[t[0].height] else -1; }
bi_contains :: (t: [] BlockTree, i: int) -> bool #export_c {
  h := t[0].l[i].height;
  return h <= t[0].height && t[0].chain[h] == i;
}
// SetTip: fill the array back from the new tip until it meets the old
// chain.
bi_set_tip :: (t: [] BlockTree, i: int) {
  t[0].height = t[0].l[i].height;
  j := i;
  while j >= 0 && t[0].chain[t[0].l[j].height] != j { t[0].chain[t[0].l[j].height] = j; j = t[0].l[j].prev; }
}
// FindFork: the last block of the active chain that is an ancestor of i.
bi_find_fork :: (t: [] BlockTree, index: int) -> int #export_c {
  i := index;
  if t[0].l[i].height > t[0].height { i = bi_ancestor(t[0].l, i, t[0].height); }
  while i >= 0 && !bi_contains(t, i) { i = t[0].l[i].prev; }
  return i;
}
// LocatorEntries: the block, the 10 before it one by one, then steps that
// double, and genesis last. Writes entries to out; returns how many.
bi_locator :: (l: [] BiLink, index: int, out: [] int) -> int #export_c {
  i := index; n := 0; step := 1;
  while i >= 0 {
    out[n] = i; n += 1;
    if l[i].height == 0 { break; }
    h := l[i].height - step;
    if h < 0 { h = 0; }
    i = bi_ancestor(l, i, h);
    if n > 10 { step *= 2; }
  }
  return n;
}
```

The same procedures hold the active chain. `CChain` is an array of
entries by height. `SetTip` fills it back from the new tip until it meets
the old chain, and `FindFork` walks back from an entry until it reaches
the active chain:

```text
locators
  main tip 99999 (28 entries): 99999 99998 99997 99996 99995 99994 99993 99992 99991 99990 99989 99988 99986 99982 99974 99958 99926 99862 99734 99478 98966 97942 95894 91798 83606 67222 34454 0
  branch tip 99999 (28 entries): 99999b 99998b 99997b 99996b 99995b 99994b 99993b 99992b 99991b 99990b 99989b 99988b 99986b 99982b 99974b 99958b 99926b 99862b 99734b 99478b 98966b 97942b 95894b 91798b 83606b 67222b 34454 0
  the two tips' last common ancestor: 49999
```

A branch tip at height 99,999 gets 28 entries, all on the branch (`b`)
until the steps reach 34,454, below the fork at 49,999. The peer finds
the fork point within one doubling step.

## The index

The index finds an entry by its hash. Core uses a hash map; here an
open-addressed table of positions does the same, keyed by the first
bytes of the hash:

<!-- source: coax/lib/bitcoin/blockindex.coax tree -->
```jai
// Core's BlockManager and ChainstateManager fields this chapter reads:
// the entries, a table from hash to entry, the active chain by height
// (CChain), the best header, the best invalid block and the next sequence
// id (nBlockSequenceId, which starts at 2).
BlockTree :: struct {
  p: ChainParams;
  l: [] BiLink; e: [] BiEntry; count: int;
  slots: [] int;            // entry + 1 by hash, 0 for empty; a power of two in size
  chain: [] int; height: int;
  best_header: int; best_invalid: int;
  next_seq: int; next_arrival: int;
}
bi_slot :: (t: [] BlockTree, hash: [] u8) -> int {
  key := 0;
  for k: 0..<7 { key |= cast(int) hash[k] << (8 * k); }
  return key & (t[0].slots.count - 1);
}
bi_same :: (a: [] u8, b: [] u8) -> bool {
  for k: 0..<32 { if a[k] != b[k] { return false; } }
  return true;
}
// The entry with this hash, or -1.
bi_find :: (t: [] BlockTree, hash: [] u8) -> int #export_c {
  s := bi_slot(t, hash);
  while t[0].slots[s] != 0 {
    i := t[0].slots[s] - 1;
    if bi_same(t[0].e[i].hash[..], hash) { return i; }
    s = (s + 1) & (t[0].slots.count - 1);
  }
  return -1;
}
// An empty tree for the network p in the caller's memory: entries, a
// table of at least twice as many slots, and room for the chain.
bi_init :: (t: [] BlockTree, p: ChainParams, l: [] BiLink, e: [] BiEntry, slots: [] int, chain: [] int) -> bool #export_c {
  if l.count != e.count || slots.count < 2 * e.count || slots.count & (slots.count - 1) != 0 || chain.count < e.count { return false; }
  for s: 0..<slots.count { slots[s] = 0; }
  t[0] = BlockTree.{p=p, l=l, e=e, slots=slots, chain=chain, height=-1, best_header=-1, best_invalid=-1, next_seq=SEQ_ID_INIT_FROM_DISK + 1, next_arrival=1};
  return true;
}
```

`AddToBlockIndex` creates the entry for an accepted header. Its parent
gives it its height and skip pointer, its chainwork is the parent's plus
the work of its own `nBits` (chapter 48), and it becomes the best header
if it has strictly more work than the best header so far:

<!-- source: coax/lib/bitcoin/blockindex.coax add -->
```jai
// AddToBlockIndex: a new entry for header h with hash hash. It finds its
// parent, takes the next height, builds its skip pointer, its maximum
// time and its chainwork (the parent's plus GetBlockProof of its nBits,
// chapter 48), raises it to BLOCK_VALID_TREE, and becomes the best header
// when it has more work. A known hash returns its entry.
bi_add_vartime :: (t: [] BlockTree, h: BlockHeader, hash: [] u8) -> int #export_c {
  known := bi_find(t, hash);
  if known >= 0 { return known; }
  if t[0].count >= t[0].e.count { return -1; }
  i := t[0].count;
  t[0].count += 1;
  x: BiEntry;
  for k: 0..<32 { x.hash[k] = hash[k]; x.merkle_root[k] = h.merkle_root[k]; }
  x.version = h.version; x.time = h.time; x.bits = h.bits; x.nonce = h.nonce;
  x.seq = SEQ_ID_INIT_FROM_DISK;
  prev_hash := h.prev;
  prev := bi_find(t, prev_hash[..]);
  t[0].e[i] = x;
  t[0].l[i] = BiLink.{prev=prev, skip=-1};
  e := t[0].e;
  l := t[0].l;
  if prev >= 0 {
    l[i].height = l[prev].height + 1;
    bi_build_skip(l, i);
  }
  e[i].time_max = ifx prev >= 0 && e[prev].time_max > x.time then e[prev].time_max else x.time;
  base := ifx prev >= 0 then e[prev].work else u256_small(0);
  e[i].work = u256_add(base, pow_work_vartime(x.bits));
  status := [1] u32.[e[i].status];
  raised := bi_raise_validity(status[..], BLOCK_VALID_TREE);
  e[i].status = status[0];
  b := t[0].best_header;
  if b < 0 || u256_compare_vartime(e[b].work, e[i].work) < 0 { t[0].best_header = i; }
  s := bi_slot(t, hash);
  while t[0].slots[s] != 0 { s = (s + 1) & (t[0].slots.count - 1); }
  t[0].slots[s] = i + 1;
  return i;
}
```

The best header is the most-work header the node knows, whether or not it
has the block. Headers sync (chapter 72) uses it to decide what to
download.

## Accepting a header

Before a header joins the index, `AcceptBlockHeader` checks it in this
order: a known header is returned as is, unless it failed before; then
the proof of work against its own `nBits` (`CheckBlockHeader`); then its
parent must be known and not failed; then the header rules of chapter 51
in the context of that parent. Genesis skips them all. The context comes
from the index itself:

<!-- source: coax/lib/bitcoin/blockindex.coax context -->
```jai
// GetMedianTimePast of entry i: the median of its time and the up to 10
// before it.
bi_times :: (l: [] BiLink, e: [] BiEntry, i: int, out: [] int) -> [] int {
  n := 0; j := i;
  while n < LOCKTIME_MEDIAN_SPAN && j >= 0 { n += 1; j = l[j].prev; }
  j = i;
  for k: 0..<n { out[n - 1 - k] = cast(int) e[j].time; j = l[j].prev; }
  return out[0..<n];
}
bi_median_time_past :: (l: [] BiLink, e: [] BiEntry, i: int) -> int #export_c {
  times: [LOCKTIME_MEDIAN_SPAN] int;
  return locktime_median_time_past_vartime(bi_times(l, e, i, times[..]));
}
// GetNextWorkRequired for a header with time new_time after entry last:
// the last nBits off a boundary (with testnet's minimum-difficulty walk
// back), and chapter 48's retarget from the period's first block on one.
bi_next_bits_vartime :: (t: [] BlockTree, last: int, new_time: u32) -> u32 #export_c {
  e := t[0].e;
  l := t[0].l;
  r := t[0].p.retarget;
  interval := retarget_interval(r);
  limit := pow_compact_encode_vartime(r.pow_limit, false);
  if (l[last].height + 1) % interval != 0 {
    if r.allow_min_difficulty {
      if cast(int) new_time > cast(int) e[last].time + r.spacing * 2 { return limit; }
      j := last;
      while l[j].prev >= 0 && l[j].height % interval != 0 && e[j].bits == limit { j = l[j].prev; }
      return e[j].bits;
    }
    return e[last].bits;
  }
  first := bi_ancestor(l, last, l[last].height - (interval - 1));
  #assert first >= 0;
  return retarget_calculate_vartime(r, BlockStamp.{time=e[last].time, bits=e[last].bits}, BlockStamp.{time=e[first].time, bits=e[first].bits});
}
```

<!-- source: coax/lib/bitcoin/blockindex.coax accept -->
```jai
// Result codes of AcceptBlockHeader, with Core's reject reason. The
// header rules keep chapter 51's codes, moved up by BI_HEADER.
BI_OK :: 0;
BI_DUPLICATE_INVALID :: 1;  // "duplicate-invalid": the header is known and failed
BI_HIGH_HASH :: 2;          // "high-hash": CheckBlockHeader's proof of work
BI_PREV_NOT_FOUND :: 3;     // "prev-blk-not-found"
BI_BAD_PREVBLK :: 4;        // "bad-prevblk": the parent failed
BI_FULL :: 5;               // no room for another entry
BI_HEADER :: 10;            // + HEADER_BAD_DIFFBITS .. HEADER_BAD_VERSION
BI_REASONS :: [6] string.["ok", "duplicate-invalid", "high-hash", "prev-blk-not-found", "bad-prevblk", "no room"];
BiAccept :: struct { code: int; index: int; }
// AcceptBlockHeader, its checks in Core's order, at the node's clock now.
// Genesis skips them. The anti-DoS work check before a header is stored
// (min_pow_checked) is chapter 71's and 72's; here every header has
// passed it.
bi_accept_header_vartime :: (t: [] BlockTree, raw: [] u8, now: int) -> BiAccept #export_c {
  r := BiAccept.{index=-1};
  h := pow_header_read_vartime(raw);
  if h.code != POW_OK { r.code = BI_HIGH_HASH; return r; }
  hash: [32] u8;
  #assert pow_block_hash_vartime(raw, hash[..]) == POW_OK;
  if !chain_hash_is_vartime(hash[..], t[0].p.genesis.hash) {
    known := bi_find(t, hash[..]);
    if known >= 0 {
      r.index = known;
      if t[0].e[known].status & BLOCK_FAILED_VALID != 0 { r.code = BI_DUPLICATE_INVALID; }
      return r;
    }
    if pow_check_vartime(hash[..], h.bits, t[0].p.retarget.pow_limit) != POW_OK { r.code = BI_HIGH_HASH; return r; }
    prev := bi_find(t, h.prev[..]);
    if prev < 0 { r.code = BI_PREV_NOT_FOUND; return r; }
    if t[0].e[prev].status & BLOCK_FAILED_VALID != 0 { r.code = BI_BAD_PREVBLK; return r; }
    times: [LOCKTIME_MEDIAN_SPAN] int;
    bits := bi_next_bits_vartime(t, prev, h.time);
    code := header_check_vartime(t[0].p, t[0].l[prev].height, bi_times(t[0].l, t[0].e, prev, times[..]), bits, h, now);
    if code != HEADER_OK { r.code = BI_HEADER + code; return r; }
  }
  r.index = bi_add_vartime(t, h, hash[..]);
  if r.index < 0 { r.code = BI_FULL; }
  return r;
}
bi_reason :: (code: int) -> string {
  if code >= BI_HEADER { return HEADER_REASONS[code - BI_HEADER]; }
  return BI_REASONS[code];
}
```

Core reports the version rule as `bad-version(0x%08x)` with the version
filled in; the code keeps chapter 51's short reason. Core also refuses a
header that has not passed the anti-DoS work threshold of chapter 71
(`too-little-chainwork`); headers here arrive after that check.

Mainnet's first 2,017 headers, from genesis across the first retarget at
2,016, are pinned from mempool.space. The index accepts all of them, and
the chainwork of block 2,016 is 2,017 times the work of `0x1d00ffff`,
4,295,032,833 hashes each:

```text
mainnet headers 0 to 2016
  2017 accepted; tip 2016 00000000a141216a896c54f211301c436e557a8d55900637bbdce14c6c7bddef
  chainwork 000000000000000000000000000000000000000000000000000007e107e107e1
  best header 2016, transactions 2017, median time past 1233057128, time max 1233063531
  locator (22 entries): 2016 2015 2014 2013 2012 2011 2010 2009 2008 2007 2006 2005 2003 1999 1991 1975 1943 1879 1751 1495 983 0
  block 2016 again: ok
  block 2016 with one bit of its nonce changed: high-hash
  block 5 before block 4: prev-blk-not-found
```

A changed header fails its proof of work before anything else is looked
at. That is why a block's proof of work is cheap to check and expensive
to fake: no other mainnet rule can be shown with a changed real header.

## Choosing the tip

The tip is the best of the candidates, ordered by
`CBlockIndexWorkComparator`:

<!-- source: coax/lib/bitcoin/blockindex.coax compare -->
```jai
// CBlockIndexWorkComparator: whether a sorts below b, so b is the better
// tip. More work wins; then the lower sequence id, the block whose data
// came first; then the lower pointer, here the lower position.
bi_worse :: (e: [] BiEntry, a: int, b: int) -> bool #export_c {
  c := u256_compare_vartime(e[a].work, e[b].work);
  if c != 0 { return c < 0; }
  if e[a].seq != e[b].seq { return e[a].seq > e[b].seq; }
  return a > b;
}
```

More work wins. Between equal work, the lower sequence id wins. The
sequence id is given when a block's data arrives and is linked to the
chain, so the block a node could connect first wins, and a miner gains
nothing by announcing a header and holding back its block. Blocks loaded
from disk all share sequence id 1 (0 on the best chain), and then the
lower pointer decides. Core's pointers are heap addresses; here the
position plays that part.

A block becomes a candidate only when its data has arrived and every
ancestor's data has too. A block whose parent is still missing waits in
Core's `m_blocks_unlinked`, and is linked, in the order its data arrived,
when the parent's turn comes:

<!-- source: coax/lib/bitcoin/blockindex.coax received -->
```jai
// TryAddBlockIndexCandidate: a block is a candidate when it is not worse
// than the tip.
bi_try_add_candidate :: (t: [] BlockTree, i: int) {
  tip := bi_tip(t);
  if tip >= 0 && bi_worse(t[0].e, i, tip) { return; }
  t[0].e[i].candidate = true;
}
// ReceivedBlockTransactions: block i's ntx transactions arrived and passed
// CheckBlock. It gets BLOCK_HAVE_DATA, and BLOCK_OPT_WITNESS once segwit
// is active, and rises to BLOCK_VALID_TRANSACTIONS. When its parent's
// whole chain has data, it and every waiting descendant, in the order
// their data arrived, get a transaction count and the next sequence id,
// and become candidates.
bi_received_block_vartime :: (t: [] BlockTree, i: int, ntx: int) #export_c {
  e := t[0].e;
  l := t[0].l;
  e[i].ntx = ntx;
  e[i].status |= BLOCK_HAVE_DATA;
  if chain_deployment_active(t[0].p, DEP_SEGWIT, l[i].height) { e[i].status |= BLOCK_OPT_WITNESS; }
  status := [1] u32.[e[i].status];
  raised := bi_raise_validity(status[..], BLOCK_VALID_TRANSACTIONS);
  e[i].status = status[0];
  p := l[i].prev;
  if p >= 0 && e[p].chain_tx == 0 {
    if bi_is_valid(e[p].status, BLOCK_VALID_TREE) { e[i].arrived = t[0].next_arrival; t[0].next_arrival += 1; }
    return;
  }
  // The queue: the block, then its unlinked children, breadth first.
  queue: [256] int;
  head := 0; tail := 0;
  queue[tail] = i; tail += 1;
  while head < tail {
    k := queue[head]; head += 1;
    pk := l[k].prev;
    e[k].chain_tx = e[k].ntx + (ifx pk >= 0 then e[pk].chain_tx else 0);
    e[k].seq = t[0].next_seq; t[0].next_seq += 1;
    bi_try_add_candidate(t, k);
    // m_blocks_unlinked's entries under k, oldest first.
    while true {
      child := -1;
      for c: 0..<t[0].count {
        if l[c].prev == k && e[c].arrived > 0 && (child < 0 || e[c].arrived < e[child].arrived) { child = c; }
      }
      if child < 0 { break; }
      e[child].arrived = 0;
      #assert tail < queue.count;
      queue[tail] = child; tail += 1;
    }
  }
}
```

`FindMostWorkChain` takes the best candidate and walks back to the active
chain. If any block on the way failed or has no data, it drops the
candidate, and everything above the block that stopped the walk, and
tries the next:

<!-- source: coax/lib/bitcoin/blockindex.coax most -->
```jai
// FindMostWorkChain: the best candidate whose path back to the active
// chain has data and no failed block. A candidate that fails the walk is
// dropped with every block above the one that failed it, and the next is
// tried. Returns -1 when no candidate is left.
bi_best_candidate :: (t: [] BlockTree) -> int {
  best := -1;
  for c: 0..<t[0].count {
    if t[0].e[c].candidate && (best < 0 || bi_worse(t[0].e, best, c)) { best = c; }
  }
  return best;
}
bi_find_most_work_chain :: (t: [] BlockTree) -> int #export_c {
  e := t[0].e;
  l := t[0].l;
  while true {
    best := bi_best_candidate(t);
    if best < 0 { return -1; }
    bad := -1; failed := false;
    j := best;
    while j >= 0 && !bi_contains(t, j) {
      #assert e[j].chain_tx != 0 || l[j].height == 0;
      failed = e[j].status & BLOCK_FAILED_VALID != 0;
      if failed || e[j].status & BLOCK_HAVE_DATA == 0 { bad = j; break; }
      j = l[j].prev;
    }
    if bad < 0 { return best; }
    if failed && (t[0].best_invalid < 0 || u256_compare_vartime(e[best].work, e[t[0].best_invalid].work) > 0) { t[0].best_invalid = best; }
    k := best;
    while k != bad {
      // Missing data, not failure: wait for it again in m_blocks_unlinked.
      if !failed { e[k].arrived = t[0].next_arrival; t[0].next_arrival += 1; }
      e[k].candidate = false;
      k = l[k].prev;
    }
    e[bad].candidate = false;
  }
  return -1;
}
// PruneBlockIndexCandidates: drop every candidate worse than the tip.
bi_prune_candidates :: (t: [] BlockTree) {
  tip := bi_tip(t);
  for c: 0..<t[0].count {
    if t[0].e[c].candidate && bi_worse(t[0].e, c, tip) { t[0].e[c].candidate = false; }
  }
}
// ActivateBestChain without connecting: move the tip to the most-work
// chain while that chain has more work than the tip. Chapter 60 connects
// and disconnects the blocks in between and may find one invalid.
bi_activate_best_chain :: (t: [] BlockTree) -> int #export_c {
  best := bi_find_most_work_chain(t);
  tip := bi_tip(t);
  if best >= 0 && (tip < 0 || bi_worse(t[0].e, tip, best)) {
    bi_set_tip(t, best);
    bi_prune_candidates(t);
  }
  return bi_tip(t);
}
```

`bi_activate_best_chain` moves the tip without connecting anything.
Connecting and disconnecting the blocks between the old tip and the new
one, and finding one invalid on the way, is chapter 60's.

The example mines regtest blocks, whose target lets almost any hash
through. Each block's Merkle root is 32 copies of its branch's letter, so
the branches differ, and its nonce is the lowest that meets the target:

```text
regtest
  genesis 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206, work 2, status 0x8b
  A1 to A5 with data: tip A5, best header A5
  headers on A5: nBits 0x207ffffe: bad-diffbits, time at the median time past 1296690403: time-too-old, version 3: bad-version, 2 hours and 1 second ahead: time-too-new
  B3, B4 on A2, work 10 against A5's 12: tip A5, best header A5
  B5, B6 headers only: tip A5, best header B6
  B6's data first: B6 has 0 transactions in its chain, sequence 1; tip A5, best header B6
  then B5's: sequence B5 10, B6 11; tip B6, best header B6
  fork of A5 from the active chain: A2, last common ancestor of A5 and B6: A2
  C7 then D7 headers on B6, D7's data first: sequence C7 13, D7 12; tip D7, best header C7
  with both at sequence 1, as after a restart, the lower position wins: C7
```

B3 and B4 fork from A2 with less work than A5 and change nothing. B5 and
B6 arrive as headers: B6 is now the best header, but the tip stays at A5.
B6's data comes first and waits with sequence 1. When B5's data arrives,
both are linked, B5 first, and B6 takes the tip. C7 and D7 tie on work;
C7's header came first and it is the best header, but D7's data came
first, so D7 is the tip.

## Invalid blocks

When a block fails `CheckBlock` or `ConnectBlock`, Core marks it
`BLOCK_FAILED_VALID`, and `InvalidChainFound` marks every descendant the
same way. A header built on any of them is refused as `bad-prevblk`, and
the failed header itself, sent again, as `duplicate-invalid`:

<!-- source: coax/lib/bitcoin/blockindex.coax invalid -->
```jai
// RecalculateBestHeader: the tip, or the first entry with more work that
// has not failed.
bi_recalculate_best_header :: (t: [] BlockTree) {
  e := t[0].e;
  l := t[0].l;
  b := bi_tip(t);
  for c: 0..<t[0].count {
    if e[c].status & BLOCK_FAILED_VALID == 0 && u256_compare_vartime(e[b].work, e[c].work) < 0 { b = c; }
  }
  t[0].best_header = b;
}
// InvalidChainFound: remember the best invalid block, mark every
// descendant failed (SetBlockFailureFlags), and choose a new best header
// when the old one descends from the failed block.
bi_invalid_chain_found :: (t: [] BlockTree, i: int) #export_c {
  e := t[0].e;
  l := t[0].l;
  if t[0].best_invalid < 0 || u256_compare_vartime(e[i].work, e[t[0].best_invalid].work) > 0 { t[0].best_invalid = i; }
  for c: 0..<t[0].count {
    if c != i && bi_ancestor(l, c, l[i].height) == i { e[c].status |= BLOCK_FAILED_VALID; }
  }
  b := t[0].best_header;
  if b >= 0 && bi_ancestor(l, b, l[i].height) == i { bi_recalculate_best_header(t); }
}
// InvalidBlockFound: block i failed a check of its data or of
// ConnectBlock. A mutated block (chapter 46's or 49's) is not marked: the
// header may be fine and another copy of the block valid.
bi_invalid_block_found :: (t: [] BlockTree, i: int, mutated: bool) #export_c {
  if mutated { return; }
  t[0].e[i].status |= BLOCK_FAILED_VALID;
  t[0].e[i].candidate = false;
  bi_invalid_chain_found(t, i);
}
// InvalidateBlock (the invalidateblock RPC), with the disconnects left to
// chapter 60: the active chain falls back to i's parent, i and its
// descendants fail, and every block that is still valid with data and as
// good as the new tip becomes a candidate again.
bi_invalidate_block :: (t: [] BlockTree, i: int) #export_c {
  e := t[0].e;
  l := t[0].l;
  if bi_contains(t, i) && l[i].prev >= 0 { bi_set_tip(t, l[i].prev); }
  e[i].status |= BLOCK_FAILED_VALID;
  bi_invalid_chain_found(t, i);
  tip := bi_tip(t);
  for c: 0..<t[0].count {
    e[c].candidate = false;
    if bi_is_valid(e[c].status, BLOCK_VALID_TRANSACTIONS) && e[c].chain_tx != 0 && !bi_worse(e, c, tip) { e[c].candidate = true; }
  }
}
```

A mutated block is not marked. Chapter 46's duplicated transactions and
chapter 49's changed witness alter a block without altering its header,
so the same header may still have a valid block. `InvalidateBlock`, the
`invalidateblock` RPC, does the same to a block on the active chain: the
tip falls back to its parent, and every block that is still valid and as
good as the new tip becomes a candidate again.

```text
  F6 to F8 on A5, and F7 fails ConnectBlock: tip D7, best header D7, best invalid F8
  a header on F8: bad-prevblk; F7 again: duplicate-invalid
  invalidateblock B5: tip B4, then tip F6, best header F6, best invalid F8
  failed: B5 B6 C7 D7 F7 F8
  entries: G0=0x8b A1=0x8b A2=0x8b A3=0x8b A4=0x8b A5=0x8b B3=0x8b B4=0x8b B5=0xab B6=0xab C7=0xab D7=0xab F6=0x8b F7=0xab F8=0xab
  locator from the tip: F6 A5 A4 A3 A2 A1 G0
```

F6 to F8 fork from A5 with more work than D7, but F7 fails, so F7 and F8
fail and D7 stays. F8 becomes the best invalid block:
`FindMostWorkChain` records the best candidate it drops for a failure.
After `invalidateblock B5`, the tip falls to B4, B5 to D7 fail, and F6,
with height 6 and no failure, is the best tip left. The status bytes read
`0x8b` (`BLOCK_VALID_TRANSACTIONS`, `BLOCK_HAVE_DATA` and
`BLOCK_OPT_WITNESS`, since regtest's segwit is active from genesis) and
`0xab` for the failed ones.

## Difficulty

The RPCs report each header's difficulty: how many times harder its target
is than the minimum target, `0xffff` times 256 to the 26th. Core computes
it in floating point:

<!-- source: coax/lib/bitcoin/blockindex.coax difficulty -->
```jai
// GetDifficulty (rpc/blockchain.cpp): how many times harder than the
// minimum target, 0xffff * 256^(0x1d - 3), the header's target is, in
// floating point.
bi_difficulty :: (bits: u32) -> f64 #export_c {
  shift := cast(int) (bits >> 24) & 0xff;
  d := cast(f64) 0xffff / cast(f64) (bits & 0x00ffffff);
  while shift < 29 { d *= 256.0; shift += 1; }
  while shift > 29 { d /= 256.0; shift -= 1; }
  return d;
}
```

## The example

[bitcoin/examples/block-index.coax](../examples/block-index.coax) builds
the skip lists and locators, admits mainnet's first 2,017 headers, and
mines the regtest forks:

<!-- source: bitcoin/examples/block-index.coax example -->
```jai
#load "../../coax/lib/bitcoin/blockindex.coax";
#import "Basic";

REGTEST_TIME :: 1296688602;    // regtest's genesis time
LATER :: 1793664000;           // 2026-11-03, the node's clock

hex_digit :: (x: int) -> string { return "0123456789abcdef"[x..<x + 1]; }
// A hash as explorers show it: the bytes reversed.
show_hash :: (h: [] u8) {
  i := 31; while i >= 0 { print("%", hex_digit(cast(int) h[i] >> 4)); print("%", hex_digit(cast(int) h[i] & 15)); i -= 1; }
}
show_status :: (s: u32) { print("0x%", hex_digit(cast(int) (s >> 4))); print("%", hex_digit(cast(int) (s & 15))); }
show_work :: (a: U256) { b: [32] u8; ok := u256_to_hash_vartime(a, b[..]); show_hash(b[..]); }

// ---- skip pointers and locators, on bare heights
skips :: () {
  print("skip pointers\n");
  print("  GetSkipHeight of 1 to 20:");
  for h: 1..<21 { print(" %", bi_skip_height(h)); }
  print("\n");
  l: [300000] BiLink;
  bi_line(l[..], 300000, 0, 0);
  print("  300000 entries, steps from 299999 to");
  to := [5] int.[0, 1, 150000, 262143, 299998];
  for k: 0..<5 { print(" %: %", to[k], bi_ancestor_walk(l[..], 299999, to[k]).steps); }
  print("\n");
  most := 0; at := 0;
  for h: 0..<262143 { s := bi_ancestor_walk(l[..], 262143, h).steps; if s > most { most = s; at = h; } }
  print("  from 262143 (2^18 - 1) to any lower height: at most % steps, to %; one parent at a time it is up to 262143\n", most, at);
}
locators :: () {
  print("locators\n");
  l: [150000] BiLink;
  bi_line(l[..], 100000, 0, 0);
  bi_line(l[..], 50000, 100000, 50000);    // a branch whose first parent is 49999
  out: [64] int;
  tips := [2] int.[99999, 149999];
  labels := [2] string.["main tip 99999", "branch tip 99999"];
  for k: 0..<2 {
    n := bi_locator(l[..], tips[k], out[..]);
    print("  % (% entries):", labels[k], n);
    for i: 0..<n { print(" %", l[out[i]].height); if out[i] >= 100000 { print("b"); } }
    print("\n");
  }
  print("  the two tips' last common ancestor: %\n", l[bi_last_common_ancestor(l[..], 99999, 149999)].height);
}

// ---- mainnet's first 2,017 headers
mainnet :: () {
  print("mainnet headers 0 to 2016\n");
  l: [2048] BiLink; e: [2048] BiEntry; slots: [4096] int; chain: [2048] int; t: [1] BlockTree;
  ok := bi_init(t[..], CHAIN_MAINNET_PARAMS, l[..], e[..], slots[..], chain[..]);
  raw: [80] u8;
  accepted := 0;
  for k: 0..<BI_MAINNET_COUNT {
    n := cp_unhex(BI_MAINNET_HEADERS[160 * k..<160 * (k + 1)], raw[..]);
    a := bi_accept_header_vartime(t[..], raw[..], LATER);
    if a.code == BI_OK { accepted += 1; bi_received_block_vartime(t[..], a.index, 1); }
  }
  tip := bi_activate_best_chain(t[..]);
  print("  % accepted; tip % ", accepted, l[tip].height); show_hash(e[tip].hash[..]); print("\n");
  print("  chainwork "); show_work(e[tip].work); print("\n");
  print("  best header %, transactions %, median time past %, time max %\n",
    l[t[0].best_header].height, e[tip].chain_tx, bi_median_time_past(l[..], e[..], tip), e[tip].time_max);
  out: [64] int;
  n := bi_locator(l[..], tip, out[..]);
  print("  locator (% entries):", n);
  for i: 0..<n { print(" %", l[out[i]].height); }
  print("\n");
  n = cp_unhex(BI_MAINNET_HEADERS[160 * 2016..<160 * 2017], raw[..]);
  print("  block 2016 again: %\n", bi_reason(bi_accept_header_vartime(t[..], raw[..], LATER).code));
  raw[76] ^= 1;
  print("  block 2016 with one bit of its nonce changed: %\n", bi_reason(bi_accept_header_vartime(t[..], raw[..], LATER).code));
  // A second index that has only seen genesis to block 3.
  l2: [8] BiLink; e2: [8] BiEntry; slots2: [16] int; chain2: [8] int; t2: [1] BlockTree;
  ok = bi_init(t2[..], CHAIN_MAINNET_PARAMS, l2[..], e2[..], slots2[..], chain2[..]);
  for k: 0..<4 { n = cp_unhex(BI_MAINNET_HEADERS[160 * k..<160 * (k + 1)], raw[..]); a := bi_accept_header_vartime(t2[..], raw[..], LATER); }
  n = cp_unhex(BI_MAINNET_HEADERS[160 * 5..<160 * 6], raw[..]);
  print("  block 5 before block 4: %\n", bi_reason(bi_accept_header_vartime(t2[..], raw[..], LATER).code));
}

// ---- regtest forks, mined here
Names :: struct { letter: [64] u8; last: int; }
name :: (names: [] Names, i: int, l: [] BiLink) {
  k := cast(int) names[0].letter[i];
  print("%", "GABCDEFGHIJ"[k..<k + 1]); print("%", l[i].height);
}
// Mine a header on prev: version 4, nBits 0x207fffff, a Merkle root of 32
// copies of the letter, ten minutes a block plus the letter in seconds,
// and the lowest nonce that meets the target.
mine :: (t: [] BlockTree, names: [] Names, prev: int, letter: int, data: bool) -> int {
  time := REGTEST_TIME + 600 * (t[0].l[prev].height + 1) + letter;
  return mine_with(t, names, prev, letter, data, 4, 0x207fffff, time);
}
mine_with :: (t: [] BlockTree, names: [] Names, prev: int, letter: int, data: bool, version: int, bits: u32, time: int) -> int {
  e := t[0].e;
  h: BlockHeader;
  h.version = version; h.bits = bits; h.time = cast(u32) time;
  for k: 0..<32 { h.prev[k] = e[prev].hash[k]; h.merkle_root[k] = cast,trunc(u8) letter; }
  raw: [80] u8;
  while true {
    ok := pow_header_write_vartime(h, raw[..]);
    if pow_header_check_vartime(raw[..], t[0].p.retarget.pow_limit) == POW_OK { break; }
    h.nonce += 1;
  }
  a := bi_accept_header_vartime(t, raw[..], LATER);
  names[0].last = a.code;
  if a.code != BI_OK { return -1; }
  names[0].letter[a.index] = cast,trunc(u8) letter;
  if data { bi_received_block_vartime(t, a.index, 1); }
  return a.index;
}
run :: (t: [] BlockTree, names: [] Names, prev: int, letter: int, count: int, data: bool) -> int {
  at := prev;
  for k: 0..<count { at = mine(t, names, at, letter, data); }
  return at;
}
// A header on prev that is refused: its reason.
check :: (t: [] BlockTree, names: [] Names, prev: int, version: int, bits: u32, time: int) -> int {
  i := mine_with(t, names, prev, 9, false, version, bits, time);
  return names[0].last;
}
state :: (t: [] BlockTree, names: [] Names) {
  l := t[0].l;
  print("tip "); name(names, bi_tip(t), l);
  print(", best header "); name(names, t[0].best_header, l);
  if t[0].best_invalid >= 0 { print(", best invalid "); name(names, t[0].best_invalid, l); }
  print("\n");
}
regtest :: () {
  print("regtest\n");
  l: [64] BiLink; e: [64] BiEntry; slots: [128] int; chain: [64] int; t: [1] BlockTree; names: [1] Names;
  ok := bi_init(t[..], CHAIN_REGTEST_PARAMS, l[..], e[..], slots[..], chain[..]);
  g: [CHAIN_GENESIS_MAX] u8;
  n := chain_genesis_block_vartime(CHAIN_REGTEST_PARAMS.genesis, g[..]);
  a := bi_accept_header_vartime(t[..], g[0..<80], LATER);
  bi_received_block_vartime(t[..], 0, 1);
  tip := bi_activate_best_chain(t[..]);
  print("  genesis "); show_hash(e[0].hash[..]); print(", work %, status ", u256_low64(e[0].work)); show_status(e[0].status); print("\n");
  a5 := run(t[..], names[..], 0, 1, 5, true);
  tip = bi_activate_best_chain(t[..]);
  print("  A1 to A5 with data: "); state(t[..], names[..]);
  mtp := bi_median_time_past(l[..], e[..], a5);
  print("  headers on A5: nBits 0x207ffffe: %", bi_reason(check(t[..], names[..], a5, 4, 0x207ffffe, mtp + 1)));
  print(", time at the median time past %: %", mtp, bi_reason(check(t[..], names[..], a5, 4, 0x207fffff, mtp)));
  print(", version 3: %", bi_reason(check(t[..], names[..], a5, 3, 0x207fffff, mtp + 1)));
  print(", 2 hours and 1 second ahead: %\n", bi_reason(check(t[..], names[..], a5, 4, 0x207fffff, LATER + 7201)));
  a2 := bi_ancestor(l[..], a5, 2);
  b4 := run(t[..], names[..], a2, 2, 2, true);
  tip = bi_activate_best_chain(t[..]);
  print("  B3, B4 on A2, work % against A5's %: ", u256_low64(e[b4].work), u256_low64(e[a5].work)); state(t[..], names[..]);
  b5 := mine(t[..], names[..], b4, 2, false);
  b6 := mine(t[..], names[..], b5, 2, false);
  tip = bi_activate_best_chain(t[..]);
  print("  B5, B6 headers only: "); state(t[..], names[..]);
  bi_received_block_vartime(t[..], b6, 1);
  tip = bi_activate_best_chain(t[..]);
  print("  B6's data first: B6 has % transactions in its chain, sequence %; ", e[b6].chain_tx, e[b6].seq); state(t[..], names[..]);
  bi_received_block_vartime(t[..], b5, 1);
  tip = bi_activate_best_chain(t[..]);
  print("  then B5's: sequence B5 %, B6 %; ", e[b5].seq, e[b6].seq); state(t[..], names[..]);
  print("  fork of A5 from the active chain: "); name(names[..], bi_find_fork(t[..], a5), l[..]);
  print(", last common ancestor of A5 and B6: "); name(names[..], bi_last_common_ancestor(l[..], a5, b6), l[..]); print("\n");
  c7 := mine(t[..], names[..], b6, 3, false);
  d7 := mine(t[..], names[..], b6, 4, false);
  bi_received_block_vartime(t[..], d7, 1);
  bi_received_block_vartime(t[..], c7, 1);
  tip = bi_activate_best_chain(t[..]);
  print("  C7 then D7 headers on B6, D7's data first: sequence C7 %, D7 %; ", e[c7].seq, e[d7].seq); state(t[..], names[..]);
  pair: [2] BiEntry;
  pair[0] = e[c7]; pair[1] = e[d7];
  pair[0].seq = SEQ_ID_INIT_FROM_DISK; pair[1].seq = SEQ_ID_INIT_FROM_DISK;
  print("  with both at sequence 1, as after a restart, the lower position wins: %\n", ifx bi_worse(pair[..], 1, 0) then "C7" else "D7");
  f8 := run(t[..], names[..], a5, 6, 3, true);
  f7 := l[f8].prev;
  bi_invalid_block_found(t[..], f7, false);
  tip = bi_activate_best_chain(t[..]);
  print("  F6 to F8 on A5, and F7 fails ConnectBlock: "); state(t[..], names[..]);
  g9 := mine(t[..], names[..], f8, 7, false);
  raw: [80] u8;
  h: BlockHeader;
  h.version = e[f7].version; h.time = e[f7].time; h.bits = e[f7].bits; h.nonce = e[f7].nonce;
  h.prev = e[l[f7].prev].hash; h.merkle_root = e[f7].merkle_root;
  ok = pow_header_write_vartime(h, raw[..]);
  print("  a header on F8: %; F7 again: %\n", bi_reason(names[0].last), bi_reason(bi_accept_header_vartime(t[..], raw[..], LATER).code));
  bi_invalidate_block(t[..], b5);
  print("  invalidateblock B5: tip "); name(names[..], bi_tip(t[..]), l[..]);
  tip = bi_activate_best_chain(t[..]);
  print(", then "); state(t[..], names[..]);
  print("  failed:");
  for i: 0..<t[0].count { if e[i].status & BLOCK_FAILED_VALID != 0 { print(" "); name(names[..], i, l[..]); } }
  print("\n  entries:");
  for i: 0..<t[0].count {
    print(" "); name(names[..], i, l[..]);
    print("="); show_status(e[i].status);
  }
  out: [64] int;
  n = bi_locator(l[..], tip, out[..]);
  print("\n  locator from the tip:");
  for i: 0..<n { print(" "); name(names[..], out[i], l[..]); }
  print("\n");
}

main :: () { skips(); locators(); mainnet(); regtest(); }
```

```shell
coax/bin/coax run bitcoin/examples/block-index.coax
```

```text
skip pointers
  GetSkipHeight of 1 to 20: 0 0 1 0 1 4 1 0 1 8 1 8 1 12 9 0 1 16 1 16
  300000 entries, steps from 299999 to 0: 7 1: 6 150000: 58 262143: 7 299998: 1
  from 262143 (2^18 - 1) to any lower height: at most 105 steps, to 2; one parent at a time it is up to 262143
locators
  main tip 99999 (28 entries): 99999 99998 99997 99996 99995 99994 99993 99992 99991 99990 99989 99988 99986 99982 99974 99958 99926 99862 99734 99478 98966 97942 95894 91798 83606 67222 34454 0
  branch tip 99999 (28 entries): 99999b 99998b 99997b 99996b 99995b 99994b 99993b 99992b 99991b 99990b 99989b 99988b 99986b 99982b 99974b 99958b 99926b 99862b 99734b 99478b 98966b 97942b 95894b 91798b 83606b 67222b 34454 0
  the two tips' last common ancestor: 49999
mainnet headers 0 to 2016
  2017 accepted; tip 2016 00000000a141216a896c54f211301c436e557a8d55900637bbdce14c6c7bddef
  chainwork 000000000000000000000000000000000000000000000000000007e107e107e1
  best header 2016, transactions 2017, median time past 1233057128, time max 1233063531
  locator (22 entries): 2016 2015 2014 2013 2012 2011 2010 2009 2008 2007 2006 2005 2003 1999 1991 1975 1943 1879 1751 1495 983 0
  block 2016 again: ok
  block 2016 with one bit of its nonce changed: high-hash
  block 5 before block 4: prev-blk-not-found
regtest
  genesis 0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206, work 2, status 0x8b
  A1 to A5 with data: tip A5, best header A5
  headers on A5: nBits 0x207ffffe: bad-diffbits, time at the median time past 1296690403: time-too-old, version 3: bad-version, 2 hours and 1 second ahead: time-too-new
  B3, B4 on A2, work 10 against A5's 12: tip A5, best header A5
  B5, B6 headers only: tip A5, best header B6
  B6's data first: B6 has 0 transactions in its chain, sequence 1; tip A5, best header B6
  then B5's: sequence B5 10, B6 11; tip B6, best header B6
  fork of A5 from the active chain: A2, last common ancestor of A5 and B6: A2
  C7 then D7 headers on B6, D7's data first: sequence C7 13, D7 12; tip D7, best header C7
  with both at sequence 1, as after a restart, the lower position wins: C7
  F6 to F8 on A5, and F7 fails ConnectBlock: tip D7, best header D7, best invalid F8
  a header on F8: bad-prevblk; F7 again: duplicate-invalid
  invalidateblock B5: tip B4, then tip F6, best header F6, best invalid F8
  failed: B5 B6 C7 D7 F7 F8
  entries: G0=0x8b A1=0x8b A2=0x8b A3=0x8b A4=0x8b A5=0x8b B3=0x8b B4=0x8b B5=0xab B6=0xab C7=0xab D7=0xab F6=0x8b F7=0xab F8=0xab
  locator from the tip: F6 A5 A4 A3 A2 A1 G0
```

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/block-index.coax](../examples/checks/block-index.coax),
which loads the example:

- on random trees of 3,000 entries, `GetAncestor` finds the entry that
  following parents finds, and `LastCommonAncestor` the highest entry both
  reach;
- however regtest blocks, data and failures arrive, no block with its
  chain's data and no failure has more work than the tip.

<!-- source: bitcoin/examples/checks/block-index.coax check -->
```jai
// Skip pointers only shorten the walk: GetAncestor finds the same entry
// as following parents one at a time, on a random tree of 3,000 entries.
#forall "GetAncestor equals the walk through parents" seed: 1..<9 #steps 4000000000 {
  l: [3000] BiLink;
  rng := [1] BiRng.[BiRng.{s=seed}];
  l[0] = BiLink.{height=0, prev=-1, skip=-1};
  for i: 1..<3000 {
    p := ifx bi_rand(rng[..], 4) == 0 then bi_rand(rng[..], i) else i - 1;
    l[i] = BiLink.{height=l[p].height + 1, prev=p};
    bi_build_skip(l[..], i);
  }
  for k: 0..<300 {
    i := bi_rand(rng[..], 3000);
    h := bi_rand(rng[..], l[i].height + 1);
    j := i;
    while l[j].height > h { j = l[j].prev; }
    #assert bi_ancestor(l[..], i, h) == j;
    // LastCommonAncestor: the highest entry both reach.
    o := bi_rand(rng[..], 3000);
    a := i; b := o;
    while a != b { if l[a].height >= l[b].height { a = l[a].prev; } else { b = l[b].prev; } }
    #assert bi_last_common_ancestor(l[..], i, o) == a;
  }
}
// Whatever order regtest blocks arrive in, the tip is the candidate
// FindMostWorkChain must choose: no block with data on a fully linked,
// unfailed chain has more work.
#forall "the tip has the most work of any linked block" seed: 1..<7 #steps 4000000000 {
  l: [48] BiLink; e: [48] BiEntry; slots: [128] int; chain: [48] int; t: [1] BlockTree;
  bi_regtest(t[..], l[..], e[..], slots[..], chain[..]);
  rng := [1] BiRng.[BiRng.{s=seed * 977}];
  for k: 0..<40 {
    p := bi_rand(rng[..], t[0].count);
    if e[p].status & BLOCK_FAILED_VALID == 0 {
      c := bi_extend(t[..], p, 1, cast,trunc(u8) (k + 1), bi_rand(rng[..], 3) != 0);
    }
    if bi_rand(rng[..], 9) == 0 { bi_invalid_block_found(t[..], 1 + bi_rand(rng[..], t[0].count - 1), false); }
    if bi_rand(rng[..], 4) == 0 {
      w := bi_rand(rng[..], t[0].count);
      if e[w].status & BLOCK_HAVE_DATA == 0 { bi_received_block_vartime(t[..], w, 1); }
    }
    tip := bi_activate_best_chain(t[..]);
    for i: 0..<t[0].count {
      if e[i].chain_tx != 0 && e[i].status & BLOCK_FAILED_VALID == 0 {
        #assert !bi_worse(e[..], tip, i) || e[tip].status & BLOCK_FAILED_VALID != 0;
      }
    }
  }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/block-index.coax
```

`coax test` runs 301 checks, including the loaded files' own checks, in about 40 seconds on 2026-11-03.

### The library's own checks

`blockindex.coax` is an `#authority` file. Its checks port Core's
`skiplist_tests.cpp` (`skiplist_test` on 300,000 entries,
`getlocator_test` with its branch, and `build_skip_height_test`'s 36
pairs) and `blockchain_tests.cpp` (the five `GetDifficulty` cases and
`invalidate_block` on 100 regtest blocks). They also check the status
levels, the tie-breaks of the comparator, blocks that arrive before their
parents, an invalid block and its descendants, and the first 2,017 mainnet
headers. Core's random draws come from its own generator; the ports draw
from a Park-Miller generator instead.

```shell
coax/bin/coax test coax/lib/bitcoin/blockindex.coax
```

`coax test` runs 299 checks, including those of the files it loads, in about 40 seconds on 2026-11-03. Mutation testing: not run (owner speed order).

### Python

[blockindex.py](../../coax/tests/bitcoin/blockindex.py) transliterates
Core with object references instead of positions: `GetSkipHeight`,
`GetAncestor`, `LastCommonAncestor` and `LocatorEntries` from `chain.cpp`,
`AddToBlockIndex` and `CBlockIndexWorkComparator` from `blockstorage.cpp`,
`AcceptBlockHeader`, `ContextualCheckBlockHeader`,
`ReceivedBlockTransactions`, `FindMostWorkChain`, `InvalidChainFound` and
`InvalidateBlock`'s candidate rules from `validation.cpp`, and
`GetNextWorkRequired` from `pow.cpp`. It reads the status values, the
sequence ids and each network's parameters from the pinned files, checks
that the lines it transliterates are there, and writes the transcript the
example must print.
[make_blockindex_vectors.py](../../coax/tests/bitcoin/make_blockindex_vectors.py)
writes the pinned headers into the library's vectors.

```shell
python3 -I coax/tests/bitcoin/blockindex.py --coax coax/bin/coax
```

On 2026-11-03 all 33 lines agree with Python, and the example's output in native debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Connecting and disconnecting blocks: chapter 58. Moving the tip block by
  block, and a reorganization that fails halfway: chapter 60.
- Storing the index on disk (`CDiskBlockIndex`) and loading it at start,
  where sequence ids 0 and 1 come from: chapters 61 to 63.
- The anti-DoS work threshold before a header is stored: chapter 71. Headers
  sync and choosing what to download: chapter 72.
- assumevalid and AssumeUTXO, which use the index to skip or defer work:
  chapters 78 and 79.

## Summary

- The block index holds every accepted header as a tree, each entry with
  its height, chainwork, status flags, parent and skip pointer.
- Skip pointers to `GetSkipHeight` let `GetAncestor` reach any height in
  O(log n) steps; `LastCommonAncestor`, `FindFork` and the locator build
  on it.
- `AcceptBlockHeader` checks a known header, the proof of work, the
  parent and then the header rules, in that order.
- The tip is the candidate with the most work; ties go to the block whose
  data arrived first, then to the lower pointer.
- A block with no data, or with an ancestor that has none, is never the
  tip.
- A failed block marks every descendant failed; headers built on them are
  refused.

## Dependencies

- [48. Compact targets, retargeting and chainwork](ch048-compact-targets-retargeting-and-chainwork.md)
- [51. Header rules: time, versions and deployments](ch051-header-rules-time-versions-and-deployments.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `chain.h` and `chain.cpp`
  (`CBlockIndex`, `GetSkipHeight`, `GetAncestor`, `LastCommonAncestor`,
  `LocatorEntries`, `CChain`), `node/blockstorage.cpp`
  (`AddToBlockIndex`, `CBlockIndexWorkComparator`), `validation.cpp` and
  `validation.h` (`AcceptBlockHeader`, `ReceivedBlockTransactions`,
  `FindMostWorkChain`, `InvalidChainFound`, `InvalidateBlock`,
  `nBlockSequenceId`), `pow.cpp`, `kernel/chainparams.cpp`,
  `rpc/blockchain.cpp` (`GetDifficulty`), and `test/skiplist_tests.cpp`
  and `test/blockchain_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/skiplist_tests.cpp).
- Mainnet's headers 0 to 2,016, from mempool.space, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/headers-0-2016.txt).
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
