# 9. A path through dense woods

> **Status: partly implemented.** The path is a set of pure functions in
> [`verse/everglade2/forest.coax`](../../verse/everglade2/forest.coax)
> (`e2_road_x`, `e2_road_dist`, `e2_level`, `e2_path`). The walk along it is in
> [`apps/openagents-web/everglade2.coax`](../../apps/openagents-web/everglade2.coax).
> The code moves into `everglade/world/` later, so this chapter names functions
> and has no source listings yet.

This chapter answers the player's question: "Where does this path go?"

The path leaves the glade on the glade's own road and winds about 440 m south
through a valley to a clearing. The woods crowd it on both sides, and the
canopy closes over it. It is the first journey in Everglade, and later the way
to the city.

## What this chapter teaches

- How to describe a winding road as a function of one coordinate.
- How to hold the road's grade under a limit.
- How the land, the woods and the walk all read the same road.
- How a test can walk the road on its own.

## What exists today

| Function | What it gives |
| --- | --- |
| `e2_road_x(z)` | The path's middle at z: the glade's road to z = -30, then a long winding way |
| `e2_road_dist(x, z)` | Metres from the middle, measured across the path where it slants |
| `e2_level(z)` | The path's height at z |
| `e2_path(x, z)` | How much bare dirt is at a point, from 0 to 1 |
| `e2_height(x, z)` | The land, flattened onto `e2_level` along a corridor round the path |
| `e2_forest(x, z)` | How thick the woods are, kept off the path from 2.5 m to 4 m out |
| `e2_auto_heading(x, z)` | Which way the `?walk` mode steers you along the path |

`E2_CLEAR_Z` (-470) is the clearing's middle, and `E2_CLEAR_R` (34 m) its
radius.

## The method

### A road as a function of z

The road never doubles back, so you can write its middle as `x = e2_road_x(z)`.
Near the glade it is the glade's own road (`cg_road_x`). From z = -30 to -75 it
blends into a sum of sines:

```text
t    = z + 30
wind = 15 sin(0.022 t) + 6 sin(0.051 t + 1.3) − 6 sin(1.3) + 2 sin(0.12 t)
x    = 3.3 + wind
```

The long sine makes the big bends, and the short ones break up the rhythm. The
`− 6 sin(1.3)` term makes the wind start at zero where the blend begins, so the
two roads meet without a kink.

### Distance across a slanted road

Where the road turns, it runs at a slant. A plain `|x − e2_road_x(z)|`
overstates the distance there. `e2_road_dist` divides by the road's slope:

```text
slope = e2_road_x(z + 0.5) − e2_road_x(z − 0.5)
dist  = |x − e2_road_x(z)| / sqrt(1 + slope²)
```

The path's width, the woods' edge, the planting and the walk bounds all use
this distance, so they agree on where the path's edge is.

### A level road in uneven land

The wild land has a valley and ridges (`e2_wild`). Along the path, `e2_height`
pulls the land onto `e2_level(z)`, fully within 2 m of the middle and fading
out by 5 m. The road stays level across its width, and the banks rise away
from it.

### The woods close round it

`e2_forest` is zero on the road and reaches its full value from 4 m out. The
first trunks stand 2.6 m from the middle, and trees grow larger away from the
path, so their crowns close over it
([chapter 5](ch005-plant-a-forest-with-a-function.md)). The follow camera
keeps in front of the trunks between it and you, so the woods never hide your
character.

## A worked example: the grade limit

Past z = -90, the path's height is:

```text
e2_level(z) = base + 4.5 sin(0.013 t) + 2.0 sin(0.034 t),   t = z + 40
```

The steepest the sum can climb is the sum of each term's steepest slope:

```text
4.5 × 0.013 + 2.0 × 0.034 = 0.0585 + 0.068 = 0.1265 m per metre of z
```

That is about one in eight. Along the path itself the grade is less, because
the path slants across z where it turns. The test's limit is one in five: no
step of 0.5 m in z may rise more than 0.1 m. So the design leaves room, and a
change that makes the hills steeper fails before a player feels it.

The path's width comes from `e2_path`. Its spread is `0.5 + 0.08 |z|`, capped
at 1.9. Half the path is dirt where `e2_path` is 0.5:

```text
exp(−d² / 1.9) = 0.5   →   d = sqrt(1.9 × ln 2) ≈ 1.15 m
```

So the dirt is about 2.3 m wide.

## The checks

In `verse/everglade2/forest.coax`:

- `#test "the land is the glade's inside the glade and the path leaves it to the
  north"`.
- `#test "the path winds and never climbs steeper than one in five"`. It also
  asserts that the path swings more than 25 m from side to side.
- `#test "the valley rises away from the path into wooded ridges"`.

In `apps/openagents-web/everglade2.coax`:

- `#test "a page's ?at= says how far down the path you start"`.
- `#test "the walk steers along the path and into the clearing"`.

In `apps/openagents-web/everglade2-walk-test.coax`:

- The three whole-path runs, at the middle and a metre to each side
  ([chapter 8](ch008-if-you-can-see-it-you-can-walk-to-it.md)).

Try it in a browser with `/worlds/everglade2?at=200&walk`: you start 200 m down
the path and walk to the clearing on your own.

## Mistakes

- **A road that doesn't meet the glade's road.** Two road functions joined at
  a point meet with a kink unless both their position and their direction
  agree. The blend from z = -30 to -75 and the `− 6 sin(1.3)` term make them
  agree, and the first test checks that the road is the glade's own up to the
  glade's edge.
- **An edge on open ground.** The walk bounds stop you 11 m from the path's
  middle. If the woods were thin there, you would hit an invisible edge in an
  open meadow. `"wherever you may not go on, thick woods stand: no edge on open
  ground"` walks both sides of the path and asserts thick woods past the edge.

## What comes next

[Chapter 10](ch010-hold-60-frames-per-second.md) keeps this many trees at 60
frames per second.
