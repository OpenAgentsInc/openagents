# 27. Elastic solids on a mesh

> **Status: unimplemented — design draft.** Nothing here runs yet.

A rubber gripper pad, a soft toy, a muscle: these deform smoothly and spring
back, and their stress matters, not just their shape. The finite element
method (FEM) splits the solid into tetrahedra, gives each an elastic energy,
and moves the vertices to balance that energy against inertia and contact.
It is more accurate per degree of freedom than particles for solids, and
the place where explicit and implicit time stepping differ most sharply.

## What this chapter teaches

- Linear tetrahedral elements, the deformation gradient per element and the
  rest-shape matrix.
- Hyperelastic energies: StVK, corotated and stable neo-Hookean.
- Explicit (symplectic Euler) and implicit (backward Euler by Newton)
  stepping, and when each is worth it.
- Assembling the stiffness matrix and solving with preconditioned CG.
- Shells and thin structures, briefly; IPC's version is chapter 30's.

## The physics

For element `e` with rest edge matrix `D_m` and current edge matrix `D_s`,
`F_e = D_s D_m⁻¹`. Total potential energy and the implicit step are

```jai
E(x)      = Σ_e V_e Ψ(F_e(x))
Ψ_snh(F)  = μ/2 (I_C − 3) + λ/2 (J − α)² − μ/2 log(I_C + 1)   // stable neo-Hookean
explicit:  v += Δt M⁻¹(f_ext − ∇E(x));  x += Δt v
implicit:  x' = argmin ½ |x − x̃|²_M / Δt² + E(x),   x̃ = x + Δt v + Δt² M⁻¹ f_ext
```

with `I_C = tr(FᵀF)`, `J = det F` and `α = 1 + μ/λ`
([Smith, de Goes and Kim, *Stable Neo-Hookean Flesh*, 2018](https://doi.org/10.1145/3180491);
[Sifakis and Barbič, FEM course notes, 2012](https://viterbi-web.usc.edu/~jbarbic/femdefo/)).
Explicit stepping needs `Δt` below the stiffest element's period; implicit
stepping is unconditionally stable but each step is a Newton solve with
projected (positive-definite) element Hessians.

## The design in Aethyr

Tetrahedral meshes come from chapter 35's tetrahedralization; rest
matrices and volumes are computed at build. Energies are routines of `F`
with analytic gradients and Hessians, each checked against its own finite
differences. The explicit profile is the first, and the one chapter 32
differentiates. The implicit profile uses Newton with line search and
Jacobi-preconditioned CG, with iteration counts and residuals recorded; its
gradients are refused until an adjoint of the Newton solve is built and
checked.

## What it builds

- `aethyr/sim/fem.coax`: element data, energies with gradients and
  projected Hessians, explicit and implicit steppers, CG, Dirichlet pins.
- Fixtures: cantilever beam, compressed cube, a pad pressed by a rigid
  finger.

## Acceptance checks and oracles

- Energy gradients and Hessians against finite differences; zero energy and
  stress at rest and under rigid rotation.
- Inversion robustness: a flattened and inverted element recovers under
  stable neo-Hookean.
- Small-deflection cantilever tip displacement against Euler–Bernoulli
  `δ = F L³ / (3 E I)` within a stated discretization error that shrinks
  under refinement.
- Explicit and implicit profiles agree at small `Δt`.
- Momentum conserved by internal forces exactly in sum.

## How Genesis does it

Genesis's FEM solver offers explicit and implicit stepping for volumetric
solids and its 1.4 line added IPC-backed shells and cloth
([FEM solver](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/fem_solver/index.html));
explicit FEM supports reverse-mode gradients and implicit FEM rejects them
([differentiable simulation](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/differentiable_simulation.html)).
Our plan has the same split, stated in the compatibility matrix.

## Dependencies

Chapter 17's scene arrays and Psionic chapter 4's sparse linear algebra.
A directly authored tetrahedral fixture suffices for the first explicit and
implicit forward calculations and their deformation checks.

Chapter 35 later provides imported/generated meshes, chapters 29–30 add
contact, and chapter 32 adds gradients. Each integration still needs its
declared acceptance evidence; none is needed to calculate the first
uncontacted tetrahedron. From Coax: R15, R21 and R23; R24 for gradients.
