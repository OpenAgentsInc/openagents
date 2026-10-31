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
| `verse/vehicle.coax` | A wheeled controller: four suspension rays along the chassis's own down, spring and damper, tyres inside the friction circle, steering that fades with speed, handbrake drift, sticky wheels on walls and ceilings, air control, jump, double jump and the dodge (a second jump with the stick pushed: a punch along it and a flip toward it); the chassis box against the arena's planes | contact |

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

Belt 2200's first playable built these (October 30).

| Module | What it gives | Loads |
| --- | --- | --- |
| `verse/orbit.coax` | Two-body Kepler motion in f64: `Orbit` (a, e, i, node, periapsis, mean anomaly at epoch, mu) and `State` (r, v); `orbit_state` and `orbit_from_state` both ways; Kepler's equation for ellipses (`kepler_elliptic`) and hyperbolas (`kepler_hyperbolic`); `orbit_propagate`. The solar system from J2000 mean elements (`orbit_body`, `orbit_body_state`: the Sun, Mercury to Saturn, the Moon about the Earth, Ceres and Vesta; DE440 GM and radii; `ORBIT_2200` is 1 January 2200). Spheres of influence (`orbit_soi_radius`, `orbit_body_soi`) and the patched-conic handoff (`orbit_relative`, `orbit_absolute`, `orbit_dominant`); the hyperbolas that leave and reach a world (`orbit_departure`, `orbit_arrival`, `orbit_arrival_state`: periapsis at a given distance, the asymptote a transfer's v-infinity) and `orbit_soi_blend`, which hands a drawn ship from its hyperbola to its heliocentric arc across the sphere without a jump. Transfers: Lambert's problem by universal variables (`orbit_lambert`), Hohmann (`orbit_hohmann`), one transfer to a target orbit (`orbit_transfer`: both burns, their delta-v, the coast) and the planner (`orbit_plan`: the cheapest over a range of flight times, or the quickest under a delta-v budget). The rocket: `orbit_delta_v`, `orbit_propellant`, `orbit_propellant_from`, `orbit_burn_time`, and a finite burn under gravity with the mass flowing (`orbit_burn_step`, RK4) | rigid |
| `verse/spacecam.coax` | The floating origin: every position stays f64 metres and the camera subtracts its own centre before scaling, so only small numbers reach f32. `spacecam_screen` and `spacecam_world` (pixel to the ecliptic), `spacecam_zoom_at` (the point under the cursor stays put), `spacecam_pan`, `spacecam_look`, `spacecam_follow` (the origin rides a ship), `spacecam_ease` (log-space zoom); twelve decades of zoom, from the outer system on a phone to 20 pixels a metre; a tilt for height | rigid |
| `verse/stars.coax` | The sky, scalar f32 for a background pass (`coax/web/tool.sh coax/web/wgsl-block.coax verse/stars.coax verse_starfield verse_sun_disc verse_sun_corona --into APP.wgsl`): three layers of hashed stars that drift with the camera, the Sun's disc with a soft limb, and its corona and halo, which keep a far Sun a glowing point | |

What the checks hold them to: Kepler's equation to 1e-12 over eccentricities
0 to 0.9 and four hyperbolas; elements to a state and back to 1e-9 (an
ellipse and a hyperbola); an orbit keeps its energy and angular momentum to
1e-9 and closes after its period (Mars's is 687 days); the Earth is at 1 AU
with a 365.25-day year in 2200, the Moon 384,400 km from it, the Earth's
sphere of influence 925,000 km and Jupiter's 48 million; a ship 100,000 km
from the Earth is in its sphere and 50,000 km from the Moon in the Moon's;
a departure hyperbola starts at its periapsis and leaves along
v-infinity at its speed, and arriving is its mirror; Lambert's arc cut from a known orbit gives back its velocities to a
millimetre a second; Earth to Mars by Hohmann is 2.94 + 2.65 km/s in 259
days, and the planner finds it within 60 m/s, and a quicker, dearer
transfer under a bigger budget; a finite burn gives the rocket equation's
delta-v to 1e-7 (Lagrange 2's tug: 120 kN at Isp 330 s); a hull 5 AU out
draws to a millionth of a pixel through the floating origin, where f32
positions would lose tens of kilometres; zooming keeps the point under the
cursor; stars are sparse and the Sun's light falls away.

### How a space game uses them

1. Keep every position in f64 metres about the Sun (`D3`): the bodies from
   `orbit_body_state(k, t)`, your own things as `Orbit`s (coasting) or
   `State`s (under thrust, `orbit_burn_step`).
2. To send a ship: `orbit_plan(from, target_orbit, now, GM_SUN, tof_min,
   tof_max, samples, fast, dv_budget)` (a scale for the times is
   `orbit_hohmann_time`); its `dv1`, `dv2` and `tof` are the burns and the
   time, `orbit_propellant_from` the propellant. Coast on
   `orbit_from_state(tr.depart, GM_SUN, now)`.
3. Near a world, hand the state over with `orbit_dominant` and
   `orbit_relative`, and propagate about its GM. To draw a transfer that
   leaves or reaches a world, build `orbit_departure` / `orbit_arrival`
   from the arc's velocity relative to the world at the burns and mix the
   two states by `orbit_soi_blend` (Belt 2200's `flight_state`).
4. Draw through a `SpaceCam`: `spacecam_screen(cam, p, w, h)` gives pixels
   in f64; convert to f32 only then. Follow a ship by making it the origin.
5. The background pass calls `verse_starfield` and the Sun's two functions
   with the Sun's pixel and radius from the camera (bring a far-off Sun
   within a million pixels along its direction, its radius scaled alike).

Belt 2200 (`apps/openagents-web/belt.coax`) is the worked example.
