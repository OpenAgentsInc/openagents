# 90. Predict your own movement

> **Status: partly implemented.** The page steps its own player at once,
> keeps 64 inputs, and when the server's state for its seat arrives it starts
> from that state and replays the inputs the server hasn't stepped
> (`everglade5e-face.c`, `everglade-net.coax`, #12706). The difference is
> eased out of the drawn body over 0.12 s, and the follow camera turns toward
> the motion your controls ask for, never the corrected velocity (#12777,
> commit 41093d1f49). The drift check (chapter 100) still finds more
> corrections in a room than its bound allows (#12778).

A round trip to the server takes 30 to 250 ms. If your character moved only
when the server answered, every key press would feel late. So the client
predicts: it moves its own player at once, with the server's own step
(chapter 87), and corrects itself when the server's answer arrives. This
chapter adds prediction, reconciliation and replay, and keeps the camera out
of it.

## What this chapter teaches

- Predict only your own movement. Everything else comes from the server.
- Reconcile: start from the server's state for the input it acknowledged.
- Replay the inputs the server hasn't stepped yet.
- Smooth the correction on the drawn body, and never on the camera.

## The method

The research doc (section 10) gives five steps. Each frame:

1. Step your input now, with the shared step, and send it (`I SEQ`).
2. Keep the input under its sequence number (`NET_HIST` 64 of them).
3. When a `D` frame carries your seat's `p` entry and `ACK`, set the
   controller to that state (`eg_step_set`).
4. Replay every kept input after `ACK`, as the wire carried it
   (`net_replay_input`).
5. Compare where the replay puts your feet with where they were drawn.

When the page and the server agree, the replay lands where you were and
nothing moves. When they disagree (a push the page didn't know of, a start
the two stepped a tick apart), the difference is the correction.

### Ease the correction on the body

A correction under `NET_SNAP` 1.5 m is not applied to the drawn body at
once. The body is drawn where it was, plus an offset that decays to zero
over `NET_SMOOTH` 0.12 s. A larger one, such as a teleport, is taken at
once.

<!-- source: apps/openagents-web/everglade-net.coax ease-correction -->
```jai
// The re-run is done: (dx, dy, dz) is where it puts you less where you were drawn. Small, it is eased
// out (you are drawn where you were, coming to where you are); past NET_SNAP m, taken at once.
net_fixed :: (s: [] Everglade, dx: f32, dy: f32, dz: f32) #export_c {
  n := net_of(s);
  n[0].fix_due = false; n[0].replays += 1;
  ox := n[0].off_x - dx; oy := n[0].off_y - dy; oz := n[0].off_z - dz;
  if abs(ox) + abs(oy) + abs(oz) > f32(NET_SNAP) { ox = 0; oy = 0; oz = 0; }
  n[0].off_x = ox; n[0].off_y = oy; n[0].off_z = oz;
}
// Where you are drawn less where you are, this frame (dt s): eased to nothing over NET_SMOOTH s.
net_offset :: (s: [] Everglade, dt: f32) -> Point #export_c {
  if !net_on(s) { return v3(0, 0, 0); }
  n := net_of(s);
  k := clamp01(1 - dt / f32(NET_SMOOTH));
  n[0].off_x *= k; n[0].off_y *= k; n[0].off_z *= k;
  if abs(n[0].off_x) + abs(n[0].off_y) + abs(n[0].off_z) < 0.001 { n[0].off_x = 0; n[0].off_y = 0; n[0].off_z = 0; }
  return v3(n[0].off_x, n[0].off_y, n[0].off_z);
}
```

### The camera is the client's

The camera is drawn only by the client, from the client's own state. It is
never corrected by the server, and it must not follow anything a correction
changes. Before #12777 the follow camera turned toward the player's
velocity, which a correction sets without the player asking, so a collider
that only the server had swung the camera. It now turns toward
`walk_asked`, the motion the controls ask for:

<!-- source: aethyr/walk.coax asked -->
```jai
// The motion your controls ask for, m/s on the flat: the wish at the pace you walk or run. A follow
// camera turns toward this, not toward the velocity, which a wall, a push or a room server's
// correction changes without you asking (#12777).
walk_asked :: (p: Player) -> Vec3 {
  w := wish(p);
  if !w.on { return Vec3.{}; }
  pace := ifx p.run then f32(RUN) else f32(WALK);
  return Vec3.{x = w.x * pace, y = 0, z = w.z * pace};
}
```

A headless run that held W into a server-only crate for 6 s, with about 2 m
of correction per update, turned the camera 2 degrees.

## A worked trace

The page steps inputs 101 to 106. The server's frame for tick `t` says
`ACK 103`, with your position 4 cm behind where the page had you after 103.

1. Set the controller to the server's state after input 103.
2. Replay 104, 105 and 106.
3. The replay ends 4 cm behind where the body was drawn, so the offset is
   +4 cm.
4. Over the next 0.12 s the offset decays toward 0 by `1 − dt / 0.12` each
   frame; the body slides 4 cm back and the camera doesn't turn.

## Run it

```shell
coax/bin/coax test aethyr/walk.coax --only "walk: the intent"
coax/bin/coax test apps/openagents-web/everglade-step.coax --only "step:"
```

```text
1 of 290 checks (--only 'walk: the intent'), 1 passed, 0 failed
5 of 295 checks (--only 'step:'), 5 passed, 0 failed
```

## What is left

- The drift check counts corrections over 5 cm. On localhost it found 112
  in 60 s, the largest 483 mm, and 7,438 mm at 250 ms (#12718, drift table at
  `cbb9a1ce9b`). Most come from state the page doesn't share with the server
  yet; chapters 93 to 95 remove the causes.
- The page's own step for interactions (doors, boats) is not the server's
  (chapter 87).

## Acceptance checks

- With the same inputs and no other players, a room page makes no
  correction over 5 cm in 60 s of walking (`DR_FIX_RATE`).
- No frame turns the camera more than 300 mrad (`DR_YAW_JUMP`), at 250 ms
  with 2% loss.

## Dependencies

Chapters 5 (replay), 9 (the controller), 10 (the camera), 87 (the shared
step) and 89 (acknowledgements).
