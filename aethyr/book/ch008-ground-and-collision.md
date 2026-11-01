# 8. Ground and collision

> **Status: implemented.** Profile: **native f32 meshes and capsule contacts**.
> The checked ground, ray, sphere-contact and walking-capsule modules live in
> `aethyr/`. `verse/terrain.coax` and the real fixed-step controller use them;
> each walking realm declares its collision profile. This profile qualifies
> native arithmetic, state publication and the existing C boundaries. Whole-realm
> coordinate migration remains in [chapter 64](ch064-every-realm-runs-on-aethyr.md).

Chapter 7 asks where shapes are. This chapter resolves a vertical capsule
against boxes, body capsules and floor sides. Terrain locomotion samples the
feet's height and slope; it does not sweep the full walking capsule against
terrain triangles. Exact sphere/mesh contacts serve queries and the camera.
You supply borrowed, read-only collider views. A successful
move returns a new position and velocity. A refused move returns the original
state and a reason you can inspect.

The controller is kinematic. It moves the walker and removes velocity into a
surface. It does not apply impulses to the bodies it touches. Part IV builds
the rigid-body solver.

## Ground is the drawn triangles

A heightfield has `n` columns, `rows` rows, a cell spacing and a south-west
corner. Zero `rows` means `n` rows. The caller supplies `(n + 1)(rows + 1)`
heights, ordered from the south. These are the same descriptor defaults and
field order that the existing terrain C interface uses:

<!-- source: aethyr/ground.coax types -->
```jai
Ground :: struct {
  n: int = 64; cell: f32 = 1.5; west: f32 = -48; south: f32 = -48;
  rim_from: f32 = 32; rim: f32 = 0.022; rows: int = 0;
}
GroundSample :: struct {
  ok: bool;
  height: units.Quantity(units.Metre, f32);
  gx: f32; gz: f32;   // dy/dx and dy/dz of the selected drawn triangle
  clamped_x: bool; clamped_z: bool;   // strictly beyond the mesh, not on its edge
}
GroundNormal :: struct { x: f32; y: f32 = 1; z: f32; }
```

Each cell splits from its south-west corner to its north-east corner. Call
its four heights `h00`, `h10`, `h01` and `h11`, and its local fractions `fx`
and `fz`. For `fx >= fz`, the height is
`h00 + (h10 − h00) fx + (h11 − h10) fz`. The other triangle gives
`h00 + (h11 − h01) fx + (h01 − h00) fz`.

Work one cell with heights `[0, 1, 2, 4]` and spacing 1 m:

1. At `(0.75, 0.25)`, the first triangle gives
   `0 + 1 × 0.75 + 3 × 0.25 = 1.5 m`. Its slopes are `(1, 3)`.
2. At `(0.25, 0.75)`, the other triangle gives
   `0 + 2 × 0.25 + 2 × 0.75 = 2 m`. Its slopes are `(2, 2)`.
3. On the diagonal, both expressions agree. The first triangle owns the tie.

The sampler uses the actual separately rounded f32 vertex coordinates.
Computing a fraction from ideal spacing alone can choose the wrong triangle
when the origin is large or spacing is not exactly representable. Intervals
and interpolation use f64; the public height and slopes are f32. An exact
stored vertex is copied, including a negative zero.

