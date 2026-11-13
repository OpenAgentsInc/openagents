# 60. Reputation from reproduced claims

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Everyone now has an identity, and the evidence and market chapters have produced a public
history for each one. This chapter turns that history into reputation that a
reader computes from the records.

## Draft

A star rating is easy to give and easy to buy. A reproduced claim is neither.
So reputation here is built from the work of chapters 43 to 52 and the receipts of this
part, and from nothing else.

For a plugin, the record shows its claims and how each stands: reproduced by
how many distinct signers, validated on whose tests, adopted or not, disputed
or not, and when its evidence was last run. It shows paid use from receipts.

For a person, the record shows what they authored and how it fared, what they
checked and whether their checks held up, and what they validated. A person
who publishes honest disputes builds reputation as surely as one who publishes
plugins.

Reputation is computed by the reader. There is no score that we hand down. The
records are public, the rules for reading them are published, and two readers
running the same rules get the same answer. Someone who disagrees with our
rules can apply their own.

Keys are free, so one person can hold a hundred. A rule that counts keys is
easy to fool. The defenses are the ones the evidence chain already has:
independence is read from who signed what and when, validation needs tests the
author didn't write, and paid use costs real money. Deciding when two keys
count as distinct is still a policy question, and it is listed below.

This is promise P14: it is solved once, in the protocol, for every member.

## What we build

A plan for what this chapter ships once the draft is approved:

- A reputation reader: the public records for a member or a person, summarized
  by published rules.
- Reputation shown wherever a member or an author appears, with the records
  one step away.
- A written rule for when two signers count as independent.

It builds on what exists today:

- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/xp_ledger.coax`](../../coax/lib/nostr/xp_ledger.coax)
- [`coax/lib/nostr/att.coax`](../../coax/lib/nostr/att.coax)
- [`coax/lib/nostr/nip58.coax`](../../coax/lib/nostr/nip58.coax)

Planned paths: `openagents/registry/reputation.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 3:30, reputation, identity and payments
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): progress that rewards verified work
  - [Episode 265, *Vulnerability Scanner*](../../docs/transcripts/265.md): triage and reproducibility
  - [Episode 230, *Calling All Agents*](../../docs/transcripts/230.md): an address to agents
- **Old code at `coder-v1.0.0`, crates:** `xp-ledger`, `gym-leaderboard`, `oa-att`, `contribution-service`
- **NIPs:** [NIP-XP](../../nostr/nips/openagents/NIP-XP.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-ATT](../../nostr/nips/openagents/NIP-ATT.md), [NIP-REG](../../nostr/nips/openagents/NIP-REG.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P14

## Open questions for the owner

1. What makes two keys distinct enough to count as independent confirmations?
