# 52. Pylons

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Our own hardware is small. This chapter adds everyone else's: a Pylon is a
program you run on your computer that serves models to the network and pays
you in Bitcoin.

## Draft

The first AI product from this team was a network for selling spare compute.
[Episode 290](../../docs/transcripts/290.md) tells the story: you ran a
program on a gaming computer, it served an open model, and you were paid in
Bitcoin. At its height about 300 people ran it at once. It worked, and it
failed for one reason. Nobody was buying. The inference was not the best, the
cheapest, the fastest or the most private.

[Episode 174](../../docs/transcripts/174.md) names that failure and proposes
agents as the buyers. [Episodes 203](../../docs/transcripts/203.md),
[214](../../docs/transcripts/214.md) and [221](../../docs/transcripts/221.md)
rebuild the network under the name Pylon. Episode 290 relaunches it and says
who it is for: people with strong computers at home that sit idle most of the
day, who would be both sellers and buyers. Gamers.

Demand is different this time because the buyer is in this book. The front
makes a decision on every turn of every conversation. Decision models are
small, and they run fast on ordinary hardware. Every guard and shopkeeper in
the Verse makes decisions too (chapter 56). That is a steady stream of small
paid jobs.

A Pylon announces what it can serve. The gateway sends it requests. The work
can be checked by recomputing it on a second machine, so payment does not rest
on trusting a stranger. Receipts and payouts are the ones from Part VI.

Episode 290 calls this having ambient compute as a primitive. Whether demand
now meets supply is the claim this chapter has to earn, and the chapter
reports the numbers either way.

## What we build

A plan for what this chapter ships once the draft is approved:

- A Pylon you install with one command, which serves decision models and
  announces itself.
- Routing through the gateway to Pylons, with a recompute check on a sample of
  results.
- Receipts and payouts for Pylon owners, using Part VI's machinery, and a live
  count of Pylons and jobs.

It builds on what exists today:

- [`psionic/lib/pylon`](../../psionic/lib/pylon)
- [`psionic/lib/mesh`](../../psionic/lib/mesh)
- [`coax/lib/gateway/pylon.coax`](../../coax/lib/gateway/pylon.coax)
- [`apps/openagents-cli/pylon.coax`](../../apps/openagents-cli/pylon.coax)
- [`docs/connect-a-pylon.md`](../../docs/connect-a-pylon.md)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 2:30 to 5:30 and 24:00 to 26:00, Pylons for gamers
  - [Episode 1, *Intro*](../../docs/transcripts/001.md): an open platform tied to a compute network
  - [Episode 174, *GPUtopia 2.0*](../../docs/transcripts/174.md): sellers without buyers
  - [Episode 178, *Swarm Inference*](../../docs/transcripts/178.md): paid inference between two machines
  - [Episode 201, *Fracking Apple Silicon*](../../docs/transcripts/201.md): idle Apple hardware
  - [Episode 203, *Pylon and Nexus*](../../docs/transcripts/203.md)
  - [Episode 214, *Compute Market*](../../docs/transcripts/214.md): a compute market beta
  - [Episode 221, *Pylon Launch*](../../docs/transcripts/221.md): the Pylon launch
  - [Episode 223, *Pay the People*](../../docs/transcripts/223.md): payout bottlenecks
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): construct additional Pylons
- **Old code at `coder-v1.0.0`, crates:** `pylon`, `psionic`, `inference`, `compute-workbench`
- **NIPs:** [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-HOST](../../nostr/nips/openagents/NIP-HOST.md)
- **Also:**
  - Psionic's chapters on the Pylon service and Nostr From Scratch's on its events, listed in [the books' shared topics](../../docs/books/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11, P12

## Open questions for the owner

1. What does a Pylon earn per decision, and who sets it: us, the Pylon's
   owner, or a market?
2. How large a sample of results do we recompute, and what happens to a Pylon
   whose answers don't match?
3. Beyond our own agent and the Verse, who is the first outside buyer?
