# 54. Search the Coax book

This chapter builds search over the three books in this repository: Coax,
Psionic and Aethyr. A query returns ranked passages. Each result shows the
passage text, its book, chapter file and heading path, the commit that last
changed the chapter, and the chapter's status. A result from a design draft or
a planned chapter is marked as not shipped.

Three retrievers rank the same passages:

- **BM25**, the lexical baseline.
- **Exhaustive vector search** over embeddings from Psionic's own encoder,
  profile `bs-attnpool-d64-v1`. It scores every passage; it is the reference
  that a faster index (chapter 55) must match.
- **The Psionic retriever**, which fuses the two rankings. The tool uses it by
  default.

Everything runs on one CPU core. The library is
[rank.coax](../lib/book-search/rank.coax) (`#authority`: ranking, status
labels, freshness and metrics), [corpus.coax](../lib/book-search/corpus.coax)
(passages, identities and words) and
[encoder.coax](../lib/book-search/encoder.coax) (the encoder and its
training). The command-line tool is
[book-search.coax](../tools/book-search.coax).

## Run it

Run commands from the repository root. The first run trains the weights and
builds the index; later runs reuse both and rebuild only a stale index.

```shell
coax/web/tool.sh psionic/tools/book-search.coax "memory that outlives the procedure that made it" --book coax --k 3
```

```text
3 results for "memory that outlives the procedure that made it" (fused; 8 of 8 words known; books at db51247275c1)

1. Coax · coax/book/ch06-memory.md · 6. Arrays, strings, memory and the context > Memory that outlives a procedure (planned)
   [planned, not built — this text describes work that is not built; do not treat it as shipped]
   revision 1f9d64a193fc, passage 93d27403c2cd9f39 part 0, score 0.0325
   Everything so far has been memory with an obvious owner: a stack frame, a constant, or the host. Real engines also need memory that one procedure creates and another frees later: a list that grows as enemies spawn, a level's worth of geometry, a cache. None of this is in `coax0`. This section describes what the spec plans, and the errors you'll see if you try it today.

2. Coax · coax/book/ch06-memory.md · 6. Arrays, strings, memory and the context > Memory that outlives a procedure (planned) > Pointers
   [planned, not built — this text describes work that is not built; do not treat it as shipped]
   revision 1f9d64a193fc, passage f528d693c3e38446 part 1, score 0.0318
   A pointer can outlive what it points at, exactly like a view. Coax doesn't plan a borrow checker to stop that. It plans to make long-lived raw pointers unnecessary most of the time, with handles.

3. Coax · coax/book/ch06-memory.md · 6. Arrays, strings, memory and the context > Memory that outlives a procedure (planned) > Handles instead of long-lived pointers
   [planned, not built — this text describes work that is not built; do not treat it as shipped]
   revision 1f9d64a193fc, passage e7c6d3a7eb190c4d part 0, score 0.0318
   A `Handle(T)` is a 32-bit index into a pool plus a generation number. When a slot is recycled, its generation changes, and a stale handle fails its check instead of reading whatever moved in. ```jai door: Handle(Entity); if e := world.get(door) { e.open = true; } ``` The engine's entity system already works this way (`ecs::Entity`). Anything that outlives a frame, is saved, or has to survive a hot reload (Chapter 19) should hold a handle rather than an address. Today, the closest thing is an `int` index into an array plus a check of `count`, which catches an index past the end but not a slot t...
```

The options:

| Option | Effect |
| --- | --- |
| `--k N` | Show N results (1 to 100; default 5). |
| `--book coax\|psionic\|aethyr` | Search one book. |
| `--mode fused\|bm25\|dense` | Choose the retriever (default `fused`). |
| `--json` | Print one JSON object for agents: every result's book, chapter, heading, part, passage hash, revision, status, `shipped` flag, score and text. |
| `--no-build` | Refuse to answer from a stale index instead of rebuilding it. |
| `--rebuild`, `--train` | Rebuild the index; also retrain the weights. |
| `--eval FILE [--verbose]` | Measure the three retrievers on a labelled query file. |

The weights and the index live in `coax/web/build/book-search/`, which Git
ignores.

## What changed from the plan

The plan named Nemotron-3-Embed-1B-BF16 as this chapter's encoder. This
chapter uses Psionic's own weights and tokenizer instead, so that the
embedding path exercises Psionic's own tokenization, attention, training and
evaluation. Three dependencies were too small for the books, so this chapter
builds the smallest version of each that works and says what it is:

- **Tokenizer.** [Chapter 123](ch123-learn-a-tokenizer.md)'s BPE trainer
  accepts at most 4,096 bytes and 64 merges. The three books hold about
  3.6 MB. This chapter keeps chapter 123's pinned splitting rule and learns a
  word vocabulary by counting: every word seen at least twice, most frequent
  first. There are no subword pieces.
- **Attention.** [Chapter 10](ch010-attention-without-training.md)'s block is
  causal and admits at most 16 positions. The encoder reads up to 256 words
  of a passage, and every word must count toward the result. This chapter
  writes one non-causal attention: a single learned query attends over all
  positions of a passage.
- **Weights.** No existing chapter trains an encoder. This chapter trains one
  on the books, with hand-written gradients and
  [chapter 124](ch124-adamw-and-explicit-precision.md)'s AdamW update. On this
  Mac it trains in under 10 seconds.

