# 55. Search without scanning everything

> **Status: partly implemented.** The HNSW index, its recall against
> exhaustive search and its saved form run on fixed unit vectors. Chapter 56
> builds it over chapter 54's passage embeddings, but only in the book-search
> tool's evaluation; the saved bytes are not yet written to a file on disk,
> and cost is reported as similarities computed, not wall-clock time.

[Chapter 54](ch054-search-the-coax-book.md) searches embeddings
exhaustively: every query computes its similarity to every stored vector.
That is exact, and its cost grows with the corpus. This chapter builds an
approximate index from scratch, a hierarchical navigable small world graph
(HNSW), and measures what it gives up. For each search budget it reports
recall against the exhaustive answer, the number of similarities the search
computed, and the bytes the index holds.

Everything runs on one CPU core. The library is
[hnsw.coax](../lib/book-search/hnsw.coax). It reuses chapter 54's hit type
(`BsHit`), tie rule (`bs_before`), top-k selection (`bs_top`) and dot product
(`bs_dot`) from [rank.coax](../lib/book-search/rank.coax), and the counter
hash `cr_bits` from [counter-rng.coax](../lib/counter-rng.coax).

## Run it

```shell
coax/bin/coax run psionic/examples/hnsw.coax
coax/bin/coax run psionic/examples/hnsw-refuse.coax
psionic/check.sh --only book-search
```

## The idea

An HNSW index is a stack of graphs over the same points. Every point is on
level 0. A quarter of them are also on level 1, a sixteenth on level 2, and so
on. Each point keeps a short list of links to similar points on every level
it is on: at most `m` links on levels 1 and up, and at most `2m` on level 0.

A search starts at the entry point, the first point that reached the top
level. On each upper level it moves greedily: it keeps the single best point
seen so far, reads that point's links, and moves whenever a neighbor is more
similar to the query. The few points on the upper levels are far apart, so
these moves are long. On level 0 it keeps a working list of `ef` points
instead of one. `ef` is the search budget: a larger list explores more of the
graph, finds more of the true nearest neighbors, and computes more
similarities.

Insertion uses the same search. A new point walks down to its own top level,
then on each level it joins it searches with a working list of `ef_build`,
links to the `m` best points found, and each of them links back.

Similarity is the dot product, as in chapter 54, so higher is better. Ties
break to the lower point id, using chapter 54's rule.

## Deterministic levels

The original paper draws each point's level from an exponential
distribution. Here the level comes from a hash of the point's id, so the same
points always build the same graph. `cr_bits(seed, id)` is the splitmix64
counter hash that chapter 54's sampler uses. Each pair of its low bits is zero
with probability 1/4, so counting zero pairs gives level 1 or more with
probability 1/4, level 2 or more with probability 1/16, and so on.

<!-- source: psionic/lib/book-search/hnsw.coax level -->
```jai
// A point's level: how many of the lowest bit pairs of its hash are zero, up
// to 15. Each pair is zero with probability 1/4, so a quarter of the points
// reach level 1, a sixteenth level 2, and so on. The hash is chapter 54's
// counter generator, keyed by the index's seed and the point's id.
hn_level_bits :: (h: u64) -> int {
  l := 0; x := h;
  while l < HN_LEVEL_MAX && (x & 3) == 0 { l += 1; x = x >> 2; }
  return l;
}
hn_level :: (seed: u64, id: int) -> int { return hn_level_bits(cr_bits(seed, cast(u64) id)); }
```

For seed 5, the hashes of ids 0 to 5 end in these bytes:

| id | low byte of `cr_bits(5, id)` | low bit pairs | level |
| ---: | --- | --- | ---: |
| 0 | `0101 1010` | `10` | 0 |
| 1 | `1111 1000` | `00`, then `10` | 1 |
| 2 | `0100 0111` | `11` | 0 |
| 3 | `0100 0101` | `01` | 0 |
| 4 | `1100 0101` | `01` | 0 |
| 5 | `0100 0100` | `00`, then `01` | 1 |

Over 4,096 ids with seed 7, 1,073 reach level 1 (expected 1,024) and 264
reach level 2 (expected 256).

## Storage

The caller supplies every array, and `hn_init` checks that they fit before
the index uses them. Vectors have 1 to 1,024 values, `m` is 2 to 32, and
`ef_build` is `m` to 512. Level-0 links take `2m` slots for every point.
Links on higher levels come from a pool, `m` slots per level, handed out in
insertion order; a point whose level needs more pool than is left is refused.

