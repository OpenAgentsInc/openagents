# 21. A knowledge base

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Prepared answers cover the common questions. This chapter adds the store
behind everything else the agent knows: entries anyone can read, each signed,
versioned and tied to its source.

## Draft

[Episode 289](../../docs/transcripts/289.md) opens its demo with this: ask
about the knowledge base, and the whole of it is loaded. Promise P2 is a
shared knowledge base the agent answers from.

The material exists. The repository holds seven books, the protocol documents,
and transcripts of 290 episodes. In [Episode
290](../../docs/transcripts/290.md) the owner calls that archive one of the
project's main assets, because an agent can go back through three years of
positions and reasoning.

A knowledge entry is a small signed record: a statement, its source, a version
and a head that points at the current version. Anyone with a key can publish
one. The same store holds two kinds of entry. Some are facts about OpenAgents,
drawn from the documents. Others are method notes of the kind the old tree
kept: how to do a task, an edge case to watch for, a slip that made a check
pass when it should not have.

An entry is not trusted because it exists. The evidence rule of chapter 43 applies to knowledge
too. A knowledge entry is a candidate until a measurement shows that giving it
to the agent helps on some declared kind of question. The old tree's notes
were written during benchmark work, so they need that measurement on new tasks
before the agent leans on them.

This is the oldest thread in the archive. The first nineteen episodes are
about chatting with documents.

## What we build

A plan for what this chapter ships once the draft is approved:

- A knowledge store with signed entries, versions and heads.
- An importer that turns documents, book chapters and transcripts into entries
  that cite their source.
- A knowledge member that answers from entries and shows which ones it used.

It builds on what exists today:

- [`coax/lib/nostr/kb`](../../coax/lib/nostr/kb)
- [`coax/lib/nostr/event.coax`](../../coax/lib/nostr/event.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Knowledge entry, head and withdrawal | `openagents.knowledge.v1` `Entry`, `Head`, `Withdrawal` ([spec 20.4](../../docs/protocol/openagents-protocol-spec.md#204-openagentsknowledgev1)) | Nostr kinds 3190, 30190 and 3191, public (NIP-KB) | Spec sketch; no `.proto` file. `coax/lib/nostr/kb` handles the NIP's JSON |

### The chapter's check

- **Source:** the store, importer and knowledge member in
  `openagents/knowledge/`, with one book chapter imported as the fixture.
- **Trace:** the importer prints each entry it made with the source path
  it cites. The head points at the latest version. Asked a question the
  chapter answers, the knowledge member replies and lists the entry IDs it
  used.
- **Refusal:** an entry with a bad signature is refused by the store with
  `refused: bad signature`. A withdrawn entry is never used in an answer.
- **Independent check:** each entry's event ID is recomputed by hand as the
  SHA-256 of its NIP-01 serialization, and its signature is verified by a
  NIP-01 library outside the tree.

Planned paths: `openagents/knowledge/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, the knowledge base
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 13:00, the transcripts as an asset
  - [Episode 11, *Chatting with a PDF*](../../docs/transcripts/011.md)
  - [Episode 19, *Chat with PDF, OpenAgents Edition*](../../docs/transcripts/019.md): document chat on our own stack
  - [Episode 40, *Agent Brain Design*](../../docs/transcripts/040.md): the knowledge-base model
  - [Episode 155, *Open Knowledge Graph*](../../docs/transcripts/155.md): an open knowledge graph with contributor incentives
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): reusable knowledge with evidence
- **Old code at `coder-v1.0.0`, crates:** `knowledge`, `memory-stream`
- **NIPs:** [NIP-KB](../../nostr/nips/openagents/NIP-KB.md)
- **Also:**
  - `knowledge/` at `coder-v1.0.0`: about 150 method, tool, edge-case and slip notes
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P2

## Open questions for the owner

1. Which of the old tree's notes come back? Many were learned from benchmark
   tasks and need new evidence.
2. Is every entry about OpenAgents public, or are some for logged-in people
   only?
