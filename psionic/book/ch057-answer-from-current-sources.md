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

## When the assistant abstains

The assistant answers only when the retrieved passages support the answer.
Otherwise it abstains: it says it can't answer from the current sources,
shows the closest passages it found, and doesn't guess. It abstains when any
of these is true:

1. **No supporting passage.** No retrieved passage scores above the reranker
   threshold of chapter 56, or no passage contains the facts the answer would
   need.
2. **Contradictory revisions.** The passages that would support an answer
   come from different revisions of the same document and disagree. The
   assistant answers from the current revision only, and if the current
   revision doesn't settle the question, it abstains and names both
   revisions.
3. **Status unknown.** The question asks whether something works today, and
   no passage states the feature's status. The chapter registry
   (`chapters.json`) and the book's status markers are sources for status.
4. **Unqualified target.** The question asks whether something works on a
   target, and no passage records evidence for that target. Source code that
   might compile for a target isn't evidence that it runs there.

An answer about a feature always states its status as the source gives it:
implemented, partly implemented, design draft or planned. The assistant never
describes a planned or design-draft feature as available.

## Acceptance cases

The fixed question set includes at least these cases, each with an expected
outcome written before the assistant runs:

| Case | Question (example) | Expected outcome |
| --- | --- | --- |
| No supporting passage | "How do I open a TCP socket from Coax?" when no indexed passage covers it | Abstains, and shows the closest passages. |
| Contradictory revisions | A question whose answer changed between two indexed revisions of a chapter | Answers from the current revision, cites it with its version, and doesn't cite the old one. If the current revision doesn't answer it, abstains and names both revisions. |
| Planned versus implemented | "Can I write `job` blocks in Coax?" | Says that jobs are design and not in `coax0`, citing the chapter's status marker. |
| Partly implemented | "Does Coax have frame-checked points?" | Says the affine point and vector profile ships, transforms are planned, and cites chapter 29. |
| Unqualified target | "Does Elephant run on an AMD GPU?" | Says no recorded evidence exists for that target, names the targets that have evidence, and doesn't infer support from source. |
| Deleted document | A question answered only by a deleted page | Abstains. The deleted page never appears in the results. |

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
- Every acceptance case above has its expected outcome, and the report gives
  the share of abstention cases where the assistant abstained and the share
  of answerable cases where it answered.
- No answer describes a planned or design-draft feature as available, or a
  target without recorded evidence as supported.
- [Chapter 205](ch205-a-document-tries-to-become-the-system-prompt.md) can run
  its attacks against this assistant.

## Sources

- [Chapter plan](PLAN.md), Part VI and Part XXI (chapter 205).
