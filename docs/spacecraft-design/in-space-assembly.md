# Robotic assembly in microgravity

What we know about robot arms building structures in orbit, written for the
Lagrange 2 factory. See [the README](README.md) for the marks.

## What microgravity changes

- Nothing has weight, but everything keeps its mass and inertia. An arm's
  motors spend torque only on accelerating the arm and its load and on
  fighting contact, not on holding them up.
- Every force the arm applies to a part, it applies back to whatever it is
  mounted on. With no ground to push against, **the factory moves**.
- Parts do not settle. A released part keeps its velocity and spin, so a
  placement must null relative motion before release, and latches need
  capture envelopes. Lagrange 1 already models this for the astronaut.
- Motions are slow. Flight arms move loads at centimeters per second to
  limit the momentum to absorb and the impact energy if something goes
  wrong [M].

**Torque scale [C]:** moving a 20 t module whose center is 10 m from the
shoulder with an angular acceleration of 0.001 rad/s² needs about
20,000 × 10² × 0.001 = 2,000 N·m at the shoulder, ignoring the module's own
rotational inertia. Speed limits, not torque, usually dominate.

## Free-flying and free-floating bases

- **Free-flying:** the base's attitude (and sometimes position) is actively
  controlled with thrusters, wheels, or CMGs while the arm moves.
- **Free-floating:** base control is off. The arm and base form one system
  with conserved total momentum. The ISS is put into free drift for some
  arm captures of visiting vehicles so that thrusters do not fight the arm
  [M].

### The momentum relation

With no external force and zero initial momentum, the base's velocity
(linear and angular, stacked as ẋ_b) and the joint rates q̇ satisfy

H_b ẋ_b + H_bm q̇ = 0, so ẋ_b = −H_b⁻¹ H_bm q̇,

where H_b is the base's inertia matrix (including the arm's mass seen from
the base) and H_bm is the coupling inertia between the joints and the base.

### The generalized Jacobian

The end effector's velocity is v_e = J_b ẋ_b + J_m q̇. Substituting the
momentum relation gives

v_e = (J_m − J_b H_b⁻¹ H_bm) q̇ = J* q̇.

J* is the **generalized Jacobian** (Umetani and Yoshida, 1989 [V]). It
depends on the mass properties as well as the geometry, so:

- Resolved-rate control with J* moves the hand along a commanded path in
  inertial space even though the base reacts.
- J* has **dynamic singularities** where it loses rank, which depend on the
  path history, not only the current joint angles [M].
- Angular momentum conservation cannot be integrated into a constraint on
  positions: it is **nonholonomic**. A closed loop in joint space can leave
  the base rotated, as a falling cat rights itself [M; Xu and Kanade cover
  this].
- **Reaction null space:** for a redundant arm, some joint motions leave the
  base's attitude unchanged. Planning in that space avoids disturbing the
  base (Nenchev, Yoshida, and colleagues [M]).

### How much the factory turns [C]

Swinging a part of mass m at radius r from the factory's center of mass by
an angle Δθ turns a factory with inertia I_f by about −(m r² / I_f) Δθ when
attitude control is off, treating the part as a point mass. Example: a 20 t
module at 20 m on a factory with I_f = 5 × 10⁷ kg·m² gives a ratio of 0.16,
so a 30° swing turns the factory almost 5°. With attitude control on, the
CMGs must absorb angular momentum of about m r² ω, here 20,000 × 400 ×
0.01 = 80,000 N·m·s for a swing at 0.01 rad/s, which exceeds even the
ISS's CMG set [M]. Four arms moving at once add or cancel.

**Design choices this suggests:** move arms in opposed pairs so reactions
cancel; keep loads close to the factory's center of mass; and slow down
when the momentum budget is spent. All three are checkable in simulation.

## Flight precedents

