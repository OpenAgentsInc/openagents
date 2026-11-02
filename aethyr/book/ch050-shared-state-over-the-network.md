# 50. Shared state over the network

> **Status: unimplemented — design draft.** Today two models of shared play
> run: Orbit League's server steps the ball and sends each player the room's
> state (`coax/web/room/room.coax`), and Belt 2200 and Hold the Stair run in
> lockstep over the relay, sending only orders and comparing state hashes
> (`apps/openagents-web/lockroom.coax`). This chapter plans Aethyr's two
> authority models on one replay foundation. Nothing here runs yet.

Presence (chapter 49) may be wrong without consequence. Shared state may
not: a goal is scored or not, a ship is destroyed or not. Aethyr offers two
ways to agree, both built on chapter 5's guarantee that the same inputs on
the same profile give the same states.

## What this chapter teaches

- Lockstep: only inputs cross the network; every client runs the same
  ticks and checks hashes.
- Server authority: one host steps the simulation; clients predict their
  own inputs and reconcile.
- Desync detection, reporting and mending from a snapshot.
- Choosing between them by player count, latency and how much state moves.

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

**Server authority.** The server steps at a fixed rate (Orbit League: 60 Hz
ticks, two 120 Hz physics steps each) with the inputs it has, and sends
each client the state. A client predicts its own entity from its unacked
inputs and, when a state arrives for tick `t`, rewinds to it and replays
its inputs after `t`; the correction is blended visually over a few frames
while the simulated state snaps.

## What exists today

- [apps/openagents-web/lockroom.coax](../../apps/openagents-web/lockroom.coax):
  lockstep over the relay for any seeded game: commits (`u N`), orders
  (`o TICK TEXT`), hashes every `LR_CHECK` ticks (`h TICK HASH`), and
  snapshots in base64 chunks under the relay's 4,096-byte frames.
- [apps/openagents-web/belt-lockstep.coax](../../apps/openagents-web/belt-lockstep.coax)
  and [belt-net.coax](../../apps/openagents-web/belt-net.coax): Belt 2200 for
  two; battles in a shared game fought to their end on their seed; a desync
  shown to both and mended from the holder's save.
- [coax/web/room/room.coax](../../coax/web/room/room.coax) and
  [room-host.c](../../coax/web/room/room-host.c): Orbit League's server with
  up to 8 rooms, quick matches with bots in empty seats, and the physics of
  `orbitleague.coax`'s `room_step`. Its checks run under a raised step limit
  because the physics checks are long.
- The protocol's `openagents.verse.sim.v1` (`TickProfile`, `InputReplay`,
  `ExecutionProfile`): a replay's guarantee holds only for its exact
  execution profile, never across builds or architectures
  ([protocol](../../docs/verse/openagents-verse-protocol.md), 10.9).

## The design in Aethyr

One session layer serves both models. A session has a tick profile, an
execution profile digest every peer must match, and a seat table. Inputs are
chapter 4's typed actions, stamped with their tick; the recording of
chapter 5 is exactly the lockstep order log, so every networked session can
be replayed offline. Lockstep is the default for small, deterministic games
(two to four seats, rules in Coax). Server authority serves realms with
physics many players touch, or with players on mismatched builds; prediction
uses the same step function the server runs. Entity IDs (chapter 6) name
everything a state message mentions.

## What it builds

- `aethyr/net/session.coax`: tick and execution profiles, seats, commits and
  the input log.
- `aethyr/net/lockstep.coax`: commit waits, hashes, desync reports and
  snapshot adoption (generalized from `lockroom.coax`).
- `aethyr/net/authority.coax`: server stepping, client prediction, rewind
  and replay, visual correction.

## Acceptance checks and oracles

- Two clients in lockstep, fed the same recorded orders with random network
  delays, reach bit-identical states and hashes at every check.
- A planted divergence (one client's state perturbed) is reported at the
  first check after it and mended by the snapshot.
- A peer with a different execution profile digest is refused at join.
- Server authority: with 150 ms simulated round trip, the predicted entity's
  drawn position never jumps by more than a stated bound on correction, and
  the server's state equals an offline replay of its input log.
- Every networked session's input log replays offline to the same final
  digest.

## Dependencies

Chapters 1, 4, 5 and 6, and chapter 49's rooms and transport suffice for
the first deterministic-game session. Physics-bearing sessions additionally
use Part IV (chapters 18–22, 36) and must qualify their solver and world-replay
profiles; those integrations are not prerequisites for an ordinary game
session. From Coax: R15 deterministic
jobs, R16 deterministic replay and snapshots,
[chapter 16](../../coax/book/ch16-determinism.md).
