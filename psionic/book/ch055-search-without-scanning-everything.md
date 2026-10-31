# 55. Search without scanning everything

> **Status: unimplemented — design draft.** Nothing here runs yet.

Exhaustive vector search reads every embedding for every query. That is fine
for a small corpus and too slow for a large one. This chapter builds one
approximate index from scratch, HNSW, and measures how much recall it gives up
for its speed.

## What this chapter covers

- HNSW graph construction: layered neighbor lists and insertion order.
- Hierarchical search from the top layer down.
- A bounded exploration parameter that sets the search budget.
- Persistence of the index to a file, and reloading it.
- Stated memory use per stored vector.
- Introduced only once the corpus is large enough that exhaustive search is
  too slow, and that size is measured.

## What it builds

- A planned HNSW index in Coax, used beside chapter 54's exhaustive search.

## Dependencies

- [Chapter 54](ch054-search-the-coax-book.md): embeddings, chunk identities
  and the exhaustive reference.

## Done means

- Recall against exhaustive search is reported at several fixed exploration
  budgets on a fixed query set.
- Query latency and index memory are reported beside each recall figure.
- A persisted index reloads and returns the same results as before it was
  saved.

## Sources

- [Chapter plan](PLAN.md), Part VI.
- [Roadmap](../docs/roadmap.md), Part VI summary.