| System | What it did | Mark |
| --- | --- | --- |
| Shuttle Remote Manipulator System (Canadarm) | 15 m arm on the orbiter; deployed and retrieved satellites, Hubble servicing, ISS assembly | [M] |
| **Canadarm2** (Space Station Remote Manipulator System) | 17 m, seven joints (shoulder 3, elbow 1, wrist 3); latching end effectors at both ends; walks end over end between grapple fixtures; berths cargo vehicles; built much of the ISS | [V] [CSA](https://asc-csa.gc.ca/eng/iss/canadarm2/nuts_bolts.asp) for length and joints; mass is 1,497 kg or 1,641 kg in different CSA pages [V, conflicting]; a 116,000 kg payload rating appears only in secondary sources [M] |
| Mobile Base System | Carries Canadarm2 along the ISS truss rail | [M] |
| **Dextre** (Special Purpose Dexterous Manipulator) | Two 3.51 m arms, 15 degrees of freedom, 1,710 kg; swaps orbital replacement units on the ISS | [V] [MDA](https://mda.space/canadarm-canadarm2-dextre) |
| Japanese Experiment Module arm, European Robotic Arm | Further ISS arms for the Japanese module and the Russian segment | [M] |
| ETS-VII (Japan, 1997) | First free-flying robot satellite experiments: an arm on a satellite capturing a target, base reaction studies | [M] |
| Orbital Express (DARPA, 2007) | Autonomous capture, part (ORU) transfer, and propellant transfer between two satellites | [M] |
| **OSAM-1** (NASA, formerly Restore-L) | Planned refueling of Landsat 7 and, with its SPIDER payload, a 5 m arm assembling seven elements into a 3 m Ka-band antenna; **cancelled March 2024** after cost and schedule problems | [V] news and NASA pages found while writing these notes |
| OSAM-2 (Archinaut One) | Planned in-space manufacturing of a solar array beam; cancelled [M] | [M] |

No mission has yet robotically assembled a large structure in orbit from
many parts. The ISS was assembled by crews and arms working together, with
many spacewalks [M]. The Lagrange 2 factory is beyond the state of the art,
which is fine for a demo as long as each step obeys physics.

## Assembly sequencing as discrete planning

The arms follow an **assembly plan**: which part, which arm, which port, in
what order. This is a discrete planning problem layered over continuous
motion.

**State:** the set of installed parts, each part's location (depot, in an
arm, installed), and each arm's state (idle, carrying, moving).

**Actions:** `pick(arm, part)`, `transfer(arm, part, waypoint)`,
`handoff(arm_a, arm_b, part)`, `place(arm, part, port)`, `latch(port)`.

**Preconditions,** each a test we can check:

- **Precedence:** a part can be installed only after the parts it attaches
  to, and only if installing it does not block a later part's insertion
  path. Precedence is a directed acyclic graph derived from the spacecraft
  document's ports.
- **Reach:** inverse kinematics finds a solution from the arm's base within
  its joint limits.
- **Clearance:** a collision-free path exists for the arm and its load.
- **Momentum and attitude:** the move's reaction stays within the factory's
  control authority.
- **Stability of the partial structure:** the partial ship's center of mass
  and stiffness stay acceptable; an unbraced cantilever may ring.
- **Resources:** an arm does one thing at a time; a port takes one part.

**Methods [M]:**

- AND/OR graphs enumerate all feasible assembly sequences of a product
  (Homem de Mello and Sanderson).
- *Assembly by disassembly:* plan the reverse, removing parts from the
  finished ship while each removal stays feasible, then reverse the result.
- Classical task planners (PDDL-style) for the symbolic layer, with motion
  checks as preconditions (task and motion planning).
- Multi-arm allocation as scheduling: assign moves to arms to minimize total
  time without conflicts, by greedy rules first and mixed-integer
  optimization if needed.

**For us:** the plan should be a typed, versioned document like the
spacecraft document, generated by a deterministic planner from the ship's
port graph, and replayable bit for bit. Each failure the issue lists
(collision, motor limit, drift off the halo) is a precondition or a runtime
check that fails visibly with its name.

## Contact and placement

- Final insertion uses compliance: impedance or admittance control lets the
  part slide into a guide rather than jam [M].
- Berthing interfaces have guide petals or alignment pins, so a coarse
  placement becomes a precise one as the part seats [M].
- Capture and latch impacts are internal impulses: momentum is conserved,
  energy is lost. Lagrange 1 tests both for the astronaut; the arms should
  pass the same tests.

## What Robot Lab gives us

Robot Lab imports the SO-101 arm, a small hobby arm [M: five joints plus a
gripper], with servo joints checked against MuJoCo (see
[`docs/lab/`](../lab/README.md) and `crates/robot-sim`). The factory arms are
scaled-up industrial arms built from the same joint and motor models. When
scaling, keep the dimensionless behavior: joint servo bandwidth well below
the structure's first mode, torque limits sized from the loads above, and
speed limits from the momentum budget.
