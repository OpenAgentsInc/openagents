# 114. Compile the backward pass

> **Status: unimplemented — design draft.** Nothing here runs yet.

Chapter 106 differentiates a tensor program in reverse mode, one operation at
a time. This chapter differentiates the IR itself and compiles the result.
The compiler plans which activations to save, chooses what to recompute,
fuses backward operations and compiles the optimizer update. Framework track.

## What this chapter covers

- A backward graph produced from chapter 38's tensor IR, using the per-operator
  vector-Jacobian rules of chapter 106.
- A saved-activation plan: which forward values the backward graph reads, and
  their lifetimes.
- Recomputation choices: recompute a cheap value instead of saving it, under
  a stated memory goal.
- Fusion of backward operations, admitted only when their buffers do not
  overlap.
- The optimizer update compiled into the same program.
- Bounded symbolic shapes and specialization from chapter 39.

## Dependencies

- [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md): the
  tensor IR and its memory plan.
- [Chapter 39](ch039-compile-a-general-tensor-program.md): symbolic shapes
  and guarded specialization.
- [Chapter 106](ch106-differentiate-any-tensor-program.md): reverse-mode
  rules.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): the optimizer update
  to compile.

## Done means

- Eager and compiled training agree on loss, gradients and parameter updates
  over several steps, within a tolerance stated before the comparison.
- Peak memory and step time are measured for eager and compiled training on
  the same model and host.
- A recomputation choice changes memory and time, never results beyond the
  stated profile.
- Chapter 131 reuses this compiler for whole-step training.

## Sources

- [Chapter plan](PLAN.md)
