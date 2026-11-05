# 21. Find where they touch

> **Status: partly implemented.** [`aethyr/sim/narrow.coax`](../../aethyr/sim/narrow.coax)
> (1,130 mutants: 717 killed, 413 still surviving)
> finds where spheres, capsules, boxes and convex hulls touch, on the CPU
> in f64: support functions, GJK distance, EPA depth, closed forms for
> spheres and capsules, boxes by the separating axis theorem with clipped
> four-point manifolds and feature IDs, and time of impact by conservative
> advancement. Its checks compare GJK and EPA with brute-force box
> distances and depths, agree with [`aethyr/contact.coax`](../../aethyr/contact.coax),
> and check symmetry and manifold stability. Signed distance fields, MPR,
> triangle meshes, heightfields, perturbed-query manifolds for curved
> shapes and device kernels are not built yet.

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

## What runs today

[`aethyr/sim/narrow.coax`](../../aethyr/sim/narrow.coax) describes a shape
as a `Shape`: a kind, a pose and the kind's sizes. A sphere is a point core
with a radius, a capsule a segment core with a radius, and boxes and hulls
of up to 32 points have radius 0. `narrow_support` returns the farthest
point of a shape along a direction; ties go to the positive side and the
lowest index, so the answer depends on the direction alone.

`narrow_pair` takes two shapes and returns a `Manifold`: up to four points
that share one normal, each with a signed distance (negative is overlap)
and a feature ID. The normal points from the second shape toward the first,
the direction that pushes the first out, as in `aethyr/contact.coax`. Each
pair takes one of three paths:

- Two spheres or capsules take the closed form: the nearest points of the
  two core segments, less the radii.
- Two boxes take the separating axis theorem over their 15 axes. A face
  axis is kept unless an edge pair overlaps 5% less, and among faces within
  2% of the least overlap the largest face is the reference, so a box on
  the ground keeps the ground's face whatever the jitter. The incident face
  is clipped against the reference face's four sides (Sutherland–Hodgman),
  and the points behind the reference face are reduced to the deepest four
  that span the most area. A feature ID names the reference face, the
  incident face and the corner or clipped edge that made the point, never
  which box came first.
- Every other pair runs GJK on the cores. When the cores are apart, the
  radii come off the distance. When they meet, EPA expands GJK's last
  simplex into a polytope of the Minkowski difference until a new support
  point adds less than 1e-10 m, and the radii add to the depth.

GJK finds the point of its simplex nearest the origin by trying every
subset of the simplex, smallest first, and keeping the nearest projection
whose weights are all positive. With at most four points that is 15
subsets, and it never divides by a near-zero determinant: such a subset is
skipped. Before any path runs, the pair is put in a canonical order by
position and kind, so swapping the shapes negates the normal and leaves the
distances, points and features the same bits.

`narrow_time_of_impact` moves both shapes along their velocities by
conservative advancement: each step advances time by the current distance
over the closing speed along the normal, until the shapes are within a
tolerance or the step ends.

The example [`find-where-they-touch.coax`](../../aethyr/examples/find-where-they-touch.coax)
runs each path:

<!-- source: aethyr/examples/find-where-they-touch.coax example -->
```jai
r6 :: (x: f64) -> f64 { return floor(x * 1000000.0 + 0.5) / 1000000.0; }
show :: (name: string, m: Manifold) {
  print("%: % point(s), distance %, normal (%, %, %)\n", name, m.count, r6(m.distance[0]),
        r6(m.normal.x), r6(m.normal.y), r6(m.normal.z));
}
corners :: (s: Shape) -> Shape {
  pts: [8] D3;
  for k: 0..<8 { pts[k] = d3(ifx (k & 1) != 0 then s.half.x else -s.half.x, ifx (k & 2) != 0 then s.half.y else -s.half.y, ifx (k & 4) != 0 then s.half.z else -s.half.z); }
  return narrow_hull(s.pos, s.rot, pts[..], 8);
}

main :: () {
  turn := dq_unit(dq(0.1, 0.3, -0.2, 0.9));
  show("sphere / sphere, apart", narrow_pair(narrow_sphere(D3_ZERO, 0.5), narrow_sphere(d3(1.5, 0.0, 0.0), 0.5)));
  show("capsule / capsule, crossed", narrow_pair(narrow_capsule(D3_ZERO, DQ.{}, 1.0, 0.25), narrow_capsule(d3(0.3, 0.0, 0.0), dq_unit(dq(1.0, 0.0, 0.0, 1.0)), 1.0, 0.25)));
  show("sphere / box, by GJK", narrow_pair(narrow_sphere(d3(0.0, 1.2, 0.0), 0.5), narrow_box(D3_ZERO, turn, d3(1.0, 0.5, 0.5))));
  show("sphere in a box, by EPA", narrow_pair(narrow_sphere(d3(0.1, 0.2, 0.0), 0.5), narrow_box(D3_ZERO, DQ.{}, d3(1.0, 0.5, 0.5))));
  a := narrow_box(D3_ZERO, DQ.{}, d3(0.5, 0.5, 0.5)); b := narrow_box(d3(0.6, 0.7, 0.1), turn, d3(0.5, 0.4, 0.3));
  sat := narrow_pair(a, b); epa := narrow_pair(corners(a), corners(b));
  show("box / box, by SAT", sat);
  show("the same boxes as hulls, by EPA", epa);
  print("  SAT's deepest point % m, EPA's depth % m\n", r6(sat.distance[0]), r6(epa.distance[0]));

  // GJK on random pairs of boxes given as hulls, against the closed form.
  worst := 0.0; steps := 0; pairs := 0;
  for k: 0..<200 {
    p := test_box(11, 20 * k, 6.0); q := test_box(12, 20 * k, 6.0);
    if test_box_depth(p, q) >= 0.0 { continue; }
    g := narrow_gjk(corners(p), corners(q));
    worst = fmax(worst, abs(g.distance - test_box_distance(p, q))); steps += g.iterations; pairs += 1;
  }
  print("GJK on % separated box pairs: within 1e-9 m of the closed form: %, % iterations on average\n", pairs, ifx worst < 0.000000001 then "yes" else "no", r6(cast(f64) steps / cast(f64) pairs));

  // A box resting on the ground, then tipped onto an edge.
  ground := narrow_box(d3(0.0, -1.0, 0.0), DQ.{}, d3(5.0, 1.0, 5.0));
  rest := narrow_pair(narrow_box(d3(0.0, 0.499, 0.0), DQ.{}, d3(0.5, 0.5, 0.5)), ground);
  print("resting box: % points\n", rest.count);
  for k: 0..<rest.count { print("  (%, %, %) distance %, feature %\n", r6(rest.point[k].x), r6(rest.point[k].y), r6(rest.point[k].z), r6(rest.distance[k]), rest.feature[k]); }
  tip := narrow_pair(narrow_box(d3(0.0, 0.7, 0.0), dq_z(PI_D * 0.25), d3(0.5, 0.5, 0.5)), ground);
  print("tipped onto an edge: % points, distance %\n", tip.count, r6(tip.distance[0]));

  // A 5 cm ball at 200 m/s toward a 1 cm plate 1 m away, over one 1/60 s step.
  plate := narrow_box(D3_ZERO, DQ.{}, d3(0.005, 1.0, 1.0));
  t := narrow_time_of_impact(narrow_sphere(d3(-1.0, 0.0, 0.0), 0.05), plate, d3(200.0 / 60.0, 0.0, 0.0), D3_ZERO, 0.000001);
  print("fast ball: first touches at % of the step, % m along\n", r6(t), r6(t * 200.0 / 60.0));
}
```

