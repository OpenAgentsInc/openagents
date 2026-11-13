# Everglade session profile v7

Version 7 makes Everglade II's familiars rows of the existing entity
replication path, their requests and the Healer's Kit's `E` (use) kinds, and
the room's answers `e` entries of `D` (#13276, #13245). Its machine-readable
contract is
[`everglade-session-v7.json`](../../proto/verse/everglade-session-v7.json).
[Version 6](everglade-session-v6.md) did the same for the monsters and the
path guard, and [version 5](everglade-session-v5.md) added the room's forest
state. All existing message fields and units keep their meanings.

Pages use version 7. Rooms use version 7 with oldest version 6, and build
pins 1.1.1: a room still seats a version 6 page for one release, so rooms can
be promoted before the page ([the previous wire](everglade-previous-wire.md),
#13398). To a version 6 page the room speaks the version 6 words: it reads
`M1`, `M3` to `M7` and `U1`, writes `m1` to `m8` and `u1`, and sends it no
familiar's row. The room picks by each seat's hello. A version 5 page
receives `r 0 version` and reloads. A version 7 page never
ships ahead of its rooms: see
[the release set](../deployment/release-set.md).

## Creature rows

A kind declares the row fields it uses once, in the codec's table
(`wire_kind_fields`). A field outside a kind's list is never written. Delta,
acknowledgement, priority, the link budget, interest and the block list are
the row layer's, as for every row.

| Kind | Number | Row id | Fields beside the pose |
| --- | --- | --- | --- |
| Monster | 13 | `200000 + 970000 + M`, M from 0 to 11 | `g` its state, `clip` its act under way (1 a blow, 2 a shot, 0 none), `t0` the tick that act began (the tick it fell, when dead; 0 none), `owner` its foe as a target code (`1000 + SEAT`, -1 none), `p1` M, `p2` its kind (0 a wolf, 1 a goblin), `hp`, `max` |
| Guard | 14 | `200000 + 970100` | `g` his state, `t0` the tick of his last swing (-1 none), `owner` his foe, `hp`, `max` |
| Familiar | 15 | `200000 + 970200 + SEAT`, its owner's seat | `owner` its owner as a target code (`1000 + SEAT`), `p1` its form (an index into the SRD's familiar forms), `hp`, `max`, `g` whose senses its owner has (0 their own, 1 its sight, 2 its sight and hearing) |

The pose is `kind x y z yaw vx vy vz` with `life` and `warp`. A monster that
is gone has no row, and an `x` for its id means it left your view. A new
`life` is a new monster or guard in the same place in the table: a page draws
nothing between the two.

A familiar that is pocketed, released or dead has no row. Its row has no
velocity: a step is one square at once. It rides on its owner's seat in
interest management, so its owner always hears it, and a page that blocks
its owner never does. [The familiar words](everglade-familiar-v2.md) lists
the requests and the answers.

What a monster or the guard did, or had done to it, is an `e mon` or
`e guard` entry of `D`:
[Everglade II guard and monsters v2](everglade-guard-monsters-v2.md).

The frames `g2`, `g3`, `h2` and `h3` of version 5 are gone.


## Trees are state

Kind 26 identifies a tree record. Its entity ID is 1,162,000 plus its sparse
record slot (1–191). `CLIP` identifies the planted tree; record reuse advances
`LIFE` and starts a new interpolation history. The room publishes each occupied record,
including burnt trees after their fire goes out. The 512-entry entity table
reserves the original slots for creatures, abilities, boats and doors and a
separate 192-slot forest range. No cast loses capacity to a tree.

The JSON's `forest.fields` defines this kind's row fields. Tilt, break
height, resting angle, moving base and extra foot lift determine the same
trunk, stump and collision shape on every page. Soot, leaf loss and frost
determine its appearance. The burning bit, starter, age and lifetime let
pages draw its fire without simulating damage or spread.

Tree rows use the existing whole-tick ACK, absolute/delta baselines,
retransmission, LIFE/WARP and acknowledged removal rules. Every seat receives
the forest state, including late joiners. Rebuild removes the rows; slot
reuse advances the record's life; a pending removal completes before a new
life is sent. A planted tree can move to another record without an old
record's removal erasing it. Byte budgets remain
unchanged. Trees use the same priority and overdue-row service as other
entities; qualification checks full-forest snapshot delivery.

The room advances tree motion and fire once per world step. A page in a room
only interpolates the room's tree transforms and draws their fire. It never
spreads tree fire or advances tree damage locally.

## A cast's view (#13289)

`C SEQ SLOT AX AY AZ TGT [MODE [VIEW]]`. `VIEW` is the moment of the room's
world the page was drawing the others at, in ms of the room's ticks, as an
arrow's `F` says it ([`everglade-projectiles-v1.md`](everglade-projectiles-v1.md)).
The room judges the cast there: who the aim names and where every other body
stands for the rules' reach, no more than 250 ms back and never ahead. A `C`
with no `VIEW` is judged at the present, so a wire 6 page is unchanged. When
`VIEW` is sent, `MODE` is sent too (0 none). The fairness rule is section
6.1.1 of the multiplayer specification.

