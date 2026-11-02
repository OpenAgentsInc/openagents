# 57. Answer from current sources

> **Status: partly implemented.** Extractive answers with citations,
> abstention, the freshness check on every citation, and a held-out
> measurement run on the CPU over chapter 54's books. Generated answers,
> answers that state a feature's status or a target's evidence, and
> chapter 205's prompt-injection attacks are not built yet.

This chapter builds a documentation assistant over the Coax, Psionic and
Aethyr books. It takes a question, retrieves passages with
[chapter 54](ch054-search-the-coax-book.md)'s retriever, reranks them with
[chapter 56](ch056-rerank-the-candidates.md)'s reranker, and then does one
of two things:

- **Answer** with one sentence copied byte for byte from a retrieved
  passage, with a citation that names the passage and its revision.
- **Abstain**, and show the closest sentence it found, when no sentence
  holds enough of the question.

It never writes new text. An extractive answer can be checked mechanically:
the quoted bytes either are or aren't in the cited passage. A generated
answer would need a separate check that each claim follows from the
evidence, and this chapter doesn't build one.

A saved answer is checked again before it is served. If the cited passage
has changed since the answer was made, or no longer exists, the answer is
refused instead of served from the old text.

The library is [answer.coax](../lib/book-search/answer.coax). It is
`#authority`: it decides whether an answer may be served. The measurement
runs in the book-search tool, [book-search.coax](../tools/book-search.coax),
which loads it beside chapters 54 to 56's files.

## Run it

Run commands from the repository root.

```shell
coax/bin/coax run psionic/examples/answer.coax
coax/bin/coax run psionic/examples/answer-refuse.coax
coax/web/tool.sh psionic/tools/book-search.coax --answer-eval psionic/fixtures/book-search/heldout.tsv
coax/web/tool.sh psionic/tools/book-search.coax --answer-eval psionic/fixtures/book-search/unanswerable.tsv
psionic/check.sh --only book-search
```

## Words

The answerer compares the question with sentences word by word. A word is
a run of ASCII letters and digits and of bytes at or above 128, so `naïve`
is one word and `x_y` is two. ASCII letters compare without case.

<!-- source: psionic/lib/book-search/answer.coax words -->
```jai
// A word is a run of ASCII letters and digits and of bytes at or above 128.
// Words compare with ASCII letters folded to lower case.
an_word_byte :: (c: u8) -> bool { return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c >= 128; }
an_lower :: (c: u8) -> u8 { if c >= 'A' && c <= 'Z' { return c + 32; } return c; }
// The first word that starts at or after at; from == to == text.count when
// none is left.
an_next_word :: (text: string, at: int) -> AnSpan {
  a := at;
  while a < text.count && !an_word_byte(text[a]) { a += 1; }
  b := a;
  while b < text.count && an_word_byte(text[b]) { b += 1; }
  return AnSpan.{from = a, to = b};
}
// Does text[from..<to] hold word (lower case) as a whole word?
an_holds :: (text: string, from: int, to: int, word: string) -> bool {
  at := from;
  while at < to {
    w := an_next_word(text[0..<to], at);
    if an_same(text, w.from, w.to, word) { return true; }
    at = w.to;
  }
  return false;
}
an_same :: (text: string, from: int, to: int, word: string) -> bool {
  if to - from != word.count { return false; }
  for i: 0..<word.count { if an_lower(text[from + i]) != word[i] { return false; } }
  return true;
}
```

## Support

Each distinct question word gets a weight. The tool and the example use
BM25's IDF from chapter 54, `ln(1 + (N - df + 0.5) / (df + 0.5))` over `N`
passages, where `df` counts the passages that hold the word. A word no
passage holds gets the largest weight, so a question about something the
books never mention can't be answered by matching its small words.

A sentence's **support** is the share of the question's total weight that
the sentence holds. The answer is the sentence with the most support among
the top three reranked passages.

