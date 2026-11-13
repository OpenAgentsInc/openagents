# 50. Lockstep for small deterministic games

> **Status: partly implemented.** The relay and `lockroom.coax` implement
> lockstep for small seeded games. The shared network profile distinguishes
> lockstep from authority; a general Metaverse Engine lockstep session is still work.

Lockstep sends orders instead of authoritative state. Each peer waits for
commits, applies orders in the same sequence and compares state hashes.
[`lockroom.coax`](../../apps/openagents-web/lockroom.coax) supplies commits,
orders, hash checks and snapshot repair; Belt 2200 uses it through
[`belt-lockstep.coax`](../../apps/openagents-web/belt-lockstep.coax).
The guarantee needs the same seed, inputs and execution profile. It is not a
cross-build or cross-architecture determinism guarantee.

## Choose a trust profile

[`aethyr/net/profile.coax`](../../aethyr/net/profile.coax) names presence,
lockstep and server authority separately. Lockstep has neither authoritative
durability nor clock-earned server steps. It suits a small deterministic
game whose peers can wait for each other. Hidden information, persistent
progression and shared combat need a different contract.

For Everglade and Orbit League, follow the server path in
[chapter 86](ch086-one-server-owns-the-scene.md). Everglade shares its movement
step between server and client ([chapter 87](ch087-server-and-client-share-one-step.md)),
predicts local movement ([chapter 90](ch090-predict-your-own-movement.md))
and applies authoritative state ([chapter 91](ch091-one-record-for-every-replicated-entity.md)).
Its offline mode runs the same server in a worker
([chapter 95](ch095-offline-is-a-local-server.md)); it does not switch to
lockstep or trust a client's progression.

## Sessions beyond the socket

The checked contracts in `aethyr/session/` cover
[`party.coax`](../../aethyr/session/party.coax),
[`travel.coax`](../../aethyr/session/travel.coax),
[`lifecycle.coax`](../../aethyr/session/lifecycle.coax) and
[`ledger.coax`](../../aethyr/session/ledger.coax). They distinguish membership,
owner transfer, command recovery and durable item mutation. They do not
supply the planned generalized lockstep input log or a universal snapshot
codec. [Chapter 5](ch005-replay-a-session-exactly.md) teaches replay; use it
with the exact execution profile a lockstep game actually qualifies.