Run it with:

```shell
coax/bin/coax run aethyr/examples/find-where-they-touch.coax
```

```text
sphere / sphere, apart: 1 point(s), distance 0.5, normal (-1.0, 0.0, 0.0)
capsule / capsule, crossed: 1 point(s), distance -0.2, normal (-1.0, 0.0, 0.0)
sphere / box, by GJK: 1 point(s), distance 0.073684, normal (0.442105, 0.894737, 0.063158)
sphere in a box, by EPA: 1 point(s), distance -0.8, normal (0.0, 1.0, 0.0)
box / box, by SAT: 4 point(s), distance -0.2, normal (-0.442105, -0.894737, -0.063158)
the same boxes as hulls, by EPA: 1 point(s), distance -0.199363, normal (-0.398726, -0.91707, 0.0)
  SAT's deepest point -0.2 m, EPA's depth -0.199363 m
GJK on 178 separated box pairs: within 1e-9 m of the closed form: yes, 2.870787 iterations on average
resting box: 4 points
  (0.5, -0.0005, 0.5) distance -0.001, feature 832
  (0.5, -0.0005, -0.5) distance -0.001, feature 833
  (-0.5, -0.0005, -0.5) distance -0.001, feature 834
  (-0.5, -0.0005, 0.5) distance -0.001, feature 835
tipped onto an edge: 2 points, distance -0.007107
fast ball: first touches at 0.2835 of the step, 0.945 m along
```

The two boxes show why the box path is not EPA. EPA finds the true least
translation, 0.199363 m along an edge pair's axis. SAT keeps the face, whose
overlap of 0.2 m is within 5% of it, and reports four points instead of
one: a stack built on face contacts rests, where one built on edge points
rocks. The resting box keeps the same four features under jitter, which is
what lets the solver of chapter 22 warm-start them. The ball would have
crossed the plate in one step; conservative advancement stops it 0.945 m
along, where its surface meets the plate's face.

The checks run with:

```shell
coax/bin/coax test aethyr/sim/narrow.coax --library
```

They compare GJK's distance between boxes given as hulls with a
brute-force distance over every corner and every edge pair, on 200 random
pairs; EPA's depth with the least overlap over all 15 axes, and check that
moving the first box out by that depth leaves the boxes touching; spheres
against spheres and boxes with `aethyr/contact.coax`; capsules' closed form
with GJK; every pair with its arguments swapped; a box resting on the
ground under a micrometre and a microradian of jitter, which must report
the same four features for 1,000 queries; and a 200 m/s ball against a
1 cm plate.

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

## What it still builds

- In `aethyr/sim/narrow.coax`: MPR as a measured profile, SDF queries
  with their error against grid resolution, triangle meshes and
  heightfields, manifolds for curved pairs by perturbed queries, time of
  impact under rotation, and device kernels generated from the same
  routines.
- A fixture set of random convex polytopes with more than eight points and
  adversarial cases (parallel faces, edge–edge, touching, deep overlap).
- Manifold stability over 10,000 solver steps rather than 1,000 queries.
- Checks that kill the 413 surviving mutants, most of them in EPA's
  polytope bookkeeping and in the box path's edge contacts and four-point
  reduction, or a written reason for each that is equivalent.

## Acceptance checks and oracles

- Primitive pairs against their closed forms at random poses.
- GJK distance on random polytopes against brute-force vertex/face
  distances; EPA depth against a brute-force minimum over face normals of
  the explicitly built Minkowski difference (small cases only).
- Agreement with `aethyr/contact.coax` on its shapes.
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