Nemotron-3-Embed-1B-BF16 stays a later target. It needs its own tokenizer
profile from chapter 52, many bidirectional layers and a device path.

## A passage has an identity

A chapter splits at its headings. Lines inside a code fence are never
headings. A section longer than 600 bytes splits again at its next blank line
outside a fence, and one longer than 2,000 bytes splits at its next line. Each
passage records:

- its book and chapter file;
- its heading path, such as `54. Search the Coax book > Measure it`;
- its part number inside the section;
- the SHA-256 of its heading path, a newline and its text.

The index also records the commit that last changed each chapter, from
`git log`, and marks chapters with uncommitted edits.

<!-- source: psionic/lib/book-search/corpus.coax chunk -->
```jai
// Splits the chapter at pool[from..<to] into passages after out[0..<n], and
// writes each heading path ("Title > Section > Subsection") into heads after
// used. A section longer than BS_PART_MIN bytes splits at its next blank line
// outside a fence; one longer than BS_PART_MAX splits at its next line.
bs_chunk :: (pool: [] u8, from: int, to: int, file: int, out: [] BsPassage, n: int, heads: [] u8, used: int) -> BsChunks {
  r := BsChunks.{passages = n, heads = used};
  stack_from: [BS_HEADS_MAX] int; stack_to: [BS_HEADS_MAX] int; depth := 0;
  fence := false; part := 0; start := from; at := from;
  while at < to {
    end := at;
    while end < to && pool[end] != '\n' { end += 1; }
    line := cast(string) pool[at..<end];
    level := ifx fence then 0 else bs_heading(line);
    split := false;
    if level > 0 { split = true; }
    else if !fence && bs_blank(line) && at - start >= BS_PART_MIN { split = true; }
    else if at - start >= BS_PART_MAX { split = true; }
    if split {
      r = bs_emit(pool, start, at, file, part, stack_from[0..<depth], stack_to[0..<depth], out, r, heads);
      if r.full { return r; }
      part += 1; start = at;
    }
    if level > 0 {
      // The heading becomes the path for what follows; its line is not text.
      d := ifx level > depth + 1 then depth + 1 else level;
      stack_from[d - 1] = at + level + 1; stack_to[d - 1] = end; depth = d;
      part = 0; start = ifx end < to then end + 1 else to;
    }
    if !fence && bs_fence(line) { fence = true; } else if fence && bs_fence(line) { fence = false; }
    at = end + 1;
  }
  return bs_emit(pool, start, to, file, part, stack_from[0..<depth], stack_to[0..<depth], out, r, heads);
}
```

The book comes from the path:

<!-- source: psionic/lib/book-search/corpus.coax identity -->
```jai
// The three books, in index order; a path outside them belongs to none.
bs_book :: (path: string) -> int {
  if bs_prefix(path, "coax/book/") { return 0; }
  if bs_prefix(path, "psionic/book/") { return 1; }
  if bs_prefix(path, "aethyr/book/") { return 2; }
  return -1;
}
bs_book_name :: (book: int) -> string {
  if book == 0 { return "Coax"; }
  if book == 1 { return "Psionic"; }
  if book == 2 { return "Aethyr"; }
  return "";
}
```

## Every passage carries its status

The status comes from the chapter file. The rule is in `rank.coax`, because a
wrong label presents unbuilt work as shipped:

- `PLAN.md` is planned work.
- A chapter whose third line is the Psionic and Aethyr draft marker,
  `> **Status: unimplemented — design draft.**`, is a design draft.
- A chapter whose third line is the Coax book's note
  `> Note: This feature is planned and not available` is planned.
- Inside an implemented chapter, a section whose heading says `(planned)` or
  `(later)` is planned.
- Every other chapter is implemented.

<!-- source: psionic/lib/book-search/rank.coax status -->
```jai
// A chapter's status comes from its own file: the plan is planned work; a
// third line that is Psionic's or Aethyr's draft marker makes a design draft,
// and one that is the Coax book's planned-feature note makes it planned;
// every other chapter is implemented. Inside an implemented chapter, a
// section whose heading says "(planned)" or "(later)" is planned.
bs_status :: (name: string, text: string) -> BsStatus {
  if name == "PLAN.md" { return BsStatus.planned; }
  if bs_line_starts(text, 2, BS_DRAFT) { return BsStatus.draft; }
  if bs_line_starts(text, 2, BS_PLANNED) { return BsStatus.planned; }
  return BsStatus.implemented;
}
bs_section_status :: (chapter: BsStatus, heading: string) -> BsStatus {
  if chapter != BsStatus.implemented { return chapter; }
  if bs_contains(heading, "(planned)") || bs_contains(heading, "(later)") { return BsStatus.planned; }
  return chapter;
}
bs_status_label :: (s: BsStatus) -> string {
  if s == BsStatus.draft { return "design draft, not implemented"; }
  if s == BsStatus.planned { return "planned, not built"; }
  return "implemented";
}
bs_shipped :: (s: BsStatus) -> bool { return s == BsStatus.implemented; }
```

The text output prints the label in brackets on every result. For a draft or
planned passage it adds that the text describes work that is not built. The
JSON output carries `status` and a `shipped` flag that is true only for
implemented passages. The index stores the chapter text, and the tool reads
each chapter's status again from that text, so a change to the rules applies
to an existing index.

## Words