<!-- source: psionic/lib/book-search/hnsw.coax init -->
```jai
// Checks the parameters and the storage, then empties the index. Neighbor
// lists hold m links on levels 1 and up and 2m on level 0; ef_build is the
// working-list size used while inserting.
hn_init :: (ix: [] HnIndex, dim: int, m: int, ef_build: int, seed: u64,
            vectors: [] f32, level: [] int, upper: [] int, base: [] int, pool: [] int, seen: [] int) -> HnStatus {
  if dim < 1 || dim > HN_DIM_MAX || m < 2 || m > HN_M_MAX { return HnStatus.shape; }
  if ef_build < m || ef_build > HN_EF_MAX { return HnStatus.budget; }
  n := level.count;
  if n < 1 || upper.count != n || seen.count != n { return HnStatus.shape; }
  if vectors.count < n * dim || base.count < n * 2 * m { return HnStatus.shape; }
  ix[0].dim = dim; ix[0].m = m; ix[0].ef_build = ef_build; ix[0].seed = seed;
  ix[0].capacity = n; ix[0].count = 0; ix[0].entry = -1; ix[0].top = -1; ix[0].used = 0;
  ix[0].vectors = vectors; ix[0].level = level; ix[0].upper = upper; ix[0].base = base; ix[0].pool = pool; ix[0].seen = seen;
  for i: 0..<n { seen[i] = 0; }
  return HnStatus.ok;
}
```

## The working list

A search keeps its working list sorted best first. A hit joins the list if
the list has room or the hit is better than the current last entry; the last
entry falls off a full list. Each entry also records whether its links have
been read.

<!-- source: psionic/lib/book-search/hnsw.coax offer -->
```jai
// Offers one hit to the working list of at most ef hits, sorted best first.
// A hit that is not better than the last of a full list is turned away; the
// list keeps which of its hits have had their links read (done).
hn_offer :: (ix: [] HnIndex, hit: BsHit, ef: int) {
  n := ix[0].wn;
  if n == ef && !bs_before(hit, ix[0].w[n - 1]) { return; }
  if n < ef { n += 1; }
  j := n - 1;
  while j > 0 && bs_before(hit, ix[0].w[j - 1]) { ix[0].w[j] = ix[0].w[j - 1]; ix[0].done[j] = ix[0].done[j - 1]; j -= 1; }
  ix[0].w[j] = hit; ix[0].done[j] = false; ix[0].wn = n;
}
```

One level's search repeatedly reads the links of the best entry not yet read.
It stops when every entry has been read. That is the same stopping rule as
the paper's two-queue version: a point that is not in the list is worse than
everything in it, so its links would only be read after all of them.

A stamp marks which points this level's search has already scored. The stamp
increases by one for each level searched, so the marks never need clearing.
The count of level searches is reported with every result.

<!-- source: psionic/lib/book-search/hnsw.coax layer -->
```jai
// Searches one level, starting from the hits already in the working list.
// Each round reads the links of the best hit not yet read; a neighbor seen
// for the first time is scored and offered. The search ends when every hit
// in the list has been read: a better point could only be reached through
// one of them.
hn_layer :: (ix: [] HnIndex, q: [] f32, layer: int, ef: int) {
  while true {
    i := 0;
    while i < ix[0].wn && ix[0].done[i] { i += 1; }
    if i == ix[0].wn { return; }
    ix[0].done[i] = true;
    links := hn_links(ix, ix[0].w[i].passage, layer);
    for e: links {
      if e < 0 { break; }
      if ix[0].seen[e] == ix[0].stamp { continue; }
      ix[0].seen[e] = ix[0].stamp;
      hn_offer(ix, BsHit.{passage = e, score = hn_sim(ix, q, e)}, ef);
    }
  }
}
// A level search starts from the hits in the working list, none read yet.
// The stamp numbers the level searches: a point is seen in this one when
// its seen value equals the stamp, so nothing needs clearing.
hn_restart :: (ix: [] HnIndex) {
  ix[0].stamp += 1;
  for i: 0..<ix[0].wn { ix[0].done[i] = false; ix[0].seen[ix[0].w[i].passage] = ix[0].stamp; }
}
// A search starts from the entry point alone.
hn_start :: (ix: [] HnIndex, q: [] f32) {
  ix[0].w[0] = BsHit.{passage = ix[0].entry, score = hn_sim(ix, q, ix[0].entry)}; ix[0].wn = 1;
}
```

## Links stay sorted

Every neighbor list is kept in order of similarity to its owner, best first.
Adding a link to a full list drops the least similar of the candidates, which
can be the new link itself. This is the paper's simple neighbor selection,
without its diversity heuristic.

<!-- source: psionic/lib/book-search/hnsw.coax link -->
```jai
// Adds a link from owner to id on one level, in order of similarity to the
// owner. When the list is full, the least similar of the m + 1 (or 2m + 1)
// candidates is dropped, which may be the new link.
hn_link :: (ix: [] HnIndex, owner: int, layer: int, id: int) {
  list := hn_links(ix, owner, layer);
  me := hn_vector(ix, owner);
  hit := BsHit.{passage = id, score = hn_sim(ix, me, id)};
  j := 0;
  while j < list.count && list[j] >= 0 && bs_before(BsHit.{passage = list[j], score = hn_sim(ix, me, list[j])}, hit) { j += 1; }
  if j == list.count { return; }
  k := list.count - 1;
  while k > j { list[k] = list[k - 1]; k -= 1; }
  list[j] = id;
}
```

