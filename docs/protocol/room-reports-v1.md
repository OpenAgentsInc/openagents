# Room reports v1

A room shard and the API talk to each other outside the session wire. The
shard posts what it knows, and the API answers with a receipt. One file
writes and reads these shapes:
[`metaverse-engine/session/room-report.coax`](../../metaverse-engine/session/room-report.coax).

- Profile and fixtures: [`proto/verse/room-reports-v1.json`](../../proto/verse/room-reports-v1.json)
- Check: `coax/web/tool.sh coax/web/room/report-profile.coax proto/verse/room-reports-v1.json`
- Registry row: `room-reports-v1` (steps E7 and E8, [#13250](https://github.com/OpenAgentsInc/openagents/issues/13250))

Every post is signed. `X-Room-Mac` is the hex HMAC-SHA256 of the body under
the shard's write key (or the seat-ticket key), and the API takes a post
only within two minutes of its `t` line. All numbers are whole numbers in
decimal. Lines end with a line feed.

## Vitals

`POST /api/playtest/room-vitals`, text.

```text
t MS
o SHARD GEN BATCH
PID HP MAX XP WANTED REV AT [OWNER] [c=CLASS]
```

| Field | Meaning |
| --- | --- |
| `MS` | When the post was signed, wall-clock milliseconds |
| `SHARD`, `GEN`, `BATCH` | Who reports: the shard, its process's generation (its start, in milliseconds) and the post's number |
| `PID` | The player's ID |
| `HP`, `MAX`, `XP`, `WANTED` | The player's values |
| `REV`, `AT` | The values' revision, and when they changed |
| `OWNER` | The owner generation of the seat's ticket. Written when above 0 |
| `CLASS` | The class to keep for the character. Written when the seat chose one |

A line's operation ID is (`SHARD`, `GEN`, `PID`, `REV`). The API applies
each once.

The API answers 200 with a receipt:

```text
receipt GEN BATCH CODES
```

`CODES` has one letter for each line of the post, in order:

| Code | Meaning |
| --- | --- |
| `a` | Applied |
| `d` | Already applied (a retry) |
| `s` | Stale: a newer revision is kept |
| `u` | No such player |
| `x` | A line the API could not read |
| `f` | Fenced: a newer owner generation has the character |

Only a receipt marks a revision saved. An API from before receipts answered
`{"kept":N}`; the reader still takes it as every line kept.

## Directory

`POST /api/playtest/room-directory`, text.

```text
t MS
s SHARD FREE
g GEN [d]
e URL
r CODE PLAYERS PID PID ...
h PLAYERS TICK_P95_US
```

| Line | Meaning |
| --- | --- |
| `s` | The shard and how many simulations it has free |
| `g` | The process's generation. `d` while it drains or is out of rotation. Written when generations are on |
| `e` | Where pages open this shard's socket. Written by a shard with an endpoint |
| `r` | One line for each room: its code, how many sit in it, and the ID of each ticketed or held player |
| `h` | The shard's players and its tick's 95th percentile in microseconds. Written last, by a shard with an endpoint |

A drain request's MAC covers the text `drain SHARD AT WAIT`.

## Control

Every two seconds the shard asks `GET /api/playtest/room-control`. The
ask's MAC covers the text `t MS`, sent as `X-Room-At`. The answer is lines:

| Line | Meaning |
| --- | --- |
| `pvp 0` or `pvp 1` | Whether players may harm each other |
| `edit STAMP` | When the world or board last changed |
| `kick PID` | A kicked or banned player. One line for each |
| `flag NAME VALUE` | A flag the API holds for the rooms. Every answer carries the whole list |
| `hold CODE SEAT STAMP` | A scripted Hold Person on a seat, once for each stamp (checks only) |
| `place CODE SEAT BOAT STAMP` | A seat put beside a rowboat (a local room only) |
| `dice fixed` or `dice random` | Fixed dice for paired checks (a local room only) |

The `event` and `levels` lines in the same answer belong to
`coax/web/room/room-events.coax`. This codec reads them as `other`.

## Checkpoint receipt

`POST /api/playtest/room-checkpoint-v2` carries a character capture
([character-checkpoint-v3.md](character-checkpoint-v3.md)). The API answers:

```json
{"v":2,"status":"applied","owner":3,"process":1762190000000,"rev":5,"accepted_at":1762190001000}
```

| Member | Meaning |
| --- | --- |
| `v` | The checkpoint route's version |
| `status` | `applied`, `duplicate`, or a refusal's word (`stale`, `fenced`, ...) |
| `owner`, `process`, `rev` | The capture the API now keeps for the character |
| `accepted_at` | When the API accepted it. 0 on a refusal |

## Who writes and who reads

| Shape | Writer | Reader |
| --- | --- | --- |
| Vitals post, directory post, control ask | The room, through the codec | The API, in C (`player.c`, `rooms.c`) |
| Vitals receipt, checkpoint receipt | The API, through the codec (`ticket-sign.coax`); the room's local stand-in for the API, through the codec | The room, through the codec |
| Control answer | The API, in SQL (`player.c`) | The room, through the codec |

The API's C readers and the SQL writer are not on the codec yet. Their
allowlist lines (`route coax/web/api/player.c`, `route coax/web/api/rooms.c`)
say so.

## What the check holds

- For each entry, the codec's writer gives the entry's exact text from its
  values.
- The receipts, control answers and checkpoint receipts are read back
  through the codec to the same values.
- Each reader refuses its `refused` texts.

The fixture says where each entry came from: posts a room wrote, and
receipts and control answers that a running API gave.
