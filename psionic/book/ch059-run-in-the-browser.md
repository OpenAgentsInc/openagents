# 59. Run in the browser

> **Status: partly implemented.** Chapter 54's search runs in a browser tab:
> the query encoder, BM25, exhaustive vector search and fusion compiled from
> Coax to Wasm, run in headless Chrome against a local `coax serve` page, with
> results equal bit for bit to the native CPU tool on the 36 held-out queries.
> Not built yet: the page on production openagents.com, a smaller distilled
> query encoder, WebGPU compute, and compressed downloads.

This chapter moves [chapter 54](ch054-search-the-coax-book.md)'s search into
the browser. The site serves three files, and the page searches them without
a server:

- **The query encoder**: chapter 54's weights file for profile
  `bs-attnpool-d64-v1`, byte for byte. It turns a query into 64 numbers.
- **The index**: every passage's heading path, identity and BM25 term lists,
  and the document embeddings the native tool computed with that same
  encoder. It holds no passage text.
- **The manifest**: five lines of text naming the encoder and the index by
  SHA-256 and size, and naming the books they were built from by one corpus
  digest.

The page compiles nothing and decides nothing in JavaScript. The search is
the same Coax that runs natively, compiled to Wasm with
[coax/web/wasm.sh](../../coax/web/wasm.sh): it hashes the downloads, checks
them against the manifest, and refuses to answer from a bundle that does not
match.

