# 4. Ground you can stand on

> **Status: implemented.** Everglade II's ground is a set of Metaverse Engine
> terrain tiles in [`everglade/world/ground.coax`](../../everglade/world/ground.coax),
> built on [`metaverse-engine/world/terrain-tile.coax`](../../metaverse-engine/world/terrain-tile.coax)
> (#12995, #13059, #13509). The listings below are copied from `// book:begin`
> markers in those two files.

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
| The land's height as a pure function | `e2_height` in [`everglade/world/forest.coax`](../../everglade/world/forest.coax) |
| A tile's format, admission and sampler | [`metaverse-engine/world/terrain-tile.coax`](../../metaverse-engine/world/terrain-tile.coax) (`terrain_tile_sample`, `terrain_tile_stitched`) |
| Everglade II's tiles, record and queries | [`everglade/world/ground.coax`](../../everglade/world/ground.coax) (`e2g_convert`, `e2g_seams`, `e2g_publish`, `e2g_record_read`, `e2g_y`) |
| The ground's colour at a pixel | `e2_ground_albedo` in [`everglade/client/gpu/everglade2.coax`](../client/gpu/everglade2.coax) |

Inside the glade, the land is exactly Character Create's own (`cg_height` in
[`metaverse/character-glade.coax`](../../metaverse/character-glade.coax)). Outside it,
`e2_height` adds the path's valley and the wooded ridges.

## The method

### One lattice for every tile

Every tile lies on one lattice: the glade's 0.65 m samples, starting at
(-32.5, -60). `e2g_at(base, index)` works out each sample's position once, in
64-bit floats, and rounds once to 32 bits. A sample that two tiles share is the
same index on the same lattice, so it is the same point and the same height in
both tiles, bit for bit.

<!-- source: everglade/world/ground.coax lattice -->
```jai
e2g_s64 :: () -> f64 { return f64(f32(E2G_SPACING)); }
// Lattice coordinate `index` (half-steps allowed through `half`) from base: one f64 sum, one rounding.
e2g_at :: (base: f32, index: int) -> f32 { return f32(f64(base) + f64(index) * e2g_s64()); }
e2g_mid :: (base: f32, index: int, step: int) -> f32 { return f32(f64(base) + (f64(index) + 0.5 * f64(step)) * e2g_s64()); }
```

The set has 24 tiles (room for `E2G_TILES` = 26):

- **The glade, tile 0.** 100 cells of 0.65 m, from (-32.5, -60) to (32.5, 5),
  with no holes. Each cell splits on the same diagonal as the pack's old
  triangles.
- **The path and clearings.** 65 m tiles in 8 rows south of the glade. Rows 0
  to 4 have 100 cells of 0.65 m. Rows 5 to 7, round the end clearing, have 50
  cells of 1.3 m.
- **Holes.** Outside the glade, a cell is solid along the ribbon round the
  path, in the city clearing, and along its brook's banks (`e2g_solid`).
  Elsewhere it is a hole. A tile with no solid cell is left out.

### Seams

Neighbours with the same spacing repeat their shared edge bit for bit
(`terrain_tile_edges_agree`). Where a 0.65 m row meets a 1.3 m row, at
z = -385, the finer edge is stitched onto the coarser one
(`terrain_tile_stitched`), so no edge cracks. At draw time, distant tiles can
draw at coarser levels (`E2G_LOD_NEAR` = 48 m, `E2G_LOD_MID` = 128 m), and
`e2g_tile_code` stitches any side that meets a coarser neighbour.

<!-- source: everglade/world/ground.coax seams -->
```jai
// 0 when every seam holds, else 1 + the tile whose seam fails.
e2g_seams :: (s: [] E2Tiles) -> int {
  for b: 0..<s[0].n {
    mb := s[0].m[b]; kb := s[0].k[b];
    // b's west neighbour, then its south one
    a := e2g_find(s, mb - 1, kb);
    if a >= 0 && !e2tt.terrain_tile_edges_agree(s[0].tile[a], e2g_h(s, a), e2g_id(s, a), e2g_wt(s, a), s[0].tile[b], e2g_h(s, b), e2g_id(s, b), e2g_wt(s, b)) { return 1 + b; }
    a = e2g_find(s, mb, kb + 1);
    if a < 0 { continue; }
    if e2g_step(kb + 1) == e2g_step(kb) {
      if !e2tt.terrain_tile_edges_agree(s[0].tile[a], e2g_h(s, a), e2g_id(s, a), e2g_wt(s, a), s[0].tile[b], e2g_h(s, b), e2g_id(s, b), e2g_wt(s, b)) { return 1 + b; }
      continue;
    }
    // a coarse tile south of a fine one: its north samples are the fine south edge's even ones, and
    // the fine edge's odd samples lie on the line between them
    na := e2g_n(s, a); nb := e2g_n(s, b); ha := e2g_h(s, a); hb := e2g_h(s, b);
    if nb != na * 2 { return 1 + b; }
    for e: 0..<nb + 1 {
      if f32_bits(hb[e]) != f32_bits(e2tt.terrain_tile_stitched(hb, nb, 0, 1, e)) { return 1 + b; }
      if e % 2 == 0 && f32_bits(hb[e]) != f32_bits(ha[e2tt.terrain_tile_edge_index(na, 2, e / 2)]) { return 1 + b; }
    }
  }
  return 0;
}
```

### One answer for "how high is the ground here"

