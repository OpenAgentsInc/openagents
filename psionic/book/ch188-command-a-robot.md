# 188. Command a robot

> **Status: unimplemented — design draft.** Psionic's Panda loop
> ([chapter 191](ch191-close-the-simulated-control-loop.md)) already runs
> operational-space control against MuJoCo. This chapter plans the
> classical control and planning layer over our own engine: joint PD,
> inverse kinematics and collision-free motion planning. Nothing here runs
> yet.

Before a learned policy can be trusted, the plain tools must work: hold a
joint at an angle, put the gripper at a pose, move from here to there
without hitting the table. They are also what a learned policy outputs
into, since most policies produce joint targets or end-effector poses, not
torques.

## What this chapter teaches

- Joint-space PD control with gravity compensation, and choosing gains.
- Inverse kinematics by damped least squares, with joint limits and
  null-space objectives.
- Sampling-based motion planning with RRT-Connect, and path shortcutting.
- Trajectory timing within velocity and acceleration limits.

## The math

PD with gravity compensation, using chapter 167's dynamics:

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

## The design in Psionic

Controllers are Coax R14 bounded control steps: fixed rate, bounded
computation, saturating outputs with the saturation reported. IK is a
bounded iteration with a stated tolerance; failure to converge is a typed
result. The planner uses chapter 168 and 169's queries for collision checks
in a single-environment scene copy, seeded from the counter-based RNG so a
plan is reproducible; shortcutting and time parameterization follow.

## What it builds

- `psionic/lib/robotics/control.coax` (PD, IK), `plan.coax` (RRT-Connect,
  shortcutting, timing).

## Acceptance checks and oracles

- PD holds a pose under gravity with steady-state error below a stated bound;
  step response matches the second-order model of its gains on a single
  joint.
- IK reaches random reachable poses within tolerance; unreachable poses
  return a typed failure; joint limits are never exceeded.
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

Chapters 167 to 170, 183. From Coax: R14 bounded control, R12 frames.
