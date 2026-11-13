# Moderation v1

Bans, mutes, kicks and a moderator's commands, with a log that no one can
edit (GitHub #13302, AAA P5.8).

The rules and the one codec are
[`metaverse-engine/session/moderation.coax`](../../metaverse-engine/session/moderation.coax). A
room runs them, and the API calls them through
[`coax/web/api/moderation-policy.coax`](../../coax/web/api/moderation-policy.coax).
The fixtures are [`proto/verse/moderation-v1.json`](../../proto/verse/moderation-v1.json),
checked by
`coax/web/tool.sh coax/web/room/moderation-profile.coax proto/verse/moderation-v1.json`.
The tables are in `coax/web/api/migrations/0170_moderation.sql`. SQL stores
rows and decides nothing.

## Who is a moderator

A moderator is one of the realm's admins. In a room, that is a seat whose
signed seat ticket has ACCT 2
([seat ticket v5](seat-ticket-v5.md)). Over the API, it is a caller for whom
`verse.edit_may('everglade', ...)` holds.

A moderator can be sanctioned like any player. A moderator can't act on
themselves, so no moderator lifts their own ban, mute or kick. A moderator
may look at themselves.

## The record

One line, fields one space apart:

```text
VERB PID MINUTES REASON
```

| Field | Meaning |
| --- | --- |
| `VERB` | `look`, `move`, `kick`, `mute`, `ban`, `unkick`, `unmute` or `unban` |
| `PID` | The player it is about: 1 to 40 of `[a-z0-9-]` |
| `MINUTES` | How long a mute or ban lasts, 1 to 525600. `0` means no end, and is the only value for every other verb |
| `REASON` | 1 to 120 printable ASCII bytes with no space first or last, or `-` for none. A kick, mute or ban needs a reason |

A reader takes a line only when it is exactly what the writer writes.

## What each verb does

| Verb | Effect | Where it is enforced |
| --- | --- | --- |
| `look` | The moderator is told the player's seat, Hit Points and place | The room |
| `move` | The player is put where the moderator stands | The room |
| `kick` | No seat ticket and no chat for 24 hours | The seat ticket, chat, and the rooms' control poll |
| `mute` | No chat line and no inbox message | `POST /api/playtest/chat/ROOM` and `POST /api/playtest/inbox` answer 403 `{"error":"muted","until":MS}` |
| `ban` | No seat ticket, no chat line and no inbox message | `POST /api/playtest/room-ticket/ROOM` answers 403 `{"error":"banned","until":MS}`. The control poll's `kick PID` lines drop a seated player within 2 seconds |
| `unkick`, `unmute`, `unban` | Lifts it | The same places |

`until` is `0` for a ban or mute with no end. A ban and a mute are kept by
the account, or by the player when there is no account. A kick is kept by
the player.

## In a room

A moderator's seat sends `X ` and the record:

```text
X ban p-0123456789abcdef 0 spam in chat, twice
```

The room answers that seat only:

```text
q ok VERB PID 0 DETAIL
q no VERB PID 0 DETAIL
```

`DETAIL` says what was seen or why not. A line from a seat that is not a
moderator's does nothing, and the seat is told nothing.

The room takes an action only after its line is in the log's outbox. When
the outbox is full (32 actions with no receipt), the room refuses the
command with `q no ... the log is full: try again`. So no action goes
unlogged.

## The room's report

The room posts its outbox to `POST /api/playtest/room-mod-log`, signed as a
[room write](room-write-v1.md) with the purpose `moderation`:

```text
t MS
o SHARD GEN BATCH
SEQ BY ROOM RECORD
...
```

| Field | Meaning |
| --- | --- |
| `MS` | When it was signed. The API takes it within two minutes |
| `SHARD GEN BATCH` | The shard, its process's generation, and the post's number |
| `SEQ` | The action's number in this process |
| `BY` | The moderator's player ID |
| `ROOM` | The room's code |

The operation ID of a line is `(SHARD, GEN, SEQ)`. The API logs each one
once, and sets or lifts the kick, mute or ban in the same statement. The
answer is `receipt GEN BATCH`. An action leaves the outbox only on that
receipt. Anything else is a failure, and the room sends the post again
after a backoff.

## Over the API

`POST /api/playtest/room-mod` takes the record as the body, from one of the
realm's admins. `look` and `move` are a room's and are refused here.

| Answer | Meaning |
| --- | --- |
| 200 `{"ok":true,"verb":V,"player":PID,"until":MS}` | Done and logged |
| 400 `{"error":"not a record"}` | Not a record, or one the rules refuse. Nothing is logged |
| 403 `{"error":"Everglade's admins only"}` | The caller is not a moderator. Nothing is logged |
| 403 `{"error":"not on yourself"}` | Logged with the outcome `self` |
| 404 `{"error":"no such player"}` | Logged with the outcome `no_player` |

`POST /api/playtest/room-kick` keeps its JSON (`{"player":PID}`, and
`"on":false` to let them back). It is the same action as `kick` or `unkick`
and is logged the same way. A report's HIDE and RESTORE of a player are
logged too.

## The log

`verse.mod_log` has one row for each action: who (`by_who`), what (`verb`),
whom (`whom`), why (`reason`), when (`at_ms`), and where it came from
(`source`, and a room's `room`, `shard`, `gen`, `seq`). `outcome` is `ok`
when it was done, else why not. `team` marks an action on one of the team's
players, so counts can leave it out.

The table is append-only. `UPDATE`, `DELETE` and `TRUNCATE` raise an error
for every role, the table's owner included.

One of the realm's admins reads it with
`GET /api/playtest/room-mod?player=PID`: the newest 200 actions about that
player (or about anyone, without `player`), oldest first, as
`{"log":[{n, at_ms, by_who, verb, whom, minutes, until_ms, reason, source, room, shard, gen, seq, outcome, team}]}`.
Anyone else gets 403.

To read it in SQL:

```sql
SELECT n, at_ms, by_who, verb, minutes, reason, source, room, outcome
FROM verse.mod_log WHERE whom = 'p-0123456789abcdef' ORDER BY n;
```