A word is a piece from chapter 123's splitting rule with leading ASCII
punctuation and spaces removed and ASCII letters lowercased. A piece with no
ASCII letter or digit, such as a dash, is not a word.

<!-- source: psionic/lib/book-search/corpus.coax words -->
```jai
// The next piece of text at `at` under chapter 123's splitting rule, as a word:
// leading ASCII punctuation and spaces removed, ASCII letters lowercased, at
// most BS_WORD_MAX bytes. size is 0 when the piece holds no ASCII letter or
// digit; such pieces are not words.
bs_word :: (table: [] u8, t: PtLayout, text: string, at: int, out: [] u8) -> BsWord {
  end := bt_piece(table, t, text, at);
  a := at;
  while a < end && text[a] < 128 && !bs_has_word(text[a..<a + 1]) { a += 1; }
  if a == end || !bs_has_word(text[a..<end]) || end - a > BS_WORD_MAX { return BsWord.{next = end}; }
  for i: a..<end { c := text[i]; out[i - a] = ifx c >= 'A' && c <= 'Z' then c + 32 else c; }
  return BsWord.{next = end, size = end - a};
}
```

The vocabulary is a hash table with linear probing. Training counts every
word in the books, sorts the words by count and then by bytes, and keeps those
seen at least twice. A word's id is its rank. At this revision the vocabulary
holds 8,578 words. A query word outside the vocabulary is dropped,
and the tool reports how many query words it knew.

<!-- source: psionic/lib/book-search/corpus.coax vocab -->
```jai
// FNV-1a over the word's bytes picks a slot; collisions probe the next slot.
bs_slot :: (v: BsVocab, word: string) -> int {
  h: u64 = 14695981039346656037;
  for c: word { h = mul_wrap(h ^ cast(u64) c, 1099511628211); }
  mask := v.slots.count - 1;
  s := cast(int) (h & cast(u64) mask);
  while v.slots[s] >= 0 && cast(string) v.pool[v.from[v.slots[s]]..<v.from[v.slots[s]] + v.size[v.slots[s]]] != word { s = (s + 1) & mask; }
  return s;
}
// The word's id, or -1 when it is not in the vocabulary.
bs_find :: (v: BsVocab, word: string) -> int { return v.slots[bs_slot(v, word)]; }
// Counts one more occurrence, adding the word when it is new; -1 when full.
bs_count :: (v: [] BsVocab, word: string) -> int {
  s := bs_slot(v[0], word);
  e := v[0].slots[s];
  if e >= 0 { v[0].count[e] += 1; return e; }
  if v[0].n == v[0].from.count || v[0].n * 2 >= v[0].slots.count || v[0].used + word.count > v[0].pool.count { v[0].full = true; return -1; }
  e = v[0].n; v[0].n += 1;
  v[0].from[e] = v[0].used; v[0].size[e] = word.count; v[0].count[e] = 1;
  for c: word { v[0].pool[v[0].used] = c; v[0].used += 1; }
  v[0].slots[s] = e;
  return e;
}
```

## BM25

BM25 scores a passage by the query words it holds. The passage's words are its
heading path and text. The constants are the usual ones, k1 = 1.2 and
b = 0.75, and the IDF is Lucene's.

<!-- source: psionic/lib/book-search/rank.coax bm25 -->
```jai
// Okapi BM25 with k1 = 1.2 and b = 0.75. The IDF is Lucene's, which stays
// positive for a term in more than half the passages. Callers pass tf >= 1
// and an average length above zero; a term no passage holds scores zero.
bs_idf :: (n: int, df: int) -> f64 { return log(1 + (f64(n) - f64(df) + 0.5) / (f64(df) + 0.5)); }
bs_bm25 :: (tf: int, df: int, n: int, length: int, average: f64) -> f64 {
  if df <= 0 || n <= 0 { return 0; }
  t := f64(tf);
  return bs_idf(n, df) * t * (BS_K1 + 1) / (t + BS_K1 * (1 - BS_B + BS_B * f64(length) / average));
}
// query: sorted distinct term ids. ids, counts: the passage's sorted distinct
// terms and how often each occurs. A merge walks both lists once.
bs_passage_bm25 :: (query: [] int, ids: [] int, counts: [] int, df: [] int, n: int, length: int, average: f64) -> f64 {
  score := 0.0; i := 0; j := 0;
  while i < query.count && j < ids.count {
    if query[i] < ids[j] { i += 1; }
    else if query[i] > ids[j] { j += 1; }
    else { score += bs_bm25(counts[j], df[ids[j]], n, length, average); i += 1; }
  }
  return score;
}
```

## Psionic's encoder

Each vocabulary word owns a row of 64 learned numbers. A single learned query
vector scores every row of a passage, a softmax turns the scores into weights,
and the weighted sum of the rows, scaled to length one, is the passage's
embedding. There is no causal mask: every position contributes to the one
output. Two embeddings compare by their dot product, which for unit vectors
is the cosine.

