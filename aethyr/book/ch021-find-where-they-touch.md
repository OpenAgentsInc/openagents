# 21. Find where they touch

> **Status: unimplemented — design draft.** [`verse/contact.coax`](../../verse/contact.coax)
> already finds sphere–sphere, sphere–box, box–plane and sphere–triangle
> contacts in f64, with swept sphere tests. This chapter plans the general
> narrow phase: any convex pair, meshes, signed distance fields and stable
> multi-point manifolds. Nothing here runs yet.

The broad phase says two bodies might touch. The narrow phase says whether
they do, and if so where: a contact normal, a penetration depth and one or
more contact points. The contact solver needs those to be accurate, but also
*stable*: a box resting on a table should report the same four corners step
after step, not points that jitter between edges, or the box will rock.

## What this chapter teaches

- Support functions and the Minkowski difference, the idea behind every
  general convex test.
- GJK for distance and intersection, EPA for penetration depth, and
  Minkowski portal refinement (MPR) as a cheaper alternative.
- Closed-form tests for primitive pairs, and the separating axis theorem for
  boxes.
- Signed distance fields for non-convex shapes and terrain.
- Contact manifolds: several points per pair, by clipping or by perturbed
  queries, and how to keep them coherent over time.

## The geometry

For convex shapes `A` and `B`, they intersect exactly when the origin lies in
`A ⊖ B = {a − b}`. Neither set is ever built; a *support function* returns
the farthest point in a direction:

```jai
s_{A⊖B}(d) = s_A(d) − s_B(−d)
dist(A, B) = min_{z ∈ A⊖B} |z|                  // GJK, by simplex descent
depth(A, B) = min_{n} max{t : t n ∈ A⊖B}        // EPA, by polytope expansion
```

GJK ([Gilbert, Johnson and Keerthi, 1988](https://doi.org/10.1109/56.2083))
converges in a few iterations for separated shapes; EPA
([van den Bergen, 2001](https://www.dtecta.com/papers/gdc2001depth.pdf))
expands the final simplex to find the minimum-translation normal when they
overlap. MPR ([Snethen, *Game Programming Gems 7*, 2008](https://github.com/erwincoumans/xenocollide))
finds a normal and depth along a portal ray and is cheaper but not always the
minimum. Signed distance fields `φ(x)` (negative inside) give depth and
normal `∇φ` at any point for arbitrary shapes, at the cost of resolution.

## The design in Aethyr

**A dispatch table by shape pair.** Sphere, capsule, box, plane, convex hull,
triangle mesh, heightfield and SDF. Pairs with closed forms use them
(sphere–sphere, sphere–capsule, capsule–capsule, box–box by the separating
axis theorem with face clipping); other convex pairs use GJK with EPA; MPR is
a profile option measured against them. Non-convex meshes are either
decomposed into convex pieces at import (chapter 35) or tested point-wise
against a baked SDF.

**Manifolds.** Box–box and convex face contacts clip the incident face
against the reference face (Sutherland–Hodgman) and keep up to four points.
For curved convex pairs a single query gives one point; a manifold is built
by rotating the pair slightly about the normal and repeating the query, then
reducing to four points that span the largest area. Points carry feature IDs
(which face or edge produced them) so the solver can warm-start them across
steps (chapter 22).

**Determinism and symmetry.** Tests are written so that swapping the two
shapes negates the normal and leaves depth and points unchanged; ties in
feature selection break by lowest feature ID. Every test is one routine used
by the CPU and generated for devices, with the CPU f64 run as the reference.

**Continuous collision.** Aethyr's swept sphere tests stay; a conservative
advancement time-of-impact for general convex pairs is added for small fast
bodies, and is required by IPC in chapter 30.

## What it builds

- `aethyr/sim/narrow.coax`: support functions, GJK, EPA, MPR, closed
  forms, SAT and clipping, SDF queries, manifold reduction and
  time-of-impact.
- A fixture set of random convex polytopes and adversarial cases (parallel
  faces, edge–edge, touching, deep overlap).

## Acceptance checks and oracles

- Primitive pairs against their closed forms at random poses.
- GJK distance on random polytopes against brute-force vertex/face
  distances; EPA depth against a brute-force minimum over face normals of
  the explicitly built Minkowski difference (small cases only).
- Agreement with `verse/contact.coax` on its shapes.
- Symmetry: every test with arguments swapped.
- Manifold stability: a box at rest on a plane reports the same four feature
  IDs for 10,000 steps.
- SDF: sign and distance at sampled points against the analytic shape;
  error reported as a function of grid resolution.
- Time of impact: a 200 m/s sphere against a thin plate is caught.

## How Genesis does it

Genesis uses MPR for the usual convex path, GJK/EPA for refinement and
differentiable operation, specialized primitive and terrain routines,
signed-distance-field paths for non-convex geometry, and multiple contacts
generated commonly by perturbed collision queries
([collision detection](https://genesis-world.readthedocs.io/en/latest/user_guide/theory/rigid_solver/collision_detection.html)).
Its 1.4.5 release improved collision performance for complicated geometry
([release](https://github.com/Genesis-Embodied-AI/genesis-world/releases/tag/v1.4.5)).
We build the same toolbox. We differ by making symmetry and feature-ID
stability checked properties, and by keeping MPR as a measured option rather
than the default until it is shown to give the same contacts.

## Dependencies

Chapter 20's candidate pairs and chapter 18's poses. The first profile uses
directly authored primitives, convex hulls and SDF fixtures, so the contact
calculation can be checked before building an asset pipeline.

Chapter 35 later supplies convex decomposition and SDF baking; that asset
profile still needs import-to-contact checks. Chapter 32 consumes GJK's
differentiable form; the backward pass does not precede the forward contact
calculation. These integrations remain part of the full toolbox described
here. From Coax: R12 frames, R15 strict math.
