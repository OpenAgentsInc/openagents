# 2. Spaces and units

> **Status: unimplemented — design draft.** Today positions are plain `f32`
> triples (`Point` in `verse/space.coax`, `Vec3` in `verse/player.coax`, `D3`
> in f64 in `verse/rigid.coax`) whose space and unit live in comments; only
> the Grid uses Coax's `quantity` library, for its fog distances. This chapter
> plans typed spaces with conversions only through named functions.
> Engine-core task: `ec-space` (#12609); the engine-core agent fills this
> chapter in as the module lands. The module,
> [aethyr/space.coax](../space.coax), landed on 2026-10-31 with refusal
> fixtures that keep mixed spaces from compiling; its listings and checks
> have not joined the chapter yet.

A number such as `1.8` is a character's height in metres, a cell count on the
Grid, a pixel offset on a phone or a distance in front of the camera. Engine
bugs are often a value read in the wrong one of these. A figure's feet drawn
a few centimetres off its physics height, a shadow placed from a stale
position, a label projected with the previous frame's camera: each is a
position used in a space it was not computed in. Aethyr gives each space its
own type, so the compiler refuses the mix.

## What this chapter teaches

- The engine's spaces: world, local (an entity's own frame), camera (view),
  clip and screen, and the Grid's integer cells.
- Points versus vectors, and which arithmetic each admits.
- Units: metres, seconds, radians, pixels and grid units, and conversions
  that are named and exact where they can be.
- One authoritative position per drawn thing.

## The method

A typed position is a value tagged with its space and unit, laid out exactly
like its three scalars. The algebra is affine:

```jai
Point(S) − Point(S)  -> Vec(S)          // a displacement
Point(S) + Vec(S)    -> Point(S)
Point(S) + Point(S)  -> refused
Vec(S) * scalar      -> Vec(S)
to_camera(view, p: Point(World)) -> Point(Camera)    // the only way across
to_screen(proj, p: Point(Camera)) -> Point(Screen)
```

Conversions between spaces are functions of a transform that carries its
source and target space: composing `World→Camera` with `Camera→Clip` checks
that the spaces meet. The world convention is fixed once: metres, seconds,
radians; `+Y` up; yaw 0 facing `+Z`, as `openagents.engine.v1` states.

Grid units are integers. The Grid's lines run every 4 m and its streets every
24 m, so `cell = ⌊x / 4 m⌋` and back is `x = 4 m · cell`; both are named
functions, and a cell is never added to a metre.

The authoritative position of a figure is its interpolated physics feet
(chapter 1's `lerp(prev, now, α)`) in world space. The figure, its shadow,
its name label and the camera's target all derive from that one value in the
same frame; none keeps a copy.

## What exists today

- [verse/space.coax](../../verse/space.coax): `Point`, `Shot`, `add`, `sub`,
  `times`, `dot`, `cross`, `mix`, `wrap_angle`, `yaw_of`; its header states
  "Units are metres, seconds and radians; +Y is up, and yaw 0 faces +Z".
- [verse/player.coax](../../verse/player.coax) declares its own `Vec3`
  arithmetic and constants (`WALK :: 3.5; // m/s`), units in comments.
- [verse/rigid.coax](../../verse/rigid.coax) and
  [verse/spacecam.coax](../../verse/spacecam.coax) use f64 `D3` and a floating
  origin, subtracting the camera's centre in f64 before anything reaches f32.
- [verse/grid.coax](../../verse/grid.coax) loads
  [coax/lib/quantity.coax](../../coax/lib/quantity.coax) (`Metre`, `Second`,
  `Radian`, `Quantity`) for `smooth` and `fog`, and projects to `Screen`
  in `verse_grid_view`.

Three vector types, units in comments, and screen positions computed in
several places: nothing stops a world point being handed to a screen function.

## What it builds

- `aethyr/space.coax`: `WorldPoint`, `LocalPoint`, `CameraPoint`,
  `ScreenPoint` and their vector counterparts over `coax.quantity` units;
  `GridCell` as integers; transforms tagged with source and target space;
  `world_to_local`, `world_to_camera`, `camera_to_screen`, `cell_of`,
  `cell_origin`; f32 and f64 variants, the f64 ones carrying a floating origin
  as `verse/spacecam.coax` does.
- `aethyr_feet(entity)`: the one authoritative drawn position, read by the
  figure, shadow, labels and camera.

## Acceptance checks and oracles

- Mixing spaces fails to compile: negative compile checks for adding two
  points, passing a world point to a screen function and adding a grid cell
  to metres, each with a diagnostic naming the expected and received types.
- The figure's feet are drawn at its physics height in every realm: a golden
  check reads the drawn feet and the physics feet in the same frame and
  requires equality, not closeness.
- Round trips: `camera_to_world(world_to_camera(p))` within one ulp per
  component for points across the realm's extent; `cell_origin(cell_of(x))`
  exact.
- Layout: a typed point has the size and alignment of its scalars (R5
  layout reflection).
- Mutation testing of `aethyr/space.coax`: 0 survivors.

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md) for the interpolated position.
From Coax: R12 units and coordinate frames and R4 type parameters
([Coax roadmap](../../coax/ROADMAP.md)), generics
([Coax chapter 8](../../coax/book/ch08-generics.md)). Background:
[game-engine systems inventory, section 6](../../docs/research/game-engine-systems.md).
