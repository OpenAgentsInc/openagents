# 3. Members

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The loop so far has one thing behind it, a model. This chapter gives it a list
of members, each a specialized thing it can hand a request to. The agent
becomes a composition.

## Draft

The thesis of this book comes from the general-agent essay: a general agent is
a composition. A general front decides what each request needs. A growing set
of specialized members does the work. Specialization lives in the members, and
generality lives in the composition.

A member is anything the front can hand a request to: a prepared answer, a
general model for conversation, Coder, another vendor's coding agent, a plugin
someone wrote, a wallet, another computer. Each member has a name, a short
description, typed input and output, a list of the effects it can have, and a
status that says what evidence stands behind it.

The idea is old in this project. [Episode 36](../../docs/transcripts/036.md)
organizes agents into reusable modules, [Episode
38](../../docs/transcripts/038.md) draws them as a graph, and [Episode
242](../../docs/transcripts/242.md) describes programs that can each be
improved on their own. What those attempts lacked was a cheap, reliable way to
choose among members (chapter 5) and a way to know that adding one made the
agent better (Part V).

Members are visible. Episode 289 shows the list on screen: here are the tools,
here are the subagents. You can always ask the agent what it has, and the
answer is the same list the front chooses from.

This chapter starts with three members: conversation, answers and Coder. Two
of them are stubs until their own chapters.

## What we build

A plan for what this chapter ships once the draft is approved:

- A member descriptor type: name, summary, input, output, effects and claim
  status.
- A members list the front reads and every surface can show.
- Three first members, two of them stubs.

It builds on what exists today:

- [`coax/lib/nostr/cap`](../../coax/lib/nostr/cap)
- [`coax/lib/nostr/agent`](../../coax/lib/nostr/agent)

Planned paths: `openagents/agent/members.coax`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30, tools and subagents on screen
  - [Episode 36, *Agent Modules 101*](../../docs/transcripts/036.md): agents as reusable modules
  - [Episode 38, *Agent Node Graphs*](../../docs/transcripts/038.md): node graphs
  - [Episode 40, *Agent Brain Design*](../../docs/transcripts/040.md): a modular brain
  - [Episode 242, *Khala, Collective Intelligence*](../../docs/transcripts/242.md): independently improvable programs
- **Old code at `coder-v1.0.0`, crates:** `route-contract`, `capability`, `openagents-chat`
- **NIPs:** [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md)
- **Also:**
  - *The Return of the General Agent* (`git show coder-v1.0.0:docs/essays/2026-10-01-the-return-of-the-general-agent.md`), Part II: a general agent is a composition
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P3, P4

## Open questions for the owner

1. What does the product call these? The essays say "members"; Episode 289
   says tools, subagents and plugins.
