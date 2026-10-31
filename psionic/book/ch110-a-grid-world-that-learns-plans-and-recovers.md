# 110. A grid world that learns, plans and recovers

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter builds a small headless navigation world and two ways to act in
it. One learns a policy from an exact planner's demonstrations. The other
plans with a learned model of the world. You see where each fails, including
an optimizer that exploits the learned model's errors.

## What this chapter covers

- A headless grid or 2D navigation scene with obstacles, goals and
  resettable state.
- An exact planner that supplies demonstrations.
- A small policy trained from those demonstrations, then trained further with
  DAgger: the planner labels the states the policy actually visits.
- A second route: plan with chapter 109's learned dynamics model, execute one
  action, then replan.
- A planted case where the planner exploits the learned model's errors, and
  the measurement that shows it.
- Evaluation on whole episodes over held-out maps and parameters:
  completion, collisions, recovery after a perturbation and computation per
  decision.
- Tiny cases checked against exhaustive search.

## Dependencies

- [Chapter 103](ch103-learning-with-arrays.md) and
  [chapter 104](ch104-gradients-as-a-program.md): the trainer and gradients.
- [Chapter 109](ch109-learn-a-physical-system.md): the learned model used for
  planning.
- A Verse character that navigates and recovers follows from this chapter. It
  needs its own consumer contract.

## Done means

- On tiny maps, the exact planner matches exhaustive search.
- Behavior cloning and DAgger are compared on held-out maps at the same
  number of planner labels, with completion, collisions and recovery
  reported.
- Planning with the learned model is compared with planning with the true
  model, and the model-exploitation case is shown and measured.
- Every episode replays from its seed and map.

## Sources

- [Chapter plan](PLAN.md)
