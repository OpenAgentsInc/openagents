# Character checkpoint profile 3

Profile 3 is profile 2 (`character-checkpoint-v2.md`) plus the character's rest,
so a restarted or drained Everglade II room resumes a Short or Long Rest where
it was (#13046). Schema: `proto/verse/character-checkpoint-v3.schema.json`.
Codec: `aethyr/session/character-checkpoint.coax`. Rest fields:
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

A profile 2 body never has `rest` or `uses`. A profile 3 body always has
`rest`. The ingress body has 22 members, or 23 with `uses`. The stored body has
21, or 22 with `uses`.

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
