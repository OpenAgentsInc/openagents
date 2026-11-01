# 56. Rerank the candidates

> **Status: partly implemented.** Candidates from BM25 and the HNSW index,
> a learned reranker, and separate index-recall and relevance measurements
> run on the CPU over chapter 54's books. Answer quality, the third
> measurement, belongs to chapter 57 and is not measured here, and the
> search tool's everyday queries do not use the reranker yet.

Lexical search and dense search each miss passages the other finds.
[Chapter 54](ch054-search-the-coax-book.md) fused their two top-100 lists by
reciprocal rank, a rule with no learned parameters.
This chapter merges the same two lists into one candidate pool, scores every
candidate with a small learned reranker, and keeps two measurements apart:

- **Index recall:** did a relevant passage reach the candidate pool at all?
- **Relevance:** among the candidates, is a relevant passage ranked near the
  top?

A third measurement, **answer quality**, asks whether an answer built from
the top passages uses them correctly. Nothing in this chapter generates
answers, so it is not measured here;
[chapter 57](ch057-answer-from-current-sources.md) owns it.

The dense list now comes from [chapter 55](ch055-search-without-scanning-everything.md)'s
HNSW index, built over chapter 54's real passage embeddings for the first
time. Chapter 55 measured that index only on random vectors.

The library is [rerank.coax](../lib/book-search/rerank.coax). It reuses
chapter 54's hit type, tie rule, fusion term and reciprocal rank from
[rank.coax](../lib/book-search/rank.coax). The measurement runs in the
book-search tool, [book-search.coax](../tools/book-search.coax), which also
loads [hnsw.coax](../lib/book-search/hnsw.coax) and chapter 241's
[verdict.coax](../lib/verdict.coax).

## Run it

Run commands from the repository root.

```shell
coax/bin/coax run psionic/examples/rerank.coax
coax/bin/coax run psionic/examples/rerank-refuse.coax
coax/web/tool.sh psionic/tools/book-search.coax --rerank-eval psionic/fixtures/book-search/heldout.tsv
psionic/check.sh --only book-search
```

## Merge the two lists

A candidate keeps its rank in each list, 0 when that list does not hold it.
The merge walks the lexical list, then adds the dense passages that the
lexical list lacks. A list that names a passage twice is refused, so a
passage can't vote twice.

<!-- source: psionic/lib/book-search/rerank.coax merge -->
```jai
// The union of the two lists, lexical order first, then the dense passages
// the lexical list lacks. both counts passages that are in both lists.
rr_merge :: (lexical: [] BsHit, dense: [] BsHit, out: [] RrCand) -> RrMerged {
  r: RrMerged; n := 0;
  for h, i: lexical {
    if rr_find(out[0..<n], h.passage) >= 0 || n == out.count { return RrMerged.{status = RrStatus.shape}; }
    out[n] = RrCand.{passage = h.passage, lexical = i + 1}; n += 1;
  }
  for h, i: dense {
    j := rr_find(out[0..<n], h.passage);
    if j >= 0 {
      if out[j].dense != 0 { return RrMerged.{status = RrStatus.shape}; }
      out[j].dense = i + 1; r.both += 1; continue;
    }
    if n == out.count { return RrMerged.{status = RrStatus.shape}; }
    out[n] = RrCand.{passage = h.passage, dense = i + 1}; n += 1;
  }
  r.count = n;
  return r;
}
rr_find :: (c: [] RrCand, passage: int) -> int {
  for x, i: c { if x.passage == passage { return i; } }
  return -1;
}
```

## Seven features

Each candidate gets seven numbers. The reranker sees nothing else.

| Feature | Value |
| --- | --- |
| lexical rank | 60 / (60 + rank) in the BM25 list, 0 when absent |
| dense rank | the same for the dense list |
| BM25 share | the candidate's BM25 score over the best BM25 score among the candidates |
| cosine | the encoder's query-passage cosine |
| heading coverage | the IDF-weighted share of query words in the passage's heading path |
| text coverage | the IDF-weighted share of query words anywhere in the passage |
| word match | for each query word, its best cosine with any passage word under the encoder's word rows; the mean |

