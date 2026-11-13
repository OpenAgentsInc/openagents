# 58. Everglade

> **Status: partly implemented.** Everglade runs at `/worlds/everglade` as
> a client of the native room or the local worker. Chapter 101 describes
> the room built in chapters 86–100. The realm uses shared Metaverse Engine modules;
> extracting every private engine path remains work.

Everglade combines the forest city, characters, the watch, quests, combat,
fire, boats and the Cultist Raid. Its current page is
[`everglade5e.coax`](../../apps/openagents-web/everglade5e.coax), with content
in [`everglade/legacy/v1/`](../../everglade/legacy/v1/plan.coax) and the shared
movement step in
[`everglade-step.coax`](../../everglade/shared/everglade-step.coax).

## Read the implementation by responsibility

- [Chapter 87](ch087-server-and-client-share-one-step.md): one movement
  step and world source for prediction and authority.
- [Chapters 91](ch091-one-record-for-every-replicated-entity.md)–[94](ch094-effects-are-state-events-are-cues.md):
  replication, bandwidth, interpolation and lasting effects.
- [Chapter 95](ch095-offline-is-a-local-server.md): the local worker starts
  first; a room takes over after seating. `?offline=worker` keeps play local.
- [Chapter 96](ch096-casts-and-combat-under-latency.md): combat admission,
  release checks and command age.
- [Chapters 99](ch099-watch-the-whole-action-path.md)–[101](ch101-the-everglade-room.md):
  measurement, drawn-state drift checks and room sizing.

The room uses the OpenAgents Metaverse Engine's network profiles and session contracts for lifecycle,
party visibility and travel. The item ledger in
[`metaverse-engine/session/ledger.coax`](../../metaverse-engine/session/ledger.coax) checks
idempotent item operations and owner fencing; its bounded model is not a
claim that every durable inventory integration is complete.

[`everglade-access.coax`](../../apps/openagents-web/everglade-access.coax)
adopts the shared settings record. [Chapter 85](ch085-everyone-can-play.md)
describes the SETTINGS pane, remapping and captions.

## Check the realm

```shell
coax/bin/coax test apps/openagents-web/everglade5e.coax
coax/web/tool.sh everglade-drift
```

The second command needs built page and room artifacts and free local ports;
see [chapter 100](ch100-prove-the-room-plays-like-the-game.md). A passing
unit suite does not establish rendering performance or a drift pass under
latency. Keep each receipt with its revision and workload.
