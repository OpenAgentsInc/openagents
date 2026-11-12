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

On the network, your identity is a key pair. The public half is your name. The
private half signs what you publish: a release, a check, an approval. Nobody
can take a key from you, and a record you signed stays yours on any relay.

On our service, your identity is an account. It is an opaque id in our own
database, and the ways you prove you are that account attach to it: an email
address, a Nostr key, a passkey. Any combination works, and a Nostr key alone
is enough. We keep the user table ourselves. No outside login provider holds a
copy, there are no passwords, and we never collect a phone number. Chapter 50
builds the account.

[Episode 207](../../docs/transcripts/207.md) is the first design: a seed
phrase you write down, with your Nostr key and your wallet key both derived
from it. The owner narrowed that on 2026-11-11: one seed is an opt-in for
people and the default for agents.

One key on every device would be a risk, since losing a phone would lose
everything. So each device gets its own key, which never leaves its hardware
and is linked to your account by a session you already hold. Your agents get
keys of their own. Both are built. An agent
signs its own work and can hold its own wallet. What it may do in your name is
written in a grant you approve and can revoke: scopes, spend caps and an
expiry.

A readable name helps people find you. A name at a domain maps to your key,
and anyone can check the mapping. The same name can be your Lightning Address
(chapter 19).

Many people will arrive without knowing what a key is. They log in with a
code sent to their email, and add a passkey or a key when they want one. If
you lose every strong way in, recovery takes 72 hours and you can cancel it
from any channel. No member of staff can skip the wait.

The command line already keeps an identity this way, in a key file on your
computer.

## What we build

A plan for what this chapter ships once the draft is approved:

- One account across the web, the apps and the Verse, with a Nostr key as a
  first-class way in.
- Device keys bound to hardware, and agent keys that act for you under a
  grant you can revoke.
- Readable names that map to keys, and a profile that shows your work.

It builds on what exists today:

- [`coax/lib/nostr/keys.coax`](../../coax/lib/nostr/keys.coax)
- [`coax/lib/nostr/nip06.coax`](../../coax/lib/nostr/nip06.coax)
- [`coax/lib/nostr/nip05.coax`](../../coax/lib/nostr/nip05.coax)
- [`coax/lib/nostr/nipkeys.coax`](../../coax/lib/nostr/nipkeys.coax)
- [`coax/lib/nostr/nip46.coax`](../../coax/lib/nostr/nip46.coax)
- [`coax/lib/access`](../../coax/lib/access)
- [`apps/openagents-cli/nostr.coax`](../../apps/openagents-cli/nostr.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Account and its authenticators | `openagents.account.v1`, `openagents.credential.v1` ([spec 15.1](../../docs/protocol/openagents-protocol-spec.md#151-openagentsaccountv1), [spec 15.2](../../docs/protocol/openagents-protocol-spec.md#152-openagentscredentialv1)) | HTTP | The login is built with hand-written JSON ([docs/auth](../../docs/auth/README.md)). The spec's sketch predates it; the [protocol review](../../docs/protocol/2026-11-11-protocol-review.md) orders its rewrite |
| Agent account, key and grant | `openagents.account.v1alpha1` ([`agents.proto`](../../proto/openagents/account/v1alpha1/agents.proto)) | HTTP, as binary protobuf | In use ([chapter 50](ch050-accounts-and-agent-grants.md)) |
| A key's name and its owner | `openagents.identity.v1` `KeyIdentity`, `OwnerAttestation` ([spec 15.4](../../docs/protocol/openagents-protocol-spec.md#154-openagentsidentityv1)) | Nostr: kind 0, NIP-05, and an `auth` tag (NIP-OA) | Spec sketch; no `.proto` file. `coax/lib/nostr/nip05.coax` and `coax/lib/nostr/nipoa.coax` handle the events |

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

1. When does a person who logged in by email get a Nostr key for signing
   public records, where is it made, and how do they take it with them?