## Insert

<!-- source: psionic/lib/book-search/hnsw.coax add -->
```jai
// Inserts the next point (its id is the current count). From the entry
// point, a greedy search (a working list of one) walks down every level
// above the new point's own; on each level the point joins, a search with
// ef_build finds its m nearest, which become its links, and each of them
// links back. A point above the old top becomes the entry point.
// found reports the point's level; sims, the similarities computed.
hn_add :: (ix: [] HnIndex, v: [] f32) -> HnResult {
  if v.count != ix[0].dim { return HnResult.{status = HnStatus.dimension}; }
  if !hn_values_ok(v) { return HnResult.{status = HnStatus.number}; }
  id := ix[0].count; m := ix[0].m;
  if id == ix[0].capacity { return HnResult.{status = HnStatus.capacity}; }
  l := hn_level(ix[0].seed, id);
  if ix[0].used + l * m > ix[0].pool.count { return HnResult.{status = HnStatus.capacity}; }
  before := ix[0].sims; stamp := ix[0].stamp;
  ix[0].level[id] = l; ix[0].upper[id] = ix[0].used; ix[0].used += l * m;
  me := hn_vector(ix, id);
  for d: 0..<v.count { me[d] = v[d]; }
  for layer: 0..<l + 1 { list := hn_links(ix, id, layer); for i: 0..<list.count { list[i] = -1; } }
  ix[0].count = id + 1;
  if id == 0 { ix[0].entry = 0; ix[0].top = l; return HnResult.{status = HnStatus.ok, found = l}; }
  hn_start(ix, me);
  layer := ix[0].top;
  while layer > l { hn_restart(ix); hn_layer(ix, me, layer, 1); layer -= 1; }
  while layer >= 0 {
    hn_restart(ix); hn_layer(ix, me, layer, ix[0].ef_build);
    list := hn_links(ix, id, layer);
    j := 0;
    while j < m && j < ix[0].wn { list[j] = ix[0].w[j].passage; j += 1; }
    for e: list { if e < 0 { break; } hn_link(ix, e, layer, id); }
    layer -= 1;
  }
  if l > ix[0].top { ix[0].entry = id; ix[0].top = l; }
  return HnResult.{status = HnStatus.ok, found = l, sims = ix[0].sims - before, levels = ix[0].stamp - stamp};
}
```

## Search

<!-- source: psionic/lib/book-search/hnsw.coax search -->
```jai
// The k points most similar to q, best first, using a working list of ef on
// level 0 (k <= ef <= 512). The levels above are walked greedily from the
// entry point, as in hn_add. sims counts the similarities computed.
hn_search :: (ix: [] HnIndex, q: [] f32, k: int, ef: int, out: [] BsHit) -> HnResult {
  if q.count != ix[0].dim { return HnResult.{status = HnStatus.dimension}; }
  if k < 1 || ef < k || ef > HN_EF_MAX { return HnResult.{status = HnStatus.budget}; }
  if out.count < k { return HnResult.{status = HnStatus.shape}; }
  if !hn_values_ok(q) { return HnResult.{status = HnStatus.number}; }
  if ix[0].count == 0 { return HnResult.{status = HnStatus.empty}; }
  before := ix[0].sims; stamp := ix[0].stamp;
  hn_start(ix, q);
  layer := ix[0].top;
  while layer > 0 { hn_restart(ix); hn_layer(ix, q, layer, 1); layer -= 1; }
  hn_restart(ix); hn_layer(ix, q, 0, ef);
  n := 0;
  while n < k && n < ix[0].wn { out[n] = ix[0].w[n]; n += 1; }
  return HnResult.{status = HnStatus.ok, found = n, sims = ix[0].sims - before, levels = ix[0].stamp - stamp};
}
```

## A trace by hand

Six points on the unit circle, with `m = 2` (so 4 level-0 slots per point),
`ef_build = 2` and seed 5:

| id | point | level |
| ---: | --- | ---: |
| 0 | (1, 0) | 0 |
| 1 | (0.6, 0.8) | 1 |
| 2 | (0, 1) | 0 |
| 3 | (-0.6, 0.8) | 0 |
| 4 | (-1, 0) | 0 |
| 5 | (0.6, -0.8) | 1 |

Points 0 to 4 build this level-0 graph, each list best first. Point 1 became
the entry point when it was inserted, because its level 1 was above point 0's
level 0. It has no level-1 links yet.

| point | level-0 links | similarities to the owner |
| ---: | --- | --- |
| 0 | 1, 2 | 0.6, 0 |
| 1 | 2, 0, 3 | 0.8, 0.6, 0.28 |
| 2 | 1, 3, 0, 4 | 0.8, 0.8, 0, 0 |
| 3 | 2, 4, 1 | 0.8, 0.6, 0.28 |
| 4 | 3, 2 | 0.6, 0 |

**Insert point 5**, (0.6, -0.8), on levels 0 and 1. Its similarities to the
other points are 0.6 (point 0), -0.28 (1), -0.8 (2), -1 (3) and -0.6 (4).

