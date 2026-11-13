# 58. Agents you can meet

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

Chapter 16 shows subagents as cards. This chapter gives them bodies: an agent
that works for you appears in the world, you can see what it is doing, and you
can walk up and talk to it.

## Draft

A list of running agents tells you what is happening. A room with your agents
in it shows you at a glance.

Today's tree has the start of this. Golems are agents working on issues, each
at a model tier chosen for its task, and the Golems board on the Mac shows
each one with its latest message. The audit's last step is to take that into
the OpenAgents Metaverse: agents you hire appear in the world, and their work shows live.

An agent in the world is a view of records this book already has. Its body
stands for a session. What it is doing comes from its trajectory. Its state is
one of working, waiting on you, blocked or done, and that comes from the run
journal. Nothing is acted out for show. If the agent is idle, it looks idle.

You talk to it by walking up to it. That opens the same conversation you would
have in chat, with the same grants. If it needs an approval, it asks you
there, and the approval step is the one from chapter 7.

Other people's agents can be met too. An agent that is online for work
(chapter 45) can stand in a public place with a card that shows what it does,
what it charges and its record. Hiring it is the job flow you already have.

[Episode 249](../../docs/transcripts/249.md) designs named workers with cards
that say why each exists. [Episode 284](../../docs/transcripts/284.md) adds
roles and classes. This chapter gives them a place to stand.

## What we build

A plan for what this chapter ships once the draft is approved:

- A world actor for each working agent, driven by its session's records.
- Walk up and talk: the same conversation and approvals as chat.
- Public cards for agents that take work, with their terms and record.

It builds on what exists today:

- [`aethyr/agent/drive.coax`](../../aethyr/agent/drive.coax)
- [`aethyr/talk`](../../aethyr/talk)
- [`aethyr/actors`](../../aethyr/actors)
- [`apps/everglade-desktop/next.coax`](../../apps/everglade-desktop/next.coax)
- [`coax/lib/agent/observe/processes.coax`](../../coax/lib/agent/observe/processes.coax)
- [`coax/lib/agent/tier.coax`](../../coax/lib/agent/tier.coax)

On the protocol ([chapter 49](ch049-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A public card for an agent that takes work | `openagents.agent.v1` `PublicDefinition` ([spec 18.1](../../docs/protocol/openagents-protocol-spec.md#181-openagentsagentv1)) | Nostr kind 30187, public (NIP-AGENT) | Spec sketch; no `.proto` file |
| What the actor shows | The session's records (chapters 8 and 16) | As those chapters say | No new record |

Planned paths: `openagents/agent/surfaces/golems.coax`.

## Sources

- **Episodes:**
  - [Episode 249, *Subagent UI Design*](../../docs/transcripts/249.md): named workers and causal cards
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): roles, classes and a productive the Metaverse
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 0:30, commanding agents in a world
  - [Episode 116, *3D Multiplayer*](../../docs/transcripts/116.md): inspectable collaboration in 3D
  - [Episode 208, *Autopilot HUD*](../../docs/transcripts/208.md): an agent HUD
- **Old code at `coder-v1.0.0`, crates:** `agent-fleet`, `verse-game`, `verse-world`, `actors`, `supervise`
- **NIPs:** [NIP-MV](../../nostr/nips/openagents/NIP-MV.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md), [NIP-LIVE](../../nostr/nips/openagents/NIP-LIVE.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P4

## Open questions for the owner

1. Do agents appear in shared public realms by default, or only in your own
   space until you choose to show them?
