# 93. One interpolation buffer for every remote entity

> **Status: partly implemented.** The page keeps each remote entity's last 8
> server states and draws it a delay behind the server clock, a delay that
> adapts to how late and how irregular the frames are (`everglade-net.coax`,
> #12706). Players and creatures use it well. Townsfolk use it too, but their
> per-kind send rule (chapter 91) leaves the buffer empty between updates,
> so they walk in place (#12783).

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

<!-- source: apps/openagents-web/everglade-net.coax interp-buffer -->
```jai
// A state into an entity's ring, oldest first: one no newer than the newest replaces it.
net_push :: (en: [] NetEnt, p: NetState) {
  c := en[0].ring_n;
  if c > 0 && p.t <= en[0].ring[c - 1].t { en[0].ring[c - 1] = p; return; }
  if c == NET_RING { for q: 0..<NET_RING - 1 { en[0].ring[q] = en[0].ring[q + 1]; } c -= 1; }
  en[0].ring[c] = p; en[0].ring_n = c + 1;
}
// Where entity en is at server time rt.
net_state_at :: (en: [] NetEnt, rt: int) -> NetState {
  c := en[0].ring_n;
  if c == 0 { return NetState.{}; }
  if rt <= en[0].ring[0].t { return en[0].ring[0]; }
  for q: 0..<c - 1 {
    a := en[0].ring[q]; b := en[0].ring[q + 1];
    if rt > b.t { continue; }
    f := f32(rt - a.t) / f32(hi_int(b.t - a.t, 1));
    o := ifx f < 0.5 then a else b;
    return NetState.{t = rt, x = lerp(a.x, b.x, f), y = lerp(a.y, b.y, f), z = lerp(a.z, b.z, f), yaw = a.yaw + net_turn(a.yaw, b.yaw) * f,
                     vx = lerp(a.vx, b.vx, f), vz = lerp(a.vz, b.vz, f), f = o.f, a = o.a};
  }
  p := en[0].ring[c - 1];
  ahead := f32(lo_int(rt - p.t, NET_AHEAD)) / 1000;
  return NetState.{t = rt, x = p.x + p.vx * ahead, y = p.y, z = p.z + p.vz * ahead, yaw = p.yaw, vx = p.vx, vz = p.vz, f = p.f, a = p.a};
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

- Townsfolk: the generic record at a fixed tick (#12783). The drift check's
  walking-in-place probe measured 8% of walking frames offline against 33%
  in a room on localhost and 55% at 250 ms.
- Teleports slide through the buffer instead of cutting to the new place
  (chapter 91's discontinuity flag).

## Acceptance checks

- A remote walker's drawn position never stands still while its walking
  clip plays, within the drift check's `DR_STALL` bound.
- Recorded states with jitter up to the delay draw no backward step.
- A teleport flagged on the record is drawn at its new place on the next
  frame.

## Dependencies

Chapters 1 (drawing between steps) and 91 (the record).
