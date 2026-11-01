# 8. Ground and collision

> **Status: partly implemented.** The capsule resolution runs: the walker in
> [verse/player.coax](../../verse/player.coax) is pushed out of walls, bodies
> and floor sides with the step, slope and pad rules from chapter 7, and the
> heightfield in [verse/terrain.coax](../../verse/terrain.coax) is the drawn
> surface. Still to do: the `aethyr/ground.coax` and `aethyr/collide.coax`
> modules, the heightfield as a collider in chapter 7's `Colliders`, a
> declared pass count per realm, and the f64 oracle. They land with the realm
> migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md)).

Chapter 7 answers questions about shapes. This chapter covers what happens
when a walker's capsule meets them. This is the game layer's collision. It is
kinematic: a walker is moved and pushed out, and it never pushes back. Bodies
that tumble and stack are Part IV's job. Both layers read the same floor.

## What this chapter teaches

- How the heightfield ground gives the height of the drawn surface.
- How a capsule is pushed out of boxes and other capsules, and how the speed
  that goes into a surface is removed.
- Which box tops are steps and which are walls.
- Why one step can't carry the walker through a thin wall.

## The walker's shape

You are a vertical capsule, `RADIUS` 0.3 m wide and `HEIGHT` 1.8 m tall, with
your feet at `pos`. The realm's colliders come from a `World`: a flat ground
height, boxes (`walls`, six numbers each), capsules (`bodies`, seven numbers
each) and floors (chapter 7's `verse/stand.coax` formats). Each fixed step
moves you horizontally by `vel * dt`, then resolves the move against them.

## The heightfield

A `Ground` is `n` by `rows` cells of size `cell` from its south-west corner,
with `(n + 1)(rows + 1)` heights listed by rows from the south. Each cell is
two triangles split along the diagonal from its south-west corner to its
north-east corner, which is how the mesh is drawn. With `(fx, fz)` the
position inside the cell, the triangle is chosen by `fx >= fz`:

<!-- source: verse/terrain.coax height -->
```jai
// The height of the drawn surface at x, z (the edge's beyond it).
verse_terrain_height :: (g: Ground, heights: [] f32, x: f32, z: f32) -> f32 #export_c {
  if g.n < 1 || g.cell <= 0 { return 0; }
  s := ground_spot(g, x, z);
  h00 := ground_point(g, heights, s.i, s.j); h10 := ground_point(g, heights, s.i + 1, s.j);
  h01 := ground_point(g, heights, s.i, s.j + 1); h11 := ground_point(g, heights, s.i + 1, s.j + 1);
  if s.fx >= s.fz { return h00 + (h10 - h00) * s.fx + (h11 - h10) * s.fz; }
  return h00 + (h11 - h01) * s.fx + (h01 - h00) * s.fz;
}
```

Work one cell by hand. Take the corner heights `h00 = 0`, `h10 = 1`,
`h01 = 2` and `h11 = 4`.

1. At `(fx, fz) = (0.75, 0.25)`, `fx >= fz`, so the height is
   `0 + (1 − 0) × 0.75 + (4 − 1) × 0.25 = 1.5`.
2. At `(0.25, 0.75)`, `fx < fz`, so the height is
   `0 + (4 − 2) × 0.25 + (2 − 0) × 0.75 = 2.0`.

The earlier draft of this chapter split each cell along the other diagonal
(`fx + fz <= 1`). That split gives 1.25 m at the first point, 0.25 m below the
drawn surface. `verse/terrain.coax`'s check "the height is the drawn surface:
exact at every grid point" compares the height with the stored heights at all
289 points of a 16 by 16 ground. A second check steps 1 cm across 400 points
and finds no step of 1 cm or more.

## Walls

A box whose top is more than `STEP_UP` (0.55 m) above your feet, and whose
bottom is below your head, is a wall. Lower tops are floors that chapter 7's
`query_stand` stands you on. The push works on the footprint: if your middle
is less than `RADIUS` from the box's footprint, you move straight away from
its nearest point until you are `RADIUS` away. If your middle is inside the
footprint, you leave by the nearest side. Then the part of your velocity
that points into the wall is removed:

<!-- source: verse/player.coax walls -->
```jai
// Pushes p sideways out of every wall too high to stand on, zeroing the
// speed that went into it; from inside a footprint, out the nearest side.
out_of_walls :: (w: World, p: Player) -> Player {
  next := p;
  for i: 0..<w.walls.count / WALL {
    b := wall(w, i);
    if !b.ok || b.hi.y <= next.pos.y + f32(STEP_UP) || b.lo.y >= next.pos.y + f32(HEIGHT) { continue; }
    e := edge(b, next.pos.x, next.pos.z);
    if e.d >= f32(RADIUS) { continue; }
    ux := 0.0f32; uz := 0.0f32; push := 0.0f32;
    if e.d > 0.0001 {
      ux = (next.pos.x - e.x) / e.d; uz = (next.pos.z - e.z) / e.d; push = f32(RADIUS) - e.d;
    } else {
      // Inside: the side you are nearest.
      west := next.pos.x - b.lo.x; east := b.hi.x - next.pos.x;
      south := next.pos.z - b.lo.z; north := b.hi.z - next.pos.z;
      least := lo(lo(west, east), lo(south, north));
      if least == west { ux = -1; } else if least == east { ux = 1; } else if least == south { uz = -1; } else { uz = 1; }
      push = least + f32(RADIUS);
    }
    next.pos.x += ux * push; next.pos.z += uz * push;
    into := next.vel.x * ux + next.vel.z * uz;
    if into < 0 { next.vel.x -= ux * into; next.vel.z -= uz * into; }
  }
  return next;
}
```

## Bodies

A body is a capsule, such as a robot in the Grid. The push uses the closest
points of your axis (`shin` to `crown`) and the body's axis (Ericson,
*Real-Time Collision Detection*, 5.1.9). If the vertical gap `dy` between the
two points is less than the sum of the radii, the horizontal distance you
need is `sqrt(need² − dy²)`. Pushes repeat up to `PASSES` (3) times, because
one push can move you into another body. Walls are resolved after each
pass:

