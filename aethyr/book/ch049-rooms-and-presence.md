# 49. Rooms and presence

> **Status: unimplemented — design draft.** Today presence is an HTTP poll
> answered from SQL (`coax/web/api/room.c`), Orbit League has its own 60 Hz
> room server and other games use a relay of unread text frames
> (`coax/web/room/`). This chapter plans Aethyr's rooms: who is in a realm,
> where they are and what they look like, as one bounded, self-asserted
> stream with interest and interpolation. Nothing here runs yet.

Presence answers a small question often: who else is here, where, and doing
what. It needs no authority (a player says where it is, and nobody's game
depends on believing it), so it can be cheap, lossy and late, as long as it
looks smooth and stays bounded on a phone.

## What this chapter teaches

- A room as a named set of seats with a capacity.
- Presence as self-asserted state: position, orientation, animation and
  look, keyed by stable entity IDs.
- Interest: send and draw only the nearest, the rest by name.
- Interpolating remote players a fixed delay behind, so jitter does not
  show.
- Rate limits and sizes that refuse rather than queue.

## The method

Each client posts its own entity state every `Δ_post` seconds and receives
the others seen within a window `W`. A remote player is drawn at a fixed
delay `D` behind the newest sample, interpolated between the two samples
that bracket it:

```jai
t_draw = t_now − D,      D ≈ 2 · Δ_post
x(t_draw) = lerp(x_a, x_b, (t_draw − t_a) / (t_b − t_a))   // t_a ≤ t_draw ≤ t_b
```

With no newer sample, the drawing holds the last pose rather than
extrapolating into a wall. Only the `k` nearest others are drawn as bodies;
the rest appear by name. A sample older than `W` drops the player. The
latency budget from a key going down on one client to the move being drawn
on another is `Δ_post + network + D`, and it is measured, not assumed.

## What exists today

- [coax/web/api/room.c](../../coax/web/api/room.c): `POST
  /api/playtest/presence/<room>` with `{x, y, z, yaw, after}`, answered with
  the others and new chat lines; rules in SQL
  (`coax/web/api/migrations/0014_verse_rooms.sql`: a page posts about every
  second and sees everyone seen in the last six seconds).
- [apps/openagents-web/fairyroom.coax](../../apps/openagents-web/fairyroom.coax):
  Fairy Forest posts every 0.25 s (`FF_ROOM_EVERY`) and draws the ten
  nearest others as fairies (`OTHERS_DRAWN`) so a phone keeps 60 FPS.
- [coax/web/presence-pair.coax](../../coax/web/presence-pair.coax): two
  headless Chromes measure how soon each sees the other move, failing over a
  millisecond budget.
- [coax/web/room/room-relay.coax](../../coax/web/room/room-relay.coax):
  relay rooms keyed by game and code, up to `RELAY_SEATS` 4, carrying text
  frames unread (`y`, `j`, `l`, `m`).
- The protocol's presence package, `openagents.verse.presence.v1`
  (`EntityState`, `PoseFrame` at most 4 KiB with 1–16 entries), in
  [docs/verse/openagents-verse-protocol.md](../../docs/verse/openagents-verse-protocol.md),
  section 10.6.

## The design in Aethyr

A room is `(realm, name)` with a capacity. A client's presence is a
`PoseFrame` of its own entities (avatar, pet, carried object) using the
stable IDs of chapter 6 and the typed spaces of chapter 2. The transport is
a WebSocket to the room server, falling back to the HTTP poll; both carry
the same frames. The receiving side keeps a small ring of samples per remote
entity and the draw delay `D`. Presence never feeds the local simulation:
remote avatars are drawn, and collide only as kinematic bodies if the realm
asks.

## What it builds

- `aethyr/net/room.coax`: rooms, seats, joins and leaves, capacity.
- `aethyr/net/presence.coax`: frame encoding, sample rings, interest and
  interpolation.

## Acceptance checks and oracles

- Two headless sessions see each other move within one second at the 95th
  percentile (the existing presence-pair budget), over both transports.
- Remote motion drawn from jittered samples (recorded jitter up to `D`) has
  no backward steps and no extrapolation past the last sample.
- A frame over 4 KiB, more than 16 entries, or posts faster than the rate
  limit are refused and counted.
- Automation sessions stay out of other players' rooms and analytics, as
  migration 0074 requires today.
- A phone with a full room keeps 60 FPS.

## Dependencies

Chapters 1 (the clock), 2 (spaces), 5 (replay, for recorded sample
streams), 6 (stable entity IDs) and 14 (the poses drawn). From Coax:
[chapter 18](../../coax/book/ch18-protocol.md) for the protocol boundary,
R13 typed clocks.
