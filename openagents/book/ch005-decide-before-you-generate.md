# 5. Decide before you generate

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 4 defines the plan. This chapter fills it in: the front asks a
decision model a few typed questions about each message and builds the plan
from the probabilities it gets back.

## Draft

A front has to be right, fast and cheap on every turn, and it has to answer in
a form code can act on. A chat model is a poor fit for that job. It is trained
to write text that reads well to a person, and nobody reads a router's output.

A decision model takes one state and a set of typed questions, such as yes or
no, a choice among named options, or a score. It answers with probabilities.
For each message the front asks: which route is this, does a prepared answer
fit, does it need more specifics, what is the risk, does the work need a
computer, and did the person name an engine? One request answers all of them.

[Episode 289](../../docs/transcripts/289.md) gives the reason this works now
when our earlier attempts did not: the models are good enough, and a type-safe
decision model makes routing feasible. In the old tree, one request answered
every router question in about 170 ms at the median. [Episodes
285](../../docs/transcripts/285.md) to [287](../../docs/transcripts/287.md)
are the work that led there.

Every decision goes through our Decisions API, which tries our own decision
model first and falls back to another. New code never calls one vendor's model
directly.

We do not route by matching keywords. A message goes where the typed decision
says, and chapter 6 measures how often that is right.

## What we build

A plan for what this chapter ships once the draft is approved:

- The front: one typed request per message, answered through the Decisions
  API, turned into a plan.
- A question set pinned by digest, so a result names exactly what was asked.
- Failover across decision routes, with the route that answered written to the
  log.

It builds on what exists today:

- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`psionic/lib/decision-models.coax`](../../psionic/lib/decision-models.coax)
- [`psionic/lib/decision-plan.coax`](../../psionic/lib/decision-plan.coax)
- [`coax/lib/nostr/dec`](../../coax/lib/nostr/dec)
- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)

Planned paths: `openagents/agent/front/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:00, routes matched through a decision model
  - [Episode 285, *Bendcoder*](../../docs/transcripts/285.md): classification before generation
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md)
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): measured decision probes
- **Old code at `coder-v1.0.0`, crates:** `jev`, `jev-hosted`, `route-contract`, `openagents-chat`
- **NIPs:** [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md), [NIP-CJ](../../nostr/nips/openagents/NIP-CJ.md)
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), "Why the front is feasible now: decision models"
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P7

## Open questions for the owner

1. Which decision models serve the front for 1.0, and in what order: our own
   on our hardware, a hosted one, and which backup?
2. What does the front do when no decision route answers: plain conversation
   with no routing, as the old tree did, or a visible error?
