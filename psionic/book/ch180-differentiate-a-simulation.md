# 180. Differentiate a simulation

> **Status: unimplemented — design draft.** Psionic already differentiates
> an oscillator through its tape ([chapter 109](ch109-learn-a-physical-system.md)
> and [chapter 111](ch111-recover-the-physics-from-the-motion.md)); this chapter plans
> gradients through the engine's solvers. Nothing here runs yet.

With gradients of a simulation, a controller can be optimized by gradient
descent, material parameters fitted to video, and a policy trained on
analytic gradients instead of only sampled returns. But contact makes the
dynamics non-smooth, implicit solvers hide their work in an iteration, and
storing every step's state for the backward pass costs memory linear in
horizon. Which solvers may be differentiated, and how, has to be a stated
property, not a hope.

## What this chapter teaches

- Reverse mode over a time-stepped simulation, and checkpointing.
- The adjoint of an implicit step: differentiate the optimality condition
  rather than the iterations.
- Gradients through contact: what they mean at a switch, and smoothing.
- Gradient modes per solver: `exact`, `custom`, `none`, enforced at build.

## The math

For a loss `L = Σ_k ℓ(x_k)` over `x_{k+1} = Φ(x_k, θ)`, the adjoint
recursion is

```jai
λ_K = ∂ℓ/∂x_K;   λ_k = ∂ℓ/∂x_k + (∂Φ/∂x_k)ᵀ λ_{k+1};   dL/dθ = Σ_k (∂Φ/∂θ)ᵀ λ_{k+1}
```

For an implicit step defined by `g(x_{k+1}, x_k, θ) = 0` (the KKT system of
chapter 170, the Newton equation of chapter 175), the implicit function
theorem gives the vector–Jacobian product with one linear solve
`(∂g/∂x_{k+1})ᵀ μ = λ_{k+1}`, which can reuse the forward Cholesky factor
([Amos and Kolter, OptNet, 2017](https://arxiv.org/abs/1703.00443);
[de Avila Belbute-Peres et al., 2018](https://proceedings.neurips.cc/paper/2018/hash/842424a1d0595b76ec4fa03c46e8d755-Abstract.html)).
Checkpointing every `√K` steps trades recomputation for memory; with a
deterministic forward pass, recomputation is exact, which is what makes it
safe ([Griewank and Walther, *Revolve*, 2000](https://doi.org/10.1145/347837.347846)).

## The design in Psionic

Each solver kernel declares its gradient mode (Coax R24). Explicit rigid,
MPM and explicit FEM get `exact` reverse kernels; the rigid constraint
solver gets a `custom` adjoint that reuses the Cholesky factor; SPH, PBD,
implicit FEM, SAP and IPC start as `none`, and a scene requesting gradients
through them is refused at build. Sleeping, islands and adaptive steps are
refused under gradients because they make the computation depend on
discrete decisions the backward pass cannot see. The tape of chapter 109
records steps; checkpoint policy is a profile field.

## What it builds

- Reverse kernels for rigid, articulation, contact (custom adjoint), MPM
  and explicit FEM; checkpointed backward driver.
- Examples: fit a ball's restitution to a recorded bounce; optimize an
  MPM soft body's actuation to move it forward.

## Acceptance checks and oracles

- Every gradient against central finite differences on smooth cases, at
  stated tolerances, per solver.
- Custom adjoint against the reverse pass of the unrolled Newton iterations
  on small problems.
- Checkpointed and fully stored backward passes are bitwise identical.
- Refusals: gradients through every `none` solver are refused with their
  named reason; mutation of the admission table leaves no survivors.

## How Genesis does it

Genesis's differentiability is selective: MPM and explicit FEM support
reverse mode, rigid is restricted, implicit FEM is rejected, PBD and SPH are
stubs, SAP and IPC unsupported; its rigid backward reuses the forward
factorization
([differentiable simulation](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/differentiable_simulation.html)).
Its kernels are differentiated by Quadrants. Our table starts the same; the
difference is that it is enforced when the program compiles (R24) and the
scene builds, and that deterministic recomputation lets checkpointing be
exact.

## Dependencies

Chapters 106, 107, 109 and 111 (tapes, transforms, adjoints), 166 to 175. From Coax: R24 kernel
derivatives, R15 strict profiles.