1. Start at the entry point, point 1: similarity -0.28. That is 1
   similarity.
2. Level 1: point 1 has no level-1 links, so the list stays `[1]`. Point 5's
   level-1 list becomes `[1]`, and point 1 links back to 5 (1 similarity).
3. Level 0, working list of 2, starting from `[1]`. Read point 1's links:
   2 (-0.8) joins, giving `[1, 2]`; 0 (0.6) joins at the front and pushes 2
   out, giving `[0, 1]`; 3 (-1) is worse than the last entry and is turned
   away. That is 3 similarities. Read point 0's links: 1 and 2 were already
   scored on this level. Every entry has been read, so the level is done.
4. Point 5's level-0 list becomes `[0, 1]`. Point 0 links back: 5 ties point
   1 at 0.6 and ranks after it by id, and beats point 2 at 0, so point 0's
   list becomes `[1, 5, 2]` (3 similarities). Point 1 links back: 5 at -0.28
   is below 2, 0 and 3 and takes the last free slot, `[2, 0, 3, 5]` (4
   similarities).

The insertion computed 1 + 1 + 3 + 3 + 4 = 12 similarities. The check
"the six circle points build the graph traced in the chapter" asserts every
link slot and every per-insertion count: 0, 2, 6, 9, 11 and 12.

**Search for (0.8, -0.6) with k = 2 and ef = 2.** The query's similarities
are 0.8 (point 0), 0 (1), -0.6 (2), -0.96 (3), -0.8 (4) and 0.96 (5).

1. Start at point 1: similarity 0.
2. Level 1, list of one: read point 1's level-1 links. Point 5 scores 0.96
   and replaces point 1. Read point 5's links: point 1 was already scored.
3. Level 0, list of 2, starting from `[5]`. Read point 5's links: point 0
   (0.8) joins, giving `[5, 0]`; point 1 is scored again, because each level
   starts with a new stamp, and at 0 it is turned away. Read point 0's links:
   1 and 5 were scored on this level, and 2 (-0.6) is turned away.
4. The answer is points 5 and 0, the true two nearest, after 1 + 1 + 2 + 1 =
   5 similarities. Exhaustive search computes 6.

With `ef = 4` the same search returns 5, 0, 1 and 2 after 7 similarities,
more than exhaustive search. On six points a graph saves nothing. The
saving comes from large corpora, measured below.

## The exhaustive oracle

The reference answer scores every point and keeps the best k with chapter
54's `bs_top`. Recall at k is the number of ids the index returns that are
also in the oracle's top k, divided by k. Both use the same tie rule, so equal
scores can't make them disagree.

<!-- source: psionic/lib/book-search/hnsw.coax exact -->
```jai
// The oracle: score every point and keep the best k with chapter 54's
// bs_top. scores holds one value per point.
hn_exact :: (ix: [] HnIndex, q: [] f32, k: int, scores: [] f64, out: [] BsHit) -> HnResult {
  if q.count != ix[0].dim { return HnResult.{status = HnStatus.dimension}; }
  if k < 1 { return HnResult.{status = HnStatus.budget}; }
  if out.count < k || scores.count < ix[0].count { return HnResult.{status = HnStatus.shape}; }
  for p: 0..<ix[0].count { scores[p] = bs_dot(q, hn_vector(ix, p)); }
  n := bs_top(scores[0..<ix[0].count], -1.0e300, out[0..<k]);
  return HnResult.{status = HnStatus.ok, found = n, sims = ix[0].count};
}
// How many of found's ids also appear in exact: recall at k is this over k.
hn_shared :: (found: [] BsHit, exact: [] BsHit) -> int {
  n := 0;
  for a: found { for b: exact { if a.passage == b.passage { n += 1; break; } } }
  return n;
}
```

## Memory and the saved form

<!-- source: psionic/lib/book-search/hnsw.coax size -->
```jai
// Bytes held for the stored points: each point's vector (4 bytes a value),
// its level, pool offset and search stamp (8 bytes each), its 2m level-0
// link slots (8 bytes each), and every pool slot handed out (8 bytes each).
hn_memory :: (ix: [] HnIndex) -> int {
  return ix[0].count * (4 * ix[0].dim + 24 + 16 * ix[0].m) + 8 * ix[0].used;
}
// The saved file: an 80-byte header, a byte per level, 4 bytes per value
// and per link slot, and an 8-byte checksum.
hn_file_size :: (ix: [] HnIndex) -> int {
  n := ix[0].count;
  return HN_HEADER + n + 4 * n * ix[0].dim + 8 * n * ix[0].m + 4 * ix[0].used + 8;
}
```

For the six circle points that is 6 × (8 + 24 + 32) + 4 × 8 = 416 bytes in
memory, and a file of 80 + 6 + 48 + 96 + 16 + 8 = 254 bytes.

