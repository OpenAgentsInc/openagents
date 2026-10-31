# 44. Inside a turning, accelerating ship

> **Status: unimplemented — design draft.** Aethyr already makes spin gravity:
> a gravity field for a spinning drum with centrifugal down and Coriolis on
> moving things (`gravity_drum`, `gravity_on`, `drum_spin_for` in
> [`verse/gravity.coax`](../../verse/gravity.coax);
> [`verse/drum.coax`](../../verse/drum.coax), used by Orbit League). This
> chapter plans interior frames, compartments and a crew-load model. Nothing
> here runs yet.

Inside a ship, "down" is whatever the ship's proper acceleration and
rotation make it. Under thrust, the floor is toward the engine; braking
after a flip keeps the same floor; in a coast, loose things float; in a
spinning drum, a dropped tool curves. A simulation can integrate interiors
in the rotating frame with fictitious forces or inertially with derived
observations. It must pick one, because doing both counts the same effect
twice.

## What this chapter teaches

- Proper acceleration as the floor, and why planetary gravity is not added
  to an interior in free fall.
- Rotating-frame dynamics: Coriolis, centrifugal and Euler terms.
- Spin habitats, foot attachment, and loose objects through a turn.
- A compartment model of pressure, volume and leaks.
- A bounded crew-tolerance model with provenance on its parameters.

## The method

In a ship frame with origin acceleration `a_origin`, rotation `R` and angular
velocity `ω`:

```jai
a_local = R⁻¹ (a_inertial − a_origin) − 2 ω × v_local
          − ω × (ω × r_local) − ω̇ × r_local
```

Each interior declares `integration: rotating | inertial`; the other is
derived for display. Spin gravity is `ω² r`: 0.3 g at 500 m needs
`ω = √(0.3 × 9.80665 / 500)` ≈ 0.0767 rad/s ≈ 0.7325 rpm. Feet attach
through a constraint with an adhesion limit and release; magnetic boots do
not make liquids settle.

Compartments are gas volumes joined by openings. Flow through an opening of
area `A` follows the orifice relation, choked when the pressure ratio
passes its critical value; a sealed door is an edge of zero area, so a leak
empties only connected volumes, on the time scale `V / (A c)`.

Crew tolerance is a simulation model, not medical guidance: magnitude, body
axis, duration, posture, restraint and accumulated exposure map to
impairment states (fine movement, standing, consciousness). Every parameter
carries provenance (explicit, inferred or invented, with unit and source),
following the factors in [NASA's acceleration brief](https://www.nasa.gov/wp-content/uploads/2023/12/ochmo-tb-024-acceleration.pdf).

## What exists today

[`verse/gravity.coax`](../../verse/gravity.coax) has `GravityField` with
none, constant, point-mass and drum kinds; `gravity_at`, `gravity_coriolis`,
`gravity_on`, `gravity_up` and `gravity_step`; `drum_spin_for` and
`drum_rim_speed`. Its checks give a 100 m drum spun for 1 g 1 g at the rim
and none on the axis. [`verse/drum.coax`](../../verse/drum.coax) has
`Drum`, `drum_up`, `drum_planes` and `drum_drive_step` for cars on the
inside of a drum. The drum is a field applied in the station frame, with no
Euler term, no frame choice and no thrust-driven floor. There are no
compartments, pressures or crew loads.

## What it builds

- `aethyr/space/interior.coax`: ship-local frames, the rotating/inertial
  choice with double-counting detection, foot constraints.
- `aethyr/space/compartments.coax`: volumes, openings, pressure and leaks.
- `aethyr/space/crew-load.coax`: the parameterized tolerance model.

## Acceptance checks and oracles

- A dropped object's curved path in a drum matches the inertial solution.
- 0.3 g at 500 m needs about 0.7325 rpm.
- A braking ship keeps its floor after the flip.
- Rotating-frame and inertial integration agree, and applying both is
  detected.
- A leak empties connected volumes on the expected time scale and leaves
  sealed ones alone.
- Reference: [NASA artificial gravity review](https://ntrs.nasa.gov/citations/19740010644).

## Dependencies

[Chapter 41](ch041-burns-consume-mass.md)'s ship mass and attitude;
[chapter 18](ch018-rigid-bodies-and-time-steps.md) and
[chapter 19](ch019-joints-in-generalized-coordinates.md) for bodies and
attachments; [chapter 2](ch002-spaces-and-units.md)'s frames. From Coax:
R12 frames, R20 provenance.