## Check

```sh
coax/web/tool.sh apps/openagents-web/everglade-wire-profile.coax proto/verse/everglade-session-v7.json
```

The independently written tree fixture checks every forest field through
the ordinary row writer and reader. Room and page suites cover burn-down,
starter attribution, snapshots, delayed delivery, rebuild and record reuse.

## The seat ticket

A page joins a room with a seat ticket from
`POST /api/playtest/room-ticket/ROOM` (`?t=TICKET`). The ticket has one
writer, [`metaverse-engine/session/seat-ticket.coax`](../../metaverse-engine/session/seat-ticket.coax),
for every version: `seat_ticket_v5_write` writes version 5
([seat-ticket-v5](seat-ticket-v5.md)), and `seat_ticket_hmac_write` writes
versions 2 and 3 and the version 4 envelope around them, the HMAC tickets the
API signs while it has no version 5 signing key (#13248). The API
(`coax/web/api/player.c`) calls both through
`coax/web/api/ticket-sign-unit.c` and builds no part of a ticket itself. The
room reads every version with `tk_check_keys` in
`coax/web/room/room-ticket.coax`.

The writer's bytes are fixed by fixtures that the writer and the room's
reader both check:

```sh
coax/web/tool.sh coax/web/room/seat-ticket-profile.coax proto/verse/seat-ticket-v5.json
coax/web/tool.sh coax/web/room/seat-ticket-hmac-profile.coax proto/verse/seat-ticket-hmac-v1.json
```

Moving the version 2 to 4 writer from C to Coax changed no byte: the C
writer and `seat_ticket_hmac_write` gave the same bytes for the fixtures and
for 200,000 seeded inputs before the C writer was removed, so rooms and the
API deploy in either order.

## Everglade II rest messages (#13046)

Two additive messages run only in Everglade II rooms; older pages ignore `z`
and older rooms ignore `Z`, so the version and pins are unchanged.

- `Z SEQ REQ` (page): a rest request. REQ is 1 Short Rest, 2 sleep (starts or
  resumes a Long Rest), 3 watch, 4 spend one Hit Die, 5 end. The room admits a
  start only from its own last stepped input of that seat: grounded, still,
  keys released, no queued input. Refusal is `r SEQ rest`.
- `z SEQ CODE DICE` (server, to that seat only): the rest the room's shared
  rules hold. SEQ is the request it answers, 0 for a change the rules' clock
  made. CODE is 0 none, 1 Short Rest, 2 Hit Dice choice, 3 asleep, 4 watch,
  5 exertion, 6 interrupted, 7 completion pending; DICE is the Hit Dice left.
  It is sent on each change and again on each new socket.

Everglade's whole-heal `L SEQ` is refused (`r SEQ rest`) in Everglade II
rooms. Asleep, the room zeroes the seat's walk and jump input.

## Frames moved into the codec (#13364)

These frames were written or read by hand on one side. Since E3b each has a
writer and a reader in `everglade/shared/everglade-wire.coax`, the room and
the pages call them, and the profile pins them. No byte on the wire changed.

- `e xp SS AMOUNT`, `e level SS LEVEL` (server, aimed at that seat): XP a
  seat earned and its class's level gone up (`everglade/server/progress.coax`).
- `w SS LEVEL` (server): a seat's WANTED level (`everglade/server/send.coax`).
- `e rc ID K AX AY AZ SEED [FX FY FZ]` (server): a raid creature's cast, aimed
  at A in cm; F, when said, where it leaves the caster's hands.
- `e wm KIND CASTER FOE AX AY AZ BX BY BZ FLIGHT LIFE` (server): a guard's
  spell in flight from A to B (cm), FLIGHT and LIFE in ms.
- `e mend CODE`, `e pburn I ON`, `e pgone I` (server, outcomes): a Repair
  stood up; a placed thing alight or out; burnt through.
- `g KIND PHASE WAVE WAVES SECS LEFT STANDING SLAIN SQUARE WARDS ARCH OUTCOME
  FOLK LEVELS CAP [PLAYERS [FRONT TOWN CAPC CAPT]]` (server, a `D` entry): the
  raid's state. The top-level `g CODE` frame is travel's.
- `v SS LEVEL XP CAP LO HI [REST]` (server, a `D` entry): your level and XP.
- `V LOOK KIND` (page): Change character (`everglade/server/input.coax`).
- `X VERB [ARG]`, `X at XMM ZMM YAW` (page): admin lines. Only a room started
  with `--dev` on 127.0.0.1 reads them (`everglade/server/admin.coax`).
- `K PID PID ...` (page): the room now reads it with `wire_read_blocks`
  (`coax/web/room/room-blocks.coax`).

The rooms that write `e rc`, `e wm`, `e mend`, `e pburn`, `e pgone`, `g` and
`v` are Everglade v1's, frozen at `everglade/server/legacy/v1.sh`'s pin; the
v1 page still reads them, through the codec.
