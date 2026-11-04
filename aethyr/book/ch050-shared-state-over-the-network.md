# 50. Lockstep for small deterministic games

> **Status: unimplemented — design draft.** Today Belt 2200 and Hold the
> Stair run in lockstep over the relay, sending only orders and comparing
> state hashes (`apps/openagents-web/lockroom.coax`). This chapter plans
> Aethyr's lockstep session on chapter 5's replay. Server authority, where
> one server steps the world and clients predict their own movement, is
> Part XII ([chapters 86–101](ch086-one-server-owns-the-scene.md)); Orbit
> League's and Everglade's rooms use it. Nothing here runs yet.

Presence (chapter 49) may be wrong without consequence. Shared state may
not: a goal is scored or not, a ship is destroyed or not. For two to four
seats running a seeded game whose rules are all in Coax, the cheapest way to
agree is lockstep: only inputs cross the network, and every client runs the
same ticks, using chapter 5's guarantee that the same inputs on the same
profile give the same states.

## What this chapter teaches

- Lockstep: only inputs cross the network; every client runs the same
  ticks and checks hashes.
- Desync detection, reporting and mending from a snapshot.
- When to choose lockstep, and when Part XII's server authority instead.

## The method

**Lockstep.** The game runs fixed ticks. An order names the tick it takes
effect on, at least one commit ahead, so the partner hears it before it may
run that tick. A tick runs only once every seat has committed to it, and
its orders apply in seat order:

```jai
run(t) requires commit_s ≥ t + 1 for every seat s
state_{t+1} = step(state_t, orders(t) sorted by (seat, sent))
every C ticks: send h_t = hash(state_t); h_t ≠ h'_t  =>  desync at t
```

On a desync both clients show it, and the holder of the game sends its
snapshot, which the other adopts. Latency is the commit lead; bandwidth is
the orders.

**Choosing.** Lockstep sends only orders, so its bandwidth doesn't grow with
the world, but every client waits for the slowest seat's commit, and every
client must run the same execution profile. Choose it for two to four seats
with rules in Coax. Choose server authority (Part XII) for more seats,
physics many players touch, hidden information, or players on mismatched
builds.

## What exists today

- [apps/openagents-web/lockroom.coax](../../apps/openagents-web/lockroom.coax):
  lockstep over the relay for any seeded game: commits (`u N`), orders
  (`o TICK TEXT`), hashes every `LR_CHECK` ticks (`h TICK HASH`), and
  snapshots in base64 chunks under the relay's 4,096-byte frames.
- [apps/openagents-web/belt-lockstep.coax](../../apps/openagents-web/belt-lockstep.coax)
  and [belt-net.coax](../../apps/openagents-web/belt-net.coax): Belt 2200 for
  two; battles in a shared game fought to their end on their seed; a desync
  shown to both and mended from the holder's save.
- [coax/web/room/room.coax](../../coax/web/room/room.coax): Orbit League's
  and Everglade's authoritative rooms, which Part XII teaches.
- The protocol's `openagents.verse.sim.v1` (`TickProfile`, `InputReplay`,
  `ExecutionProfile`): a replay's guarantee holds only for its exact
  execution profile, never across builds or architectures
  ([protocol](../../docs/verse/openagents-verse-protocol.md), 10.9).

## The design in Aethyr

A session has a tick profile, an execution profile digest every peer must
match, and a seat table. Inputs are chapter 4's typed actions, stamped with
their tick; the recording of chapter 5 is exactly the lockstep order log, so
every lockstep session can be replayed offline. Entity IDs (chapter 6) name
everything a snapshot mentions.

## What it builds

- `aethyr/net/session.coax`: tick and execution profiles, seats, commits and
  the input log.
- `aethyr/net/lockstep.coax`: commit waits, hashes, desync reports and
  snapshot adoption (generalized from `lockroom.coax`).

## Acceptance checks and oracles

- Two clients in lockstep, fed the same recorded orders with random network
  delays, reach bit-identical states and hashes at every check.
- A planted divergence (one client's state perturbed) is reported at the
  first check after it and mended by the snapshot.
- A peer with a different execution profile digest is refused at join.
- Every lockstep session's input log replays offline to the same final
  digest.

## Dependencies

Chapters 1, 4, 5 and 6, and chapter 49's rooms and transport suffice for
the first deterministic-game session. Physics-bearing lockstep sessions
additionally use Part IV (chapters 18–22, 36) and must qualify their solver
and world-replay profiles; those integrations are not prerequisites for an
ordinary game session. From Coax: R15 deterministic
jobs, R16 deterministic replay and snapshots,
[chapter 16](../../coax/book/ch16-determinism.md).
