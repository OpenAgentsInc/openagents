# 97. Interest without leaks

> **Status: unimplemented — design draft.** Today a seat hears every player
> in its room, and creatures, zones, townsfolk and events within one radius,
> `EG_VIEW` 120 m, tested entity by entity for every seat (`eg_near` in
> `room-everglade.coax`). The same radius serves entry and exit, so an
> entity at the edge is created and removed repeatedly. A blocked player is
> hidden by the page, which still receives them. #12789 builds the design
> below.

Interest management decides which information each client gets. It saves
bandwidth and server time, and it is a security boundary: anything sent to
a client can be read from it, encrypted or not. This chapter adds interest
with separate entry and exit radii, non-spatial subscriptions, and a rule
that nothing private reaches a client.

## What this chapter teaches

- The interest pipeline, from spatial candidates to the frame.
- Hysteresis at the edge of interest.
- Subscriptions that are not spatial, such as a party member's health.
- Withholding what a client isn't entitled to see.

## The design

The research doc (section 7) gives the pipeline. For each seat, each tick:

1. **Candidates** from a spatial index: a grid of cells over the realm, each
   listing its entities. Only cells within the outer radius are read, so the
   cost follows nearby entities, not every entity times every seat.
2. **Copy and phase**: only entities of this room and this phase.
3. **Visibility and permission**: drop what the seat isn't entitled to see.
4. **Subscriptions**: add what isn't spatial: party members' health on any
   map, the world event's state.
5. **Priority and encoding**: chapter 92's budget and chapter 91's record.

**Hysteresis.** An entity enters interest inside `R_in` and leaves only
outside `R_out > R_in` (for example 120 m and 135 m). An entity moving
along the edge is created once and removed once, instead of on every step
across one radius.

**Nothing private reaches a client.**

- A player the seat has blocked isn't sent at all, so they can't be read
  from the wire.
- An invisible or hidden entity (Invisibility, Greater Invisibility) isn't
  sent to seats that haven't seen it, until it is seen.
- A player's private state, such as their cooldowns and saved progress, is
  sent only to that player. What others can see, such as their look and
  their spells' effects, is public.

**Counted.** Relevant entities per seat are reported in vitals (chapter 99).

## What it builds

- A cell index over each room's entities, updated on move.
- The per-seat pipeline in the server's send path, replacing `eg_near`'s
  scan.
- Interest state per seat: who is in, entered at which radius.

## Acceptance checks and oracles

- An entity walked back and forth across 120 m is created and removed at
  most once per crossing of the hysteresis band.
- A blocked player's id never appears in the blocker's frames (checked on
  the raw wire).
- An invisible raider appears in a seat's frames only after that seat can
  see it.
- With 16 seats and 120 entities, the interest step's time grows with
  entities near each seat, not with all entities.

## Dependencies

Chapters 7 (spatial queries) and 92 (priority).
