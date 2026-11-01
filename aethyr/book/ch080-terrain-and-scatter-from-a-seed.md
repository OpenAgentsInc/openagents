# 80. Terrain and scatter from a seed

> **Status: unimplemented — design draft.** Today
> [`verse/terrain.coax`](../../verse/terrain.coax) builds a heightfield from
> rolling hills, levelled where paths run and on flats. Everglade scatters its
> forest on a jittered grid with a fixed seed
> (`apps/openagents-web/everglade.coax`), and made realms compose their
> features and ground detail in
> [`myrealm-shapes.coax`](../../apps/openagents-web/myrealm-shapes.coax) with
> a seed from the realm's text. Each realm has its own generator and its own
> random numbers. This chapter plans one generation module. Nothing in
> `aethyr/` runs yet.
>
> Made realms now lay out a site round a middle
> ([`myrealm-site.coax`](../../apps/openagents-web/myrealm-site.coax)): walls
> as segment colliders with two gateways, a centre, paving, a path, and a
> layout's own beds, ring or clearing. Scatter keeps out of the walls, the
> gateways and the open middle. A realm without a site draws the same random
> numbers as before, so its layout is unchanged.

A realm's ground and the things on it come from rules and a seed, not from a
hand-placed list. This chapter builds the rules: a heightfield shaped by
hills, paths and flats; scatter that keeps clear of what must stay open; and
composition that puts the asked-for things where a first visit sees them.

## What this chapter teaches

- How to build a heightfield whose sampled surface equals the drawn one.
- How to level ground under paths and flats and ease it back to the hills.
- How to scatter trees, rocks and plants on a jittered grid with exclusion
  zones.
- How to place headline features in view of where you start.
- How to use one counter-based random stream, so adding a feature changes
  no other feature's layout.

## Outline

1. **The heightfield.** A `Ground` has n cells a side (or n × rows), `cell`
   metres each, from its south-west corner, with (n + 1)² heights. Each cell
   is two triangles split south-west to north-east, as the GPU draws it, so
   `verse_terrain_height` is exactly the drawn surface. Everglade uses
   `EG_CELLS` (104) cells of `EG_CELL` (2 m) from `EG_EDGE` (-104 m).
2. **Hills and the bowl.** Hills are `HILL` (1.6 m) high and rise toward the
   edge from `RIM_FROM` (32 m) at `RIM` (0.022 m per m²), so a realm sits in
   a shallow bowl. A city sets its own rim: Everglade's starts at
   `EG_RIM_FROM` (80 m) at `EG_RIM` (0.004).
3. **Paths and flats.** `verse_terrain_build` levels the ground on each path
   strip and on flats (plazas, ponds), easing back to the hills over
   `PATH_BLEND` (3 m). It refuses more than `TERRAIN_LINKS` (256) links.
4. **Scatter.** Everglade puts a tree every `TREE_SPACING` (4.2 m),
   jittered by a fixed seed, wherever the woods are, with a metre clear round
   each trunk and none on the river, the crypt or the ruins. Bushes, ferns
   and rocks fill the tree line. A thicket at `EG_THICKET` (101.3 m) marks
   the edge past `EG_BOUND` (100 m). Aethyr states exclusion zones as data
   and checks them.
5. **Composition.** Made realms place headline features in slots ahead of
   the start (`SLOT_A`, left and right, then ahead, then wider) and herds in
   near places (`NEAR_A`). A forest ground frames the first buildings with
   trees (`wood`) and keeps the view ahead open. Birds circle inside the view
   and climb over roofs (`owl_home`, `roofs_out`). Ground detail (tufts,
   flowers, stones, warm and cool patches) is never a feature.