<!-- source: psionic/lib/book-search/answer.coax support -->
```jai
// The share of the query's weight that text[from..<to] holds. words: the
// query's distinct words; weight: one per word (the tool and the example
// use BM25's IDF, so a word the books lack weighs the most). 0 when the
// query weighs nothing.
an_support :: (words: [] string, weight: [] f64, text: string, from: int, to: int) -> f64 {
  total := 0.0; held := 0.0;
  for w, i: words {
    total += weight[i];
    if an_holds(text, from, to, w) { held += weight[i]; }
  }
  if total <= 0 { return 0; }
  return held / total;
}
// A sentence ends just after '.', '?' or '!' when white space or the end of
// the text follows, before a blank line, or at the end of the text.
an_space :: (c: u8) -> bool { return c == ' ' || c == '\n' || c == '\t' || c == '\r'; }
an_sentence_end :: (text: string, from: int) -> int {
  for i: from..<text.count {
    c := text[i];
    if (c == '.' || c == '?' || c == '!') && (i + 1 == text.count || an_space(text[i + 1])) { return i + 1; }
    if c == '\n' && i + 1 < text.count && text[i + 1] == '\n' { return i; }
  }
  return text.count;
}
// The sentence of text with the most support; the earlier one on a tie.
an_best_span :: (words: [] string, weight: [] f64, text: string) -> AnSpan {
  best := AnSpan.{support = -1};
  at := 0;
  while at < text.count {
    if an_space(text[at]) { at += 1; continue; }
    end := an_sentence_end(text, at);
    s := an_support(words, weight, text, at, end);
    if s > best.support { best = AnSpan.{from = at, to = end, support = s}; }
    at = end;
  }
  return best;
}
```

<!-- source: psionic/lib/book-search/answer.coax answer -->
```jai
// passages: the retrieved texts, best first; the caller passes at most
// AN_TOP. The answer is the best sentence over them, from the better-ranked
// passage on a tie. It is given only when its support reaches least and is
// above zero; otherwise the assistant abstains and keeps the span it found.
an_answer :: (words: [] string, weight: [] f64, passages: [] string, least: f64) -> AnAnswer {
  r := AnAnswer.{verdict = AnVerdict.abstain, rank = -1, support = -1};
  for text, k: passages {
    s := an_best_span(words, weight, text);
    if s.support > r.support { r = AnAnswer.{verdict = AnVerdict.abstain, rank = k, from = s.from, to = s.to, support = s.support}; }
  }
  if r.rank >= 0 && r.support >= least && r.support > 0 { r.verdict = AnVerdict.answer; }
  return r;
}
```

Two settings were fixed before the assistant ran on any held-out question,
and nothing was changed after:

- The support floor `AN_SUPPORT_MIN` is 0.5. The assistant answers only
  when the best sentence holds at least half the question's weight.
- The answer comes from the top `AN_TOP` = 3 reranked passages.

## Citations and revisions

A citation names a passage by its **identity**: its file, its heading path
and its part number, joined by `#`. The identity stays the same when the
passage's text changes. The citation also records the passage's
**revision digest** when the answer was made: SHA-256 of the heading path, a
newline and the text, the same hash chapter 54 stores for every passage.
Any edit to the heading or the text changes the digest.

Before a saved answer is served, `an_check` looks for the identity among
the passages as they are now:

- No passage has the identity: **deleted**.
- The passage's digest differs from the cited one: **changed**.
- The cited bytes aren't exactly the quoted answer: **span**.
- Otherwise the citation is **current**, and only then is the answer served.

<!-- source: psionic/lib/book-search/answer.coax cite -->
```jai
// A passage's revision digest is chapter 54's passage hash: SHA-256 of its
// heading path, a newline and its text. Any edit to either changes it.
an_digest :: (heading: string, text: string) -> [32] u8 {
  state: [112] u8; out: [32] u8; nl := [1] u8.['\n'];
  r := sha256_init(state[..]);
  an_feed(state[..], heading);
  r = sha256_update_checked(state[..], nl[..], false);
  an_feed(state[..], text);
  r = sha256_final_checked(state[..], out[..], false);
  return out;
}
// A passage's identity stays the same across revisions: its file, heading
// path and part, joined by '#'. "" when out is too small.
an_id :: (path: string, heading: string, part: int, out: [] u8) -> string {
  digits: [20] u8; d := 0; v := part;
  while d == 0 || v > 0 { digits[d] = cast(u8) ('0' + v % 10); v /= 10; d += 1; }
  n := path.count + heading.count + 2 + d;
  if n > out.count { return ""; }
  at := an_put(out, 0, path); out[at] = '#'; at = an_put(out, at + 1, heading); out[at] = '#'; at += 1;
  while d > 0 { d -= 1; out[at] = digits[d]; at += 1; }
  return cast(string) out[0..<n];
}
// Checks a citation against the passages as they are now (ids, heading paths
// and texts, one each). The answer may be served only when the check is
// current: the passage exists, its digest is the cited one, and the cited
// bytes are exactly the quoted answer.
an_check :: (c: AnCite, quote: string, ids: [] string, heads: [] string, texts: [] string) -> AnCheck {
  for id, i: ids {
    if id != c.id { continue; }
    if !an_same_digest(an_digest(heads[i], texts[i]), c.digest) { return AnCheck.changed; }
    if c.from < 0 || c.from > c.to || c.to > texts[i].count || texts[i][c.from..<c.to] != quote { return AnCheck.span; }
    return AnCheck.current;
  }
  return AnCheck.deleted;
}
an_serve :: (c: AnCheck) -> bool { return c == AnCheck.current; }
```