The rank features are chapter 54's fusion terms scaled so rank 1 is 60/61.
The word match is a late-interaction score: it compares words one by one
instead of comparing one pooled vector per side, so it can see that a
passage holds a near-synonym of one query word even when the pooled
embeddings disagree. It uses the encoder's own learned rows; no new
weights are trained for it.

<!-- source: psionic/lib/book-search/rerank.coax features -->
```jai
// query: sorted distinct term ids with a weight each (the tool uses BM25's
// IDF); words: a passage's sorted distinct term ids. The weighted share of
// the query that the passage holds, 0 when the query weighs nothing.
rr_coverage :: (query: [] int, weight: [] f64, words: [] int) -> f64 {
  total := 0.0; held := 0.0; i := 0; j := 0;
  for x: weight { total += x; }
  while i < query.count && j < words.count {
    if query[i] < words[j] { i += 1; }
    else if query[i] > words[j] { j += 1; }
    else { held += weight[i]; i += 1; }
  }
  if total <= 0 { return 0; }
  return held / total;
}
// Late interaction over the encoder's word rows: unit holds every row scaled
// to length one (rr_unit). For each query word, its best cosine with any
// passage word; their mean. A zero row scores 0 against every word.
rr_late :: (unit: [] f64, width: int, query: [] int, words: [] int) -> f64 {
  if query.count == 0 || words.count == 0 { return 0; }
  sum := 0.0;
  for a: query {
    best := -1.0;
    for b: words {
      c := 0.0;
      for d: 0..<width { c += unit[a * width + d] * unit[b * width + d]; }
      best = (best + c + abs(best - c)) / 2;
    }
    sum += best;
  }
  return sum / f64(query.count);
}
// Scales each row of table (width numbers) to length one into unit; a zero
// row stays zero.
rr_unit :: (table: [] f64, width: int, unit: [] f64) {
  for r: 0..<table.count / width {
    n := 0.0;
    for d: 0..<width { n += table[r * width + d] * table[r * width + d]; }
    n = sqrt(n);
    for d: 0..<width { unit[r * width + d] = ifx n > 0 then table[r * width + d] / n else 0.0; }
  }
}
// One candidate's seven features, in RR_FEATURE_NAMES order. Ranks become
// 60 / (60 + rank), chapter 54's fusion term scaled to 1 at rank 0; bm25 is
// divided by the best BM25 score among the candidates (0 when none scored).
rr_row :: (c: RrCand, bm25: f64, best: f64, cosine: f64, heading: f64, text: f64, late: f64, out: [] f64) {
  out[0] = rr_rank_term(c.lexical);
  out[1] = rr_rank_term(c.dense);
  out[2] = ifx best > 0 then bm25 / best else 0.0;
  out[3] = cosine; out[4] = heading; out[5] = text; out[6] = late;
}
rr_rank_term :: (rank: int) -> f64 { if rank < 1 { return 0; } return f64(BS_FUSE) / f64(BS_FUSE + rank); }
```

## The pairwise loss

The reranker's score is a weighted sum of the seven features. Training asks
that every relevant candidate of a query outscore every other candidate of
that query. For a relevant `i` and another `j` with scores `s_i` and `s_j`,
the pair loss is the logistic loss of RankNet:

```jai
loss(i, j) = log(1 + exp(s_j - s_i))
d loss / d w = sigmoid(s_j - s_i) * (f_j - f_i)
```

A query's loss is the mean over its pairs, and the training loss is the
mean over queries, so a query with many candidates does not outweigh one
with few. An L2 term `l2 / 2 * |w|^2` keeps the weights bounded.

### A trace by hand

Take one query with two candidates and two features. The relevant one has
features (1, 0), the other (0, 1). Start from `w = (0, 0)`:

- Both scores are 0, so the loss is `log(1 + e^0) = log 2 = 0.6931`.
- `sigmoid(0) = 0.5`, so the gradient is `0.5 * ((0, 1) - (1, 0)) = (-0.5, 0.5)`.
- One step with learning rate 1 gives `w = (0, 0) - (-0.5, 0.5) = (0.5, -0.5)`.
- Now `s_i = 0.5` and `s_j = -0.5`, so the loss is `log(1 + e^-1) = 0.3133`.

The example below prints these four numbers from the library, and a check
asserts them.

