# Space primitives

The engine pieces every space game in the Verse shares. They are Coax
modules under `verse/`, in f64, each with its own checks
(`coax/bin/coax test --library verse/<module>.coax --steps 2000000000`).
A game's own module keeps only its rules and its drawing and loads these.

## Bodies, contacts, gravity, thrust and wheels

Orbit League's first playable built these (October 30).

| Module | What it gives | Loads |
| --- | --- | --- |
| `verse/rigid.coax` | Rigid bodies: Newton for the centre of mass, Euler's equations, RK4 attitude, sleep, a momentum ledger | |
| `verse/contact.coax` | Contacts: sphere-sphere, sphere-oriented box, box-plane (its corners), sphere-triangle and sphere-mesh; rays (plane, sphere, triangle); swept sphere tests against planes and spheres; the impulse answer (restitution, Coulomb friction, positional push); forces and impulses at a point | rigid |
| `verse/gravity.coax` | Gravity as a field: none, constant, point mass (inverse-square, `mu = G M`) or a spinning drum (centrifugal down, Coriolis on what moves), each scaled (a station spinning down). `gravity_at(field, pos)`, `gravity_on(field, pos, vel)`, `gravity_up`, `gravity_step`; `drum_spin_for(radius, g)`, circular orbit speed and period | rigid |
| `verse/thrust.coax` | Thrusters (force at a body point along a body direction) burning a tank: real propellant (mass falls, Tsiolkovsky delta-v) or a game budget (a boost meter); RCS torque clamped to its jets, and a rate hold | contact |
| `verse/vehicle.coax` | A wheeled controller: four suspension rays along the chassis's own down, spring and damper, tyres inside the friction circle, steering that fades with speed, handbrake drift, sticky wheels on walls and ceilings, air control, jump and double jump; the chassis box against the arena's planes | contact |

What the checks hold them to: a dropped ball comes back to e² of its
height; equal spheres swap velocities at e = 1 and every collision keeps
momentum; a sliding ball rolls at 5/7 of its speed; a ball at 200 m/s is
caught by the swept test before it passes a wall; a point mass gives 9.82
at the Earth's surface and a circular orbit stays circular; a 100 m drum
spun for 1 g gives 1 g at its rim and none on its axis; a burn's delta-v
is Tsiolkovsky's to 0.1%; RCS holds a spin rate; a roadster settles on its
springs at m g / 4 a wheel, reaches its top speed, turns, brakes, jumps,
and drives along a wall.

## How a space game uses them

1. Keep the bodies in an array of `RigidBody` and a `RigidWorld` over it.
   Step at a fixed rate (Orbit League: 120 Hz substeps of each frame).
2. Each step: ask the field (`gravity_on`) for each body's pull, or pass it
   to `vehicle_step` for a car; add thrust (`thrust_apply`, `rcs_apply`);
   `rigid_step(w, D3_ZERO)`.
3. Then contacts: for each pair or body and wall, find (`contact_*`), then
   answer (`contact_resolve`, `contact_resolve_static`). For a fast small
   body, sweep first (`sweep_sphere_plane`) and split the step at the hit.
4. The game draws from the bodies' positions and attitudes (f64 to f32 at
   the edge, after any floating origin).

## Orbits, large scales and the sky

Belt 2200's section: `verse/orbit.coax` (Kepler, patched conics,
transfers), the floating origin and large-scale camera, and the starfield.
