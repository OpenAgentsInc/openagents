# 10. The camera follows

> Engine-core task: `ec-camera` (#12609). The modules are
> [aethyr/follow.coax](../follow.coax), the follow camera, and
> [aethyr/shot.coax](../shot.coax), the shot every camera makes. Every
> walking realm draws its camera through them, and the free orbit in
> [aethyr/camera.coax](../../aethyr/camera.coax) draws its shot through
> `aethyr/shot.coax`. The camera steps after the player's step, and the
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
- One shot type for every camera: third person, first person, top down, the
  free orbit and the dive.
- Stepping the camera after the player, so the player is the same with any
  camera.

Heightfields use the checked sphere contact from chapter 8. A refused query
retains the incoming tracking state and reports `Follow.status`; a later
successful query clears that status. A terrain contact can pull closer than
the legacy 0.35 m minimum so that minimum cannot place the eye inside a mesh.
The terrain probe covers the distance that the minimum-distance rule could
publish, even when a box first reduces the room below that minimum.
The Player exposes camera-query status independently of physical-step status.

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
  status: QueryStatus;
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
FollowRoom :: struct { room: f32; terrain_room: f32; status: QueryStatus; }
follow_room_checked :: (c: Colliders, limit: f32, target: WorldPoint, offset: WorldPoint, max: f32) -> FollowRoom {
  room := follow_lo(limit, max);
  if c.boxes.count >= QUERY_BOX { room = follow_room_boxes(c, room, target, offset, max); }
  if c.capsules.count >= QUERY_CAPSULE { room = follow_room_capsules(c, room, target, offset, max); }
  terrain_room := max;
  if c.heightfields.count > 0 {
    // Keep the existing wall/body bands; only the mesh uses the exact sphere sweep.
    // Probe the distance the closest-distance rule could publish, even when a wall's band
    // reduced `room` below that minimum. The terrain cap is applied after the legacy rule.
    prospective := follow_lo(max, follow_hi(f32(FOLLOW_CLOSEST), room));
    terrain_scene := Colliders.{heightfields = c.heightfields, heightfield_budget = c.heightfield_budget};
    surface := query_sweep(terrain_scene, target, f32(FOLLOW_CLEAR), world_scale(offset, prospective));
    if surface.status != QueryStatus.ok { return FollowRoom.{status = surface.status}; }
    if surface.hit { terrain_room = prospective * surface.t; room = follow_lo(room, terrain_room); }
  }
  return FollowRoom.{room = room, terrain_room = terrain_room};
}
follow_room :: (c: Colliders, limit: f32, target: WorldPoint, offset: WorldPoint, max: f32) -> f32 {
  return follow_room_checked(c, limit, target, offset, max).room;
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
// once to the room it has, or backs out toward it by FOLLOW_EASE each 1/60 s.
// A mesh contact may be nearer than FOLLOW_CLOSEST; a refused query preserves state.
follow_track :: (f: Follow, c: Colliders, limit: f32, dest: WorldPoint, dt: f32) -> Follow {
  next := follow_spring(f, dest, dt);
  available := follow_room_checked(c, limit, next.target, follow_offset(next), next.max_dist);
  if available.status != QueryStatus.ok { refused := f; refused.status = available.status; return refused; }
  room := follow_lo(available.terrain_room, follow_hi(f32(FOLLOW_CLOSEST), available.room));
  next.dist = follow_lo(room, next.dist + (room - next.dist) * (1 - math_pow(f32(1 - FOLLOW_EASE), dt * 60)));
  next.status = QueryStatus.ok;
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

<!-- source: aethyr/walk.coax drawn -->
```jai
// The feet and facing to draw this frame.
walk_feet :: (p: Player) -> Vec3 { return mix3(p.prev_pos, p.pos, alpha(p)); }
walk_face :: (p: Player) -> f32 {
  return wrap_angle(p.prev_face + wrap_angle(p.face - p.prev_face) * alpha(p));
}
// The camera this frame, Aethyr's third-person shot: what it looks at, and where it is. A wall
// pulls the camera in at once, so it is never shown further out than the last step.
walk_shot :: (p: Player) -> CameraShot { return follow_shot(follow_of(p), wp(p.prev_target), p.prev_dist, alpha(p)); }
walk_at :: (p: Player) -> Vec3 { return vp(walk_shot(p).at); }
walk_eye :: (p: Player) -> Vec3 { return vp(walk_shot(p).eye); }
```

## One shot for every camera

Every camera answers with a `CameraShot`: an eye and the point it looks at.
A realm can then cut from one camera to another, or blend between two, and
no camera needs to know about the others. `aethyr/shot.coax` holds the type
and the cameras that need no `Follow`. It loads only `aethyr/space.coax`
(chapter 2), so a realm with cameras of its own can load it without the
colliders.

<!-- source: aethyr/shot.coax shot -->
```jai
// What a camera shows: the eye, and the point it looks at.
CameraShot :: struct { eye: WorldPoint; at: WorldPoint; }

SHOT_TOP_UP :: 22.0;       // top down: the eye, above the feet
SHOT_TOP_BACK :: 0.01;     // and this far toward -Z, so the view has a forward direction

// Top down: high over the drawn feet, looking down at them.
shot_top :: (feet: WorldPoint) -> CameraShot {
  return CameraShot.{eye = world_point(feet.x, feet.y + f32(SHOT_TOP_UP), feet.z - f32(SHOT_TOP_BACK)), at = feet};
}
```

The follow camera makes two shots. In third person, the shot is the follow
camera drawn between its last two steps. In first person, the eye is 1.6 m
above the drawn feet, and it looks the way the follow camera looks:

<!-- source: aethyr/follow.coax shot -->
```jai
// The follow camera's shots (aethyr/shot.coax holds the shot type and the cameras that need no
// Follow): third person, between its last two steps (`share` is the loop's alpha), and first
// person, the eyes above the drawn feet looking the way the follow camera looks.
FOLLOW_FIRST_EYE :: 1.6;     // first person: the eyes, above the feet

follow_shot :: (f: Follow, prev_target: WorldPoint, prev_dist: f32, share: f32) -> CameraShot {
  at := follow_at(prev_target, f.target, share);
  return CameraShot.{eye = follow_eye(at, follow_offset(f), prev_dist, f.dist, share), at = at};
}
follow_first_shot :: (f: Follow, feet: WorldPoint) -> CameraShot {
  eye := world_point(feet.x, feet.y + f32(FOLLOW_FIRST_EYE), feet.z);
  return CameraShot.{eye = eye, at = world_sub(eye, follow_offset(f))};
}
```

The walker's view modes are these three shots. Each one reads only what is
drawn this frame:

<!-- source: aethyr/walk.coax views -->
```jai
// The camera a mode shows, as one of Aethyr's shots (aethyr/follow.coax): first person, the
// follow behind you, or top down. Each reads only what is drawn this frame.
walk_view :: (p: Player, mode: int) -> CameraShot {
  if mode == 0 { return follow_first_shot(follow_of(p), wp(walk_feet(p))); }
  if mode == 2 { return shot_top(wp(walk_feet(p))); }
  return walk_shot(p);
}
walk_view_eye :: (p: Player, mode: int) -> Vec3 { return vp(walk_view(p, mode).eye); }
walk_view_at :: (p: Player, mode: int) -> Vec3 { return vp(walk_view(p, mode).at); }
```

### The free orbit

An open realm uses the free orbit in `aethyr/camera.coax`. Its `Orbit` keeps
the look, the zoom and the easing, and its shot comes from
`aethyr/shot.coax`. The eye sits on a line from the pivot at your shoulder.
As you zoom in past the closest distance, `first` goes from 0 to 1: the
shoulder offset closes, and the line's start rises to your eyes, 0.12 m in
front of your face. The orbit only draws, so it keeps the C library's sine
and cosine, and its shots are the same bits as before the move. A check of
200,000 random orbits gave no difference in any eye, look-at point or ray.

<!-- source: aethyr/shot.coax orbit -->
```jai
// The free orbit: it circles a pivot at your shoulder and, zoomed in past its closest, goes on
// into your head. `first` is how far in it is, from 0 (outside) to 1 (your eyes). It draws only,
// and it uses the C library's sine and cosine as it did in aethyr/camera.coax, so its shots are
// the same bits as before.
SHOT_ORBIT_DIST :: 5.2;      // how far ahead of the eye the point it looks at is
SHOT_ORBIT_HEIGHT :: 1.45;   // the pivot, above the feet
SHOT_ORBIT_SHOULDER :: 0.55; // how far right of you the view is centred
SHOT_ORBIT_EYE :: 1.62;      // first person: the eyes, above the feet
SHOT_ORBIT_FACE :: 0.12;     // and this far in front of the pivot, clear of your face

// The line the eye sits on: from `from`, along the unit `dir`, `far` metres long.
ShotRay :: struct { from: WorldPoint; dir: WorldPoint; far: f32; }

// Where the free orbit looks (yaw 0 faces +Z; pitch is down), and its level right hand.
shot_orbit_forward :: (yaw: f32, pitch: f32) -> WorldPoint {
  return world_point(sin(yaw) * cos(pitch), -sin(pitch), cos(yaw) * cos(pitch));
}
shot_orbit_right :: (yaw: f32) -> WorldPoint { return world_point(-cos(yaw), 0, sin(yaw)); }
// The unit vector along v, or `otherwise` when v has no length.
shot_unit_or :: (v: WorldPoint, otherwise: WorldPoint) -> WorldPoint {
  size := sqrt(world_dot(v, v));
  if !(size > 0.000001) || !shot_finite(size) { return otherwise; }
  return world_scale(v, 1 / size);
}
// The eye's line: `zoom` back from the pivot and right by the shoulder. Going in, the shoulder
// closes and the line's start rises from the pivot to your eyes, a little in front of your face.
shot_orbit_ray :: (pivot: WorldPoint, yaw: f32, pitch: f32, zoom: f32, first: f32) -> ShotRay {
  out_back := world_scale(shot_orbit_forward(yaw, pitch), -zoom);
  out_want := world_add(out_back, world_scale(shot_orbit_right(yaw), f32(SHOT_ORBIT_SHOULDER) * (1 - first)));
  facing := world_point(sin(yaw), 0, cos(yaw));
  ray: ShotRay;
  ray.from = world_add(pivot, world_scale(world_add(world_point(0, f32(SHOT_ORBIT_EYE - SHOT_ORBIT_HEIGHT), 0), world_scale(facing, f32(SHOT_ORBIT_FACE))), first));
  ray.far = sqrt(world_dot(out_want, out_want));
  ray.dir = shot_unit_or(out_want, world_point(0, 0, -1));
  return ray;
}
// The shot: the eye `shown` metres along the line, looking forward.
shot_orbit :: (ray: ShotRay, shown: f32, yaw: f32, pitch: f32) -> CameraShot {
  seen := world_add(ray.from, world_scale(ray.dir, shown));
  return CameraShot.{eye = seen, at = world_add(seen, world_scale(shot_orbit_forward(yaw, pitch), f32(SHOT_ORBIT_DIST)))};
}
```

### The dive

The dive takes the camera from a shot far away, such as a page's overview,
down to a live camera's shot. It is eased at both ends with smootherstep,
`6t^5 − 15t^4 + 10t^3`. The point it looks at moves 1.3 times faster, so the
view turns to you before the eye lands. It reads the shot it ends on in
every frame, so it lands on a camera that is still moving. A frame counts
for at most 0.25 s, the same as the loop's frame clamp, and a time that
isn't a number is refused: a dive of no length is a cut.

<!-- source: aethyr/shot.coax dive -->
```jai
// The dive: from a shot far off (the overview, a page's own view) down to a live camera's shot
// in `length` seconds, eased at both ends. The point it looks at arrives SHOT_DIVE_LEAD times
// sooner than the eye, so the view turns to you before the eye lands. It takes the frame's time,
// as drawing does, and reads the shot it ends on each frame, so it lands on a moving camera.
SHOT_DIVE :: 0.75;          // seconds, unless the realm gives its own
SHOT_DIVE_LEAD :: 1.3;
SHOT_DIVE_FRAME :: 0.25;    // the most one frame advances it, as the loop's frame clamp

ShotDive :: struct { from: CameraShot; elapsed: f32; length: f32 = SHOT_DIVE; }

// A dive from `from` taking `took` seconds. A time that is zero, negative or not a number is a cut.
shot_dive_start :: (from: CameraShot, took: f32) -> ShotDive {
  dive: ShotDive;
  dive.from = from;
  dive.length = ifx shot_finite(took) && took > 0 then took else 0.0f32;
  return dive;
}
// The dive a frame of `seconds` later. A frame that isn't a number, or is negative, counts as
// none; a long one counts as SHOT_DIVE_FRAME.
shot_dive_advance :: (dive: ShotDive, seconds: f32) -> ShotDive {
  dive_next := dive;
  if shot_finite(seconds) { dive_next.elapsed = shot_lo(dive.elapsed + shot_clamp(seconds, 0, f32(SHOT_DIVE_FRAME)), 1000000); }
  return dive_next;
}
// How far through it is, from 0 to 1, and whether it has landed.
shot_dive_share :: (dive: ShotDive) -> f32 {
  if !(dive.length > 0) { return 1; }
  return shot_clamp(dive.elapsed / dive.length, 0, 1);
}
shot_dive_done :: (dive: ShotDive) -> bool { return shot_dive_share(dive) >= 1; }
// Smootherstep: 0 and 1 at the ends, with no speed there.
shot_ease :: (x: f32) -> f32 { u := shot_clamp(x, 0, 1); return u * u * u * (u * (u * 6 - 15) + 10); }
// The shot this frame, on the way to `to`.
shot_dive :: (dive: ShotDive, to: CameraShot) -> CameraShot {
  share := shot_dive_share(dive);
  return CameraShot.{eye = shot_mix(dive.from.eye, to.eye, shot_ease(share)), at = shot_mix(dive.from.at, to.at, shot_ease(share * f32(SHOT_DIVE_LEAD)))};
}
```

## The camera steps after the player

The player's step moves you and nothing else. The camera has its own step,
`camera_step`, which runs after each of the player's steps. It reads the
step's result and the colliders and changes only the camera's fields:
`target`, `spring_vel` and `dist`. `Player` keeps those fields, because the
realms' C faces read them. `steps_in` (chapter 1) runs `step_once`, the
player's step and then the camera's, so every realm that advances through
`verse_player_advance` gets the same camera as before, bit for bit.

<!-- source: aethyr/walk.coax camera -->
```jai
// The camera's step, after the player's (aethyr/follow.coax): its target follows you on the
// spring, and it pulls in at once where something is in the way and backs out gently. It reads
// the step's result and the colliders and changes only the camera's fields, so the player's step
// is the same with the camera or without it. The player keeps those fields because the realms'
// C faces read them.
camera_step :: (w: World, p: Player, dt: f32) -> Player {
  next := p;
  cam := follow_track(follow_of(p), colliders(w), w.room, wp(look_at(p)), dt);
  next.target = vp(cam.target); next.spring_vel = vp(cam.vel); next.dist = cam.dist;
  next.camera_query_status = cam.status;
  return next;
}
// One fixed step of the player, then of its camera: what steps_in runs each step.
step_once :: (w: World, p: Player, dt: f32) -> Player {
  next := step(w, p, dt);
  if next.collision_status != CollideStatus.ok { return next; }
  return camera_step(w, next, dt);
}
```

The walker steers by the camera's turn, `theta`. The look input sets it
(chapter 4), and the camera's step never changes it. So the camera can step
every step, never, or twice a step, and the player is the same. The check
runs a walk into a wall and around it, past a body, with two jumps and a
steady turn, at 60 FPS. It compares every field the player's step owns after
every frame, and it confirms that the three cameras differ and that the
wall pulled the camera in:

<!-- source: aethyr/walk.coax readonly -->
```jai
// The camera only reads the player (aethyr/follow.coax, book chapter 10). Through a walk and a
// run into a wall and round it, past a body, two jumps and a steady turn, at 60 FPS, the player is the same,
// bit for bit, whether its camera steps after every step, never, or twice a step. Only the
// camera's own fields differ, and the camera did meet the wall.
walker_same :: (a: Player, b: Player) -> bool {
  return a.pos.x == b.pos.x && a.pos.y == b.pos.y && a.pos.z == b.pos.z && a.vel.x == b.vel.x && a.vel.y == b.vel.y
    && a.vel.z == b.vel.z && a.face == b.face && a.on_ground == b.on_ground && a.act == b.act && a.theta == b.theta
    && a.phi == b.phi && a.max_dist == b.max_dist && a.prev_pos.x == b.prev_pos.x && a.prev_pos.y == b.prev_pos.y
    && a.prev_pos.z == b.prev_pos.z && a.prev_face == b.prev_face && a.sliding == b.sliding
    && a.jumps.waiting == b.jumps.waiting && a.jumps.taken == b.jumps.taken && a.clock.ticks == b.clock.ticks
    && a.clock.carry_ns == b.clock.carry_ns;
}
// A frame as advance_in runs it, with the camera stepped `cams` times after each player step.
frame_with :: (w: World, p: Player, frame: f32, cams: int) -> Player {
  next := p;
  next.clock = loop_accept(p.clock, cast(f64) frame);
  for i: 0..<loop_due(next.clock) {
    next.prev_pos = next.pos; next.prev_target = next.target; next.prev_face = next.face; next.prev_dist = next.dist;
    next = step(w, next, STEP);
    for c: 0..<cams { next = camera_step(w, next, STEP); }
  }
  next.clock = loop_ran(next.clock);
  return next;
}
#test "the player is the same whether its camera steps every step, never, or twice a step" {
  walls := [6] f32.[-3, 0, 4, 3, 2.5, 4.5];                    // a wall across your path, 4 m ahead
  bodies := [7] f32.[1.2, 0, 1.5, 1.2, 1.8, 1.5, 0.35];         // a robot beside it
  w: World; w.walls = walls[..]; w.bodies = bodies[..];
  start := walk_place(0, 0, 0, 0);
  runs: [3] Player;
  for c: 0..<3 { runs[c] = start; }
  frame := 1.0f32 / 60.0;
  pulled := false; stopped := false; parted := 0;
  for f: 0..<240 {
    for c: 0..<3 {
      runs[c] = walk_input(runs[c], ifx f > 120 then 1.0f32 else 0, 1, 0, 0, f > 40, f == 30 || f == 150, 0.01, 0);
      runs[c] = frame_with(w, runs[c], frame, c);
    }
    #assert walker_same(runs[0], runs[1]) && walker_same(runs[2], runs[1]);
    if runs[1].dist < f32(CAM_MAX) - 0.5 { pulled = true; }
    if runs[1].pos.z == 3.7 { stopped = true; }               // the wall's face, less your radius
    if runs[0].target.z != runs[1].target.z && runs[2].target.z != runs[1].target.z { parted += 1; }
  }
  // It walked into the wall, round it and jumped twice, and the three cameras really differed.
  #assert stopped && runs[1].pos.z > 10 && runs[1].clock.ticks == 256 && runs[1].jumps.taken == 2;
  #assert pulled && parted > 200;
  // The same run with steps_in's own camera gives the same player too.
  q := start;
  for f: 0..<240 {
    q = advance_in(w, walk_input(q, ifx f > 120 then 1.0f32 else 0, 1, 0, 0, f > 40, f == 30 || f == 150, 0.01, 0), frame);
  }
  #assert walker_same(q, runs[1]) && q.target.z == runs[1].target.z && q.dist == runs[1].dist;
  #assert q.prev_target.z == runs[1].prev_target.z && q.prev_dist == runs[1].prev_dist;
}
#forall "walker_same compares every field it names" k: 0..<21 {
  a := walk_input(walk_advance(walk_place(1, 0.5, 2, 0.3), 0.04), 0, 1, 0, 0, false, true, 0, 0);
  #assert walker_same(a, a);
  b := a;
  if k == 0 { b.pos.x += 1; } else if k == 1 { b.pos.y += 1; } else if k == 2 { b.pos.z += 1; } else if k == 3 { b.vel.x += 1; }
  else if k == 4 { b.vel.y += 1; } else if k == 5 { b.vel.z += 1; } else if k == 6 { b.face += 1; } else if k == 7 { b.on_ground = !b.on_ground; }
  else if k == 8 { b.act = Act.running; } else if k == 9 { b.theta += 1; } else if k == 10 { b.phi += 1; } else if k == 11 { b.max_dist += 1; }
  else if k == 12 { b.prev_pos.x += 1; } else if k == 13 { b.prev_pos.y += 1; } else if k == 14 { b.prev_pos.z += 1; }
  else if k == 15 { b.prev_face += 1; } else if k == 16 { b.sliding = !b.sliding; } else if k == 17 { b.jumps.waiting += 1; }
  else if k == 18 { b.jumps.taken += 1; } else if k == 19 { b.clock.ticks += 1; } else { b.clock.carry_ns += 1; }
  #assert !walker_same(a, b) && !walker_same(b, a);
  c := a; c.target.x += 1; c.dist += 1; c.spring_vel.y += 1; c.prev_target.z += 1; c.prev_dist += 1;
  #assert walker_same(a, c);    // the camera's fields are not the player's
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
4. **The dive halfway.** At share 0.5, smootherstep gives
   `0.125 × (0.5 × (3 − 15) + 10) = 0.5`, so the eye is halfway. The point it
   looks at is at share `0.5 × 1.3 = 0.65`, and
   `0.65^3 × (0.65 × (3.9 − 15) + 10) = 0.274625 × 2.785 = 0.764831` of the
   way.
5. **Top down.** Over feet at (7, 0.5, 2), the eye is at (7, 22.5, 1.99) and
   it looks at the feet.

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

The example
[aethyr/examples/one-camera-at-a-time.coax](../examples/one-camera-at-a-time.coax)
walks into a wall and jumps three times over: with the camera stepped every
step, never, and twice a step. It prints the shots each camera makes of the
last frame, and a dive from an overview down to the follow camera in 48
frames of 1/64 s. Then it tries a dive whose time isn't a number:

<!-- source: aethyr/examples/one-camera-at-a-time.coax example -->
```jai
#load "../player.coax";
#import "Basic";

// The cameras read the walker and never move it. One walk into a wall and a jump, with the
// camera stepped after every step, never, or twice a step; the shots each camera makes of the
// last frame; a dive down to the follow camera; and a dive whose time isn't a number.
main :: () {
  walls := [6] f32.[-3, 0, 4, 3, 2.5, 4.5];              // a wall 4 m ahead
  w: World; w.walls = walls[..];
  p: [3] Player;
  for c: 0..<3 { p[c] = verse_player_place(0, 0, 0, 0); }
  for f: 0..<120 {
    for c: 0..<3 { p[c] = frame_with(w, verse_player_input(p[c], 0, 1, 0, 0, f > 20, f == 60, 0, 0), 1.0f32 / 60, c); }
  }
  for c: 0..<3 {
    print("camera steps % a step: feet at (%, %, %); the camera looks at z = %\n", c, p[c].pos.x, p[c].pos.y, p[c].pos.z, p[c].target.z);
  }
  print("the same player each way: %\n", walker_same(p[0], p[1]) && walker_same(p[2], p[1]));
  names := [3] string.["first person", "follow", "top down"];
  for m: 0..<3 {
    s := verse_player_view(p[1], m);
    print("%: eye (%, %, %), looking at (%, %, %)\n", names[m], s.eye.x, s.eye.y, s.eye.z, s.at.x, s.at.y, s.at.z);
  }
  high := CameraShot.{eye = world_point(0, 38, -62), at = world_point(0, 0, 3)};
  d := shot_dive_start(high, 0.75);
  for k: 0..<49 {
    if k % 12 == 0 {
      s := shot_dive(d, verse_player_view(p[1], 1));
      print("dive % of 48: share %, eye (%, %, %)\n", k, shot_dive_share(d), s.eye.x, s.eye.y, s.eye.z);
    }
    d = shot_dive_advance(d, 0.015625);
  }
  print("halfway, the point it looks at is % of the way\n", shot_ease(0.5 * f32(SHOT_DIVE_LEAD)));
  bad := shot_dive_start(high, f32(0.0 / 0.0));
  print("a dive of NaN seconds: length %, landed at once: %\n", bad.length, shot_dive_done(bad));
}
```

```shell
coax/bin/coax run aethyr/examples/one-camera-at-a-time.coax
```

```text
camera steps 0 a step: feet at (-1.1754262e-06, 0.0, 3.7); the camera looks at z = 0.0
camera steps 1 a step: feet at (-1.1754262e-06, 0.0, 3.7); the camera looks at z = 3.6999998
camera steps 2 a step: feet at (-1.1754262e-06, 0.0, 3.7); the camera looks at z = 3.6999998
the same player each way: true
first person: eye (-1.1651814e-06, 1.6, 3.7), looking at (-1.2433748e-06, 1.1527865, 4.594427)
follow: eye (-7.6061826e-07, 3.489109, -0.23548007), looking at (-1.1046689e-06, 1.5213693, 3.6999998)
top down: eye (-1.1651814e-06, 22.0, 3.69), looking at (-1.1651814e-06, 0.0, 3.7)
dive 0 of 48: share 0.0, eye (0.0, 38.0, -62.0)
dive 12 of 48: share 0.25, eye (-7.873587e-08, 34.42758, -55.606407)
dive 24 of 48: share 0.5, eye (-3.8030913e-07, 20.744555, -31.11774)
dive 36 of 48: share 0.75, eye (-6.818824e-07, 7.0615253, -6.629074)
dive 48 of 48: share 1.0, eye (-7.6061826e-07, 3.489109, -0.23548126)
halfway, the point it looks at is 0.7648305 of the way
a dive of NaN seconds: length 0.0, landed at once: true
```

The feet are the same three times. Only the point the camera looks at
differs: a camera that never steps still looks at the start. The dive is at
share 0.5 after 24 frames, and the point it looks at is 0.7648305 of the
way, the value worked out above. A dive of `NaN` seconds is refused: its
length is 0, and it lands at once.

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
coax/bin/coax test aethyr/shot.coax --library
coax/bin/coax mutate aethyr/shot.coax
coax/bin/coax test aethyr/player.coax --library
coax/bin/coax test verse/gridwalk.coax --library
```

`aethyr/follow.coax` has fifteen checks. They cover:

- the look's wrap and limits, and the zoom's range from 1 m to 20 m;
- the spring against its closed form, and stopping on the target instead of
  crossing it;
- the closest points of two segments, a segment and a point, and two points;
- room against hand-worked distances for boxes and capsules ahead, beside,
  at a slant and inside, and for a wall just behind you;
- pulling in at once, and backing out by the same amount for one step of `dt`
  or two of `dt / 2`;
- the eye never drawn further out than the last step;
- tracking leaves the colliders exactly as they were;
- the third-person shot against `follow_at` and `follow_eye`, and the
  first-person shot looking along the follow camera's direction.

`aethyr/shot.coax` has five checks. They cover:

- the top-down shot, worked by hand;
- the free orbit's line outside, halfway in and inside, its forward and
  right directions, and a line too short to have a direction;
- a dive that starts on its first shot, lands exactly on the live one after
  48 frames, and moves at most 3.43 m a frame (smootherstep's fastest,
  1.875 times the mean speed over 87.78 m);
- dives of no length, negative length or `NaN` length are cuts, and frames
  that aren't a time don't move a dive;
- the helpers' edges: which of two equal values a minimum or clamp returns,
  seen in the sign of a zero, and the ends of the finite range.

`aethyr/player.coax` adds the check that the player is the same whether the
camera steps every step, never, or twice a step, and a check that the
comparison sees each of the 21 fields it names.

`coax mutate` kills all 189 mutants of `aethyr/follow.coax` and all 54 of
`aethyr/shot.coax`, and no mutant survives on the changed lines of
`aethyr/player.coax` or `aethyr/camera.coax`.

## Still to do

The realm migration ([chapter 64](ch064-every-realm-runs-on-aethyr.md))
moves the rest of the realms' cameras onto `CameraShot`:

- `aethyr/points.coax`'s `Shot`, which the overview, Start's dolly and the
  free orbit's realms use, becomes `CameraShot`. Today the orbit converts
  between the two at its edge.
- The homepage's dive into the Grid (`apps/openagents-web/newland-walk.coax`,
  `wk_dive`) moves onto `shot_dive`.
- `SpaceCam` (`aethyr/spacecam.coax`) makes `CameraShot`s. (Ice Cream
  Man's boom is done: `verse/icecream/camera.coax` is a `Follow` now.)
- The free orbit's easing (`verse_orbit_follow`, `verse_orbit_limit`) moves
  from the frame to the fixed step, and `verse_orbit_room` reads the realm's
  `Colliders` through `follow_room`.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md),
[2](ch002-spaces-and-units.md), [4](ch004-input-becomes-typed-actions.md),
[7](ch007-ask-the-world-where-things-are.md) and
[9](ch009-a-character-moves.md). From Coax: R12 frames and R13 clocks
([Coax roadmap](../../coax/ROADMAP.md)). Background:
[game-engine systems inventory, section 36](../../docs/research/game-engine-systems.md).
