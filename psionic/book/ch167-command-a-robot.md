# 167. Command a robot

This chapter's first runnable profile is **position-only Panda inverse
kinematics**: seven source-limited hinge coordinates place the fixed grip SITE
near a typed world target. Its owned geometry and independent numerical
reference need no simulator step or learned policy.

The separate [chapter 170](ch170-close-the-simulated-control-loop.md) already
runs operational-space control against MuJoCo. Joint PD and gravity-control
rollout, orientation/nullspace IK, collision-free planning, trajectory timing
and compiler-enforced bounded control remain the next layers of this chapter.

## A source-bound position IK query

We can now ask the owned Panda kinematics for seven radian coordinates that
place its grip SITE near a requested world position. This is the first runnable
slice of this chapter. It uses the exact twelve-body/seven-hinge source profile
and the already qualified grip Jacobian. The fingers do not change this fixed
SITE position; this query has no finger coordinate or orientation objective.

`pik_solve` takes seven typed radian seed coordinates, one
`Point3(PfkWorld, Metre, f64)` target, an integer update budget from zero through
96, and one caller-owned output slot. Seeds must already lie in the seven
inclusive joint ranges captured from the pinned compiled source model. The
public target box is ten metres per world coordinate. These are source and
numerical admissions, not collision or actuator permission.

The [implementation](../lib/robotics/panda-ik.coax) solves a three-dimensional
damped least-squares problem.
We normalize each Jacobian column by exactly one radian. Its coefficient then
has metres, so the damping coefficient is **0.05 metres** in this coordinate
basis. With `e = target - position`, the coefficient equation is:

```jai
A  = J * transpose(J) + (0.05 * 0.05) * identity3;
dq = transpose(J) * inverse(A) * e;
```

The small positive diagonal regularizes singular geometric configurations. The
implementation reuses the existing checked three-by-three Cholesky inverse
inside the fixed stride-seven controller matrix storage. Each raw `dq`
coefficient restores the same one-radian basis. We cap the largest absolute
joint increment at 0.1 radians by scaling the whole vector, then project each
new coordinate into its pinned source range. Projection can prevent progress;
there is no global reachability guarantee or adaptive line search.

The update budget counts actual proposed updates. An initially solved seed can
succeed with zero updates. A successful solution has Euclidean position error
at most **one micrometre**. Exhaustion returns `not_converged` and leaves the
entire caller output unchanged. It is a statement about this seed and fixed
iteration profile, not a proof that the target is unreachable.

`pik_step` exposes a separately admitted one-update candidate, its freshly
computed grip position, residual and number of projected coordinates. It can
publish useful diagnostic progress before convergence. The solver accumulates
those counts and publishes a result only after the residual criterion passes.
All non-OK paths preserve output and spare storage. The raw adapters name their
radian and world-metre coefficient boundaries explicitly.

The following example starts joint1 at its source upper range. Its target is
one centimetre farther along world -X than that configuration's independently
computed grip position. The independent reference predicts eleven updates,
eleven projected coordinates and a final residual of approximately
`4.6571171e-7` metres. Display values round to micrometres or microradians; the
library retains F64 values.

<!-- source: psionic/examples/panda-ik.coax pik-query -->
```jai
 // Typed radians and one explicit world/metre target. Joint1 starts exactly
 // at its source upper limit; the target is displaced 1cm along world -X.
 seed:=pik_home();seed[0].value=2.8973;
 target:[1]Point3(PfkWorld,pfku.Metre,f64);
 target[0].x= -1.2036558811324911;target[0].y=0.11059065150635836;target[0].z=1.0110438715851884;
 solution:[1]PikOutput;
 #assert pik_solve(seed[..],target[..],11,solution[..])==PikStatus.ok;
 #assert solution[0].error_metres<=1e-6;
 print("updates: %; projected coordinates: %\n",solution[0].updates,solution[0].projected_coordinates);
 p:=solution[0].position;
 print("grip xyz (um): % % %\n",pik_example_micro(p.x),pik_example_micro(p.y),pik_example_micro(p.z));
 print("residual below 1 um\n");
 q:=solution[0].angles;
 print("candidate joints (urad): % % % % % % %\n",pik_example_micro(q[0].value),pik_example_micro(q[1].value),pik_example_micro(q[2].value),pik_example_micro(q[3].value),pik_example_micro(q[4].value),pik_example_micro(q[5].value),pik_example_micro(q[6].value));
 // Exhaustion refuses publication; a previous candidate stays intact.
 before:=solution[0];
 #assert pik_solve(seed[..],target[..],0,solution[..])==PikStatus.not_converged;
 pik_same(solution[0],before);
 print("zero-update unsolved: not_converged; output unchanged\n");
```

```shell
coax/bin/coax run psionic/examples/panda-ik.coax
```

The frozen experiment contains 49 contexts: seven near-start targets from
retained freshly forwarded SDK grip samples; 24 reachable targets generated by
an independent authored homogeneous-matrix calculation; the home target;
fourteen exact source-range endpoints; and three explicit budget/nonconvergence
contexts. A separately frozen oracle profile adds the existing eleven-update,
eleven-projection contract case, preserving the original 49-case bytes and
budgets. The SDK samples bind both the target and its matching joint tuple;
they are not cached geometry paired with a later coordinate state. No SDK,
rendering, integration or neural inference runs are needed for these checks.

