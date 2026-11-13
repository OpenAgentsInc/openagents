# 8. If you can see it, you can walk to it

> **Status: partly implemented.** Everglade II's colliders are built in
> [`everglade/shared/move.coax`](../shared/move.coax)
> from the same item list as the drawn pieces. Rock hulls are baked into
> [`verse/everglade2/rock-hulls.coax`](../../verse/everglade2/rock-hulls.coax),
> and the OpenAgents Metaverse Engine's [`aethyr/physics/query.coax`](../../aethyr/physics/query.coax) and
> [`aethyr/physics/collide.coax`](../../aethyr/physics/collide.coax) resolve them. The page
> and the room server step you on the same code. This chapter names functions
> and has no source listings yet.

This chapter answers the player's question: "Why can't I walk there?"

The rule is simple. If you can see it, you can walk to it. If something stops
you, you can see what stopped you, and it stops you where you see it. An
invisible wall is a bug, not a design choice.

## What this chapter teaches

- How to build every collider from the same list and transform as the drawn
  piece.
- How to give a rock a collider shaped like the rock.
- How to bound where a player may go without showing an edge.
- How walk tests run the real controller to catch an invisible wall.

## What exists today

| Collider | How it is built | Where |
| --- | --- | --- |
| Tree trunk | A capsule sized to its tree kind's trunk, scaled and turned with the piece | `e2_trunk`, `E2_TRUNK_FIT` |
| Rock and boulder | Its mesh's convex hull, scaled and turned with the piece and set on the land | `e2_rock_hull`, `e2_kit_rock`, `E2_HULL_*` in `rock-hulls.coax` |
| The glade's own trees and rocks | The same, from the glade's item list | `e2_glade_rock`, `e2_glade_stone` |
| Where you may stand | The glade's 26 m circle, the path's corridor and the clearing | `e2_may_stand`, `e2m_keep` |

`e2m_world` gathers the colliders near you each step: up to 48 trunks
(`E2_NEAR_BODIES`) and 16 rocks (`E2_NEAR_HULLS`). The shared step,
`eg_step_advance_among` in
[`everglade/shared/everglade-step.coax`](../../everglade/shared/everglade-step.coax),
moves you among them. The page, NPC first aid and the room server call the same
step, so the server agrees with what you see.

Pebbles, a few centimetres high, don't stop you.

## The method

### One list, two uses

Every collider comes from the same item list and the same transform as the
piece that the GPU draws. If a piece is left out of the pack, it is left out
of the colliders by the same test. `e2_glade_kept` in
[`verse/everglade2/forest.coax`](../../verse/everglade2/forest.coax) is that
test for the glade's own pieces. Both the pack tool and the colliders call it.

### A rock is its hull

A rock's collider is the convex hull of its own mesh.
`verse/tools/assets/everglade2_rock_hulls.coax` bakes the hulls from the
Stylized Nature meshes into `rock-hulls.coax` (`E2_HULL_V`, `E2_HULL_F`). The
page scales and turns each hull with its placed piece.

A hull has a top and sides:

- **The top is ground.** `query_hull_top` answers whether you are over the
  hull and how high its top is there. You step onto a rock lower than 0.55 m.
  You jump onto a taller one and stand on its top.
- **The sides stop you.** `collide_hulls` cuts the hull at three heights, your
  step height, your middle and your head, and pushes you out of each slice by
  what your radius reaches into it. A gentle face lets you walk up it. A steep
  face slides you off through the walk code.

### Bound the walk with woods, not walls

You may stand in the glade's 26 m circle (`E2_GLADE_WALK`), within 11 m of the
path's middle (`E2_WALK`), or in the clearing. A step past that edge slides
along it or stays where it started (`e2m_keep`). The edge itself is invisible,
so it must never stand on open ground. Thick woods stand wherever you may not
go.

## A worked example: the three slices

`collide_hulls` tests three slices, from your step height to your height. The
walk test measures your distance from a hull's sides at the same heights
(`e2t_side` in
[`everglade/client/walk-test.coax`](../client/walk-test.coax)):

```text
slice i      height above your feet
0            0.55 + 1.25 × 0 / 2 = 0.55 m   your step height
1            0.55 + 1.25 × 1 / 2 = 1.175 m  your middle
2            0.55 + 1.25 × 2 / 2 = 1.80 m   your head
```

Take the glade's mossy rock by the path, a Rock_Big_1 about a metre high:

- Its sides cut slices 0 and 1, so walking into it stops you at its side.
- It doesn't reach slice 2, so your head passes over it.
- It is taller than 0.55 m, so you can't step onto it. You jump onto it, and
  `query_hull_top` gives you its top as ground.

A low rock under 0.55 m doesn't cut slice 0, so you step up onto it and walk
across its top.

## The checks

In `everglade/client/walk-test.coax`, through the real controller
step:

- `#test "walking into the glade's mossy rock from three sides stops you at its
  side, where you see it"`.
- `#test "you step up onto a low rock, walk across its top with your feet on it,
  and down off it"`.
- `#test "you jump onto the glade's mossy rock and stand on its top, feet on
  it"`.
- `#test "walking straight into a trunk stops you, and you pass close beside
  it"`.
- `#test "you run the whole path, glade to clearing, down its middle, and nothing
  stops you"`, and the same a metre left and a metre right of the middle.

In `everglade/shared/move.coax`:

- `#test "each rock's hull is its mesh's: its bounds are the mesh's, every vertex
  inside every face"`.
- `#test "a placed rock's hull is scaled and turned with it and set on the land's
  plane"`.
- `#test "every tree kind has a trunk sized to it"`.
- `#test "a step past the edge slides along it, or stays where it started"`.

In `verse/everglade2/forest.coax` and `everglade/client/client.coax`:

- `#test "nothing of the glade's stands on the path out"`.
- `#test "wherever you may not go on, thick woods stand: no edge on open
  ground"`.

```sh
coax/bin/coax test --library everglade/client/walk-test.coax
coax/bin/coax test --library everglade/shared/move.coax
```

## Mistakes

### The invisible wall on the path

Character Create's glade had a small rock in the middle of the road at
z = -13. Everglade II kept the glade's composition, and the rock's collider
came with it. As you left the glade, the camera behind you hid the rock behind
your own body, so you hit something that you couldn't see.

Commit `59acb8e8ec` (#13023) drops that rock from both the pack and the
colliders. `e2_glade_kept` refuses any glade piece within 1 m of the road's
middle past z = -1, and any piece within 4 m of the road past z = -26. The test
`"nothing of the glade's stands on the path out"` asserts that exactly one of
the glade's pieces is left out. The same commit added the whole-path walk
tests, at the middle and a metre to each side, so any new collider on the path
fails them.

### Rock colliders that didn't match the rocks

The first rock colliders (`179b8d86d3`) were capsules along each rock's long
axis, fitted to its footprint. A capsule is round, and a rock is not: the
capsule's sides don't follow a rock's flat faces and corners, so where it
stopped you and where you saw the rock disagreed. A capsule also has no top to
stand on.

Commit `7afe501d33` gives every rock and boulder its mesh's convex hull. The
move test compares each hull with its mesh's bounds and checks that every
vertex is inside every face. The walk tests walk into the mossy rock from three
sides, step onto a low rock and stand on the mossy rock's top.

## What comes next

Part III takes you out of the glade. [Chapter 9](ch009-a-path-through-dense-woods.md)
builds the path.