<!-- source: aethyr/ground.coax sample -->
```jai
// A point beyond the rectangle reads its nearest edge. At the exact north/east edge the
// last cell has fraction 1, not an epsilon below it, so every stored vertex is exact.
ground_sample :: (g: Ground, heights: [] f32,
    x: units.Quantity(units.Metre, f32), z: units.Quantity(units.Metre, f32)) -> GroundSample {
  out: GroundSample;
  count := ground_points(g);
  if count == 0 || heights.count < count || !ground_finite(x.value) || !ground_finite(z.value) { return out; }
  ax := ground_axis(g.west, g.cell, g.n, x.value);
  az := ground_axis(g.south, g.cell, ground_rows(g), z.value);
  fx := ax.fraction; fz := az.fraction;
  k := az.i * (g.n + 1) + ax.i;
  h00 := heights[k]; h10 := heights[k + 1];
  h01 := heights[k + g.n + 1]; h11 := heights[k + g.n + 2];
  if !ground_finite(h00) || !ground_finite(h10) || !ground_finite(h01) || !ground_finite(h11) { return out; }
  y := 0.0; gx := 0.0; gz := 0.0;
  if fx >= fz {
    y = f64(h00) + (f64(h10) - f64(h00)) * fx + (f64(h11) - f64(h10)) * fz;
    gx = (f64(h10) - f64(h00)) / ax.width; gz = (f64(h11) - f64(h10)) / az.width;
  } else {
    y = f64(h00) + (f64(h11) - f64(h01)) * fx + (f64(h01) - f64(h00)) * fz;
    gx = (f64(h11) - f64(h01)) / ax.width; gz = (f64(h01) - f64(h00)) / az.width;
  }
  // Copy a stored vertex instead of reconstructing it through rounded differences.
  if fx == 0 {
    if fz == 0 { y = f64(h00); } else if fz == 1 { y = f64(h01); }
  } else if fx == 1 {
    if fz == 0 { y = f64(h10); } else if fz == 1 { y = f64(h11); }
  }
  if !ground_finite(f32(gx)) || !ground_finite(f32(gz)) { return GroundSample.{}; }
  return GroundSample.{ok = true, height = units.Quantity.{value = f32(y)}, gx = f32(gx), gz = f32(gz),
    clamped_x = ax.clamped, clamped_z = az.clamped};
}
```

A point beyond the rectangle reads its nearest edge. The sample reports the
selected triangle's slopes and which axes were clamped. A consumer extending
the edge as a surface sets the slope on each clamped axis to zero before
forming its normal. Both the spatial queries and `verse_terrain_normal` do
that. At a corner beyond both axes, the continuation's normal is straight up.

The descriptor admits at most 1,048,576 cells on either axis, finite in-range
coordinates, positive spacing and distinguishable rounded vertices. Admission
also conservatively requires spacing of at least four ulps of the largest
rounded intermediate; a grid can be geometrically distinct and still fail
this supported numeric profile. Coordinates, heights and slopes must be
strictly between −1,000,000 and 1,000,000. Only the
four sampled heights are read and validated. Invalid dimensions, a short view,
an invalid sampled height or an unsupported gradient return `ok = false`.
`ground_sample` takes metre quantities, so supplying seconds or a bare number
fails the unit check with `E0317`.

## Ask about a heightfield

`QueryHeightfield` pairs a descriptor with its height view. Add a view of these
pairs to `Colliders.heightfields`. A valid mesh replaces the flat fallback,
including where its valleys are below that fallback. The highest mesh wins;
equal heights retain the earlier index. Boxes and floor shapes can still
supply a higher surface you can step onto.

New runtime consumers use `query_ground_checked` or `query_stand_checked`.
They refuse malformed fields and tables larger than 16 fields. The older
`query_ground` retains its permissive skip behavior for trusted callers.
The real controller consumes the checked receipt before publishing a step.

The other three questions work on the same mesh:

- A ray traverses actual cell intervals and solves the triangle plane exactly.
  A narrow crest is not skipped by a fixed marching distance.
- Sphere overlap checks closest points on triangle faces, edges and vertices.
- A moving sphere checks the offset face, edge capsules and vertex spheres,
  and returns the first touch. Its returned point is the sphere's centre.

The traversal budget defaults to 4,096 cells per field. A ray parameter follows
its supplied direction; a sphere sweep uses a fraction in `[0, 1]`. Terrain
hit selection compares f64 times and keeps the selected helper's point,
then narrows the public distance. Two hits that round to the same f32 distance
still select the nearer surface.

`QueryStatus` distinguishes `ok`, `invalid`, `terrain`, `exhausted` and
`capacity`. Exhaustion is a refusal. It is never a clear ray. An overlap
preflights every field before writing IDs, so a later refusal cannot publish
half an answer. `query_overlap_checked` returns a status and count; the legacy
count interface returns `-1` on a heightfield refusal. A short output view
still receives the leading IDs and returns the full successful count.

The follow camera uses a sphere of radius 0.2 m against heightfields. A valid
contact may pull it closer than the legacy 0.35 m minimum. A refused query
preserves its incoming tracking state and reports the reason. Camera failure
is independent of the player's physical step.

## Resolve the capsule

`aethyr/collide.coax` owns the resolution. `CollideProfile` defaults to a radius
of 0.3 m, a height of 1.8 m, a step height of 0.55 m, three passes and at most
0.15 m horizontal travel. Its segment-degeneracy epsilon is fixed at 10⁻⁶.
The admitted pass range is 1–16; each shape table holds at most 4,096 complete
shapes. Partial tails and malformed complete boxes or capsules keep chapter
7's skip rule.