<!-- source: psionic/lib/book-search/rerank.coax loss -->
```jai
// log(1 + e^x) and 1 / (1 + e^-x), written so neither overflows.
rr_softplus :: (x: f64) -> f64 { return (x + abs(x)) / 2 + log(1 + exp(-abs(x))); }
rr_sigmoid :: (x: f64) -> f64 { return exp(x - rr_softplus(x)); }
rr_score :: (w: [] f64, f: [] f64) -> f64 {
  s := 0.0;
  for k: 0..<f.count { s += w[k] * f[k]; }
  return s;
}
// Candidates of query q are rows groups[q]..<groups[q + 1] of f (w.count
// features each). For every relevant i and other j of one query the pair loss
// is softplus(s_j - s_i); a query's loss is the mean over its pairs, and the
// total the mean over queries that have a pair, plus l2/2 |w|^2. grad gets
// the gradient.
rr_loss :: (w: [] f64, f: [] f64, label: [] bool, groups: [] int, l2: f64, grad: [] f64) -> RrLoss {
  F := w.count; r: RrLoss; total := 0.0;
  for k: 0..<F { grad[k] = 0; }
  for q: 0..<groups.count - 1 {
    a := groups[q]; b := groups[q + 1];
    pairs := 0; sum := 0.0; dq: [RR_FEATURES] f64;
    for i: a..<b {
      if !label[i] { continue; }
      si := rr_score(w, f[i * F..<(i + 1) * F]);
      for j: a..<b {
        if label[j] { continue; }
        d := rr_score(w, f[j * F..<(j + 1) * F]) - si;
        sum += rr_softplus(d); pairs += 1;
        p := rr_sigmoid(d);
        for k: 0..<F { dq[k] += p * (f[j * F + k] - f[i * F + k]); }
      }
    }
    if pairs == 0 { continue; }
    r.queries += 1; r.pairs += pairs;
    total += sum / f64(pairs);
    for k: 0..<F { grad[k] += dq[k] / f64(pairs); }
  }
  if r.queries == 0 { return r; }
  norm := 0.0;
  for k: 0..<F { grad[k] = grad[k] / f64(r.queries) + l2 * w[k]; norm += w[k] * w[k]; }
  r.loss = total / f64(r.queries) + l2 / 2 * norm;
  return r;
}
```

Training is full-batch gradient descent from zero weights. There is no
sampling inside it, so the same rows give the same weights on every run.

<!-- source: psionic/lib/book-search/rerank.coax train -->
```jai
rr_finite :: (v: [] f64) -> bool {
  for x: v { if !(x >= -RR_VALUE_MAX && x <= RR_VALUE_MAX) { return false; } }
  return true;
}
rr_budget_ok :: (c: RrConfig) -> bool {
  return c.steps >= 1 && c.steps <= RR_STEPS_MAX && c.lr > 0 && c.lr <= RR_VALUE_MAX && c.l2 >= 0 && c.l2 <= RR_VALUE_MAX;
}
// Full-batch gradient descent on rr_loss from zero weights. w.count is the
// number of features (1 to 7); groups starts at 0, never decreases and ends
// at the row count.
rr_train :: (c: RrConfig, f: [] f64, label: [] bool, groups: [] int, w: [] f64) -> RrTrained {
  F := w.count; n := label.count;
  if F < 1 || F > RR_FEATURES || f.count != n * F || groups.count < 2 { return RrTrained.{status = RrStatus.shape}; }
  if groups[0] != 0 || groups[groups.count - 1] != n { return RrTrained.{status = RrStatus.shape}; }
  for q: 0..<groups.count - 1 { if groups[q + 1] < groups[q] { return RrTrained.{status = RrStatus.shape}; } }
  if !rr_finite(f) { return RrTrained.{status = RrStatus.number}; }
  if !rr_budget_ok(c) { return RrTrained.{status = RrStatus.budget}; }
  for k: 0..<F { w[k] = 0; }
  g: [RR_FEATURES] f64; r: RrTrained;
  for step: 0..<c.steps {
    l := rr_loss(w, f, label, groups, c.l2, g[0..<F]);
    if l.queries == 0 { return RrTrained.{status = RrStatus.empty}; }
    if step == 0 { r = RrTrained.{queries = l.queries, pairs = l.pairs, first = l.loss}; }
    for k: 0..<F { w[k] -= c.lr * g[k]; }
  }
  r.last = rr_loss(w, f, label, groups, c.l2, g[0..<F]).loss;
  return r;
}
```

