# 10. The camera follows

> **Status: partly implemented.** Engine-core task: `ec-camera` (#12609).
> The follow camera runs: [aethyr/follow.coax](../follow.coax) holds the
> look, the zoom, the spring on the point it looks at, room from the
> colliders and the eye drawn between steps, and every walking realm moves
> its camera through it. Still to do: the orbit, first-person and dive
> cameras as producers of one `Shot`, and moving the spring out of the
> player's step, so that the camera module updates the camera and the
> player's step never touches it.

If a camera chases the last physics state, it moves in steps of 1/64 s while
the screen refreshes at 60 or 120 Hz, and the character appears to shake.
If it eases in the physics step and again every frame, it lags twice, by a
different amount at each frame rate. If it resolves its own collisions in
physics, it can change what happens. Aethyr's rule: the camera eases in one
place, on the fixed step, with easing that is a closed form in the step's
time; it asks the colliders for room and never changes them; and the frame
draws it between the last two steps.

## What this chapter teaches

- The follow camera's state: turn, tilt, distance, and a spring-followed
  target.
- A critically damped spring that never overshoots.
- Room from the colliders: pulled in at once, backed out gently.
- Easing that is the same for any step size.
- Drawing the camera between the last two steps.

## The state

<!-- source: aethyr/follow.coax follow -->
```jai
FOLLOW_PI :: 3.14159265;
FOLLOW_TAU :: 6.2831853;
FOLLOW_DIST :: 4.4;           // the distance a camera starts at, in metres
FOLLOW_CLOSEST :: 0.35;      // the nearest a wall or body pulls it in
FOLLOW_EASE :: 0.18;         // the share of the way back out closed each 1/60 s
FOLLOW_SPRING_TIME :: 0.1;   // the spring's time constant, in seconds
FOLLOW_CLEAR :: 0.2;         // how far the camera keeps from a collider
FOLLOW_PHI_MIN :: 0.1;       // the tilt, down from straight up: never straight up or down
FOLLOW_PHI_MAX :: 3.04159265;
FOLLOW_ZOOM_MIN :: 1.0;      // the range the zoom keeps the distance in
FOLLOW_ZOOM_MAX :: 20.0;
FOLLOW_HEIGHT :: 1.52;       // the point it looks at, above the walker's feet

// A follow camera: theta around +Y from +Z and phi down from +Y, both in radians; `dist` is how
// far out it is shown, `max_dist` how far the zoom asks for; `target` is the point it looks at
// and `vel` that point's spring velocity.
Follow :: struct {
  theta: f32 = FOLLOW_PI;
  phi: f32 = 1.10714872;     // atan2(400, 200): behind and above
  dist: f32 = FOLLOW_DIST;
  max_dist: f32 = FOLLOW_DIST;
  target: WorldPoint = WorldPoint.{x = 0, y = FOLLOW_HEIGHT, z = 0};
  vel: WorldPoint;
}
```

## Look and zoom

The look input changes the turn and tilt directly. The tilt stays between
`FOLLOW_PHI_MIN` and `FOLLOW_PHI_MAX`, so the camera never looks straight up
or down. A value that isn't a number changes nothing. `follow_offset` is the
unit direction from the target to the camera. It uses `math_sin` and
`math_cos` (chapter 5), because the walker's step reads it.

<!-- source: aethyr/follow.coax look -->
```jai
// The look input turns and tilts the camera (radians; positive turns left and looks up), and
// the zoom moves it out (metres). A value that isn't a number changes nothing.
follow_look :: (f: Follow, dyaw: f32, dpitch: f32) -> Follow {
  next := f;
  if follow_finite(dyaw) { next.theta = follow_wrap(f.theta + dyaw); }
  if follow_finite(dpitch) { next.phi = follow_clamp(f.phi + dpitch, FOLLOW_PHI_MIN, FOLLOW_PHI_MAX); }
  return next;
}
follow_zoom :: (f: Follow, delta: f32) -> Follow {
  next := f;
  if follow_finite(delta) { next.max_dist = follow_clamp(f.max_dist + delta, FOLLOW_ZOOM_MIN, FOLLOW_ZOOM_MAX); }
  return next;
}
// The unit direction from the target out to the camera. It uses aethyr/math.coax, so a step
// that reads it has the same bits on every host.
follow_offset :: (f: Follow) -> WorldPoint {
  out := math_sin(f.phi);
  return world_point(out * math_sin(f.theta), math_cos(f.phi), out * math_cos(f.theta));
}
```

## The spring

The target follows the point 1.52 m above your feet with a critically damped
spring (*Game Programming Gems 4*, 1.10). Its time constant is 0.1 s, so
`ω = 2 / 0.1 = 20` per second. From rest at distance `x0`, the exact spring
leaves `x0 (1 + ω t) e^(−ω t)`. The step uses a rational approximation of the
exponential, and if the result would cross the target, it stops on the
target with no speed. It never overshoots.

## Room

`follow_room` finds how far out the camera has room along `offset`. Each box
and capsule is widened by `FOLLOW_CLEAR` (0.2 m), and the camera stops where
the line from the target enters one. If the target is already inside a box's
widened band, the box holds the camera in only if the line runs into the box
itself. This keeps a camera that backs away from a wall out of the wall
(Dead Gears, 2026-10-31).

<!-- source: aethyr/follow.coax room -->
```jai
// How far out along `offset` (a unit direction) from `target` the camera has room, up to `max`
// and `limit` (a realm's own ceiling on it): boxes and capsules are widened by FOLLOW_CLEAR, and
// the camera stops where the line enters one. A box the target is already beside (inside its
// FOLLOW_CLEAR band) holds the camera in only if the line runs into the box itself, so backing
// away from a wall never puts the camera in it. It reads the colliders and changes nothing.
follow_room :: (c: Colliders, limit: f32, target: WorldPoint, offset: WorldPoint, max: f32) -> f32 {
  room := follow_lo(limit, max);
  if c.boxes.count >= QUERY_BOX { room = follow_room_boxes(c, room, target, offset, max); }
  if c.capsules.count >= QUERY_CAPSULE { room = follow_room_capsules(c, room, target, offset, max); }
  return room;
}
follow_room_boxes :: (c: Colliders, start: f32, target: WorldPoint, offset: WorldPoint, max: f32) -> f32 {
  room := start;
  o := query_axes(target); d := query_axes(offset);
  for i: 0..<query_boxes(c) {
    b := query_box(c, i);
    if !b.ok { continue; }
    l := query_axes(b.low); h := query_axes(b.high);
    enter := 0.0f32; leave := max; hit := true;
    for k: 0..<3 {
      a := l[k] - f32(FOLLOW_CLEAR); z := h[k] + f32(FOLLOW_CLEAR);
      if abs(d[k]) < 0.000001 {
        if o[k] < a || o[k] > z { hit = false; }
      } else {
        t0 := (a - o[k]) / d[k]; t1 := (z - o[k]) / d[k];
        enter = follow_hi(enter, follow_lo(t0, t1)); leave = follow_lo(leave, follow_hi(t0, t1));
      }
    }
    if !hit || enter > leave { continue; }
    if enter > 0 { room = follow_lo(room, enter); continue; }
    real := 0.0f32; out := max; meets := true;
    for k: 0..<3 {
      if abs(d[k]) < 0.000001 {
        if o[k] < l[k] || o[k] > h[k] { meets = false; }
      } else {
        t0 := (l[k] - o[k]) / d[k]; t1 := (h[k] - o[k]) / d[k];
        real = follow_hi(real, follow_lo(t0, t1)); out = follow_lo(out, follow_hi(t0, t1));
      }
    }
    if meets && real <= out { room = follow_lo(room, follow_hi(0, real - f32(FOLLOW_CLEAR))); }
  }
  return room;
}
follow_room_capsules :: (c: Colliders, start: f32, target: WorldPoint, offset: WorldPoint, max: f32) -> f32 {
  room := start;
  far := world_add(target, world_scale(offset, max));
  for i: 0..<query_capsules(c) {
    bd := query_capsule(c, i);
    if !bd.ok { continue; }
    cl := follow_closest(target, far, bd.a, bd.b);
    gap := world_sub(cl.p, cl.q);
    d2 := world_dot(gap, gap);
    r := bd.r + f32(FOLLOW_CLEAR);
    if d2 >= r * r { continue; }
    // The line enters the widened capsule about this far before the closest point: further
    // when it crosses the axis at a slant.
    axis := world_sub(bd.b, bd.a);
    size := world_dot(axis, axis);
    slant := 1.0f32;
    if size > 0.000001 {
      along := world_dot(axis, offset);
      slant = sqrt(follow_hi(1 - along * along / size, 0.04));
    }
    room = follow_lo(room, follow_hi(cl.s * max - sqrt(r * r - d2) / slant, 0));
  }
  return room;
}
```

## One step of the camera

`follow_track` runs once a fixed step. It moves the target on the spring.
Then, if the room is shorter than the distance, the camera pulls in to it at
once, but never nearer than `FOLLOW_CLOSEST` (0.35 m). If the room is longer,
the camera backs out by `FOLLOW_EASE` (0.18) of the remaining distance each
1/60 s. The back-out uses `1 − (1 − 0.18)^(60 dt)`, so two steps of `dt / 2`
back out as far as one step of `dt`:

<!-- source: aethyr/follow.coax track -->
```jai
// One step of the camera: the target follows `dest` on the spring, then the camera pulls in at
// once to the room it has (never nearer than FOLLOW_CLOSEST), or backs out toward it by
// FOLLOW_EASE each 1/60 s, a closed form in dt.
follow_track :: (f: Follow, c: Colliders, limit: f32, dest: WorldPoint, dt: f32) -> Follow {
  next := follow_spring(f, dest, dt);
  room := follow_hi(f32(FOLLOW_CLOSEST), follow_room(c, limit, next.target, follow_offset(next), next.max_dist));
  next.dist = follow_lo(room, next.dist + (room - next.dist) * (1 - math_pow(f32(1 - FOLLOW_EASE), dt * 60)));
  return next;
}
```

## Drawing between steps

The frame mixes the last two steps' targets by the loop's alpha (chapter 1).
The distance is mixed too, but it is never drawn further out than the last
step. A wall pulls the camera in at once, and the frame doesn't show it
outside the wall:

<!-- source: aethyr/follow.coax eye -->
```jai
// Where to draw the camera: the target mixed between the last two steps by `share` (the loop's alpha, in [0, 1]),
// out along the camera's direction by the mixed distance, but never further than the last step
// (a wall pulls the camera in at once, and drawing never shows it further out).
follow_at :: (prev: WorldPoint, cur: WorldPoint, share: f32) -> WorldPoint {
  return world_point(prev.x + (cur.x - prev.x) * share, prev.y + (cur.y - prev.y) * share, prev.z + (cur.z - prev.z) * share);
}
follow_eye :: (at: WorldPoint, offset: WorldPoint, prev_dist: f32, dist: f32, share: f32) -> WorldPoint {
  d := follow_lo(prev_dist + (dist - prev_dist) * share, dist);
  return world_point(at.x + offset.x * d, at.y + offset.y * d, at.z + offset.z * d);
}
```

The walker draws its camera with these two calls:

<!-- source: verse/player.coax drawn -->
```jai
// The feet and facing to draw this frame.
verse_player_feet :: (p: Player) -> Vec3 #export_c { return mix3(p.prev_pos, p.pos, alpha(p)); }
verse_player_face :: (p: Player) -> f32 #export_c {
  return wrap_angle(p.prev_face + wrap_angle(p.face - p.prev_face) * alpha(p));
}
// The camera this frame: what it looks at, and where it is. A wall pulls the
// camera in at once, so it is never shown further out than the last step.
verse_player_at :: (p: Player) -> Vec3 #export_c { return vp(follow_at(wp(p.prev_target), wp(p.target), alpha(p))); }
verse_player_eye :: (p: Player) -> Vec3 #export_c {
  return vp(follow_eye(wp(verse_player_at(p)), follow_offset(follow_of(p)), p.prev_dist, p.dist, alpha(p)));
}
```

## A worked example

Work these out by hand:

1. **The spring.** After one step of 1/64 s, `ω t = 20 / 64 = 0.3125`, so the
   exact spring leaves `(1 + 0.3125) e^(−0.3125) = 0.96025` of the distance.
   After eight steps, `ω t = 2.5`, and it leaves `3.5 e^(−2.5) = 0.28730`.
2. **The wall.** A box whose face is 2.5 m behind the target is widened by
   0.2 m, so the camera has 2.3 m of room.
3. **Backing out.** When the wall goes, the camera backs out from 2.3 m
   toward 4.4 m. After 0.5 s, 30 sixtieths of a second have passed, so the
   distance is `4.4 − 2.1 × 0.82^30 = 4.394547` m, whatever the step size.

The example [aethyr/examples/follow-a-wall.coax](../examples/follow-a-wall.coax)
runs the same three cases, and backs out in steps of 1/30, 1/60, 1/120 and
1/144 s:

<!-- source: aethyr/examples/follow-a-wall.coax example -->
```jai
#load "../follow.coax";
#import "Basic";

// The follow camera on its own: its target chasing a point 1 m away on the spring, a wall
// 2.5 m behind it, and backing out once the wall is gone, in steps of four sizes.
main :: () {
  f: Follow; f.phi = f32(FOLLOW_PI) / 2;                  // level, looking along +Z, out to -Z
  f.target = world_point(0, 0, 0);
  g := f;
  for k: 1..<9 {
    g = follow_spring(g, world_point(1, 0, 0), 0.015625);
    if k == 1 || k == 2 || k == 4 || k == 8 {
      x := 20 * f32(k) * 0.015625;                          // omega t, omega = 2 / 0.1 s
      print("spring at step %: % m left; (1 + wt) e^-wt = %\n", k, 1 - g.target.x, (1 + x) * exp(-x));
    }
  }
  wall := [6] f32.[-1, 0, -3, 1, 3, -2.5];               // its face 2.5 m behind the target
  c := Colliders.{boxes = wall[..]};
  near := follow_track(f, c, 1000, f.target, 0.015625);
  print("with the wall: % m out\n", near.dist);
  for hz: [4] int.[30, 60, 120, 144] {
    h := near;
    for k: 0..<hz / 2 { h = follow_track(h, Colliders.{}, 1000, f.target, 1 / f32(hz)); }
    print("0.5 s after it goes, at % Hz: % m out\n", hz, h.dist);
  }
  print("closed form: 4.4 - 2.1 * 0.82^30 = %\n", 4.4 - 2.1 * pow(0.82, 30));
  print("drawn halfway from 4.4 m to 2.3 m: % m out\n", -follow_eye(world_point(0, 0, 0), world_point(0, 0, -1), 4.4, near.dist, 0.5).z);
}
```

```shell
coax/bin/coax run aethyr/examples/follow-a-wall.coax
```

```text
spring at step 1: 0.96045023 m left; (1 + wt) e^-wt = 0.9602455
spring at step 2: 0.8701707 m left; (1 + wt) e^-wt = 0.86979985
spring at step 4: 0.64518565 m left; (1 + wt) e^-wt = 0.6446358
spring at step 8: 0.2877878 m left; (1 + wt) e^-wt = 0.2872975
with the wall: 2.3 m out
0.5 s after it goes, at 30 Hz: 4.394547 m out
0.5 s after it goes, at 60 Hz: 4.3945465 m out
0.5 s after it goes, at 120 Hz: 4.394547 m out
0.5 s after it goes, at 144 Hz: 4.394547 m out
closed form: 4.4 - 2.1 * 0.82^30 = 4.394547007928371
drawn halfway from 4.4 m to 2.3 m: 2.3 m out
```

The spring stays within 0.0005 of the exact spring for the first eight steps.
The four step sizes agree with the closed form to within one `f32` rounding.
Drawn halfway between a step at 4.4 m and a step at 2.3 m, the camera is at
2.3 m: never outside the wall.

## Checks

The Grid's session check drives a walk, a run and two jumps at 60 FPS, where
the 64 Hz steps don't land one to a frame. The drawn eye moves at most
0.177 m a frame, the distance you can run and climb in a frame. Its move
changes by at most 0.016 m from one frame to the next (measured 0.013 m). It
rises on every frame of a rising arc. Drawing the last step instead of the
mixed one changes the move by more than three times the bound:

<!-- source: verse/gridwalk.coax smooth -->
```jai
// The camera follows without a jump (Aethyr's follow, aethyr/follow.coax, book chapter 10): at
// 60 FPS the 64 Hz steps don't land one to a frame, yet as you walk, run and jump the eye drawn
// each frame moves no further than you can in a frame, its move changes by at most 0.016 m from
// one frame to the next (measured 0.013 m), and on a jump's rising arc it rises every frame.
// Drawing the last step instead changes its move by more than three times that.
#test "the Grid's camera follows a walk, a run and a jump at 60 FPS without a jump" {
  g: [1] GridWalk; g[0] = GridWalk.{};
  gridwalk_place(g[..], 28, 28, -2.35619449, 1.35, 9);
  frame := 1.0f32 / 60.0;
  far := sqrt(f32(pc.RUN * pc.RUN + pc.JUMP * pc.JUMP)) * frame;   // 0.177 m: a run and a jump's climb
  was: pc.Vec3; was_move: pc.Vec3; was_feet := 0.0f32;
  last: pc.Vec3; last_move: pc.Vec3; lurch := 0.0f32;
  rising := 0;
  for f: 0..<240 {
    gridwalk_frame(g[..], 0, 1, f < 90, f == 120 || f == 190, 0, 0, 0, 1000, 2000, 2, frame, false);
    eye := gridwalk_eye(g[..]); feet := gridwalk_feet(g[..]).y;
    move := pc.sub(eye, was);
    p := g[0].you;
    shown := pc.add(p.target, pc.times(pc.orbit_offset(p), p.dist));   // the last step, not mixed
    step_move := pc.sub(shown, last);
    if f >= 3 {
      change := pc.sub(move, was_move);
      #assert sqrt(pc.dot(move, move)) <= far && sqrt(pc.dot(move, move)) > 0;
      #assert sqrt(pc.dot(change, change)) <= 0.016;
      if !p.on_ground && feet > was_feet { #assert move.y > 0; rising += 1; }
      d := pc.sub(step_move, last_move);
      lurch = pc.hi(lurch, sqrt(pc.dot(d, d)));
    }
    was = eye; was_move = move; was_feet = feet; last = shown; last_move = step_move;
  }
  #assert rising >= 30;    // both jumps' rising arcs were checked
  #assert lurch > 3 * 0.016;
}
```

```shell
coax/bin/coax test aethyr/follow.coax --library
coax/bin/coax mutate aethyr/follow.coax
coax/bin/coax test verse/gridwalk.coax --library
```

`aethyr/follow.coax` has thirteen checks. They cover:

- the look's wrap and limits, and the zoom's range from 1 m to 20 m;
- the spring against its closed form, and stopping on the target instead of
  crossing it;
- the closest points of two segments, a segment and a point, and two points;
- room against hand-worked distances for boxes and capsules ahead, beside,
  at a slant and inside, and for a wall just behind you;
- pulling in at once, and backing out by the same amount for one step of `dt`
  or two of `dt / 2`;
- the eye never drawn further out than the last step;
- tracking leaves the colliders exactly as they were.

`coax mutate` makes 188 mutants of `aethyr/follow.coax`, and the checks kill
all 188.

## Still to do

The rest of `ec-camera`:

- **More cameras as one `Shot`.** First person and top down are view modes in
  `verse/player.coax` (`verse_player_view_eye`, `verse_player_view_at`), and
  the free `Orbit` is in `verse/camera.coax`. The dive camera doesn't exist
  yet. Each becomes a producer of `verse/camera.coax`'s `Shot` in
  `aethyr/follow.coax`.
- **The spring leaves the player's step.** Today `step` in
  `verse/player.coax` calls `follow_track`, and `Player` keeps the camera's
  fields because the realms' C faces read them. The camera module updates the
  camera after the player's step, and the step no longer touches it.
- **A read-only check.** Player digests (chapter 5) are the same with the
  camera updated every step, never, or twice a step.

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md)) moves
Ice Cream Man's boom (`verse/icecream/camera.coax`) and `SpaceCam`
(`verse/spacecam.coax`) onto the same `Shot`.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md),
[2](ch002-spaces-and-units.md), [4](ch004-input-becomes-typed-actions.md),
[7](ch007-ask-the-world-where-things-are.md) and
[9](ch009-a-character-moves.md). From Coax: R12 frames and R13 clocks
([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, section 36](../../docs/research/game-engine-systems.md).
