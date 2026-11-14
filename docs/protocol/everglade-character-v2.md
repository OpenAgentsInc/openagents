# Everglade character v2

Profile `everglade-character-v2`. Issue #13519, after
[v1](everglade-character-v1.md) (#13290).

v2 is v1 with one more key, `range`: the Archery Range's lasting tally. The
schema and its pinned bytes are in
[`proto/verse/everglade-character-v2.schema.json`](../../proto/verse/everglade-character-v2.schema.json).
The one codec, and the one owner of the record's rules, is still
[`everglade/model/character.coax`](../../everglade/model/character.coax)
(`#authority`). The checks are:

```sh
coax/web/tool.sh everglade/model/tools/character-profile.coax proto/verse/everglade-character-v1.schema.json
coax/web/tool.sh everglade/model/tools/character-profile.coax proto/verse/everglade-character-v2.schema.json
```

## The record

v1's 11 keys in v1's order, then:

| Key | Value | Who changes it |
| --- | --- | --- |
| `v` | `2` | |
| `range` | 61 whole numbers, each 0 to 9,999,999: the longest run of hits, then for each way of shooting (on foot, from horseback, from dragonback) and each band of distance (to 15, 25, 40, 60 m, past 60), way major, its shots, hits, points and golds. A cell's hits are at most its shots, its golds at most its hits, its points from one to ten a hit; the run is at most every hit there is (`everglade/range/score.coax` `rg_tally_ok`) | A claim (`char_claim`): an arrow's `shot` and `run`, or RESET's clear |

A record whose tally is empty (all 0) is written as v1, byte for byte. So every
record kept before v2, and every character that never shot at the range, keeps
its exact bytes, and a room from before v2 still reads it.

## The write rules

- `char_claim` adds an arrow to the tally (`rg_tally_shot`), with the claim's
  other parts, all or none. RESET's claim (`shot` -1) empties it.
- `char_accept` (a room's write: the seat ticket, a checkpoint) keeps the kept
  record's tally. Only a claim changes it.
- The item ledger applies a claim once per operation ID, so an arrow's claim
  sent twice counts once ([item operation v1](everglade-item-op-v1.md), `shot`
  and `run`).

## Where it lives

`verse.characters`, as v1. Migration 0192 lets the body's `v` be 2 with a
`range` of 61 numbers, and keeps v1 without one. It rewrites no row.

## Versions and migration

The upgrade is in the reader: `char_read` takes v1 (exactly its 11 keys) as a v2
record whose `range` is all 0, the written default, and v2 (its 12 keys, `range`
exactly 61 numbers). Any other version, a v1 body with a `range`, or a v2 body
without one is refused. The store writes v2 at the next accepted write that
leaves a tally. v1's schema and pinned bytes stay as they were.