## Rerank and measure

Reranking scores every candidate and sorts them with chapter 54's rule:
higher score first, then the lower passage id.

<!-- source: psionic/lib/book-search/rerank.coax rerank -->
```jai
// Scores every candidate (row i of f belongs to cands[i]) and writes them to
// out best first; -1 when out or f does not fit.
rr_rerank :: (w: [] f64, f: [] f64, cands: [] RrCand, out: [] BsHit) -> int {
  F := w.count;
  if out.count < cands.count || f.count != cands.count * F { return -1; }
  for c, i: cands {
    hit := BsHit.{passage = c.passage, score = rr_score(w, f[i * F..<(i + 1) * F])};
    j := i;
    while j > 0 && bs_before(hit, out[j - 1]) { out[j] = out[j - 1]; j -= 1; }
    out[j] = hit;
  }
  return cands.count;
}
```

Relevance uses chapter 54's reciprocal rank at 10 and nDCG at 10 with gain 1
for a relevant passage. Index recall per query is whether the list holds a
relevant passage at all.

<!-- source: psionic/lib/book-search/rerank.coax metrics -->
```jai
// nDCG at k with gain 1 for a relevant passage: DCG of the list over the DCG
// of an ideal list that puts all `total` relevant passages first.
rr_ndcg :: (hits: [] BsHit, relevant: [] bool, total: int, k: int) -> f64 {
  dcg := 0.0; ideal := 0.0;
  for i: 0..<k {
    if i < hits.count && relevant[hits[i].passage] { dcg += log(2.0) / log(f64(i + 2)); }
    if i < total { ideal += log(2.0) / log(f64(i + 2)); }
  }
  if ideal == 0 { return 0; }
  return dcg / ideal;
}
// Index recall for one query: does the list, or the candidate pool, hold a
// relevant passage at all?
rr_holds :: (hits: [] BsHit, relevant: [] bool) -> bool { for h: hits { if relevant[h.passage] { return true; } } return false; }
rr_pool_holds :: (cands: [] RrCand, relevant: [] bool) -> bool { for c: cands { if relevant[c.passage] { return true; } } return false; }
```

## A small example

[rerank.coax](../examples/rerank.coax) runs the hand trace, trains a
two-feature reranker (the two rank features) on three training queries, and
reranks a fourth query that training never saw. In the training queries the
relevant passage is always first in the dense list and never first in the
lexical list, so the reranker learns a large dense weight and a negative
lexical one.

