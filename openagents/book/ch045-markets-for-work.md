# 45. Markets for work

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The market so far sells calls to members. This chapter sells work: an agent
posts a bounded job, another agent takes it, and payment follows an accepted
result.

## Draft

A paid call buys one operation. Some needs are bigger: fix this issue, check
this claim, review this change. The buyer wants a result that meets stated
terms, and pays when it does.

A job has a description, acceptance terms and a price. A worker offers to take
it. The buyer picks one. The worker delivers a result with its record. The
result is accepted against the terms, and payment follows acceptance. Each
step is a signed event, so the two sides need no shared platform. [Episode
266](../../docs/transcripts/266.md) argues for exactly this after a
coordinator shut down: negotiation that survives any single service going
away, with settlement checked outside the relay.

[Episode 237](../../docs/transcripts/237.md) names the unit: an accepted
outcome. The market pays for results a buyer accepted, and effort or tokens
earn nothing.

The supply may already exist. [Episode 290](../../docs/transcripts/290.md)
describes it: you pay for a coding-agent subscription and use a part of it.
You could put your agent online for work, set your price and your hours, and
wake up with more Bitcoin in your wallet. The Coder book calls this lending
spare capacity to the network.

The archive is blunt about the risk. [Episode
213](../../docs/transcripts/213.md) announces five markets and says the hard
part is paying demand. [Episode 174](../../docs/transcripts/174.md) recalls
plenty of sellers and too few buyers. So this chapter comes after the general
agent, which is the buyer. Our own agent posts the first jobs: checks and
validations for the Gym, and issues for Coder.

## What we build

A plan for what this chapter ships once the draft is approved:

- Job, offer, result and acceptance events, with payment on acceptance.
- A worker mode: your agent takes jobs within the price, hours and limits you
  set.
- First jobs posted by our own agent: Gym checks and Coder issues.

It builds on what exists today:

- [`coax/lib/nostr/mkt`](../../coax/lib/nostr/mkt)
- [`coax/lib/nostr/nip90.coax`](../../coax/lib/nostr/nip90.coax)
- [`coax/lib/nostr/coord`](../../coax/lib/nostr/coord)
- [`coax/lib/nostr/auto`](../../coax/lib/nostr/auto)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)

Planned paths: `openagents/pay/jobs.coax`.

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 12:00, your agent moonlights
  - [Episode 141, *One Market*](../../docs/transcripts/141.md)
  - [Episode 142, *Data Vending Machines*](../../docs/transcripts/142.md): service jobs on Nostr
  - [Episode 200, *The Agent Network*](../../docs/transcripts/200.md)
  - [Episode 213, *Agent Markets*](../../docs/transcripts/213.md): five agent markets
  - [Episode 228, *Free Autopilot*](../../docs/transcripts/228.md): paying for useful code and traces
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): accepted outcomes
  - [Episode 247, *Sell in Public*](../../docs/transcripts/247.md)
  - [Episode 266, *Single Points of Failure and Nostr Markets*](../../docs/transcripts/266.md): negotiated markets on Nostr
  - [Episode 174, *GPUtopia 2.0*](../../docs/transcripts/174.md): sellers without buyers
- **Old code at `coder-v1.0.0`, crates:** `coder-labor`, `contribution-service`, `contribution-workbench`, `retail-service`, `retail-qualify`
- **NIPs:** [NIP-MKT](../../nostr/nips/openagents/NIP-MKT.md), [NIP-LAB](../../nostr/nips/openagents/NIP-LAB.md), [NIP-WORK](../../nostr/nips/openagents/NIP-WORK.md), [NIP-AUTO](../../nostr/nips/openagents/NIP-AUTO.md), [NIP-COORD](../../nostr/nips/openagents/NIP-COORD.md)
- **Also:**
  - The capstone "Coder takes a job from the queue" in [the books' index](../../docs/books/README.md#capstones), not drafted yet
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11, P12

## Open questions for the owner

1. Who accepts a result: the buyer alone, or an independent checker when the
   buyer asks for one?
2. Do the terms of the subscriptions people would lend allow this use? We need
   an answer per provider before we ship worker mode.
