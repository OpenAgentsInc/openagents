# 22. Retrieval by meaning

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

A knowledge base is only useful if the right entry is found. This chapter adds
search by meaning, using embeddings, and measures it against questions whose
source is known.

## Draft

The agent can't put the whole knowledge base in front of a model on every
turn. It has to pick. The archive has tried many ways to pick: embeddings and
similarity search in [Episodes 13](../../docs/transcripts/013.md) to
[16](../../docs/transcripts/016.md), a hosted code index in [Episode
107](../../docs/transcripts/107.md), a knowledge graph in [Episode
122](../../docs/transcripts/122.md), and repository maps in [Episodes
156](../../docs/transcripts/156.md) to [160](../../docs/transcripts/160.md).

For the knowledge base we use embedding search. Each entry gets a vector. A
question gets a vector. The nearest entries are the candidates, and a decision
model can rank them. We do not match keywords to pick entries, for the same
reason we do not match keywords to pick routes: a keyword rule works on the
examples its author thought of and fails without notice on the rest.

Retrieval is measured before it is trusted. You write questions whose answer
is in a known entry and count how often that entry comes back in the top few.
The count goes in the chapter.

Each answer cites the entries it used. If nothing relevant comes back, the
agent says it doesn't know.

Repository search is a different problem and belongs to Coder. Coder's map of
a project is a member in its own right, and it is the capability chapter 43
re-measures.

## What we build

A plan for what this chapter ships once the draft is approved:

- An embedding index over knowledge entries, built through the gateway.
- A retrieval step: nearest entries, an optional typed ranking, and citations.
- A retrieval test set with known sources, and its measured hit rate.

It builds on what exists today:

- [`psionic/lib/book-search`](../../psionic/lib/book-search)
- [`coax/lib/gateway`](../../coax/lib/gateway)
- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`coax/lib/nostr/kb`](../../coax/lib/nostr/kb)

Planned paths: `openagents/knowledge/index.coax`.

## Sources

- **Episodes:**
  - [Episode 13, *RAG First Principles*](../../docs/transcripts/013.md): retrieval first principles
  - [Episode 14, *Embeddings 101*](../../docs/transcripts/014.md): embeddings
  - [Episode 15, *Similarity Search*](../../docs/transcripts/015.md)
  - [Episode 23, *Embedding our Codebase*](../../docs/transcripts/023.md): embedding a codebase
  - [Episode 107, *Codebase Indexing via Greptile*](../../docs/transcripts/107.md): a hosted code index
  - [Episode 122, *Codebase Indexing via GraphRAG*](../../docs/transcripts/122.md): GraphRAG
  - [Episode 160, *Relevant Files*](../../docs/transcripts/160.md)
- **Old code at `coder-v1.0.0`, crates:** `knowledge`, `code-index`, `code-graph`
- **NIPs:** [NIP-KB](../../nostr/nips/openagents/NIP-KB.md), [NIP-CTX](../../nostr/nips/openagents/NIP-CTX.md)
- **Also:**
  - `recipes/retrieval-rerank.recipe.json` at `coder-v1.0.0`
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P2

## Open questions for the owner

1. Which model makes the embeddings: a hosted one through the gateway, or one
   we serve with Psionic?