<!-- source: psionic/examples/rerank.coax example -->
```jai
// Prints x to four decimals, rounded half away from zero.
put4 :: (x: f64) {
  h := 0.5; if x < 0 { h = -0.5; }
  t := cast(int) (x * 10000 + h);
  if t < 0 { print("-"); t = -t; }
  print("%.", t / 10000); print("%", t / 1000 % 10); print("%", t / 100 % 10); print("%", t / 10 % 10); print("%", t % 10);
}
// Two features per candidate: chapter 54's fusion terms, 60 / (60 + rank).
row :: (c: RrCand, out: [] f64) { out[0] = rr_rank_term(c.lexical); out[1] = rr_rank_term(c.dense); }

main :: () {
  // The hand trace: one relevant candidate (1, 0), one other (0, 1).
  f := [4] f64.[1, 0, 0, 1]; label := [2] bool.[true, false]; groups := [2] int.[0, 2];
  w: [2] f64; g: [2] f64;
  l := rr_loss(w[..], f[..], label[..], groups[..], 0, g[..]);
  print("hand trace: loss at w = 0 is "); put4(l.loss); print(", gradient ("); put4(g[0]); print(", "); put4(g[1]); print(")\n");
  r := rr_train(RrConfig.{steps = 1, lr = 1, l2 = 0}, f[..], label[..], groups[..], w[..]);
  print("one step with lr 1: w = ("); put4(w[0]); print(", "); put4(w[1]); print("), loss "); put4(r.last); print("\n\n");

  // Three training queries.
  train := [10] RrCand.[
    RrCand.{passage = 0, lexical = 1}, RrCand.{passage = 1, lexical = 2, dense = 1}, RrCand.{passage = 2, dense = 3},
    RrCand.{passage = 3, lexical = 1, dense = 4}, RrCand.{passage = 4, dense = 1}, RrCand.{passage = 5, lexical = 3, dense = 2},
    RrCand.{passage = 6, lexical = 1}, RrCand.{passage = 7, lexical = 2}, RrCand.{passage = 8, dense = 1}, RrCand.{passage = 9, lexical = 4, dense = 2}];
  tl := [10] bool.[false, true, false, false, true, false, false, false, true, false];
  tg := [4] int.[0, 3, 6, 10];
  tf: [20] f64;
  for c, i: train { row(c, tf[i * 2..<i * 2 + 2]); }
  r = rr_train(RrConfig.{steps = 300, lr = 0.5, l2 = 0.001}, tf[..], tl[..], tg[..], w[..]);
  print("trained on % queries, % pairs: loss ", r.queries, r.pairs); put4(r.first); print(" -> "); put4(r.last);
  print("; weights lexical "); put4(w[0]); print(", dense "); put4(w[1]); print("\n\n");

  // A new query: BM25 and dense search each return three passages.
  lex := [3] BsHit.[BsHit.{passage = 7, score = 9}, BsHit.{passage = 2, score = 8}, BsHit.{passage = 5, score = 1}];
  den := [3] BsHit.[BsHit.{passage = 2, score = 0.9}, BsHit.{passage = 4, score = 0.8}, BsHit.{passage = 7, score = 0.1}];
  relevant := [8] bool.[false, false, false, false, true, false, false, false];
  cands: [6] RrCand;
  m := rr_merge(lex[..], den[..], cands[..]);
  print("merged: % candidates, % in both lists\n", m.count, m.both);
  for c: cands[0..<m.count] { print("  passage %: lexical rank %, dense rank %\n", c.passage, c.lexical, c.dense); }
  print("index recall: BM25 alone holds a relevant passage: %; the merged pool: %\n", rr_holds(lex[..], relevant[..]), rr_pool_holds(cands[0..<m.count], relevant[..]));
  scores: [8] f64; fusedhits: [6] BsHit;
  bs_fuse(lex[..], den[..], scores[..]);
  n := bs_top(scores[..], 0, fusedhits[..]);
  print("fused:   ");
  for h: fusedhits[0..<n] { print(" %", h.passage); }
  print("; reciprocal rank "); put4(bs_reciprocal(bs_first_relevant(fusedhits[0..<n], relevant[..]), 10));
  print(", nDCG@10 "); put4(rr_ndcg(fusedhits[0..<n], relevant[..], 1, 10)); print("\n");
  cf: [12] f64; ranked: [6] BsHit;
  for c, i: cands[0..<m.count] { row(c, cf[i * 2..<i * 2 + 2]); }
  k := rr_rerank(w[..], cf[0..<m.count * 2], cands[0..<m.count], ranked[..]);
  print("reranked:");
  for h: ranked[0..<k] { print(" %", h.passage); }
  print("; reciprocal rank "); put4(bs_reciprocal(bs_first_relevant(ranked[0..<k], relevant[..]), 10));
  print(", nDCG@10 "); put4(rr_ndcg(ranked[0..<k], relevant[..], 1, 10)); print("\n");
  for h: ranked[0..<k] { print("  passage % scores ", h.passage); put4(h.score); print("\n"); }
}
```

```text
hand trace: loss at w = 0 is 0.6931, gradient (-0.5000, 0.5000)
one step with lr 1: w = (0.5000, -0.5000), loss 0.3133

trained on 3 queries, 7 pairs: loss 0.6931 -> 0.3457; weights lexical -0.9784, dense 3.8812

merged: 4 candidates, 2 in both lists
  passage 7: lexical rank 1, dense rank 3
  passage 2: lexical rank 2, dense rank 1
  passage 5: lexical rank 3, dense rank 0
  passage 4: lexical rank 0, dense rank 2
index recall: BM25 alone holds a relevant passage: false; the merged pool: true
fused:    2 7 4 5; reciprocal rank 0.3333, nDCG@10 0.5000
reranked: 4 2 7 5; reciprocal rank 1.0000, nDCG@10 1.0000
  passage 4 scores 3.7560
  passage 2 scores 2.8708
  passage 7 scores 2.7341
  passage 5 scores -0.9318
```