`e2g_y(g, x, z)` answers the walk. On a tile it samples the tile's own
triangles with `terrain_tile_sample`, the same triangles the GPU draws. Past
the tiles, it falls back to `e2_height` and counts the fallback (`e2_probe`
98).

<!-- source: everglade/world/ground.coax query -->
```jai
// Where you stand at (x, z), in the realm's frame: the admitted tile's triangle where a tile has
// ground there, e2_height past them (counted).
e2g_y :: (g: [] E2Ground, x: f32, z: f32) -> f32 {
  if g[0].ok {
    s := e2g_live_set(g);
    for t: 0..<s[0].n {
      f := s[0].frame[t]; span := f32(f.n) * f.cell;
      if x < f.west || x > f.west + span || z < f.south || z > f.south + span { continue; }   // the sampler clamps past its edges
      q := e2tt.terrain_tile_sample(f, e2g_h(s, t), e2g_hl(s, t), x, z);
      if q.ok { return q.height; }
    }
  }
  g[0].outside += 1;
  return e2_height(x + f32(g[0].frame_x), z + f32(g[0].frame_z));
}
```

The pack draws the ground 0.035 m lower than the tiles (`E2G_DRAW_DROP`). That
offset is declared, so it is decoration, not disagreement.

### Admit the whole set, or keep the old one

The pack tool converts the land once and writes the whole set into the pack's
catalog as one record (E2T1). The page reads it with `e2g_record_read`. Each
tile must come out at the revision that the record names, and then the seams
and the set's revision are checked. Only then does `e2g_publish` switch the
live set, whole. If anything refuses, the page counts the refusal and keeps
the set it had. With no record, the page converts the land itself.

<!-- source: everglade/world/ground.coax publish -->
```jai
// Publishes the staged set (sets[1 - cur]) when its seams hold and every frame converts. The status.
// The set may be any of the tiles (by cell, the cells' that are in); the bake needs the glade's.
e2g_publish :: (g: [] E2Ground, route: int) -> int {
  if !e2g_stored(g) { return e2g_refuse(g, E2G_REC_LAYOUT); }
  nx := 1 - g[0].cur; s := g[0].sets[nx..<nx + 1];
  if s[0].n < 0 || s[0].n > E2G_TILES { return e2g_refuse(g, E2G_REC_LAYOUT); }
  if e2g_seams(s) != 0 { return e2g_refuse(g, E2G_REC_SEAM); }
  e2g_frames(g, s);
  for t: 0..<s[0].n { if s[0].frame[t].n == 0 { return e2g_refuse(g, E2G_REC_LAYOUT); } }
  g[0].cur = nx; g[0].ok = true; g[0].route = route; g[0].published += 1;
  gi := e2g_find(s, 0, -1); g[0].glade = gi >= 0;
  if g[0].glade {
    g[0].tile = s[0].tile[gi]; g[0].frame = s[0].frame[gi]; g[0].receipt = s[0].receipt[gi];
    if !e2tt.terrain_tile_same(g[0].key.tile, g[0].receipt.output) { g[0].baked = -1; }   // a new glade tile wants a new bake
  }
  e2g_gpu(g);
  return E2G_REC_OK;
}
```

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
side. The fine edge then follows the coarse edge exactly, and no gap opens
between them.

<!-- source: metaverse-engine/world/terrain-tile.coax lod -->
```jai
// LOD profile v1: level L draws every 2^L-th sample (L 0..2; cells divisible by 2^L). Where a
// side meets a neighbour drawn at a coarser level, its vertices between the coarse ones are
// moved onto the line between them, so the finer edge lies on the coarser one: no crack, and
// the coarse vertices keep their stored bits. Returns the stitched height of vertex e along the
// side (0 for any request outside the profile).
terrain_tile_stitched :: (heights: [] f32, n: int, side: int, level: int, e: int) -> f32 #effects(all) {
  if level < 0 || level > 2 || e < 0 || e > n || heights.count != (n + 1) * (n + 1) { return 0; }
  s := 1 << level;
  if n % s != 0 { return 0; }
  lo := (e / s) * s;
  a := heights[terrain_tile_edge_index(n, side, lo)];
  if lo == e { return a; }
  b := heights[terrain_tile_edge_index(n, side, lo + s)];
  return f32(f64(a) + (f64(b) - f64(a)) * f64(e - lo) / f64(s));
}
```

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

In `everglade/world/ground.coax`:

- `#test "the glade's tile is the pack's old grid: admitted, its samples the
  land, its weights the layers"`.
- `#test "the path's tiles meet the glade and each other bit for bit, the coarse
  rows on the fine edge, and the walk never steps"`.
- `#test "the record round-trips: the pack's route admits the same set, and any
  damage refuses with the old set kept"`.
- `#test "far from the origin the tiles answer as at it: queries, the drawn grid
  and the bake's frame agree"`. A set placed 1e8 m out answers the same queries.
- `#test "the drawn ground: normals as the pack drew them, every level's corners
  filled, the GPU's slots and the far record"`.

In `everglade/world/forest.coax`:

- `#test "the land is the glade's inside the glade and the path leaves it to the
  north"`.

In `metaverse-engine/world/terrain-tile.coax`:

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
coax/bin/coax test everglade/world/ground.coax --library
coax/bin/coax test everglade/world/forest.coax --library
coax/bin/coax test --library metaverse-engine/world/terrain-tile.coax
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
