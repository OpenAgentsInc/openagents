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
- A room's seats are one constant (`WIRE_SEATS`, with `EG_SEATS`, `RELAY_SEATS`, `IM_SEATS` and `EGB_SEATS` held equal to it by a check in `room.coax`), and the most a zone declares another (`WIRE_ZONE_SEATS`, 150, with `EG_ZONE_SEATS` and the rest held equal; both hosts refuse to start when `room-world.h` says another). No integer holds a bit a seat any more: a set of seats is `metaverse-engine/core/slot-set.coax`, an event names the seat it is aimed at, and the table's slots after the players' are counted from the seats' number.
- A process runs 4 rooms (`EG_ROOMS`, 16 before): the directory gives a shard 2 (`verse.room_shards.rooms_cap`), and a 150-seat zone gets a shard with a cap of 1.
- The design number is 150 players a zone ([0003](0003-150-a-zone.md)); `WorldStore` holds 4,096 slots.

**Why a process, not a thread.** Issue #13275 allows either. The room's state is one record (`RmServer`) and every procedure takes it. A tick writes parts of it that every room shares: the relay's frame queue, the connection table, checkpoints, the item ledger's outbox, vitals, statistics and journeys. A thread a zone needs each of those split by zone or locked, and no `#test` or `coax mutate` run can show that two threads never meet in them. A process a zone needs none of that:

- The room VM already runs one process a shard (`oa-room@N`), each with its own loop and its own wait set (`room-wait.h`), and the directory already places an instance on a shard.
- A zone's crash, memory and scheduling policy are its own. The other zones keep their players.
- The same checks cover one process whatever the host runs beside it.