The independent tooling computes analytic matrix-product tangents and solves
its own pivoted Gaussian three-by-three system. The product's quaternion/cross
Jacobian path and Cholesky inverse are not used to generate reference
trajectories. Coordinate, freshly computed position and residual comparisons
use the frozen **2e-10 absolute, zero-relative** coefficient budgets; update and
projection counts must agree exactly. The native record retains every
trajectory, all refusals and all outcomes, including exhaustion.

The new authority roots have **102/102** and **152/152** actually emitted
mutation sites killed, with zero survivors. Twelve focused source groups cover
38 cases. Four meaningful compile refusals distinguish metre coordinates from
radians, HalfMetre targets from Metre targets, nominal world frames, and radian
outputs from lengths. The example matches its independently prewritten golden
in debug and release. The independent reference and native proof cover 50 contexts: **47 converge
and three exhaust their declared budgets**. Debug, release and fail-fast
ASan/UBSan each pass **15,507 assertions**. All **363 events**, including **228
trajectory updates** and **35 standalone steps**, agree on exact statuses and
update/projection counters. Maximum errors are `1.22e-14` radians, `4.22e-15`
metres in position and `9.00e-16` metres in residual, inside the unchanged
`2e-10` budgets. The three complete owned output streams are byte-identical.
Fourteen source-limit and six target-box boundaries, 50 IEEE poisons, eight
overlap cases, complete input/output snapshots and ten late actual/expected
reader poisons pass. The record is
[`native-qualification.json`](../fixtures/robotics/panda-ik/native-qualification.json).

The numerical qualification uses the named Darwin arm64 scalar-libm F64
profile, with contraction and fast math disabled. It carries the existing FK
and Jacobian evidence without repeating their passing suites. Cross-target
bit identity remains a separate determinism requirement. This chapter still
needs orientation and nullspace IK, PD and gravity-control rollout, collision
queries, RRT-Connect, trajectory timing and compiler-enforced bounded control.
This position candidate is useful before any of those claims are made.

## The remaining control and planning layers

The following sections are design work; the position-only profile above does
not qualify these tools.


- Joint-space PD control with gravity compensation, and choosing gains.
- Inverse kinematics by damped least squares, with joint limits and
  null-space objectives.
- Sampling-based motion planning with RRT-Connect, and path shortcutting.
- Trajectory timing within velocity and acceleration limits.

## Planned control and planning math

PD with gravity compensation, using Aethyr chapter 19's dynamics:

```jai
τ = K_p (q_ref − q) + K_d (q̇_ref − q̇) + g(q)
```

Damped least squares IK for an end-effector error `e` with Jacobian `J`:

```jai
Δq = Jᵀ (J Jᵀ + λ² I)⁻¹ e + (I − J⁺ J) Δq_null
```

([Buss, *Introduction to IK with Jacobian Transpose, Pseudoinverse and
Damped Least Squares*, 2004](https://www.math.ucsd.edu/~sbuss/ResearchWeb/ikmethods/iksurvey.pdf)).
RRT-Connect grows trees from start and goal and tries to connect them,
checking each edge for collision at a stated resolution
([Kuffner and LaValle, ICRA 2000](https://doi.org/10.1109/ROBOT.2000.844730)).

## Planned integration in Psionic

The remaining control design calls for compiler-enforced Coax R14 bounded
steps: fixed rate, bounded computation, saturating outputs with reported
saturation. The position query above has an explicit iteration cap and typed
nonconvergence, while compiler enforcement remains pending. The planner uses Aethyr chapter 20 and 21's queries for collision checks
in a single-environment scene copy, seeded from the counter-based RNG so a
plan is reproducible; shortcutting and time parameterization follow.

## Planned additional modules

- `psionic/lib/robotics/control.coax` (PD and broader IK), `plan.coax` (RRT-Connect,
  shortcutting, timing).

## Remaining acceptance checks and oracles

- PD holds a pose under gravity with steady-state error below a stated bound;
  step response matches the second-order model of its gains on a single
  joint.
- Broader IK reaches held-out position/orientation targets within declared
  tolerances; exhausted iterations return typed nonconvergence, preserving
  the distinction between local failure and global unreachability. Joint
  ranges remain enforced.
- Planned paths are collision-free when re-checked at 10× resolution;
  the same seed gives the same plan.
- Timed trajectories never exceed velocity and acceleration limits.

## How Genesis does it

Genesis provides PD control, inverse kinematics and RRT-Connect planning
through OMPL-style interfaces on its rigid entities
([control your robot](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/control_your_robot.html),
[IK and motion planning](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/inverse_kinematics_motion_planning.html)).
We plan the same three tools, written in Coax with bounded control steps
and reproducible planning.

## Dependencies

The runnable position profile depends on the owned fixed Panda FK and grip
Jacobian, the checked small matrix algebra, native F64 and the initial typed
quantity/point profile. The remaining control/planning design depends on
Aethyr chapters 19 to 22 and 35, full frame transforms and compiler R14
bounded control. Those requirements are not waived by fixed arrays.