A changed passage is refused even when the quoted sentence itself is
unchanged. The digest covers the whole passage, so a correction elsewhere
in it, which may change what the sentence means, still forces the question
to be asked again.

## A small example

[answer.coax](../examples/answer.coax) holds a store of four passages from
three chapters, and a second version of it in which one passage is
corrected and one chapter is deleted.

<!-- source: psionic/examples/answer.coax store -->
```jai
// Version 1: four passages from three chapters.
V1_PATH :: [4] string.["psionic/book/ch054.md", "psionic/book/ch054.md", "coax/book/ch06.md", "psionic/book/ch056.md"];
V1_HEAD :: [4] string.["54. Search > Ranking", "54. Search > Freshness", "6. Memory > Bounds checks", "56. Rerank > Training"];
V1_TEXT :: [4] string.[
  "Each passage is scored with BM25. The settings are k1 = 1.2 and b = 0.75. Fusion adds 1/(60 + rank) for each list.",
  "An index is fresh when every chapter it holds is unchanged. A stale index is refused with exit status 3.",
  "Every index is checked against the length. An index out of range stops the program with a bounds error.",
  "The reranker trains for 300 steps with learning rate 0.5. Training starts from zero weights."];
// Version 2: the ranking passage is corrected and chapter 6 is deleted.
V2_PATH :: [3] string.["psionic/book/ch054.md", "psionic/book/ch054.md", "psionic/book/ch056.md"];
V2_HEAD :: [3] string.["54. Search > Ranking", "54. Search > Freshness", "56. Rerank > Training"];
V2_TEXT :: [3] string.[
  "Each passage is scored with BM25. The settings are k1 = 0.9 and b = 0.4. Fusion adds 1/(60 + rank) for each list.",
  "An index is fresh when every chapter it holds is unchanged. A stale index is refused with exit status 3.",
  "The reranker trains for 300 steps with learning rate 0.5. Training starts from zero weights."];
```

### A trace by hand

Take the question `BM25 settings k1 and b` against version 1. Its five
words each occur in one passage of four, the ranking passage, so each
weighs `ln(1 + 3.5 / 1.5) = ln(10/3) = 1.2040`. That passage has three
sentences:

| Sentence | Question words held | Support |
| --- | --- | ---: |
| `Each passage is scored with BM25.` | bm25 | 1/5 = 0.2 |
| `The settings are k1 = 1.2 and b = 0.75.` | settings, k1, and, b | 4/5 = 0.8 |
| `Fusion adds 1/(60 + rank) for each list.` | none | 0 |

The second sentence wins with support 0.8, above the floor of 0.5. The
first sentence is 33 bytes and a space follows it, so the answer starts at
byte 34 and its 39 bytes end at byte 73.

