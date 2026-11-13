# 27. Programs

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

Plans so far are a step or a few. This chapter names the multi-step ones and
stores them: a program is a typed workflow whose steps decide, call members or
hand off work.

## Draft

Some work has a known shape. Solving an issue is: read it, find the files,
change them, run the checks, fix what failed, open the pull request. Reviewing
a change has its own shape. So does answering a question from the knowledge
base.

A program writes that shape down. It is a typed workflow, and each step is one
of a few kinds. A decide step asks a decision model a pinned question. A call
step runs a member. A delegate step hands a bounded task to an engine. Because
the steps are typed, a program can be read, checked, measured and improved one
step at a time.

The idea comes from the graphs of plugins in [Episode
75](../../docs/transcripts/075.md), through the separately improvable programs
of [Episode 242](../../docs/transcripts/242.md), to the state machines of
[Episode 286](../../docs/transcripts/286.md). The old tree shipped eight
programs, among them solve-issue, review-changes, delegate-fan-out and
answer-question. It also shipped twenty recipes, small workflows built on
decision models for jobs such as ticket triage, log classification and
citation checks.

A program is one of the four kinds of thing a person can contribute. The
others are a plugin, a skill and a knowledge entry. Like any member, a program
is a candidate until it is measured. A search for a better version of a
program ships only through that measurement.

The front can call a program by name, and a program can call the front's
members.

## What we build

A plan for what this chapter ships once the draft is approved:

- A program format with decide, call and delegate steps, and a runner that
  logs each step.
- Ports of the first programs: solve-issue, review-changes and
  answer-question.
- A check that a program only calls members its grant covers.

It builds on what exists today:

- [`coax/lib/nostr/opt`](../../coax/lib/nostr/opt)
- [`coax/lib/nostr/cj`](../../coax/lib/nostr/cj)
- [`coax/lib/systemone`](../../coax/lib/systemone)
- [`coax/lib/microcoder/loop.coax`](../../coax/lib/microcoder/loop.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| Program and its steps | `openagents.program.v1` `Program`, `Step`, published by a `ProgramHead` ([spec 20.1](../../docs/protocol/openagents-protocol-spec.md#201-openagentsprogramv1)) | Nostr kind 30182 (NIP-PRG) | Spec sketch; no `.proto` file |
| A recorded step | `openagents.program.v1` `StepEnvelope` ([spec 20.1](../../docs/protocol/openagents-protocol-spec.md#201-openagentsprogramv1)) | In the run journal (chapter 18) | Spec sketch; no `.proto` file |

Planned paths: `openagents/agent/programs/`.

## Sources

- **Episodes:**
  - [Episode 75, *Run All Plugins*](../../docs/transcripts/075.md): running a graph of plugins
  - [Episode 114, *Planning & Execution*](../../docs/transcripts/114.md)
  - [Episode 242, *Khala, Collective Intelligence*](../../docs/transcripts/242.md): independently optimizable programs
  - [Episode 286, *System One in Coding Agents*](../../docs/transcripts/286.md): explicit state and conditional instructions
- **Old code at `coder-v1.0.0`, crates:** `program-launch`, `workflow-run`, `briefed-agent`
- **NIPs:** [NIP-PRG](../../nostr/nips/openagents/NIP-PRG.md), [NIP-OPT](../../nostr/nips/openagents/NIP-OPT.md), [NIP-DEC](../../nostr/nips/openagents/NIP-DEC.md)
- **Also:**
  - `programs/*.json` and `recipes/*.recipe.json` at `coder-v1.0.0`
  - [Coder book, chapter 8: Programs as tool calls](../../coder/book/ch008-programs-as-tool-calls.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P5, P6

## Open questions for the owner

1. Which of the twenty recipes come back, and are they members of the general
   agent or examples in this book?
