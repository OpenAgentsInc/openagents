# Lagrange 1: the L1 construction station

**Lagrange 1** is a Verse zone set on a small crewed station orbiting the
Sun–Earth L1 point, about 1.5 million km sunward of Earth. It is where a ship
frame begins: an astronaut in a maneuvering pack carries parts from a depot to
a keel jig. The physics is real where it matters at each scale, and each
approximation is named below.

Enter from the plaza's east arch (**L1 portal** on the map). Controls are in
[the zone guide](zones.md#lagrange-1-controls). The simulation lives in
[`verse-lagrange`](../../crates/verse-lagrange/), which uses the shared
zone-agnostic [`physics`](../../crates/physics/) crate for rigid bodies, fixed
stepping, and restorable world state; the scene, input mapping, and rendering
live in [`zones/lagrange.rs`](../../crates/verse/src/zones/lagrange.rs).

## Two clocks

| Clock | Rate | What it drives |
| --- | --- | --- |
| Local | Real time | The astronaut, pack thrust and propellant, parts, collisions |
| Orbital | 3,600 × real time (one hour per second) | The station's orbit about L1, station-keeping burns, Earth and Moon positions, mission day |

At one hour per second, one pass around the ~178-day Lissajous orbit takes
about 71 minutes of play. The two clocks are displayed separately; speeding up
the orbit never changes the local rigid-body step.

Local physics runs in fixed steps of 1/120 s (`PHYSICS_DT`). Frame time
accumulates and whole steps run, so the result does not depend on the frame
rate; the renderer interpolates between the last two poses. A frame runs at
most 12 steps (0.1 s); time beyond that is dropped and counted in
`Station::clock.dropped`. The orbit advances by each step's 1/120 s × 3,600.

`Station::save` returns a versioned `StationState` that restores and continues
bit for bit (with `serde_json`, enable `float_roundtrip`). `Station::record`
journals the pilot command and every action (`Input`: grab, release, fly-to,
stop), including NIP-MV operator commands, stamped with the physics tick;
`Station::replay` runs a journal from its start state.

## Orbit: the circular restricted three-body problem

[`orbit.rs`](../../crates/verse-lagrange/src/orbit.rs) integrates the full
nonlinear CR3BP in the rotating frame, in normalized units (1 AU, 1/n ≈ 58.1
days). The Sun and the Earth–Moon barycenter (EMB) move on circular orbits.

| Constant | Value | Source |
| --- | --- | --- |
| GM Sun | 1.32712440018 × 10²⁰ m³/s² | IAU 2015 nominal |
| GM Earth | 3.986004418 × 10¹⁴ m³/s² | WGS 84 |
| GM Moon | 4.9048695 × 10¹² m³/s² | DE440 |
| Separation | 1 AU = 1.495978707 × 10¹¹ m | IAU |
| Mass ratio μ | 3.0404 × 10⁻⁶ | Derived |

L1 comes from Newton's method on the collinear quintic: about 0.01001 AU (1.497
million km) from the EMB. Tests check that a body at rest there feels zero
rotating-frame acceleration.

Linearized about L1, Richardson's coefficient is c₂ ≈ 4.061. The in-plane
eigenvalue λ ≈ 2.533 gives an unstable e-folding time of about 23 days; the
in-plane and vertical frequencies (ω_p ≈ 2.086, ω_v ≈ 2.015) give periods of
about 175 and 181 days, bracketing the ~178-day periods flown by SOHO, ACE, and
DSCOVR. The station starts on a linear Lissajous orbit with about 48,000 km of
along-track and 50,000 km of out-of-ecliptic amplitude.

Integration is classical fourth-order Runge–Kutta with steps of at most 0.002
time units (about 2.8 hours). Without control, the Jacobi integral is conserved
to better than one part in 10⁹ over 60 days, and a 1 cm/s perturbation grows
past a million kilometers within 400 days. That is why real L1 missions fly
station-keeping maneuvers.

### Station-keeping

Every seven mission days the station evaluates the linear unstable mode, using
the left eigenvector of the linearized system, and applies the smallest
in-plane Δv that cancels it. Burns below 0.5 mm/s are skipped. The orbit stays
bounded over two years, spending about 8.6 m/s per year in tests (105 burns). Flight controllers use
nonlinear halo targeting and typically spend a few m/s per year; this
linear-mode controller is simpler, which is why it uses a modest orbit. Burns
appear as brief glows on the truss-tip thruster pods and in the HUD.

### What you see in the sky

- The **Sun** is at −Z with its true angular diameter (~0.54°). Structures are
  lit from that side; the solar arrays face it and the radiators stand edge-on.
