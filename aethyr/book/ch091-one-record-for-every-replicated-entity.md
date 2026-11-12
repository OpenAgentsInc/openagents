# 91. One record for every replicated entity

> **Status: partly implemented.** Wire 3 (#12783, commit edd9601102) sends
> every replicated entity, players, townsfolk, creatures, zones and the
> world's abilities, as one `s ID BACK MASK V...` row
> ([everglade-wire.coax](../../everglade/shared/everglade-wire.coax)):
> state at a fixed tick with velocity, as a delta against the last tick the
> page acknowledged (`HEARD`). Rows carry `LIFE` and `WARP` (commit
> 67b4f62448): an id gone a tick and back is a new life, and a move of more
> than `WIRE_WARP_M` (3 m) in a tick is a jump, so neither is drawn between.
> The record is Everglade's; a shared schema for other realms is #12845.

The server has a state for every entity. Every client needs part of it,
every tick, in as few bytes as can be trusted. This chapter adds one
schema-driven record that carries any entity's state, and the rules for
sending it as a change from what the client already has.

## What this chapter teaches

- One schema of replicated fields, shared by every entity kind.
- State at a fixed tick, with velocity.
- Deltas against a baseline the client acknowledged, not against the last
  frame sent.
- Entity lifecycle: create, update, remove, and generation numbers.
- Teleports marked as discontinuities.

## Why one model

Per-kind rules fail in ways nobody sees until a player does. Townsfolk sent
only after 0.3 m of movement, with no velocity, give the page nothing to
draw between updates. Measured on 2146dc01d9, updates came every 233 ms
against an 80 ms draw delay, and 85 to 91% of walking frames were drawn
without moving: the owner saw "NPCs walking in place". The fix isn't a
better threshold for townsfolk. It is one rule for everything, so a fix
reaches every kind.

The research doc (section 8) lists the delivery semantics each kind of
information needs. Transforms want the newest useful state. Lifecycle wants
explicit recovery. A schema with per-field precision serves them all.

## The design

**One record.** `s ID BACK MASK V V ...`:

- `ID` names the entity: a seat `SS`, a townsperson `100000 + I`, a thing the
  server made `200000 + EID`.
- `MASK` says which fields follow, from one field list for every kind:
  `KIND X Y Z YAW VX VY VZ G CLIP HP MAX COND OWNER P1 P2 TEMP FORM CARRY T0
  DUR`. Units are cm, cm/s, centiradians and ticks.
- `x ID` removes an entity.

**State at a fixed tick.** Every entity's state is sampled at the same tick,
with its velocity, so the client can draw it between samples (chapter 93).
Nothing waits for a distance threshold.

**Deltas against an acknowledged baseline.** The research doc's example: a
state for tick 120 written as a change from tick 115 is useful only if the
client has 115. The client acknowledges the last tick it received whole
(`HEARD`, sent with its input). The server writes each field as
`new − base` against that tick's state, and `BACK` says how many ticks back
the base is. `BACK 0` is absolute. A removal is sent again until a tick
that holds it is acknowledged. A full snapshot (`D` with `F` bit 1) clears
the client's table.

**Generation numbers.** A seat's slot is reused when a new player sits.
A late record for the slot's previous holder must not move the new one.
Each record carries its entity's generation, and the client drops a record
whose generation is not the one it holds. (Things the server makes already
take a new id each time within a room's life.)

**Teleports are discontinuities.** A respawn, Dimension Door or a return to
where you stood sets a flag on the record. The interpolation buffer cuts to
the new place instead of sliding through the town.

**One decoder.** The client's side is `WireMirror`, one decoder that the
page, the check harnesses, the drift test and the worker host (chapter 95)
all load. There is no second encoder anywhere.

## What it builds

- `everglade/shared/everglade-wire.coax` version 3: the record, its
  field table and `WireMirror`.
- On the server, `wire_seat_*` and `WireHist`: per-seat bases by tick.
- Later, `aethyr/net/replicate.coax`: the same model for every realm.

## Acceptance checks and oracles

- A client that applies every frame in order holds a table equal, field by
  field, to the server's state at the acknowledged tick.
- Dropping any frame and resuming gives the same table after the next frame
  whose base the client holds.
- A record with a stale generation is dropped and counted.
- No send rule names an entity kind.

## Dependencies

Chapters 6 (entity identity) and 89 (acknowledgements).