The saved form is little-endian: an 8-byte magic, `PSHNSW01`; nine 8-byte
words (version, dimension, `m`, `ef_build`, seed, point count, entry point
plus one, top level plus one, pool slots used); one byte per point's level;
each value's binary32 bits; every link slot plus one, so an empty slot is 0;
and an FNV-1a 64-bit checksum of everything before it.

<!-- source: psionic/lib/book-search/hnsw.coax save -->
```jai
// Writes the index into out and returns its length, or -1 when out is too
// small. Links are stored plus one, so an empty slot is 0.
hn_save :: (ix: [] HnIndex, out: [] u8) -> int {
  size := hn_file_size(ix);
  if out.count < size { return -1; }
  n := ix[0].count; d := ix[0].dim; m := ix[0].m;
  for i: 0..<8 { out[i] = HN_MAGIC[i]; }
  head := [9] u64.[cast(u64) HN_VERSION, cast(u64) d, cast(u64) m, cast(u64) ix[0].ef_build, ix[0].seed,
    cast(u64) n, cast(u64) (ix[0].entry + 1), cast(u64) (ix[0].top + 1), cast(u64) ix[0].used];
  for i: 0..<9 { hn_put(out, 8 + 8 * i, head[i]); }
  at := HN_HEADER;
  for i: 0..<n { out[at] = cast(u8) ix[0].level[i]; at += 1; }
  for i: 0..<n * d { hn_put4(out, at, f32_bits(ix[0].vectors[i])); at += 4; }
  for i: 0..<n * 2 * m { hn_put4(out, at, cast(u32) (ix[0].base[i] + 1)); at += 4; }
  for i: 0..<ix[0].used { hn_put4(out, at, cast(u32) (ix[0].pool[i] + 1)); at += 4; }
  hn_put(out, at, hn_fnv(out[0..<at]));
  return size;
}
```

Loading checks more than the checksum. Because levels come from the seed,
the loader recomputes every level and refuses a file whose levels differ. It
also checks that the header's sizes account for the file's length exactly,
that the entry point is the first point on the top level, and that every link
names another point on that level.

<!-- source: psionic/lib/book-search/hnsw.coax load -->
```jai
// Reads a saved index into ix's storage. The file must carry a matching
// checksum and be exactly the size its header implies. Then every stored
// fact is checked against the rules that built it: each level is the seed's
// level for that id, the pool holds m slots per level above 0, the entry is
// the first point on the top level, every value is finite, and every link
// names another point on that level, filled slots first. A valid file that
// does not fit ix's storage is refused with capacity.
hn_load :: (ix: [] HnIndex, file: [] u8) -> HnStatus {
  if file.count < HN_HEADER + 8 { return HnStatus.format; }
  if hn_fnv(file[0..<file.count - 8]) != hn_get(file, file.count - 8) { return HnStatus.format; }
  if cast(string) file[0..<8] != HN_MAGIC || hn_get(file, 8) != cast(u64) HN_VERSION { return HnStatus.format; }
  dw := hn_get(file, 16); mw := hn_get(file, 24); efw := hn_get(file, 32); seed := hn_get(file, 40);
  if dw < 1 || dw > cast(u64) HN_DIM_MAX || mw < 2 || mw > cast(u64) HN_M_MAX || efw < mw || efw > cast(u64) HN_EF_MAX { return HnStatus.format; }
  d := cast(int) dw; m := cast(int) mw; ef := cast(int) efw;
  // The header's sizes must account for the file's length exactly.
  room := file.count - HN_HEADER - 8; per := 1 + 4 * d + 8 * m;
  if hn_get(file, 48) > cast(u64) (room / per) { return HnStatus.format; }
  n := cast(int) hn_get(file, 48); rest := room - n * per;
  if rest % 4 != 0 || hn_get(file, 72) != cast(u64) (rest / 4) { return HnStatus.format; }
  used := rest / 4;
  vectors := ix[0].vectors; level := ix[0].level; upper := ix[0].upper; base := ix[0].base; pool := ix[0].pool; seen := ix[0].seen;
  if hn_init(ix, d, m, ef, seed, vectors, level, upper, base, pool, seen) != HnStatus.ok { return HnStatus.capacity; }
  if n > ix[0].capacity || used > pool.count { return HnStatus.capacity; }
  at := HN_HEADER; total := 0; top := -1; entry := -1;
  for i: 0..<n {
    l := cast(int) file[at]; at += 1;
    if l != hn_level(seed, i) { return HnStatus.format; }
    level[i] = l; upper[i] = total; total += l * m;
    if l > top { top = l; entry = i; }
  }
  if total != used || hn_get(file, 56) != cast(u64) (entry + 1) || hn_get(file, 64) != cast(u64) (top + 1) { return HnStatus.format; }
  for i: 0..<n * d {
    bits := hn_get4(file, at); at += 4;
    if (bits >> 23) & 0xff == 0xff { return HnStatus.format; }
    vectors[i] = f32_from_bits(bits);
  }
  if !hn_values_ok(vectors[0..<n * d]) { return HnStatus.format; }
  for i: 0..<n * 2 * m { base[i] = cast(int) hn_get4(file, at) - 1; at += 4; }
  for i: 0..<used { pool[i] = cast(int) hn_get4(file, at) - 1; at += 4; }
  for id: 0..<n {
    for layer: 0..<level[id] + 1 {
      empty := false;
      for e: hn_links(ix, id, layer) {
        if e < 0 { empty = true; continue; }
        if empty || e >= n || e == id || level[e] < layer { return HnStatus.format; }
      }
    }
  }
  ix[0].count = n; ix[0].entry = entry; ix[0].top = top; ix[0].used = used;
  return HnStatus.ok;
}
```