The library is [bundle.coax](../lib/book-search/bundle.coax) (`#authority`:
whether the page may answer) and
[browser.coax](../lib/book-search/browser.coax) (the file formats, the
loader and the search). The page is
[psionic/web/book-search/](../web/book-search/index.html): `search.coax` (the
module's state and entry points), `face.c` (its Wasm exports), `index.html`
(the host page) and `check.sh` (the headless run).

## Which encoder the browser may use

A query embedding is only useful against document embeddings from a
compatible encoder. Two encoders that both produce 64 numbers are not
compatible because of that: a different vocabulary, a different row for one
word or a different query vector puts the query in another space, and the
cosine scores mean nothing. Equal vector dimensions prove nothing.

The draft of this chapter named three ways to show compatibility: a measured
alignment, distillation from the original query encoder, or a matched query
and document encoder pair. This chapter uses the third, in its strictest
form: the browser downloads the encoder that embedded the documents, and the
manifest names it by the SHA-256 of its bytes. The index records the digest
of the encoder that made its embeddings, and the page refuses an index whose
encoder digest differs from the manifest's. A smaller distilled encoder is
not built; it would need its own measured alignment against these
embeddings before a page could use it.

## The admission rule

The page computes what it downloaded: each file's SHA-256 and size, the
encoder digest the index records, the corpus digest the index records, and
the corpus digest of the file list the index holds. One procedure decides
from those values alone:

<!-- source: psionic/lib/book-search/bundle.coax admit -->
```jai
// The checks run in this order, and the first that fails names the refusal:
// the manifest parses and names this encoder profile; the encoder and index
// bytes are the ones the manifest names; the encoder's own header names the
// same profile; the index parses; the index's
// embeddings were made by that same encoder; the index's recorded corpus
// digest is the digest of the files it lists; and that corpus is the books
// the manifest says are current.
bb_admit :: (m: BbManifest, s: BbSeen) -> BbVerdict {
  if !m.ok { return BbVerdict.malformed; }
  if m.profile != BB_PROFILE { return BbVerdict.wrong_profile; }
  if s.encoder_bytes != m.encoder_bytes || !bb_same(s.encoder, m.encoder) { return BbVerdict.encoder_changed; }
  if s.index_bytes != m.index_bytes || !bb_same(s.index, m.index) { return BbVerdict.index_changed; }
  if s.profile != BB_PROFILE { return BbVerdict.wrong_profile; }
  if !s.index_ok || !bb_same(s.index_corpus, s.listed_corpus) { return BbVerdict.corrupt_index; }
  if !bb_same(s.index_encoder, m.encoder) { return BbVerdict.other_encoder; }
  if !bb_same(s.index_corpus, m.corpus) { return BbVerdict.stale_index; }
  return BbVerdict.admitted;
}
```

The corpus digest is SHA-256 over every chapter file in path order: its
path, a newline and its 32-byte content hash. The site writes the manifest
from the books as they are when it deploys, so an index built from older
books, served from a cache or a stale build, names another corpus and is
refused.

A worked case. Chapter 54's tool trained the weights at revision
`331ac8a5a616`; their file is 2,879,613 bytes with SHA-256
`66a9e806e7b80477…`. Suppose the page receives that encoder, the real index,
and a manifest whose corpus line names other books:

1. The manifest parses and names `bs-attnpool-d64-v1`.
2. SHA-256 of the 2,879,613 encoder bytes is `66a9e806…`, the manifest's
   value, at the manifest's size.
3. The index's SHA-256 and size are the manifest's.
4. The encoder's own header names `bs-attnpool-d64-v1`.
5. The index parses, and the corpus digest of its 401 listed files equals the
   corpus digest it records.
6. The index records encoder `66a9e806…`, the manifest's.
7. The index's corpus digest is not the manifest's: **stale index**.

Change one byte of the encoder instead, and step 2 fails: **wrong encoder**.
Change that byte and also rewrite the manifest to name the changed file, and
steps 2 to 5 pass but step 6 fails: the index's embeddings were made by
another encoder.

## The files

The manifest is text, so a person can read what the page will check:

<!-- source: psionic/lib/book-search/browser.coax manifest -->
```jai
// The manifest, five lines of text:
//   psionic-book-search-bundle 1
//   profile bs-attnpool-d64-v1
//   encoder <sha256 hex> <bytes>
//   index <sha256 hex> <bytes>
//   corpus <sha256 hex>
bb_write_manifest :: (w: [] BbW, m: BbManifest) {
  hex: [64] u8;
  bb_text(w, BB_MANIFEST_MAGIC); bb_text(w, "\nprofile "); bb_text(w, m.profile);
  bb_text(w, "\nencoder "); bb_text(w, bs_hex(m.encoder, 32, hex[..])); bb_byte(w, ' '); bb_decimal(w, m.encoder_bytes);
  bb_text(w, "\nindex "); bb_text(w, bs_hex(m.index, 32, hex[..])); bb_byte(w, ' '); bb_decimal(w, m.index_bytes);
  bb_text(w, "\ncorpus "); bb_text(w, bs_hex(m.corpus, 32, hex[..])); bb_byte(w, '\n');
}
```

The index is little-endian binary. Terms and counts fit in 16 bits because
the vocabulary has fewer than 65,536 words:

<!-- source: psionic/lib/book-search/browser.coax index -->
```jai
// The index: magic; the encoder digest that made its embeddings; its corpus
// digest; every file (path, content hash, status, book); the heading paths;
// every passage (file, heading span, part, BM25 length, term span, identity
// hash); document frequencies; the average length; terms and counts as
// 16-bit numbers; and the embeddings in binary32.
bb_write_index :: (w: [] BbW, s: BbStore, encoder: [32] u8) {
  made := encoder; corpus := bb_corpus(s);
  bb_text(w, BB_INDEX_MAGIC); bb_bytes(w, made[..]); bb_bytes(w, corpus[..]);
  bb_u32(w, s.file_n);
  for f: 0..<s.file_n { bb_str(w, bb_path(s, f)); bb_bytes(w, s.hashes[32 * f..<32 * f + 32]); bb_u32(w, s.status[f]); bb_u32(w, s.book[f]); }
  bb_u32(w, s.heads_n); bb_bytes(w, s.heads[0..<s.heads_n]);
  bb_u32(w, s.passage_n);
  for p: 0..<s.passage_n {
    q := s.passages[p];
    bb_u32(w, q.file); bb_u32(w, q.head_from); bb_u32(w, q.head_to); bb_u32(w, q.part);
    bb_u32(w, s.length[p]); bb_u32(w, s.tf[p]); bb_u32(w, s.tt[p]); bb_bytes(w, q.hash[..]);
  }
  bb_u32(w, s.words);
  for e: 0..<s.words { bb_u32(w, s.df[e]); }
  bb_f64(w, s.average);
  bb_u32(w, s.terms_n);
  for i: 0..<s.terms_n { bb_uint(w, cast(u64) s.terms[i], 2); }
  for i: 0..<s.terms_n { bb_uint(w, cast(u64) s.counts[i], 2); }
  for i: 0..<s.passage_n * BE_WIDTH { bb_f32(w, s.embeddings[i]); }
}
```

Loading hashes the three files, parses the encoder and, when the encoder
names this profile, the index into the caller's storage, checking every
count and span against that storage, and asks `bb_admit`:

<!-- source: psionic/lib/book-search/browser.coax load -->
```jai
// Hashes and parses the three downloaded files into s and decides whether
// the page may answer from them. Nothing is searched unless this says
// admitted.
bb_load :: (manifest: [] u8, encoder: [] u8, index: [] u8, s: [] BbStore) -> BbVerdict {
  m := bb_read_manifest(cast(string) manifest);
  seen := BbSeen.{encoder_bytes = encoder.count, index_bytes = index.count};
  seen.encoder = bs_file_hash(encoder, 0, encoder.count);
  seen.index = bs_file_hash(index, 0, index.count);
  seen.profile = bb_read_encoder(encoder, s);
  if seen.profile == BB_PROFILE {
    h := bb_read_index(index, s);
    seen.index_ok = h.ok; seen.index_encoder = h.encoder; seen.index_corpus = h.corpus;
    if h.ok { seen.listed_corpus = bb_corpus(s[0]); }
  }
  return bb_admit(m, seen);
}
```

## The search

The search is chapter 54's, over the bundle instead of the books: the
query's known words, its embedding, BM25's top 100, exhaustive vector
search's top 100, and reciprocal-rank fusion.

<!-- source: psionic/lib/book-search/browser.coax search -->
```jai
// Chapter 54's search over the bundle: the query's known words, its
// embedding, BM25's top 100, exhaustive vector search's top 100, and their
// reciprocal-rank fusion. scores holds one number per passage.
bb_search :: (s: BbStore, text: string, scores: [] f64, out: [] BbResult) {
  table := PT_TOY; t := pt_layout(table[..]); word: [64] u8;
  ids: [4096] int; n := 0; words := 0; at := 0;
  while at < text.count && n < ids.count {
    w := bs_word(table[..], t, text, at, word[..]);
    if w.size > 0 { words += 1; id := bs_find(s.vocab[0], cast(string) word[0..<w.size]); if id >= 0 { ids[n] = id; n += 1; } }
    at = w.next;
  }
  out[0].words = words; out[0].known = n;
  be_embed(s.table, s.query, ids[0..<n], out[0].e[..]);
  distinct: [4096] int; for i: 0..<n { distinct[i] = ids[i]; }
  k := bs_distinct(distinct[0..<n]);
  P := s.passage_n;
  for p: 0..<P { scores[p] = bs_passage_bm25(distinct[0..<k], s.terms[s.tf[p]..<s.tt[p]], s.counts[s.tf[p]..<s.tt[p]], s.df, P, s.length[p], s.average); }
  out[0].ln = bs_top(scores[0..<P], 0, out[0].lexical[..]);
  for p: 0..<P { scores[p] = ifx n == 0 then -2.0 else bs_dot(out[0].e[..], s.embeddings[p * BE_WIDTH..<(p + 1) * BE_WIDTH]); }
  out[0].dn = bs_top(scores[0..<P], -2, out[0].dense[..]);
  bs_fuse(out[0].lexical[0..<out[0].ln], out[0].dense[0..<out[0].dn], scores[0..<P]);
  out[0].fn = bs_top(scores[0..<P], 0, out[0].fused[..]);
}
```

To compare two runs exactly, each result is also written as one line of bit
patterns: the 64 binary32 values of the query embedding, then the first ten
passages of each ranking with their binary64 scores in hex.

<!-- source: psionic/lib/book-search/browser.coax line -->
```jai
// One result as exact bits: known/words, the 64 binary32 embedding values,
// then the first ten of each ranking as passage:score (binary64 bits).
bb_hits :: (w: [] BbW, name: string, hits: [] BsHit) {
  bb_byte(w, ' '); bb_text(w, name); bb_byte(w, '=');
  for i: 0..<hits.count {
    if i < BB_SHOW {
      if i > 0 { bb_byte(w, ','); }
      bb_decimal(w, hits[i].passage); bb_byte(w, ':'); bb_hex_bits(w, f64_bits(hits[i].score), 16);
    }
  }
}
bb_line :: (w: [] BbW, e: [] f32, words: int, known: int, bm: [] BsHit, dn: [] BsHit, fu: [] BsHit) {
  bb_decimal(w, known); bb_byte(w, '/'); bb_decimal(w, words); bb_text(w, " e=");
  for d: 0..<e.count { bb_hex_bits(w, cast(u64) f32_bits(e[d]), 8); }
  bb_hits(w, "bm25", bm); bb_hits(w, "dense", dn); bb_hits(w, "fused", fu);
  bb_byte(w, '\n');
}
```

## A hand-checked example

A small book makes every number checkable by hand. Its encoder is set, not
trained: word number e has the unit vector on axis e as its row, and the
query vector is zero.

<!-- source: psionic/lib/book-search/browser.coax small -->
```jai
// Indexes the chapters with a hand-set encoder: word e's row is the unit
// vector on axis e and the query vector is zero, so attention is uniform and
// a passage's embedding is its word counts scaled to length one. At most 64
// distinct words.
bb_small_books :: (b: [] BbSmall, paths: [] string, chapters: [] string) -> bool {
  bb_small(b);
  from: [8] int; to: [8] int; at := 0; name := 0;
  if paths.count > from.count { return false; }
  for f: 0..<paths.count {
    b[0].path_from[f] = name; for c: paths[f] { b[0].names[name] = c; name += 1; } b[0].path_to[f] = name;
    from[f] = at; for c: chapters[f] { b[0].pool[at] = c; at += 1; } to[f] = at;
  }
  if !bb_index_books(b[0].pool[..], from[0..<paths.count], to[0..<paths.count], b[0].store[..], b[0].ids[..]) { return false; }
  for e: 0..<BE_WIDTH {
    for d: 0..<BE_WIDTH { b[0].table[e * BE_WIDTH + d] = ifx e == d then 1 else 0; }
    b[0].query[e] = 0;
  }
  bb_embed_all(b[0].store[..], b[0].ids[..]);
  return true;
}
```

Work out the query "squared error" before running it.

- With a zero query vector every attention score is 0, so the softmax gives
  each known word weight 1/2.
- The sum is 1/2 on the axis of "squared" and 1/2 on the axis of "error".
  Its length is √(1/4 + 1/4) = 1/√2, so the unit vector has 1/√2 ≈
  0.70710678 on both axes. In binary32 that is `0x3f3504f3`.
- The passage "1. Learning a number > Measure the error" is its heading
  path and its text, "The loss is the mean squared error of the
  predictions." Its words are "the" 4 times, "error" twice, "squared" once
  and ten other words once each. Its embedding is those counts divided by
  their length √(16 + 4 + 1 + 10) = √31.
- The cosine with the query is (1 + 2) / (√2 · √31) = 3/√62 ≈ 0.381000.

[browser-search.coax](../examples/browser-search.coax) builds the book,
writes the three files, loads them into a second store as the page does, and
searches both:

<!-- source: psionic/examples/browser-search.coax example -->
```jai
main :: () {
  paths := [2] string.["psionic/book/ch001-learning-a-number.md", "psionic/book/ch054-search-the-book.md"];
  chapters := [2] string.[
    "# 1. Learning a number\n\nTrain one weight by gradient descent.\n\n## Measure the error\n\nThe loss is the mean squared error of the predictions.\n",
    "# 54. Search the book\n\n> **Status: unimplemented — design draft.** Nothing here runs yet.\n\n## Rank passages\n\nRank passages by BM25, then by squared error of embeddings.\n"];
  native: [1] BbSmall;
  if !bb_small_books(native[..], paths[..], chapters[..]) { print("the book does not fit\n"); return; }
  s := native[0].store[0];
  print("native index: % files, % passages, % words\n", s.file_n, s.passage_n, s.words);

  // The three files the site serves.
  enc: [32768] u8; idx: [32768] u8; man: [512] u8;
  we := [1] BbW.[BbW.{buf = enc[..]}]; wi := [1] BbW.[BbW.{buf = idx[..]}]; wm := [1] BbW.[BbW.{buf = man[..]}];
  bb_write_encoder(we[..], s, "");
  encoder := bs_file_hash(enc[..], 0, we[0].n);
  bb_write_index(wi[..], s, encoder);
  m := BbManifest.{ok = true, profile = BB_PROFILE, encoder = encoder, encoder_bytes = we[0].n, index_bytes = wi[0].n, corpus = bb_corpus(s)};
  m.index = bs_file_hash(idx[..], 0, wi[0].n);
  bb_write_manifest(wm[..], m);
  hex: [64] u8;
  print("encoder.bin % bytes, sha256 %\n", we[0].n, bs_hex(m.encoder, 8, hex[..]));
  print("index.bin   % bytes, sha256 %\n", wi[0].n, bs_hex(m.index, 8, hex[..]));
  print("corpus      sha256 %\n", bs_hex(m.corpus, 8, hex[..]));

  // The page: hash, parse and admit what it downloaded, then search it.
  page: [1] BbSmall; bb_small(page[..]);
  v := bb_load(man[0..<wm[0].n], enc[0..<we[0].n], idx[0..<wi[0].n], page[0].store[..]);
  print("page: %\n", bb_verdict_text(v));
  query := "squared error";
  scores: [32] f64; r: [1] BbResult; mine: [1] BbResult;
  bb_search(page[0].store[0], query, scores[..], r[..]);
  bb_search(s, query, scores[..], mine[..]);
  a := bs_find(page[0].vocab[0], "squared"); b := bs_find(page[0].vocab[0], "error");
  print("query \"%\": % of % words known; e[%] = e[%] = 0x%\n", query, r[0].known, r[0].words, a, b, cast(string) bb_bits32(r[0].e[a], hex[..]));
  for i: 0..<r[0].fn {
    h := r[0].fused[i]; p := h.passage;
    f := page[0].passages[p].file;
    status := bs_section_status(cast(BsStatus) page[0].status[f], bb_heads(page[0].store[0], p));
    print("%. % > % [%]\n   bm25 rank %, dense rank %\n", i + 1, bb_path(page[0].store[0], f), bb_heads(page[0].store[0], p), bs_status_label(status),
          bb_rank(r[0].lexical[0..<r[0].ln], p), bb_rank(r[0].dense[0..<r[0].dn], p));
  }
  print("dense score of the best passage: %\n", r[0].dense[0].score);
  l1: [4096] u8; l2: [4096] u8;
  w1 := [1] BbW.[BbW.{buf = l1[..]}]; w2 := [1] BbW.[BbW.{buf = l2[..]}];
  bb_result_line(w1[..], r[..]); bb_result_line(w2[..], mine[..]);
  print("result line % bytes; page and native lines equal: %\n", w1[0].n, cast(string) l1[0..<w1[0].n] == cast(string) l2[0..<w2[0].n]);
}
```

```text
native index: 2 files, 4 passages, 36 words
encoder.bin 10042 bytes, sha256 a9c4630b5eabea06
index.bin   2006 bytes, sha256 d3589ef39fa00822
corpus      sha256 505937e484281adb
page: admitted
query "squared error": 2 of 2 words known; e[16] = e[12] = 0x3f3504f3
1. psionic/book/ch001-learning-a-number.md > 1. Learning a number > Measure the error [implemented]
   bm25 rank 1, dense rank 1
2. psionic/book/ch054-search-the-book.md > 54. Search the book > Rank passages [design draft, not implemented]
   bm25 rank 2, dense rank 2
3. psionic/book/ch001-learning-a-number.md > 1. Learning a number [implemented]
   bm25 rank 0, dense rank 3
4. psionic/book/ch054-search-the-book.md > 54. Search the book [design draft, not implemented]
   bm25 rank 0, dense rank 4
dense score of the best passage: 0.3810003812005398
result line 726 bytes; page and native lines equal: true
```

Both axes hold `0x3f3504f3`, and the best passage's dense score is 0.381000,
as worked out above; it differs from 3/√62 in the seventh digit because the
stored embedding is binary32. The draft chapter's passage is labelled as not
shipped, as in chapter 54. The last line compares the result line from the
loaded bundle with the one from the store it was written from: equal.

## Refusals

[browser-search-refuse.coax](../examples/browser-search-refuse.coax) builds
the same book twice, before and after chapter 54's text changed, and hands
the page bundles that do not match their manifest:

<!-- source: psionic/examples/browser-search-refuse.coax refuse -->
```jai
main :: () {
  paths := [2] string.["psionic/book/ch001-learning-a-number.md", "psionic/book/ch054-search-the-book.md"];
  before := [2] string.["# 1. Learning a number\n\nThe loss is the mean squared error.\n", "# 54. Search the book\n\n> **Status: unimplemented — design draft.** Nothing here runs yet.\n"];
  now := [2] string.["# 1. Learning a number\n\nThe loss is the mean squared error.\n", "# 54. Search the book\n\nRank passages by BM25 and by squared error of embeddings.\n"];
  old: [1] BbSmall; cur: [1] BbSmall;
  if !bb_small_books(old[..], paths[..], before[..]) || !bb_small_books(cur[..], paths[..], now[..]) { print("the book does not fit\n"); return; }
  current := bb_corpus(cur[0].store[0]);
  good: [1] Served; serve(cur[..], current, good[..]);
  attempt("current bundle", good[0].man[0..<good[0].mn], good[0].enc[0..<good[0].en], good[0].idx[0..<good[0].xn]);

  // An index and encoder built before chapter 54 changed, under today's manifest.
  stale: [1] Served; serve(old[..], current, stale[..]);
  attempt("index built from the old books", stale[0].man[0..<stale[0].mn], stale[0].enc[0..<stale[0].en], stale[0].idx[0..<stale[0].xn]);

  // One number of the query vector differs; the manifest names the real encoder.
  enc := good[0].enc; enc[good[0].en - 1] ^= 1;
  attempt("encoder with one changed byte", good[0].man[0..<good[0].mn], enc[0..<good[0].en], good[0].idx[0..<good[0].xn]);

  // The same changed encoder with a manifest that names it: the sizes and the
  // 64 numbers per word match, but the index was embedded by the other one.
  other: [1] BbSmall; ok := bb_small_books(other[..], paths[..], now[..]); other[0].query[3] = 0.5;
  mixed: [1] Served; serve(other[..], current, mixed[..]);
  mb: [512] u8; wm := [1] BbW.[BbW.{buf = mb[..]}];
  m := bb_read_manifest(cast(string) mixed[0].man[0..<mixed[0].mn]);
  m.index = bs_file_hash(good[0].idx[..], 0, good[0].xn); m.index_bytes = good[0].xn;
  bb_write_manifest(wm[..], m);
  attempt("another encoder beside this index", mb[0..<wm[0].n], mixed[0].enc[0..<mixed[0].en], good[0].idx[0..<good[0].xn]);

  // A download cut short, under a manifest that names the cut bytes.
  cut := good[0].xn - 100;
  m = bb_read_manifest(cast(string) good[0].man[0..<good[0].mn]);
  m.index = bs_file_hash(good[0].idx[..], 0, cut); m.index_bytes = cut;
  mc: [512] u8; wc := [1] BbW.[BbW.{buf = mc[..]}]; bb_write_manifest(wc[..], m);
  attempt("index cut short", mc[0..<wc[0].n], good[0].enc[0..<good[0].en], good[0].idx[0..<cut]);
  attempt("index cut short, old manifest", good[0].man[0..<good[0].mn], good[0].enc[0..<good[0].en], good[0].idx[0..<cut]);
}
```

```text
current bundle: admitted
index built from the old books: refused: stale index: it holds other books than the manifest's current corpus
encoder with one changed byte: refused: wrong encoder: its bytes are not the encoder the manifest names
another encoder beside this index: refused: the index's embeddings were made by another encoder
index cut short: refused: the index does not parse or its corpus digest does not match its files
index cut short, old manifest: refused: the index bytes are not the index the manifest names
```

Each case names its own refusal, and none of them searches. The exit status
is 0: refusing is the expected result.

## Run it in a browser

The Wasm module is built from `search.coax`, which holds the page's state in
one struct (Coax has no heap), and `face.c`, which exports scalar procedures
and the addresses of the download buffers. `index.html` fetches the module
and the three files, copies the bytes into the module's memory, calls
`search_load`, and shows the verdict. Typing a query calls `search_query`.

The check runs it without a visible browser:

```shell
psionic/web/book-search/check.sh
```

It does this:

1. Runs chapter 54's tool with `--bundle coax/web/build/book-search-web/bundle
   --reference psionic/fixtures/book-search/heldout.tsv`. The tool loads its
   index (rebuilding it if the books changed), writes the three files, and
   writes one native result line per held-out query, from chapter 54's own
   `search` procedure, to `reference.txt`.
2. Compiles the module with `coax/web/wasm.sh` (clang for `wasm32-wasip1`,
   `-O2`, no fast math, no floating-point contraction).
3. Makes three bundles the page must refuse: the encoder with its last byte
   changed (`wrong-encoder`); that changed encoder with a manifest naming it,
   beside the real index (`other-encoder`); and the real encoder and index
   with a manifest whose corpus digest names other books (`stale`).
4. Serves the directory with `coax/web/serve.sh --static` on 127.0.0.1.
5. Loads the page in headless, muted Chrome (`coax/web/tool.sh shot`) once
   per bundle, with `?queries=heldout.tsv`, and reads the page's report.
6. Compares the browser's 36 result lines with the native ones byte for
   byte, and writes [receipt.json](../fixtures/browser-search/receipt.json).

The run for this chapter, on Linux x86-64 in HeadlessChrome 152, on a busy
machine (load average near 30), with this chapter's text in the working
tree on top of revision `331ac8a5a616`:

| Measure | Value |
| --- | --- |
| Result lines equal to native, 36 queries | yes: both files have SHA-256 `c7b10c555b4f2fe1…` |
| Tolerance | none: every bit of each embedding and each listed score |
| Download: module, manifest, encoder, index | 100,085 + 288 + 2,879,613 + 5,592,998 = 8,572,984 bytes |
| Download with gzip -9 (measured on the files, not served) | 40,157 + 234 + 2,575,279 + 3,224,729 = 5,840,399 bytes |
| Fetch, instantiate, hash and admit, ready (localhost) | 62 ms, 28 ms, 120 ms; ready after 210 ms |
| 36 queries over 7,731 passages | 154 ms in all |
| Wasm memory | 64 MiB, the same after the queries (no growth) |
| Refused bundles | wrong encoder, another encoder, stale index |

There is no tolerance because none is needed here: the native tool and the
Wasm module compute the same binary64 and binary32 operations in the same
order. The square root is a Wasm instruction, and the `exp` and `log` of
wasi-libc and of glibc returned the same bits on every value these queries
reached. If a future libm differs, the comparison fails and the chapter must
state a measured tolerance instead of passing.

The memory is fixed: Coax has no heap, so the page asks for 64 MiB up front
and the module never grows it. The state takes 35.6 MB of it and the
download buffers most of the rest.

Retrieval quality in the browser is chapter 54's, because the rankings are
the same. Recall@k for k up to 10 and MRR@10 depend only on the first ten
passages of each ranking, and those are equal bit for bit. Chapter 54's tool
measures them on the same books:

```shell
coax/web/tool.sh psionic/tools/book-search.coax --eval psionic/fixtures/book-search/heldout.tsv
```

```text
36 queries over 7731 passages and 10633 words, books at 331ac8a5a616, weights 66a9e806e7b80477
| Retriever | Recall@1 | Recall@5 | Recall@10 | MRR@10 |
| --- | ---: | ---: | ---: | ---: |
| BM25 | 0.333 | 0.556 | 0.694 | 0.450 |
| Exhaustive vector search | 0.361 | 0.639 | 0.750 | 0.465 |
| Psionic retriever (fused) | 0.361 | 0.611 | 0.806 | 0.465 |
```

These figures differ from chapter 54's table because the books have grown
since that chapter was measured (4,956 passages then); both measure the same
procedures.

## Checks

```shell
psionic/check.sh --only book-search
```

[check_book_search.sh](../tests/check_book_search.sh) runs the checks of
`bundle.coax` and `browser.coax` with chapters 54 to 57's, in debug and
release builds, runs both examples in both builds against their recorded
output, and mutates `bundle.coax` and `browser.coax`. The checks cover
every refusal and their order, the manifest's exact format, little-endian
reads and range checks, and a bundle written, read back and searched: its
embedding and every BM25 and dense score are compared bit for bit with
chapter 54's `be_embed`, `bs_dot` and `bs_passage_bm25` on the original
store, and with the hand values above. The readers are also fed storage cut
to exactly what the small book needs, which they accept, and one entry
smaller in each count, which they refuse; files one byte short or one byte
long; and every index field (status, book, heading and term spans, the
average length) at its last allowed value and one past it. The small-book
builder is checked at exactly eight files, a full pool and name buffer, and
one passage, word, word id or term too many. `coax mutate` kills all 50
mutants of `bundle.coax`, the `#authority` file, and all 963 of
`browser.coax` (`--root-only`); none survive.

## What this does not do yet

- **Production.** The page runs on a local `coax serve`. Putting it on
  openagents.com is a deploy the owner runs; it is listed in
  [NEEDS_OWNER.md](../../NEEDS_OWNER.md).
- **A smaller query encoder.** The browser downloads the full encoder, 2.9 MB.
  A distilled encoder would need a measured alignment with the stored
  document embeddings before the page could accept it.
- **WebGPU compute.** Everything runs on the CPU in Wasm. Coax has no
  qualified WebGPU compute lowering yet
  ([chapter 37](ch037-a-coax-compute-subset.md)).
- **Compression.** The development server sends the files uncompressed;
  gzip -9 would shrink the download from 8.6 MB to 5.8 MB.
- **Other browsers.** Only headless Chrome on Linux was measured.
- **Passage text.** The index holds heading paths, not text; results show
  where to read, not a snippet.

## Sources

- [Chapter plan](PLAN.md), Part VI and the Coax feature map.
- [Coaxial](../../coax/web.md): the browser host, the Coax-to-Wasm build and
  `coax serve`.
- WebAssembly Core Specification 2.0: floating-point instructions are IEEE
  754 operations with round-to-nearest, including `f64.sqrt`.