<!-- source: psionic/lib/book-search/encoder.coax forward -->
```jai
// One sequence: scores s_i = q . e_i / sqrt(64) over every position, a =
// softmax(s), v = sum a_i e_i, u = v / |v|. At most BE_TOKENS ids are read;
// an empty sequence or a zero sum leaves u zero, which scores 0 against all.
be_forward :: (table: [] f64, query: [] f64, ids: [] int, t: [] BeTrace) {
  n := ifx ids.count < BE_TOKENS then ids.count else BE_TOKENS;
  t[0].n = n; t[0].r = 0;
  for d: 0..<BE_WIDTH { t[0].v[d] = 0; t[0].u[d] = 0; }
  if n == 0 { return; }
  scale := 1.0 / sqrt(f64(BE_WIDTH));
  top := -1.0e300;
  for i: 0..<n {
    row := ids[i] * BE_WIDTH; s := 0.0;
    for d: 0..<BE_WIDTH { s += query[d] * table[row + d]; }
    t[0].a[i] = s * scale;
    if t[0].a[i] > top { top = t[0].a[i]; }
  }
  sum := 0.0;
  for i: 0..<n { t[0].a[i] = exp(t[0].a[i] - top); sum += t[0].a[i]; }
  for i: 0..<n {
    t[0].a[i] /= sum; row := ids[i] * BE_WIDTH;
    for d: 0..<BE_WIDTH { t[0].v[d] += t[0].a[i] * table[row + d]; }
  }
  r := 0.0;
  for d: 0..<BE_WIDTH { r += t[0].v[d] * t[0].v[d]; }
  r = sqrt(r); t[0].r = r;
  if r == 0 { return; }
  for d: 0..<BE_WIDTH { t[0].u[d] = t[0].v[d] / r; }
}
```

The backward pass goes through the normalization, the weighted sum and the
softmax. A check compares it with central finite differences on a small
batch; the largest difference is under 1e-7.

<!-- source: psionic/lib/book-search/encoder.coax backward -->
```jai
// Adds dL/dtable and dL/dquery for one sequence, given dL/du.
be_backward :: (table: [] f64, query: [] f64, ids: [] int, t: BeTrace, du: [] f64, g_table: [] f64, g_query: [] f64) {
  if t.n == 0 || t.r == 0 { return; }
  // Through u = v / r: dv = (du - u (u . du)) / r.
  dot := 0.0;
  for d: 0..<BE_WIDTH { dot += t.u[d] * du[d]; }
  dv: [BE_WIDTH] f64;
  for d: 0..<BE_WIDTH { dv[d] = (du[d] - t.u[d] * dot) / t.r; }
  // Through v = sum a_i e_i and a = softmax(s).
  g: [BE_TOKENS] f64; mean := 0.0;
  for i: 0..<t.n {
    row := ids[i] * BE_WIDTH; s := 0.0;
    for d: 0..<BE_WIDTH { s += dv[d] * table[row + d]; }
    g[i] = s; mean += t.a[i] * s;
  }
  scale := 1.0 / sqrt(f64(BE_WIDTH));
  for i: 0..<t.n {
    row := ids[i] * BE_WIDTH;
    ds := t.a[i] * (g[i] - mean) * scale;
    for d: 0..<BE_WIDTH {
      g_query[d] += ds * table[row + d];
      g_table[row + d] += t.a[i] * dv[d] + ds * query[d];
    }
  }
}
```

Training uses only the books; it never sees a query from the evaluation. Each
batch pairs a query side with a passage side, and the loss asks each query to
pick its own passage among the batch, with temperature 0.05:

<!-- source: psionic/lib/book-search/encoder.coax batch -->
```jai
// One batch of B (query, passage) pairs: sims S[b][j] = u_b . w_j / tau, loss
// mean_b(-log softmax_j(S[b])[b]); gradients are added into g_table, g_query.
// ids holds every sequence; query b is ids[qf[b]..<qt[b]], passage j is
// ids[pf[j]..<pt[j]]. traces holds 2B traces.
be_batch :: (table: [] f64, query: [] f64, ids: [] int, qf: [] int, qt: [] int, pf: [] int, pt: [] int, tau: f64, traces: [] BeTrace, g_table: [] f64, g_query: [] f64) -> BeBatch {
  b := qf.count; out: BeBatch;
  for k: 0..<b {
    be_forward(table, query, ids[qf[k]..<qt[k]], traces[k..<k + 1]);
    be_forward(table, query, ids[pf[k]..<pt[k]], traces[b + k..<b + k + 1]);
  }
  p: [BE_BATCH * BE_BATCH] f64;
  for i: 0..<b {
    top := -1.0e300;
    for j: 0..<b {
      s := 0.0;
      for d: 0..<BE_WIDTH { s += traces[i].u[d] * traces[b + j].u[d]; }
      p[i * b + j] = s / tau;
      if p[i * b + j] > top { top = p[i * b + j]; }
    }
    sum := 0.0; best := 0;
    for j: 0..<b { if p[i * b + j] > p[i * b + best] { best = j; } }
    if best == i { out.correct += 1; }
    for j: 0..<b { p[i * b + j] = exp(p[i * b + j] - top); sum += p[i * b + j]; }
    out.loss += -log(p[i * b + i] / sum) / f64(b);
    for j: 0..<b { p[i * b + j] = (p[i * b + j] / sum - ifx i == j then 1.0 else 0.0) / (f64(b) * tau); }
  }
  du: [BE_WIDTH] f64;
  for i: 0..<b {
    for d: 0..<BE_WIDTH { du[d] = 0; }
    for j: 0..<b { for d: 0..<BE_WIDTH { du[d] += p[i * b + j] * traces[b + j].u[d]; } }
    be_backward(table, query, ids[qf[i]..<qt[i]], traces[i], du[..], g_table, g_query);
  }
  for j: 0..<b {
    for d: 0..<BE_WIDTH { du[d] = 0; }
    for i: 0..<b { for d: 0..<BE_WIDTH { du[d] += p[i * b + j] * traces[i].u[d]; } }
    be_backward(table, query, ids[pf[j]..<pt[j]], traces[b + j], du[..], g_table, g_query);
  }
  return out;
}
```

