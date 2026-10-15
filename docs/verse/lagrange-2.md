# Lagrange 2

A spaceship factory on a halo orbit about Earth–Moon L2 ([issue
#11625](https://github.com/OpenAgentsInc/openagents/issues/11625)). Four
robot arms build a ship from parts in microgravity, and then the ship flies
a test burn away from L2. All of it is simulated: the orbit, the arms, the
factory's reaction and attitude control, and the burn. None of it is
animated.

You walk in from the Grid through the **LAGRANGE 2** arch, and walk back
out through the **THE GRID** arch on the deck's aft-port corner. You fly
free as a camera drone. Nothing you do pushes the factory.

The design background is in
[docs/spacecraft-design/](../spacecraft-design/README.md). This page
describes what was built and where it lives.

| Crate | What it owns |
| --- | --- |
| `verse-lagrange` (`earth_moon`, `orbit::Collinear`) | Earth–Moon constants, L1 and L2, the halo orbit, station-keeping, departure |
| `verse-lagrange2` | The spacecraft document, the factory arms, the stage-checked assembly, and the test burn |
| `verse-zone-lagrange2` | The scene, the sky, the clocks, the HUD caption, and the orbit display |
| `verse` | The `ZoneId::Lagrange2` zone, the Grid arch, and `examples/lagrange2_film.rs` |

## The orbit

The model is the circular restricted three-body problem in the Earth–Moon
rotating frame.

**Constants.**
- μ = 0.0121506, from the DE440 GM values: the Earth's GM is
  `earth_moon::GM_EARTH_DE440` and the Moon's is `orbit::GM_MOON`.
- The Earth–Moon distance is D = 384,400 km.

**L1 and L2.** One collinear-point type (`orbit::Collinear`) gives both
points:
- L2 is 64,515 km beyond the Moon's center.
- c₂ = 3.1904.
- The instability e-folds every 2.0 days.
- Sun–Earth L1 is the same code with its own signs.

**The halo.**
- It is a southern L2 halo found by differential correction, with
  continuation in z₀. Its period is 14.77 days.
- Its half-widths are about 12,600 km in x, 36,200 km in y, and 14,000 km
  in z.
- On it, the Earth never passes behind the Moon. The tests check 10
  revolutions.

**Station-keeping.**
- Once a day the controller targets the reference halo two days
  downstream, using the state transition matrix.
- Three disturbances keep it honest:
  - solar radiation pressure on a large factory;
  - 1 km and 1 mm/s of navigation error;
  - 2% burn-execution error.
- A year costs 7.39 m/s in about 360 burns. ARTEMIS flew Earth–Moon L2 for
  7.39 m/s a year (Folta et al. 2013). That match was not tuned; the test
  accepts 2 to 15 m/s.

The orbit tests are in `crates/verse-lagrange/src/earth_moon_tests.rs`.

## The spacecraft document

`verse_lagrange2::ship::Spacecraft` is the document. Its schema tag is
`spacecraft.v1`.

**What a part carries:**
- its kind: truss, module, tank (with propellant), or engine (with thrust
  and Isp);
- its subsystem;
- its maturity class, which sets its mass contingency (2%, 10%, or 20%);
- its shape, which gives its inertia;
- where it sits in the ship;
- the latch to its parent, with a rated force and moment.

**Derived values:**
- total mass;
- center of mass;
- the inertia tensor, as a parallel-axis sum;
- Δv from the rocket equation;
- the thrust-line offset;
- each latch's load at ignition: the mass on the far side of the latch from
  the engine, times the acceleration, plus its moment.

**Checks.** Every edit (`Spacecraft::edit`) must pass these. A failed edit
is refused with the requirement's name:
- the mass budget;
- the thrust line;
- the test burn's Δv;
- the latch loads;
- the assembly tree: one root on the cradle, no loops, no orphans.

Each accepted edit bumps `revision`, and `version()` is a content hash of
the document.

**The sample ship** is a 98.4 t tug, 105 t with contingency against a
115 t budget. It has:
- a two-piece keel;
- two 24.5 t tanks;
- a 120 kN, Isp 330 s engine;
- a habitat;
- a power box and a radiator.

It carries about 1.9 km/s of Δv.

**Its parts come from CAD.** The structure is a CAD Lab assembly,
`crates/verse-lagrange2/cad/lagrange-tug.json`: a 17 mm aluminium square
tube for each keel, thin-walled tanks and habitat hull, a steel engine
shell, and box tubes for the power and radiator panels, joined by
fastened joints at the latch points. Its mass feed (`cad-mass-feed.v1`,
`lagrange-tug.mass.json`, written as `cad_export` `feed` does, by
`cargo run -p cad --example mass_feed`) gives each part's dry mass,
center of mass, and inertia tensor, and each joint's origin.
`Spacecraft::from_feed` builds the document from it; a small typed table
adds what CAD does not know: each part's role, propellant, engine,
maturity, the envelope the arms grasp, and its latch rating. Propellant
adds a solid fill's inertia to a tank's measured shell. The `cad` crate's
test `the_lagrange_tug_feed_is_its_documents` keeps the checked-in feed
equal to what CAD measures for the document.

## The arms

Each arm is Robot Lab's SO-101 at 100 times its size (`arm.rs`).

**Kept from the SO-101:**
- the same five joints, in the same order (pan, lift, elbow, wrist flex,
  wrist roll);
- the same joint ranges, from the pinned URDF;
- the link lengths, scaled.

**Changed:**
- The elbow and wrist zeros sit at bends (90° and 45°), as the SO-101's
  URDF frames do.
- Masses (1 to 4 t per link) and motor ratings (120 to 600 kN m) are sized
  like flight arms rather than scaled from the SO-101, since a solid 100×
  SO-101 would weigh hundreds of tonnes.
- Speed limits are 0.03 to 0.05 rad/s.

**The motor model** is Robot Lab's: an effort-capped, speed-capped position
servo per joint.

**In the physics world**, each arm is a chain of `physics` hinges. The
first hinge is anchored on the factory body, so every move pushes the
factory back. The test `the_arm_is_the_so101_scaled_up` checks the limits
and lengths against the URDF.

## The factory

The factory is one `physics` world (`factory.rs`).

**The deck:**
- 800 t;
- 96 × 96 m;
- free-flying.

**Attitude control.** A CMG model holds the deck's attitude:
- The controller's torque on the deck comes out of the CMGs' stored
  momentum, so deck, arms, parts, and CMGs together conserve angular
  momentum exactly.
- Capacity is 1 MN m s, and torque is limited to 400 kN m.
- Running out of CMG momentum is a named failure.

**Parts:**
- They start clamped on racks beside their arms.
- An arm grasps a part with a weld at its grapple fixture.
- The arm places the part within the latch's capture envelope: 3 cm, 1°,
  and 1 cm/s.
- The latch then closes as a weld at the design pose, rated at the
  document's latch rating.

**Clearance.** Checks use capsules for links and boxes for parts. They
refuse an arm passing through a part, another arm, or the deck.

## Assembly: plan, simulate, check, act

`assembly.rs` follows the loop in
[autonomy-and-environment.md](../spacecraft-design/autonomy-and-environment.md)
and the [Aether AI plan](../research/aether-ai/README.md).

- **Plan.** `Plan` (`assembly-plan.v1`) lists each part in precedence
  order, with its arm and parent. It is made for one ship version, after
  inverse-kinematics reach checks at every waypoint.
- **Simulate.** Before a stage moves anything, the sequencer runs it on a
  copy of the factory, with the same physics and no faults. The run happens
  a few steps per real step, so the zone never stalls. If the copy hits a
  limit, the stage is refused and nothing moves. The limits are:
  - reach;
  - a motor at its effort limit, or unable to track;
  - a collision;
  - CMG saturation;
  - the attitude box.
- **Act.** The robot closes every fast loop itself:
  - **Tracking:** each joint follows its move.
  - **Latch envelope:** the latch closes only inside it.
  - **Stuck latch:** the arm steps back and retries, at most twice, then
    makes safe.
  - **Controller reset:** the move restarts from the last passed
    checkpoint. The part stays in the grapple.
- **Check.** After each stage:
  - the part must be latched;
  - it must be within 5 mm and 0.2° of its place;
  - it must hold a 5 kN proof load, applied as an internal pair, so the
    momentum books still balance;
  - it must match the simulation within 1 cm;
  - the factory must be inside its attitude box;
  - nothing may be unrestrained.

  A failed check makes safe: every arm stops, parts stay held, and the
  attitude hold continues.
- **The ground** approves the plan and supervises by stages. No stage
  starts while the link is down. The stage under way finishes, and the
  sequence resumes on reconnect.

**Hold points.** No stage starts while anything holds it; the sequence
resumes from a fresh simulation of the next stage.
- **A solar particle event warning** makes the factory go quiet within
  15 factory minutes (`QUIET_WITHIN`): a stage that has not grasped its
  part stops, backs off the rack, and parks, leaving the part on its rack;
  a part already in an arm is latched and released first, the shortest
  way to a secured state. Then every arm is parked, every part is on a
  rack or latched, and nothing moves until the all clear
  (`solar_event_quiet_hold`).
- **A collision-avoidance burn** from the ground waits for the next hold
  point: the stage under way finishes. The deck's thrusters then push
  10 kN until the whole factory's center of mass has the requested
  velocity change, the arms, racks, and latched work riding on their
  joints. After a 10 s coast the latched parts must still be within their
  pose tolerance with no latch at its rating (`debris_avoidance_hold`).

**Frame rate.** Hosts pace the factory with one clock
(`assembly::frame_clock`): frame time times the speed-up becomes whole
1/30 s steps, the rest carried to the next frame, at most 240 steps a
frame. The assembly at 30, 60, and 144 frames a second matches stepping
the same count exactly (`assembly_is_frame_rate_independent`).

**How long it takes.** The sample ship takes 8 stages and about 67 factory
minutes. The CMGs peak at a few hundred kN m s, well under their capacity.

**The arms against MuJoCo.** `oracle.rs` writes the factory arm, carrying
a loaded 24.5 t tank, as MuJoCo XML with the same bodies, joints, limits,
and servo law. MuJoCo 3.3.7 runs a 40 s move from the parking pose to the
transit height through Robot Lab's native runner
(`cargo run -p robot-sim --features mujoco --example lagrange2_arm_oracle -- EXECUTION.json crates/verse-lagrange2/oracle/arm-reference.json`)
and records `oracle/arm-reference.json`. `physics` replays the same
targets; on an Apple M-series Mac the largest joint difference is
1.5 × 10⁻⁵ rad, against #11607's 0.025 rad tolerance
(`arms_match_mujoco_for_a_reference_move`). The test fails if the arm
changes without a new reference.

## The test burn

`burn.rs` flies the assembled ship as a rigid body.

**Mass properties** come from its parts as the tanks drain.

**Each step:**
- drains `dm`;
- changes velocity by `v_e dm / (m − dm/2)`.

This is momentum-exact against the exhaust, and matches the rocket equation
to the midpoint rule's error.

**Attitude.** The gimbal aims through a lagged estimate of the center of
mass, and reaction-control thrusters hold attitude.

**The keel rings.** The ship's first axial mode is a two-mass lumped
model: the parts forward of the keel joint ride on a spring `E A / d`,
with the keel's section from its CAD mass (`A = m / (ρ L)`) and `d` the
distance between the two halves' centers of mass, at 1% damping
(`physics::Mode`). It sits near 23 Hz. Ignition is a step load: the keel
joint overshoots to 1.97 times its static 41 kN, and at cutoff the keel
rings down at its damping (`the_burn_rings_the_truss`). The 80 kN peak is
inside the joint's 300 kN latch rating, which the static check alone
would not show.

**Departure.** After 150 m/s, the velocity is handed to the three-body
integrator. The ship is more than 20,000 km off the halo three days later,
and the orbit display draws its path.

## The zone

**Two clocks:**
- The factory runs at 8× by default. **Slower** and **Faster** span 1× to
  64×.
- The orbit runs 60× faster than the factory, so a station-keeping burn
  comes every few factory minutes.

**Controls.** **Pause**, **New ship**, **Forces** (CMG momentum and arm
reach), **Art** (exposure), and **The Grid**.

**The sky** is physical:
- The Sun lies in the orbit plane, and the deck is tilted 60° toward it.
- The Moon is seen from its far side, 3 to 4° across.
- The Earth is 1.6° across, beyond the Moon.

**The orbit display** floats above the deck. It shows:
- the halo;
- L2;
- the Moon;
- the direction of the Earth;
- where the factory is now;
- after the burn, the ship's path away.

**The assembly loops.** A minute after the ship leaves, the racks refill
and the arms start a new ship.

## Where it runs

- **OpenAgents Desktop.** The Grid's engine renderer draws only the Grid.
  When a walk-in arch leads to a zone, `crates/openagents-desktop/src/grid.rs`
  switches to the legacy renderer's `render::Layer` on the window's device,
  and switches back on return.
- **Web.** `crates/everglade-web/src/web.rs` swaps the canvas from the
  Grid engine to the legacy renderer when the Grid page walks into a zone,
  and back again. The page keeps ticking while the next renderer opens.
- **Phones.** The phones already reopen their surface between the Grid and
  the legacy zones (`reopen_for_zone`), so the arch works there with no
  phone-specific code.

## The demo film

```sh
cargo run -p verse --example lagrange2_film -- film ~/Downloads/lagrange-2-factory.mp4
cargo run -p verse --example lagrange2_film -- still out.png 1200 aft
```

The film:
1. walks in from the Grid;
2. watches the assembly at about 90× (faster while stages are being
   simulated);
3. follows the ship's burn;
4. ends on the orbit display with the departure path.

The still views are `aft`, `wide`, `close`, `display`, `moon`, and `burn`.

## Approximations

- **The tidal field across the deck** (about 2 × 10⁻⁹ m/s²) is left out.
  The CR3BP carries the orbit.
- **Attitude.** The factory holds a fixed attitude in the rotating frame,
  and the CMGs are an ideal momentum store with a capacity and a torque
  limit.
- **During the burn**, which takes about two minutes, the ship flies in the
  station's local frame, and its velocity is then handed to the
  three-body integrator.
- **Propellant** adds a solid fill's inertia to each tank's measured
  shell. The ship has no slosh.
- **Structural modes.** Only the keel's first axial mode is modelled, as
  two lumped halves (see the test burn above). Bending, the tanks' latch
  modes, and slosh are left out.
