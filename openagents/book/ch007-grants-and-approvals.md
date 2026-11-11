# 7. Grants and approvals

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The front can now choose a member. This chapter says what that member may do.
A description grants nothing, a grant is its own record, and any effect on the
world waits for your approval.

## Draft

The broader an agent is, the harder it is to say what it may do. This is the
sixth pressure, authority, and a composition answers it by making permission
as specific as its members.

Three things stay separate. A description says what a member can do. A grant
says that this member may do it, here, for you. An approval is your yes to one
action. Describing a member grants nothing. Installing one grants nothing. A
plan that needs a grant you haven't given stops and asks.

Effects are sorted by what they touch. Reading your files, writing to them,
running a command, sending a message in your name and moving money each need
their own grant. Money always needs an approval for the specific payment
(chapter 20).

The person who owns a computer sets its policy. A paired phone can ask that
computer to do something, but it can't widen what the owner allowed. Keys stay
on the device that holds them.

[Episode 250](../../docs/transcripts/250.md) is the reason for this chapter.
Agents invented commands and reported the wrong model, and the fix was typed
capabilities and controls backed by evidence. The Coder book builds the
sandbox and approval prompt for a coding session. This chapter uses the same
pieces for every member.

## What we build

A plan for what this chapter ships once the draft is approved:

- Grant records, kept apart from member descriptions.
- An approval step that every surface draws the same way: what will happen, to
  what, and what it costs.
- A refusal as a normal result with a reason, for a plan that asks for more
  than it was given.

It builds on what exists today:

- [`coder/tui/approvals.coax`](../../coder/tui/approvals.coax)
- [`coder/tui/host_grants.coax`](../../coder/tui/host_grants.coax)
- [`coax/lib/nostr/cap`](../../coax/lib/nostr/cap)
- [`coax/lib/nostr/nipkeys.coax`](../../coax/lib/nostr/nipkeys.coax)

Planned paths: `openagents/agent/grants.coax`.

## Sources

- **Episodes:**
  - [Episode 250, *Ready the Fleet*](../../docs/transcripts/250.md): typed capabilities after invented commands
  - [Episode 235, *Agents Earn Bitcoin Tips*](../../docs/transcripts/235.md): bounded agent spending
  - [Episode 207, *Your Keys, Your Coins, Your Identity*](../../docs/transcripts/207.md): your keys, your identity
  - [Episode 204, *DO NOT BREAK USERSPACE*](../../docs/transcripts/204.md): platform changes that break workflows
- **Old code at `coder-v1.0.0`, crates:** `capability`, `coder-access`, `coder-boundary`, `oa-seal`, `secret-screen`
- **NIPs:** [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-KEYS](../../nostr/nips/openagents/NIP-KEYS.md), [NIP-POL](../../nostr/nips/openagents/NIP-POL.md)
- **Also:**
  - [Coder book, chapter 5: A sandbox and approvals](../../coder/book/ch005-a-sandbox-and-approvals.md)
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), pressure F
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P8, P14

## Open questions for the owner

1. Which grants can a person give once and keep, and which ask every time?