- The **Earth** is at +Z, about 0.49° across, fully lit because L1 sees its day
  side. A cyan reticle marks it. As the station moves around its orbit, the
  Earth drifts a few degrees across the sky, following the Sun–Earth–vehicle
  angle.
- The **Moon** circles the Earth once per synodic month (29.53 days), inclined
  5.1° to the ecliptic, and appears within about 15° of the Earth.

The Earth's position includes the Moon's reflex motion about the barycenter.
Bodies are drawn on a 1.85 km sky shell at their true angular sizes, inside the
camera's 2 km far plane. Stars are a fixed decorative field, not a catalog.

## Local physics

Near the station, [`station.rs`](../../crates/verse-lagrange/src/station.rs)
uses the CR3BP linearized about L1 in SI units, including the Coriolis terms of
the rotating frame. At station scale the tidal field is about 10⁻¹¹ m/s², true
microgravity, but it is integrated so free parts obey the real field.

Scene axes: −Z points at the Sun, +Z at the Earth, +Y at ecliptic north, and +X
along Earth's orbital motion.

### The astronaut and the pack

| Property | Value |
| --- | --- |
| Suited astronaut plus pack, dry | 230 kg |
| Nitrogen propellant | 20 kg |
| Thrust along any commanded direction | 40 N |
| Specific impulse (cold N₂) | 70 s |
| Flight-control speed limit | 2 m/s relative to the station |
| Minimum-impulse deadband | 4 mm/s |
| Safety tether range | 140 m from the station's center |

The pack is a hypothetical construction unit comparable to the Manned
Maneuvering Unit, not a model of a specific flight article. Its fly-by-wire
control law commands a velocity: joystick input asks for 2 m/s along the
commanded direction, and no input asks the pack to hold position. The pack
thrusts toward that command within the 40 N limit. Propellant use follows
ṁ = F / (Isp g₀), and the HUD's Δv reserve follows the ideal rocket equation
over the current total mass. With no propellant, there is no thrust and the
astronaut coasts. The airlock ring refills the tank at 2 kg/s when you are
nearly stopped inside it.

Attitude is held automatically, so the astronaut turns only in yaw. Collisions
treat the body as a 0.9 m sphere against the habitat, node, truss, solar arrays,
and depot backboard, removing closing velocity with a soft 0.2 restitution. The
keel jig is open lattice and can be flown through.

### Parts and rigid bodies

| Part | Mass | Envelope |
| --- | --- | --- |
| Main engine | 450 kg | 2.2 × 2.2 × 3.0 m |
| Propellant tank | 320 kg | 2.6 m diameter, 3.6 m |
| Aft and fore keel trusses | 180 kg each | 1.2 × 1.2 × 4.0 m |
| RCS pod | 140 kg | 2.4 × 1.0 × 1.2 m |
| Avionics bay | 90 kg | 1.4 m cube |

Parts are torque-free rigid bodies with principal moments of inertia (uniform
boxes; the tank is a thin-walled cylinder). Euler's equations are integrated
with RK4 and the attitude quaternion with the exponential map. Tests check
conservation of angular momentum and rotational energy over two minutes and
reproduce the Dzhanibekov effect: a spin about the intermediate axis flips while
spin about the major axis stays stable.

Grabbing is a perfectly inelastic capture, so momentum is conserved and the
combined mass slows the pack. Holding a part moves it rigidly with the
astronaut. Releasing it away from its slot leaves it drifting at the release
velocity with a small residual tumble, as real hands never let go perfectly.
Drifting parts more than 120 m from the depot are reeled back by their tethers.

## Approximations

- The Sun and the EMB move on circular orbits (no eccentricity), the Moon is a
  point on a circular inclined orbit, and planetary perturbations and solar
  radiation pressure are omitted.
- Station-keeping cancels only the linear unstable mode, so it spends more Δv
  than flight halo control and uses a smaller orbit.
- Local physics linearizes about L1 rather than about the moving station.
- The astronaut's attitude is held; only translation is simulated for the pack.
- Carried parts do not collide with the station.
- Construction state is local and resets on each visit; there is no shared
  editing authority. `StationState` can be saved and restored, but the zone
  does not persist it.

## Tests

`cargo test -p verse-lagrange` covers L1's location and linear constants, the
exact unstable eigenvector, Jacobi conservation and uncontrolled divergence,
two years of controlled flight, microgravity magnitude, the rocket equation
and position hold, inelastic capture, latch conditions, collisions, frame-rate
independence, the frame step cap, save and restore, and journal replay.
`cargo test -p physics` covers the shared rigid-body, clock, world, and trace
mechanisms.
`cargo test -p verse --lib zones` covers portal entry and return, flight, and
the grab-carry-latch flow. Render the scene offline with:

```sh
cargo run -p verse --all-features --example lagrange_capture -- out.png [spawn|jig|carry|sun|earth]
```