## Measure it

[hnsw.coax](../examples/hnsw.coax) builds indexes over 500 and 4,000 unit
vectors of 16 values with `m = 8` and `ef_build = 64`. The vectors come from
the counter hash, uniform in [-1, 1) in each coordinate, scaled to unit
length; the 200 queries come from the same hash with another seed. For each
`ef` it reports recall at 10 against the exhaustive oracle and the mean number
of similarities per query. Then it saves the larger index, reloads it into
separate storage, and runs every query on both.

<!-- source: psionic/examples/hnsw.coax example -->
```jai
N :: 4000; D :: 16; M :: 8; EF_BUILD :: 64; QUERIES :: 200; K :: 10;
POOL :: 16000; FILE :: 900000;
Store :: struct { v: [N * D] f32; level: [N] int; upper: [N] int; base: [N * 2 * M] int; pool: [POOL] int; seen: [N] int; }

// Prints num / den to three decimals, rounded half up.
put3 :: (num: int, den: int) {
  t := (num * 2000 + den) / (2 * den);
  print("%.", t / 1000); print("%", t / 100 % 10); print("%", t / 10 % 10); print("%", t % 10);
}

build :: (ix: [] HnIndex, store: [] Store, s: int, n: int) -> int {
  st := hn_init(ix, D, M, EF_BUILD, 55, store[s].v[0..<n * D], store[s].level[0..<n], store[s].upper[0..<n], store[s].base[0..<n * 2 * M], store[s].pool[..], store[s].seen[0..<n]);
  if st != HnStatus.ok { print("init: %\n", hn_status_name(st)); return -1; }
  v: [D] f32; sims := 0;
  for i: 0..<n {
    hn_fixture(1, i, v[..]);
    r := hn_add(ix, v[..]);
    if r.status != HnStatus.ok { print("add: %\n", hn_status_name(r.status)); return -1; }
    sims += r.sims;
  }
  return sims;
}

measure :: (ix: [] HnIndex, n: int) {
  efs := [5] int.[10, 20, 40, 80, 160];
  shared: [5] int; sims: [5] int;
  scores: [N] f64; exact: [K] BsHit; found: [K] BsHit; q: [D] f32;
  for qi: 0..<QUERIES {
    hn_fixture(2, qi, q[..]);
    e := hn_exact(ix, q[..], K, scores[..], exact[..]);
    for b: 0..<5 {
      r := hn_search(ix, q[..], K, efs[b], found[..]);
      shared[b] += hn_shared(found[0..<r.found], exact[0..<e.found]); sims[b] += r.sims;
    }
  }
  for b: 0..<5 {
    print("  ef %: recall@10 ", efs[b]); put3(shared[b], QUERIES * K);
    print(", "); put3(sims[b], QUERIES); print(" similarities per query, "); put3(sims[b], QUERIES * n); print(" of exhaustive\n");
  }
  print("  exhaustive: recall 1.000 at % similarities per query\n", n);
}

main :: () {
  ix: [1] HnIndex; store: [2] Store; file: [FILE] u8;
  sizes := [2] int.[500, N];
  for s: sizes {
    built := build(ix[..], store[..], 0, s);
    if built < 0 { return; }
    count: [HN_LEVEL_MAX + 1] int;
    for i: 0..<s { count[store[0].level[i]] += 1; }
    print("% unit vectors of % values; m %, ef_build %, seed 55\n", s, D, M, EF_BUILD);
    print("  points whose highest level is 0");
    for l: 1..<ix[0].top + 1 { print(" / %", l); }
    print(": %", count[0]);
    for l: 1..<ix[0].top + 1 { print(" / %", count[l]); }
    print("; entry point % on level %\n", ix[0].entry, ix[0].top);
    mem := hn_memory(ix[..]);
    print("  build: % similarities; memory % bytes (% per point, % of them the vector)\n", built, mem, mem / s, 4 * D);
    measure(ix[..], s);
  }
  // Save the larger index, reload it into separate storage, and compare.
  size := hn_save(ix[..], file[..]);
  copy: [1] HnIndex;
  st := hn_init(copy[..], 1, 2, 2, 0, store[1].v[..], store[1].level[..], store[1].upper[..], store[1].base[..], store[1].pool[..], store[1].seen[..]);
  st = hn_load(copy[..], file[0..<size]);
  print("saved % bytes (% per point); reload: %\n", size, size / N, hn_status_name(st));
  same := 0; a: [K] BsHit; b: [K] BsHit; q: [D] f32;
  for qi: 0..<QUERIES {
    hn_fixture(2, qi, q[..]);
    ra := hn_search(ix[..], q[..], K, 40, a[..]); rb := hn_search(copy[..], q[..], K, 40, b[..]);
    ok := ra.found == rb.found && ra.sims == rb.sims;
    for i: 0..<ra.found { if a[i].passage != b[i].passage || a[i].score != b[i].score { ok = false; } }
    if ok { same += 1; }
  }
  print("reloaded index at ef 40: % of % queries return the same hits, scores and cost\n", same, QUERIES);
}
```

