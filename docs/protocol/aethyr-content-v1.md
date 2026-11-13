# Content tables v1 (`aethyr.content.v1`)

A game's data as versioned tables of rows, in one text document. A room loads
the document at start, and the OpenAgents Metaverse Engine edits it. Tracked in GitHub #13294
(AAA P4.5).

- Schema and fixtures: [`proto/aethyr/content-v1.json`](../../proto/aethyr/content-v1.json)
- Codec and validator: `metaverse-engine/content/tables.coax` (`#authority`)
- Everglade's tables: `everglade/content/everglade.content`, bound by
  `everglade/model/content.coax` (`#authority`)
- Editor: `metaverse-engine/editor/content-edit.coax`, on the Metaverse Engine's edit log

## The document

```text
aethyr.content.v1 NAME REV
reach N
table NAME id COL:KIND ...
sum COL by COL N
ID|CELL|CELL...
```

| Line | Meaning |
| --- | --- |
| `aethyr.content.v1 NAME REV` | The document's name and its revision, 1 and up |
| `reach N` | How far from the middle a place may be, in centimetres |
| `table NAME id COL:KIND ...` | A table. Its first column is always the row's ID |
| `sum COL by COL N` | A rule of the table above: for each value of the second column, the first column's cells add up to `N` |
| `ID\|CELL\|...` | A row of the table above |

A column's kind:

| Kind | Cell |
| --- | --- |
| `int` | A whole number, 0 to 2147483647 |
| `text` | One line of text with no bar (`\|`) and no control byte |
| `cm` | A place in centimetres, within the reach either way |
| `ref.TABLE` | The ID of a row of `TABLE` |
| `opt.TABLE` | The ID of a row of `TABLE`, or 0 for none |

## One form

A document has exactly one form:

- Tables stand in ascending order by name. Rows stand in ascending order by ID.
- Numbers have no leading zeros.
- Every line ends in one newline.

`ct_read` writes what it read and compares the bytes, so a document that reads
is the document a writer writes. A change of content is a change of bytes, and
a regenerated file is byte-identical.

## What the validator refuses

- A dangling ID: a `ref` or `opt` cell that names no row.
- A place out of reach: a `cm` cell past the document's reach. This is how a
  spawn that can't be reached is refused.
- A sum that is wrong: a group whose cells don't add up to the rule's `N`. This
  is how a loot table that sums wrong is refused.
- Any other bytes than the document's one form.

## Limits

| Limit | Value |
| --- | --- |
| Document | 65,536 bytes |
| Tables | 16 |
| Columns in a table, with its ID | 8 |
| Rows in a document | 1,024 |
| Text cells in a document | 32,768 bytes |
| Sum rules in a table | 4 |
| A name | 15 bytes: a lowercase letter, then lowercase letters, digits and underscores |

A document past a limit doesn't read.

## Everglade's tables

`everglade/model/content.coax` names the tables Everglade needs and refuses a
document with other tables or columns.

| Table | Columns | Meaning |
| --- | --- | --- |
| `ability` | `name` | Abilities and auras, by content ID (#13286) |
| `creature` | `name`, `kind`, `xp` | `kind` is the room's creature template |
| `dialogue` | `npc`, `point`, `quest`, `line` | Who says the line, where they stand, and the quest it offers (0 for none) |
| `item` | `name`, `slot` | `slot` is 0 for an item that isn't worn, else 1 to 10: the character record's slots, in order |
| `loot` | `creature`, `item`, `weight` | A creature's drops. Weights add up to 100 for each creature. Item 0 is nothing |
| `point` | `name`, `x`, `z`, `r` | A named place and its radius, in centimetres |
| `quest` | `name` | A quest |
| `reward` | `quest`, `item`, `coin`, `xp` | Paid in ID order once a quest's steps are done |
| `spawn` | `creature`, `point`, `count` | Where a creature appears |
| `step` | `quest`, `creature`, `point`, `item`, `talk`, `count` | One objective, in ID order: kill `count` of a creature, reach a point, bring `count` of an item, or talk |
| `template` | `creature`, `block`, `style`, `notice`, `leash`, `run` | How a creature fights (`docs/metaverse/creatures.md`) |
| `troop` | `spawn`, `group`, `face`, `respawn`, `slot` | A spawn's place in the room |
| `troop_beat` | `troop`, `point`, `rest` | A patrol's beat: the points its troop walks, in ID order, and the seconds it stands at each |
| `troop_phase` | `troop`, `hp`, `after`, `style`, `run` | A boss script: at a percent of its Hit Points or seconds after it is roused, a troop's member fights by another style and run |

The tables come in three sets: the first ten, those with `template` and
`troop`, and those with `troop_beat` and `troop_phase`. Each set is the one
before with tables added after it. Table order is by name, so a new table's
name must sort after `troop_phase`. That keeps every table's place the same
in an older document, which a room still reads for one release
([the previous pins](everglade-previous-pins.md)).

Everglade also refuses an item slot the record doesn't have, a step with no
target or more than one, a quest with no step, and a quest with more stages
than the record holds (255). It refuses a phase with no key (`hp` and `after`
both 0), with `hp` over 100, or with a style that doesn't fight, and a beat
point with a rest over 600 seconds.

## Edit the tables

Each edit is one step on the Metaverse Engine's edit log, with undo and redo. The file is
written at its next revision only when every table still holds its rules.

```sh
coax/web/tool.sh metaverse-engine/tools/content-edit.coax everglade/content/everglade.content \
  row creature '3|Bear|0|450' \
  row loot '5|3|0|100' \
  row spawn '3|3|2|2'
coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content --carry
```

Before the first edit after a release, keep the tables production runs, so
rooms can serve the pages still on them
([the previous pins](everglade-previous-pins.md), #13408):

```sh
coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content --keep
```

`--carry` also writes `everglade/content/shipped.coax`, the copy of the tables
that Everglade's page is built with (#13381), and
`everglade/content/previous.coax`, the room's copy of the kept tables. Commit
them with the tables. `coax/web/tool.sh everglade-parity` fails when a copy
isn't its file's bytes.

A room reads the file at start. A page reads its built-in copy. Each says the
tables' revision in its content pin
([the content pin](everglade-content-ids-v1.md#the-content-pin)), so a page and
a room with different tables never meet, except that a room seats the revision
just before its own for one release and serves that seat from the kept tables.
Release the room and the page from the same commit after an edit.

| Edit | Meaning |
| --- | --- |
| `row TABLE 'ID\|CELL\|...'` | Add a row, or change the row with that ID |
| `drop TABLE ID` | Take a row away |
| `table NAME 'id COL:KIND ...;sum COL by COL N'` | Add a table, or change its columns |

## Versions and migration

A change to the document's lines or kinds is a new id (`aethyr.content.v2`),
never an edit in place. A reader refuses an id it doesn't know.