<!-- source: verse/player.coax across -->
```jai
// Moves p horizontally by (dx, dz), then out of every body it overlaps,
// sideways, and zeroes the part of its velocity going into one. Standing
// right under or on a body's axis, you go back the way you came (or to -Z).
move_across :: (w: World, p: Player, dx: f32, dz: f32) -> Player {
  next := p;
  next.pos.x += dx;
  next.pos.z += dz;
  n := w.bodies.count / BODY;
  for pass: 0..<PASSES {
    pushed := false;
    for i: 0..<n {
      bd := body(w, i);
      if !bd.ok { continue; }
      c := closest(shin(next.pos), crown(next.pos), bd.a, bd.b);
      need := bd.r + f32(RADIUS);
      dy := c.p.y - c.q.y;
      if abs(dy) >= need { continue; }
      hx := c.p.x - c.q.x; hz := c.p.z - c.q.z;
      h := sqrt(hx * hx + hz * hz);
      reach := sqrt(need * need - dy * dy);
      if h >= reach { continue; }
      if h < 0.0001 {
        hx = -dx; hz = -dz;
        h = sqrt(hx * hx + hz * hz);
        if h < 0.0001 { hx = 0; hz = -1; h = 1; }
        hx = hx / h * 0.0001; hz = hz / h * 0.0001; h = 0.0001;
      }
      ux := hx / h; uz := hz / h;
      next.pos.x += ux * (reach - h);
      next.pos.z += uz * (reach - h);
      into := next.vel.x * ux + next.vel.z * uz;
      if into < 0 { next.vel.x -= ux * into; next.vel.z -= uz * into; }
      pushed = true;
    }
    next = out_of_walls(w, next);
    if !pushed { break; }
  }
  return next;
}
```

## Floor sides

A floor such as a roof can cut through you from the side: its surface is
above your step height and below your head. `off_floors` checks the new
position after the move. If a floor now cuts through you and did not before,
it undoes the move on one axis, or on both, and stops your speed on that
axis.

## An example

The example [aethyr/examples/push-out.coax](../examples/push-out.coax) holds
forward for 2 s (128 steps) against five sets of colliders:

<!-- source: aethyr/examples/push-out.coax example -->
```jai
#load "../../verse/player.coax";
#import "Basic";

// A walker meets a realm's colliders: a 0.1 m wall it runs into, two steps 0.55 m and 0.56 m
// high, a post (a capsule), and a wall whose corners are reversed. Each run is 2 s of holding
// forward (+Z) from z = 0, on the 1/64 s step.
walk :: (w: World, run: bool) -> Player {
  p := verse_player_place(0, 0, 0, 0);
  for k: 0..<128 { p = step(w, verse_player_input(p, 0, 1, 0, 0, run, false, 0, 0), STEP); }
  return p;
}
show :: (what: string, p: Player) {
  print("%: feet at (%, %, %), speed into it %\n", what, p.pos.x, p.pos.y, p.pos.z, p.vel.z);
}
main :: () {
  thin := [6] f32.[-2, 0, 3, 2, 2, 3.1];         // a 0.1 m wall whose face is at z = 3
  a: World; a.walls = thin[..];
  show("run into a 0.1 m wall", walk(a, true));
  print("a step's travel at RUN: % m, the capsule's width: % m\n", f32(RUN) * f32(STEP), 2 * f32(RADIUS));
  low := [6] f32.[-2, 0, 3, 2, 0.55, 20];
  high := [6] f32.[-2, 0, 3, 2, 0.56, 20];
  b: World; b.walls = low[..];
  c: World; c.walls = high[..];
  show("walk onto a 0.55 m step", walk(b, false));
  show("walk into a 0.56 m step", walk(c, false));
  pole := [7] f32.[0, 0, 3, 0, 2, 3, 0.4];       // upright, radius 0.4 m, on the line you walk
  d: World; d.bodies = pole[..];
  show("walk into a post", walk(d, false));
  reversed := [6] f32.[2, 2, 3.1, -2, 0, 3];     // the first wall with its corners swapped
  e: World; e.walls = reversed[..];
  show("run at a wall with reversed corners", walk(e, true));
}
```

