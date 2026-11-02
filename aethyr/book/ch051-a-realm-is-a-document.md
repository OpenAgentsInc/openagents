# 51. A realm is a document

> **Status: unimplemented — design draft.** The typed `RealmDoc`, its
> catalogue and a pure `layout` below are not built. What runs today: the
> builder at `/realms/new` makes a realm the server keeps as a versioned JSON
> document (`aethyr.realm`, version 1) with a revision number and a SHA-256
> hash, served as a line spec that `myrealm.coax` lays out from the slug's
> seed. [What exists today](#what-exists-today) lists it.

A realm someone makes is not code. It is a small record: the camera, the art
style, the ground, the time of day, a list of features with bounded values,
the tools visitors get and the things placed since. Everything a visitor sees
must follow from that record and a seed, the same on every device. This part
of the book is Aethyrforge, the tools that make and change such records; this
chapter defines the record itself.

## What this chapter teaches

- A realm as a versioned, typed document: setup, features, placements, rules.
- A catalogue as the closed set of what the engine can build, with bounded
  values, from which both the planner's schema and the document's types come.
- Deterministic layout: the same document and seed give the same entities,
  byte for byte, natively and in the browser.
- Asking for what the engine cannot build yet without losing it.

## The method

The catalogue is data: each feature has an id, an enumerated value set and a
count range (trees: kind ∈ {round, pine, dead, mixed, palm, blossom, autumn,
acacia}, count 3 to 20). A document is a list of feature instances drawn only
from it, plus a fixed setup and the placements:

```jai
RealmDoc :: struct {
    version: u32;                 // the document schema, migrated by chapter 3's rules
    setup:   Setup;               // camera, style, ground, time; fixed once chosen
    features: [MAX_FEATURES] Feature;  feature_n: u32;
    wants:   [MAX_WANTS] Want;    wants_n: u32;   // asked for, not buildable yet
    placed:  [MAX_PLACED] Placed; placed_n: u32;  // thing, x, z, yaw, entity id
    tools:   ToolMask;
}
layout :: (doc: *RealmDoc, seed: u64, out: *World) -> LayoutStatus;
```

`layout` is a pure function. Its randomness comes from a counter-based
generator keyed by the seed and the feature's index, so adding a feature does
not move the others. Composition rules decide where things go: what was asked
for stands where a first-time visitor sees it from the spawn (headline slots
ahead, near slots for herds), the ground gets its own unlisted dressing, and
fliers circle clear of anything standing under them.

Words never reach the engine. A planner model answers under a JSON schema
generated from the catalogue; the server keeps only catalogue features with
catalogue values. Whatever was asked that no feature draws becomes a `Want`
line, carried through every revision, so nothing asked for is silently dropped.

## What exists today

A made realm is a document with an identity. `GET
/api/playtest/realms/<slug>/doc` answers it and its hash:

```text
{"doc": {"doc": "aethyr.realm", "version": 1, "slug": "forge-court", "rev": 2,
         "name": "Forge Court",
         "setup": {"camera": "third", "style": "lowpoly", "ground": "stone", "time": "day", "palette": null},
         "features": [{"feature": "trees", "variant": "round", "count": 6, "label": "Six round trees"}],
         "off": [], "tools": {"build": false}, "placed": [],
         "moved": [{"entity": "trees:2", "x": 3.26, "z": -4.5}],
         "base": null, "forked_from": null},
 "sha256": "..."}
```

- The rules are in
  [coax/web/api/migrations/0093_realm_documents.sql](../../coax/web/api/migrations/0093_realm_documents.sql):
  `verse.realm_doc` builds the document from the `verse.made_realms` row,
  and `verse.realm_doc_sha` hashes it. Any change to what the realm is
  (its features, the rows switched off, its tools, things placed, moves)
  makes a new revision; each saved, published, forked or made revision is
  kept in `verse.realm_revisions` with its hash.
- `moved` holds an override for each moved entity, named by feature and
  layout index (`trees:2`). The seeded layout keeps those names stable, and
  the page applies the moves after laying everything else out, so a move
  shifts only that entity
  ([chapter 52](ch052-edit-a-realm-in-place.md)).
- A save names the revision it was read at, and the server refuses a stale
  one with `409` and the revision there now. The newer write never simply
  wins. [Chapter 82](ch082-save-and-come-back.md) uses the same rule for a
  player's save slot.
- The line spec carries the document's identity: `rev N`, `published N`
  (0 when never published), `listed 0|1`, `from SLUG REV|NAME` for a fork,
  and `moved ENTITY X Z` per move.
- `shaped` (migration
  [0109_realm_shapes.sql](../../coax/web/api/migrations/0109_realm_shapes.sql))
  holds each entity's turn, size and removal, and is in the document only
  when it has an entry, so a document written before it keeps its bytes and
  hash. The spec says `shaped ENTITY YAW SCALE 0|1` and `shapes 1`.
- `lore` `{WHO: TEXT}` and `quests` `{GIVER: {ask, steps, reward}}`
  (migration
  [0111_realm_edit_log.sql](../../coax/web/api/migrations/0111_realm_edit_log.sql))
  follow the same rule: in the document only when it has some. They used to
  be rows beside the document, so a publish, a fork and an undo missed them;
  lore written before moved into each realm's document as one new revision,
  and every older revision kept its bytes and hash. `GET .../panes` reads
  them, a visitor's from the revision published. `aethyr/forge/doc.coax`
  doesn't read them into the model yet; they are slots there (`lore:WHO`,
  `quest:GIVER`), and the server's undo log names them the same way.
