# 56. Rerank the candidates

> **Status: unimplemented — design draft.** Nothing here runs yet.

Lexical and dense search each miss results the other finds. This chapter
merges both result lists and then scores a short list more carefully. It also
keeps three measurements apart, so a gain in one stage is never credited to
another.

## What this chapter covers

- Merging BM25 and dense results into one candidate list.
- A reranking stage that scores a small shortlist with a more expensive
  model.
- Three separate measurements:
  - index recall: did the right passage reach the candidate list;
  - retrieval relevance: is it ranked near the top;
  - answer quality: does the final answer use it correctly.

## What it builds

- A planned merge step and a reranker interface over chapter 54's chunk
  identities.

## Dependencies

- [Chapter 54](ch054-search-the-coax-book.md): BM25 and dense search.
- [Chapter 55](ch055-search-without-scanning-everything.md): the approximate
  index.

## Done means

- Each of the three measurements is reported on the same fixed query set,
  before and after reranking.
- The reranker's cost per query is reported.
- A change is credited only to the stage whose measurement moved.

## Sources

- [Chapter plan](PLAN.md), Part VI.
