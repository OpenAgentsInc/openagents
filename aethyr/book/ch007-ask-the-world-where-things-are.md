# 7. Ask the world where things are

> Engine-core task: `ec-query` (#12609). The module is
> [aethyr/query.coax](../query.coax), with its oracles in
> [aethyr/tests/query-oracle.coax](../tests/query-oracle.coax). The walking
> controller in [verse/player.coax](../../verse/player.coax) asks it for the
> ground under your feet.

Several systems ask where things are in a realm:

- Movement asks what is under your feet.
- The camera asks how far back it can sit before a wall.
- A guard asks whether it can see you.
- The editor asks what a tap hit.
- Placement asks whether a thing fits.

These are four queries over one set of shapes: ground, ray, overlap, and
sweep. When each system writes its own version, the versions disagree. For
example, a character stands on a ledge that the camera ignores, or a realm's
copy of the step height drifts from the walker's. Aethyr gives each realm one
set of colliders and one query module.

## What this chapter teaches

- How to describe a realm's colliders: a flat ground, boxes, capsules, and
  floors.
- The contract of each of the four queries.
- One definition of the ground: its height, its slope, and which surface it
  is.
- How to check a query against a separate oracle that uses containment
  instead of formulas.

## The method

A realm puts its colliders in one `Colliders` value, in world space
([chapter 2](ch002-spaces-and-units.md)). Then any system can ask:

- `query_ground(c, x, z, below, reach)`: the highest surface at or below
  `below`. `query_stand` asks for feet: it sets `below` to the feet plus the
  step height.
- `query_ray(c, o, d, far)`: the first point that the ray `o + t d` reaches,
  for `t` in [0, `far`].
- `query_overlap(c, centre, r, out)`: the colliders that a sphere overlaps.
- `query_sweep(c, from, r, by)`: the first point at which a sphere that moves
  from `from` along `by` touches a collider, for `t` in [0, 1].

The queries follow these rules:

- Every shape is solid. A ray or sweep that starts inside a shape hits it at
  `t = 0`.
- Ties go to the earlier collider: the ground, then the boxes, then the
  capsules, each in index order. The answer depends on the scene, not on the
  order of a search.
- A collider with a value that isn't a number, a box whose corners are
  reversed, and a capsule without a radius are skipped. A partial last entry
  in an array is ignored.
- The queries never change the colliders.

## The rules

The step height, the steepest slope you can stand on, and the floor pad are
defined once, here. The walker reads them from this module.

<!-- source: aethyr/query.coax rules -->
```jai
QUERY_STEP_UP :: STAND_STEP;     // 0.55 m: the highest top a walker steps onto
QUERY_STEEP :: STAND_STEEP;      // tan 55 degrees: a steeper surface doesn't hold you
QUERY_FLOOR_PAD :: 0.15;         // how far past a floor's edge your middle may be and it holds you
QUERY_BOX :: 6;                  // numbers a box: min x, y, z, max x, y, z
QUERY_CAPSULE :: 7;              // numbers a capsule: one end, the other, the radius
QUERY_TOO_BIG :: 1000000.0;      // a coordinate this large is not one

QueryShape :: enum u8 { none; ground; box; capsule; floor; }

// One realm's colliders. The arrays are the realm's own; a partial last entry is ignored.
Colliders :: struct {
  ground: f32;
  boxes: [] f32;
  capsules: [] f32;
  floors: [] f32;      // STAND_FLOATS numbers each (verse/stand.coax)
}
```

Floors use the formats in `verse/stand.coax`: quads, cones, domes, and rings.
Floors answer `query_ground`. Rays, overlaps, and sweeps cover the ground,
the boxes, and the capsules.

## The ground

`query_ground` starts from the flat ground. A box's top replaces it if the
top is no higher than `below`, is higher than the surface found so far, and
its footprint is less than `reach` away. For the walker, `reach` is the
capsule's radius, 0.3 m. A floor replaces the result if it is higher. The
result says whether the surface is too steep to stand on.

<!-- source: aethyr/query.coax ground -->
```jai
// The surface under (x, z): its height, its slope (dy/dx, dy/dz), whether it is too steep to
// stand on, and which collider it is.
QueryGround :: struct { y: f32; gx: f32; gz: f32; steep: bool; shape: QueryShape = QueryShape.ground; index: int; }

// The highest surface at (x, z) at or below `below`: the ground, a box's top whose footprint is
// less than `reach` away (a walker's radius), or a floor (within QUERY_FLOOR_PAD of its edge).
// A surface only as high as one already found doesn't replace it.
query_ground :: (c: Colliders, x: f32, z: f32, below: f32, reach: f32) -> QueryGround {
  out := QueryGround.{y = c.ground};
  for i: 0..<query_boxes(c) {
    b := query_box(c, i);
    if !b.ok || b.high.y > below || b.high.y <= out.y { continue; }
    if query_footprint(b, x, z) < reach { out.y = b.high.y; out.shape = QueryShape.box; out.index = i; }
  }
  h := stand_top(c.floors, c.floors.count / STAND_FLOATS, x, z, below, f32(QUERY_FLOOR_PAD));
  if h.y > out.y && query_finite(h.y) {
    out = QueryGround.{y = h.y, gx = h.gx, gz = h.gz, shape = QueryShape.floor, index = h.id};
  }
  out.steep = stand_steep(StandHit.{gx = out.gx, gz = out.gz});
  return out;
}
// What feet at `feet` stand on: the highest surface they can step onto.
query_stand :: (c: Colliders, x: f32, z: f32, feet: f32, reach: f32) -> QueryGround {
  return query_ground(c, x, z, feet + f32(QUERY_STEP_UP), reach);
}
```

The walker's `floor_under` is now one call to `query_stand`. Every walking
realm (the Grid, Everglade, Fairy Forest, the garden, Hecate's Cemetery, the
realm host, and Dead Gears) reaches the ground through it:

<!-- source: verse/player.coax ground -->
```jai
floor_under :: (w: World, x: f32, z: f32, feet: f32) -> StandHit {
  g := query_stand(colliders(w), x, z, feet, f32(RADIUS));
  return StandHit.{y = g.y, gx = g.gx, gz = g.gz};
}
```

## Rays

`query_ray` tests each shape with its own entry function and keeps the
smallest `t`:

- The ground is a plane that is solid below. A ray from above that points
  down reaches it at `t = (ground - o.y) / d.y`.
- A box uses the slab method. The ray's entry is the latest of the three
  axes' entries, and the face it enters by gives the normal.
- A capsule is a cylinder around the segment, plus a sphere at each end. The
  ray's entry is the earliest root of the three that lies on the shape.

`t` is in lengths of `d`, so a unit `d` gives `t` in metres.

<!-- source: aethyr/query.coax ray -->
```jai
// Where a ray first reaches a collider: t along the direction (in its lengths), the point, the
// outward normal there, and which collider. A ray that starts inside one hits it at t = 0,
// with the normal facing back along the ray.
QueryHit :: struct { hit: bool; t: f32; point: WorldPoint; normal: WorldPoint; shape: QueryShape; index: int = -1; }

// The first t in [0, far] at which o + t d enters box low-high (slabs), and the face it enters by.
QueryEntry :: struct { hit: bool; t: f32; normal: WorldPoint; }
query_enter_box :: (low: WorldPoint, high: WorldPoint, o: WorldPoint, d: WorldPoint, far: f32) -> QueryEntry {
  out: QueryEntry;
  enter := -f32(QUERY_TOO_BIG); leave := f32(QUERY_TOO_BIG);
  face: int; side: f32;   // the axis entered by, and the sign of its outward normal
  ov := query_axes(o); dv := query_axes(d); lv := query_axes(low); hv := query_axes(high);
  for k: 0..<3 {
    if dv[k] == 0 {
      if ov[k] < lv[k] || ov[k] > hv[k] { return out; }
      continue;
    }
    t0 := (lv[k] - ov[k]) / dv[k]; t1 := (hv[k] - ov[k]) / dv[k];
    nearer := stand_lo(t0, t1);
    if nearer > enter { enter = nearer; face = k; side = -dv[k] / abs(dv[k]); }
    leave = stand_lo(leave, stand_hi(t0, t1));
  }
  if leave < stand_hi(enter, 0) || enter > far { return out; }
  out.hit = true;
  if enter <= 0 { out.normal = world_scale(world_unit(d), -1); return out; }   // inside
  out.t = enter;
  n: [3] f32;
  n[face] = side;
  out.normal = world_point(n[0], n[1], n[2]);
  return out;
}
// The first t in [0, far] at which o + t d enters the capsule a-b of radius r: its side, or a
// sphere at either end.
query_enter_capsule :: (a: WorldPoint, b: WorldPoint, r: f32, o: WorldPoint, d: WorldPoint, far: f32) -> QueryEntry {
  out: QueryEntry;
  gap := world_sub(o, query_nearest(a, b, o));
  if world_dot(gap, gap) <= r * r { out.hit = true; out.normal = world_scale(world_unit(d), -1); return out; }
  best := f32(QUERY_TOO_BIG);
  // the side: o + t d at r from the axis' line, between the ends. A capsule that is one point
  // (size 0), or a ray along its axis (qa 0), gives no number here, and only the ends count.
  ab := world_sub(b, a); size := world_dot(ab, ab);
  m := world_sub(o, a);
  dp := world_sub(d, world_scale(ab, world_dot(d, ab) / size));
  mp := world_sub(m, world_scale(ab, world_dot(m, ab) / size));
  qa := world_dot(dp, dp); qb := world_dot(mp, dp); qc := world_dot(mp, mp) - r * r;
  disc := qb * qb - qa * qc;
  if disc >= 0 {
    t := (-qb - sqrt(disc)) / qa;
    s := (world_dot(m, ab) + t * world_dot(d, ab)) / size;
    if t >= 0 && s >= 0 && s <= 1 { best = t; }
  }
  // the ends
  ends := [2] WorldPoint.[a, b];
  for e: 0..<2 {
    me := world_sub(o, ends[e]);
    ea := world_dot(d, d); eb := world_dot(me, d); ec := world_dot(me, me) - r * r;
    ed := eb * eb - ea * ec;
    if ed >= 0 {
      t := (-eb - sqrt(ed)) / ea;
      if t >= 0 { best = stand_lo(best, t); }
    }
  }
  if best > far { return out; }
  out.hit = true; out.t = best;
  p := world_add(o, world_scale(d, best));
  out.normal = world_unit(world_sub(p, query_nearest(a, b, p)));
  return out;
}
// The first t in [0, far] at which o + t d reaches height y from above (the ground is solid below).
query_enter_ground :: (y: f32, o: WorldPoint, d: WorldPoint, far: f32) -> QueryEntry {
  out: QueryEntry;
  if o.y < y { out.hit = true; out.normal = world_scale(world_unit(d), -1); return out; }
  if !(d.y < 0) { return out; }
  t := (y - o.y) / d.y;
  if t > far { return out; }
  out.hit = true; out.t = t; out.normal = world_point(0, 1, 0);
  return out;
}
query_keep :: (best: QueryHit, e: QueryEntry, shape: QueryShape, index: int) -> QueryHit {
  if !e.hit || (best.hit && e.t >= best.t) { return best; }
  return QueryHit.{hit = true, t = e.t, normal = e.normal, shape = shape, index = index};
}
query_ray :: (c: Colliders, o: WorldPoint, d: WorldPoint, far: f32) -> QueryHit {
  best: QueryHit;
  if !query_point_finite(o) || !query_point_finite(d) || !query_finite(far) || !(world_dot(d, d) > 0) { return best; }
  reach := stand_hi(far, 0);
  best = query_keep(best, query_enter_ground(c.ground, o, d, reach), QueryShape.ground, 0);
  for i: 0..<query_boxes(c) {
    b := query_box(c, i);
    if b.ok { best = query_keep(best, query_enter_box(b.low, b.high, o, d, reach), QueryShape.box, i); }
  }
  for i: 0..<query_capsules(c) {
    k := query_capsule(c, i);
    if k.ok { best = query_keep(best, query_enter_capsule(k.a, k.b, k.r, o, d, reach), QueryShape.capsule, i); }
  }
  if best.hit { best.point = world_add(o, world_scale(d, best.t)); }
  return best;
}
```

## Overlaps

`query_overlap` writes the colliders that a sphere overlaps into `out`, in
collider order, and returns how many there are. If `out` is too small, the
count is still complete. A sphere that only touches a shape doesn't overlap
it.

<!-- source: aethyr/query.coax overlap -->
```jai
QueryId :: struct { shape: QueryShape; index: int; }

// The colliders a sphere overlaps (closer than its radius), in collider order: the ground,
// then boxes, then capsules, each by index. It writes as many as fit in `out` and returns how
// many there are.
query_overlap :: (c: Colliders, centre: WorldPoint, r: f32, out: [] QueryId) -> int {
  n := 0;
  if !query_point_finite(centre) || !query_finite(r) || !(r > 0) { return 0; }
  if centre.y - r < c.ground { n = query_put(out, n, QueryShape.ground, 0); }
  for i: 0..<query_boxes(c) {
    b := query_box(c, i);
    if !b.ok { continue; }
    q := world_point(stand_clamp(centre.x, b.low.x, b.high.x), stand_clamp(centre.y, b.low.y, b.high.y), stand_clamp(centre.z, b.low.z, b.high.z));
    g := world_sub(centre, q);
    if world_dot(g, g) < r * r { n = query_put(out, n, QueryShape.box, i); }
  }
  for i: 0..<query_capsules(c) {
    k := query_capsule(c, i);
    if !k.ok { continue; }
    g := world_sub(centre, query_nearest(k.a, k.b, centre));
    if world_dot(g, g) < (r + k.r) * (r + k.r) { n = query_put(out, n, QueryShape.capsule, i); }
  }
  return n;
}
query_put :: (out: [] QueryId, n: int, shape: QueryShape, index: int) -> int {
  if n < out.count { out[n] = QueryId.{shape = shape, index = index}; }
  return n + 1;
}
```

## Sweeps

A sphere of radius `r` touches a shape when its centre reaches the shape grown
by `r`. So `query_sweep` casts a ray from the centre against grown shapes:

- The ground, raised by `r`.
- A capsule, with its radius increased by `r`.
- A box grown by `r`. This shape has rounded edges and corners. It is the
  union of three boxes, each grown by `r` along one axis, and twelve
  capsules of radius `r`, one along each edge. The capsules' end spheres
  cover the corners.

<!-- source: aethyr/query.coax sweep -->
```jai
// A sphere of radius r moving from `from` by `by` first touches a collider at t in [0, 1]: the
// ray from its centre against each collider grown by r. A box grown by r is three boxes, each
// grown by r along one axis, and its twelve edges as capsules of radius r.
query_enter_grown_box :: (low: WorldPoint, high: WorldPoint, r: f32, o: WorldPoint, d: WorldPoint, far: f32) -> QueryEntry {
  best: QueryEntry;
  for k: 0..<3 {
    g: [3] f32;
    g[k] = r;
    gl := world_point(low.x - g[0], low.y - g[1], low.z - g[2]);
    gh := world_point(high.x + g[0], high.y + g[1], high.z + g[2]);
    best = query_sooner(best, query_enter_box(gl, gh, o, d, far));
  }
  for e: 0..<12 {
    k := e / 4;   // the edges along x, then y, then z
    u := f32(e % 2); v := f32((e / 2) % 2);
    a := query_corner(low, high, k, u, v, 0); b := query_corner(low, high, k, u, v, 1);
    best = query_sooner(best, query_enter_capsule(a, b, r, o, d, far));
  }
  return best;
}
// An end of the edge along axis k, at the low (0) or high (1) side of each other axis.
query_corner :: (low: WorldPoint, high: WorldPoint, k: int, u: f32, v: f32, end: int) -> WorldPoint {
  side := [3] f32.[0, 0, 0];
  side[k] = f32(end);
  side[(k + 1) % 3] = u; side[(k + 2) % 3] = v;
  return world_point(low.x + (high.x - low.x) * side[0], low.y + (high.y - low.y) * side[1], low.z + (high.z - low.z) * side[2]);
}
query_sooner :: (a: QueryEntry, b: QueryEntry) -> QueryEntry {
  if !b.hit || (a.hit && b.t >= a.t) { return a; }
  return b;
}
query_sweep :: (c: Colliders, from: WorldPoint, r: f32, by: WorldPoint) -> QueryHit {
  best: QueryHit;
  if !query_point_finite(from) || !query_point_finite(by) || !query_finite(r) || !(r > 0) { return best; }
  if !(world_dot(by, by) > 0) {
    // not moving: a touch at 0, or none
    hits: [1] QueryId;
    if query_overlap(c, from, r, hits[..]) == 0 { return best; }
    return QueryHit.{hit = true, point = from, normal = world_point(0, 1, 0), shape = hits[0].shape, index = hits[0].index};
  }
  best = query_keep(best, query_enter_ground(c.ground + r, from, by, 1), QueryShape.ground, 0);
  for i: 0..<query_boxes(c) {
    b := query_box(c, i);
    if b.ok { best = query_keep(best, query_enter_grown_box(b.low, b.high, r, from, by, 1), QueryShape.box, i); }
  }
  for i: 0..<query_capsules(c) {
    k := query_capsule(c, i);
    if k.ok { best = query_keep(best, query_enter_capsule(k.a, k.b, k.r + r, from, by, 1), QueryShape.capsule, i); }
  }
  if best.hit { best.point = world_add(from, world_scale(by, best.t)); }
  return best;
}
```

The first version grew each of the three boxes along two axes instead of
one. That shape has square edges, so a sphere that passed close to an edge
hit it early. The sweep oracle found the error on its fourth random scene:
the sweep reported a touch at `t = 0.330`, and marching found the first
touch at `t = 0.840`.

## An example

The example [aethyr/examples/four-questions.coax](../examples/four-questions.coax)
asks all four questions about one scene:

<!-- source: aethyr/examples/four-questions.coax example -->
```jai
#load "../query.coax";
#import "Basic";

// One scene and the four questions: a 0.5 m step and a 0.6 m wall on the ground, and a post
// (a capsule). Feet step onto the step but not the wall, a ray finds the wall's face, a sphere
// set down by the post overlaps it, and a ball thrown at the wall touches it short of its face.
main :: () {
  boxes := [12] f32.[0, 0, 0, 1, 0.5, 1,   2, 0, 0, 3, 0.6, 1];
  posts := [7] f32.[5, 0, 0.5, 5, 2, 0.5, 0.25];
  c := Colliders.{boxes = boxes[..], capsules = posts[..]};
  for x: [3] f32.[0.5, 2.5, 4] {
    g := query_stand(c, x, 0.5, 0, 0.3);
    print("stand at x %: y %, on %, index %\n", x, g.y, g.shape, g.index);
  }
  h := query_ray(c, world_point(-1, 0.3, 0.5), world_point(1, 0, 0), 10);
  print("ray along +x: t %, on %, index %, normal (%, %, %)\n", h.t, h.shape, h.index, h.normal.x, h.normal.y, h.normal.z);
  got: [4] QueryId;
  n := query_overlap(c, world_point(5.4, 1, 0.5), 0.2, got[..]);
  print("overlap by the post: % (%, index %)\n", n, got[0].shape, got[0].index);
  s := query_sweep(c, world_point(1.5, 2, 0.5), 0.1, world_point(0.5, -2, 0));
  print("sweep of a 0.1 m ball: t %, centre (%, %), on %, index %\n", s.t, s.point.x, s.point.y, s.shape, s.index);
}
```

```shell
coax/bin/coax run aethyr/examples/four-questions.coax
```

```text
stand at x 0.5: y 0.5, on box, index 0
stand at x 2.5: y 0.0, on ground, index 0
stand at x 4.0: y 0.0, on ground, index 0
ray along +x: t 1.0, on box, index 0, normal (-1.0, 0.0, 0.0)
overlap by the post: 1 (capsule, index 0)
sweep of a 0.1 m ball: t 0.79999995, centre (1.9, 0.4000001), on box, index 1
```

The feet stand on the 0.5 m step, but not on the 0.6 m wall. The ray reaches
the step's face at `x = 0`. The ball's centre stops 0.1 m short of the wall's
face at `x = 2`.

## Checks

The oracles don't use the formulas in `query.coax`. They test whether a point
is inside each shape, read straight from the colliders' arrays. To find a
ray's first hit, an oracle marches along the ray in 2 cm steps, and then
halves the last step 40 times. A sweep's oracle does the same with a sphere.
Each random scene has a ground, three boxes, and three capsules. The query
must agree with the oracle on whether there is a hit, on which collider it
is, and on `t` to within 1 mm. Rays and sweeps each run 48 scenes, in three
checks of 16, so each check stays inside the compile-time step budget.

<!-- source: aethyr/tests/query-oracle.coax oracle -->
```jai
// Two rays into scene `seed`: each first hit is where marching by containment first finds a
// collider, within a millimetre, and is the same collider; the normal faces the ray and points
// out of the solid it hit.
oracle_rays :: (seed: int) {
  r := [1] OracleDraw.[OracleDraw.{rng = seed * 7919 + 12345}];
  s := oracle_scene(r[..]);
  c := Colliders.{ground = s.ground, boxes = s.boxes[..], capsules = s.capsules[..]};
  for k: 0..<2 {
    o := oracle_point(r[..]);
    d := world_unit(world_sub(oracle_point(r[..]), o));
    if k == 1 && seed % 3 == 0 { d = world_unit(world_point(d.x, abs(d.y) + 0.1, d.z)); }   // some rays up: they may reach nothing
    q := query_ray(c, o, d, 10);
    m := oracle_march(c, o, d, 0, 10, 0.02);
    #assert q.hit == m.found.hit;
    if q.hit {
      #assert abs(q.t - m.t) <= 0.001 && q.shape == m.found.shape && q.index == m.found.index;
      #assert world_distance(q.point, world_add(o, world_scale(d, q.t))).value < 0.0001;
      #assert abs(world_dot(q.normal, q.normal) - 1) < 0.001;
      if q.t > 0 {
        #assert world_dot(q.normal, d) < 0;
        #assert oracle_inside(c, q.shape, q.index, world_sub(q.point, world_scale(q.normal, 0.01)));
        #assert !oracle_inside(c, q.shape, q.index, world_add(q.point, world_scale(q.normal, 0.01)));
      } else {
        #assert world_distance(q.normal, world_scale(d, -1)).value < 0.0001;
      }
    }
  }
}
```

The course check runs the walker over a 0.5 m ledge. The walker runs, jumps
onto the ledge, lands, walks across, and walks off the far side. The states
are the same, bit for bit, whether the realm passes walls alone, or the same
walls among far bodies and floors. When the ledge is a floor instead of a
box, the states are the same until you walk off. You drop off the floor
first, because a floor holds you while your middle is within 0.15 m of its
edge, and a box's top holds you while your capsule is over it.

<!-- source: verse/player.coax course -->
```jai
#test "jumping onto, landing on and walking off a ledge is the same in every realm" {
  ledge := [6] f32.[-2, 0, 1.5, 2, 0.5, 3.5];
  among := [12] f32.[-2, 0, 1.5, 2, 0.5, 3.5,   40, 0, 40, 41, 3, 41];
  bodies := [7] f32.[-40, 0, -40, -40, 1.5, -40, 0.4];
  floors: [2 * STAND_FLOATS] f32;
  stand_put_box(floors[..], 0, 30, 30, 34, 34, 1);
  as_floor: [STAND_FLOATS] f32;
  stand_put_box(as_floor[..], 0, -2, 1.5, 2, 3.5, 0.5);
  a: World; a.walls = ledge[..];
  b: World; b.walls = among[..]; b.bodies = bodies[..]; b.floors = floors[..];
  c: World; c.floors = as_floor[..];
  pa := held(verse_player_place(0, 0, 0, 0), 0, 1, false);
  pb := pa; pc := pa;
  landed := -1; off_a := -1; off_c := -1;
  for i: 0..<220 {
    jump := i == 12;
    pa = step(a, verse_player_input(pa, 0, 1, 0, 0, false, jump, 0, 0), STEP);
    pb = step(b, verse_player_input(pb, 0, 1, 0, 0, false, jump, 0, 0), STEP);
    pc = step(c, verse_player_input(pc, 0, 1, 0, 0, false, jump, 0, 0), STEP);
    #assert same_step(pa, pb) && pa.pos.y == pb.pos.y && pa.pos.z == pb.pos.z;
    if landed < 0 && i > 12 && pa.on_ground { landed = i; #assert pa.pos.y == 0.5 && pa.pos.z > 2; }
    if off_a < 0 && landed >= 0 && !pa.on_ground { off_a = i; }
    if off_c < 0 && landed >= 0 && !pc.on_ground { off_c = i; }
    if off_c < 0 { #assert same_step(pa, pc) && pa.pos.y == pc.pos.y; }
  }
  // it landed on the ledge, walked off it, and came down on the ground: the floor first
  #assert landed > 12 && off_a > landed && off_c > landed && off_c < off_a;
  #assert pa.on_ground && pc.on_ground && pa.pos.y == 0 && pc.pos.y == 0 && pa.pos.z > 6;
  // a realm whose ground is not at 0 stands you on it, and on the ledge's top over it
  higher: World; higher.ground = 0.25; higher.walls = ledge[..];
  #assert floor_at(higher, 0, 0, 0.25) == 0.25 && floor_at(higher, 0, 2, 0.25) == 0.5 && floor_at(higher, 0, 2, -1) == 0.25;
}
```

```shell
coax/bin/coax test aethyr/query.coax --library
coax/bin/coax mutate aethyr/query.coax --library --root-only
coax/bin/coax test verse/player.coax --library
```

`aethyr/query.coax` has fourteen checks. They cover:

- the ground: step height, footprint reach, ties, and broken boxes;
- floors: height, slope, steepness, the floor pad, and floors against boxes;
- rays against closed forms: faces, end spheres, starts inside, `far`, and
  inputs that aren't a ray;
- overlaps and sweeps against closed forms: rounded edges, touching at the
  start, a sphere that doesn't move, and bad input;
- exact boundaries: infinite and very large numbers, flat and inside-out
  boxes, touching without overlapping, ties at an edge, tangent rays, and a
  hit at exactly `far`;
- sweeps into each of a box's six faces and twelve edges;
- the ray, sweep, and overlap oracles over random scenes.

`coax mutate` makes 312 mutants of `aethyr/query.coax`, and the checks kill all 312. The last five were boundary mutants that looked equivalent. An independent review found float32 inputs that reach each one: an origin the inside test calls outside, whose side or end root is exactly 0; a ray that enters the side at exactly `s = 0` or `s = 1`, a few ulps from the end sphere's root; and a sweep where a face and an edge give exactly the same `t`. The last check in the file pins those inputs. On `verse/player.coax`, no mutant survives on the lines this task changed.

## Still to do

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md))
moves the remaining copies onto this module:

- Heightfield ground. `verse_terrain_height` and `verse_terrain_ray` in
  `verse/terrain.coax`, and each realm's `ground_at` (Everglade, Fairy Forest,
  the garden, and the villa) become a heightfield collider with exact ray
  marching by cell.
- Rays against floors, for picking on roofs and rubble.
- Everglade's own copies of the 0.55 m step height in its wall filters.
- `verse/stand.coax` moves into `aethyr/` with the other engine modules.

The camera ([chapter 10](ch010-the-camera-follows.md)) moves `camera_room`
in `verse/player.coax` and `verse_orbit_room` in `verse/camera.coax` onto
`query_ray`. A grid index, like `StandGrid` for floors, can come later; a
query with and without it must give the same answer.

## Dependencies

This chapter uses world space from [chapter 2](ch002-spaces-and-units.md).
Collider IDs that survive spawn and despawn come from
[chapter 6](ch006-entities-are-born-and-die.md). The physics part adds its
own broad and narrow phases
([chapter 20](ch020-find-the-pairs-that-might-touch.md),
[chapter 21](ch021-find-where-they-touch.md)), which must agree with these
queries on shared shapes. From Coax, R12 frames and R15 strict math come
later ([Coax roadmap](../../coax/ROADMAP.md)). For background, see the
[game-engine systems inventory, sections 14 and 26](../../docs/research/game-engine-systems.md).
