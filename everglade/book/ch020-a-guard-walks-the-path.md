# 20. A guard walks the path

> **Status: partly implemented.** The path guard's patrol runtime, threat list,
> and room replication (`WK_GUARD`, `e guard`) live in
> [`everglade/server/guard.coax`](../server/guard.coax),
> [`everglade/server/creatures.coax`](../server/creatures.coax), and
> [`everglade/client/guard.coax`](../client/guard.coax). Bounded NPC decision
> routing, route planning, and replay checks live in
> [`everglade/npc/guard-decision.coax`](../npc/guard-decision.coax),
> [`everglade/npc/guide-route.coax`](../npc/guide-route.coax), and
> [`everglade/npc/guard-replay.coax`](../npc/guard-replay.coax). A standalone
> runnable book example remains to be extracted.

This chapter answers the player's question: "Who is walking down the path in
armour, and how does he know where to turn around?"

An MMORPG road feels empty when only players move on it. Everglade II puts an
armoured patrol guard on the forest path—a level-10 SRD Fighter
(`rules/srd/npcs.coax`, `rs_path_guard`: AC 19, HP 84) driven entirely by the
room's creature runtime rather than bespoke client scripts.

## What this chapter teaches

- How the patrol guard is declared as content-table rows (creature, template,
  spawn, troop, and beat) instead of hard-coded coordinates.
- How the room replicates the guard's pose (`WK_GUARD`) and combat cues
  (`e guard`) through the shared creature codec.
- How the client interpolates the guard in the same jitter buffer as remote
  players so his walk never stutters.
- How Pylon-backed dialogue and direction routing (`everglade/npc/`) stay
  bounded and replayable without blocking the 30 Hz room tick.

## What exists today

| Piece | Role | Where |
| --- | --- | --- |
| Room guard state & cues | Maps patrol runtime state (`E2G_WALKING`/`STANDING`/`FIGHTING`/`DOWN`/`NONE`) to `WK_GUARD` and `e guard` | [`everglade/server/guard.coax`](../server/guard.coax) |
| Creature patrol runtime | Walks the beat, turns at endpoints, steers around trunks/players, manages threat list | [`everglade/server/creatures.coax`](../server/creatures.coax) |
| Creature wire codec | Shared `proto/verse/everglade2-combat-v2.json` encoder/decoder for `e guard` cues | [`everglade/server/creature-wire.coax`](../server/creature-wire.coax) |
| Client guard view | Interpolates `WK_GUARD` rows behind a jitter buffer and animates swing/walk clips | [`everglade/client/guard.coax`](../client/guard.coax) |
| Guard AI & route replay | Bounded decision envelopes, directions, latency telemetry, and deterministic replay | [`everglade/npc/guard-decision.coax`](../npc/guard-decision.coax), [`everglade/npc/guide-route.coax`](../npc/guide-route.coax), [`everglade/npc/guard-replay.coax`](../npc/guard-replay.coax) |

## The method

### Patrol as a creature behavior, not a special case

`everglade/server/guard.coax` deliberately owns no walking physics and no
combat math. It reads `e2_patrol(v, k)` from the shared creature runtime
(`everglade/server/creatures.coax`). A room whose content tables declare no
patrol row spawns no guard (`E2G_NONE`). When a patrol row exists, the runtime
steps his position along the path at 30 Hz, steers around trees and players,
and emits `WK_GUARD` rows whenever his state changes—or once per second while
standing still—so late joiners always see him in the exact spot he stands.

### Decisions arrive asynchronously without stalling the tick

When a player asks the guard for directions, the request crosses the
asynchronous decision boundary described in
[Psionic chapters 28–32](../../psionic/book/ch028-a-decision-arrives-while-play-continues.md).
`everglade/npc/guard-decision.coax` bounds input text, enforces request-ID
fences and tick deadlines, and falls back to an authored deterministic reply
whenever a remote model misses its deadline.

## Worked trace: guard state transitions

```text
Patrol spawned, moving along beat   -> E2G_WALKING  (0)
Reached beat endpoint pause         -> E2G_STANDING (1)
Player damages guard (threat added) -> E2G_FIGHTING (2), foe = seat
Guard reaches 0 HP                  -> E2G_DOWN     (3), told on down_t tick
Despawned / room without patrol row -> E2G_NONE     (4)
```

## The checks

```shell
coax/bin/coax test --library everglade/npc/guard-decision.coax
coax/bin/coax test --library everglade/npc/guard-replay.coax
coax/bin/coax test coax/web/room/room.coax --only "guard" --steps 4000000000
```

## What comes next

The guard never strikes first. [Chapter 21](ch021-attack-the-guard.md) covers
what happens the moment you swing at him or loose an arrow.