The pairs come from the structure of the books. Half the time the query is a
passage's heading path and the target is its text. Otherwise the query is a
span of 4 to 15 words from the text, and the target is the heading path and
the text without that span. The counter-based generator from
`counter-rng.coax` draws every choice from one seed, so training is
reproducible on one machine. AdamW updates only the rows that a batch used,
with weight decay off.

<!-- source: psionic/lib/book-search/encoder.coax train -->
```jai
// Trains table (vocab x 64) and query in place. Passage k's heading words are
// ids[hf[k]..<ht[k]] and its text words ids[bf[k]..<bt[k]]. Every step draws
// `batch` distinct passages; for each, half the time the query is its heading
// path and the target its text, otherwise the query is a span of 4 to 15
// words and the target the heading and text with that span removed.
be_train :: (c: BeConfig, ids: [] int, hf: [] int, ht: [] int, bf: [] int, bt: [] int, table: [] f64, query: [] f64,
             m: [] f64, v: [] f64, g: [] f64, mq: [] f64, vq: [] f64, gq: [] f64, mark: [] int) -> BeTrained {
  r := BeTrained.{first = -1, last = -1};
  passages := hf.count;
  if c.batch < 2 || c.batch > BE_BATCH || passages < c.batch || c.steps < 1 || c.tau <= 0 { return r; }
  group := AwGroup.{lr = c.lr, beta1 = 0.9, beta2 = 0.999, eps = 1e-8, decay = 0};
  rng := [1] CrSeed.[CrSeed.{seed = c.seed}];
  for i: 0..<table.count { table[i] = 0.2 * (be_draw(rng[..]) - 0.5); m[i] = 0; v[i] = 0; g[i] = 0; }
  for d: 0..<BE_WIDTH { query[d] = 0; mq[d] = 0; vq[d] = 0; gq[d] = 0; }
  for i: 0..<mark.count { mark[i] = -1; }
  work: [BE_BATCH * (BE_QUERY + BE_TOKENS + BE_QUERY)] int;
  qf: [BE_BATCH] int; qt: [BE_BATCH] int; pf: [BE_BATCH] int; pt: [BE_BATCH] int; chosen: [BE_BATCH] int;
  traces: [2 * BE_BATCH] BeTrace;
  early := 0.0; late := 0.0; window := ifx c.steps < 200 then (c.steps + 3) / 4 else 50;
  for step: 0..<c.steps {
    used := 0;
    for k: 0..<c.batch {
      p := 0;
      while true {
        p = cast(int) (be_draw(rng[..]) * f64(passages));
        fresh := true;
        for j: 0..<k { if chosen[j] == p { fresh = false; } }
        if fresh { break; }
      }
      chosen[k] = p;
      body := bt[p] - bf[p]; heads := ht[p] - hf[p];
      span := body >= 8 && (heads == 0 || be_draw(rng[..]) < 0.5);
      if span {
        spread := 4 + cast(int) (be_draw(rng[..]) * 12); if spread > body / 2 { spread = body / 2; }
        at := bf[p] + cast(int) (be_draw(rng[..]) * f64(body - spread + 1));
        qf[k] = used; for i: at..<at + spread { work[used] = ids[i]; used += 1; } qt[k] = used;
        pf[k] = used;
        for i: hf[p]..<ht[p] { if used - pf[k] < BE_TOKENS { work[used] = ids[i]; used += 1; } }
        for i: bf[p]..<bt[p] { if (i < at || i >= at + spread) && used - pf[k] < BE_TOKENS { work[used] = ids[i]; used += 1; } }
        pt[k] = used;
      } else {
        from := ifx heads > BE_QUERY then ht[p] - BE_QUERY else hf[p];
        qf[k] = used; for i: from..<ht[p] { work[used] = ids[i]; used += 1; } qt[k] = used;
        pf[k] = used; for i: bf[p]..<bt[p] { if used - pf[k] < BE_TOKENS { work[used] = ids[i]; used += 1; } } pt[k] = used;
      }
    }
    out := be_batch(table, query, work[0..<used], qf[0..<c.batch], qt[0..<c.batch], pf[0..<c.batch], pt[0..<c.batch], c.tau, traces[..], g, gq);
    if step < window { early += out.loss / f64(window); }
    if step >= c.steps - window { late += out.loss / f64(window); }
    t := step + 1;
    for i: 0..<used {
      id := work[i];
      if mark[id] == step { continue; }
      mark[id] = step;
      for d: 0..<BE_WIDTH {
        k := id * BE_WIDTH + d;
        cell := aw_cell(group, t, table[k], g[k], m[k], v[k]);
        table[k] = cell.p; m[k] = cell.m; v[k] = cell.v; g[k] = 0;
      }
    }
    for d: 0..<BE_WIDTH { cell := aw_cell(group, t, query[d], gq[d], mq[d], vq[d]); query[d] = cell.p; mq[d] = cell.m; vq[d] = cell.v; gq[d] = 0; }
  }
  return BeTrained.{ok = true, steps = c.steps, first = early, last = late};
}
```