A call to `collide_move` does these steps:

1. Validate the profile, state, capacities and proposed travel.
2. Move horizontally. Resolve body capsules, then walls, in stable collider
   order. Repeat up to the declared pass count while body pushes continue.
3. Roll back an axis, or both axes, if the move newly enters a floor side.
4. Measure remaining penetration. Publish only if the residual is within the
   resolver's 10⁻⁵ m tolerance.

A wall is a box whose top is above the capsule's step height and whose bottom
is below its head. The footprint's nearest point supplies the sideways push.
If the centre is inside the footprint, the nearest side supplies it. After a
push, subtract only velocity directed into the contact normal. Velocity away
from the surface remains.

For a body capsule, find the closest points on its axis and the walker's
axis. With summed radii `r` and vertical separation `dy`, the required
horizontal separation is `sqrt(r² − dy²)`. The segment calculation preserves
the published walker's near-parallel and short-axis decisions. An independent
f64 oracle finds the distance by piecewise quadratic minimization.

The status is `ok`, `bad_profile`, `bad_state`, `capacity`, `travel_limit` or
`unresolved`. Every non-`ok` result preserves the supplied position and
velocity. Its pass count, residual and floor-side flag describe the attempted
resolution. They are diagnostics, not permission to accept it.

This is endpoint resolution with a bounded travel envelope. At `RUN = 7.5`
m/s and `STEP = 1/64` s, travel is `7.5 / 64 = 0.1171875 m`, below the
0.15 m limit and half the capsule's radius. The thin-wall fixture starts
outside a static 0.1 m wall and stops at its face minus the radius. The
profile does not promise general continuous collision detection, safe
arbitrary initial penetration, or moving-collider history.

## The real controller and each realm

The Player's actual fixed step calls `collide_move`, then obtains a checked
ground receipt at the candidate position. Either refusal preserves the incoming
physical and control state, including a queued jump. The frame clock still
consumes the scheduled time. Public getters report collision status, a
saturating refusal count, residual, passes, the floor-side flag, ground-query
status and camera-query status. Success clears the last status while retaining
the cumulative refusal count.

`World.heightfields` is empty by default. A world that supplies a mesh walks,
jumps, slides and lands at its sampled world height. The example below uses a
surface at 2.5 m and lands at 2.5 m. The existing Garden, Fairy, Cemetery and
Everglade faces retain their floor-relative controller/drawing adapters from
[chapter 2](ch002-spaces-and-units.md); their full coordinate migration belongs
to chapter 64.

`aethyr/collision-profiles.coax` declares the same supported numeric profile
for these realm families:

| Family | Accepted identities |
| --- | --- |
| Grid | `the-grid`, `newland` |
| Haunted House | `haunted-house`, `sandbox`, `realm` |
| Garden | `garden` |
| Fairy Forest | `fairy-forest`, `fairy` |
| Everglade | `everglade` |
| Hecate's Cemetery | `hecates-cemetery`, `villa` |
| Dead Gears | `dead-gears`, `deadgears` |

The C faces use `verse_player_advance_in_realm`; Grid and Dead Gears bind their
Coax `World` from the table. An unknown identity returns an invalid declaration
and preserves the input Player. The old C exports retain their signatures.
Trusted direct helpers such as `move_across` retain their larger-move contract;
new runtime movement uses the checked fixed-step entry.

The resolver reads bodies but never pushes them back. `verse/contact.coax`
owns explicit rigid-body impulse and force calls. `verse/demolish.coax`
resolves debris contacts and publishes floor views. `verse/throw.coax` owns
throwing and fetching. None of these connections implies an automatic
walker-to-rigid-body impulse.

## Run the boundary cases

The [example](../examples/ground-and-contacts.coax) computes the worked triangle,
resolves a wall, refuses an excessive move, and runs the real Player on raised
terrain:

