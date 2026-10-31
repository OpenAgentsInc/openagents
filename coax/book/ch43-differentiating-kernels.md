# 43. Differentiating kernels

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R24 (spec stage 11).

Psionic differentiates its tensor programs. R24 extends that to admitted
Coax kernels, so a physics step can supply gradients to a learning loop. The
compiler generates forward mode (tangents) and reverse mode (adjoints) over
the GPU and strict-math subset. You choose saved values, recomputation and
checkpointing, and the compiler reports their memory. A kernel that has no
gradient says so, and a program that needs one from it is refused when it
compiles.

## What this chapter will cover

- Forward and reverse mode over an admitted kernel.
- Saved values, recomputation and checkpointing, with reported memory.
- Custom adjoints, such as one that reuses a forward factorization.
- Gradient modes `exact`, `custom` and `none`, enforced at compile time.
- Checking derivatives against finite differences.

## Outline

1. **A small kernel and its tangent.** One integration step, differentiated
   in forward mode, checked against central differences.
2. **Reverse mode.** The adjoint of the same step, and an adjoint-consistency
   check (the dot-product test between tangent and adjoint).
3. **What to keep.** Saved values against recomputation, and checkpointing
   over many steps. The report gives the memory each choice uses.
4. **A custom adjoint.** A constraint solve that reuses its forward
   factorization to solve the transposed system. The compiler checks the
   signature; the derivative test checks the values. Uncompiled sketch:

   ```jai
   // Sketch only: no syntax is accepted yet.
   solve_contacts :: (s: *Step) #gradient custom(solve_contacts_adjoint) { ... }
   ```

5. **Gradient modes.** Each kernel declares `exact`, `custom` or `none`. A
   differentiable program that reaches a `none` kernel is refused, with the
   path to it. It is never given a zero gradient.
6. **Gradients in a simulation.** How Aethyr's per-solver gradient modes
   ([Aethyr chapter 32](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch032-differentiate-a-simulation.md))
   map to these declarations.

## The consumer and the gate

The consumer is the Aethyr engine's differentiable rigid step and one
particle solver. The gate:

- Both pass finite-difference and adjoint-consistency checks on fixed cases.
- The constraint solve's custom adjoint matches the generated one.
- A program that differentiates through a kernel marked `none` is refused
  with its path.

## Done when

- R24 has shipped with the gate above.
- The chapter's examples, including the refused `none` fixture, run under
  `check-examples.sh` with exact output and exit status.
- Chapter 17's planned section links here and states what shipped.

## Dependencies

- R4 generics ([chapter 8](ch08-generics.md)), R6 typed code generation, R15
  strict math ([chapter 32](ch32-strict-math-and-integrators.md)) and R21
  layouts ([chapter 40](ch40-logical-tensors.md)).
- [Chapter 17](ch17-gpu.md) for the GPU subset.