The settings are 1,500 steps, 64 pairs a batch, temperature 0.05, learning
rate 0.01 and seed 54. They were chosen on the development set
[dev.tsv](../fixtures/book-search/dev.tsv), before the held-out set was
written. Over training, the batch loss falls from about 2.8 to about 0.5. The
weights file stores the vocabulary, the 64-number rows and the query vector in
binary32; the index records the weights' SHA-256, and an index built with
other weights is rebuilt.

## Rank and fuse

A higher score ranks first. Equal scores rank the earlier passage first, so
every ranking is fully ordered and the same on every run.

<!-- source: psionic/lib/book-search/rank.coax rank -->
```jai
// a ranks before b: a higher score, or an equal score and an earlier passage.
bs_before :: (a: BsHit, b: BsHit) -> bool {
  if a.score > b.score { return true; }
  if a.score < b.score { return false; }
  return a.passage < b.passage;
}
// The best out.count passages whose score is above least, best first.
bs_top :: (scores: [] f64, least: f64, out: [] BsHit) -> int {
  n := 0;
  for p: 0..<scores.count {
    if scores[p] <= least { continue; }
    hit := BsHit.{passage = p, score = scores[p]};
    if n == out.count && (n == 0 || !bs_before(hit, out[n - 1])) { continue; }
    if n < out.count { n += 1; }
    j := n - 1;
    while j > 0 && bs_before(hit, out[j - 1]) { out[j] = out[j - 1]; j -= 1; }
    out[j] = hit;
  }
  return n;
}
```

The Psionic retriever takes the top 100 passages from BM25 and the top 100
from vector search and fuses them by reciprocal rank: rank r adds
1 / (60 + r). A passage that both lists rank highly comes first.

<!-- source: psionic/lib/book-search/rank.coax fuse -->
```jai
// Reciprocal-rank fusion: rank r (1 is best, 0 is absent) adds 1/(60 + r).
bs_rrf :: (lexical: int, dense: int) -> f64 {
  s := 0.0;
  if lexical > 0 { s += 1.0 / f64(BS_FUSE + lexical); }
  if dense > 0 { s += 1.0 / f64(BS_FUSE + dense); }
  return s;
}
// scores: one per passage, zeroed here, then each passage's fused score.
bs_fuse :: (lexical: [] BsHit, dense: [] BsHit, scores: [] f64) {
  for i: 0..<scores.count { scores[i] = 0; }
  for i: 0..<lexical.count { scores[lexical[i].passage] += bs_rrf(i + 1, 0); }
  for i: 0..<dense.count { scores[dense[i].passage] += bs_rrf(0, i + 1); }
}
```

## The index and its revision

The index file holds the books as they were when it was built: every chapter's
path, SHA-256, last commit and text; every passage and its identity; and every
passage embedding. Results come from this stored text, not from the files on
disk. Before it answers, the tool hashes every chapter file again and compares
the lists:

<!-- source: psionic/lib/book-search/rank.coax fresh -->
```jai
// held: the index's files (sorted paths) and their content hashes; now: the
// files as they are. Fresh means identical lists: a changed, added or removed
// chapter makes the index stale, and first names the first held file that
// changed or went away.
bs_freshness :: (held: [] string, held_hash: [] string, now: [] string, now_hash: [] string) -> BsFresh {
  r := BsFresh.{first = -1};
  i := 0; j := 0;
  while i < held.count || j < now.count {
    if j == now.count || (i < held.count && bs_less(held[i], now[j])) {
      r.removed += 1; if r.first < 0 { r.first = i; } i += 1;
    } else if i == held.count || bs_less(now[j], held[i]) {
      r.added += 1; j += 1;
    } else {
      if held_hash[i] != now_hash[j] { r.changed += 1; if r.first < 0 { r.first = i; } }
      i += 1; j += 1;
    }
  }
  r.fresh = r.changed == 0 && r.added == 0 && r.removed == 0;
  return r;
}
```

A changed, added or removed chapter makes the index stale. By default the tool
says what changed and rebuilds the index with the current weights, which takes
a few seconds. With `--no-build` it refuses to answer.

## A stale index

This is the broken case: an index built before this chapter was rewritten.
The chapter changed from a design draft to an implemented chapter, so an
answer from the old index would show the old draft text and the old status.
The tool finds the change before it answers:

```shell
coax/web/tool.sh psionic/tools/book-search.coax "attention without a causal mask" --no-build
```

```text
book-search: stale index: it holds the books at revision db51247275c1, and the books now (revision db51247275c1, uncommitted edits in chapter files: 1) differ:
  changed files: 1
  added files:   0
  removed files: 0
  first: psionic/book/ch054-search-the-coax-book.md
    held content sha256 cc804e502671ba79
    now  content sha256 54bb9740a516ce82
book-search: refusing to answer from a stale index (--no-build); run without --no-build to rebuild it
```

The exit status is 3. The same rule runs without files in
[book-search-stale.coax](../examples/book-search-stale.coax):