```text
500 unit vectors of 16 values; m 8, ef_build 64, seed 55
  points whose highest level is 0 / 1 / 2 / 3 / 4 / 5: 375 / 99 / 20 / 4 / 1 / 1; entry point 66 on level 5
  build: 138598 similarities; memory 118240 bytes (236 per point, 64 of them the vector)
  ef 10: recall@10 0.866, 129.140 similarities per query, 0.258 of exhaustive
  ef 20: recall@10 0.965, 185.405 similarities per query, 0.371 of exhaustive
  ef 40: recall@10 0.997, 269.260 similarities per query, 0.539 of exhaustive
  ef 80: recall@10 1.000, 373.145 similarities per query, 0.746 of exhaustive
  ef 160: recall@10 1.000, 471.550 similarities per query, 0.943 of exhaustive
  exhaustive: recall 1.000 at 500 similarities per query
4000 unit vectors of 16 values; m 8, ef_build 64, seed 55
  points whose highest level is 0 / 1 / 2 / 3 / 4 / 5 / 6: 2975 / 758 / 198 / 52 / 11 / 3 / 3; entry point 504 on level 6
  build: 2258808 similarities; memory 952768 bytes (238 per point, 64 of them the vector)
  ef 10: recall@10 0.733, 174.450 similarities per query, 0.044 of exhaustive
  ef 20: recall@10 0.878, 255.010 similarities per query, 0.064 of exhaustive
  ef 40: recall@10 0.959, 396.470 similarities per query, 0.099 of exhaustive
  ef 80: recall@10 0.995, 642.575 similarities per query, 0.161 of exhaustive
  ef 160: recall@10 1.000, 1033.805 similarities per query, 0.258 of exhaustive
  exhaustive: recall 1.000 at 4000 similarities per query
saved 560472 bytes (140 per point); reload: ok
reloaded index at ef 40: 200 of 200 queries return the same hits, scores and cost
```

At 500 points, a search that finds 99.7% of the true top 10 (`ef` 40) still
computes more than half as many similarities as exhaustive search. At 4,000
points the same `ef` finds 95.9% for under a tenth of the cost, and `ef` 160
finds all of them for about a quarter. Recall at a fixed `ef` falls as the
corpus grows, so the budget has to be chosen for the corpus size, not once.

The index holds 238 bytes per point in memory for 16-value vectors: 64 for
the vector, 24 for its level, pool offset and search stamp, 128 for 16
level-0 link slots, and the rest for the pool. The saved file is 140 bytes per
point, because it stores links in 4 bytes and levels in 1. The links, not the
vectors, dominate at this size; with chapter 54's 64-value vectors the vector
share rises to 256 bytes.

These figures are similarities computed, not seconds. A similarity costs the
same in both searches, but a graph search also pays for sorting its working
list and following links, so the wall-clock ratio will be worse than the
similarity ratio. Chapter 54's index has 4,956 passages; this chapter has not
measured either search's time on it.

## Refusals

A malformed request gets a typed status, never a guess.
[hnsw-refuse.coax](../examples/hnsw-refuse.coax) sends one of each to the
circle index.

