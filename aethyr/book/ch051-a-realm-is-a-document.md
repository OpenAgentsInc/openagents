# 51. A realm is a document

> **Status: unimplemented — design draft.** Aethyrforge's realm builder runs
> today at `/realms/new`, and a made realm is already a line spec the server
> keeps, laid out by `myrealm.coax`. This chapter plans the typed realm
> document Aethyr reads instead. Nothing here runs yet.

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
- The spec today is a line protocol parsed in the module; its types live
  partly in SQL, partly in Coax, and nothing checks that the two agree.

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
[chapter 16](ch016-budgets-that-refuse.md) (budgets) and
[chapter 17](ch017-a-scene-owns-the-state.md) (the scene).