What it costs: each process holds a whole server record, and travel between zones always crosses processes (#13282 carries the character). A shard that runs one large zone reports one free simulation to the directory, not `EG_ROOMS`.

**A zone's storage (#13275, built).** A room's seats are a view (`EgRoom.seat`), and every loop over them runs to its count (`eg_cap`), not to a constant.

- A room handed no storage has its own 16 seats (`seat_own`, bound at the server's start), its battle's own creature table (`RS_MAX`) and interest's own rows. Its frames are the bytes they were: `coax/remote/check.sh room PROD` holds that against production.
- A zone of more declares its cap once, before any room opens (`OA_ROOM_ZONE_SEATS=N`, 17 to 150: `room-host.c zone_make`, `everglade/server/seats.coax eg_zone_use`). The host makes storage sized by that cap and hands it in as views: the seats (each with its replica of the table), the battle's creatures and squares (through the rules' `rs_use`: a creature for each seat, each summoned creature and each seat's familiar), and interest's per-seat rows (`im_use`). It is taken whole or refused. The process then opens room 0 only and reports one free simulation.
- Seat-keyed tables that are small stay in the record with room for the most a zone declares (`EG_ZONE_SEATS`, 150): a seat's `heard_n` and `n_t` rows, the room's tickets, WANTED levels and name entries, the battle's `seat`, `live` and `familiar`, interest's lists. The process's own per-seat tables (`EG_SEAT_ROWS`: stats, each seat's character) hold 150 rows: four rooms of 16, or the one zone.
- The table (`WireHist`) holds `slots` rows: the layout's `WIRE_SLOTS`, and a zone's seats past the 16th after its last slot (`wire_seat_slot`), so a room of 16 has the slots it always had. In the battle, summoned creatures start after the seats (`EgBattle.seats`, `egb0`), and the familiars after those.
- Checks: `everglade/server/zone-checks.coax` (150 sit in one zone and the 151st is refused; a zone of 40 is stepped and told for ticks, a seat past the 16th holding itself and its neighbours). A check's budget holds a tick or two of 150 seats, so 150 seats' ticks are measured on a build host with `bots.coax --zone`.

**What a 150-seat zone still needs before players reach one** (none changes a room of 16):

- **The relay's seat bound (built).** A SEAT was below 16 in the relay's contract; it is below 150 in version 2 (`proto/verse/room-relay-v2.json`, `RLW_SEATS`), which reads every version 1 frame. A player's row id is its seat, and the page keeps players by id, so a page in a zone holds the seats past the 16th that interest sends it.
- **The directory.** `verse.room_max()` and the API's `ROOM_SEATS` place 16 players an instance; a zone's shard needs its cap there, and `rooms_cap` 1.
- **The rules' zones.** `RS_ZONES` is 8 a battle: one concentration area a caster is too few for 150 (#13285).

**The seats are not World entities yet (#13274, #13275).** A seat's place is `EgSeat.pose`, read and written in 84 lines of 23 files (the host's `rm_eg_moved`, the checks' `e2mt_at`, `e2gt_at`, `wpt_at`, `encounter-sim.coax`'s `es_place`). The World's columns hold a transform and a body; a seat's grounded bits, clip and carry are not columns, and its Hit Points are the rules' (`RsCreature.hp`), not the World's health column. Moving the place alone into the World would leave a seat in three records, and the creatures it fights (monsters, the guard, familiars) in none, so no system could run from the World's columns: interest and the table would still read the old records. Seats become entities together with the creatures (#13293) and with the rules reading health from the World; until then there is one copy, `EgSeat.pose`, and no World in the room.

**Rows (#13276).** Every replicated entity is a row of the session wire, from its World columns.

- A kind declares its fields once, in one table: which of the row's fields it uses and what each means. Delta, acknowledgement, priority, the link budget, interest and the block list come from the row layer.
- Monsters and the guard already stand in the battle as creatures. Their rows take a kind each (`WK_MONSTER`, `WK_GUARD`) in slots after the forest's, with state in `G`, the act in `CLIP`, the act's tick in `T0`, the foe as a target code in `OWNER`, and the monster's index and template in `P1` and `P2`. A new life is the row's `LIFE`.
- Familiars and arrows in flight are rows too. What happened (a blow, a fall, a shot loosed, a familiar's answer) is an `e` entry of the `D` frame on the reliable event class.
- `g2 g3 h2 h3`, `m1` to `m8`, `M1` to `M7`, `U1` and `u1` are deleted with their codecs, profiles and allowlist lines. Commands a player sends become `E` (use) kinds. The projectile frames `F a d` stay.
- The wire version rises (5 to 6). A wire 6 room still seats a wire 5 page for one release and writes it the four old frames ([the previous wire](../protocol/everglade-previous-wire.md), #13398); a wire 6 page and a wire 5 room never meet.
- **Built in two changes.** The guard and the monsters are on `main` at wire 6 (cd7854a3a5): `g2 g3 h2 h3` are gone, and what the two did are `e guard` and `e mon` entries. Familiars and the kit follow at wire 7: a familiar is a `WK_FAMILIAR` row, each request an `E` kind (9 to 15), each answer an `e fam`, `e fam-cast` or `e kit` entry told to the asker alone ([the profile](../protocol/everglade-familiar-v2.md)).
- **State is a row, never an entry.** Whose senses a familiar's owner has was a frame of its own (`m8`), resent each second in case one was lost. It is state, so it is a field of the familiar's row (`G`) and the row layer delivers it.
- **A wire 7 room still speaks wire 6 for one release ([the previous wire](../protocol/everglade-previous-wire.md), #13398).** To a seat whose hello said wire 6 the room sends no familiar's row, says each `e fam`, `e fam-cast` and `e kit` entry as the old `m1`, `m3` to `m7` and `u1` frame, sends `m2` and `m8` beside `D`, and reads its `M1`, `M3` to `M7` and `U1`. Those writers are `everglade/server/wire-prev.coax`, and they go when the wire rises again.
- **`e guard` and `e mon` are not in `WIRE_OUTCOMES`.** That list is the town's: a page that counts fewer of them asks for the town again (`O`). A creature's entry is kept another way (`egc_kept`): it has the outcomes' room in a tick's events, is outside the cue budget, and goes to the seats near and to the seat it names.
- **A player's arrow keeps `F a d`.** A launch with a deterministic flight is one frame at each end; a row a tick would cost more and be less exact. A monster's arrow is the `e mon` loosed entry.

**Prediction (#13277, engine half built).** `metaverse-engine/net/predict.coax` keeps a page's inputs by number, the room's ACK and the re-run, and a ledger of actions that wait for the room. `metaverse-engine/net/interp.coax` is the interpolation buffer, its clock and the easing of corrections. A realm supplies its step and its row's fields.

## Consequences

- **A wire change ships as one release set.** `coax/web/release.sh` ships the page alone, and production's rooms are promoted separately. The page is released often; a page that speaks wire 6 to a room that speaks wire 5 is refused, tells the player to reload, and is refused again, so nobody can play. `release-set ship` already stops a page whose pins no live shard runs, but it compares the engine, rules and content pins, not the wire version, and plain `release.sh` compares nothing. Both checks now read the wire version (#13387, [the release set](../deployment/release-set.md)): a page whose version a live shard does not speak is refused by `release.sh` and by `release-set ship`, and ships only with its rooms. The owner decided that rooms and page ship together as one release set, so #13276 may land on `main`: until the set ships, `release.sh` refuses the page and production keeps the page and rooms it has.
- The room's own creature histories on the page (`e2n_mon_at`, `e2n_guard_at`) go when their rows arrive: every entity is then drawn from the engine's one buffer.
- Orbit League's page predicts nothing today. It is the second realm for the predictor once its car is stepped on the page with the room's vehicle step.
- The creature runtime (#13293), navigation (#13281) and the binary wire (#13278) build on these: a creature is a World entity with a template, a path is a system, and the field table that names each kind's row fields is the one the binary encoder is generated from.
