# 57. Thin clients

> **Status: design draft, for the owner to edit.** This chapter is a plan.
> It ships no code yet, and paths marked planned do not exist.

## Premise

The runtime, the API and the world exist. This chapter builds each remaining
surface as a thin client of them: web, terminal, desktop and phone, with no
agent logic of their own.

## Draft

Chapter 2 made a rule: no surface has its own agent logic. This chapter is the
proof. It walks through each surface and shows that it only draws what the
runtime gives it.

A surface does four things. It takes your input: text, voice, a tap, a step in
the world. It draws the stream the runtime sends back: the reply, the plan,
each step, the result. It draws an approval and returns your answer. And it
supplies what only the device has, such as its key store, its files and its
notifications.

The web is openagents.com, drawn in Coax in the browser. The terminal is the
`openagents` command line, with Coder's terminal interface inside it. The
desktop and phone apps are native shells around the same Coax code.

The archive argues both sides of the interface question. [Episode
196](../../docs/transcripts/196.md) says a terminal is not enough. [Episode
275](../../docs/transcripts/275.md) goes back to a small terminal because a
person needs a tool that works every day. Phone control runs from [Episode
139](../../docs/transcripts/139.md) through [Episode
281](../../docs/transcripts/281.md). A thin client ends the argument. Every
surface gets every capability, and each can be as plain or as rich as suits
the device.

Two product rules hold on all of them. There is one conversation, with no
switch between a chat mode and a work mode. And the screen shows only things
that work. A control that does nothing is not drawn.

The test is simple. A new member added to the registry appears on every
surface with no change to any of them.

## What we build

A plan for what this chapter ships once the draft is approved:

- Each surface reduced to input, drawing the stream, approvals and device
  services.
- A check per surface that a newly added member appears and runs with no
  surface change.
- Shared drawing code for the plan, the approval step and the record, used by
  all surfaces.

It builds on what exists today:

- [`apps/openagents-web`](../../apps/openagents-web)
- [`apps/openagents-cli`](../../apps/openagents-cli)
- [`apps/openagents-desktop`](../../apps/openagents-desktop)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`apps/everglade-mobile`](../../apps/everglade-mobile)
- [`apps/shared`](../../apps/shared)
- [`coax/lib/tui`](../../coax/lib/tui)
- [`coder/tui/coder.coax`](../../coder/tui/coder.coax)
- [`coax/web`](../../coax/web)

Planned paths: `openagents/agent/surfaces/`.

## Sources

- **Episodes:**
  - [Episode 289, *OpenAgents*](../../docs/transcripts/289.md): 0:30 and 3:00, every surface
  - [Episode 139, *Going Mobile*](../../docs/transcripts/139.md)
  - [Episode 187, *Mobile Sync*](../../docs/transcripts/187.md)
  - [Episode 196, *Ditch the TUI*](../../docs/transcripts/196.md): against the terminal alone
  - [Episode 275, *Coder*](../../docs/transcripts/275.md): back to a small terminal
  - [Episode 277, *Coder Terminal*](../../docs/transcripts/277.md): the Coder terminal
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): Coder on phones
- **Old code at `coder-v1.0.0`, crates:** `openagents-desktop`, `openagents-mobile`, `openagents-terminal`, `openagents-cli`, `openagents-web`, `openagents-ui`, `coder-mobile`, `terminal-app`
- **NIPs:** [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md), [NIP-SESS](../../nostr/nips/openagents/NIP-SESS.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P13, P16

## Open questions for the owner

1. Does Android get the full OpenAgents app for 1.0, or does the Android game
   app come first and the agent follow?
