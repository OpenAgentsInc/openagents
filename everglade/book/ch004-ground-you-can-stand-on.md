# 4. Ground you can stand on

> **Status: partly implemented.** Everglade II's ground is a set of Metaverse Engine
> terrain tiles in [`verse/everglade2/ground.coax`](../../verse/everglade2/ground.coax),
> built on [`aethyr/world/terrain-tile.coax`](../../aethyr/world/terrain-tile.coax)
> (#12995, #13059). The code moves into `everglade/world/` later, so this
> chapter names functions and has no source listings yet.

This chapter answers the player's question: "Am I standing on what I see?"

Your character's feet and the drawn ground must agree everywhere: at every
sample, along every triangle and across every seam between tiles. If they
don't, your feet float above the ground or sink into it, and the world stops
looking solid.

## What this chapter teaches

- How to lay tiles on one shared lattice so that neighbours agree bit for bit.
- How to join a fine tile to a coarse one without a crack.
- Why the walk must read the same triangles that the GPU draws.
- How a tile set is admitted whole, or refused whole.

## What exists today

| Piece | Where |
| --- | --- |
| The land's height as a pure function | `e2_height` in [`verse/everglade2/forest.coax`](../../verse/everglade2/forest.coax) |
| A tile's format, admission and sampler | `aethyr/world/terrain-tile.coax` (`terrain_tile_sample`) |
| Everglade II's tiles, record and queries | `verse/everglade2/ground.coax` (`e2g_convert`, `e2g_seams`, `e2g_record_read`, `e2g_y`) |
| The ground's colour at a pixel | `e2_ground_albedo` in [`everglade/client/gpu/everglade2.coax`](../client/gpu/everglade2.coax) |

Inside the glade, the land is exactly Character Create's own (`cg_height` in
[`verse/character-glade.coax`](../../verse/character-glade.coax)). Outside it,
`e2_height` adds the path's valley and the wooded ridges.

## The method

### One lattice for every tile

Every tile lies on one lattice: the glade's 0.65 m samples, starting at
(-32.5, -60). `e2g_at(base, index)` works out each sample's position once, in
64-bit floats, and rounds once to 32 bits. A sample that two tiles share is the
same index on the same lattice, so it is the same point and the same height in
both tiles, bit for bit.

The set has 21 tiles:

- **The glade, tile 0.** 100 cells of 0.65 m, from (-32.5, -60) to (32.5, 5).
  Each cell splits on the same diagonal as the pack's old triangles.
- **The path.** 65 m tiles in rows south of the glade. Rows 0 to 4 have 100
  cells of 0.65 m. Rows 5 to 7, round the clearing, have 50 cells of 1.3 m.
- **Holes.** A path cell is solid only along a ribbon round the path
  (`e2g_solid`). Elsewhere it is a hole. A tile with no solid cell is left
  out.

### Seams

Neighbours with the same spacing repeat their shared edge exactly. Where a
0.65 m row meets a 1.3 m row, at z = -385, the finer edge is stitched onto the
coarser one, so no edge cracks. Every tile draws at its stored level, so
nothing swaps as you walk.

### One answer for "how high is the ground here"

`e2g_y(g, x, z)` answers the walk. On a tile it samples the tile's own
triangles with `terrain_tile_sample`, the same triangles the GPU draws. Past
the tiles, it falls back to `e2_height` and counts the fallback (`e2_probe`
98).

The pack draws the ground 0.035 m lower than the tiles. That offset is
declared, so it is decoration, not disagreement.

### Admit the whole set, or keep the old one

The pack tool converts the land once and writes the whole set into the pack's
catalog as one record (E2T1). The page reads it with `e2g_record_read`. Each
tile must come out at the revision that the record names, and then the seams
and the set's revision are checked. Only then does the set publish, whole. If
anything refuses, the page counts the refusal and keeps the set it had. With
no record, the page converts the land itself.

## A worked example: where a fine row meets a coarse row

Row 4 has 0.65 m cells. Row 5 has 1.3 m cells, twice as wide. Along their
shared edge at z = -385, row 4 has 101 samples and row 5 has 51.

Row 5's sample `i` sits at the same lattice index as row 4's sample `2i`:

```text
x(row 5, i)  = base + (2i) × 0.65    computed in f64, rounded once
x(row 4, 2i) = base + (2i) × 0.65    the same expression, the same bits
```

So every coarse sample is also a fine sample. The fine samples in between
(`2i + 1`) are stitched to the coarse edge (`terrain_tile_stitched`): their
heights are set on the straight line between the two coarse samples on either
side. The fine edge then
follows the coarse edge exactly, and no gap opens between them.

Now compare reading the smooth function instead of the triangles. Between two
samples 0.65 m apart, a triangle is flat, but `e2_height` curves. On a slope
whose height bends by 0.2 m per metre per metre, the largest gap between a
straight chord and the curve over one cell is:

```text
gap = cell² × bend / 8 = 0.65² × 0.2 / 8 ≈ 0.011 m
```

A centimetre is enough to show feet sinking into the ground in a close shot.
That is why the walk reads `e2g_y` on the tiles, not `e2_height`.

## The checks

In `verse/everglade2/ground.coax`:

- `#test "the glade's tile is the pack's old grid: admitted, its samples the
  land, its weights the layers"`.
- `#test "the path's tiles meet the glade and each other bit for bit, the coarse
  rows on the fine edge, and the walk never steps"`.
- `#test "the record round-trips: the pack's route admits the same set, and any
  damage refuses with the old set kept"`.
- `#test "far from the origin the tiles answer as at it: queries, the drawn grid
  and the bake's frame agree"`. A set placed 1e8 m out answers the same queries.

In `aethyr/world/terrain-tile.coax`:

- `#test "split tiles agree on their shared edges and sample as the whole"`.
- `#test "hand-worked: height, slope and holes on both triangles of one
  cell"`.
- `#forall "a stitched LOD edge lies on its coarser neighbour's edge, with no
  crack"`, which also shows that the unstitched edge would crack.

In a headless browser,
[`coax/web/everglade2-terrain-check.coax`](../../coax/web/everglade2-terrain-check.coax)
walks the glade, the path and the fine-to-coarse seam. It passes only on the
pack route with no refusal and no memory growth.

```sh
coax/bin/coax test verse/everglade2/ground.coax --library
coax/bin/coax test --library aethyr/world/terrain-tile.coax
```

## Mistakes

- **Two answers to one question.** Before #12995, the walk read `e2_height`
  everywhere, while the GPU drew the pack's flat triangles. Between samples the
  two disagree, as the example shows. The walk now stands on the admitted
  triangles, and the seam test asserts that the walk never steps at a tile
  edge.
- **A half-loaded ground.** A set that publishes tile by tile can show one new
  tile next to an old one. The set publishes whole or not at all, and the
  record test damages the record to prove that the old set stays.

## What comes next

[Chapter 5](ch005-plant-a-forest-with-a-function.md) plants the forest on this
ground.
