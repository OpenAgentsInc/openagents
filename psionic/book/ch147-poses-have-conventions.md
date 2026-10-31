# 147. Poses have conventions

> **Status: unimplemented — design draft.** Nothing here runs yet.

A pose is a set of joint rotations on a skeleton plus a root position. Two
motion files can describe the same movement with different rotation formats,
axes, units and frame rates. This chapter makes those conventions explicit
data, so motion from a model reaches a Verse character without silent
conversion errors.

## What this chapter covers

- Rotation representations: quaternions, rotation matrices, axis-angle and
  the continuous 6D form, with exact or bounded-error conversions between
  them.
- Skeleton hierarchy: parent indices, rest offsets and forward kinematics
  from local rotations to world positions.
- Root coordinates: which axis is up, which is forward, and whether the root
  stores absolute positions or per-frame deltas.
- Units and timing: meters or centimeters, frames per second, and resampling
  between frame rates.
- Retargeting fixtures: the same motion mapped onto two skeletons with
  different bone lengths.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): matrix products.
- Coax R12 scalar quantities and planned frame types
  ([Coax roadmap](../../coax/ROADMAP.md)), adopted as they ship.

## Done means

- Every rotation conversion round-trips within a stated tolerance on a fixed
  set of rotations, including near-singular cases.
- Forward kinematics matches an independent implementation on fixed poses.
- Resampling and retargeting fixtures match hand-computed results.
- A pose with an undeclared convention is refused.

## Sources

- [Chapter plan](PLAN.md), Part XIV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), MotionBricks entry.
