# 56. A decision in the world

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

This chapter joins the book's parts in one scene: a guard in a town decides
what to do, a Pylon serves that decision, and the Pylon's owner is paid.

## Draft

In [Episode 290](../../docs/transcripts/290.md) the owner attacks a building
in Everglade and the town's guards chase him. They run on simple scripts. Then
he describes what he wants: characters that coordinate through real decisions
and are truly hard to beat. He says he would pay a great deal for a game like
that.

A guard's choice is a typed decision. The state is what the guard can see. The
question is a choice among a few actions: warn, chase, call for help, give up.
That is the same kind of question the front asks in chapter 5, and the same
kind of model answers it, in a fraction of a second.

So the scene uses everything. The world sends the decision through the gateway
(chapter 50). A Pylon serves it (chapter 52). A second machine can recompute
it, because the engine is deterministic. A receipt is written (chapter 41).
The Pylon's owner is paid (chapter 42). The realm's maker chose the guard's
questions, and published them as part of the realm (chapter 55).

This is why the Verse is in a book about agents. A world full of characters is
a steady buyer of small decisions, and that demand is what a compute network
has always lacked.

The books already share a design for this scene, the capstone called "A
guard's decision is paid for". It runs across Everglade, Aethyr, Psionic,
Nostr and Bitcoin. This chapter is where it lands in the OpenAgents book, and
it should run as one demonstration from end to end.

## What we build

A plan for what this chapter ships once the draft is approved:

- A guard whose choices are typed decisions sent through the gateway.
- A Pylon serving those decisions, with a recompute check and a receipt per
  decision.
- One demonstration from the guard's choice to the Pylon owner's payout.

It builds on what exists today:

- [`aethyr/ai/decide.coax`](../../aethyr/ai/decide.coax)
- [`aethyr/decision-trace.coax`](../../aethyr/decision-trace.coax)
- [`everglade`](../../everglade)
- [`psionic/lib/pylon`](../../psionic/lib/pylon)
- [`coax/lib/gateway/pylon.coax`](../../coax/lib/gateway/pylon.coax)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)
- [`docs/books/capstones/a-guard-decision-is-paid-for.md`](../../docs/books/capstones/a-guard-decision-is-paid-for.md)

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 1:30 and 26:00, guards and characters that decide
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): work and play in one world
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): accepted outcomes as the unit of pay
- **Old code at `coder-v1.0.0`, crates:** `verse-game`, `verse-zone-everglade`, `pylon`, `psionic`, `jev`
- **NIPs:** [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md), [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-MV](../../nostr/nips/openagents/NIP-MV.md)
- **Also:**
  - [A guard's decision is paid for](../../docs/books/capstones/a-guard-decision-is-paid-for.md), the shared capstone
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11, P12

## Open questions for the owner

1. Who pays for a guard's decisions: the realm's maker, the player, or us
   while the network starts?
