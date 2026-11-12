# 1. The simplest agent

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

This is the first chapter, so there is nothing before it. You build one loop:
read a message, call one model, print the reply, and write both to a log.
Every later chapter adds one thing to this loop and says how to tell whether
it helped.

## Draft

An agent is a loop around a model. [Episode 5](../../docs/transcripts/005.md)
defines it with five words: chat, plans, actions, users and tasks. The
smallest version has only the first. You type a message, the loop sends it to
a model, and the model's reply comes back.

That loop is where the 2023 wave of general agents started, and it is where
they stalled. The loop can talk, but it can't do dependable work. It doesn't
know your repository, your policy or your customer. Nobody can say whether it
is right. Its cost has no bound. It has no job a buyer would pay for. And
nothing says what it may touch. The general-agent essay calls these six
pressures: execution, local knowledge, evaluation, cost, a buyer and
authority.

We start with the bare loop on purpose. You can read all of it in one sitting,
and each of the six gaps is easy to see in it. The rest of the book closes
them one at a time. It does not close them by making the loop smarter. It
closes them by putting specialized members behind the loop and by measuring
each member before the loop relies on it.

The Coder book starts from the same place for one job, a repository task. This
book starts there for every job. Where the two overlap, this book links to
Coder's chapter and does not repeat it.

## What we build

A plan for what this chapter ships once the draft is approved:

- A loop in Coax: one message in, one model call through the gateway, one
  reply out, and a log line for each.
- A `chat` verb in the `openagents` command line that runs the loop from a
  terminal. This is a plan; the verb's name is the owner's to set.
- A fixture run with a recorded model reply, so the loop's checks run with no
  network.

It builds on what exists today:

- [`coax/lib/gateway`](../../coax/lib/gateway)
- [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
- [`apps/openagents-cli`](../../apps/openagents-cli)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| The log | Coder's transcript log, id `openagents.session.v1` on disk; planned package `openagents.session.log.v1alpha1` ([compliance plan](../../docs/protocol/2026-11-11-compliance-plan.md), step C1) | A file of JSON lines; not sent | Planned. Written by hand in `coax/lib/agent/session/log.coax` |

Planned paths: `openagents/agent/loop.coax`.

## Sources

- **Episodes:**
  - [Episode 1, *Intro*](../../docs/transcripts/001.md): the first statement of the project
  - [Episode 5, *Agent First Principles*](../../docs/transcripts/005.md): agents as chat, plans, actions, users and tasks
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): what the loop becomes
- **Old code at `coder-v1.0.0`, crates:** `openagents-chat`, `openagents-cli`
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), Part I: the six pressures
  - [Coder book, chapter 1: The simplest loop](../../coder/book/ch001-the-simplest-loop.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1
