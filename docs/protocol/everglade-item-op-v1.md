# Item operation v1 (`everglade.item-op.v1`)

How a room changes what a character has: loot, quest progress and rewards, and
what the character wears. Each change is one operation through the item ledger.
Tracked in GitHub #13291 (AAA P4.2) and #13292 (AAA P4.3).

- Schema and fixtures: [`proto/verse/everglade-item-op-v1.json`](../../proto/verse/everglade-item-op-v1.json)
- Codec: `everglade/model/item-op.coax` (`#authority`)
- Rules: `aethyr/session/ledger.coax` (the ledger) and
  `everglade/model/character.coax` `char_claim` (the record), both `#authority`
- Room: `coax/web/room/room-items.coax`
- Route: `POST /api/playtest/room-items` (`coax/web/api/player.c` `room_items`),
  signed as a [room write](room-write-v1.md) with purpose `items`

The server is the authority. A page never writes an item, coin, a quest's stage
or a reward: it sends an ask to its room (`E SEQ KIND ID`), and the room decides.

## The operation

```json
{"id":"s3-1762190000-7","op":"grant","pid":"p-a","gen":4,"ver":2,"item":null,"to":"",
 "kind":3,"sent":1762190000000,"quest":1,"from":8,"stage":9,"coin":25,"xp":100,"slot":-1,"wear":0}
```

| Field | Meaning |
| --- | --- |
| `id` | The operation's ID, 1 to 80 bytes. A room writes `sSHARD-PROCESS-COUNT`. The ledger applies an ID once |
| `op` | `read`, `grant`, `consume` or `claim` (and the ledger's `transfer`, `escrow`, `settle`, `cancel`, which no room sends yet) |
| `pid` | The player |
| `gen` | The room's owner generation for the player, from the seat ticket |
| `ver` | The inventory version the room last heard |
| `item` | The item the operation names, or `null` |
| `to` | Always empty from a room |
| `kind` | A grant's content kind: an `item` row's ID in the [content tables](aethyr-content-v1.md) |
| `sent` | When the operation was first sent, in milliseconds. A retry keeps it |
| `quest`, `from`, `stage` | The claim's quest part: move quest `quest` from stage `from` to stage `stage`. `quest` 0 means none |
| `coin`, `xp` | Coin and XP the claim adds |
| `slot`, `wear` | Wear item `wear` in equipment slot `slot` (0 to 9). `wear` 0 empties the slot. `slot` below 0 means none |

| Operation | What it does |
| --- | --- |
| `read` | Answers the character as the store has it. Nothing changes and nothing is recorded |
| `grant` | Makes a new item of `kind` for the player |
| `consume` | Uses up the player's item `item`. An escrowed item can't be consumed |
| `claim` | Changes no item. Only the claim on the record applies |

## The claim

Every operation but `read` can carry a claim on the character's
[record](everglade-character-v1.md). The API applies the claim in the same
transaction as the operation, or applies neither.

- **Quest stage.** The claim names the stage the quest is at (`from`) and the
  next one (`stage`). If the quest isn't at `from`, the claim is refused. A stage
  only rises. So a reward sent twice, or sent under a new ID after a day, pays
  once.
- **Coin and XP.** Coin can't fall below 0. XP can't fall. Neither passes the
  record's range.
- **Worn.** The item must be in the bag. An item worn elsewhere moves to the
  slot.

A refused claim answers `held`, and nothing of the operation is kept.

## The answer

```json
{"status":"ok","item":12,"replay":false,"ver":5,"kinds":[3,1,3],"rec":{...}}
```

| Field | Meaning |
| --- | --- |
| `status` | `ok`, or why not: `conflict`, `stale`, `version`, `held`, `full`, `expired`, `no_item`, `exhausted` |
| `item` | The item granted or named, or `null` |
| `replay` | `true` when this is the ID's recorded answer and nothing was applied now |
| `ver` | The character's inventory version now |
| `kinds` | The content kind of each item in the record's bag, in the bag's order |
| `rec` | The character's record now, or `null` when the store has none |

The answer always carries the character as the store has it. A room keeps no
count of its own: it takes its copy from each answer.

| Status | Meaning | What a room does |
| --- | --- | --- |
| `ok` | Applied, or recorded as applied | Tells the seat |
| `version` | The inventory changed since the room read it | Sends the operation again under a new ID with the version just heard, up to 8 times |
| `stale` | Another room owns the character now | Drops every operation it holds for the character |
| `held` | The claim or the ledger refused it | Drops the operation |
| `full` | The bag holds 32 items | Drops the grant |
| `conflict`, `expired`, `no_item`, `exhausted` | The ledger's other refusals | Drops the operation |

## Retries

A room sends one operation at a time, in order. It sends the same bytes again
until the API answers:

- No answer, a 5xx or a body that isn't an answer: the same bytes go again after
  1 second. A post with no answer after 15 seconds has failed.
- A 4xx is final: the operation is dropped.
- An answer to an earlier post is ignored.

A player is told an item dropped only after the API granted it. If a room dies
with an operation in flight, the API has either applied it or not: the item is
in the bag at the next seat, or it isn't, and it is never there twice.

## What a page hears

The room tells each seat its own character's record, when it sits and when it
changes (`everglade/shared/everglade-wire.coax`):

| Entry | Meaning |
| --- | --- |
| `e quest SS QUEST STAGE` | A quest's stage |
| `e item SS ITEM KIND SLOT` | An item in the bag: its content kind and its slot (0 not worn, 1 to 10 worn). `KIND` 0 means the item is gone |
| `e coin SS COIN` | The coin it holds |
| `e say SS DIALOGUE` | The dialogue line it was just told |
| `e loot SS KIND` | An item of this kind just dropped for it |

A page asks with `E SEQ KIND ID`: kind 6 talks to dialogue `ID`, kind 7 wears
item `ID`, kind 8 empties equipment slot `ID`. The room answers `r SEQ no` when
it doesn't take the ask.

## Versions and migration

A change to the fields is a new id (`everglade.item-op.v2`), never an edit in
place. Migration 0172 adds the `rider` column that keeps an operation's claim
with its ID, and teaches `verse.item_reconcile` the `consume` and `claim`
operations.
