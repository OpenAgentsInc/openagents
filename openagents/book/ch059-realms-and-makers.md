# 59. Realms and makers

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Part VI pays plugin authors. This chapter pays makers: a realm, a character or
an asset is a release in the same registry, bought with the same wallet and
paid out the same way.

## Draft

[Episode 290](../../docs/transcripts/290.md) describes a feed full of demos
that nobody pays for. People build remarkable things with their coding agents,
post them, and two days later no one looks. The episode asks: what if you
could list what you made, and be paid when people use it?

The example is small on purpose. A fairy in one of our realms looks poor.
Anyone could make a better one in a few minutes. So there is an upgrade button
beside it, a short list of the best replacements, and a price of a few cents.
The maker is paid.

Nothing new is needed to build this. A realm, a character, a model and a rule
set are each a package with a release, signed by its maker (chapter 38). A
purchase is a paid call (chapter 40) with a receipt (chapter 41). The maker's
share arrives by payout (chapter 42). A maker has the same identity and
reputation as a plugin author.

Forking is part of the design. Episode 290's example is a fork of our town
where it is always night and zombies come out. The fork is a new release that
depends on the original, and the dependency is how the original's maker can
share in what the fork earns.

Realms can be made by talking, as plugins can. In the episode, three people in
the audience pitch games, and their design documents are written while they
watch. The realm creator is chapter 26's interview with a different output.

We also plan a curated front page. Anyone can publish, and we choose what to
feature.

## What we build

A plan for what this chapter ships once the draft is approved:

- Realms, characters and assets as registry releases, with listings in the
  world and on the web.
- Buying in place: an upgrade next to the thing it replaces, paid from your
  wallet.
- Forks as releases that depend on their source, with the split recorded.

It builds on what exists today:

- [`verse`](../../metaverse)
- [`metaverse-engine/editor`](../../metaverse-engine/editor)
- [`apps/openagents-cli/realm.coax`](../../apps/openagents-cli/realm.coax)
- [`coax/web/project/project-publish.coax`](../../coax/web/project/project-publish.coax)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/x402`](../../coax/lib/x402)
- [`coax/lib/nostr/nipmv.coax`](../../coax/lib/nostr/nipmv.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A realm, character or asset release | `openagents.extension.v1` `Release`, `Listing` ([spec 20.2](../../docs/protocol/openagents-protocol-spec.md#202-openagentsextensionv1)) | Nostr kinds 3184 and 30184, public | Spec sketch; no `.proto` file |
| A realm document and its edits | `openagents.verse.doc.v1alpha1` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step E6) | HTTP | Built in SQL; `metaverse-engine/editor/doc.coax` and `edit.coax` are the client's one codec, and one fixture of stored bytes and SHA-256 checks both |

Planned paths: `openagents/registry/realms.coax`.

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 8:00 to 10:00 and 27:00 to 30:00, a marketplace, forks and a curated store
  - [Episode 180, *Zero to Website in 60 Seconds*](../../docs/transcripts/180.md): a website in sixty seconds
  - [Episode 229, *Autopilot Sites*](../../docs/transcripts/229.md): sites made by an agent
  - [Episode 189, *Toward an Agentic MMORPG*](../../docs/transcripts/189.md): a game with Bitcoin in it
- **Old code at `coder-v1.0.0`, crates:** `verse-content`, `verse-bake`, `verse-zone-everglade`, `world-tree`, `game-lab`
- **NIPs:** [NIP-MV](../../nostr/nips/openagents/NIP-MV.md), [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md)
- **Also:**
  - Building the OpenAgents Metaverse Engine's part on makers, and the capstone "A maker is paid" in [the books' index](../../docs/books/README.md#capstones), not drafted yet
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P10, P11

## Open questions for the owner

1. Is the maker's share the same as a plugin author's, and what does a fork's
   maker owe the original's?
2. Who curates the featured list, and by what written rule?
