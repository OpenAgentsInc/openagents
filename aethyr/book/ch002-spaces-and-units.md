# 2. Spaces and units

> Engine-core task: `ec-space` (#12609). The module
> [aethyr/space.coax](../space.coax) and its refusal check
> [aethyr/refusals.coax](../refusals.coax) landed in `a93d35c2dd`.
> This chapter's profile is native f32 spatial boundaries: typed conversions,
> compiler refusals, and the interpolated world anchor consumed by every
> current avatar renderer. It does not qualify browser pixels or consolidate
> every legacy vector type. The [verification record](../../docs/verification/aethyr-book-implementation-2026-11-01.json)
> identifies the compiler, source and checks.

A number such as `1.8` can be a height in metres, a distance on the Grid, a
pixel offset, or a depth in front of the camera. Many engine bugs come from
reading a value in the wrong one of these spaces. For example, the Grid drew
a figure's feet at floor height while the camera rose with the jump: the
drawing and the camera read the position in different spaces.

Aethyr gives each space its own type, so the compiler refuses code that mixes
them.

## What this chapter teaches

- The engine's spaces: world, local, view, screen, and the Grid's drawing
  units.
- How to move a point between spaces with named functions only.
- How to use units as types, so that metres and grid units don't mix.
- How to check that code that mixes spaces doesn't compile.

## The method

A point's type is its space. Each space is a struct of the same scalars, so a
`WorldPoint` costs the same as three `f32` values. The compiler keeps the
types apart: if you pass a `GridPoint` to a function that takes a
`WorldPoint`, the code doesn't compile.

| Space | Type | Units and axes |
| --- | --- | --- |
| world | `WorldPoint` | Metres. `+Y` is up, and yaw 0 faces `+Z` (`openagents.engine.v1`). Physics positions are in this space. |
| local | `LocalPoint` | Metres relative to a `Placement`, which is an origin and a yaw, such as a room or a carried object. |
| view | `ViewPoint` | Metres from a `Lens`. `x` is right, `y` is up, and `z` is depth ahead. |
| screen | `ScreenPoint` | CSS pixels. `+y` is down. |
| grid | `GridPoint` | The Grid's drawing units, two metres each, measured from the middle of the places. |

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

Distances use Coax's `quantity` library, the first part of roadmap item R12.
A grid unit is a unit of length equal to two metres. Two is a power of two, so
the conversion is exact. Use `grid_metres` to convert; there is no other way.

<!-- source: aethyr/space.coax units -->
```jai
// A grid unit is two metres (the Grid draws a metre as half a unit, newland-walkers.coax
// WALK_SCALE): a power of two, so converting is exact.
GridUnit :: struct @unit(mass=0,length=1,time=0,angle=0,numerator=2,denominator=1) {}
GRID_PER_METRE :: 0.5;
aethyr_grid_per_metre :: () -> f32 { return GRID_PER_METRE; }   // not #export_c: two libraries a page links both load this file
```

## The conversions

The Grid draws one metre as half a unit. It must scale heights by the same
factor as the ground. `world_to_grid` is the only function that converts a
world point for the Grid's drawing, and it scales all three axes:

<!-- source: aethyr/space.coax grid -->
```jai
// The world as the Grid draws it: `mid` is the places' middle in grid units, and grid
// heights are metres times GRID_PER_METRE too, so a figure's feet are drawn at its physics height.
world_to_grid :: (mid: GridPoint, w: WorldPoint) -> GridPoint {
  return GridPoint.{x = mid.x + w.x * GRID_PER_METRE, y = mid.y + w.y * GRID_PER_METRE, z = mid.z + w.z * GRID_PER_METRE};
}
grid_to_world :: (mid: GridPoint, g: GridPoint) -> WorldPoint {
  return world_point((g.x - mid.x) / GRID_PER_METRE, (g.y - mid.y) / GRID_PER_METRE, (g.z - mid.z) / GRID_PER_METRE);
}
```

A `Lens` converts a world point to the camera's view, and the view to the
screen. If a point is nearer than the near plane, or behind the camera,
`view_to_screen` returns `Seen.ok` as `false`. It doesn't project the point
to the wrong side of the screen.

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

The example
[aethyr/examples/feet-in-every-space.coax](../examples/feet-in-every-space.coax)
converts a figure's feet at the top of a jump into every space:

<!-- source: aethyr/examples/feet-in-every-space.coax example -->
```jai
#load "../space.coax";
#import "Basic";

// A figure at the top of its jump, 1.17 m up, followed through every space by name.
main :: () {
  feet := world_point(2, 1.17, -4);
  room := Placement.{origin = world_point(2, 0, -4), yaw = 0};
  local := world_to_local(room, feet);
  print("world  (%, %, %) m\n", feet.x, feet.y, feet.z);
  print("local  (%, %, %) m in a room at (2, 0, -4)\n", local.x, local.y, local.z);
  mid := GridPoint.{x = 4, y = 0, z = -6};
  g := world_to_grid(mid, feet);
  print("grid   (%, %, %) units: the height scales as the ground does\n", g.x, g.y, g.z);
  lens := lens_at(world_point(2, 3, -10), world_point(2, 1, -4), 400, 300, 500, 0.05);
  v := world_to_view(lens, feet);
  s := view_to_screen(lens, v);
  print("view   depth % m; screen (%, %) px, drawn %\n", v.z, s.at.x, s.at.y, s.ok);
  back := grid_to_world(mid, g);
  print("back from the grid (%, %, %) m\n", back.x, back.y, back.z);
  d := grid_distance(GridPoint.{x = 0, y = 0, z = 0}, GridPoint.{x = 3, y = 0, z = 4});
  print("5 grid units are % m\n", grid_metres(d).value);
  behind := world_to_screen(lens, world_point(2, 1, -12));
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

## At a realm's drawing boundary

The position drawn this frame is the interpolation of two fixed steps, not
the position from the last step alone (chapter 1). Some older realms also
step their controller on a flat floor. Its height is relative to that floor;
the realm adds the terrain or bridge height when it draws. This is a local
point crossing into the world, and it needs one named conversion.

Work the boundary by hand. Suppose the previous feet are `(1, 0.25, -6)` and
the next feet are `(5, 1.25, 2)`. A frame one quarter of the way between them
draws `(2, 0.5, -4)`. On terrain at 2.5 m, the world feet are `(2, 3, -4)`.
Dropping the controller's height puts the figure 0.5 m too low. Adding the
terrain twice puts it 2.5 m too high. Drawing the next step instead loses
the interpolation on all three axes.

The existing realm boundary keeps its `Point` layout for C callers, but
the conversion goes through Aethyr's distinct local and world types:

<!-- source: verse/space.coax feet -->
```jai
// A realm that steps its controller on a flat floor supplies that floor's world
// height from its terrain or bridge once. Figures and cameras share the resulting
// world point. Point is retained at the older realms' C boundary.
feet_in_world :: (flat_feet: Point, floor_y: f32) -> Point {
  placement := feet_space.Placement.{origin = feet_space.world_point(0, floor_y, 0), yaw = 0};
  local := feet_space.LocalPoint.{x = flat_feet.x, y = flat_feet.y, z = flat_feet.z};
  world := feet_space.local_to_world(placement, local);
  return v3(world.x, world.y, world.z);
}
```

The Garden, Fairy Forest, Everglade and the Cemetery use this conversion
before drawing their figure and following it with the camera. Everglade's
wading height is another floor origin. Swimming instead supplies an absolute
world height; it does not add the jump height a second time.

The Grid uses `world_to_grid` for the feet, camera eye and aim. Its remote
visitors keep their admitted height alongside their horizontal position,
ease all three coordinates, and draw through `figure_up`. Previously the
remote drawing kept only horizontal position and put every visitor on the
floor, even when the presence record said they were jumping.

The runnable example
[feet-at-a-realm-boundary.coax](../examples/feet-at-a-realm-boundary.coax)
performs that calculation, then converts to Grid units with a nonzero origin:

<!-- source: aethyr/examples/feet-at-a-realm-boundary.coax example -->
```jai
#load "../../verse/space.coax";
#load "../space.coax" as engine;
#import "Basic";

main :: () {
  feet := mix(v3(1, 0.25, -6), v3(5, 1.25, 2), 0.25);
  world := feet_in_world(feet, 2.5);
  mid := engine.GridPoint.{x = 4, y = 3, z = -6};
  grid := engine.world_to_grid(mid, engine.world_point(world.x, world.y, world.z));
  print("interpolated feet: (%, %, %) m above the flat floor\n", feet.x, feet.y, feet.z);
  print("world feet: (%, %, %) m\n", world.x, world.y, world.z);
  print("Grid feet: (%, %, %) units\n", grid.x, grid.y, grid.z);
}
```

```shell
coax/bin/coax run aethyr/examples/feet-at-a-realm-boundary.coax
```

```text
interpolated feet: (2.0, 0.5, -4.0) m above the flat floor
world feet: (2.0, 3.0, -4.0) m
Grid feet: (5.0, 4.5, -8.0) units
```

The production-boundary check is
[aethyr/tests/check_drawn_feet.py](../tests/check_drawn_feet.py). It compiles
separate Coax probes against the real realm code and checks their native
render records against independently calculated coordinates. Each probe
also receives deliberately wrong producer values: a dropped height, a
doubled height, the last step instead of interpolation, and an extra floor
offset. Those controls must disagree with the independently specified
world point. They test the boundary comparison; they do not establish that
every possible fault in the renderer is detected.

```shell
python3 aethyr/tests/check_drawn_feet.py --coax coax/bin/coax
```

## Check that mixed spaces don't compile

A type error is only useful if a check keeps it in place. Otherwise, a later
cast can remove it without anyone noticing. Each fixture in
`aethyr/tests/spaces/` states on its first line what the compiler must
report. The tool `aethyr/refusals.coax` compiles each fixture and compares
the result.

This fixture passes a grid point where a world point is required:

<!-- source: aethyr/tests/spaces/grid-as-world.coax fixture -->
```jai
#load "../../space.coax";
f :: (l: Lens, g: GridPoint) -> Seen { return world_to_screen(l, g); }
main :: () {}
```

```text
aethyr/tests/spaces/grid-as-world.coax:4:66: error[E0317]: argument 2 of 'world_to_screen' is WorldPoint, given GridPoint
```

This fixture adds metres to grid units:

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

There are six fixtures:

- a grid point passed as a world point (E0317);
- a world point passed as a view point (E0317);
- a screen point passed as a view point (E0317);
- a local point stored as a world point (E0325);
- metres added to grid units (E0822);
- a control that reaches each space through its named conversion and must
  compile. If nothing compiled, the refusals would pass for the wrong
  reason; the control catches that.

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

The checks in `aethyr/space.coax` cover:

- world arithmetic;
- distances in their units, and the exact grid conversion;
- local-to-world round trips at sixteen yaws, and a quarter turn;
- the Grid's height scaling: feet 1.17 m up are drawn 0.585 units up;
- the lens: ahead, right, up, a point exactly on the near plane, and points
  behind the camera.

`coax mutate` kills 78 of 78 mutants of `aethyr/space.coax`. It kills 47 of
47 mutants of `aethyr/refusals.coax`, whose checks read a fixture's
expectation and decide whether the compiler met it.

## The qualified boundary

The native check covers these production paths. A figure's anchor is the
interpolated controller position in world space, after its declared floor
origin is applied once. It is not the last fixed step or an animated ankle.

| Renderer | What the check observes |
| --- | --- |
| Grid | The walking frame's converted root and the figure's emitted stroke geometry; presence height has its own update and reordering check. |
| Garden | The emitted figure's head sphere, minus its authored offset. |
| Fairy Forest | The first emitted figure sphere compared with the same pose at world origin. |
| Cemetery and villa | The world root handed to the figure library and the resulting bone translation. |
| Everglade | The Ranger's object matrix; the authored checks also cover wading and swimming. |
| Dead Gears | The shared controller's handoff and the engineer's emitted bone translation. |
| Haunted House and Sandbox | The avatar instance matrices for both guest variants. |
| Made realms | The actor-to-pawn-to-person path's emitted pieces. |
| Ice Cream Man | The avatar's torso matrix, minus its authored model offset. |

The nine paths each have one independently specified valid input and four
invalid producer controls. Object and bone coordinates are compared within
0.00002 m; the Grid also checks its projected stroke translation within
0.001 pixels. No downloaded mesh geometry is needed to test the translation
of a figure's root.

Orbit League's cars, Belt's ships, Hold the Stair's tiles, and views without
a player figure have no walking-foot anchor. They are not additional avatar
cases. The figure editor's posed previews likewise have no controller state
to interpolate.

GPU renderers consume world-space object matrices, so their boundary is
checked before GPU submission. They do not need to project each vertex on
the CPU through `world_to_view`. Browser execution and final rasterized
pixels are separate qualifications in chapters 11 and 85.

## Later consolidation

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md))
replaces the older vector types behind the existing C interfaces:

- `Point` in [verse/space.coax](../../verse/space.coax);
- `Vec3` in [verse/player.coax](../../verse/player.coax);
- the f64 `D3` type with a floating origin in
  [verse/rigid.coax](../../verse/rigid.coax) and
  [verse/spacecam.coax](../../verse/spacecam.coax). When these move, this
  module gains f64 spaces. These types and floating-origin arithmetic are
  outside this chapter's native f32 profile.

## Dependencies

This chapter uses the interpolated position from
[chapter 1](ch001-a-frame-has-a-fixed-step.md). From Coax, you need R12 units
and coordinate frames ([Coax roadmap](../../coax/ROADMAP.md)), whose first
part is [coax/lib/quantity.coax](../../coax/lib/quantity.coax), and generics
([Coax chapter 8](../../coax/book/ch08-generics.md)). For background, see the
[game-engine systems inventory, section 6](../../docs/research/game-engine-systems.md).
