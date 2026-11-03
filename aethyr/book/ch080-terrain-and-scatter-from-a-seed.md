# 80. Terrain and scatter from a seed

> **Status: partly implemented.** The ground is chapter 8's heightfield,
> [aethyr/terrain.coax](../../aethyr/terrain.coax): rolling hills in a
> shallow bowl, levelled where paths run and on flats, sampled exactly as it
> is drawn. Scatter is new and shared:
> [aethyr/scatter.coax](../../aethyr/scatter.coax) places things on a
> jittered grid of world cells. Each cell's draws come from chapter 5's
> counter-based streams ([aethyr/rand.coax](../../aethyr/rand.coax)), keyed
> by the realm's seed, the layer's stable ID, the generator's version and the
> property, with the cell's grid coordinates as the counter. Keep-outs are
> data, and a scatter is placed whole or refused. The example below runs
> natively in debug and release. `coax/web/tool.sh replay-hosts scatter`
> builds the same site to wasm and runs it in headless Chrome. All seven
> digests (scatter, keep-outs, the trees' heights on the built ground, a
> second layer, a split window, another version and another seed) are equal
> to native. No realm draws its scatter through the module yet: Everglade,
> made realms and the Sandbox keep their own generators. Composition, edits
> keyed by derived IDs and cells streamed by chapter 38 are not built either.
> See [Still to do](#still-to-do).

A realm's ground and the things on it come from rules and a seed, not from a
hand-placed list. This chapter has two halves: a heightfield whose sampled
surface is the drawn one, and scatter that keeps clear of what must stay
open. The scatter draws nothing in sequence, so adding a layer, a keep-out
or a property moves nothing else.

## What this chapter teaches

- How a heightfield's sampled surface equals the drawn one, and how paths
  and flats level it.
- How to scatter on a jittered grid of world cells with keep-outs.
- Why a counter-based draw keyed by stable IDs beats one stream stepped in
  turn: a cell's thing is the same in any window and any load order.
- How to show that the same seed gives the same bits natively and in the
  browser.

## The ground

A `Ground` has `n` cells a side (or `n` × `rows`), `cell` metres each, from
its south-west corner, with (n + 1)² heights by rows from the south
([chapter 8](ch008-ground-and-collision.md)). Each cell is two triangles
split south-west to north-east, as the GPU draws it:

<!-- source: aethyr/ground.coax types -->
```jai
Ground :: struct {
  n: int = 64; cell: f32 = 1.5; west: f32 = -48; south: f32 = -48;
  rim_from: f32 = 32; rim: f32 = 0.022; rows: int = 0;
}
GroundSample :: struct {
  ok: bool;
  height: units.Quantity(units.Metre, f32);
  gx: f32; gz: f32;   // dy/dx and dy/dz of the selected drawn triangle
  clamped_x: bool; clamped_z: bool;   // strictly beyond the mesh, not on its edge
}
GroundNormal :: struct { x: f32; y: f32 = 1; z: f32; }
```

`verse_terrain_build` fills the heights. The hills are `HILL` (1.6 m) high
and rise toward the edge from `rim_from` metres out at `rim` metres per
metre squared, so a realm sits in a shallow bowl. Everglade sets its own rim
(`EG_RIM_FROM`, 80 m, at `EG_RIM`, 0.004). On each path strip the ground
runs at `PATH_LEVEL` (0.4) of the hills under it. On a flat (a plaza, a
pond) it runs at the flat's level. Both ease back to the hills over
`PATH_BLEND` (3 m). The builder refuses more than `TERRAIN_LINKS` (256) path
links. A height is read from the drawn triangle, so a foot set on it stands
on what you see:

<!-- source: aethyr/terrain.coax height -->
```jai
// The height of the drawn surface at x, z (the edge's beyond it).
verse_terrain_height :: (g: Ground, heights: [] f32, x: f32, z: f32) -> f32 #export_c {
  q := ground_sample(g, heights, terrain_units.Quantity.{value = x}, terrain_units.Quantity.{value = z});
  return q.height.value;   // an invalid sample publishes the legacy zero height
}
```

## Scatter

A layer is one kind of scatter: a forest, a tree line's bushes, a field of
stones. Its cells are the world's own: cell (i, j) covers
[i, i + 1) × [j, j + 1) spacings from the origin. A window asks for some of
those cells, and keep-outs (a circle, or a box) say what must stay open:

<!-- source: aethyr/scatter.coax layer -->
```jai
SCATTER_PROPS :: 8;               // property streams a layer may use
SCATTER_LAYERS :: 1099511627776;  // 2^40 stable layer IDs
SCATTER_VERSIONS :: 65536;        // generator versions
SCATTER_REACH :: 1048576;         // 2^20 cells either side of the origin, each way
SCATTER_KINDS :: 65536;           // kinds a layer may choose among
SCATTER_TAU :: 6.28318530717958647692;
// The property streams.
S_X :: 0; S_Z :: 1; S_SIZE :: 2; S_YAW :: 3; S_KIND :: 4; S_FILL :: 5;

// One layer of scatter: a forest, a tree line's bushes, a field of stones.
ScatterLayer :: struct {
  layer: int;            // the layer's stable ID, from the realm document; never reused
  version: int = 1;      // the generator's version: raised when these rules change
  spacing: f32 = 4;      // metres a cell, each way
  jitter: f32 = 0.6;     // the share of its cell a thing may stray from the middle, 0 to 1
  fill: f32 = 1;         // the share of cells that get a thing, 0 to 1
  size_lo: f32 = 1;      // a thing's size is drawn from size_lo to size_hi
  size_hi: f32 = 1;
  kinds: int = 1;        // and its kind from 0 to kinds - 1
  clear: f32 = 1;        // metres it keeps from every keep-out
}
// What must stay open: a circle (x0, z0, radius r) or a box (x0, z0 to x1, z1).
Keep :: struct { box: bool; x0: f32; z0: f32; x1: f32; z1: f32; r: f32; }
keep_circle :: (x: f32, z: f32, r: f32) -> Keep { return Keep.{x0 = x, z0 = z, r = r}; }
keep_box :: (x0: f32, z0: f32, x1: f32, z1: f32) -> Keep { return Keep.{box = true, x0 = x0, z0 = z0, x1 = x1, z1 = z1}; }
// The cells asked for: cols west to east from cell i0, rows south to north from cell j0.
Window :: struct { i0: int; j0: int; cols: int; rows: int; }
// One placed thing. Its name is its layer and its cell (i, j).
Placed :: struct { layer: int; i: int; j: int; x: f32; z: f32; size: f32; yaw: f32; kind: int; }
```

### The keys

Every property draws from its own stream: the version, the layer and the
property each get their own bits. Every cell takes its own draw of each
stream: its grid coordinates, never its place in a list.

<!-- source: aethyr/scatter.coax keys -->
```jai
// Which stream a property draws from: the version, the layer and the property, each in its
// own bits, so no two of them share a stream.
scatter_stream :: (l: ScatterLayer, prop: int) -> int {
  return (l.version * SCATTER_LAYERS + l.layer) * SCATTER_PROPS + prop;
}
// Which draw of the stream a cell takes: its grid coordinates, so a cell's draws are its own.
scatter_counter :: (i: int, j: int) -> int {
  return (j + SCATTER_REACH) * (2 * SCATTER_REACH) + (i + SCATTER_REACH);
}
scatter_unit :: (seed: int, l: ScatterLayer, prop: int, i: int, j: int) -> f32 {
  return rand_unit(seed, scatter_stream(l, prop), scatter_counter(i, j));
}
```

The rules from the design hold here:

1. **A layer has a stable ID.** It comes from the realm document
   ([chapter 51](ch051-a-realm-is-a-document.md)) and is never reused.
2. **A generated thing is named by its layer and its cell.** Its name doesn't
   change when a list is reordered or another thing is inserted.
3. **Each property has its own stream.** If you draw kinds among more of
   them, only the kinds change. The positions, sizes and turns stay the same
   (the module's check "another layer, or one property more, moves nothing").
4. **The generator's version is part of the key.** If you raise it, the
   layout is new. A layer that keeps the old version keeps its layout.

### One cell

A cell is filled when its fill draw is below the layer's `fill` share. Its
thing strays from the cell's middle by up to half the `jitter` share of a
cell each way, so with a jitter of 1 or less it stays in its own cell:

<!-- source: aethyr/scatter.coax cell -->
```jai
// The thing in cell (i, j), if the cell is filled: kind -1 when it isn't. Its place strays
// from the cell's middle by up to half the jitter's share of a cell, each way.
scatter_cell :: (seed: int, l: ScatterLayer, i: int, j: int) -> Placed {
  p := Placed.{layer = l.layer, i = i, j = j, kind = -1};
  if scatter_unit(seed, l, S_FILL, i, j) >= l.fill { return p; }
  p.x = (f32(i) + 0.5 + (scatter_unit(seed, l, S_X, i, j) - 0.5) * l.jitter) * l.spacing;
  p.z = (f32(j) + 0.5 + (scatter_unit(seed, l, S_Z, i, j) - 0.5) * l.jitter) * l.spacing;
  p.size = l.size_lo + scatter_unit(seed, l, S_SIZE, i, j) * (l.size_hi - l.size_lo);
  p.yaw = scatter_unit(seed, l, S_YAW, i, j) * f32(SCATTER_TAU);
  p.kind = rand_below(seed, scatter_stream(l, S_KIND), scatter_counter(i, j), l.kinds);
  return p;
}
```

### Keep-outs

A thing stays `clear` metres from every keep-out. A circle reaches its
radius plus the clearance. A box reaches its edges plus the clearance, and
a point inside a box is never clear, even with no clearance. The distance
outside a box is computed without comparisons, from
|x − x0| + |x − x1| − (x1 − x0), which is twice the gap on that axis and 0
between the box's sides:

<!-- source: aethyr/scatter.coax clear -->
```jai
// Whether x, z keeps `clear` metres from every keep-out. Inside a box is never clear.
scatter_clear :: (keeps: [] Keep, x: f32, z: f32, clear: f32) -> bool {
  for k: keeps {
    if k.box {
      inside := x >= k.x0 && x <= k.x1 && z >= k.z0 && z <= k.z1;
      // how far outside the box each way: 0 between its sides
      dx := (abs(x - k.x0) + abs(x - k.x1) - (k.x1 - k.x0)) * 0.5;
      dz := (abs(z - k.z0) + abs(z - k.z1) - (k.z1 - k.z0)) * 0.5;
      if inside || dx * dx + dz * dz < clear * clear { return false; }
    } else {
      dx := x - k.x0; dz := z - k.z0; reach := k.r + clear;
      if dx * dx + dz * dz < reach * reach { return false; }
    }
  }
  return true;
}
```

### A window, placed whole

`scatter_fill` counts first and writes second. A scatter that doesn't fit
in its output is refused with nothing written. Everglade's `plant_forest`
stops silently at its `TREES` limit, so the trees past the limit are lost
without a word:

<!-- source: aethyr/scatter.coax fill -->
```jai
// Every thing a window's cells hold, clear of the keep-outs, in out: by rows from the south,
// west to east in each. The count, or -1 with nothing written when the layer or window can't be
// drawn or out hasn't room for them all: a scatter is placed whole or not at all.
scatter_fill :: (seed: int, l: ScatterLayer, w: Window, keeps: [] Keep, out: [] Placed) -> int {
  if !scatter_layer_ok(l) || !scatter_window_ok(w) { return -1; }
  n := 0;
  for pass: 0..<2 {
    for j: w.j0..<w.j0 + w.rows {
      for i: w.i0..<w.i0 + w.cols {
        p := scatter_cell(seed, l, i, j);
        if p.kind < 0 || !scatter_clear(keeps, p.x, p.z, l.clear) { continue; }
        if pass == 1 { out[n] = p; }
        n += 1;
      }
    }
    if n > out.count { return -1; }
    if pass == 0 { n = 0; }
  }
  return n;
}
```

Two scatters are the same when their digests are, FNV-1a over every field's
32-bit words, as chapter 5's step digests are:

<!-- source: aethyr/scatter.coax digest -->
```jai
// The bits of every placed thing, in order: two scatters are the same exactly when their
// digests are (FNV-1a over 32-bit words, as aethyr/replay.coax's digests).
scatter_mix :: (h: int, v: int) -> int { return ((h ^ (v & 4294967295)) * 16777619) % 4294967296; }
scatter_digest :: (out: [] Placed, n: int) -> int {
  h := 2166136261;
  for k: 0..<n {
    p := out[k];
    h = scatter_mix(h, p.layer); h = scatter_mix(h, p.layer >> 32);
    h = scatter_mix(h, p.i); h = scatter_mix(h, p.j);
    h = scatter_mix(h, cast(int) f32_bits(p.x)); h = scatter_mix(h, cast(int) f32_bits(p.z));
    h = scatter_mix(h, cast(int) f32_bits(p.size)); h = scatter_mix(h, cast(int) f32_bits(p.yaw));
    h = scatter_mix(h, p.kind);
  }
  return h;
}
```

## A site, worked by hand

The example's site is a 48 m ground of 2 m cells round the origin, one path
west to east, a pond and a hut. The forest is layer 7 at version 1, 4 m
cells, 85% filled:

<!-- source: aethyr/tests/scatter-hosts.coax site -->
```jai
// The site: a 48 m ground of 2 m cells round the origin, one path west to east 10 m north of the
// middle, and a pond levelled at 0.2 m below the start. The pond, the path's strip and a hut
// are kept clear of trees.
sh_ground :: () -> Ground { return Ground.{n = 24, cell = 2, west = -24, south = -24}; }
SH_NODES :: [4] f32.[-24, 10, 24, 10];
SH_LINKS :: [2] int.[0, 1];
SH_HALF :: 1.5;
SH_FLATS :: [4] f32.[8, -8, 5, -0.2];
sh_keeps :: () -> [3] Keep {
  return [3] Keep.[keep_circle(8, -8, 5), keep_box(-24, 8.5, 24, 11.5), keep_box(-14, -4, -8, 2)];
}
sh_forest :: () -> ScatterLayer {
  return ScatterLayer.{layer = 7, spacing = 4, jitter = 0.6, fill = 0.85, size_lo = 0.75, size_hi = 1.25, kinds = 4, clear = 1};
}
sh_rocks :: () -> ScatterLayer {
  return ScatterLayer.{layer = 8, spacing = 3, jitter = 0.9, fill = 0.3, size_lo = 0.3, size_hi = 0.8, kinds = 2, clear = 0.5};
}
SH_WINDOW :: Window.{i0 = -6, j0 = -6, cols = 12, rows = 12};
```

The first tree is cell (−6, −6). Its counter is
(−6 + 2²⁰) · 2²¹ + (−6 + 2²⁰) = 2,199,011,721,210. Its streams are
(1 · 2⁴⁰ + 7) · 8 + property: 8,796,093,022,264 for x and
8,796,093,022,269 for the fill. `aethyr/tests/rand_oracle.py`, written from
the published hash and not from the Coax, gives these draws:

| Property | Stream | 32 bits | Unit |
| --- | --- | --- | --- |
| fill | …269 | 301,785,690 | 0.0702649 (below 0.85: filled) |
| x | …264 | 2,382,871,905 | 0.5548056 |
| z | …265 | 1,805,547,636 | 0.4203868 |
| size | …266 | 221,245,048 | 0.0515126 |
| kind | …268 | `below(…, 4)` = 2 | |

So x = (−6 + 0.5 + (0.5548056 − 0.5) · 0.6) · 4 = −21.868 m,
z = (−5.5 + (0.4203868 − 0.5) · 0.6) · 4 = −22.191 m, and the scale is
0.75 + 0.0515126 · 0.5 = 0.776. The example prints that tree as
(−2187 cm, 31 cm, −2219 cm), scale 78/100, kind 2. The 31 cm is the built
ground's height under it.

## The example

[aethyr/examples/scatter-from-a-seed.coax](../examples/scatter-from-a-seed.coax)
builds the ground and scatters the forest with no keep-outs, then with
them. It checks that no tree stands inside a keep-out, adds a layer of
rocks and scatters the trees again. It sets this against Everglade's way:
one stream stepped five times a cell, with one draw added before the first
cell. Then it prints the seven digests the browser must match:

<!-- source: aethyr/examples/scatter-from-a-seed.coax example -->
```jai
#load "../tests/scatter-hosts.coax";
#import "Basic";

cm :: (m: f32) -> int { return cast(int) (m * 100 + ifx m < 0 then -0.5f32 else 0.5f32); }

// Everglade's way (apps/openagents-web/everglade.coax next_seed): one stream stepped in turn.
lcg :: (seed: int) -> int { return (seed * 1103515245 + 12345) % 2147483648; }

// The site of aethyr/tests/scatter-hosts.coax: a forest over a built ground, with a pond, a
// path and a hut kept clear. Then what moves when something is added, both ways, and the
// digests the browser must match.
main :: () {
  heights: [625] f32;
  print("ground built: %\n", sh_heights(heights[..]));
  out: [SH_MOST] Placed;
  keeps := sh_keeps();
  open := scatter_fill(SH_SEED, sh_forest(), SH_WINDOW, keeps[0..<0], out[..]);
  n := scatter_fill(SH_SEED, sh_forest(), SH_WINDOW, keeps[..], out[..]);
  print("cells %, trees % with nothing kept clear, % with the pond, the path and the hut\n",
        SH_WINDOW.cols * SH_WINDOW.rows, open, n);
  inside := 0;
  for t: 0..<n { if !scatter_clear(keeps[..], out[t].x, out[t].z, 0) { inside += 1; } }
  print("trees inside a keep-out: %\n", inside);
  for t: 0..<3 {
    p := out[t];
    y := verse_terrain_height(sh_ground(), heights[..], p.x, p.z);
    print("tree (%, %): at (% cm, % cm, % cm), scale %/100, kind %\n", p.i, p.j, cm(p.x), cm(y), cm(p.z), cm(p.size), p.kind);
  }

  // a second layer: the trees keep every number
  rocks: [SH_MOST] Placed;
  r := scatter_fill(SH_SEED, sh_rocks(), Window.{i0 = -8, j0 = -8, cols = 16, rows = 16}, keeps[..], rocks[..]);
  again: [SH_MOST] Placed;
  a := scatter_fill(SH_SEED, sh_forest(), SH_WINDOW, keeps[..], again[..]);
  print("rocks added: %, trees the same: %\n", r, a == n && scatter_digest(again[..], a) == scatter_digest(out[..], n));

  // one stream in turn, five draws a cell, with one more draw before the first cell
  moved := 0;
  s1 := 12345; s2 := lcg(12345);
  for c: 0..<SH_WINDOW.cols * SH_WINDOW.rows {
    for d: 0..<5 { s1 = lcg(s1); s2 = lcg(s2); }
    if s1 != s2 { moved += 1; }
  }
  print("one stream, one draw added: % of % cells moved\n", moved, SH_WINDOW.cols * SH_WINDOW.rows);

  print("digests");
  for k: 0..<SH_CASES { print(" %", scatter_case(k)); }
  print("\n");
}
```

```text
ground built: true
cells 144, trees 125 with nothing kept clear, 106 with the pond, the path and the hut
trees inside a keep-out: 0
tree (-6, -6): at (-2187 cm, 31 cm, -2219 cm), scale 78/100, kind 2
tree (-5, -6): at (-1805 cm, 84 cm, -2271 cm), scale 93/100, kind 2
tree (-4, -6): at (-1492 cm, 85 cm, -2124 cm), scale 83/100, kind 3
rocks added: 58, trees the same: true
one stream, one draw added: 144 of 144 cells moved
digests 4254204106 357612983 2780734763 4141171555 357612983 62911725 236774374
```

The keep-outs take 19 of the 125 trees, and every tree left is the open
scatter's tree with the same bits (the module's check "a keep-out takes
away only what it covers"). With one stream stepped in turn, one added draw
moves all 144 cells. With keyed draws, the rocks move none of the trees.
The fifth digest equals the second, because the window asked for in two
halves gives the same things as the whole.

## Checks

- `coax/bin/coax test aethyr/scatter.coax --library`: 17 of 17. Layers and
  windows are drawn only in range. Streams and counters never collide. A
  cell's numbers are its draws, worked through, and it is filled exactly
  when its fill draw is below the share. Fill 0 places nothing, fill 1 fills
  every cell, and no jitter puts each thing in its middle. A keep-out's
  reach is checked at each side, a corner and exactly at the clearance.
  Other checks: refusal when there's no room, a split window equal to the
  whole, another layer or property moving nothing, a new version, and the
  digest.
- `coax/bin/coax mutate aethyr/scatter.coax --library --root-only`: 188
  mutants, 188 killed.
- `coax/bin/coax test aethyr/tests/scatter-hosts.coax --library`: the seven
  cases draw, and the split window is the whole.
- `coax/web/tool.sh replay-hosts scatter`: aethyr/tests/scatter-hosts.coax
  built as pages are built (`coax/web/wasm.sh`: -O2, no fast math, no
  contraction) and run in headless, muted Chrome. Its seven digests equal
  the native run's, including every tree's height on the built ground,
  whose hills take sines and cosines.

## Still to do

- **Realms on the module.** Everglade's forest, tree line and thicket
  (`plant_forest`, `plant_thicket` in
  `apps/openagents-web/everglade.coax`) still step one linear congruential
  stream. Made realms seed a Park–Miller stream from an FNV hash of the
  realm's text (`seed_of`, `rand` in `apps/openagents-web/myrealm.coax`).
  Moving them changes their layouts, so each move needs its goldens
  re-approved. Everglade is being changed by its owner now and is left
  alone here.
- **Regions as data.** Everglade keeps its trees in the woods
  (`everglade_in_woods`) and on the tree line. Scatter has keep-outs but no
  keep-ins, and no "near an edge" rule.
- **Composition.** Made realms put the asked-for features in slots ahead of
  the start (`SLOT_A`, `NEAR_A` in `myrealm-shapes.coax`) and frame them
  with trees. That stays in the realm. An `aethyr/forge/compose.coax` with
  checks is still to do.
- **Edits over generated content.** A moved or removed tree, stored in the
  realm document under its layer and cell and applied after any
  regeneration that keeps the name ([chapter 52](ch052-edit-a-realm-in-place.md)).
- **Streamed cells.** Scatter's cells are world cells, and a window's things
  don't depend on the window. Generating per cell of
  [chapter 38](ch038-stream-a-large-world.md)'s streamed world, with a
  floating origin, is still to do.
- **The document's seed and version.** A realm document records the seed and
  the generator version its layers use. Today they are arguments.

## What exists today

- [aethyr/terrain.coax](../../aethyr/terrain.coax) and
  [aethyr/ground.coax](../../aethyr/ground.coax): the heightfield, hills,
  path levelling, height, normal and ray queries.
- [aethyr/scatter.coax](../../aethyr/scatter.coax): keyed scatter with
  keep-outs.
- [aethyr/rand.coax](../../aethyr/rand.coax): the counter-based draw, with
  its Python oracle.
- [verse/everglade/plan.coax](../../verse/everglade/plan.coax): Everglade's
  ground, bounds, streets and clearings.
- [apps/openagents-web/everglade.coax](../../apps/openagents-web/everglade.coax):
  Everglade's forest scatter (`TREE_SPACING`, 4.2 m).
- [apps/openagents-web/myrealm-shapes.coax](../../apps/openagents-web/myrealm-shapes.coax)
  and [myrealm-site.coax](../../apps/openagents-web/myrealm-site.coax):
  made-realm composition, and a site laid out round a middle.
- [docs/verse/realm-builder.md](../../docs/verse/realm-builder.md),
  "Composition": the placement rules for made realms.

## Acceptance checks and oracles

The chapter is done when:

- `verse_terrain_height` and the drawn mesh agree at every vertex and at
  sampled interior points. Chapter 8's checks hold this.
- No scattered thing stands in a keep-out, for every realm. Holds for the
  module. Realms are not on it yet.
- The same document and seed give byte-identical ground and scatter natively
  and in the browser. Holds for the module's site (`replay-hosts scatter`).
- Adding a layer or a keep-out leaves every other placement unchanged.
  Holds for the module. Edits keyed by name are still to do.
- A streamed cell generates the same content in any load order. Holds for a
  window's cells, but chapter 38's streaming isn't wired in yet.
- Made-realm goldens change only where a fix is visible. Not started.

## Dependencies

Chapters 2 (spaces and units), 5 (the counter-based draw), 7 (queries over
the ground), 8 (ground and collision), 16 (budgets), 38 (streaming in
cells) and 51 (the realm document and its seed).
