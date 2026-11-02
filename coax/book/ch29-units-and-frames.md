# 29. Units and coordinate frames

> **Status: partly implemented.** This is roadmap R12 (spec stage 10).
> These parts ship in `coax0` for native C output:
> [nominal scalar quantities](https://github.com/OpenAgentsInc/openagents/blob/main/coax/QUANTITIES.md),
> derived units (`UnitProduct`, `UnitQuotient`, `UnitOne`), binary-scale
> `convert_unit`, and
> [an initial affine profile](https://github.com/OpenAgentsInc/openagents/blob/main/coax/SPATIAL.md)
> of `Point3` and `Vector3` in declared frames with a minimum precision.
> Transforms, rotating and time-varying frames, rounded conversions and
> user-defined operators are planned. Sections that describe planned work
> say so.

A number in a physics program has a unit and, if it is a position, a frame.
Without types, those facts live in comments. This chapter shows how you put
them in the type, so the compiler refuses metres added to feet, a world point
used as a view point, or a position transform applied to a velocity. The first
section runs today. The rest of the chapter describes the planned design.

## Points and vectors in a frame (runs today)

In this section, you declare a frame, place two points in it, and compute the
point halfway between them. The program is
[`examples/ch29_frames.coax`](examples/ch29_frames.coax).

A frame is an empty struct with a `@frame` annotation. All four attributes
are required: handedness (`right` or `left`), convention (`cartesian`, `body`
or `optical`), continuity (`continuous` or `jumping`) and minimum precision
(`f32` or `f64`). `Point3(Frame, Unit, R)` and `Vector3(Frame, Unit, R)` take
a frame, a unit from `coax/lib/quantity.coax` and an `f32` or `f64` carrier.

<!-- checked-source: examples/ch29_frames.coax -->
```jai
// A frame is an empty tag. All four attributes are required.
World :: struct @frame(handedness=right, convention=cartesian,
    continuity=continuous, min_precision=f64) {}

// The point halfway between two points in the same frame.
midpoint :: (a: Point3(World, u.Metre, f64), b: Point3(World, u.Metre, f64)) -> Point3(World, u.Metre, f64) {
  offset := b - a;          // point - point is a vector
  half := offset / 2.0;     // vector / raw scalar is a vector
  return a + half;          // point + vector is a point
}
```

You build a point by declaring a zero value and assigning its `.x`, `.y` and
`.z` components. The compiler admits these operations when the frame, unit
and carrier match:

| Operation | Result |
| --- | --- |
| point − point | vector |
| point + vector, vector + point, point − vector | point |
| vector + vector, vector − vector, −vector | vector |
| vector × raw scalar, raw scalar × vector, vector / raw scalar | vector |

It refuses point + point, point scaling, dot and cross products, equality and
ordering on these types. It never converts a frame, unit or precision for you.

### Work the numbers by hand

`main` places Earth at x = 149,597,870,700 m (one astronomical unit) and the
ship at x = 149,597,870,760 m, y = 8 m. Compute the result before you run it:

1. `ship - earth` is a vector: (149,597,870,760 − 149,597,870,700, 8 − 0,
   0 − 0) = (60, 8, 0) m.
2. Half of that vector is (30, 4, 0) m.
3. `earth + half` is a point. Its offset from Earth is (30, 4, 0) m.

The frame's minimum precision is `f64` because of these magnitudes. At
1 AU, adjacent `f32` values are 16,384 m apart, so the 60 m offset would round
to zero. Adjacent `f64` values there are about 30.5 µm apart.

Run the program:

```shell
coax/bin/coax run coax/book/examples/ch29_frames.coax
```

The output matches the hand calculation:

```text
offset 60.0 8.0 0.0
middle minus earth: 30.0 4.0 0.0
```

### Check it independently

The example's `#test` computes the same midpoint from plain `f64`
arithmetic, `a.x + (b.x - a.x) / 2.0`, and compares each coordinate with the
typed result. Run it:

```shell
coax/bin/coax test coax/book/examples/ch29_frames.coax
```

The check passes in the book's debug and release runs.

### Break it on purpose

[`examples/ch29_frames_refused.coax`](examples/ch29_frames_refused.coax)
subtracts a point in a `World` frame from a point in a `Ship` frame:

<!-- checked-source: examples/ch29_frames_refused.coax -->
```jai
World :: struct @frame(handedness=right, convention=cartesian,
    continuity=continuous, min_precision=f64) {}
Ship :: struct @frame(handedness=right, convention=body,
    continuity=continuous, min_precision=f64) {}

main :: () {
  a: Point3(World, u.Metre, f64);
  b: Point3(Ship, u.Metre, f64);
  wrong := b - a;   // refused: the points are in different frames
  print("%\n", wrong.x);
}
```

The compiler refuses it with `E0834` and exits with status 1:

```text
examples/ch29_frames_refused.coax:12:14: error[E0834]: nominal frames differ: Point3(Ship,Metre,f64) and Point3(World,Metre,f64)
    wrong := b - a;   // refused: the points are in different frames
               ^
1 error
```

Two frames with identical attributes are still different frames. To mix
them, you need a transform from one frame to the other, which is planned
(outline item 5). Adding two points in the same frame gives the same code
with the message `affine operation is not admitted`.

### What this profile doesn't do

- It has no transforms, rotating frames or state transforms.
- `.x`, `.y` and `.z` are raw scalars. Assigning them asserts the unit and
  frame. The compiler doesn't check where the number came from.
- Metal, CUDA, SPIR-V and WGSL output refuse spatial values (`E0836`).
- The first consumer is the Verse space camera, `aethyr/spacecam.coax`. Its
  world point and floating origin are `f64` points, and the published camera
  signatures keep their raw layouts.

[coax/SPATIAL.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/SPATIAL.md)
lists the full profile, its mutation records and its native qualification.

## What the full design covers

- Dimensioned quantities, and how a conversion between units is written.
- Points, vectors and rigid transforms tagged with a frame and a unit.
- Transforms that carry their source and target frames.
- Moving and rotating frames, and full state transforms.
- Precision as part of a frame's type.

## Outline

1. **Quantities (ships, except rounded conversions).** `Quantity(Unit, R)` over `f32` or `f64`, with `@unit`
   tags for mass, length, time and angle. Adding two quantities needs the
   same nominal unit. Conversions are explicit (`convert_unit`), exact where
   the scale is exact, and rounded under a stated rule where it is not.
2. **Derived dimensions (ships for declared unit tags).** Products and quotients check their dimensions, so
   W, J/K, W/(m²·K⁴) and m³/s² are checked types. A radiator law or a
   gravitational parameter is dimension-checked, not only labelled. Kelvin
   is not yet in the unit metadata, so temperature dimensions are planned.
3. **Points and vectors (ships).** A point minus a point is a vector. A point plus a
   vector is a point. Adding two points is refused. Mixing frames or units is
   refused with a diagnostic that names the expected type, the received type
   and the conversion or transform you need.
4. **Frames (ships as declarations).** A frame declares handedness, a convention (`cartesian`,
   `body` or `optical`), continuity (`continuous` or `jumping`, for globally
   corrected frames) and a minimum precision. These facts survive import and
   arithmetic.
5. **Transforms (planned).** A transform carries its source and target frames, and
   composing two transforms checks that the frames meet. Quaternions,
   matrices and axis-angle state their handedness, normalization and
   tolerance rules.
6. **Rotating and moving frames (planned).** A full state transform maps position and
   velocity together, including the angular-velocity term
   `v_world = v_frame + R (v_local + ω × r_local)`. A position-only transform
   applied to a velocity is refused. This follows the distinction that JPL's
   [SPICE frames](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html)
   draw between position and state transforms.
7. **Time-varying frames (planned).** A frame's transform comes from a named source,
   such as an ephemeris or a rotation model with a version and coverage,
   evaluated at a typed instant from [chapter 30](ch30-clocks-and-ticks.md).
8. **Precision tiers (minimum precision ships; recorded origin changes are
   planned).** At 1 AU, `f32` spacing is 16,384 m and `f64` spacing
   is about 30.5 µm. An astronomical frame holds `f64` metres. A local frame
   for contacts or drawing is reached by an explicit, recorded origin change.
9. **Operator overloads (planned).** User-defined operators wait until roadmap R11
   effect checks can prove them pure, nonallocating and nonfallible. Until
   then, the operators are built in.

An uncompiled sketch of the planned transform surface:

```jai
// Sketch only: not accepted by coax0.
Ship :: struct @frame(handedness=right, convention=body,
    continuity=continuous, min_precision=f64) {}
drum_to_ship: StateTransform(DrumInterior, Ship, Metre, f64);
v_ship := apply(drum_to_ship, state_local);   // position and velocity
bad := apply_position(drum_to_ship, state_local.velocity); // refused
```

## The consumer and the gate

The roadmap names three consumers. Their adoption today:

- One Verse camera and world transform path. The space camera,
  `aethyr/spacecam.coax`, uses `f64` points for its world point and floating
  origin. The rest of the camera transform is untyped.
- The joint and pose values of the simulated robot in
  [Psionic chapters 168–170](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch168-imitation-before-robotics-scale.md).
  The Panda forward-kinematics queries (`psionic/lib/robotics/panda-fk-types.coax`
  and `panda-fk-math.coax`) return typed world positions. Joints and poses
  are untyped.
- The spacecraft scenario's heliocentric, body-fixed, ship and drum-interior
  states (`aethyr/orbit.coax`, `aethyr/gravity.coax`). Not adopted; these need
  the planned state transforms.

Fixtures cover point and vector algebra, refused frame and unit mixing,
composition through a frame chain, exact and rounded conversions, a jumping
frame, a refused position-only transform of a velocity, a rotating-frame state
round trip against the inertial result, an `f32` astronomical position
refused where `f64` is required, and diagnostics that name the missing
conversion. Generated code and layouts must match the untagged version byte
for byte where no conversion is written.

## Done when

- R12 ships with the fixtures above in the compiler's conformance tests.
- The three consumers use the types, and their layouts match the untagged
  code byte for byte.
- Every runnable section has an example in `examples/` with a `.out` file
  that `check-examples.sh` runs in debug and release. The affine section
  meets this today.
- The status marker is removed when every section runs, and the chapter
  registry (`chapters.json`) records the chapter as implemented.

## Dependencies

- Roadmap R4 (type parameters, [chapter 8](ch08-generics.md)) and R5
  (annotations and layout reflection, [chapter 13](ch13-data-layout.md)).
- R11 effect summaries, for user operator overloads.
- Used by Aethyr chapters
  [2](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch002-spaces-and-units.md),
  [39](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch039-ephemerides-time-scales-and-frames.md)
  and [44](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch044-inside-a-turning-accelerating-ship.md).
