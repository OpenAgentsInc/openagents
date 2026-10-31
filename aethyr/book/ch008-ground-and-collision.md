# 8. Ground and collision

> **Status: unimplemented — design draft.** Today the ground is a
> heightfield (`verse/terrain.coax`), floors are listed shapes
> (`verse/stand.coax`), walls are boxes and robots are capsules
> (`verse/player.coax`), and space realms use f64 rigid bodies with contacts
> (`verse/rigid.coax`, `verse/contact.coax`). This chapter plans how a realm
> declares its ground and colliders once, and how a moving character is
> resolved against them. Nothing here runs yet.

Chapter 7 answers questions about shapes. This chapter decides which shapes
a realm has and what happens when a character's capsule meets them. It is
the game layer's collision: kinematic, stable and cheap, for walkers and
followers. Dynamic bodies that tumble and stack are Part IV's job. The two
must not disagree about where the floor is.

## What this chapter teaches

- Building a heightfield ground that is exactly the drawn surface.
- Declaring colliders: static boxes, floors, meshes, moving capsules.
- Resolving a capsule against them: push out, then remove the velocity that
  goes into the surface.
- Steps, slopes and the floor pad.
- Why game-layer collision and rigid-body contact are kept separate.

## The method

**Ground.** A `Ground` is `n × rows` cells of size `c` from its south-west
corner, with `(n + 1)(rows + 1)` heights. Each cell is two triangles split
from south-west to north-east, so with `(fx, fz)` the position in the cell,

```jai
h(x, z) = fx + fz ≤ 1 ? h00 + fx (h10 − h00) + fz (h01 − h00)
                      : h11 + (1 − fx)(h01 − h11) + (1 − fz)(h10 − h11)
```

which is exactly what the GPU draws. Build is deterministic: rolling hills
rising toward the rim, levelled along paths and on flats.

**Character resolution.** A character is a vertical capsule of radius `r`
and height `H`. Each step it moves horizontally by `v Δt`, then is pushed out
of each overlapping collider along the separating direction, and the
component of velocity into that direction is removed:

```jai
for pass in 0..<P:                      // P = 3 today
  for c in overlapping(capsule):
    n, depth = separation(capsule, c)
    pos += n * depth
    vel -= n * min(0, dot(vel, n))      // stop going into it
```

Box tops up to `step_up` above the feet are floors, higher ones are walls.
A floor whose surface would cut through the capsule between a step and the
head stops horizontal motion. `pad` lets a floor hold a capsule whose middle
is just past its edge.

**Separation from rigid bodies.** Rigid bodies (Part IV) resolve contacts
with impulses and mass. A character is kinematic; it pushes nothing except
through explicit coupling a realm declares (throwing, carrying, kicking a
ball), which goes through the rigid-body API, not through the capsule pass.

## What exists today

- [verse/terrain.coax](../../verse/terrain.coax): `Ground`,
  `verse_terrain_build`, `verse_terrain_height` ("exactly the surface that
  is drawn"), `verse_terrain_normal`, paths with `verse_path_nearest`, and
  followers routing along them.
- [verse/player.coax](../../verse/player.coax): `RADIUS :: 0.3`,
  `HEIGHT :: 1.8`, `PASSES :: 3`, `STEP_UP :: 0.55`, `FLOOR_PAD`,
  `move_across`, `out_of_walls`, `off_floors`, `closest` (segment–segment
  for capsules).
- [verse/stand.coax](../../verse/stand.coax): floor shapes and `stand_cuts`.
- [verse/rigid.coax](../../verse/rigid.coax) and
  [verse/contact.coax](../../verse/contact.coax): f64 rigid bodies, sphere,
  box, plane and mesh contacts, sequential impulses with `REST_SPEED`,
  `SLOP` and `PUSH_SHARE`, used by Orbit League and Belt 2200.
- [verse/throw.coax](../../verse/throw.coax): thrown and carried objects
  that bounce, roll and rest on the ground.

## What it builds

- `aethyr/ground.coax`: the heightfield type and build, moved from
  `verse/terrain.coax`, read by chapter 7's `ground` query.
- `aethyr/collide.coax`: realm collider declaration (static and moving),
  capsule resolution with a declared pass count, and the steps/slopes rules.
- The boundary note: which realms couple characters to rigid bodies, and
  through which calls.

## Acceptance checks and oracles

- Drawn equals walked: for random points, the heightfield's `h(x, z)` equals
  the height interpolated from the drawn triangle's vertices.
- Push-out: after resolution no capsule overlaps a static collider by more
  than a stated tolerance; velocity into each contact normal is non-positive.
- Step rule: a 0.55 m top is stepped onto and a 0.56 m top stops the walker,
  in every realm.
- Oracle: an independent f64 capsule–box and capsule–capsule distance
  (Ericson's closest points) compared with the f32 path.
- No tunnelling at the walker's top speed: at `RUN` and 1/64 s, a 0.1 m wall
  is never crossed (speed times step is well under the capsule's diameter;
  the check asserts that bound).

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md),
[chapter 2](ch002-spaces-and-units.md),
[chapter 6](ch006-entities-are-born-and-die.md) and
[chapter 7](ch007-ask-the-world-where-things-are.md). From Coax: R12 units and
R15 strict math ([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, sections 16 and 26](../../docs/research/game-engine-systems.md),
[space primitives](../../docs/verse/space.md).