<!-- source: aethyr/examples/ground-and-contacts.coax example -->
```jai
#load "../ground.coax" as ground;
#load "../collide.coax" as contact;
#load "../query.coax" as queries;
#load "../space.coax" as world;
#load "../../coax/lib/quantity.coax" as units;
#load "../../verse/player.coax" as walker;
#import "Basic";

main :: () {
  mesh := ground.Ground.{n = 1, cell = 1, west = 0, south = 0};
  heights := [4] f32.[0, 1, 2, 4];
  sampled := ground.ground_sample(mesh, heights[..], units.Quantity.{value = 0.75}, units.Quantity.{value = 0.25});
  print("drawn triangle: height % m, slopes (%, %)\n", sampled.height.value, sampled.gx, sampled.gz);
  walls := [6] f32.[-2, 0, 1, 2, 2, 1.1];
  contact_scene := queries.Colliders.{boxes = walls[..]};
  profile: contact.CollideProfile;
  initial := contact.CollideState.{feet = world.world_point(0, 0, 0.75), velocity = world.world_point(0, 0, 4)};
  moved := contact.collide_move(contact_scene, profile, initial, 0, 0.1);
  print("wall: %, feet z %, speed into it %, passes %\n", moved.status, moved.state.feet.z, moved.state.velocity.z, moved.passes);
  refused := contact.collide_move(contact_scene, profile, initial, 0, 0.16);
  print("too far: %, original feet z %\n", refused.status, refused.state.feet.z);
  walk_heights := [4] f32.[2.5, 2.5, 2.5, 2.5];
  fields := [1] queries.QueryHeightfield.[queries.QueryHeightfield.{
    ground = ground.Ground.{n = 1, cell = 4, west = -2, south = -2}, heights = walk_heights[..]}];
  walk_world := walker.World.{heightfields = fields[..]};
  person := walker.place_in(walk_world, 0, 2.5, 0, 0);
  person = walker.verse_player_input(person, 0, 1, 0, 0, false, true, 0, 0);
  person = walker.step(walk_world, person, walker.STEP);
  print("terrain jump: above 2.5 m = %, query %\n", person.pos.y > 2.5, person.query_status);
  for tick: 0..<48 { person = walker.step(walk_world, person, walker.STEP); }
  print("terrain landing: feet y %, grounded %\n", person.pos.y, person.on_ground);
}
```

```shell
coax/bin/coax run aethyr/examples/ground-and-contacts.coax
```

```text
drawn triangle: height 1.5 m, slopes (1.0, 3.0)
wall: ok, feet z 0.7, speed into it 0.0, passes 1
too far: travel_limit, original feet z 0.75
terrain jump: above 2.5 m = true, query ok
terrain landing: feet y 2.5, grounded true
```

The refused 0.16 m move exceeds the 0.15 m declaration. Its feet remain at
`z = 0.75`; it has not accepted a partly resolved candidate. The existing
[push-out example](../examples/push-out.coax) also checks the 0.55/0.56 m step
boundary, a post, a reversed box and the static thin wall.

## Check the implementation

Checks precede the new implementation. Ground, ray, contact, query, collision
profile, resolver, camera and controller admission have zero-survivor mutation
gates with separately recorded scopes. Native debug, release and ASan/UBSan
harnesses compare actual mesh triangles, ray hits, sphere contacts, the legacy
terrain C ABI and pinned published capsule contacts. The reference Player is
read from commit `3c3b4c3a39`, so adopting the resolver cannot make parity
compare it with itself.

The [verification record](../../docs/verification/aethyr-book-implementation-2026-11-01.json)
links the exact sources, commands and evidence. Large realm integration
scenarios retain their complete bodies and assertions as ordinary Coax native
integration programs when the interpreter's resource limits cannot hold the
rendered scenario. Their records are distinct from native-check ABI evidence;
all imported authoritative checks still run before C emission. This native arithmetic profile does not qualify
browser raster output or every realm's completed migration.

The native gameplay programs are a required companion gate after changing
their source closures. Run each realm/profile command separately; sanitizer
instrumentation can reuse the exact admitted debug C:

```shell
python3 aethyr/tests/check_native_gameplay.py --realm everglade --mode debug --work-dir /tmp/aethyr-everglade-debug
python3 aethyr/tests/check_native_gameplay.py --realm everglade --mode release --work-dir /tmp/aethyr-everglade-release
python3 aethyr/tests/check_native_gameplay.py --realm everglade --mode sanitized --reuse-emission /tmp/aethyr-everglade-debug/emission.json --work-dir /tmp/aethyr-everglade-sanitized
```

Repeat with `--realm deadgears` and separate output directories. The runner
checks original body hashes and finite bindings, verifies that an ordinary
`#assert false` traps, and refuses stale emission reuse. These programs retain
the full rendered spell, camera, quest and game scenarios; they do not replace
the authoritative primitive checks.
