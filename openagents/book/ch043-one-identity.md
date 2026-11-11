# 43. One identity

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Authors, checkers and payers have appeared as "a key". This chapter makes that
concrete: one identity that is yours on the web, in every app and in the
Verse, with keys for your devices and your agents under it.

## Draft

Episode 289 promises reputation, identity and payments "all figured out",
once, for every plugin. Identity comes first, because the other two attach to
it.

Your identity is a key pair. The public half is your name on the network. The
private half signs what you publish: a release, a check, an approval. No
account on our servers stands behind it, so nobody can take it from you.
[Episode 207](../../docs/transcripts/207.md) is the design: a seed phrase you
write down, with your Nostr key and your wallet key both derived from it.

One key on every device would be a risk, since losing a phone would lose
everything. So each device holds its own key, and your main key signs a
statement that the device may act for you within limits. Your agents get keys
the same way. An agent can sign its own work and hold its own wallet, and what
it may do in your name is written in a record you can revoke.

A readable name helps people find you. A name at a domain maps to your key,
and anyone can check the mapping.

Many people will arrive without knowing what a key is. They can start by
logging in the usual way, and the app makes a key for them. What happens to
that key afterward is the open question below.

The command line already keeps an identity this way, in a key file on your
computer.

## What we build

A plan for what this chapter ships once the draft is approved:

- One identity across the web, the apps and the Verse, from one seed.
- Device keys and agent keys that act for you under a signed, revocable
  statement.
- Readable names that map to keys, and a profile that shows your work.

It builds on what exists today:

- [`coax/lib/nostr/keys.coax`](../../coax/lib/nostr/keys.coax)
- [`coax/lib/nostr/nip06.coax`](../../coax/lib/nostr/nip06.coax)
- [`coax/lib/nostr/nip05.coax`](../../coax/lib/nostr/nip05.coax)
- [`coax/lib/nostr/nipkeys.coax`](../../coax/lib/nostr/nipkeys.coax)
- [`coax/lib/nostr/nip46.coax`](../../coax/lib/nostr/nip46.coax)
- [`coax/lib/access`](../../coax/lib/access)
- [`apps/openagents-cli/nostr.coax`](../../apps/openagents-cli/nostr.coax)

Planned paths: `openagents/registry/identity.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 3:30, identity figured out
  - [Episode 82, *Nostr Login*](../../docs/transcripts/082.md): logging in with Nostr
  - [Episode 207, *Your Keys, Your Coins, Your Identity*](../../docs/transcripts/207.md): keys, coins and identity
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 28:30, a key that lets an agent act for you within limits
  - [Episode 211, *Autopilot Online*](../../docs/transcripts/211.md): a personalized identity
- **Old code at `coder-v1.0.0`, crates:** `oa-auth`, `openagents-login`, `oa-tokens`, `tenancy`, `oa-connections`, `coder-connect`
- **NIPs:** [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-VAULT](../../nostr/nips/openagents/NIP-VAULT.md)
- **Also:**
  - Nostr From Scratch covers the network of keys, and Bitcoin From Scratch covers signing for agent treasuries. See [the books' shared topics](../../docs/books/README.md).
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P14

## Open questions for the owner

1. When a person logs in without a key and we make one, who holds it, and how
   do they take it with them later?
