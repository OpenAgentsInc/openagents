# 71. The Metaverse is a surface

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.

## Premise

The agent has lived in chat windows. This chapter puts it in a world: the
OpenAgents Metaverse is a real-time 3D place on the open web, and it is another surface of
the same runtime, not a separate product.

## Draft

[Episode 290](../../docs/transcripts/290.md) opens on openagents.com and says
it is not a slide deck, it is a playtest. The projects stand as towers on a
grid. You click "walk around" and you are in a small multiplayer world with
whoever else is there.

The reason is stated in the first minute. Working with agents should not feel
like arguing with a terminal. It should feel a little more like a video game:
a 3D world with 2D interfaces woven into it, where you have some fun while you
direct your agents.

The archive has circled this for years: a heads-up display in [Episode
111](../../docs/transcripts/111.md), a multiplayer workspace in [Episode
116](../../docs/transcripts/116.md), a game announced in [Episode
189](../../docs/transcripts/189.md), a walkable run board in [Episode
240](../../docs/transcripts/240.md), and a design for experience points and
classes in [Episode 284](../../docs/transcripts/284.md). The argument is
always that a game interface makes many agents, their work and their money
easy to see.

What changed is the engine. The OpenAgents Metaverse Engine is written in Coax, and it is built for the
web first so that a realm is a link you can share. Its own book builds it, and
Building Everglade builds a whole game on it.

In this book the Metaverse is a surface in the sense of chapter 1. It links the
same runtime. The agent you talk to in the world is the one you talk to in
chat, in the same conversation, with the same members, grants and wallet. A
panel in the world and a page on the web show the same records.

## What we build

A plan for what this chapter ships once the draft is approved:

- The agent runtime linked into the Metaverse, with chat as a panel in the world.
- World views of things this book built: the member list, the Gym board, and
  your wallet.
- One conversation that continues between the world and any other surface.

It builds on what exists today:

- [`metaverse-engine/README.md`](../../metaverse-engine/README.md)
- [`metaverse-engine/talk`](../../metaverse-engine/talk)
- [`metaverse-engine/ui`](../../metaverse-engine/ui)
- [`metaverse-engine/net`](../../metaverse-engine/net)
- [`verse`](../../metaverse)
- [`apps/openagents-web`](../../apps/openagents-web)
- [`coax/lib/nostr/nipmv.coax`](../../coax/lib/nostr/nipmv.coax)
- [`metaverse-engine/book/README.md`](../../metaverse-engine/book/README.md)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A realm session | Text frames, described frame by frame in [`everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json) | Frames on a WebSocket or WebTransport | In use: one hand-packed codec, `everglade/shared/everglade-wire.coax`, pinned by that profile. The compliance rule allows this kind ([chapter 66](ch066-the-compliance-rule.md)) |
| Presence in a world | `openagents.verse.presence.v1alpha1` ([`presence.proto`](../../proto/openagents/verse/presence/v1alpha1/presence.proto)) | None yet. NIP-MV carries poses as kinds 23300 and 33301, unchanged | Schema file; no product consumer |

### The chapter's check

- **Source:** a `coax drive` case in headless Chrome: open a realm, open the
  chat panel, send `hello` to a fake runtime with a recorded reply, then
  open `/chat` in the same session.
- **Trace:** `tap` the panel, `type hello`, `tap` send, then `state
  transcript` in the world and on `/chat`. Both print `you: hello` and the
  same reply, in one conversation with one ID.
- **Refusal:** a grant revoked from `/chat` is revoked in the world too. The
  panel's next action that needs it is refused with the same message chat
  shows.
- **Independent check:** the realm session's frames are compared with the
  bytes listed in
  [`everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json),
  and the chat side gives the transcript that the existing
  [`first-milestone`](../examples/cases/first-milestone.trace) case expects.

Planned paths: none. The Metaverse surface is a realm route in
[`apps/openagents-web`](../../apps/openagents-web) over
[`metaverse-engine/talk`](../../metaverse-engine/talk).

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 0:00 to 2:30, a playtest, not a slide deck
  - [Episode 111, *Heads-Up Display*](../../docs/transcripts/111.md): a heads-up display
  - [Episode 116, *3D Multiplayer*](../../docs/transcripts/116.md): a multiplayer 3D workspace
  - [Episode 176, *Hand Tracking*](../../docs/transcripts/176.md)
  - [Episode 189, *Toward an Agentic MMORPG*](../../docs/transcripts/189.md): toward an MMORPG
  - [Episode 240, *The Metaverse*](../../docs/transcripts/240.md)
  - [Episode 283, *Coder for Gamers?*](../../docs/transcripts/283.md): gamers as an audience
  - [Episode 284, *Gamifying Coder*](../../docs/transcripts/284.md): a productive the Metaverse
- **Old code at `coder-v1.0.0`, crates:** `verse`, `verse-core`, `verse-engine`, `verse-host`, `verse-net`, `verse-world`, `openagents-web`
- **NIPs:** [NIP-MV](../../nostr/nips/openagents/NIP-MV.md), [NIP-AGENT](../../nostr/nips/openagents/NIP-AGENT.md)
- **Also:**
  - [Building the OpenAgents Metaverse Engine](../../metaverse-engine/book/README.md)
  - [Building Everglade](../../everglade/book/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P13
