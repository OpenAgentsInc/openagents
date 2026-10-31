# 214. Remember a fact, then revise it

> **Status: unimplemented — design draft.** Nothing here runs yet.

An agent that works across sessions needs to remember facts and to forget them
when they change. This chapter builds a source-linked memory store. Every
fact names where it came from and its project scope, and you can correct or
delete it.

## What this chapter covers

- A fact record: the content, its source (a session log event, a file span or
  a document version) and its project scope.
- Correction and deletion. A corrected fact replaces the old one. A deleted
  fact stops appearing.
- Retrieval scoped to the current project.
- Cross-session retrieval tests. A corrected fact never returns in its old
  form.

## Dependencies

- [Chapter 201](ch201-model-visible-means-logged.md): log events as sources.
- [Chapter 54](ch054-search-the-coax-book.md) and [Chapter 57](ch057-answer-from-current-sources.md): retrieval and the rule that deleted documents stop
  appearing.

## Done means

- Every retrieved fact links to an existing source.
- After a correction, retrieval tests across later sessions never return the
  old form.
- After a deletion, the fact does not appear in any later retrieval.
- A fact from one project scope never appears in another.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