<!-- source: psionic/examples/hnsw-refuse.coax refuse -->
```jai
main :: () {
  s: [2] Store; ix: [1] HnIndex; iy: [1] HnIndex;
  st := hn_init(ix[..], 2, 2, 2, 5, s[0].v[..], s[0].level[0..<6], s[0].upper[0..<6], s[0].base[..], s[0].pool[..], s[0].seen[0..<6]);
  out: [4] BsHit; q := [2] f32.[0.8, -0.6];
  print("search an empty index: %\n", hn_status_name(hn_search(ix[..], q[..], 1, 1, out[..]).status));
  circle := HN_CIRCLE;
  for i: 0..<6 { r := hn_add(ix[..], circle[i * 2..<i * 2 + 2]); }
  print("six points on the circle, m 2, ef_build 2, seed 5\n");
  r := hn_search(ix[..], q[..], 2, 2, out[..]);
  print("query (0.8, -0.6), k 2, ef 2: %, points % and %, % similarities\n", hn_status_name(r.status), out[0].passage, out[1].passage, r.sims);
  three := [3] f32.[0.8, -0.6, 0];
  print("a query of 3 values: %\n", hn_status_name(hn_search(ix[..], three[..], 2, 2, out[..]).status));
  print("k 0: %\n", hn_status_name(hn_search(ix[..], q[..], 0, 2, out[..]).status));
  print("k 3, ef 2: %\n", hn_status_name(hn_search(ix[..], q[..], 3, 2, out[..]).status));
  print("k 2, ef 513: %\n", hn_status_name(hn_search(ix[..], q[..], 2, 513, out[..]).status));
  far := [2] f32.[10000000, 0];
  print("a query value of 10^7: %\n", hn_status_name(hn_search(ix[..], far[..], 2, 2, out[..]).status));
  seventh := [2] f32.[0, -1];
  print("a seventh point into storage for six: %\n", hn_status_name(hn_add(ix[..], seventh[..]).status));
  file: [256] u8;
  n := hn_save(ix[..], file[..]);
  st = hn_init(iy[..], 2, 2, 2, 5, s[1].v[..], s[1].level[..], s[1].upper[..], s[1].base[..], s[1].pool[..], s[1].seen[..]);
  print("saved % bytes; reload: %\n", n, hn_status_name(hn_load(iy[..], file[0..<n])));
  print("reload without the last byte: %\n", hn_status_name(hn_load(iy[..], file[0..<n - 1])));
  file[100] ^= 1;
  print("reload with one bit of a vector flipped: %\n", hn_status_name(hn_load(iy[..], file[0..<n])));
  file[100] ^= 1;
  st = hn_init(iy[..], 2, 2, 2, 5, s[1].v[..], s[1].level[0..<5], s[1].upper[0..<5], s[1].base[..], s[1].pool[..], s[1].seen[0..<5]);
  print("reload into storage for five points: %\n", hn_status_name(hn_load(iy[..], file[0..<n])));
}
```

```text
search an empty index: empty: the index holds no points
six points on the circle, m 2, ef_build 2, seed 5
query (0.8, -0.6), k 2, ef 2: ok, points 5 and 0, 5 similarities
a query of 3 values: dimension: vector length differs from the index
k 0: budget: need 1 <= k <= ef <= 512
k 3, ef 2: budget: need 1 <= k <= ef <= 512
k 2, ef 513: budget: need 1 <= k <= ef <= 512
a query value of 10^7: number: a value is not finite or exceeds 10^6
a seventh point into storage for six: capacity: storage is full or too small
saved 254 bytes; reload: ok
reload without the last byte: format: not a valid saved index
reload with one bit of a vector flipped: format: not a valid saved index
reload into storage for five points: capacity: storage is full or too small
```

The checks also flip every one of the 254 bytes of the circle's saved file in
turn and expect each load to be refused. They edit header fields, levels,
values and links and recompute the checksum, so the structure checks, not the
checksum, must catch each edit.

## Checks

```shell
psionic/check.sh --only book-search
```

[check_book_search.sh](../tests/check_book_search.sh) runs the checks of
`hnsw.coax` with chapter 54's files in debug and release builds, runs both
examples in both builds against their recorded output, mutates `rank.coax`
and `hnsw.coax`, and checks the book. The checks cover:

- the level rule against its bit patterns, and the level frequencies over
  4,096 ids;
- every parameter and storage bound at its edge;
- the working list's order, eviction and read marks;
- sorted links and the dropped worst link;
- the hand trace above, link by link, with its similarity counts;
- recall and similarity counts on 60 random 8-value vectors at `ef` 5, 10
  and 60, compared with an independent model of the same rules (83, 98 and
  100 of 100 shared ids; 617, 838 and 1,361 similarities);
- a save and reload that returns the same graph, the same answers and the
  same bytes when saved again;
- every refusal shown above, and the damaged files.

`coax mutate` on `hnsw.coax` kills 750 mutants; none survive.

## What this does not do yet

- This chapter's figures come from random unit vectors.
  [Chapter 56](ch056-rerank-the-candidates.md) builds the index over chapter
  54's 5,899 passage embeddings inside the book-search tool's `--rerank-eval`:
  at `ef` 200 its top 100 shares 98.6% of the exhaustive top 100. The tool's
  everyday search still scans exhaustively.
- The saved form is bytes in memory. Writing them to a file and reading them
  back needs the tool's host, which this chapter does not call.
- Cost is counted in similarities. Wall-clock latency, and the corpus size at
  which the graph search becomes faster than exhaustive search in seconds, are
  not measured.
- Points can be added but not removed or replaced. A changed chapter means
  rebuilding the index.
- Neighbor selection is the simple rule, not the paper's diversity heuristic,
  and a full list drops its worst link even when that leaves a point
  unreachable from some other.
- [Chapter 56](ch056-rerank-the-candidates.md) merges and reranks candidates;
  it can use this index's `hn_search` as its dense candidate source.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- Yu. A. Malkov and D. A. Yashunin, "Efficient and robust approximate nearest
  neighbor search using Hierarchical Navigable Small World graphs", IEEE
  TPAMI, 2018 (arXiv:1603.09320).
