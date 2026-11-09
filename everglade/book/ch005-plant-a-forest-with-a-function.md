# 5. Plant a forest with a function

> **Status: partly implemented.** The planting is
> [`verse/everglade2/plant.coax`](../../verse/everglade2/plant.coax) and the
> grass is [`verse/everglade2/field.coax`](../../verse/everglade2/field.coax).
> The code moves into `everglade/world/` later, so this chapter names functions
> and has no source listings yet.

This chapter answers the player's question: "Why does this forest feel real?"

A real forest has no repeated tiles and no empty patches that show a grid.
Trees crowd the path, the understory is thick at the verges and the woods thin
in small dells. Everglade II plants about 100,000 pieces of the Quaternius
Stylized Nature kit to get that look, and you place none of them by hand.

## What this chapter teaches

- How to plant a forest as a pure function of the land.
- How to make the result the same on every load, with no saved list.
- How tests keep the path clear and the woods dense.

## What exists today

`e2_plant(out, kinds, first)` plants the whole forest at load, in a fraction of
a second. It walks a grid of 16 m planting cells (`E2_CELL`), 27 across and 46
deep (`E2_CELLS_X`, `E2_CELLS_Z`), and calls `e2_plant_cell` for each one.
`first[c]` records where cell `c`'s pieces start, so the page can find the
pieces near any point.

Each piece is eight floats (`E2_ITEM_FLOATS`): position, yaw, vertical and
horizontal scale, and a packed tint. Its kit piece is one byte. The kit has 36
pieces (`E2_KIT_NAME`): 15 trees (oaks, birches and pines), bushes, ferns,
rocks, flowers, mushrooms, clover and pebbles.

The planting reads only the land functions in
[`verse/everglade2/forest.coax`](../../verse/everglade2/forest.coax):

| Function | What it says |
| --- | --- |
| `e2_forest(x, z)` | How thick the woods stand, from 0 to 1 |
| `e2_road_dist(x, z)` | Metres from the path's middle |
| `e2_path(x, z)` | How much bare path is here |
| `e2_clearing(x, z)` | How far inside the clearing this is |
| `e2_autumn(x, z)` | How golden the crowns are here |

## The method

### A seed from the cell, not from a clock

Each cell's random numbers come from its own position:
`seed = i × 131 + j × 977`. Every choice in the cell is `cg_hash(seed + k, n)`
for some small `k` and `n`. The same cell always plants the same pieces, so
the forest is the same on every load and on every machine, and nothing needs to
be saved.

### Trees

1. Lay a 2.6 m grid over the cell (`E2_TREE_STEP`) and jitter each point
   within its square.
2. Skip the point unless a hash is at most `0.97 × e2_forest`.
3. Skip it if it is on the path, in the clearing, or within 2.6 m of the path's
   middle.
4. Pick a kind by the autumn at that point: more birches and gold oaks in
   autumn patches, more green oaks and pines elsewhere.
5. Grow trees larger away from the path, up to 1.35 times at 30 m, so their
   crowns make a continuous canopy behind the near trunks.

### The understory

Each cell tries 120 understory points. The chance to keep one depends on where
it is: 0.95 on the verges (1.6 m to 6 m from the path), 0.8 among the trunks
within 35 m of the path and less deep in the woods. Ferns, bushes, flowers,
mushrooms, clover and rocks share the kept points. Now and then a cell gets a
boulder, never within 6 m of the path.

### Grass

The grass isn't planted on the CPU. The GPU places one tuft in each cell of a
grid fixed to the land (`field.coax`), so grass costs no planting time and no
memory for a list. [Chapter 10](ch010-hold-60-frames-per-second.md) shows how
it thins with distance.

## A worked example: one cell beside the path

Take a cell deep in the woods that the path crosses through its middle, from
north to south. The estimates below follow `e2_plant_cell`'s rules.

- **Trees.** The tree grid has 6 × 6 = 36 points. A band 5.2 m wide round the
  path's middle (2.6 m each side) is about a third of the cell's 16 m width, so
  about 12 points hold no tree. Beside the path `e2_forest` is above 0.8, so
  about 0.85 of the other 24 points keep a tree: about 20 trees.
- **The verges.** The verges run from 1.6 m to 6 m on each side, 8.8 m of the
  16 m width, so about 55% of the 120 understory points, or 66, fall on them.
  Each keeps a piece 95% of the time: about 63 pieces.
- **The rest.** About 30 points fall more than 6 m from the path, among the
  trunks. Each keeps a piece 80% of the time: about 24 pieces. The points on
  the bare path keep nothing.

That is about 107 pieces for one 256 m² cell beside the path. Across the
cells that have woods, the total comes to about 100,000. The planting stops at
170,000 pieces (`E2_ITEMS`), far above that.

## The checks

In `verse/everglade2/plant.coax`:

- `#test "the woods are planted, dense beside the path, none on it or in the
  clearing"`. It asserts that the planting makes more than 20,000 pieces and
  fewer than `E2_ITEMS`, that no piece stands where `e2_path` is above 0.35,
  that no tree stands within 2.5 m of the path or in the clearing, that there
  are more than 8,000 trees, and that more than 200 trees stand within 9 m of
  the path.

In `verse/everglade2/forest.coax`:

- `#test "the woods close round the glade and leave its open ground alone"`.
- `#test "nothing of the glade's stands on the path out"`: the glade's own
  composition, which isn't planted by this function, is filtered by
  `e2_glade_kept` ([chapter 8](ch008-if-you-can-see-it-you-can-walk-to-it.md)).

In `verse/everglade2/field.coax`:

- `#test "the grass covers the land, leaves the path bare and thins in the
  woods"`.

```sh
coax/bin/coax test --library verse/everglade2/plant.coax
coax/bin/coax test --library verse/everglade2/field.coax
```

## Mistakes

- **A tree on the path.** A jittered grid point can land on the path's edge.
  The planting skips a tree wherever `e2_path` is above 0.05, and the test
  checks every planted piece against the path.
- **A piece placed by hand.** A hand-placed piece doesn't move when the path
  moves. The glade's own composition is the one hand-made set in Everglade II,
  and it caused the invisible wall in
  [chapter 8](ch008-if-you-can-see-it-you-can-walk-to-it.md). Everything else
  is planted by this function.

## What comes next

[Chapter 6](ch006-the-glades-light.md) lights the glade and the woods.