<!-- source: psionic/examples/book-search-stale.coax stale -->
```jai
main :: () {
  paths := [2] string.["psionic/book/ch001-learning-a-number.md", "psionic/book/ch054-search-the-book.md"];
  built := [2] string.["# 1. Learning a number\n\nTrain one weight.\n", "# 54. Search the book\n\n> **Status: unimplemented — design draft.** Nothing here runs yet.\n"];
  now := [2] string.["# 1. Learning a number\n\nTrain one weight.\n", "# 54. Search the book\n\nRank passages with BM25 and an encoder.\n"];
  held: [2] string; current: [2] string; hx: [2][64] u8; cx: [2][64] u8; pool: [256] u8;
  for f: 0..<2 {
    for i: 0..<built[f].count { pool[i] = built[f][i]; }
    held[f] = bs_hex(bs_file_hash(pool[..], 0, built[f].count), 32, hx[f][..]);
    for i: 0..<now[f].count { pool[i] = now[f][i]; }
    current[f] = bs_hex(bs_file_hash(pool[..], 0, now[f].count), 32, cx[f][..]);
  }
  same := bs_freshness(paths[..], held[..], paths[..], held[..]);
  print("index against the books it was built from: fresh %\n", same.fresh);
  r := bs_freshness(paths[..], held[..], paths[..], current[..]);
  print("index against the books now: fresh %, changed %, added %, removed %\n", r.fresh, r.changed, r.added, r.removed);
  if !r.fresh {
    print("stale index: % changed since the index was built\n", paths[r.first]);
    print("  held sha256 %, now %\n", held[r.first][0..<16], current[r.first][0..<16]);
    print("  held status: %; now: %\n", bs_status_label(bs_status("ch054-search-the-book.md", built[1])), bs_status_label(bs_status("ch054-search-the-book.md", now[1])));
    print("refusing to answer from the stale index; rebuild it\n");
  }
}
```

```text
index against the books it was built from: fresh true
index against the books now: fresh false, changed 1, added 0, removed 0
stale index: psionic/book/ch054-search-the-book.md changed since the index was built
  held sha256 dd5d1bed4e762740, now 7b255e77ba98c18f
  held status: design draft, not implemented; now: implemented
refusing to answer from the stale index; rebuild it
```

## Search a small book

[book-search.coax](../examples/book-search.coax) builds two chapters in
memory, one implemented and one a design draft, and searches them with BM25.
Both chapters match the query; the draft's result says so.

<!-- source: psionic/examples/book-search.coax example -->
```jai
main :: () {
  chapters := [2] string.[
    "# 1. Learning a number\n\nTrain one weight by gradient descent.\n\n## Measure the error\n\nThe loss is the mean squared error of the predictions.\n",
    "# 54. Search the book\n\n> **Status: unimplemented — design draft.** Nothing here runs yet.\n\n## Rank passages\n\nRank passages by BM25, then by squared error of embeddings.\n"];
  names := [2] string.["ch001-learning-a-number.md", "ch054-search-the-book.md"];
  pool: [1024] u8; heads: [256] u8; passages: [8] BsPassage;
  status: [2] BsStatus; n := 0; used := 0; at := 0;
  for c: 0..<2 {
    from := at; for ch: chapters[c] { pool[at] = ch; at += 1; }
    status[c] = bs_status(names[c], chapters[c]);
    r := bs_chunk(pool[..], from, at, c, passages[..], n, heads[..], used);
    n = r.passages; used = r.heads;
  }
  // Words and their vocabulary ids.
  slots: [256] int; vpool: [1024] u8; vfrom: [64] int; vsize: [64] int; vcount: [64] int;
  v := [1] BsVocab.[BsVocab.{slots = slots[..], pool = vpool[..], from = vfrom[..], size = vsize[..], count = vcount[..]}];
  bs_vocab_clear(v[..]);
  table := PT_TOY; t := pt_layout(table[..]); word: [64] u8;
  terms: [8][64] int; counts: [8][64] int; distinct: [8] int; length: [8] int; df: [64] int; total := 0;
  for p: 0..<n {
    q := passages[p]; ids: [64] int; k := 0;
    texts := [2] string.[cast(string) heads[q.head_from..<q.head_to], cast(string) pool[q.from..<q.to]];
    for text: texts {
      i := 0;
      while i < text.count { w := bs_word(table[..], t, text, i, word[..]); if w.size > 0 { ids[k] = bs_count(v[..], cast(string) word[0..<w.size]); k += 1; } i = w.next; }
    }
    length[p] = k; total += k;
    for i: 0..<k { terms[p][i] = ids[i]; }
    distinct[p] = bs_distinct(terms[p][0..<k]);
    for i: 0..<distinct[p] { c := 0; for j: 0..<k { if ids[j] == terms[p][i] { c += 1; } } counts[p][i] = c; df[terms[p][i]] += 1; }
  }
  query := [2] string.["squared", "error"]; qids: [2] int;
  for i: 0..<2 { qids[i] = bs_find(v[0], query[i]); }
  qn := bs_distinct(qids[..]);
  scores: [8] f64;
  for p: 0..<n { scores[p] = bs_passage_bm25(qids[0..<qn], terms[p][0..<distinct[p]], counts[p][0..<distinct[p]], df[..], n, length[p], f64(total) / f64(n)); }
  hits: [3] BsHit;
  shown := bs_top(scores[0..<n], 0, hits[..]);
  print("query \"squared error\": % passages, % with a match\n", n, shown);
  hex: [64] u8;
  for i: 0..<shown {
    q := passages[hits[i].passage];
    s := bs_section_status(status[q.file], cast(string) heads[q.head_from..<q.head_to]);
    print("%. % > % [%]\n", i + 1, names[q.file], cast(string) heads[q.head_from..<q.head_to], bs_status_label(s));
    print("   passage %, part %, shipped: %\n", bs_hex(q.hash, 8, hex[..]), q.part, bs_shipped(s));
  }
}
```

