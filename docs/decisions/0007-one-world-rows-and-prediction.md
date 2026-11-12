# 0007. One World in the room, every entity a row, prediction in the engine

Date: 2026-11-12
Status: Accepted
Source: [Everglade and Aethyr against AAA MMORPG practice](../audits/2026-11-11-everglade-aethyr-aaa-audit.md), sections G4, G5 and G6; tracker #13257, issues #13274 to #13277; [everglade/ARCHITECTURE.md](../../everglade/ARCHITECTURE.md)

## Context

The room held its players in a 16-seat array, its monsters, guard, familiars and arrows in four more records, and told the pages about them through four families of frames beside the row layer. Reconciliation was written in each of Everglade's two pages. The engine had an entity store and nothing that held a realm's values, order of work or neighbours.

These four issues are one change to how a room holds and tells its entities, so they share one design.

## Decision

**The World (#13274, built).** `aethyr/core/world.coax` holds a realm's entities on the client, in the room and in the offline worker: the entity store, fixed columns keyed by slot (transform, body, health, kind, replication row), an ordered system list whose declared reads and writes are checked, and a spatial grid (`aethyr/core/spatial.coax`). Caps are the realm's storage, taken whole or refused. Archetypes are not used.

**The room core (#13275).** A zone is one `World` sized by the zone's declared cap, stepped by one thread.

- A seat is an entity with a seat record beside its slot. Nothing indexes by a seat count.
- The shapes that carry the count today go: one-integer seat masks (an event's `ev_who`, a creature's `hurt_by`, interest's block words) become lists of entity slots or one bit a slot in a column of words; arrays `[EG_SEATS]` inside a seat (`heard_n`, `heard_w`, `n_t`, `w_t`) become rows of a per-recipient table sized by what interest lets the seat see; ids packed by 1,000 (`EG_TGT_SEAT`, `room-tick.c`) become `EntityId`s.
- The tick is the World's system list, in this order: input, movement, creatures, battle, effects, rows, interest, send. Each declares its components, so two zones never share a step and a later scheduler can run systems that do not meet.
- One zone to a thread: the host's `poll()` loop hands each zone's sockets to that zone's thread. A slow zone delays nobody else.
- The design number is 150 players a zone ([0003](0003-150-a-zone.md)); `WorldStore` holds 4,096 slots.

**Rows (#13276).** Every replicated entity is a row of the session wire, from its World columns.

- A kind declares its fields once, in one table: which of the row's fields it uses and what each means. Delta, acknowledgement, priority, the link budget, interest and the block list come from the row layer.
- Monsters and the guard already stand in the battle as creatures. Their rows take a kind each (`WK_MONSTER`, `WK_GUARD`) in slots after the forest's, with state in `G`, the act in `CLIP`, the act's tick in `T0`, the foe as a target code in `OWNER`, and the monster's index and template in `P1` and `P2`. A new life is the row's `LIFE`.
- Familiars and arrows in flight are rows too. What happened (a blow, a fall, a shot loosed, a familiar's answer) is an `e` entry of the `D` frame on the reliable event class.
- `g2 g3 h2 h3`, `m1` to `m8`, `M1` to `M7`, `U1` and `u1` are deleted with their codecs, profiles and allowlist lines. Commands a player sends become `E` (use) kinds. The projectile frames `F a d` stay.
- The wire version rises (5 to 6), so a page and a room on either side of the change never meet.

**Prediction (#13277, engine half built).** `aethyr/net/predict.coax` keeps a page's inputs by number, the room's ACK and the re-run, and a ledger of actions that wait for the room. `aethyr/net/interp.coax` is the interpolation buffer, its clock and the easing of corrections. A realm supplies its step and its row's fields.

## Consequences

- **A wire change ships as one release set.** `coax/web/release.sh` ships the page alone, and production's rooms are promoted separately. The page is released often; a page that speaks wire 6 to a room that speaks wire 5 is refused, tells the player to reload, and is refused again, so nobody can play. `release-set ship` already stops a page whose pins no live shard runs, but it compares the engine, rules and content pins, not the wire version, and plain `release.sh` compares nothing. So #13276 lands on `main` only when (a) the page's release refuses to ship ahead of the rooms' wire, and (b) the rooms are promoted in the same set. Until then its code is not on `main`.
- The room's own creature histories on the page (`e2n_mon_at`, `e2n_guard_at`) go when their rows arrive: every entity is then drawn from the engine's one buffer.
- Orbit League's page predicts nothing today. It is the second realm for the predictor once its car is stepped on the page with the room's vehicle step.
- The creature runtime (#13293), navigation (#13281) and the binary wire (#13278) build on these: a creature is a World entity with a template, a path is a system, and the field table that names each kind's row fields is the one the binary encoder is generated from.