For `an index out of range`, `an` and `index` occur in two passages
(weight `ln(1 + 2.5 / 2.5) = ln 2 = 0.6931`) and `out`, `of` and `range` in
one (1.2040). The total is `2 × 0.6931 + 3 × 1.2040 = 4.9982`. The bounds
sentence holds all five words, so its support is 1. The freshness
passage's first sentence holds only `an` and `index`: `1.3863 / 4.9982 =
0.2774`.

For `stale weights in the reranker`, `in` occurs in no passage (weight
`ln(1 + 4.5 / 0.5) = ln 10 = 2.3026`) and `the` in three
(`ln(1 + 1.5 / 3.5) = 0.3567`); the other three words weigh 1.2040. The
total is 6.2712. The best sentence, `The reranker trains for 300 steps with
learning rate 0.5.`, holds `the` and `reranker`: `1.5607 / 6.2712 =
0.2489`. That is under 0.5, so the assistant abstains.

### The program

The example retrieves with BM25 over the store, using chapter 54's
`bs_bm25` and `bs_top`, and answers from the top three. (The tool below
adds chapter 56's reranker between the two steps; four passages need no
reranking.) A saved answer is served only after `an_check`.

<!-- source: psionic/examples/answer.coax ask -->
```jai
// Retrieve with BM25 (chapter 54's formula, words from answer.coax), keep the
// top AN_TOP, and answer with the best-supported sentence or abstain.
ask :: (question: string, path: [] string, head: [] string, text: [] string, ids: [] string) -> Asked {
  N := text.count; pool: [256] u8; words: [16] string; weight: [16] f64; df: [16] int;
  n := an_query_words(question, pool[..], words[..]);
  total := 0;
  for t: text { total += an_words(t); }
  average := f64(total) / f64(N);
  print("question: %\n  words:", question);
  for k: 0..<n {
    for t: text { if an_count(t, words[k]) > 0 { df[k] += 1; } }
    weight[k] = bs_idf(N, df[k]);
    print(" % (df %) ", words[k], df[k]); put4(weight[k]); if k + 1 < n { print(","); }
  }
  print("\n");
  scores: [8] f64; hits: [AN_TOP] BsHit;
  for t, p: text {
    for k: 0..<n { tf := an_count(t, words[k]); if tf > 0 { scores[p] += bs_bm25(tf, df[k], N, an_words(t), average); } }
  }
  m := bs_top(scores[0..<N], 0, hits[..]);
  found: [AN_TOP] string;
  for h, k: hits[0..<m] { found[k] = text[h.passage]; print("  retrieved %: % (BM25 ", k + 1, head[h.passage]); put4(h.score); print(")\n"); }
  a := an_answer(words[0..<n], weight[0..<n], found[0..<m], AN_SUPPORT_MIN);
  r := Asked.{answer = a};
  if a.rank < 0 { print("  abstain: no passage holds a query word\n\n"); return r; }
  p := hits[a.rank].passage;
  r.quote = text[p][a.from..<a.to];
  r.cite = AnCite.{id = ids[p], from = a.from, to = a.to, digest = an_digest(head[p], text[p])};
  if a.verdict == AnVerdict.abstain {
    print("  abstain: the best sentence holds "); put4(a.support); print(" of the query, under the floor "); put4(AN_SUPPORT_MIN);
    print("\n  closest: \"%\" (%)\n\n", r.quote, head[p]);
    return r;
  }
  hx: [64] u8;
  print("  answer (support "); put4(a.support); print("): \"%\"\n", r.quote);
  print("    cites % · % · % · part 0 · bytes %..<% · revision %\n\n", book_of(path[p]), path[p], head[p], a.from, a.to, an_hex(r.cite.digest, 6, hx[..]));
  return r;
}
```

<!-- source: psionic/examples/answer.coax serve -->
```jai
// A saved answer is served only while its citation is current.
serve :: (question: string, saved: Asked, head: [] string, text: [] string, ids: [] string) {
  c := an_check(saved.cite, saved.quote, ids, head, text);
  if an_serve(c) { print("serve \"%\": \"%\"\n", question, saved.quote); return; }
  print("serve \"%\": refused, %\n", question, an_check_name(c));
}
```

```text
store version 1: 4 passages from 3 chapters

question: BM25 settings k1 and b
  words: bm25 (df 1) 1.2040, settings (df 1) 1.2040, k1 (df 1) 1.2040, and (df 1) 1.2040, b (df 1) 1.2040
  retrieved 1: 54. Search > Ranking (BM25 5.5328)
  answer (support 0.8000): "The settings are k1 = 1.2 and b = 0.75."
    cites Psionic · psionic/book/ch054.md · 54. Search > Ranking · part 0 · bytes 34..<73 · revision 393fb299481f

question: an index out of range
  words: an (df 2) 0.6931, index (df 2) 0.6931, out (df 1) 1.2040, of (df 1) 1.2040, range (df 1) 1.2040
  retrieved 1: 6. Memory > Bounds checks (BM25 5.3364)
  retrieved 2: 54. Search > Freshness (BM25 1.6393)
  answer (support 1.0000): "An index out of range stops the program with a bounds error."
    cites Coax · coax/book/ch06.md · 6. Memory > Bounds checks · part 0 · bytes 43..<103 · revision 2bab3e225e05