- [aethyr/forge/doc.coax](../forge/doc.coax) reads a document into the edit
  model and moves an older version forward (version 1 without `shaped` is the
  same realm with none). It refuses a newer version, or anything out of
  bounds, whole, so a client keeps the last document it could read.

Older parts of the builder:

- [docs/verse/realm-builder.md](../../docs/verse/realm-builder.md): the flow
  (setup questions, prompt, board, toggles, MAKE IT), the catalogue
  (`verse.builder_catalogue()` in `coax/web/api/migrations/0048_realm_builder.sql`),
  cameras, palettes, limits (200 placed things, 50 waiting cards) and the
  composition rules.
- [apps/openagents-web/builder.coax](../../apps/openagents-web/builder.coax):
  the board page; it never reads the words, only shows the board and sends
  back which rows are checked.
- [apps/openagents-web/myrealm.coax](../../apps/openagents-web/myrealm.coax)
  and [myrealm-shapes.coax](../../apps/openagents-web/myrealm-shapes.coax):
  `GET /api/playtest/realms/<slug>` read as lines, laid out from the slug
  (`SLOT_A`, `NEAR_A`, `owl_home`, `roofs_out`).
- The spec is still a line protocol parsed in the module. The document's
  types live partly in SQL and partly in Coax
  ([aethyr/forge/edit.coax](../forge/edit.coax) and [doc.coax](../forge/doc.coax)), and only the
  API tests and the page's `#test`s check that the two agree.

## What it builds

- `aethyr/forge/doc.coax`: `RealmDoc`, its line and binary encodings, version
  migration through chapter 3's declared defaults.
- `aethyr/forge/catalogue.coax`: the catalogue as one table, from which the
  SQL schema and the planner's JSON schema are generated.
- `aethyr/forge/layout.coax`: `layout`, emitting chapter 6 entities into a
  chapter 17 scene, under chapter 16's budgets.

## Acceptance checks and oracles

- Encode then decode is the identity for every catalogue feature at its
  bounds; an unknown feature, an out-of-range count or a value outside its
  enum is refused with its field named.
- `layout` of the same document and seed gives byte-identical entity tables
  natively and in the browser; adding one feature leaves every other
  feature's entities unchanged.
- Every asked thing either maps to a feature or stays as a `Want` line
  through ten revisions (the builder's `eval-builder.sh` corpus as fixtures).
- The generated SQL catalogue equals `verse.builder_catalogue()`; drift fails.
- A document over its budgets (placed things, vertices) is refused at
  layout, never clipped.

## Dependencies

[Chapter 3](ch003-state-starts-from-its-defaults.md) (defaults and
versions), [chapter 6](ch006-entities-are-born-and-die.md) (stable IDs),
[chapter 16](ch016-budgets-that-refuse.md) (budgets) suffice for the typed
document and deterministic layout. A physics-bearing realm later maps that
document into [chapter 17](ch017-a-scene-owns-the-state.md)'s scene and checks
that mapping; constructing the document does not require the physics engine.
