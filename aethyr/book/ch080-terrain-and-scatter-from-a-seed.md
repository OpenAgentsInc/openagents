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
   counter-based generator keyed by the seed and each feature's index, as
   [chapter 51](ch051-a-realm-is-a-document.md) requires.
7. **Large worlds.** Generation runs per cell of
   [chapter 38](ch038-stream-a-large-world.md)'s streamed world, so a cell
   generates the same content whatever the load order.
8. **Edits over generated content.** A placed thing or a removed tree is an
   edit over the generated layout, stored in the realm document, never
   written back into the generator's output.

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
- Adding a feature leaves every other feature's placements unchanged.
- A streamed cell generates the same content in any load order.
- Made-realm goldens change only where a fix is visible.

## Dependencies

Chapters 2 (spaces and units), 7 (queries over the ground), 8 (ground and
collision), 16 (budgets), 38 (streaming in cells) and 51 (the realm document
and its seed).
