# 23. Memory and consent

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The knowledge base is shared. This chapter adds what is yours: facts the agent
keeps about you between conversations, which you can read and change and which
go nowhere without your consent.

## Draft

A shared knowledge base makes the agent informed. Memory makes it yours. It
remembers which project you work on, how you like results reported, and what
you decided last week.

[Episode 113](../../docs/transcripts/113.md) builds memories and reflections
from the research of the time. The Coder book rebuilds the core: remember a
fact, then revise it when it changes. This chapter moves that to the general
agent, so a fact learned in a coding session is there when you chat from your
phone.

Memory has three rules. You can see it: every stored fact is listed, with
where it came from. You can change it: you can edit or delete any fact, and
the agent then acts on the new version. And it stays yours: memory is stored
for your key, encrypted, and is not part of the shared knowledge base.

Sharing is a separate act with its own record. The archive tried several
models. [Episode 46](../../docs/transcripts/046.md) told people to treat
uploads as public. [Episode 215](../../docs/transcripts/215.md) proposed
selling redacted traces by choice. [Episode
245](../../docs/transcripts/245.md) split a sharing tier from a private tier.
They don't agree, and this book does not pick one. It builds the mechanism
each needs: a consent record that says who owns an input, who may see it, and
what reuse or payment was agreed.

Access to a record is not permission to publish it or to train on it.

## What we build

A plan for what this chapter ships once the draft is approved:

- A memory store per identity, encrypted, with a list you can read and edit.
- A consent record that every publish or share step requires.
- A check that a member without a memory grant never receives memory.

It builds on what exists today:

- [`coder/tui/memory.coax`](../../coder/tui/memory.coax)
- [`coax/lib/nostr/vault.coax`](../../coax/lib/nostr/vault.coax)
- [`coax/lib/nostr/nip44.coax`](../../coax/lib/nostr/nip44.coax)
- [`coax/lib/access`](../../coax/lib/access)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A stored memory | `openagents.vault.v1` `VaultObject` with key slots ([spec 15.3](../../docs/protocol/openagents-protocol-spec.md#153-openagentsvaultv1)) | Nostr: ciphertext in a kind 3188 envelope (NIP-VAULT); a device also keeps a file | Spec sketch; no `.proto` file. `coax/lib/nostr/vault.coax` builds the `user` tier. Coder's memory log is a hash-chained file; a fixture to pin it is planned |
| Who may see an input | `openagents.policy.v1` `DisclosurePolicy` ([spec 19.2](../../docs/protocol/openagents-protocol-spec.md#192-openagentspolicyv1)) | Nostr: a private record (NIP-POL) | Spec sketch; no `.proto` file |
| Consent to publish or share | None yet | Planned | Gap. This chapter's consent record needs a schema |

Planned paths: `openagents/knowledge/memory.coax`.

## Sources

- **Episodes:**
  - [Episode 113, *Agent Memories & Reflections*](../../docs/transcripts/113.md): memories and reflections
  - [Episode 202, *Recursive Language Models*](../../docs/transcripts/202.md): what persists between steps
  - [Episode 46, *Hello EpsteinSleuth*](../../docs/transcripts/046.md): uploads treated as public
  - [Episode 215, *Data Market*](../../docs/transcripts/215.md): opt-in sale of redacted traces
  - [Episode 245, *Khala Code*](../../docs/transcripts/245.md): sharing and private tiers
  - [Episode 270, *Deploying Sarah*](../../docs/transcripts/270.md): a long-running agent's memory
- **Old code at `coder-v1.0.0`, crates:** `memory-stream`, `oa-vault`, `private-fs`, `user-home`
- **NIPs:** [NIP-VAULT](../../nostr/nips/openagents/NIP-VAULT.md), [NIP-CTX](../../nostr/nips/openagents/NIP-CTX.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md)
- **Also:**
  - [Coder book, chapter 18: Consent before sharing](../../coder/book/ch018-consent-before-sharing.md)
  - [Coder book, chapter 19: Remember a fact, then revise it](../../coder/book/ch019-remember-a-fact-then-revise-it.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P2, P14

## Open questions for the owner

1. Where does memory live: on the device only, encrypted on a relay, or on our
   service?
2. Which sharing model do we offer first: none, opt-in publishing, or opt-in
   sale?
