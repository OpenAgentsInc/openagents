# 42. Device graphs and zero-copy buffers

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R23 (spec stage 11).

An iterative solver launches the same few kernels many times per step. R23
lets a program capture that launch sequence once and replay it, and lets
buffers cross library boundaries without copies when ownership allows.
Psionic already replays CUDA graphs and leases DLPack views through its own
library code. This chapter covers the language forms that make those checked
programs instead of library conventions. [Chapter 17](ch17-gpu.md) and
[chapter 20](ch20-interop.md) introduce the plan.

## What this chapter will cover

- Capturing a bounded launch sequence with fixed, admitted bindings.
- Bounded dynamic scalars, such as an iteration count or a tolerance.
- Device-side loops where a target allows them, and the host-driven
  fallback.
- Importing another component's buffer as a view, and refusing use after
  release.
- Transfers as explicit, counted effects.

## Outline

1. **Many small launches.** Show the launch pattern of one contact solver
   iteration and its measured launch overhead.
2. **Capture.** A device program holds the launches, immutable bindings and
   bounded scalars. Uncompiled sketch:

   ```jai
   // Sketch only: no syntax is accepted yet.
   solve :: #device_graph (iterations: bounded(64), tolerance: f32) {
       for 0..iterations-1 { contact_pass(); if residual() < tolerance break; }
   }
   ```

3. **Device loops and the fallback.** Where the target runs a device-side
   exit test, the loop stays on the device. Where it can't, the host iterates.
   Results are identical, and the report says which path ran.
4. **Imported buffers.** A view states layout, placement, lifetime and
   synchronization. A use after the exporter releases the buffer is refused.
5. **Transfers are effects.** Every host–device copy is an R11 effect that
   `coax explain` counts. A step that reads state back without saying so
   fails its transfer budget.
6. **Psionic's first consumers.** CUDA graph replay and leased DLPack views
   move from library effects to the language form.

## The consumer and the gate

The consumers are Aethyr's contact solver
([Aethyr chapter 22](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch022-contact-as-an-optimization.md))
and a policy reading observations
([Aethyr chapter 34](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch034-pictures-for-policies.md)).
The gate:

- The solver's iterations replayed as one device program equal the unrolled
  launches bit for bit.
- A policy reads simulated observations through a shared buffer with zero
  recorded copies.
- A use after the exporter released the buffer is refused.

## Done when

- R23 has shipped with the gate above.
- The chapter's examples run under `check-examples.sh`, including the
  refused use-after-release fixture with its exit status.
- Chapters 17 and 20 link here and state what shipped.

## Dependencies

- R3 ownership ([chapter 6](ch06-memory.md),
  [chapter 15](ch15-asynchronous-work.md)), R9 GPU interfaces
  ([chapter 17](ch17-gpu.md)), R11 effects and R15 strict profiles
  ([chapter 32](ch32-strict-math-and-integrators.md)).
- [Chapter 40](ch40-logical-tensors.md) for placement.
