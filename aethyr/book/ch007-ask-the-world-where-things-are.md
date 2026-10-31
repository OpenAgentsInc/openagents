# 7. Ask the world where things are

> **Status: unimplemented — design draft.** Spatial questions are answered
> in many places: `floor_under`, `move_across` and `camera_room` in
> `verse/player.coax`, `verse_orbit_room` in `verse/camera.coax`,
> `verse_terrain_height` and `verse_terrain_ray` in `verse/terrain.coax`,
> the ray and sweep tests in `verse/contact.coax`, `stand_top` in
> `verse/stand.coax`, and each realm's own `ground_at`. This chapter plans one
> query API over a realm's colliders. Engine-core task: `ec-query` (#12609);
> the engine-core agent fills this chapter in as the module lands. Nothing
> here runs yet.

Movement asks "what is under my feet?", the camera asks "how far back can I
sit before a wall?", a guard asks "can I see the player?", the editor asks
"what did the tap hit?", and placement asks "will this fit here?". They are
four queries over one set of shapes: ground, raycast, overlap and sweep. When
each system writes its own version, they disagree. A character stands on a
ledge the camera ignores, or Everglade's own `STEP_UP` copy drifts from the
walker's. Aethyr gives the realm one collider set and one query module, and
one meaning of "on the ground".

## What this chapter teaches

- A realm's colliders: heightfield, boxes, capsules, spheres, floors
  (quads, cones, domes, rings) and triangle meshes.
- The four queries and their exact contracts.
- One ground definition: height, slope and which surface.
- Acceleration structures that do not change answers.

## The method

```jai
ground(x, z, below)   -> Hit { y, slope (gx, gz), id }   // highest surface ≤ below
raycast(o, d, far)    -> Hit { t, point, normal, id }    // first t in [0, far]
overlap(shape)        -> [id]                            // canonically sorted
sweep(shape, Δ)       -> Hit { t ∈ [0, 1], normal, id }  // first touch along Δ
```

`ground` is the walker's question. Its answer is the highest surface at or
below `below = feet + step_up`, so a character steps onto a 0.55 m top and
not a 0.6 m one. The slope comes from the surface: a heightfield's triangle
(split south-west to north-east, exactly as drawn), a floor's gradient, zero
for a box top. A slope past `tan 55°` is reported as steep; chapter 9 decides
what steep does.

Every query is answered against the same colliders, in world space
(chapter 2), with ties broken by collider ID so the answer is a function of
the scene, not of storage order. A uniform grid (as `StandGrid` already uses
for floors) or a bounding-volume hierarchy may accelerate the search; a
query with and without acceleration must return identical results. The
module never mutates; it is read by movement, camera, AI, picking and
placement during a step, and the colliders change only at the step's flush
(chapter 6).

## What exists today

- [verse/player.coax](../../verse/player.coax): `World` (flat `ground`,
  `bodies` as capsules, `walls` as boxes, `floors`), `floor_under` (ground,
  wall tops within `STEP_UP`, `stand_top`), `move_across` (capsule push-out
  in `PASSES` passes), `camera_room` (slab tests against walls, capsules).
- [verse/stand.coax](../../verse/stand.coax): floors as `STAND_QUAD`,
  `STAND_CONE`, `STAND_DOME`, `STAND_RING`; `stand_top`, `stand_cuts`,
  `stand_steep`, and `StandGrid` with `stand_grid_top`.
- [verse/terrain.coax](../../verse/terrain.coax): `Ground`,
  `verse_terrain_height`, `verse_terrain_normal`, `verse_terrain_ray`
  (fixed `RAY_STEP` samples).
- [verse/camera.coax](../../verse/camera.coax): `verse_orbit_room` against
  spheres and boxes.
- [verse/contact.coax](../../verse/contact.coax): f64 `ray_plane`,
  `ray_sphere`, `ray_triangle`, `sweep_sphere_plane`, `sweep_sphere_sphere`.
- Realm copies: `ground_at` in `apps/openagents-web/everglade.coax`,
  `fairy.coax`, `garden.coax` and `villa.coax`; Everglade repeats the
  `0.55` step height in its wall filters.

## What it builds

- `aethyr/query.coax`: `Colliders` (one per realm, built at setup and
  updated at flush), `ground`, `raycast`, `overlap`, `sweep`, an optional
  grid index, and exact heightfield ray marching by cell rather than fixed
  samples.
- The shared constants: step height, steep slope and floor pad, defined once.

## Acceptance checks and oracles

- Jumping, landing and walking off ledges behave the same in every realm: the
  same scripted course built in each realm's collider set gives identical
  ground answers and identical walker trajectories (with chapter 9).
- Oracles: brute-force queries over every collider without acceleration;
  closed forms for sphere, box and plane rays; `verse/contact.coax`'s f64
  tests as an independent implementation for rays and sweeps.
- Heightfield `ground` equals the drawn surface: sampled points on the
  rendered triangles agree with `ground` within one ulp.
- Symmetry and order: overlap results are sorted and identical across
  insertion orders.
- Mutation testing of `aethyr/query.coax`: 0 survivors.

## Dependencies

[Chapter 2](ch002-spaces-and-units.md) (world space and units) and
[chapter 6](ch006-entities-are-born-and-die.md) (collider IDs and flush).
The physics part later adds its own broad and narrow phases
([chapter 20](ch020-find-the-pairs-that-might-touch.md),
[chapter 21](ch021-find-where-they-touch.md)) and must agree with these
queries on shared shapes. From Coax: R12 frames and R15 strict math
([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, sections 14 and 26](../../docs/research/game-engine-systems.md).