BM25 alone never retrieved passage 4, so it fails index recall for this
query; the merged pool holds it. Fusion ranks it third. The reranker puts it
first, which raises the reciprocal rank from 1/3 to 1 and nDCG@10 from 0.5
to 1. The pool did not change, so the whole gain is relevance, and index
recall gets none of the credit.

## Refusals

[rerank-refuse.coax](../examples/rerank-refuse.coax) sends one malformed
request of each kind. Each is refused with a typed status; a reranking that
doesn't fit its output returns -1.

<!-- source: psionic/examples/rerank-refuse.coax refuse -->
```jai
main :: () {
  lex := [3] BsHit.[BsHit.{passage = 7}, BsHit.{passage = 2}, BsHit.{passage = 7}];
  den := [2] BsHit.[BsHit.{passage = 2}, BsHit.{passage = 4}];
  out: [4] RrCand;
  print("a lexical list that names passage 7 twice: %\n", rr_status_name(rr_merge(lex[..], den[..], out[..]).status));
  print("two distinct lists into room for 2 of 3: %\n", rr_status_name(rr_merge(lex[0..<2], den[..], out[0..<2]).status));
  f := [4] f64.[1, 0, 0, 1]; label := [2] bool.[true, false]; groups := [2] int.[0, 2]; w: [2] f64;
  c := RrConfig.{steps = 300, lr = 0.5, l2 = 0.001};
  print("a good training set: %\n", rr_status_name(rr_train(c, f[..], label[..], groups[..], w[..]).status));
  print("3 feature values for 2 candidates of 2 features: %\n", rr_status_name(rr_train(c, f[0..<3], label[..], groups[..], w[..]).status));
  back := [3] int.[0, 3, 2];
  print("query groups that run backwards: %\n", rr_status_name(rr_train(c, f[..], label[..], back[..], w[..]).status));
  bad := f; bad[2] = 0.0 / 0.0;
  print("a feature that is not a number: %\n", rr_status_name(rr_train(c, bad[..], label[..], groups[..], w[..]).status));
  bad = f; bad[2] = 2000000;
  print("a feature of 2 * 10^6: %\n", rr_status_name(rr_train(c, bad[..], label[..], groups[..], w[..]).status));
  x := c; x.lr = 0;
  print("learning rate 0: %\n", rr_status_name(rr_train(x, f[..], label[..], groups[..], w[..]).status));
  x = c; x.steps = 0;
  print("0 steps: %\n", rr_status_name(rr_train(x, f[..], label[..], groups[..], w[..]).status));
  none := [2] bool.[false, false];
  print("a query with no relevant candidate: %\n", rr_status_name(rr_train(c, f[..], none[..], groups[..], w[..]).status));
  cands := [2] RrCand.[RrCand.{passage = 3}, RrCand.{passage = 9}]; hits: [1] BsHit;
  print("rerank 2 candidates into room for 1: %\n", rr_rerank(w[..], f[..], cands[..], hits[..]));
}
```

```text
a lexical list that names passage 7 twice: shape: a list, a row or a group does not fit
two distinct lists into room for 2 of 3: shape: a list, a row or a group does not fit
a good training set: ok
3 feature values for 2 candidates of 2 features: shape: a list, a row or a group does not fit
query groups that run backwards: shape: a list, a row or a group does not fit
a feature that is not a number: number: a value is not finite or exceeds 10^6
a feature of 2 * 10^6: number: a value is not finite or exceeds 10^6
learning rate 0: budget: need 1 <= steps <= 100000, 0 < lr <= 10^6, 0 <= l2 <= 10^6
0 steps: budget: need 1 <= steps <= 100000, 0 < lr <= 10^6, 0 <= l2 <= 10^6
a query with no relevant candidate: empty: no query has both a relevant and another candidate
rerank 2 candidates into room for 1: -1
```

The program exits with status 0: every refusal is an expected answer.

## Criteria fixed before the run

These were written down before the held-out queries were run, and nothing
was changed after:

- **Index recall.** The HNSW index uses `m` 16, `ef_build` 128 and seed 55
  over chapter 54's passage embeddings. At `ef` 200 its top 100 must share
  at least 95% of the exhaustive top 100, averaged over the 36 held-out
  queries. The merged pool with HNSW must hold a relevant passage for at
  most one query fewer than the merged pool with exhaustive search.
