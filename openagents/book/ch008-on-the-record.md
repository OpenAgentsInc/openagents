# 8. On the record

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The agent now decides, plans and acts with permission. This chapter writes all
of it down: every decision, plan, grant and member call goes into a record you
can read and replay.

## Draft

[Episode 33](../../docs/transcripts/033.md) defines an open agent as one you
can inspect: you can see each run, each step's input and output, and what it
cost. That definition is older than any code in the repository, and this
chapter keeps it.

The rule comes from the Coder book: if the model saw it, it is in the log.
Here it widens. The log holds the message, the typed decision and its
probabilities, the plan, each grant and approval, each member call and its
result, and the cost of each step.

There are two kinds of record, and they do different jobs. A run journal is
the authority on what happened: what was admitted, what was dispatched and how
it ended. A trajectory is the readable trace of the steps, including the steps
of any member that is itself an agent. Journals decide. Trajectories let you
look.

A record is yours. It is private unless you publish it. [Episode
228](../../docs/transcripts/228.md) proposes public trajectories for public
work, and [Episode 215](../../docs/transcripts/215.md) proposes selling
redacted traces. Both are choices a person makes, and chapter 13 covers the
consent rules.

Part V depends on this chapter. You can't measure whether a member helped
unless both runs, with it and without it, are on the record in the same form.

## What we build

A plan for what this chapter ships once the draft is approved:

- The run journal for the general agent: decisions, plans, grants, calls,
  results and costs.
- Trajectories in the shared trace format, linking a parent step to a member's
  own trace.
- A viewer on each surface, and a replay check that rebuilds every request
  from the journal.

It builds on what exists today:

- [`coax/lib/agent/journal.coax`](../../coax/lib/agent/journal.coax)
- [`coax/lib/agent/journal-flow.coax`](../../coax/lib/agent/journal-flow.coax)
- [`coax/lib/nostr/run`](../../coax/lib/nostr/run)
- [`coax/lib/nostr/atif`](../../coax/lib/nostr/atif)

Planned paths: `openagents/agent/journal.coax`.

## Sources

- **Episodes:**
  - [Episode 33, *Agent Inspectability Planning*](../../docs/transcripts/033.md): open means inspectable
  - [Episode 34, *Agent Inspection UX Design*](../../docs/transcripts/034.md): inspection design
  - [Episode 35, *Agent Inspection UI*](../../docs/transcripts/035.md): inspection UI
  - [Episode 162, *Reasoning Stream*](../../docs/transcripts/162.md): a reasoning stream
  - [Episode 228, *Free Autopilot*](../../docs/transcripts/228.md): public trajectories
  - [Episode 249, *Subagent UI Design*](../../docs/transcripts/249.md): inspectable child histories
- **Old code at `coder-v1.0.0`, crates:** `atif`, `atif-nostr`, `hook-log`, `coder-history`, `receipts`
- **NIPs:** [NIP-RUN](../../nostr/nips/openagents/NIP-RUN.md), [NIP-ATIF](../../nostr/nips/openagents/NIP-ATIF.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md)
- **Also:**
  - [Coder book, chapter 2: Model-visible means logged](../../coder/book/ch002-model-visible-means-logged.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P4, P12

## Open questions for the owner

1. Confirm the default: records are private, and publishing one is a separate,
   explicit act.
