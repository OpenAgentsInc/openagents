# 6. Measure the front

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The front now makes a decision on every message. This chapter measures it like
any classifier, so a threshold means something and adding a route is a small,
tested change.

## Draft

Every member inherits the front's mistakes, so the front gets measured first.

The method is ordinary. You keep a set of messages labeled with the right
route. You hold some of them out. You run the front on the held-out set and
count. You calibrate, so that when the front says 0.9 it is right about nine
times in ten. Then you set thresholds on the calibrated numbers. A prepared
answer, for example, is served only when the front is very sure.

In the old tree the router read 0.907 route accuracy on held-out
conversations, and prepared answers were served only above a precision
threshold. Those numbers belong to that system, that model and that label set.
This chapter does not carry them forward. It rebuilds the measurement, and the
new numbers go in the chapter when they exist.

The measurement is what makes the front extensible. A new route is a new
option in a typed question plus labeled rows. You add them, recalibrate, check
the held-out set and release. No model is retrained. Episode 289's "show the
deck" and "do it with Codex" were each added to the old router this way in a
day.

The essay records one failure to keep in mind. The night before a launch, the
decision provider's account ran out of credits, and every turn fell back to
plain conversation. A composition should not have a single point of judgment.

## What we build

A plan for what this chapter ships once the draft is approved:

- A labeled message set with a held-out split, and a tool that reports route
  accuracy and calibration.
- A release gate: a new question set ships only if the held-out numbers do not
  get worse.
- The steps for adding a route, written as a recipe a contributor can follow.

It builds on what exists today:

- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`psionic/lib/decision-calibration.coax`](../../psionic/lib/decision-calibration.coax)
- [`psionic/lib/decision-quality.coax`](../../psionic/lib/decision-quality.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Labeled messages and calibration | `openagents.decision.v1` `CalibrationSet`, `CalibrationRecord` ([spec 16.1](../../docs/protocol/openagents-protocol-spec.md#161-openagentsdecisionv1)) | Files; not sent | Spec sketch; no `.proto` file |

Planned paths: `openagents/agent/front/eval/`.

## Sources

- **Episodes:**
  - [Episode 287, *Building a System One Coding Agent*](../../docs/transcripts/287.md): observed trials against a hypothetical best router
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 1:00
  - [Episode 120, *Exploring SWE-bench Verified*](../../docs/transcripts/120.md): benchmarks as a discipline
- **Old code at `coder-v1.0.0`, crates:** `route-contract`, `chat-load-bench`, `jev`
- **NIPs:** [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md), [NIP-EVAL](../../nostr/nips/openagents/NIP-EVAL.md)
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), the table of measured front properties
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P7

## Open questions for the owner

1. Where do the labeled messages come from now? The old set is at the tag. New
   ones would come from real conversations, which needs a consent rule and
   must leave out our own testing.