- **Relevance.** The reranker is credited only by comparing it with fusion
  on the same HNSW candidates, so a change in the index can't be credited
  to it. It is also compared with chapter 54's fused retriever, the
  baseline. Both comparisons use per-query reciprocal rank at 10 and
  [chapter 241](ch241-gates-that-can-say-unverifiable.md)'s decision gate:
  1,000 replicates that redraw whole queries, seed 56, ceiling 1, and
  [chapter 240](ch240-how-far-a-metric-moves-by-itself.md)'s noise floor. A
  comparison counts as a gain only when its verdict is *passed*.
- **The floor.** Eight more rerankers are trained, each on a different half
  of the training queries (seeds 1 to 8). The floor is how far their mean
  reciprocal rank moves between runs, with blocks of one run.
- **Not gated.** nDCG@10 and the reranker's time per query are reported.

The reranker's settings were fixed first as well: the seven features above,
300 steps, learning rate 0.5, L2 0.001. Its training queries come from the
books, never from a query file. The tool draws 1,200 passages with seed 56.
For even draws the query is the passage's deepest heading (at least two known
words), and every passage under that heading in the chapter is relevant. For
odd draws the query is a span of 5 to 10 words from the passage text, and
every passage of the same section is relevant. Each reranker trains on the
half of these queries its seed picks; the reported one uses seed 56. Of the
1,200 draws, 1,104 qualify; the rest are headings with fewer than two known
words or passages too short for a span.

## Measure it

```shell
coax/web/tool.sh psionic/tools/book-search.coax --rerank-eval psionic/fixtures/book-search/heldout.tsv
```

```text
36 held-out queries over 5899 passages, books at e0c7d09fc4dd, weights 20e46cbf39fdf985
HNSW over the passage embeddings: m 16, ef_build 128, seed 55; built with 7045472 similarities in 353 ms; 3418888 bytes in memory
training: 1104 synthetic queries (1090 with a relevant and another candidate), 186534 candidate rows, features in 5624 ms; 9 rerankers trained in 21092 ms
reranker (seed 56): 568 queries, 288664 pairs, loss 0.6931 -> 0.1618
  weight lexical rank: 0.6365
  weight dense rank: 1.8243
  weight BM25 share: 1.4263
  weight cosine: 1.1176
  weight heading coverage: 1.7629
  weight text coverage: 1.7735
  weight word match: 0.8117

Index recall
| Dense source | Recall@100 against exhaustive | Similarities per query |
| --- | ---: | ---: |
| HNSW, ef 100 | 0.942 | 945.8 |
| HNSW, ef 200 | 0.986 | 1500.8 |
| HNSW, ef 400 | 0.997 | 2321.9 |
| Exhaustive | 1.000 | 5899 |
queries whose candidates hold a relevant passage (of 36): BM25 top 100 35, exhaustive dense top 100 34, HNSW dense top 100 34, merged with exhaustive dense 35, merged with HNSW dense 35; mean merged pool 170.2 candidates

Relevance
| Ranking | Recall@10 | MRR@10 | nDCG@10 |
| --- | ---: | ---: | ---: |
| BM25 | 0.778 | 0.502 | 0.376 |
| Exhaustive vector search | 0.833 | 0.548 | 0.418 |
| Fused, exhaustive dense (chapter 54) | 0.861 | 0.555 | 0.429 |
| Fused, HNSW dense | 0.861 | 0.555 | 0.429 |
| Reranked, HNSW candidates | 0.861 | 0.598 | 0.494 |
reranking: 5.89 ms per query for the features and nine rerankings (one core)

floor over 8 reranker runs: MRR@10 run means 0.599 0.600 0.598 0.599 0.602 0.599 0.598 0.603; sd 0.0017, floor 0.0046
reranked against fused on the same HNSW candidates: MRR@10 gain 0.0434, 45 of 1000 replicates at or under the floor; verdict unverifiable, near the floor
reranked against chapter 54's fused retriever: MRR@10 gain 0.0434, 45 of 1000 replicates at or under the floor; verdict unverifiable, near the floor
```

### Index recall