question: stale weights in the reranker
  words: stale (df 1) 1.2040, weights (df 1) 1.2040, in (df 0) 2.3026, the (df 3) 0.3567, reranker (df 1) 1.2040
  retrieved 1: 56. Rerank > Training (BM25 2.9974)
  retrieved 2: 54. Search > Freshness (BM25 1.1978)
  retrieved 3: 6. Memory > Bounds checks (BM25 0.4957)
  abstain: the best sentence holds 0.2489 of the query, under the floor 0.5000
  closest: "The reranker trains for 300 steps with learning rate 0.5." (56. Rerank > Training)

serve "BM25 settings k1 and b": "The settings are k1 = 1.2 and b = 0.75."
serve "an index out of range": "An index out of range stops the program with a bounds error."

store version 2: the ranking passage now says k1 = 0.9 and b = 0.4; coax/book/ch06.md is deleted

serve "BM25 settings k1 and b": refused, changed: the cited passage has a new revision; ask again
serve "an index out of range": refused, deleted: no current passage has the cited identity

question: BM25 settings k1 and b
  words: bm25 (df 1) 0.9808, settings (df 1) 0.9808, k1 (df 1) 0.9808, and (df 1) 0.9808, b (df 1) 0.9808
  retrieved 1: 54. Search > Ranking (BM25 4.5332)
  answer (support 0.8000): "The settings are k1 = 0.9 and b = 0.4."
    cites Psionic · psionic/book/ch054.md · 54. Search > Ranking · part 0 · bytes 34..<72 · revision 1c08c98132a6

question: an index out of range
  words: an (df 1) 0.9808, index (df 1) 0.9808, out (df 0) 2.0794, of (df 0) 2.0794, range (df 0) 2.0794
  retrieved 1: 54. Search > Freshness (BM25 2.3295)
  abstain: the best sentence holds 0.2392 of the query, under the floor 0.5000
  closest: "An index is fresh when every chapter it holds is unchanged." (54. Search > Freshness)

