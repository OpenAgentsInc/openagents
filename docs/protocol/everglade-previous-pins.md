# The previous pins: a room seats two builds for one release

Issue: [#13408](https://github.com/OpenAgentsInc/openagents/issues/13408).
This is [the previous wire](everglade-previous-wire.md)'s rule (#13398) for
the three pins beside the wire version.
Code: `everglade/shared/pins-prev.coax` (the rule, `#authority`),
`everglade/server/pins-prev.coax` (what the room does for such a seat),
`metaverse-engine/session/compat.coax` (admission, `#authority`),
`coax/web/room/room-items.coax` `ri_doc` (the content tables a seat plays by).

## The rule

A page says its build in `P` before its hello: its engine (`WIRE_ENGINE`), its
rules (`WIRE_RULES`) and its content pin (`WIRE_CONTENT` with its content
tables' revision). A room seats two builds: its own, and the build of the
release before. A page on any other build is refused, `r 0 version`.

A release moves its rooms before its page, and a page a player has open is the
previous release's until they reload. Without this rule, a change to any pin
makes the rooms refuse every page and bot between the two steps of a release.
That is how 83 content rows turned `main` red (#13391).

| Room | Page | Result |
| --- | --- | --- |
| build B | build B | plays |
| build B | the release before's build | plays: the room serves that seat what its build reads |
| build B | all three pins neither B's nor the release before's (a mix of the two included) | refused, `r 0 version` |
| the release before's build | build B | refused: a page never ships ahead of its rooms ([the release set](../deployment/release-set.md)) |

The three pins are matched together. A page whose engine is the old value and
whose rules are the new one is no build that was ever released, so it is
refused.

A room reports both builds on its vitals:

```text
build engine=1 rules=1 content=2 wire=5-6 schema=2
seats-before engine=1 rules=1 content=1
```

When nothing was raised, `seats-before` is the room's own build.

One raise a release, for each pin. A second raise before production runs the
first leaves production's page two behind, and the pair check goes red.

## Content: a seat plays by its page's tables

A room is built with the content tables of the release before
(`everglade/content/previous.coax`, the bytes of
`everglade/content/everglade.previous.content`) and reads its own at start. It
keeps the tables before only when they are Everglade's, the same document, and
exactly one revision below its own.

A seat whose content pin is the one before plays by the tables before. Its
quests' steps and rewards, its drops, the dialogue it may hear and the slot an
item is worn in are that revision's rows. The room never names a row that
page lacks, and a row the new revision changed means to that seat what its page
shows. A seat on the room's own revision plays by the room's tables, in the
same room.

Because a seat gets its own page's tables whole, any edit is compatible: rows
added, changed or dropped. Content IDs still never renumber
([content IDs](everglade-content-ids-v1.md)).

Two limits, both for the one release:

- A character keeps what it earned. If it gained an item kind on a new page
  and then opens an old page, that page has no row for the kind.
- The world's creatures are the room's. The tables' `spawn` and `creature`
  rows that the room's build reads are the room's own for every seat.

### Raise the content revision

1. Before the edit, keep the tables production runs:
   `coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content --keep`.
   Do this only when `everglade.content` is what production runs. If an
   earlier raise has not been released yet, the kept file is already right and
   your edit has to wait for that release.
2. Edit the tables ([content tables](aethyr-content-v1.md)).
3. `coax/web/tool.sh everglade/model/tools/content-check.coax everglade/content/everglade.content --carry`
   writes `shipped.coax` (the page's copy) and `previous.coax` (the room's copy
   of the tables before). Commit all four files.

## Engine and rules: declared, and served by the room

`everglade/shared/pins-prev.coax` holds two constants:

| Constant | Meaning |
| --- | --- |
| `PPREV_UNDER` | This build's `WIRE_ENGINE`, `WIRE_RULES` and `WIRE_CONTENT` when the file was written |
| `PPREV_SERVES` | The release before's |

To raise `WIRE_ENGINE`, `WIRE_RULES` or `WIRE_CONTENT`, do all of this in the
commit that raises it:

1. Set `PPREV_SERVES` to the pins production runs and `PPREV_UNDER` to the new
   ones. Each served pin is the new value or the one below it.
2. Write in `everglade/server/pins-prev.coax` what the room does for a seat on
   the old value. `pprev_seat_behind(v, k, s)` says which pins a seat is behind
   on (`PPREV_ENGINE`, `PPREV_RULES`, `PPREV_CONTENT`). The page predicts with
   its own copy of the engine and the rules, so the room keeps the old step or
   the old rule for that seat, as `wire-prev.coax` keeps the old frames. Delete
   what the release before that one needed.
3. Add a room check that a seat on the old value plays, as the content checks
   there do.

Both constants are 1 1 1 today: no engine or rules raise has shipped through
this rule yet, and no seat is behind on either.

## What fails when a pin is raised without this

| Check | What it catches |
| --- | --- |
| The room's check "previous pins: this declaration was written for this build, one raise at most a pin" | `WIRE_ENGINE`, `WIRE_RULES` or `WIRE_CONTENT` changed and `PPREV_UNDER` did not, or a served pin is two below. The room's suite fails |
| `content-check.coax` (the step `everglade-content`) | The kept tables are not Everglade's, or not the revision before the tables' own (and not the same bytes) |
| `everglade-parity`, rule 4 | `previous.coax` is not the kept file's bytes |
| The room's checks "previous pins: a page on the tables before sits beside one on this room's" and "a seat on the tables before plays by them" | Admission or the per-seat tables stopped working |
| `coax/remote/check.sh e2 PROD`, the line `e2 prev-pins` (`coax/web/room/pins-pair.coax`) | Production's pins are neither this build's nor the release before's, or the tables behind production's pin are other bytes than this build holds for it. The after-landing check runs it on every head of `main` |
| `coax/remote/check.sh e2 PROD`, the lines `e2 prev-wire` and `e2 holds` | Production's own bots are refused by this commit's room |

## The release tools

- `release-set ship` moves the page only onto shards that run the page's own
  build. It reads the page's content pin from the tables the page is built
  with (`rl_page_pins`).
- The directory places a page on a shard that runs its build
  (`verse.room_pick`). When no shard runs it, the page is placed as if it named
  none, and the room decides: it seats the release before's page.
