# 57. Answer from current sources

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter builds a Coax documentation assistant that answers from retrieved
passages and shows them. An answer must use the evidence it cites, and a
corrected or deleted document must stop appearing in results.

## What this chapter covers

- Answers built only from retrieved passages, shown with their source spans
  and versions.
- Index updates: a corrected document replaces its old chunks, and a deleted
  document disappears from results.
- A check that each answer uses the passages it cites.
- The assistant as the target of chapter 205's prompt-injection lab.

## What it builds

- A planned assistant over chapters 54–56, with a versioned document store.

## Dependencies

- [Chapter 53](ch053-choosing-the-next-token.md): generation with stop rules.
- [Chapter 54](ch054-search-the-coax-book.md) through
  [chapter 56](ch056-rerank-the-candidates.md): retrieval and reranking.
- A served generator, such as [chapter 64](ch064-execute-gpt-oss-20b.md)'s
  gpt-oss-20b.

## Done means

- Every answer lists its passages with source span and version.
- After a document is corrected, no query returns its old text; after it is
  deleted, no query returns it at all.
- On a fixed question set, you report the share of answers that use their
  cited evidence, judged by a stated check.
- [Chapter 205](ch205-a-document-tries-to-become-the-system-prompt.md) can run
  its attacks against this assistant.

## Sources

- [Chapter plan](PLAN.md), Part VI and Part XXI (chapter 205).