```text
query "squared error": 4 passages, 2 with a match
1. ch001-learning-a-number.md > 1. Learning a number > Measure the error [implemented]
   passage 26b81cb2515a99e3, part 0, shipped: true
2. ch054-search-the-book.md > 54. Search the book > Rank passages [design draft, not implemented]
   passage 564ccc79383e12df, part 0, shipped: false
```

## Measure it

The held-out set, [heldout.tsv](../fixtures/book-search/heldout.tsv), has 36
queries: 12 for each book. It was written after the encoder's settings were
fixed, in words that differ from the headings where possible. Each query names
its relevant passages by chapter file and heading. A heading that is a section
makes every passage under that section relevant; a chapter title makes every
passage of the chapter relevant.

- **Recall@k** is the share of queries whose first k results hold at least one
  relevant passage.
- **MRR@10** is the mean of 1 / rank of the first relevant result, counting 0
  when none is in the first 10.

<!-- source: psionic/lib/book-search/rank.coax metrics -->
```jai
// The 1-based rank of the first relevant hit, or 0 when none is listed.
bs_first_relevant :: (hits: [] BsHit, relevant: [] bool) -> int {
  for i: 0..<hits.count { if relevant[hits[i].passage] { return i + 1; } }
  return 0;
}
// How many relevant passages the first k hits hold.
bs_found :: (hits: [] BsHit, relevant: [] bool, k: int) -> int {
  n := 0;
  for i: 0..<hits.count { if i < k && relevant[hits[i].passage] { n += 1; } }
  return n;
}
// Recall at k, per query: does the first k hold a relevant passage? A label
// names a section or a chapter, which may span several passages; finding any
// of them answers the query.
bs_hit :: (hits: [] BsHit, relevant: [] bool, k: int) -> bool { return bs_found(hits, relevant, k) > 0; }
// Reciprocal rank with a cutoff: 1/rank when 1 <= rank <= cutoff, else 0.
bs_reciprocal :: (rank: int, cutoff: int) -> f64 {
  if rank < 1 || rank > cutoff { return 0; }
  return 1.0 / f64(rank);
}
```

```shell
coax/web/tool.sh psionic/tools/book-search.coax --eval psionic/fixtures/book-search/heldout.tsv
```

```text
36 queries over 4956 passages and 8578 words, books at db51247275c1, weights aa856e054d542b10
| Retriever | Recall@1 | Recall@5 | Recall@10 | MRR@10 |
| --- | ---: | ---: | ---: | ---: |
| BM25 | 0.417 | 0.667 | 0.778 | 0.505 |
| Exhaustive vector search | 0.361 | 0.667 | 0.722 | 0.477 |
| Psionic retriever (fused) | 0.417 | 0.639 | 0.806 | 0.518 |
```

BM25 beats exhaustive vector search over Psionic's own embeddings at
Recall@1, Recall@10 and MRR@10, and ties it at Recall@5. The fused retriever
ties BM25 at Recall@1, has the best Recall@10 and MRR@10, and is one query
behind both at Recall@5. With 36 queries, one query moves a recall figure by
0.028, so differences of one or two queries are within noise. The figures
were measured with this chapter's final text in the working tree, on top of
the revision shown. The development set's figures chose the settings and are
not part of this evaluation.

## Checks

```shell
psionic/check.sh --only book-search
```

[check_book_search.sh](../tests/check_book_search.sh) runs the checks of all
three files in debug and release builds, runs both examples in both builds
against their recorded output, mutates `rank.coax` and checks the book. The
checks cover the status rules (including a marker on the wrong line), the BM25
formula, query terms made distinct, the tie rule, the top-k floor, fusion,
recall and reciprocal rank at their cutoffs, and freshness with changed,
added and removed files. `coax mutate` on `rank.coax` kills all
284 mutants; none survive. The encoder's checks include the
finite-difference gradient check and a short training run that lowers the
loss and repeats exactly.

## What this does not do yet

- The encoder is small: a learned word table and one attention-pooling query.
  It has no subwords and no positions, and it loses to BM25 on the held-out
  set.
- Nemotron-3-Embed-1B-BF16 is not implemented.
- There is no approximate index; [chapter 55](ch055-search-without-scanning-everything.md)
  adds one and measures it against this chapter's exhaustive search.
- Fusion is reciprocal rank only; [chapter 56](ch056-rerank-the-candidates.md)
  reranks the candidates.
- Nothing generates answers; [chapter 57](ch057-answer-from-current-sources.md)
  answers from these passages.
- Training is reproducible on one machine. Weights from another C compiler or
  math library may differ in their last bits; the index records the weights'
  digest, not a claim that every machine produces it.
- The status of a chapter comes from its own marker. A shared status registry
  for the books does not exist yet; when one lands, `bs_status` should read
  it.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- S. Robertson and H. Zaragoza, "The Probabilistic Relevance Framework: BM25
  and Beyond", 2009.
- G. Cormack, C. Clarke and S. Büttcher, "Reciprocal Rank Fusion Outperforms
  Condorcet and Individual Rank Learning Methods", SIGIR 2009.
- A. van den Oord, Y. Li and O. Vinyals, "Representation Learning with
  Contrastive Predictive Coding", 2018 (the in-batch softmax loss).