serve "BM25 settings k1 and b": "The settings are k1 = 0.9 and b = 0.4."
```

The program prints the numbers worked out above: support 0.8 for the
settings sentence at bytes 34 to 73, 1 for the bounds sentence, and 0.2489
for the closest sentence about the reranker, which abstains.

In version 2 both saved answers are refused. The first cites the ranking
passage, whose text now says `k1 = 0.9`, so its digest changed. The second
cites chapter 6, which is gone. Asked again, the first question is answered
from the corrected sentence with a new digest. The second question now
abstains, and the deleted passage never appears among the results.

## Refusals

[answer-refuse.coax](../examples/answer-refuse.coax) checks one good
citation and then breaks it one way at a time.

<!-- source: psionic/examples/answer-refuse.coax refuse -->
```jai
main :: () {
  ids := [2] string.["psionic/book/ch054.md#54. Search > Ranking#0", "psionic/book/ch054.md#54. Search > Freshness#0"];
  heads := [2] string.["54. Search > Ranking", "54. Search > Freshness"];
  texts := [2] string.["Each passage is scored with BM25. The settings are k1 = 1.2 and b = 0.75.", "A stale index is refused with exit status 3."];
  good := AnCite.{id = ids[0], from = 34, to = 73, digest = an_digest(heads[0], texts[0])};
  quote := "The settings are k1 = 1.2 and b = 0.75.";
  print("the saved citation as made: %\n", an_check_name(an_check(good, quote, ids[..], heads[..], texts[..])));
  print("the same citation quoting k1 = 1.5: %\n", an_check_name(an_check(good, "The settings are k1 = 1.5 and b = 0.75.", ids[..], heads[..], texts[..])));
  past := good; past.to = 74;
  print("a span one byte past the passage: %\n", an_check_name(an_check(past, quote, ids[..], heads[..], texts[..])));
  edited := texts; edited[0] = "Each passage is scored with BM25. The settings are k1 = 1.2 and b = 0.75!";
  print("the passage's last byte edited: %\n", an_check_name(an_check(good, quote, ids[..], heads[..], edited[..])));
  moved := heads; moved[0] = "54. Search > Scoring";
  print("the passage's heading renamed: %\n", an_check_name(an_check(good, quote, ids[..], moved[..], texts[..])));
  print("the passage's chapter removed: %\n", an_check_name(an_check(good, quote, ids[1..<2], heads[1..<2], texts[1..<2])));
  pool: [8] u8; words: [2] string;
  print("a question of three words into room for two: %\n", an_query_words("stale index refused", pool[..], words[..]));
  print("a question of 9 letters into 8 bytes: %\n", an_query_words("freshness", pool[..], words[..]));
  out: [16] u8;
  print("an identity of 17 bytes into 16: \"%\"\n", an_id("psionic/a.md", "T", 10, out[..]));
}
```

```text
the saved citation as made: current
the same citation quoting k1 = 1.5: span: the cited bytes are not the quoted answer
a span one byte past the passage: span: the cited bytes are not the quoted answer
the passage's last byte edited: changed: the cited passage has a new revision; ask again
the passage's heading renamed: changed: the cited passage has a new revision; ask again
the passage's chapter removed: deleted: no current passage has the cited identity
a question of three words into room for two: -1
a question of 9 letters into 8 bytes: -1
an identity of 17 bytes into 16: ""
```

The program exits with status 0: every refusal is an expected answer. A
one-byte edit at the end of the passage, outside the quoted span, still
refuses the answer, because the digest covers the whole passage.

## Measure it

The tool's `--answer-eval` mode trains chapter 56's reranker (seed 56, the
same settings), then for each question retrieves BM25 and HNSW candidates,
reranks them and answers from the top three. Every answer's citation is
checked with `an_check` against the books as read from disk, separately
from the index the answer came from.

It reports three things separately:

- **Retrieval:** recall@1, recall@3 and MRR@10 of the reranked list
  against the labels, before any answer is chosen.
- **Abstention:** how many questions the assistant abstained on.
- **Citations:** how many citations were current against the books on
  disk, and **citation precision**: the share of answers whose cited
  passage is one the question's labels name. The quoted span is always
  inside the cited passage, by construction and by `an_check`; citation
  precision asks whether that passage is one that answers the question.

The questions are chapter 54's 36 held-out queries, every one labelled with
the sections that answer it, and eight questions written before the first
run that no passage answers
([unanswerable.tsv](../fixtures/book-search/unanswerable.tsv)).

```shell
coax/web/tool.sh psionic/tools/book-search.coax --answer-eval psionic/fixtures/book-search/heldout.tsv
```

```text
| Question | Verdict | Support | Cited passage | Labelled relevant | Citation now |
| --- | --- | ---: | --- | --- | --- |
| never leave half-written output after a failed request | answer | 0.608 | 19. Builds and hot reload > In the real world: the iOS app's build | no | current |
| multiplying dimensions could overflow the element count | abstain | 0.242 | 2. Shapes and storage > Count before multiplying | yes | - |
| negative strides and transposing without copying | abstain | 0.397 | 2. Shapes and storage > Checks and their limits | no | - |
| NFC normalization before byte pair encoding | answer | 0.775 | 9. Text becomes a model request > Unicode normalization changes the bytes before BPE | yes | current |
| is the model's confidence well calibrated on held out data | abstain | 0.423 | 223. Distill Elephant into a small router | no | - |
| measuring latency with a monotonic clock around the real work | answer | 0.590 | 130. Profile a training step > The step, phase by phase | no | current |
| split the data by session so the test set does not leak | abstain | 0.342 | 120. Data and resumable state > One split per group | no | - |
| how ties are broken when choosing which pair of bytes to merge | abstain | 0.322 | 53. Choosing the next token > Checks | no | - |
| AdamW bias correction and the step counter | answer | 0.795 | 124. AdamW, Muon and explicit precision > Resume | no | current |
| an evaluation suite identified by three hashes | abstain | 0.327 | Proposed table of contents > XXIV. Measure, then improve | no | - |
| learn which of three models to send a task to from logs | abstain | 0.350 | 225. Learn which model to call | yes | - |
| continuous batching with a fair scheduler across priority classes | abstain | 0.344 | 175. Continuous batching and a fair scheduler > What remains | yes | - |
| a switch over an enum must handle every case | answer | 0.920 | 5. Structs, enums and unions > Matching on an enum > What `switch` will look like | yes | current |
| default values for struct fields | answer | 0.792 | Appendix D. Coax for C++ programmers > Integers and initialization | no | current |
| a slice into an array does not keep the array alive | abstain | 0.251 | 6. Arrays, strings, memory and the context > A view doesn't keep its array alive | yes | - |
| what happens when an index is out of range | answer | 0.552 | 7. Errors and cleanup > Traps | no | current |
| return an error code instead of crashing | abstain | 0.237 | 4. Reading errors > The parts of an error | no | - |
| write a procedure once for many types | answer | 0.553 | 8. Generics and constraints | no | current |
| compute a lookup table while compiling | answer | 0.544 | 6. Arrays, strings, memory and the context > Views > What coax0 checks about views | no | current |
| are my tests strong enough to catch a changed operator | abstain | 0.328 | 11. Writing checks > What checks don't prove | no | - |
| floating point results identical on every run and machine | answer | 0.509 | Proposed table of contents | no | current |
| generate a C header for exported procedures | abstain | 0.476 | 24. A complete boundary > Generate the representation adapter | no | - |
| physical units such as meters and seconds in types | answer | 0.550 | 8. Generics and constraints > Computing types (later) | no | current |
| GPU shaders written in Coax | answer | 0.809 | 16. Determinism > Summary | no | current |
| the values a new game state begins with | abstain | 0.357 | 48. One world at three fidelities | no | - |
| converting between world space and screen space | answer | 0.547 | Building Aethyr, World Engine > Chapters > I. Foundations | no | current |
| record the inputs and play a session back identically | abstain | 0.451 | 5. Replay a session exactly > Checks | yes | - |
| spawning and despawning entities | abstain | 0.339 | 6. Entities are born and die > Checks | yes | - |
| raycast queries against the world | answer | 0.703 | Building Aethyr, World Engine > Chapters > IX. A living world | no | current |
| the character stands on the terrain and does not fall through | abstain | 0.452 | 9. A character moves > Checks | no | - |
| third person camera that orbits behind the player | answer | 0.571 | 10. The camera follows > Drawing between steps | yes | current |
| skinned figures and blending animation clips | answer | 0.591 | 14. Figures and animation | yes | current |
| hotbar and on-screen display | answer | 0.699 | 17. GPU programs | no | current |
| refuse to load a scene that exceeds the memory budget | abstain | 0.201 | 16. Budgets that refuse > The declaration | yes | - |
| cloth and rope simulated with constraints | answer | 0.792 | 25. Cloth and rope from constraints > What this chapter teaches | yes | current |
| planet positions and astronomical time scales | answer | 0.549 | 30. Typed clocks and multirate ticks > What this chapter will cover | no | current |

