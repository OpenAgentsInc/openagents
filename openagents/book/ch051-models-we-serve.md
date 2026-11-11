# 51. Models we serve

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The gateway reaches other people's models. This chapter adds our own: Psionic,
the engine that runs, serves and trains models in Coax, starting with the
decision models the front depends on.

## Draft

An agent whose every thought is rented can be switched off by its landlord.
The front makes a decision on every turn, so the decision model is the first
one we want on hardware we control.

Psionic is our machine-learning engine. [Episode
216](../../docs/transcripts/216.md) introduces it as an inference and training
stack. It was first written in Rust and is now being rebuilt in Coax, with its
own book that goes from tensors to serving and training.

Decision models suit owned hardware. They are small. [Episode
290](../../docs/transcripts/290.md) names the open decision models a Pylon
runs today and says one answers in a quarter of a second on an ordinary gaming
computer. A small general model handles simple tool use beside them.

Serving is the first half. The second is that a decision can be checked. If
the same model, the same state and the same question give the same
probabilities, a second machine can recompute an answer and compare. That is
what lets a stranger's machine serve decisions for pay in the next chapter.

Training is the long road. [Episodes 224](../../docs/transcripts/224.md) and
[238](../../docs/transcripts/238.md) are about training across many machines,
and the records from Part I are the kind of data a decision model learns from,
where people consent. Training a better front is a plan and has no date.

This chapter builds little that is new. It connects the front to a model that
Psionic serves and measures it with chapter 6's test set against the hosted
route.

## What we build

A plan for what this chapter ships once the draft is approved:

- The front's decisions served by Psionic on our own hardware, behind the
  gateway.
- A side-by-side run of chapter 6's test set: our served model against the
  hosted route, with accuracy, time and cost.
- A recompute check: a second machine reproduces a served decision.

It builds on what exists today:

- [`psionic/lib/decision-models.coax`](../../psionic/lib/decision-models.coax)
- [`psionic/lib/decision-session.coax`](../../psionic/lib/decision-session.coax)
- [`psionic/lib/decision-calibration.coax`](../../psionic/lib/decision-calibration.coax)
- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`coax/lib/gateway/psionic.coax`](../../coax/lib/gateway/psionic.coax)
- [`psionic/book/README.md`](../../psionic/book/README.md)

## Sources

- **Episodes:**
  - [Episode 216, *Psionic*](../../docs/transcripts/216.md)
  - [Episode 217, *Psionic: Fast Qwen 3.5*](../../docs/transcripts/217.md): fast inference on an open model
  - [Episode 224, *Distributed Training 101*](../../docs/transcripts/224.md): distributed training
  - [Episode 236, *Tassadar*](../../docs/transcripts/236.md): a model that executes
  - [Episode 238, *The Training Run Begins*](../../docs/transcripts/238.md)
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 25:00, decision models on gaming computers
  - [Episode 145, *Going Local*](../../docs/transcripts/145.md)
- **Old code at `coder-v1.0.0`, crates:** `psionic`, `inference`, `jev-hosted`, `model-access`
- **NIPs:** [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md)
- **Also:**
  - [Psionic: ML From Scratch](../../psionic/book/README.md), the decision chapters
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P7

## Open questions for the owner

1. Which models does 1.0 serve on our own hardware, and which stay hosted?
