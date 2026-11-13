# Character checkpoint profile 3

Profile 3 is profile 2 (`character-checkpoint-v2.md`) plus the character's rest,
so a restarted or drained Everglade II room resumes a Short or Long Rest where
it was (#13046). Schema: `proto/verse/character-checkpoint-v3.schema.json`.
Codec: `metaverse-engine/session/character-checkpoint.coax`. Rest fields:
`rules/srd/rest-carry.coax`.

## Body

A profile 3 body has every profile 2 member with profile 2's meaning and range,
except `v` is 3, and one more required member: `rest`, an array of exactly 19
integers, each from 0 to its bound:

| # | Field | Bound |
|---|-------|-------|
| 0 | Short Rest phase: 0 none, 1 resting, 2 choosing Hit Dice | 2 |
| 1 | Short Rest age in ms (phase 1 only) | 3600000 |
| 2 | Hit Dice left (when seeded) | 20 |
| 3 | Hit Dice seeded | 1 |
| 4 | Short Rest recovery used | 1 |
| 5 | Restoration used | 1 |
| 6 | Long Rest phase: 0 none, 1 open, 2 interrupted, 3 pending | 3 |
| 7 | Long Rest mode: 1 sleep, 2 watch, 3 exertion (0 when phase is 0) | 3 |
| 8–11 | Long Rest elapsed, sleep, light and exertion time in ms | 86400000 |
| 12 | Interruptions | 99 |
| 13 | Quiet time in ms | 86400000 |
| 14 | Fallback | 1 |
| 15 | A Long Rest completed | 1 |
| 16 | Age of that completion in ms, capped at 16 hours | 57600000 |
| 17 | A Short Rest is due | 1 |
| 18 | Light activity reported | 1 |

A profile 3 body may also have `uses`: an array of exactly 3 integers, each
from 0 to 99. These are the limited-use counts after profile 2's 11 in `state`,
in the rules' order (`RS_USES` 11 to 13): Uncanny Metabolism, Magical Cunning
and Faithful Steed. A room writes `uses` with every profile 3 body. A body
without it, from an older room, still reads, and those three uses start full.

A profile 3 body may also have `combat` (#13282): what a fight left on the
character, so that it crosses between zone owners and comes back after a
restart. It is an array of 5 numbers, then 8 for each lasting effect, with at
most 16 effects (5 + 8k numbers, 133 at most).

| Index | Meaning | High |
| ---: | --- | ---: |
| 0 | Room ticks (30 a second) until its turn's beat ends | 60 |
| 1 | The parts of the turn it has spent: 1 Action, 2 Bonus Action, 8 a slot's spell | 15 |
| 2 | Room ticks since its last press | 15 |
| 3 | 1 + the spell it concentrates on (0: none) | 1000 |
| 4 | That spell's ticks left (0: no clock ends it) | 2592000000000 |

Each effect, in the order the body holds them:

| Offset | Meaning | High |
| ---: | --- | ---: |
| 0 | The Aura's kind (the rules' `EF_*`), 1 at least | 40 |
| 1 | Its `cond`: a condition mask, or the kind's own number | 2147483647 |
| 2 | 1 when the character is its own source, 0 when its source stayed behind | 1 |
| 3 | 1 + its spell (0: none) | 1000 |
| 4 | Its ticks left (0: no clock ends it) | 2592000000000 |
| 5 | 1 + the ability of the save it repeats (0: none) | 6 |
| 6 | That save's DC | 99 |
| 7 | 1 when the character's own turn's end ends it | 1 |

An effect's and a spell's tick is a microsecond of the battle's clock. The
effects a room leaves out are the ones that belong to the world left behind: a
zone's hold, a readied response, a helper's distraction, a command still to
obey, natural sleep (the rest carries it) and a failed Influence. The spell the
character concentrates on goes only when one of its own carried effects is that
spell's. `rules/srd/aura-carry.coax` owns these rules; the turn's three numbers
are `everglade/shared/cast-pace.coax`'s.

A profile 2 body never has `rest`, `uses` or `combat`. A profile 3 body always
has `rest`. The ingress body has 22 members, and one more for each of `uses`
and `combat` it has. The stored body has one fewer.

The fullest body (every number at its high, 16 effects) is under 2,048 bytes,
also as the store returns it, with a space after every `:` and `,`.

## Exchange

Profile 3 uses profile 2's endpoint (`POST /api/playtest/room-checkpoint-v2`),
table, receipts and ownership fences. Its receipt is profile 2's `{v:2,...}`.
Migration `0136_character_checkpoints_v3.sql` lets the table hold both bodies.

A page asks for its ticket with `checkpoint=2&rest=1`. The API then puts the
stored body in the v4 ticket as it is. Without `rest=1`, the API gives a
profile 3 body's profile 2 part: `v` 2, with no `rest` and no `uses`. An older page and an
older room therefore never see a profile 3 body. An API that predates this
ignores `rest=1`, and since it refuses profile 3 bodies, it holds only
profile 2 ones.

An Everglade II room writes profile 3 for each seat. If the API answers a
profile 3 post with 400 or 5xx, the room sends that capture and every later one
as profile 2 for the rest of its process (`two_only`, counted in `two_falls`).
So rooms and the API can deploy in either order. Rolling the API back after it
has stored profile 3 bodies hands them to pages as they are, and only a room
with profile 3 can read them, so roll back rooms and pages first. A room or API
from before `uses` refuses a body that has it: such an API answers 400, and the
room falls back to profile 2 as above. A room rolled back to before `uses`
treats a stored body that has it as no capture.

## Restore

A room lays a body's `uses` on the restored body, each at most that body's
own maximum, whether or not its rest fits.

A room lays a body's `combat` on the restored body first, all or none
(`rs_auras_in`): the effects against its own battle clock, then the turn
against its own tick. A list the rules refuse (a kind that does not carry, more
effects than the body has room for, a Concentration none of its effects is
held by) leaves the body with no effect and a fresh turn. The same ticket shown
twice takes the one seat over; it never builds a second body from one capture.

A room writes `combat` exactly as it stands when a journey to another room
starts and when the room shuts down. Between those, a character is not written
again only because an effect's clock ran, so a restore after a crash may give
an effect back the ticks it had when the character last changed.

An API from before `combat` answers 400 to a body that has it. The room then
writes profile 3 without `combat` (`plain`), and only if that is refused too
does it fall back to profile 2. A ticket for a page that asks profile 2 gets
the body without `rest`, `uses` and `combat`.

A room lays a profile 3 rest on the restored body with `rs_carry_in`, all or
none, after profile 2's restore. Offline time earns no rest: an open rest
resumes with its counts as captured, stamped at the restore. A pending
completion resumes as the open rest it ended, and the scheduler completes it
again. A sleeping open rest puts the body back to sleep. An interrupted one
stays interrupted until the player asks again. The completion age keeps the
16-hour gate between Long Rests, and the room's rest clock starts at
`RS_CARRY_EPOCH` (17 hours) so that age is always representable. Hit Dice and
the once-per-rest recovery flags carry exactly; more Hit Dice than the
restored body has refuses the rest. A body that is dead or at 0 HP keeps its Hit Dice and history but no
rest in progress. A rest that does not fit the body (out of range,
inconsistent, or a body that cannot sleep) is dropped, and the character
starts with no rest in progress.