6. **Random numbers.** Made realms seed from an FNV hash of the realm's text
   (`seed_of`) and step a Park–Miller generator (`rand`, multiplier 48271).
   Everglade and the Sandbox use their own. Aethyr replaces them with one
   counter-based generator keyed by the realm's seed, the generator's
   version and each feature's stable ID, as described in
   [Randomness keyed by stable IDs](#randomness-keyed-by-stable-ids).
7. **Large worlds.** Generation runs per cell of
   [chapter 38](ch038-stream-a-large-world.md)'s streamed world, so a cell
   generates the same content whatever the load order.
8. **Edits over generated content.** A placed thing or a removed tree is an
   edit over the generated layout, stored in the realm document, never
   written back into the generator's output.

## Randomness keyed by stable IDs

> **Design draft.** Not implemented.

A feature's position in a list isn't a stable name. If the generator keys
randomness by index, inserting a feature at index 2 moves every feature after
it, and a saved edit that refers to "tree 40" lands on a different tree.
Aethyr keys every draw by values that don't change when the list changes:

```jai
draw(key: RandomKey, counter: u32) -> u64
RandomKey :: struct {
  seed:      u64   // the realm document's seed (chapter 51)
  generator: u32   // the generator's version, raised when its rules change
  feature:   u64   // the feature's stable ID from the realm document
  stream:    u32   // which property: position, rotation, scale, species
}
```

The rules:

1. **Every feature has a stable ID.** The realm document assigns it when the
   feature is created, and it never changes. Reordering, renaming or moving
   the feature keeps it. A deleted ID is never reused.
2. **Generated things derive their IDs.** A tree that scatter places inside a
   feature, or inside a streamed cell, gets an ID derived from its parent's ID
   and its grid coordinates, not from its order in a list.
3. **Each property has its own stream.** Position, rotation, scale and
   species use different `stream` values, so adding a property doesn't shift
   the draws of the others.
4. **The generator version is part of the key.** A change to the generator's
   rules raises `generator`. A document records the version it was generated
   with, and it regenerates with that version until the author upgrades it.
5. **Edits refer to stable IDs.** A moved or removed tree is stored as an
   edit keyed by the tree's derived ID, so the edit applies to the same tree
   after any regeneration that keeps the ID.

### Checks for stable randomness

- **Insertion.** Inserting a feature anywhere in the document leaves every
  other feature's generated content byte-identical.
- **Deletion.** Deleting a feature removes only its content. No other
  feature's content changes, and its ID isn't reused by a later feature.
- **Reordering.** Reversing the order of the document's features gives
  byte-identical generated content.
- **Regeneration.** Generating twice from the same document, seed and
  generator version gives byte-identical output natively and in the browser.
  Raising the generator version changes only the content whose rules
  changed, and a document that keeps the old version is unchanged.
- **Saved edits.** A document with edits to five trees (two moved, two
  removed, one replaced) is regenerated after an unrelated feature is
  inserted, deleted and reordered. Every edit still applies to the same tree,
  checked by derived ID and by position.

## What exists today

- [verse/terrain.coax](../../verse/terrain.coax): the heightfield, hills,
  path levelling, height, normal and ray queries.
- [verse/everglade/plan.coax](../../verse/everglade/plan.coax): Everglade's
  ground, bounds, streets and clearings.
- [apps/openagents-web/everglade.coax](../../apps/openagents-web/everglade.coax):
  the forest scatter (`TREE_SPACING`).
- [apps/openagents-web/myrealm.coax](../../apps/openagents-web/myrealm.coax)
  and [myrealm-shapes.coax](../../apps/openagents-web/myrealm-shapes.coax):
  made-realm composition and its seed.
- [docs/verse/realm-builder.md](../../docs/verse/realm-builder.md),
  "Composition": the placement rules for made realms.
- [docs/research/game-engine-systems.md](../../docs/research/game-engine-systems.md),
  sections 16 and 17: terrain, vegetation and procedural generation.

## What it builds

- `aethyr/forge/ground.coax`: heightfields, hills, rims, paths and flats.
- `aethyr/forge/scatter.coax`: jittered scatter with exclusion zones.
- `aethyr/forge/compose.coax`: slots and framing for asked-for features.
- `aethyr/forge/random.coax`: the counter-based generator.

## Acceptance checks and oracles

The chapter is done when:

- `verse_terrain_height` and the drawn mesh agree at every vertex and at
  sampled interior points, as today's tests require.
- No scattered tree stands in an exclusion zone, checked for every realm.
- The same document and seed give byte-identical ground and scatter natively
  and in the browser.
- Adding a feature leaves every other feature's placements unchanged, and
  the checks for stable randomness pass.
- A streamed cell generates the same content in any load order.
- Made-realm goldens change only where a fix is visible.

## Dependencies

Chapters 2 (spaces and units), 7 (queries over the ground), 8 (ground and
collision), 16 (budgets), 38 (streaming in cells) and 51 (the realm document
and its seed).