36 questions (36 labelled, 0 marked unanswerable) over 7530 passages, books at 1b4de66cce03, weights 20e46cbf39fdf985
reranker (seed 56): 1119 training queries, loss 0.6931 -> 0.1781; support floor 0.50, top 3
retrieval, reranked (labelled questions): recall@1 0.500, recall@3 0.639, MRR@10 0.593
answered 19, abstained 17 of 36
citations current against the books on disk: 19 of 19 answers
citation precision (the cited passage is labelled relevant): 5 of 19 answers = 0.263
abstained on labelled questions: 17; of those, 7 had a labelled passage as the closest span
answering: 51.08 ms per question for retrieval, reranking and the answer (one core)
```

```shell
coax/web/tool.sh psionic/tools/book-search.coax --answer-eval psionic/fixtures/book-search/unanswerable.tsv
```

```text
| Question | Verdict | Support | Cited passage | Labelled relevant | Citation now |
| --- | --- | ---: | --- | --- | --- |
| how do I bake sourdough bread at home | abstain | 0.230 | 19. Builds and hot reload > Measure the whole loop | no label | - |
| the capital city of Australia | abstain | 0.268 | 58. Everglade > Acceptance checks and oracles | no label | - |
| symptoms of the common flu | abstain | 0.027 | 3. Common concepts > Constants | no label | - |
| how to tune a guitar by ear | abstain | 0.359 | 69. Voices in a realm > Acceptance checks and oracles | no label | - |
| deploy a web service to Kubernetes with Helm charts | abstain | 0.205 | 54. Publish a realm > The method | no label | - |
| write a SQL join across three tables | abstain | 0.252 | 77. People who talk back > Outline | no label | - |
| the history of the Roman empire | abstain | 0.190 | 206. Condense the history | no label | - |
| open a TCP socket and send an HTTP request | answer | 0.576 | 57. Answer from current sources > Acceptance cases | no label | current |

