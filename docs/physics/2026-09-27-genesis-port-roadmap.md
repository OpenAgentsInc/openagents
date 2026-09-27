# Genesis example port roadmap for Verse physics

**Status:** Roadmap, 2026-09-27. Tracking issue [#9788](https://github.com/OpenAgentsInc/openagents/issues/9788). Follows [Genesis lessons for Verse physics zones](2026-09-27-genesis-for-verse-zones.md). No zone authority, networking, or runtime dependency on Genesis is approved here.

Porting means **reimplementing the mechanism in Rust in the shared, zone-agnostic [`physics`](../../crates/physics/src/lib.rs) crate** and proving it with a test there. Zones consume it. Lagrange 1 is the first consumer and the acceptance scene, not the home: any zone that needs rigid bodies, contact, joints, thrusters, or sensors uses the same crate. No Genesis source, assets, or Python runtime enter the repo. Genesis stays a pinned reference and, where noted, an offline oracle for calibration runs.

Reference: [Genesis `236719768c72e8d0cefb1607565885fd2c1bab6b`](https://github.com/Genesis-Embodied-AI/Genesis/tree/236719768c72e8d0cefb1607565885fd2c1bab6b) (v1.4.2 + 20, Apache-2.0), cloned read-only at `projects/repos/Genesis` in the workspace. All 123 files under `examples/` were surveyed.

## Progress

All nine phases landed on 2026-09-27, in the shared
[`physics`](../../crates/physics/src/lib.rs) crate with Lagrange 1 as the first
consumer: GP-0 `ade3d61631`, GP-1 `56a2664a94`, GP-2 `e098cd7363`, GP-3
`b52b5ea804`, GP-4 `c1e93251b8`, GP-5 `136f756f4c`, GP-6 `eb9c42383c`, GP-7
`8e2a99608f`, and GP-8 (this change).

- **Budget.** `cargo run --release -p verse-lagrange --example step_budget`
  runs 30 s of a busy scene: the astronaut carrying the engine under thrust
  and attitude control while five parts strike the station. On an Apple M5 Max
  a step takes 0.018 ms on average and 0.054 ms at worst. The budget is 1 ms
  per step on the slowest supported phone; the phone measurement is pending
  with the owner (the **Forces** overlay shows the step time on device).
- **Randomized invariants.** Seeded tests vary mass, inertia, shape, center
  of mass offset, friction, restitution, joint stiffness, and tether length:
  momentum is exact through collisions and joints, loads below the Coulomb
  limit hold and past it slip, and tethers never add energy. A Lagrange test
  throws every part at the station at random speeds and spins.
- **Genesis oracle.** [`crates/physics/oracle/`](../../crates/physics/oracle/README.md)
  replays the tank-into-panel scene in Genesis. Approach matches to rounding
  and post-impact spin within about 5%, but the rebound differs because the
  restitution models differ, so no fidelity is claimed yet.

## Selection

Lagrange 1 is a six-part EVA assembly in microgravity at Sun–Earth L1: one astronaut with a thruster pack, drifting parts, a fixed station, a jig with slots, and a tether range. An example is worth porting when it demonstrates a mechanism that scene lacks today (see the gap table in the [audit](2026-09-27-genesis-for-verse-zones.md#what-exists-in-verse)): contact with torque, force at a point, constraint-based grasp/latch/tether, sleep, telemetry, or a conservation test.

| Example | Mechanism to port | Phase |
| --- | --- | --- |
| [`ipc/ipc_momentum.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/ipc/ipc_momentum.py) | Zero-gravity momentum ledger: fire one body into another, sum total momentum each step, report relative error | GP-1 |
| [`collision/contact_manifold.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/collision/contact_manifold.py) | Box–box multi-point manifold, exercised by a scripted kinematic sweep that logs manifold size changes | GP-3 |
| [`collision/contype.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/collision/contype.py) | `contype`/`conaffinity` bitmask collision filtering | GP-3 |
| [`rigid/friction_breakaway.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/friction_breakaway.py), [`rigid/torsional_grasp.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/torsional_grasp.py) | Elliptic Coulomb cone with torsional friction; held-vs-slipped classification by accumulated drift | GP-3, GP-4 |
| [`rigid/apply_external_wrench.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/apply_external_wrench.py) | Per-link external force + torque applied before the step | GP-2 |
| [`drone/quadcopter_controller.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/drone/quadcopter_controller.py), [`drone/interactive_drone.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/drone/interactive_drone.py) | Cascaded position → velocity → attitude control feeding a thruster mixer; hold/release key input | GP-2 |
| [`viewer_plugin/mouse_interaction.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/viewer_plugin/mouse_interaction.py) + [plugin](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/genesis/vis/viewer_plugins/plugins/mouse_interaction.py) | Grab a body **at the hit point** with a critically damped implicit spring impulse, plus pendulum-swing damping | GP-4 |
| [`rigid/suction_cup.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/suction_cup.py) | Runtime `add_weld_constraint` / `delete_weld_constraint` between two links | GP-5 |
| [`rigid/closed_loop.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/closed_loop.py) | `connect` (point) versus `weld` (point + orientation) equality constraints | GP-5 |
| [`rigid/bolt_nut_self_screw.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/bolt_nut_self_screw.py) | Stiff constraint time constant plus substeps for tight contact; release drive when seated | GP-5 (notes), GP-8 |
| [`rigid/wrecking_ball.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/wrecking_ball.py) | Chain of free links held by contact, welded to a dense end mass | GP-5 (deferred option) |
| [`rigid/hibernation.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/hibernation.py) | Island sleep; live awake-body count and physics step rate | GP-6 |
| [`sensors/contact_force_go2.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/sensors/contact_force_go2.py), [`sensors/imu_franka.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/sensors/imu_franka.py), [`sensors/lidar_teleop.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/sensors/lidar_teleop.py), [`tutorials/draw_debug.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/tutorials/draw_debug.py) | Contact-force and IMU sensors, raycast sensor, debug overlay of spheres/lines | GP-7 |
| [`rigid/domain_randomization.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/domain_randomization.py), [`rigid/set_phys_attr.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/rigid/set_phys_attr.py), [`speed_benchmark/timers.py`](https://github.com/Genesis-Embodied-AI/Genesis/blob/236719768c72e8d0cefb1607565885fd2c1bab6b/examples/speed_benchmark/timers.py) | Per-run mass/CoM/friction variation; step timing | GP-8 |

**Not ported:** locomotion and manipulation RL (`locomotion/`, `manipulation/`, `drone/hover_*`), IK and robot control tutorials, FEM/MPM/SPH/PBD/IPC/SAP deformable and fluid coupling, smoke, rendering and USD import, multi-GPU/DDP, differentiable push. A vacuum EVA scene with six rigid parts has no use for them. The audit keeps fluids and deformables for a future pressurized habitat study.

## Phases

Each phase lands as its own change. It adds the generic mechanism and its tests to `crates/physics`, then moves Lagrange 1 onto it with a zone test in [`crates/verse-lagrange/src/tests.rs`](../../crates/verse-lagrange/src/tests.rs) and a note in [the L1 physics guide](../verse/lagrange-1.md). Zone-specific rules stay in the zone crate: for Lagrange, the orbit, the L1 tide field, the pack's thruster layout, part and slot definitions, and latch thresholds. The existing CR3BP, Jacobi, station-keeping, propellant, capture-momentum, latch-threshold, and station-collision tests must keep passing at every phase.

### GP-0 — Fixed step, restorable state, scripted scenarios ([#9786](https://github.com/OpenAgentsInc/openagents/issues/9786))

Prerequisite from the audit's adaptation step 1. No Genesis port, but it is the harness every later phase tests with.

- Create `crates/physics`: `Body` (moved from `verse_lagrange::RigidBody`, with body kinds dynamic/static/kinematic), a `World` of bodies stepped at a fixed `dt` under a caller-supplied acceleration field, a `FixedStep` accumulator, previous-pose interpolation, and a comparable `Trace`. No zone types.
- `Station::step` accepts frame time into an accumulator and advances fixed `PHYSICS_DT` (start at 1/120 s) substeps, with a named maximum substep count per frame. Report the time it drops; don't silently drop it. The orbit advances by the same accounted time × `ORBIT_WARP`. The renderer interpolates.
- Add a serializable, versioned `StationState` distinct from the existing HUD [`Snapshot`](../../crates/verse-lagrange/src/station.rs): orbit state, station-keeping schedule, bodies, grasp/install state, propellant, target, climb, tick.
- Scenario runner in tests: a named initial state, a `Command` script by tick, and checkpoints of pose/momentum/propellant/mission time. Copy the **pattern** of `contact_manifold.py`: one scripted sweep through phases, log only on state transitions, and a short-run mode for CI versus a long run for local investigation.
- NIP-MV remote commands (`fly`, `grab`, `release`, `stop`) from [#9773](https://github.com/OpenAgentsInc/openagents/pull/9773) join the same tick-stamped command stream, so remote and local input replay identically.

**Accept:** Same state and command stream give identical results on one machine. Results at 30, 60, and 144 fps frame pacing agree within a stated tolerance. Save → restore → continue matches an uninterrupted run.

### GP-1 — Zero-g momentum ledger (from `ipc_momentum.py`) ([#9778](https://github.com/OpenAgentsInc/openagents/issues/9778))

That example sets gravity to zero, launches a cube at 4 m/s into a sphere, and tracks per-body and total momentum plus relative error. Port that measurement as a reusable test helper, not the IPC solver.

- `Station::momentum_ledger()` returns total linear momentum and angular momentum about the station origin for astronaut + free/carried parts. Report thruster impulse and propellant exhaust momentum as named external terms.
- Run the L1 tide and station-keeping **off** for ledger scenarios, so external force is zero by construction.
- Scenarios: coast, part–part collision, astronaut–part collision, grab, carry, release, and every thruster burn with its exhaust term.

**Accept:** Relative momentum error below a stated bound (start 1e-9 for coasting, 1e-6 through contacts) in every ledger scenario. This test gates GP-3 to GP-5. Today's carry path, which sets the part's velocity to the astronaut's, is expected to **fail** it; record that as the baseline.

### GP-2 — Force at a point and thruster allocation (from `apply_external_wrench.py`, drone controller) ([#9779](https://github.com/OpenAgentsInc/openagents/issues/9779))

- `Body::apply_force_at(point, force)` and `apply_torque(torque, dt)` accumulate a wrench cleared after each step, as `apply_links_external_wrench` does.
- Replace the pack's single center-of-mass force with named thruster positions and directions on the astronaut body. Allocate a desired force and torque to thruster firings with a mixer, the way `DronePIDController.__mixer` maps thrust/roll/pitch/yaw to four rotors. Allocate by bounded least squares or a fixed table over thruster pairs. Propellant is spent by total thrust × time / (Isp·g₀), the same as today.
- Cascade control like `quadcopter_controller.py`: position (fly-to target) → velocity (existing 2 m/s limit and deadband) → attitude. Yaw becomes an attitude **target** held by thrusters rather than a value written to `self.yaw`. Keep the hold/release input model from `interactive_drone.py` for keyboard, and map it for the mobile stick.
- Plumes render at the actual firing thrusters.

**Accept:**
- An off-center single thruster produces the expected linear and angular acceleration from the inertia tensor.
- Holding attitude through a translation keeps orientation error within a bound.
- Propellant use matches impulse / exhaust velocity.
- The GP-1 ledger balances when the exhaust term is included.

### GP-3 — Contact shapes, filters, manifolds, friction (from `contact_manifold.py`, `contype.py`, friction examples) ([#9780](https://github.com/OpenAgentsInc/openagents/issues/9780))

- Collision shapes: an oriented box for each part, a capsule for the astronaut, boxes for station modules. Truss lattice uses a compound of boxes or is marked non-solid. Keep six-part scale in mind: brute-force pair tests are fine; no broad-phase tree until GP-6 measurements ask for one.
- Filtering: a `contype`/`conaffinity` bitmask pair per shape; two shapes collide when `a.contype & b.conaffinity | b.contype & a.conaffinity != 0`. Use it for "carried part does not collide with the carrier", "installed parts collide only with free bodies", and the non-solid lattice.
- Box–box manifold: clip the reference face against the incident face and keep up to four points. This is the "contact patch" scheme in `contact_manifold.py`, not re-detection on perturbed copies. Port its scripted tilt/yaw/penetration/slide sweep as a test that asserts manifold size per phase.
- Sequential-impulse solver with restitution, a Coulomb cone for tangential friction, and torsional friction about the normal (`torsional_grasp.py`). Use the **elliptic** cone behavior as the target: `friction_breakaway.py` shows the pyramidal cone creeping well below the Coulomb limit. Port its held/slipped drift criterion as a test that sweeps load fractions 0.25–1.05 of μ·N.
- Swept or substepped tests for fast parts against thin panels.

**Accept:**
- The GP-1 ledger passes through collisions.
- No tunneling at the maximum release speed.
- Penetration stays within a stated slop.
- The breakaway sweep holds at ≤ 0.95 and slips at 1.05.
- The spinning-tank-into-panel-edge experiment from the audit shows angular impulse.
- The old six-AABB sphere push-out ([`step_parts`](../../crates/verse-lagrange/src/station.rs)) is removed.

### GP-4 — Grab as a soft constraint at the hit point (from the mouse-interaction plugin) ([#9781](https://github.com/OpenAgentsInc/openagents/issues/9781))

This is the most directly portable mechanism. For each axis the plugin computes the arm from the center of mass to the grabbed point, the effective mass along that axis `1 / (1/m + (r×d)·I⁻¹(r×d))`, and a critical damping coefficient from spring stiffness. It then solves one **implicit** impulse whose end-of-step velocity follows the spring-damper response: softness `1 / (dt·(c + dt·k))`, bias rate `k / (c + dt·k)`. That stays stable at any stiffness. A second term damps the pendulum swing about the grabbed point using the inertia about that point.

- Pick the hit point by raycast from the hands (GP-7 raycast) against part shapes. Store it in part-local coordinates.
- Each step, apply the soft point-to-hand impulse to the part and the equal-and-opposite impulse to the astronaut. Carrying a heavy tank then slows and turns the astronaut, and the GP-1 ledger balances with no preset spin on release.
- Hold rotation with torsional/tangential friction limits from GP-3 when the grip is pinched. Refuse or break the grab above a maximum grip impulse and show that on the HUD.

**Accept:**
- Grab → rotate → release conserves momentum.
- A part grabbed off-center swings and is damped without overshoot.
- Exceeding the grip limit breaks the grab.
- The existing capture-momentum test is rewritten against the constraint rather than removed.

### GP-5 — Latch as weld, tether as connect (from `suction_cup.py`, `closed_loop.py`) ([#9782](https://github.com/OpenAgentsInc/openagents/issues/9782))

- Constraint set with runtime `add_weld(a, b, frame)` / `remove_weld(a, b)` as in `suction_cup.py`, solved in the same impulse loop as contacts.
- Latch: allowed only when position error, **orientation** error, relative linear speed, and relative angular speed are all under thresholds. Then add a weld between the part and the jig slot frame. Installed parts become static (GP-6). Unlatching is not required for L1 but the API supports removal.
- Tether: a unilateral **connect**-style distance constraint from the airlock anchor to the astronaut harness point. It is slack inside `EVA_RANGE` and becomes an inelastic limit with a maximum tension beyond it. That replaces the position clamp; keep the clamp only as an emergency boundary. `PART_TETHER` gets the same treatment.
- Tight-fit tuning note from `bolt_nut_self_screw.py`: close fits need a stiffer constraint time constant and more substeps. Expose both as named constants and test them rather than tuning by eye.
- Deferred option: a visible tether line built as a chain of short links (as in `wrecking_ball.py`) only if a single distance constraint looks wrong on screen. It costs many bodies for a visual effect.

**Accept:** The audit's docking experiment passes: excess angular speed or misalignment is refused, and a latched part stays fixed under impacts. At the 140 m limit the astronaut neither teleports nor gains energy, and recorded tether tension matches the momentum change.

### GP-6 — Sleep and static bodies (from `hibernation.py`) ([#9783](https://github.com/OpenAgentsInc/openagents/issues/9783))

- Build islands from bodies connected by contacts or constraints. An island sleeps after its bodies stay below linear and angular thresholds for N steps. It wakes on new contact, grab, thrust, or constraint change.
- **Microgravity rule:** never sleep an island that is not touching a static body. A drifting part at 1 mm/s is still moving. In L1 this effectively means installed parts (already static) and settled contact stacks on the station.
- Port the example's instrumentation: awake-body count and physics step rate, available in a debug HUD row and in the GP-8 timing report.

**Accept:** A slowly drifting free part never sleeps. A sleeping settled part wakes on contact and resumes with the same state. Step time drops measurably with sleeping bodies in a stress scene of about 50 parts, a test-only scene.

### GP-7 — Telemetry and debug (from sensors and `draw_debug.py`) ([#9784](https://github.com/OpenAgentsInc/openagents/issues/9784))

- Contact-force readout per body (`contact_force_go2.py`): normal and tangential impulse / dt, used by the HUD ("impact 340 N") and by tests.
- IMU on the astronaut (`imu_franka.py`): specific force and angular rate from the solved state. Show g-load and spin rate on the HUD; they explain thruster and tether events to the player.
- Raycast (`lidar_teleop.py`): used by GP-4 hit-point picking and a proximity readout to the nearest structure.
- Debug overlay (`draw_debug.py`, `contact_manifold.py` markers): contact points, normals, manifold polygons, constraint anchors, tether line, and thruster vectors. Only in the renderer adapter [`zones/lagrange.rs`](../../crates/verse/src/zones/lagrange.rs); the physics crate emits data only.

**Accept:** Sensor values match analytic cases: a thruster burn's acceleration, and a contact impulse in a head-on collision. The overlay can be toggled and costs nothing when off.

### GP-8 — Calibration, variation, budget (from randomization, timing, Genesis oracle) ([#9785](https://github.com/OpenAgentsInc/openagents/issues/9785))

- Property tests varying part mass, center-of-mass offset, inertia, friction, and restitution within authored ranges, as `domain_randomization.py` and `set_phys_attr.py` vary them per environment. Every GP-1 to GP-5 invariant must hold across the sampled range.
- Step budget: per-step timing for contacts, constraints, and orbit, as in `speed_benchmark/timers.py`. Record it on desktop and on the lowest supported mobile device, and set a frame-time budget.
- Genesis oracle, offline and optional: for GP-3 and GP-4, write pinned Genesis scripts that build the same geometry, masses, initial pose, and timestep with zero gravity. Record trajectories and compare them against the Rust scenario within tolerance. The scripts and recorded traces live outside the Verse runtime. The first such script and its location get proposed in the GP-3 change, not here.

**Accept:** Randomized invariants pass in CI at a bounded case count. The measured mobile step fits the budget. Oracle comparisons for the tank-into-panel and off-center-grab scenes agree within stated tolerances before the audit's "fidelity" claim is made.

## Order and dependencies

```
GP-0 ─► GP-1 ─► GP-2 ─┐
               └────► GP-3 ─► GP-4 ─► GP-5 ─► GP-6
                                         GP-7 (alongside GP-3 onward)
                                         GP-8 (after GP-5)
```

GP-0 and GP-1 are small and unblock everything. GP-3 to GP-5 are the visible realism, per the audit. GP-6 to GP-8 depend on having something to measure. Choosing a reviewed Rust collision/constraint library (for example `parry3d`/`rapier3d`) instead of hand-rolled GP-3/GP-5 code is decided at GP-3 against the mobile, determinism, and restorable-state requirements. Either way the ported tests stay the acceptance authority.

## Boundaries

- Generic physics lives in `crates/physics`, which has no renderer, I/O, or zone knowledge. Zone crates such as `verse-lagrange` own their rules and configuration; `crates/verse` only adapts input and meshes.
- No shared or networked physics authority is implied. NIP-MV commands remain inputs to a locally owned station.
- The orbital model and its approximations are unchanged by this roadmap. Any orbital fidelity change is a separate study, as the audit states.
