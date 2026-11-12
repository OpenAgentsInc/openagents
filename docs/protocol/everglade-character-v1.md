# Everglade character v1

Profile `everglade-character-v1`. Issue #13290 (AAA P4.1), tracker #13257.

The Character aggregate: the one record the server owns for each Everglade
character. The schema and its pinned bytes are in
[`proto/verse/everglade-character-v1.schema.json`](../../proto/verse/everglade-character-v1.schema.json).
The one codec, and the one owner of the record's rules, is
[`everglade/model/character.coax`](../../everglade/model/character.coax)
(`#authority`, zero surviving mutants). The check is:

```sh
coax/web/tool.sh everglade/model/tools/character-profile.coax proto/verse/everglade-character-v1.schema.json
```

## The record

A JSON object with exactly these 11 keys, written in this order:

| Key | Value | Who changes it |
| --- | --- | --- |
| `v` | `1` | |
| `who` | The durable identity, `[a-z0-9-]{1,40}`: the account's text, as the item ledger names it (`verse.item_who`, [decision 0006](../decisions/0006-item-ledger-rules-in-coax.md)) | Never |
| `class` | The class word, `srd-` and 3 to 12 lowercase letters, or `""` before the first seat | Set once (`char_class_set`), never changed |
| `xp` | 0 to 9,999,999 | Only rises |
| `floor` | 1 to 6: the level the server grants without the XP (a realm's starting level, an admin's choice) | The server |
| `known` | Ability content IDs (#13286) learned past the class kit, ascending, no repeats, at most 32 | The room |
| `bag` | Item IDs (`verse.items.id`), ascending, no repeats, at most 32 | The item ledger only |
| `inv` | The ledger's inventory version | The item ledger only |
| `worn` | 10 slots (head, body, hands, legs, feet, neck, ring, main, off, ranged): an item ID in `bag`, or 0; no item twice | The room |
| `coin` | Copper pieces, 0 to 999,999,999. Game currency, never money | The room |
| `quests` | `[quest content ID, stage]` pairs, stage 0 to 255, ascending by ID, no repeats, at most 32 | The room (#13292) |

The level is never stored. It is `srd_level_from(xp, floor, 6)`
(`char_level`), from the rules' one table in
`aethyr/rules/srd/advancement.coax`, for every level Everglade II has (1 to 6).
The class kit's own abilities follow from class and level and are not stored.

A reader refuses a body that breaks any rule: a missing or extra key, a
number that is not whole or is out of range, a list out of order, a worn item
not in the bag. It never repairs one.

## The store's write rule

`char_accept(kept, room)` is what the store keeps when a room writes a record
over the one it has. It refuses (the result is not ok) a write for another
character, a write whose XP is below the kept XP (a write from behind), and a
write that changes a class already set. Otherwise it keeps the room's record
with the kept `floor`, the kept `bag` and `inv` (a room never writes items),
and takes off any worn item no longer in the kept bag.

Items change only through the item ledger (`aethyr/session/ledger.coax`,
#13291). The store fills `bag` and `inv` from the ledger when it loads a
record.

## Versions and migration

- `v` names the version. A reader accepts only the versions it knows; a newer
  version is refused, never guessed at.
- A change to the record is a new version (`v: 2`) with its own profile
  file, never an edit to v1 in place. v1's schema and pinned bytes stay.
- Each new version comes with an upgrade: a function in `character.coax` that
  reads the previous version and returns the new record, with a written
  default for every new field. The store upgrades a record when it loads it
  and writes the new version at the next accepted write. A stored record is
  never rewritten by a migration script alone.
- Removing a field means a new version that drops it; the upgrade says where
  its data goes, or that it is discarded and why.
- Content IDs inside the record (`known`, `quests`, and the item kinds behind
  `bag`) are stable IDs (#13286). Renaming content never changes them.

## What is not in the record

The session state a room resumes from (Hit Points, place, spent slots and
uses, the rest) stays in the character checkpoint
([v3](character-checkpoint-v3.md)), with its owner generation, receipts and
30-second loss bound. Party, account and items are referenced by ID only.
