# 16. Subagents you can see

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

One task goes to one engine. This chapter splits a request into several tasks
that run at once, and shows every one of them: who is working, on what, and
what each has said.

## Draft

Some requests are many tasks. "Fix these ten issues" is ten pieces of work
that don't depend on each other. The agent hands each to a subagent and
reports when they finish.

Episode 289 promises that the subagents are visible, and the archive explains
why that is hard. In [Episode 249](../../docs/transcripts/249.md) child
conversations were missing from the screen. In [Episode
250](../../docs/transcripts/250.md) a fleet reported the wrong models and
invented commands. A fleet you can't inspect is a fleet you can't trust. So
each subagent has a name, a card that says what it was asked and why, its own
full history, and the model it actually ran on.

Today's tree has the first version. Golemancy gives each issue a model tier,
so a small fix goes to a small model and a hard one to a strong model. The
Golems board shows each agent's card with its latest message. Part IX puts the
same agents in the world.

Three rules keep a fleet useful. A subagent ends by reporting done or blocked,
never in the middle of a wait. Agents don't send each other courtesy replies.
And a parent can stop a child at any time.

The Coder book covers forks, subagents and jobs inside one coding session.
This chapter covers the fleet a general agent runs across tasks.

## What we build

A plan for what this chapter ships once the draft is approved:

- Fan-out in the plan: one request, several member calls, one report.
- A card and a full history for every subagent, on every surface.
- Tier selection per task, and a stop that reaches every child.

It builds on what exists today:

- [`coax/lib/agent/tier.coax`](../../coax/lib/agent/tier.coax)
- [`coax/lib/agent/local-agents.coax`](../../coax/lib/agent/local-agents.coax)
- [`apps/everglade-desktop/next.coax`](../../apps/everglade-desktop/next.coax)
- [`coax/lib/agent/session-reader.coax`](../../coax/lib/agent/session-reader.coax)

Planned paths: `openagents/agent/fleet.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, subagents it can delegate to
  - [Episode 191, *Project Tricoder*](../../docs/transcripts/191.md): phone control of a desktop agent
  - [Episode 249, *Subagent UI Design*](../../docs/transcripts/249.md)
  - [Episode 250, *Ready the Fleet*](../../docs/transcripts/250.md)
  - [Episode 278, *Coder Commands Codex & Claude*](../../docs/transcripts/278.md): bounded concurrent delegation
- **Old code at `coder-v1.0.0`, crates:** `agent-fleet`, `supervise`, `coder-scheduler`, `background`, `coder-lease`
- **NIPs:** [NIP-COORD](../../nostr/nips/openagents/NIP-COORD.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md), [NIP-ATIF](../../nostr/nips/openagents/NIP-ATIF.md), [NIP-WORK](../../nostr/nips/openagents/NIP-WORK.md)
- **Also:**
  - `programs/delegate-fan-out.json` at `coder-v1.0.0`
  - [Coder book, chapter 14: Forks, subagents and jobs](../../coder/book/ch014-forks-subagents-and-jobs.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3, P4
