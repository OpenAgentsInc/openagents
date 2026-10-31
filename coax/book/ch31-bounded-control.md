# 31. Bounded control

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R14 (spec stage 10).

A control step must do a bounded amount of work with a bounded amount of
memory. Today you get that by convention: fixed arrays, no allocation, and
review. This chapter shows the planned `#bounded` contract, which makes the
compiler check those rules through the whole call graph and report a stack
bound per target.

`#bounded` establishes bounded work and bounded storage. It doesn't establish
a wall-clock deadline. Elapsed time also depends on the processor, its clock
speed, caches, interrupts, the operating system's scheduler and other work on
the same machine. A deadline such as "every control step finishes within
1 ms" needs separate evidence for each target, as described in
[What the checks don't establish](#what-the-checks-dont-establish).

## What this chapter will cover

- What `#bounded` declares and what it forbids.
- How the checker finds the path to a forbidden call.
- Loop bounds the checker can see.
- Scratch memory from a caller-owned region.
- Stack bounds per target, compared with measurement.
- Bounded solver building blocks.

## Outline

1. **The contract.** `#bounded` declares a maximum stack and a named scratch
   region of fixed size.
2. **What it forbids.** Allocation, blocking, recursion, unbounded loops and
   calls with unknown effects, anywhere in the call graph. That includes
   generic instantiations, callbacks, cleanup and error paths.
3. **Diagnostics.** The checker derives facts from R11 effect summaries and
   R3 storage rules. A violation names the path to the offending call.
4. **Loops.** Each loop needs a bound the checker can see, such as a fixed
   array length or a constant iteration budget.
5. **Scratch.** Scratch comes from a caller-owned region
   ([chapter 6](ch06-memory.md)). Nothing escapes the call.
6. **Stack bounds.** The compiler computes the stack bound per target from
   the lowered form. A bound it can't compute is a refusal, not an estimate.
   You compare it with native measurement.
7. **Solvers.** Library code on top of the contract: iteration budgets,
   residuals and convergence status returned as results.

An uncompiled sketch:

```jai
// Sketch only: not accepted by coax0.
control_step :: (s: *RobotState, out: *Command) #bounded(stack=4096, scratch=ctrl) {
    for i: 0..JOINTS-1 { out.torque[i] = pd(s, i); }
}
```

## The consumer and the gate

- The simulated robot's control step is `#bounded`; see
  [Psionic chapter 167](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch167-command-a-robot.md).
- One Verse per-frame kernel is `#bounded`.

Fixtures add allocation, blocking, recursion, an unbounded loop and an
unknown foreign call one at a time, directly and through a generic helper or
callback. Each must be refused with its path. The computed stack bound must
cover the measured native stack on each claimed target.

## What the checks don't establish

The checker proves that every path through a `#bounded` procedure has a
finite iteration count, a computed stack bound and no allocation or blocking
call. It doesn't measure time. To claim a deadline on a target, you need all
of the following for that target:

1. The exact build: compiler version, build profile and C compiler flags.
2. The hardware and operating system the claim covers, named exactly.
3. A measured worst-case execution time for the procedure under a stated
   load, with the method recorded: inputs that drive the longest path, the
   number of runs and the highest observed time.
4. The margin between that measured time and the deadline.

A deadline recorded on one target doesn't carry over to another target, a
different clock speed or a different build profile. Without that record, a
`#bounded` procedure has bounded work and storage, and its timing is unknown.

## Done when

- R14 ships with those fixtures and both consumers compile under
  `#bounded`.
- Stack bounds are reported per target and checked against measurement.
- Any deadline the consumers claim has its own per-target timing record, kept
  separate from the `#bounded` check.
- This chapter's examples, including refused ones with `.status` files, run
  under `check-examples.sh`.

## Dependencies

- R3 scratch regions and allocators ([chapter 6](ch06-memory.md)).
- R4 generics ([chapter 8](ch08-generics.md)) and R11 effect summaries.
- Used by [chapter 35](ch35-sensors-and-actuators.md)'s supervisor and by
  Aethyr [chapter 9](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch009-a-character-moves.md).