At `ef` 200 the HNSW top 100 shares 98.6% of the exhaustive top 100 and
computes a quarter of the similarities, so the index criterion passes.
Missing 1.4% of the dense list cost nothing measurable: the HNSW dense list
holds a relevant passage for the same 34 queries as exhaustive search, the
merged pools for the same 35, and fusion over HNSW candidates scores exactly
what fusion over exhaustive candidates scores. BM25's own top 100 already
holds a relevant passage for 35 queries, so the dense list adds no index
recall on this query set; it can only help ranking.

### Relevance

The one query no pool holds is outside the reach of every ranking, so the
ceiling for recall@10 is 35 of 36. The reranker reorders the same pool that
fusion ranks. It leaves recall@10 at 0.861, raises MRR@10 from 0.555 to
0.598 and nDCG@10 from 0.429 to 0.494.

The gate does not call this a gain. The eight floor runs put the floor at
0.0046, and the gain of 0.043 is ten times larger. But the 36 queries are
few: when whole queries are drawn again, 45 of 1,000 replicates fall at or
below the floor, and *clear* allows 25. The verdict is *unverifiable, near
the floor*, for both comparisons. They are identical because fusion over HNSW
candidates and chapter 54's fused retriever give every held-out query the
same reciprocal rank. Under the criteria fixed above, this chapter does not claim that
the reranker improves relevance; it needs more held-out queries to decide.

The learned weights are all positive. The dense rank, both coverage features
and the BM25 share carry the most weight; the lexical rank the least, since
the BM25 share already holds the same information on a finer scale.

### Cost

Features and nine rerankings take 5.89 ms per query on one core, for 170
candidates on average. Computing the features dominates: one reranking is
seven multiply-adds per candidate and a sort. The HNSW index takes
0.35 s to build, and training all nine rerankers takes 21 s.

### What changed in the baseline

Chapter 54 recorded MRR@10 of 0.505, 0.477 and 0.518 at revision
db51247275c1. The books have grown since, to 5,899 passages, and the encoder
was retrained on them, so the same retrievers now score 0.502, 0.548 and
0.555. Two held-out labels named chapter titles that later gained their
chapter numbers (`Determinism` became `16. Determinism`, and `GPU programs`
became `17. GPU programs`); the labels were updated to the new titles. They
still name the same chapters, and no query changed.

## Checks

```shell
psionic/check.sh --only book-search
```

[check_book_search.sh](../tests/check_book_search.sh) runs the checks of
`rerank.coax` with chapters 54 and 55's files in debug and release builds,
runs both examples in both builds against their recorded output, mutates
`rerank.coax` and checks the book. The checks cover:

- the merge, including duplicates and exact output room;
- coverage at both ends of the lists and with zero weights;
- word match over unit rows, including zero rows and opposite words;
- the hand trace above, the L2 term, and loss and gradient at score gaps
  of 1,000 without overflow;
- the gradient against central finite differences on three queries, one
  with no pair, and the loss against a sum written out by hand;
- training that lowers the loss, repeats exactly and separates a separable
  set;
- every refusal shown above, with each bound at its edge;
- nDCG@10 against hand values, and the tie rule when reranking.

`coax mutate` on `rerank.coax` kills all 321 mutants; none survive.

## What this does not do yet

- Answer quality is not measured. Nothing generates an answer from the
  reranked passages; [chapter 57](ch057-answer-from-current-sources.md)
  does.
- The tool's everyday search (`"query"`, `--mode`) still ranks with fusion.
  The reranker is trained and measured only by `--rerank-eval`, which trains
  it again on every run.
- 36 held-out queries are too few to decide a gain of this size. More
  labelled queries, written without looking at the reranker's results, are
  the next step.
- The reranker is linear in seven hand-chosen features. It has no
  cross-attention between query and passage.
- Training queries come from the books' own structure. Their wording differs
  from the held-out queries, which were written to avoid heading words.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- C. Burges et al., "Learning to Rank using Gradient Descent", ICML 2005
  (RankNet's pairwise logistic loss).
- K. Järvelin and J. Kekäläinen, "Cumulated Gain-Based Evaluation of IR
  Techniques", ACM TOIS, 2002 (nDCG).
- O. Khattab and M. Zaharia, "ColBERT: Efficient and Effective Passage
  Search via Contextualized Late Interaction over BERT", SIGIR 2020 (the
  late-interaction maximum over words).