```shell
coax/bin/coax run aethyr/examples/push-out.coax
```

```text
run into a 0.1 m wall: feet at (-1.2754847e-06, 0.0, 2.7), speed into it 0.0
a step's travel at RUN: 0.1171875 m, the capsule's width: 0.6 m
walk onto a 0.55 m step: feet at (-6.052999e-07, 0.54999983, 6.923828), speed into it 3.5
walk into a 0.56 m step: feet at (-6.052999e-07, 0.0, 2.7), speed into it 0.0
walk into a post: feet at (-0.00008809681, 0.0, 2.3), speed into it 5.9604645e-08
run at a wall with reversed corners: feet at (-1.2754847e-06, 0.0, 14.589844), speed into it 7.5
```

Each number can be checked by hand:

- **The thin wall.** At `RUN`, one step moves you 0.1171875 m, less than the
  0.6 m width of your capsule. The step can't carry your middle past the
  wall's face, so the push always finds you on the near side. You stop at
  `z = 3 − 0.3 = 2.7` with no speed into the wall.
- **The steps.** The 0.55 m top is a floor. You walk onto it and stand at
  0.55 m; the shown 0.54999983 m is the last of the `STEP_SMOOTH` easing.
  The 0.56 m top is a wall, and you stop at 2.7 m.
- **The distance walked.** `ACCEL` is 60 m/s², so one step adds 0.9375 m/s.
  At `WALK` (3.5 m/s), the first three steps move you `(0.9375 + 1.875 +
  2.8125) / 64` m and the other 125 move you `3.5 × 125 / 64` m. The total
  is `443.125 / 64 = 6.923828` m, which is the printed `z`.
- **The post.** Its radius is 0.4 m, so your middle stops `0.4 + 0.3 = 0.7`
  m from its axis, at `z = 2.3`. The speed left into it, 6 × 10⁻⁸ m/s, and
  the 0.09 mm sideways drift come from rounding in `f32`.
- **The broken wall.** A box with reversed corners is not a box. It is
  skipped, so you run through. At `RUN`, the first eight steps add 0.9375
  m/s each and the other 120 move at 7.5 m/s, so `z = (0.9375 × 36 + 7.5 ×
  120) / 64 = 14.589844` m.

## Checks

```shell
coax/bin/coax test verse/player.coax --library
coax/bin/coax test verse/terrain.coax --library
```

These checks in `verse/player.coax` cover this chapter:

- "you walk up to a body and stop at its edge", "a body moving into you
  pushes you" and "a body passes over your head and under your feet";
- "standing on a body's axis, you go back the way you came";
- "bodies with bad numbers, and a partial one, are skipped" and "walls with
  bad numbers, inside out or partial are skipped";
- "you walk up to the desk and stop", "a desk's corner rounds you off it,
  and inside one you go out the nearest side" and "you jump onto the desk
  and stand on it; the screen is still a wall";
- "walking into a low roof's edge from the side stops you; one under a step
  you walk onto".

## Still to do

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md)) adds:

- `aethyr/ground.coax`, the heightfield moved from `verse/terrain.coax`, as a
  collider that chapter 7's `query_ground` reads. Today `Colliders.ground` is
  one flat height, and Everglade, Fairy Forest, the garden and the villa each
  read their own heights.
- `aethyr/collide.coax`, the capsule resolution moved from
  `verse/player.coax`, with each realm declaring its static and moving
  colliders and its pass count.
- A drawn-equals-walked check at random points inside cells, not only at
  grid points.
- An f64 oracle for capsule–box and capsule–capsule distances, compared with
  the `f32` path.
- A note on which realms couple walkers to rigid bodies
  (`verse/rigid.coax`, `verse/contact.coax`, `verse/throw.coax`) and through
  which calls. A walker pushes a rigid body only through the rigid-body API.

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md),
[chapter 2](ch002-spaces-and-units.md),
[chapter 6](ch006-entities-are-born-and-die.md) and
[chapter 7](ch007-ask-the-world-where-things-are.md). From Coax: R12 units and
R15 strict math ([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, sections 16 and 26](../../docs/research/game-engine-systems.md),
[space primitives](../../docs/verse/space.md).