8 questions (0 labelled, 8 marked unanswerable) over 7530 passages, books at 1b4de66cce03, weights 20e46cbf39fdf985
reranker (seed 56): 1119 training queries, loss 0.6931 -> 0.1781; support floor 0.50, top 3
answered 1, abstained 7 of 8
citations current against the books on disk: 1 of 1 answers
abstained on unanswerable questions: 7 of 8
answering: 11.50 ms per question for retrieval, reranking and the answer (one core)
```

### What the numbers say

Retrieval and answering fail in different places, and the separate
measurements show where.

- **Citations resolve.** All 19 citations were current against the books on
  disk: the identity existed, the digest matched and the bytes were the
  quoted sentence.
- **Citation precision is low: 5 of 19 (0.263).** The reranker puts a
  labelled passage first for half the questions (recall@1 0.500), but the
  answerer doesn't follow the reranker's order. It takes the sentence with
  the most support from any of the top three passages, so a sentence that
  repeats the question's words in a less relevant passage beats the
  reranker's first choice. `GPU shaders written in Coax` is answered from
  chapter 16's summary, and `hotbar and on-screen display` from chapter 17.
- **Abstention is too frequent on answerable questions: 17 of 36.** For 7
  of those 17, the closest sentence was in a labelled passage but held less
  than half the question's weight. The held-out questions were written to
  avoid the books' heading words, so a correct sentence often shares few
  words with its question.
- **Unanswerable questions mostly abstain: 7 of 8.** The eighth, `open a TCP
  socket and send an HTTP request`, was answered from this chapter's own
  earlier draft, whose acceptance table quoted the same question. The books
  do contain the sentence; it just doesn't answer it.

Under the settings fixed before the run, this assistant is not good enough
to serve: about one answer in four cites a passage that answers the
question. The freshness check works; the choice of sentence is the weak
part. Ranking sentences by the reranker's order first, or learning the
support floor on chapter 54's development queries instead of fixing it by
hand, are the next things to try. They must be measured on new held-out
questions, since these 36 have now been seen.

## Checks

```shell
psionic/check.sh --only book-search
```

[check_book_search.sh](../tests/check_book_search.sh) runs the checks of
`answer.coax` with chapters 54 to 56's files in debug and release builds,
runs both examples in both builds against their recorded output, mutates
`answer.coax` and checks the book. The checks cover:

- word boundaries at every character class edge, case folding and
  bytes above 127;
- distinct query words, and refusal when the words or their bytes don't
  fit;
- support against hand values, sentence ends at each rule, and the tie
  rules for sentences and passages;
- the support floor at, just above and at zero;
- revision digests against SHA-256 values computed outside Coax with
  Python's `hashlib`, including a 600-byte text fed in pieces;
- identities, and every outcome of `an_check`: current, a changed text, a
  renamed heading, a deleted passage, a different quote, and spans that
  start before 0, end past the text or run backwards.

`coax mutate` on `answer.coax` kills all 296 mutants; none survive.

## What this does not do yet

- **No generated answers.** The answer is always one copied sentence. A
  generator, such as [chapter 53](ch053-choosing-the-next-token.md)'s
  constrained decoding over a served model, and a check that each generated
  claim follows from its cited passages, are not built.
- **No status-aware answers.** The draft's acceptance cases about planned
  and partly implemented features ("Can I write `job` blocks in Coax?") and
  about targets without recorded evidence ("Does Elephant run on an AMD
  GPU?") need the answer to state the source's status. Chapter 54's search
  results already carry each passage's status, but the answerer doesn't
  use it.
- **One revision at a time.** The store holds only the current revision of
  each passage, so a stale answer is refused rather than compared with the
  old revision. Answers that name both revisions are not built.
- **Chapter 205's attacks** have not been run against this assistant.
- **The sentence choice is weak**, as measured above.

## Sources

- [Chapter plan](PLAN.md), Part VI and Part XXI (chapter 205).
- S. Robertson and H. Zaragoza, "The Probabilistic Relevance Framework:
  BM25 and Beyond", Foundations and Trends in Information Retrieval, 2009
  (the IDF used as word weights).
- FIPS 180-4, Secure Hash Standard (SHA-256).
