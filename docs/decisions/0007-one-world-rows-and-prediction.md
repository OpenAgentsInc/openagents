# 0007. One World in the room, every entity a row, prediction in the engine

Date: 2026-11-12
Status: Accepted
Source: [Everglade and Aethyr against AAA MMORPG practice](../audits/2026-11-11-everglade-aethyr-aaa-audit.md), sections G4, G5 and G6; tracker #13257, issues #13274 to #13277; [everglade/ARCHITECTURE.md](../../everglade/ARCHITECTURE.md)

## Context

The room held its players in a 16-seat array, its monsters, guard, familiars and arrows in four more records, and told the pages about them through four families of frames beside the row layer. Reconciliation was written in each of Everglade's two pages. The engine had an entity store and nothing that held a realm's values, order of work or neighbours.

These four issues are one change to how a room holds and tells its entities, so they share one design.

## Decision

**The World (#13274, built).** `metaverse-engine/core/world.coax` holds a realm's entities on the client, in the room and in the offline worker: the entity store, fixed columns keyed by slot (transform, body, health, kind, replication row), an ordered system list whose declared reads and writes are checked, and a spatial grid (`metaverse-engine/core/spatial.coax`). Caps are the realm's storage, taken whole or refused. Archetypes are not used.

**The room core (#13275).** A zone is one `World` sized by the zone's declared cap, stepped by one thread.

- A seat is an entity with a seat record beside its slot. Nothing indexes by a seat count.
- The shapes that carry the count today go: one-integer seat masks (an event's `ev_who`, a creature's `hurt_by`, interest's block words) become lists of entity slots or one bit a slot in a column of words; arrays `[EG_SEATS]` inside a seat (`heard_n`, `heard_w`, `n_t`, `w_t`) become rows of a per-recipient table sized by what interest lets the seat see; ids packed by 1,000 (`EG_TGT_SEAT`, `room-tick.c`) become `EntityId`s.
- The tick is the World's system list, in this order: input, movement, creatures, battle, effects, rows, interest, send. Each declares its components, so two zones never share a step and a later scheduler can run systems that do not meet.
- One zone to a process (settled 2026-11-12, #13275; this replaces "one zone to a thread"). A zone of 150 seats gets a room process to itself: a shard. A slow zone delays nobody else, because the kernel schedules each process on its own. See "Why a process, not a thread" below.
- The seats' number is one constant (`WIRE_SEATS`, with `EG_SEATS`, `RELAY_SEATS`, `IM_SEATS` and `EGB_SEATS` held equal to it by a check in `room.coax`; both hosts refuse to start when `room-world.h` says another). No integer holds a bit a seat any more: a set of seats is `metaverse-engine/core/slot-set.coax`, an event names the seat it is aimed at, and the table's slots after the players' are counted from the seats' number.
- A process runs 4 rooms (`EG_ROOMS`, 16 before): the directory gives a shard 2 (`verse.room_shards.rooms_cap`), and a 150-seat zone gets a shard with a cap of 1.
- The design number is 150 players a zone ([0003](0003-150-a-zone.md)); `WorldStore` holds 4,096 slots.

**Why a process, not a thread.** Issue #13275 allows either. The room's state is one record (`RmServer`) and every procedure takes it. A tick writes parts of it that every room shares: the relay's frame queue, the connection table, checkpoints, the item ledger's outbox, vitals, statistics and journeys. A thread a zone needs each of those split by zone or locked, and no `#test` or `coax mutate` run can show that two threads never meet in them. A process a zone needs none of that:

- The room VM already runs one process a shard (`oa-room@N`), each with its own loop and its own wait set (`room-wait.h`), and the directory already places an instance on a shard.
- A zone's crash, memory and scheduling policy are its own. The other zones keep their players.
- The same checks cover one process whatever the host runs beside it.

What it costs: each process holds a whole server record, and travel between zones always crosses processes (#13282 carries the character). A shard that runs one large zone reports one free simulation to the directory, not `EG_ROOMS`.

**What still holds the seats at 16** (found 2026-11-12 by setting the constants to 150; none is built yet):

- **The server record is compile-time storage.** Every room check declares a whole `RmServer`, and the compiler allows a check 33,554,432 value slots. The record uses 33.49 million of them at 16 seats, and with 150 seats it does not fit even with one room. A zone's seats, replicas, history and interest must become the zone's own storage, sized by its declared cap and handed to the room as views (as `RmServer.e2land` and the World's storage already are), so a check makes a zone of 16 and a zone process makes one of 150. The loops then run to the zone's cap, not to a constant.
- **The rules' creature table** holds 216 creatures a battle (16 seats, 96 creatures, familiars). It is the combat context's (#13285); 150 seats need it sized by the zone's cap too.
- **The wire.** `WIRE_SEATS`, the page's per-seat arrays and its 512-entity mirror change together, as a wire version, through the release mechanism of #13398.

**Rows (#13276).** Every replicated entity is a row of the session wire, from its World columns.

- A kind declares its fields once, in one table: which of the row's fields it uses and what each means. Delta, acknowledgement, priority, the link budget, interest and the block list come from the row layer.
- Monsters and the guard already stand in the battle as creatures. Their rows take a kind each (`WK_MONSTER`, `WK_GUARD`) in slots after the forest's, with state in `G`, the act in `CLIP`, the act's tick in `T0`, the foe as a target code in `OWNER`, and the monster's index and template in `P1` and `P2`. A new life is the row's `LIFE`.
- Familiars and arrows in flight are rows too. What happened (a blow, a fall, a shot loosed, a familiar's answer) is an `e` entry of the `D` frame on the reliable event class.
- `g2 g3 h2 h3`, `m1` to `m8`, `M1` to `M7`, `U1` and `u1` are deleted with their codecs, profiles and allowlist lines. Commands a player sends become `E` (use) kinds. The projectile frames `F a d` stay.
- The wire version rises (5 to 6). A wire 6 room still seats a wire 5 page for one release and writes it the four old frames ([the previous wire](../protocol/everglade-previous-wire.md), #13398); a wire 6 page and a wire 5 room never meet.
- **Built in two changes.** The guard and the monsters are on `main` at wire 6 (cd7854a3a5): `g2 g3 h2 h3` are gone, and what the two did are `e guard` and `e mon` entries. Familiars and the kit (`m1` to `m8`, `M1` to `M7`, `U1`, `u1`) follow at wire 7.
- **`e guard` and `e mon` are not in `WIRE_OUTCOMES`.** That list is the town's: a page that counts fewer of them asks for the town again (`O`). A creature's entry is kept another way (`egc_kept`): it has the outcomes' room in a tick's events, is outside the cue budget, and goes to the seats near and to the seat it names.
- **A player's arrow keeps `F a d`.** A launch with a deterministic flight is one frame at each end; a row a tick would cost more and be less exact. A monster's arrow is the `e mon` loosed entry.

**Prediction (#13277, engine half built).** `metaverse-engine/net/predict.coax` keeps a page's inputs by number, the room's ACK and the re-run, and a ledger of actions that wait for the room. `metaverse-engine/net/interp.coax` is the interpolation buffer, its clock and the easing of corrections. A realm supplies its step and its row's fields.

## Consequences

- **A wire change ships as one release set.** `coax/web/release.sh` ships the page alone, and production's rooms are promoted separately. The page is released often; a page that speaks wire 6 to a room that speaks wire 5 is refused, tells the player to reload, and is refused again, so nobody can play. `release-set ship` already stops a page whose pins no live shard runs, but it compares the engine, rules and content pins, not the wire version, and plain `release.sh` compares nothing. Both checks now read the wire version (#13387, [the release set](../deployment/release-set.md)): a page whose version a live shard does not speak is refused by `release.sh` and by `release-set ship`, and ships only with its rooms. The owner decided that rooms and page ship together as one release set, so #13276 may land on `main`: until the set ships, `release.sh` refuses the page and production keeps the page and rooms it has.
- The room's own creature histories on the page (`e2n_mon_at`, `e2n_guard_at`) go when their rows arrive: every entity is then drawn from the engine's one buffer.
- Orbit League's page predicts nothing today. It is the second realm for the predictor once its car is stepped on the page with the room's vehicle step.
- The creature runtime (#13293), navigation (#13281) and the binary wire (#13278) build on these: a creature is a World entity with a template, a path is a system, and the field table that names each kind's row fields is the one the binary encoder is generated from.
