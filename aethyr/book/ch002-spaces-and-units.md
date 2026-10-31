# 2. Spaces and units

> Engine-core task: `ec-space` (#12609). The module,
> [aethyr/space.coax](../space.coax), and its refusal check,
> [aethyr/refusals.coax](../refusals.coax), landed in `a93d35c2dd`. The task
> stays open for one check that needs the realms on it: the figure's feet
> drawn at their physics height in every realm, which lands with the realm
> migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md)).

A number such as `1.8` is a character's height in metres, a distance on the
Grid, a pixel offset on a phone or a depth in front of the camera. Engine
bugs are often a value read in the wrong one of these. A figure's feet drawn
at floor height while the camera rose with the jump, a shadow placed from a
stale position, a label projected with the previous frame's camera: each is a
position used in a space it was not computed in. Aethyr gives each space its
own type, so the compiler refuses the mix.

## What this chapter teaches

- The engine's spaces: world, local (a placement's own frame), view (the
  camera's), screen and the Grid's drawing units.
- Moving between spaces only through named functions.
- Units as types: metres and grid units, converted exactly and only by name.
- Checking that a mix does not compile.

## The method

A position's type is its space. Each space is a struct of the same scalars,
so a `WorldPoint` costs what three `f32`s cost, and the compiler keeps them
apart: a `GridPoint` handed to a function that wants a `WorldPoint` is a type
error, whatever the numbers are.

| Space | Type | Units and axes |
| --- | --- | --- |
| world | `WorldPoint` | metres, `+Y` up, yaw 0 faces `+Z` (`openagents.engine.v1`): where physics puts things |
| local | `LocalPoint` | metres in a `Placement`: an origin and a yaw (a room, a carried thing) |
| view | `ViewPoint` | metres from a `Lens`: `x` right, `y` up, `z` ahead (depth) |
| screen | `ScreenPoint` | CSS pixels, `+y` down |
| grid | `GridPoint` | the Grid's drawing units, two metres each, from the places' middle |

<!-- source: aethyr/space.coax types -->
```jai
WorldPoint :: struct { x: f32; y: f32; z: f32; }
LocalPoint :: struct { x: f32; y: f32; z: f32; }
ViewPoint :: struct { x: f32; y: f32; z: f32; }
ScreenPoint :: struct { x: f32; y: f32; }
GridPoint :: struct { x: f32; y: f32; z: f32; }

// Where a local space sits in the world: its origin and its yaw about +Y.
Placement :: struct { origin: WorldPoint; yaw: f32; }

// A camera's lens: the eye and its basis in the world, and how the view lands on screen.
Lens :: struct {
  eye: WorldPoint;
  right: WorldPoint; up: WorldPoint; ahead: WorldPoint;   // unit directions in the world
  cx: f32; cy: f32;                                        // the screen point straight ahead
  focal: f32;                                              // pixels a metre at one metre's depth
  near: f32;                                               // metres: nothing nearer is drawn
}

// A point on screen, with its depth, or none (ok false) when it is behind the near plane.
Seen :: struct { at: ScreenPoint; depth: f32; ok: bool; }
```

Distances carry units through Coax's `quantity` library (R12's first slice).
A grid unit is a unit of length worth two metres, a power of two, so
converting is exact; `grid_metres` is the only way across.

<!-- source: aethyr/space.coax units -->
```jai
// A grid unit is two metres (the Grid draws a metre as half a unit, newland-walkers.coax
// WALK_SCALE): a power of two, so converting is exact.
GridUnit :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=2,denominator=1) {}
GRID_PER_METRE :: 0.5;
aethyr_grid_per_metre :: () -> f32 #export_c { return GRID_PER_METRE; }   // for a C face
```

## The conversions

The Grid draws a metre as half a unit, and it must scale heights as it
scales the ground. When it did not, a jump's feet were drawn at floor height
while the camera, which read the physics position, rose: two sources of one
position. `world_to_grid` is now the one way a world point reaches the
Grid's drawing:

<!-- source: aethyr/space.coax grid -->
```jai
// The world as the Grid draws it: `mid` is the places' middle in grid units, and grid
// heights are metres times GRID_PER_METRE too, so a figure's feet are drawn at its physics height.
world_to_grid :: (mid: GridPoint, w: WorldPoint) -> GridPoint {
  return GridPoint.{x = mid.x + w.x * GRID_PER_METRE, y = mid.y + w.y * GRID_PER_METRE, z = mid.z + w.z * GRID_PER_METRE};
}
grid_to_world :: (mid: GridPoint, g: GridPoint) -> WorldPoint {
  return world((g.x - mid.x) / GRID_PER_METRE, (g.y - mid.y) / GRID_PER_METRE, (g.z - mid.z) / GRID_PER_METRE);
}
```

A lens turns a world point into the camera's view and the view onto the
screen. A point nearer than the near plane, or behind the eye, is not drawn:
`Seen.ok` is false rather than a point projected through the eye to the
wrong side of the screen.

<!-- source: aethyr/space.coax screen -->
```jai
world_to_view :: (l: Lens, w: WorldPoint) -> ViewPoint {
  v := world_sub(w, l.eye);
  return ViewPoint.{x = world_dot(v, l.right), y = world_dot(v, l.up), z = world_dot(v, l.ahead)};
}
view_to_world :: (l: Lens, v: ViewPoint) -> WorldPoint {
  return world_add(l.eye, world_add(world_scale(l.right, v.x), world_add(world_scale(l.up, v.y), world_scale(l.ahead, v.z))));
}
view_to_screen :: (l: Lens, v: ViewPoint) -> Seen {
  if v.z < l.near { return Seen.{at = ScreenPoint.{x = 0, y = 0}, depth = v.z, ok = false}; }
  return Seen.{at = ScreenPoint.{x = l.cx + v.x / v.z * l.focal, y = l.cy - v.y / v.z * l.focal}, depth = v.z, ok = true};
}
world_to_screen :: (l: Lens, w: WorldPoint) -> Seen { return view_to_screen(l, world_to_view(l, w)); }
```

[aethyr/examples/feet-in-every-space.coax](../examples/feet-in-every-space.coax)
follows a figure at the top of its jump through every space:

<!-- source: aethyr/examples/feet-in-every-space.coax example -->
```jai
#load "../space.coax";
#import "Basic";

// A figure at the top of its jump, 1.17 m up, followed through every space by name.
main :: () {
  feet := world(2, 1.17, -4);
  room := Placement.{origin = world(2, 0, -4), yaw = 0};
  local := world_to_local(room, feet);
  print("world  (%, %, %) m\n", feet.x, feet.y, feet.z);
  print("local  (%, %, %) m in a room at (2, 0, -4)\n", local.x, local.y, local.z);
  mid := GridPoint.{x = 4, y = 0, z = -6};
  g := world_to_grid(mid, feet);
  print("grid   (%, %, %) units: the height scales as the ground does\n", g.x, g.y, g.z);
  lens := lens_at(world(2, 3, -10), world(2, 1, -4), 400, 300, 500, 0.05);
  v := world_to_view(lens, feet);
  s := view_to_screen(lens, v);
  print("view   depth % m; screen (%, %) px, drawn %\n", v.z, s.at.x, s.at.y, s.ok);
  back := grid_to_world(mid, g);
  print("back from the grid (%, %, %) m\n", back.x, back.y, back.z);
  d := grid_distance(GridPoint.{x = 0, y = 0, z = 0}, GridPoint.{x = 3, y = 0, z = 4});
  print("5 grid units are % m\n", grid_metres(d).value);
  behind := world_to_screen(lens, world(2, 1, -12));
  print("a point behind the eye is drawn %\n", behind.ok);
}
```

```shell
coax/bin/coax run aethyr/examples/feet-in-every-space.coax
```

```text
world  (2.0, 1.17, -4.0) m
local  (0.0, 1.17, 0.0) m in a room at (2, 0, -4)
grid   (5.0, 0.585, -8.0) units: the height scales as the ground does
view   depth 6.2707963 m; screen (400.0, 287.1407) px, drawn true
back from the grid (2.0, 1.17, -4.0) m
5 grid units are 10.0 m
a point behind the eye is drawn false
```

