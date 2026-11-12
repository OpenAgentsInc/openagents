# 93. One interpolation buffer for every remote entity

> **Status: partly implemented.** The page keeps each remote entity's last 8
> server states and draws it a delay behind the server clock, a delay that
> adapts to how late and how irregular the frames are (#12706). The buffer is
> the engine's since #13277: [aethyr/net/interp.coax](../net/interp.coax), with
> Everglade's entity record beside it in `everglade/client/net-core.coax`. Since wire 3 every remote entity is drawn from this one buffer,
> townsfolk included: 100 ms or more as jitter grows, velocity for
> `NET_AHEAD` ms on a gap, then held. Townsfolk drawn walking in place fell
> from 31-47% of their walking frames to 0% headless (#12783, commit
> edd9601102). A new `LIFE` or a `WARP` starts an entity's buffer again.

A remote player's state arrives 30 times a second, late and unevenly. Drawn
as it arrives, it stutters. This chapter adds one buffer that every remote
entity is drawn from: players, townsfolk, creatures and zones alike.

## What this chapter teaches

- Drawing remote entities a short delay in the past.
- Estimating the server clock from pings.
- An adaptive delay from lateness, jitter and interval.
- What to do when the buffer runs dry.

## The method

Each remote entity keeps a ring of its last `NET_RING` 8 states, oldest
first, each stamped with its server time. Each frame the page draws every
entity at the server time `now − delay`, between the two states that
bracket it, the yaw turning the short way. Past the newest state, the
entity goes on by its velocity for at most `NET_AHEAD` 150 ms, then holds.

<!-- source: aethyr/net/interp.coax interp-buffer -->
```jai
// A state into a ring holding n states, oldest first: one no newer than the newest replaces it. The
// ring is the realm's (NET_RING long at most); the new count comes back.
net_ring_push :: (ring: [] NetState, n: int, p: NetState) -> int {
  c := n;
  if c > 0 && p.t <= ring[c - 1].t { ring[c - 1] = p; return c; }
  if c == ring.count { for q: 0..<c - 1 { ring[q] = ring[q + 1]; } c -= 1; }
  ring[c] = p;
  return c + 1;
}
// State p gone on by its velocity to server time rt, `ahead` ms at most, then held.
net_on_from :: (p: NetState, rt: int, ahead_ms: int) -> NetState {
  ahead := f32(nc_lo(nc_hi(rt - p.t, 0), ahead_ms)) / 1000;
  return NetState.{t = rt, x = p.x + p.vx * ahead, y = p.y + p.vy * ahead, z = p.z + p.vz * ahead, yaw = p.yaw, vx = p.vx, vy = p.vy, vz = p.vz, f = p.f, a = p.a};
}
// Where the ring's entity (its states, oldest first) is at server time rt; it goes on past its
// newest state for ahead_ms at most.
net_ring_at :: (ring: [] NetState, rt: int, ahead_ms: int) -> NetState {
  c := ring.count;
  if c == 0 { return NetState.{}; }
  if rt <= ring[0].t { return ring[0]; }
  for q: 0..<c - 1 {
    a := ring[q]; b := ring[q + 1];
    if rt > b.t { continue; }
    f := f32(rt - a.t) / f32(nc_hi(b.t - a.t, 1));
    o := ifx f < 0.5 then a else b;
    return NetState.{t = rt, x = nc_lerp(a.x, b.x, f), y = nc_lerp(a.y, b.y, f), z = nc_lerp(a.z, b.z, f), yaw = a.yaw + net_turn(a.yaw, b.yaw) * f,
                     vx = nc_lerp(a.vx, b.vx, f), vy = nc_lerp(a.vy, b.vy, f), vz = nc_lerp(a.vz, b.vz, f), f = o.f, a = o.a};
  }
  // past the newest: on by its velocity for ahead_ms (a frame late or lost), then held
  return net_on_from(ring[c - 1], rt, ahead_ms);
}
// Where an entity went on to past its newest state (lead: that state; on: drawn past it last
// frame), and the difference from where the states after it put it (x, y, z), eased out.
NetLead :: struct { lead: NetState; on: bool; x: f32; y: f32; z: f32; }
// Drawn past its newest state last frame, and newer states came since: where they put it differs
// from where it went on to (a turn or a stop it made meanwhile). That difference is drawn and eased
// out over NET_EASE s, so it doesn't jump back; past NET_SNAP m (a new life, a jump), taken at once.
// ring: its states (one at least); p: where the ring has it at rt now; dt: the frame's seconds.
net_lead_ease :: (l: [] NetLead, ring: [] NetState, p: NetState, rt: int, dt: f32, ahead_ms: int) {
  c := ring.count;
  newest := ring[c - 1];
  if c < 2 { l[0].x = 0; l[0].y = 0; l[0].z = 0; }
  else if l[0].on && newest.t > l[0].lead.t {
    q := net_on_from(l[0].lead, rt, ahead_ms);
    ex := l[0].x + q.x - p.x; ey := l[0].y + q.y - p.y; ez := l[0].z + q.z - p.z;
    if abs(ex) + abs(ey) + abs(ez) > f32(NET_SNAP) { ex = 0; ey = 0; ez = 0; }
    l[0].x = ex; l[0].y = ey; l[0].z = ez;
  }
  l[0].lead = newest; l[0].on = rt > newest.t;
  k := nc_clamp01(1 - dt / f32(NET_EASE));
  l[0].x *= k; l[0].y *= k; l[0].z *= k;
  if abs(l[0].x) + abs(l[0].y) + abs(l[0].z) < 0.001 { l[0].x = 0; l[0].y = 0; l[0].z = 0; }
}
```

**The server clock.** The page's monotonic clock plus an offset. The offset
starts from the relay's first `y` frame and is then taken from the ping
with the lowest round trip of the last 8, `clock + rtt / 2 − mono`.

**The delay.** After each new tick the page updates three running means,
each by a tenth: how late states arrive, how much that lateness varies
(jitter), and the interval between them. The delay is

```jai
delay = clamp(NET_BASE + late + interval + 2 × jitter, NET_LOW, NET_HIGH)   // 20 + ..., 80 to 300 ms
```

Without the lateness term, the buffer ran dry whenever two states arrived
together, and the others stood and then lurched.

## Why one buffer is not enough alone

The buffer can only draw what it is given. A walker sent only after 0.3 m,
with no velocity, has one state every 233 ms against an 80 ms delay: the
buffer holds the last pose while the walk cycle plays. The fix is in what
is sent (chapter 91: state at a fixed tick with velocity), not in a second
buffer for townsfolk.

## Run it

The page's network checks run with Everglade's:

```shell
coax/bin/coax test apps/openagents-web/everglade5e.coax --only "net:"
```

## What is left

- Generalizing this buffer for other realms remains part of #12845.
  Fixed-tick townsfolk rows and LIFE/WARP resets already run. Historical
  walking-in-place measurements describe the earlier counterexample, not
  the current status; chapter 100 measures the current artifact.

## Acceptance checks

- A remote walker's drawn position never stands still while its walking
  clip plays, within the drift check's `DR_STALL` bound.
- Recorded states with jitter up to the delay draw no backward step.
- A teleport flagged on the record is drawn at its new place on the next
  frame.

## Dependencies

Chapters 1 (drawing between steps) and 91 (the record).
