# 130. Profile a training step

> **Status: unimplemented — design draft.** Nothing here runs yet.

You cannot speed up a training step until you know where its time goes.
This chapter measures one step at operator level on the CPU and the GPU,
finds one bottleneck, fixes it and measures the change. Framework track.

## What this chapter covers

- Operator-level timing on the CPU and the GPU.
- Kernel launch counts and launch overhead.
- GPU idle time.
- Host-to-device and device-to-host transfers.
- Allocation lifetimes and peak activation memory.
- Backward cost compared with forward cost.
- Compile cost, reported apart from steady-state step time.
- One bottleneck found, fixed and measured.

## Dependencies

- [Chapter 129](ch129-train-a-whole-model-on-one-gpu.md): the GPU training
  step to profile.
- [Chapter 18](ch018-time-a-decision.md): monotonic phase timing.
- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): roofline
  measurements of the host.

## Done means

- A profile of one step accounts for its wall-clock time by operator,
  transfer and idle time, on a named host and build.
- The chosen bottleneck is named with its share of the step.
- After the fix, the step time is measured again on the same workload, with
  at least five repetitions, and results are unchanged within the stated
  profile.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