## Mixing spaces does not compile

A type error that nobody checks can be lost to a cast someone adds later, so
the refusals are checks too. Each fixture in `aethyr/tests/spaces/` names on
its first line what the compiler must say about it, and
`aethyr/refusals.coax` (a Coax tool) compiles each and compares. The bug
above, a grid point drawn as a world one:

<!-- source: aethyr/tests/spaces/grid-as-world.coax fixture -->
```jai
#load "../../space.coax";
f :: (l: Lens, g: GridPoint) -> Seen { return world_to_screen(l, g); }
main :: () {}
```

```text
aethyr/tests/spaces/grid-as-world.coax:4:66: error[E0317]: argument 2 of 'world_to_screen' is WorldPoint, given GridPoint
```

Metres added to grid units:

<!-- source: aethyr/tests/spaces/metres-plus-grid.coax fixture -->
```jai
#load "../../space.coax";
f :: (a: WorldPoint, g: GridPoint) -> f32 { d := world_distance(a, a) + grid_distance(g, g); return d.value; }
main :: () {}
```

```text
aethyr/tests/spaces/metres-plus-grid.coax:4:71: error[E0822]: incompatible quantity operands: Quantity(Metre, f32) and Quantity(GridUnit, f32)
  fix: write convert_unit(Metre, value) explicitly
```

The six fixtures are those two, a world point given for a view point and a
screen point for a view point (E0317), a local point stored as a world one
(E0325), and a control that reaches each space through its named conversion
and must compile, so a fixture can't pass because nothing compiles at all.

```shell
coax/web/tool.sh aethyr/refusals.coax
```

```text
ok   aethyr/tests/spaces/grid-as-world.coax (E0317)
ok   aethyr/tests/spaces/local-as-world.coax (E0325)
ok   aethyr/tests/spaces/metres-plus-grid.coax (E0822)
ok   aethyr/tests/spaces/ok-every-space-by-name.coax (ok)
ok   aethyr/tests/spaces/screen-as-view.coax (E0317)
ok   aethyr/tests/spaces/world-as-view.coax (E0317)
6 fixtures, 6 held, 0 failed
```

## Checks

```shell
coax/bin/coax test aethyr/space.coax --library
coax/bin/coax mutate aethyr/space.coax
coax/bin/coax test aethyr/refusals.coax --library
coax/bin/coax mutate aethyr/refusals.coax
```

`space.coax`'s checks cover the world arithmetic, distances in their units
and the exact grid conversion, local round trips at sixteen yaws, a quarter
turn, the Grid's height scaling (feet 1.17 m up drawn 0.585 units up), and
the lens: ahead, right, up, the near plane exactly and nothing behind it.
Mutation kills 78 of 78 mutants of `space.coax` and 47 of 47 of
`refusals.coax`, whose own checks read a fixture's expectation and decide
whether the compiler met it.

## Still to do

The figure's feet drawn at their physics height in every realm is a
cross-realm check: it reads the drawn feet and the physics feet in the same
frame and requires equality, not closeness. It needs every realm's drawing to
go through `world_to_grid` or `world_to_view`, so it lands with the migration
([chapter 64](ch064-every-realm-runs-on-aethyr.md)), which also replaces the
older vector types: `Point` in [verse/space.coax](../../verse/space.coax),
`Vec3` in [verse/player.coax](../../verse/player.coax), and the f64 `D3` with
a floating origin in [verse/rigid.coax](../../verse/rigid.coax) and
[verse/spacecam.coax](../../verse/spacecam.coax), which gains f64 spaces here
when it moves.

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md) for the interpolated position
a figure is drawn at. From Coax: R12 units and coordinate frames
([Coax roadmap](../../coax/ROADMAP.md)), whose first slice is
[coax/lib/quantity.coax](../../coax/lib/quantity.coax), and generics
([Coax chapter 8](../../coax/book/ch08-generics.md)). Background:
[game-engine systems inventory, section 6](../../docs/research/game-engine-systems.md).
