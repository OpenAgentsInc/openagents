# 47. Every step is an event

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

This chapter builds no new mechanism. It goes back over Parts I to VI and
shows that each step is already an event of a named kind. Once that is true,
the agent is not an application we own. It is a protocol anyone can speak.

## Draft

The general-agent essay puts it in five words: the protocol is the agent.

Take one request through the book. The message and the front's typed judgment
are a conversation job and a decision. The members it could go to are
described as capabilities, and their code is a release in the registry. The
grant is its own record. The run has a journal, and its steps are a
trajectory. The measurement of a member is an evaluation report. A second
person's rerun is a check. Outside tests give a validation. The decision to
make it a default is an adoption. The points for that work are awards. The
paid call has a payment and a receipt.

Every one of those is a signed event with a published format. Episode 288
points at the list and says that a developer can read these documents today,
point an agent at them and take part. There are 37 OpenAgents NIPs in the
repository now.

The consequence is in Episode 289: other projects may adapt and adopt this,
"like little other universes all going, and we're all speaking the same open
data protocol". A second team can write its own front, run its own relay and
ship its own defaults. Its members and ours are the same releases, its
evidence counts for ours, and its authors are paid the same way.

The essay is careful about gaps, and this chapter keeps the list. Some steps
have no public event yet. One example is the admission of a member to a single
private turn. A step with no carrier is a place where you still have to trust
the application.

## What we build

A plan for what this chapter ships once the draft is approved:

- A table from every step in this book to the NIP and event kind that carries
  it, kept next to the NIPs.
- A second, small reader that uses only public events to rebuild one agent's
  member list and the evidence for each member.
- A list of steps that have no carrier yet, each with an issue.

It builds on what exists today:

- [`nostr/nips/openagents/README.md`](../../nostr/nips/openagents/README.md)
- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)
- [`coax/lib/nostr/dec`](../../coax/lib/nostr/dec)
- [`coax/lib/nostr/cap`](../../coax/lib/nostr/cap)
- [`coax/lib/nostr/ext`](../../coax/lib/nostr/ext)
- [`coax/lib/nostr/run`](../../coax/lib/nostr/run)
- [`coax/lib/nostr/atif`](../../coax/lib/nostr/atif)
- [`coax/lib/nostr/eval`](../../coax/lib/nostr/eval)
- [`coax/lib/nostr/xp_ledger.coax`](../../coax/lib/nostr/xp_ledger.coax)
- [`coax/lib/x402`](../../coax/lib/x402)

Planned paths: `openagents/book/PROTOCOL.md`.

## Sources

- **Episodes:**
  - [Episode 288, *Three DevDays Later*](../../docs/transcripts/288.md): 2:15, read the NIPs and take part today
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 2:30, other universes on one protocol
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): open protocols for reusable agent components
  - [Episode 230, *Calling All Agents*](../../docs/transcripts/230.md): an address to agents
- **Old code at `coder-v1.0.0`, crates:** `nostr`, `atif-nostr`, `route-contract`, `ext-eval`, `xp-ledger`
- **NIPs:** [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md), [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-EXT](../../nostr/nips/openagents/NIP-EXT.md), [NIP-REG](../../nostr/nips/openagents/NIP-REG.md), [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md), [NIP-ATIF](../../nostr/nips/openagents/NIP-ATIF.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md), [NIP-XP](../../nostr/nips/openagents/NIP-XP.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-PRG](../../nostr/nips/openagents/NIP-PRG.md), [NIP-KB](../../nostr/nips/openagents/NIP-KB.md), [NIP-POL](../../nostr/nips/openagents/NIP-POL.md)
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), Part IV: the protocol is the agent
  - *Test-Time Capabilities* (`git show coder-v1.0.0:docs/essays/2026-09-29-test-time-capabilities.md`), "How the protocol carries test-time capabilities"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P12

## Open questions for the owner

1. Which gaps do we close before 1.0, and which do we ship with and name?
