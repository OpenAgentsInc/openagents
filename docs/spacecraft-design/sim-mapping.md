# Mapping the knowledge to our code

How each topic in this folder maps to the crates we have, what Lagrange 2
([issue #11625](https://github.com/OpenAgentsInc/openagents/issues/11625))
still has to build, and the tests that would prove it, written in the style
of [Lagrange 1's tests](../../crates/verse-lagrange/src/tests.rs). Numbers
carry the marks from [the README](README.md); test thresholds come from [V]
or [C] values only.

## What exists

| Crate | What it gives Lagrange 2 | Gap |
| --- | --- | --- |
| [`verse-lagrange`](../../crates/verse-lagrange/) `orbit` | CR3BP acceleration, Jacobi integral, and RK4 that take any μ; the linear constants and unstable-mode left eigenvector; a linear Lissajous orbit; a station-keeping loop | The `L1` type solves only the L1 quintic, and its c₂ formula is L1's. No L2, no halo (differential correction), no Earth-visibility check |
| `verse-lagrange` `station` | The construction sandbox: parts with real mass properties, grab, latch, collisions, ropes, plumes, flexible arrays, the momentum ledger, save, restore, replay, two clocks | Built around one astronaut; no arms; linearizes the field about Sun–Earth L1 |
| [`physics`](../../crates/physics/) | Rigid bodies (`body`), contacts, joints including servoed `Hinge` and `Slider` with limits, `ThrusterSet` allocation and `Pid`, flexible `Mode`s, `Ledger`, ropes, plumes, sensors, fixed stepping | No CMG or reaction wheel model; no articulated-chain solver beyond joints; no generalized Jacobian helper |
| [`robot-description`](../../crates/robot-description/) | URDF import and pinned SO-101 | Industrial-arm descriptions for the factory |
| [`robot-lab`](../../crates/robot-lab/) | Versioned, checked lab documents (robot, scene, joints, inertials) and private storage | No spacecraft document kind |
| [`robot-sim`](../../crates/robot-sim/) | MuJoCo 3.3.7 runner, native and Linux-qualified, used as a reference for joints (#11607) | Not available on phone or web, so it can check our physics but not run the zone |
| [`verse-zone-lagrange`](../../crates/verse-zone-lagrange/) | Scene, input, and rendering for Lagrange 1, physical sky, PBR | A Lagrange 2 scene with the Moon large in the sky |

## What Lagrange 2 needs, by topic

### Orbit ([astrodynamics.md](astrodynamics.md))

1. **Generalize the collinear point.** Turn `orbit::L1` into a collinear
   point type that takes μ and which point (L1 or L2). The only differences
   are the quintic's signs, the point's x coordinate (1 − μ + γ for L2), and
   c₂, which for L2 is (μ + (1 − μ) γ³ / (1 + γ)³) / γ³ [C, checked by our
   computation reproducing the L2 table]. Keep the Sun–Earth tests unchanged.
2. **Earth–Moon constants** from DE440: GM_E = 398,600.435507 km³/s² and
   GM_M = 4,902.800118 km³/s² [V], D = 384,400 km [M, conventional, cite
   it]. The existing `GM_MOON = 4.9048695e12` labelled DE440 does not match
   DE440; fix it in its own small change (the Sun–Earth tests should still
   pass, since it moves the Sun–Earth μ by about 5 parts in a million) [C].
3. **Halo orbit** by differential correction from Richardson's third-order
   guess, in normalized units. Store the corrected initial state and period.
4. **Station-keeping** every day or less, since the e-folding time is about
   2 days [C]; Lagrange 1's 7-day cadence would let errors grow about 33
   times between burns. A downstream x–z crossing target, as ARTEMIS flew,
   is the better upgrade.
5. **Clock:** at Lagrange 1's 3,600× orbital clock, one 15-day halo
   revolution takes 6 minutes of play [C].
6. **Local field:** the tidal gradient near Earth–Moon L2 is about
   (2c₂ + 1) n² ≈ 5.2 × 10⁻¹¹ s⁻² [C], so 100 m from the station center the
   residual acceleration is about 5 × 10⁻⁹ m/s²: still microgravity, but
   five times Sun–Earth L1's.

### The spacecraft document ([design-method.md](design-method.md), [subsystems.md](subsystems.md))

A new document kind beside Robot Lab's, versioned the same way:

- Parts: kind, mass, maturity class (for contingency), local center of
  mass, inertia tensor, collider, ports (frame, type, rated latch load).
- Subsystem tags for the mass budget; the engine's thrust, I_sp, and mount
  frame; tanks with propellant mass and density.
- Derived values: total mass, center of mass, inertia tensor (parallel-axis
  sums), propellant fraction, Δv by the rocket equation, thrust-line offset.
- Checks: margin ≥ 0, thrust-line offset within a limit, every port's latch
  load margin ≥ 0 at the test burn's acceleration, and the port graph is
  connected and acyclic in its precedence.

### The factory ([in-space-assembly.md](in-space-assembly.md))

- At least four arms as chains of `physics` `Hinge` joints with servos,
  torque limits, and speed limits, scaled from the Robot Lab joint models.
- Arms mounted on the factory body, so their reactions move it.
- Factory attitude control: a CMG or reaction wheel model with a momentum
  capacity and saturation, plus thrusters for desaturation through the
  existing `ThrusterSet`.
- A deterministic assembly planner reading the port graph and writing a
  plan document; the runtime executes it and names any failure.

### The test burn

- The assembled ship's mass properties come from the welded parts; the
  engine fires through a gimbal that tracks the center of mass as
  propellant drains; the burn is momentum-exact like the astronaut's pack.
- After the burn, the ship's state hands back to the CR3BP integrator to
  show the departure.

## Tests

Each test below names what it checks and where its threshold comes from.
Thresholds marked "first run" are set from the first correct run and then
frozen, as Lagrange 1 did for its station-keeping cost.

### Orbit

| Test | Checks | Threshold source |
| --- | --- | --- |
| `l2_sits_where_the_earth_moon_system_puts_it` | μ within 10⁻⁶ of 0.0121506; L2 between 64,400 and 64,600 km beyond the Moon; zero rotating-frame acceleration at rest (< 10⁻¹²); c₂, λ, ω_p within 2 × 10⁻³ of 3.1904, 2.1587, 1.8626; e-folding 1.9–2.1 days; light time 1.4–1.6 s | [C] and [V] tables in astrodynamics.md |
| `earth_moon_l1_matches_the_published_offset` | L1 at 15.09% ± 0.01% of D short of the Moon | [V] Wikipedia table |
| `the_l2_unstable_left_eigenvector_is_exact` | Same identity as L1's test | Algebra |
| `l2_uncontrolled_motion_conserves_jacobi_and_leaves_l2` | Jacobi drift < 10⁻⁹ over 30 days; a 1 cm/s kick departs past a distance within a time | Jacobi from L1's test; departure first run |
| `a_corrected_halo_closes` | After one period, position within 1 km and velocity within 1 mm/s of the start; period 14–16 days for a large halo | [C] linear periods; [V] ARTEMIS 15.47 days |
| `a_halo_keeps_the_earth_in_view` | Over 10 revolutions, the Earth–ship line never passes within the Moon's radius | [C] geometry |
| `station_keeping_holds_a_halo_for_a_year` | Stays within a bound of the reference; annual Δv positive and under a cap; burns counted | First run; compare with ARTEMIS's 7.39 m/s per year [V] in the doc, not the test |

### Spacecraft document

| Test | Checks |
| --- | --- |
| `mass_properties_are_the_sum_of_parts` | Mass, center of mass, and inertia tensor equal a hand-computed parallel-axis sum to 10⁻⁹ relative, and equal the `physics` body built from the welded parts |
| `a_negative_margin_is_refused` | An edit that drives the system margin below zero is refused with the requirement's name |
| `an_off_axis_engine_is_refused` | Thrust line beyond its offset limit is refused |
| `latch_loads_follow_the_burn_acceleration` | Each port's load equals the outboard mass times acceleration plus the offset moment |
| `the_sample_ship_passes_every_check` | The checked-in sample ship is valid |

### Factory

| Test | Checks |
| --- | --- |
| `arm_motion_conserves_the_factorys_momentum` | With attitude control off and no thrusters, linear and angular momentum of factory plus arms plus parts stay constant to 10⁻¹⁰ relative through a full move, with the ledger as in L1 |
| `a_swing_turns_the_free_floating_factory_as_predicted` | Point-mass case: factory rotation equals −(m r² / I) Δθ within 1% |
| `the_generalized_jacobian_predicts_the_hand` | The hand's inertial velocity equals J* q̇ to 10⁻⁶ relative on a free-floating base |
| `a_closed_joint_loop_leaves_the_base_turned` | A joint path that returns to its start leaves a nonzero base rotation (nonholonomy) |
| `opposed_arms_cancel_their_reactions` | Mirrored moves leave the factory's attitude unchanged to 10⁻⁹ |
| `cmg_saturation_is_a_named_failure` | Exceeding the momentum capacity stops the plan with that failure |
| `a_torque_limit_is_a_named_failure` / `a_collision_is_a_named_failure` | Each failure stops the plan and names the arm, joint, or bodies |
| `four_arms_assemble_the_sample_ship` | The plan completes; every part latched at its port within the latch envelope; mass properties equal the document's |
| `assembly_replays_bit_for_bit` / `assembly_is_frame_rate_independent` | As L1's save, restore, replay, and frame pacing tests |
| `arms_match_mujoco_for_a_reference_move` | One scaled arm's joint trajectories match the MuJoCo oracle within its documented tolerance (#11607) |

### Test burn

| Test | Checks |
| --- | --- |
| `the_burn_obeys_the_rocket_equation` | Δv equals I_sp g₀ ln(m₀ / m_f) to 10⁻⁶ relative; exhaust momentum balances in the ledger |
| `the_gimbal_tracks_the_center_of_mass` | Body rate stays below a limit as propellant drains |
| `the_burn_rings_the_truss` | Flexible modes are excited and decay with their damping, as L1's arrays |
| `the_ship_departs_l2` | After the burn, the ship leaves a set neighborhood of the halo within a set time (first run) |

## Order of work

1. Collinear point generalization, DE440 constants, and the orbit tests.
2. Halo correction and station-keeping.
3. The spacecraft document and its checks with a sample ship.
4. One arm on a free-floating base with the conservation and Jacobian tests.
5. Four arms, the planner, failures, and replay.
6. The test burn.
7. The zone, sky, and demo capture.

Fault and operations scenarios (link loss, stuck latches, controller resets, solar events, restraint, debris holds) are listed in [autonomy-and-environment.md](autonomy-and-environment.md#4-what-this-adds-to-the-simulation).
