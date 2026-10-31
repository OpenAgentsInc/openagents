# 150. Put generated motion in Verse

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter connects chapter 149's motion to a Verse character. You give a
character a destination, Psionic generates the motion, and Verse plays it on
the character's skeleton. It is the first major Verse machine-learning
demonstration in the plan.

## What this chapter covers

- An explicit skeleton and target interface between Psionic and Verse: the
  joint list, rotation convention, root convention, units and frame rate.
- Bounded generation: a maximum motion length and a deadline, with a typed
  refusal when either is exceeded.
- Animation handoff: generated poses converted to the Verse animation format
  and played on a character.
- A reproducible scene: the same destination and seed produce the same
  poses.
- The Unitree G1 robot is a separate embodiment and profile, not an
  interchangeable asset.

## Dependencies

- [Chapter 149](ch149-execute-motionbricks.md): MotionBricks execution.
- [Chapter 147](ch147-poses-have-conventions.md): pose conventions.
- The Aethyr engine's figures and animation
  ([Aethyr chapter 14](../../aethyr/book/ch014-figures-and-animation.md)).

## Done means

- A destination-to-pose scene reproduces the same pose sequence for the same
  seed.
- The handed-off animation matches the generated poses after conversion, on
  every joint, within a stated tolerance.
- A generation that misses its deadline leaves the character in a defined
  fallback state.

## Sources

- [Chapter plan](PLAN.md), Part XIV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md).
