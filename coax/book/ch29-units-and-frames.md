# 29. Units and coordinate frames

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R12 (spec stage 10).
> Nominal scalar quantities already ship for native C; see
> [QUANTITIES.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/QUANTITIES.md).
> An initial affine profile of points and vectors is described in
> [SPATIAL.md](https://github.com/OpenAgentsInc/openagents/blob/main/coax/SPATIAL.md).
> Everything else in this chapter is planned.

A number in a physics program has a unit and, if it is a position, a frame.
Today most Coax code keeps those facts in comments. This chapter shows how
you will put them in the type, so the compiler refuses metres added to feet,
a world point used as a view point, or a position transform applied to a
velocity. The tags cost nothing at runtime: a frame-checked value has the
same layout as its plain scalar fields.

## What this chapter will cover

- Dimensioned quantities, and how a conversion between units is written.
- Points, vectors and rigid transforms tagged with a frame and a unit.
- Transforms that carry their source and target frames.
- Moving and rotating frames, and full state transforms.
- Precision as part of a frame's type.

## Outline

1. **Quantities.** `Quantity(Unit, R)` over `f32` or `f64`, with `@unit`
   tags for mass, length, time and angle. Adding two quantities needs the
   same nominal unit. Conversions are explicit (`convert_unit`), exact where
   the scale is exact, and rounded under a stated rule where it is not.
2. **Derived dimensions.** Products and quotients check their dimensions, so
   W, J/K, W/(m²·K⁴) and m³/s² are checked types. A radiator law or a
   gravitational parameter is dimension-checked, not only labelled.
3. **Points and vectors.** A point minus a point is a vector. A point plus a
   vector is a point. Adding two points is refused. Mixing frames or units is
   refused with a diagnostic that names the expected type, the received type
   and the conversion or transform you need.
4. **Frames.** A frame declares handedness, a convention (`cartesian`,
   `body` or `optical`), continuity (`continuous` or `jumping`, for globally
   corrected frames) and a minimum precision. These facts survive import and
   arithmetic.
5. **Transforms.** A transform carries its source and target frames, and
   composing two transforms checks that the frames meet. Quaternions,
   matrices and axis-angle state their handedness, normalization and
   tolerance rules.
6. **Rotating and moving frames.** A full state transform maps position and
   velocity together, including the angular-velocity term
   `v_world = v_frame + R (v_local + ω × r_local)`. A position-only transform
   applied to a velocity is refused. This follows the distinction that JPL's
   [SPICE frames](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/frames.html)
   draw between position and state transforms.
7. **Time-varying frames.** A frame's transform comes from a named source,
   such as an ephemeris or a rotation model with a version and coverage,
   evaluated at a typed instant from [chapter 30](ch30-clocks-and-ticks.md).
8. **Precision tiers.** At 1 AU, `f32` spacing is 16,384 m and `f64` spacing
   is about 30.5 µm. An astronomical frame holds `f64` metres. A local frame
   for contacts or drawing is reached by an explicit, recorded origin change.
9. **Operator overloads.** User-defined operators wait until roadmap R11
   effect checks can prove them pure, nonallocating and nonfallible. Until
   then, the operators are built in.

An uncompiled sketch of the planned surface:

```jai
// Sketch only: not accepted by coax0.
Ship :: struct @frame(handedness=right, convention=body,
    continuity=continuous, min_precision=f64) {}
drum_to_ship: StateTransform(DrumInterior, Ship, Metre, f64);
v_ship := apply(drum_to_ship, state_local);   // position and velocity
bad := apply_position(drum_to_ship, state_local.velocity); // refused
```

## The consumer and the gate

The roadmap names three consumers:

- one Verse camera and world transform path;
- the joint and pose values of the simulated robot in
  [Psionic chapters 168–170](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch168-imitation-before-robotics-scale.md);
- the spacecraft scenario's heliocentric, body-fixed, ship and drum-interior
  states (`verse/orbit.coax`, `verse/gravity.coax`).

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
- This chapter's examples are files in `examples/` with `.out` files, and
  `check-examples.sh` runs them in debug and release.
- The planned note is removed from every section that runs.

## Dependencies

- Roadmap R4 (type parameters, [chapter 8](ch08-generics.md)) and R5
  (annotations and layout reflection, [chapter 13](ch13-data-layout.md)).
- R11 effect summaries, for user operator overloads.
- Used by Aethyr chapters
  [2](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch002-spaces-and-units.md),
  [39](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch039-ephemerides-time-scales-and-frames.md)
  and [44](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch044-inside-a-turning-accelerating-ship.md).
