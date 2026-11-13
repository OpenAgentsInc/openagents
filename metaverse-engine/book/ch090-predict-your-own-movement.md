# 90. Predict your own movement

> **Status: partly implemented.** The page steps its own player at once,
> keeps 64 inputs, and when the server's state for its seat arrives it starts
> from that state and replays the inputs the server hasn't stepped (#12706).
> The bookkeeping is the engine's, once (#13277):
> [metaverse-engine/net/predict.coax](../net/predict.coax) keeps the inputs, says when a
> re-run is due and holds the ledger of actions that wait for the room (261
> mutants, all killed), and [metaverse-engine/net/interp.coax](../net/interp.coax) eases
> the correction out of the drawn body over 0.12 s. Everglade's two pages
> (`everglade/client/net.coax`, `apps/openagents-web/everglade-net.coax`)
> supply their step and their row's fields and no longer carry a copy each.
> The follow camera turns toward the motion your controls ask for, never the
> corrected velocity (#12777, commit 41093d1f49). The drift check (chapter
> 100) still finds more corrections in a room than its bound allows (#12778).

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
2. Keep the input under its sequence number (`net_predict_input`,
   `NET_PREDICT_HIST` 64 of them).
3. When a `D` frame carries your seat's `p` entry and `ACK`, set the
   controller to that state (`eg_step_set`).
4. Replay every kept input after `ACK`, as the wire carried it
   (`net_predict_pending`, `net_predict_take`).
5. Compare where the replay puts your feet with where they were drawn.

When the page and the server agree, the replay lands where you were and
nothing moves. When they disagree (a push the page didn't know of, a start
the two stepped a tick apart), the difference is the correction.

### Ease the correction on the body

A correction under `NET_SNAP` 1.5 m is not applied to the drawn body at
once. The body is drawn where it was, plus an offset that decays to zero
over `NET_SMOOTH` 0.12 s. A larger one, such as a teleport, is taken at
once.

<!-- source: metaverse-engine/net/interp.coax correct -->
```jai
// Where you are drawn less where your prediction has you: what the re-runs of your inputs moved
// you by, not yet eased out.
NetCorrect :: struct { x: f32; y: f32; z: f32; }
// A re-run is done: (dx, dy, dz) is where it puts you less where you were. You stay drawn where you
// were and the difference is eased out; past NET_SNAP m (a teleport, a throw) it is taken at once.
net_correct_take :: (c: [] NetCorrect, dx: f32, dy: f32, dz: f32) {
  ox := c[0].x - dx; oy := c[0].y - dy; oz := c[0].z - dz;
  if abs(ox) + abs(oy) + abs(oz) > f32(NET_SNAP) { ox = 0; oy = 0; oz = 0; }
  c[0].x = ox; c[0].y = oy; c[0].z = oz;
}
// This frame's (dt s) offset, eased to nothing over NET_SMOOTH s.
net_correct_step :: (c: [] NetCorrect, dt: f32) -> [3] f32 {
  k := nc_clamp01(1 - dt / f32(NET_SMOOTH));
  c[0].x *= k; c[0].y *= k; c[0].z *= k;
  if abs(c[0].x) + abs(c[0].y) + abs(c[0].z) < 0.001 { c[0].x = 0; c[0].y = 0; c[0].z = 0; }
  return [3] f32.[c[0].x, c[0].y, c[0].z];
}
```

### The engine keeps the inputs and the ledger

A realm packs an input into whole numbers and hands it to the engine. The
realm keeps the room's word on its own body (those are its row's fields)
and runs the re-run as a loop over the inputs the engine still holds:

<!-- source: metaverse-engine/net/predict.coax predict -->
```jai
NET_PREDICT_HIST :: 64;     // your inputs kept for the room's word on you
NET_PREDICT_WORDS :: 8;     // an input's whole numbers at most

NetPredict :: struct {
  seq: int;                                              // your newest input's number (0: none yet)
  acked: int;                                            // the newest input the room said it stepped
  due: bool;                                             // the room's newer word waits for the re-run
  replays: int;                                          // re-runs done
  at: [NET_PREDICT_HIST] int;                            // each kept input's number (0 none), at number % NET_PREDICT_HIST
  n: [NET_PREDICT_HIST] int;                             // how many words it has
  words: [NET_PREDICT_HIST * NET_PREDICT_WORDS] int;     // its words
}

// One step's input, kept for the re-run: its number (from 1, one more each), or 0 and nothing kept
// for an input of more than NET_PREDICT_WORDS words.
net_predict_input :: (p: [] NetPredict, words: [] int) -> int #effects(all) {
  if words.count > NET_PREDICT_WORDS { return 0; }
  p[0].seq += 1;
  slot := p[0].seq % NET_PREDICT_HIST;
  p[0].at[slot] = p[0].seq; p[0].n[slot] = words.count;
  for w: 0..<words.count { p[0].words[slot * NET_PREDICT_WORDS + w] = words[w]; }
  return p[0].seq;
}
// A number taken by a command that is no input (a cast or a use, in a realm that numbers them with
// its inputs): the room steps nothing for it, so nothing is kept under it and no re-run steps it.
net_predict_number :: (p: [] NetPredict) -> int #effects(all) {
  p[0].seq += 1;
  p[0].at[p[0].seq % NET_PREDICT_HIST] = 0;
  return p[0].seq;
}
// The input numbered `seq` into out, when it is still kept: how many words it has, or -1.
net_predict_held :: (p: [] NetPredict, seq: int, out: [] int) -> int #effects(all) {
  if seq < 1 { return -1; }
  slot := seq % NET_PREDICT_HIST;
  if p[0].at[slot] != seq || out.count < p[0].n[slot] { return -1; }
  for w: 0..<p[0].n[slot] { out[w] = p[0].words[slot * NET_PREDICT_WORDS + w]; }
  return p[0].n[slot];
}
// The room's word on you names the last input of yours it stepped. True when that is newer than
// any word before: the realm keeps the word's fields, and a re-run is due.
net_predict_heard :: (p: [] NetPredict, ack: int) -> bool #effects(all) {
  if ack <= p[0].acked { return false; }
  p[0].acked = ack; p[0].due = true;
  return true;
}
net_predict_due :: (p: [] NetPredict) -> bool #effects(all) { return p[0].due; }
// The inputs the room has not stepped yet that are still kept: how many.
net_predict_pending :: (p: [] NetPredict) -> int #effects(all) {
  c := 0;
  for d: 0..<NET_PREDICT_HIST {
    q := p[0].seq - NET_PREDICT_HIST + 1 + d;
    if q > p[0].acked && p[0].at[q % NET_PREDICT_HIST] == q { c += 1; }
  }
  return c;
}
// The number of the k-th of them, oldest first, or 0 when there is no k-th.
net_predict_nth :: (p: [] NetPredict, k: int) -> int #effects(all) {
  c := 0;
  for d: 0..<NET_PREDICT_HIST {
    q := p[0].seq - NET_PREDICT_HIST + 1 + d;
    if q <= p[0].acked || p[0].at[q % NET_PREDICT_HIST] != q { continue; }
    if c == k { return q; }
    c += 1;
  }
  return 0;
}
// The k-th of them into out: how many words it has, or -1 when there is no k-th.
net_predict_take :: (p: [] NetPredict, k: int, out: [] int) -> int #effects(all) { return net_predict_held(p, net_predict_nth(p, k), out); }
// The re-run is done.
net_predict_done :: (p: [] NetPredict) #effects(all) {
  p[0].due = false;
  p[0].replays = add_sat(p[0].replays, 1);
}
// A new socket to the room: every input sent so far counts as stepped (the room starts you from
// its own word), and no re-run waits. The numbers go on from where they were.
net_predict_rejoin :: (p: [] NetPredict) #effects(all) { p[0].acked = p[0].seq; p[0].due = false; }
```

An action that waits for the room (a cast, a rest) is not movement, and
nothing of it is predicted: it plays when the room says yes
([chapter 102](ch102-ask-act-present.md)). The ledger holds what was
pressed until the room confirms it or refuses it, or until no word comes:

<!-- source: metaverse-engine/net/predict.coax ledger -->
```jai
NET_ACTIONS :: 16;          // actions a ledger holds: the pending ones and the newest finished
NET_ACT_NONE :: 0;          // an empty line
NET_ACT_PENDING :: 1;       // pressed; the room has not said
NET_ACT_CONFIRMED :: 2;     // the room took it
NET_ACT_REFUSED :: 3;       // the room refused it, with a reason
NET_ACT_TIMED_OUT :: 4;     // no word within the realm's wait

// One action: the realm's kind and request number, when it was pressed and when it ended (the
// realm's clock, ms), and the room's reason for a refusal (the realm's code).
NetAction :: struct { state: int; kind: int; seq: int; at: int; done_at: int; why: int; }
NetLedger :: struct {
  a: [NET_ACTIONS] NetAction;
  pressed: int; confirmed: int; refused: int; timed_out: int;   // how many of each, since the ledger was made
  dropped: int;                                                 // presses with no line free: every line was pending
}

// The line holding request `seq`, or -1.
net_action_find :: (l: [] NetLedger, seq: int) -> int #effects(all) {
  for i: 0..<NET_ACTIONS { if l[0].a[i].state != NET_ACT_NONE && l[0].a[i].seq == seq { return i; } }
  return -1;
}
// You pressed: request `seq` (1 or more, the realm's number) of the realm's `kind` waits for the
// room from `now`. It takes an empty line, else the line of the action that ended longest ago.
// False, and nothing changed, for a number below 1 or already in the ledger, or when every line is
// still pending (counted in `dropped`).
net_action_press :: (l: [] NetLedger, kind: int, seq: int, now: int) -> bool #effects(all) {
  if seq < 1 || net_action_find(l, seq) >= 0 { return false; }
  slot := -1;
  for i: 0..<NET_ACTIONS {
    s := l[0].a[i].state;
    if s == NET_ACT_PENDING { continue; }
    if s == NET_ACT_NONE { slot = i; break; }
    if slot < 0 || l[0].a[i].done_at < l[0].a[slot].done_at { slot = i; }
  }
  if slot < 0 { l[0].dropped = add_sat(l[0].dropped, 1); return false; }
  l[0].a[slot] = NetAction.{state = NET_ACT_PENDING, kind = kind, seq = seq, at = now};
  l[0].pressed = add_sat(l[0].pressed, 1);
  return true;
}
// The room took request `seq`: its kind, or -1 when no pending action has that number (the room's
// word came twice, or after the wait ran out).
net_action_confirm :: (l: [] NetLedger, seq: int, now: int) -> int #effects(all) {
  i := net_action_find(l, seq);
  if i < 0 || l[0].a[i].state != NET_ACT_PENDING { return -1; }
  l[0].a[i].state = NET_ACT_CONFIRMED; l[0].a[i].done_at = now;
  l[0].confirmed = add_sat(l[0].confirmed, 1);
  return l[0].a[i].kind;
}
// The room refused request `seq` for `why` (the realm's code): its kind, or -1 as above.
net_action_refuse :: (l: [] NetLedger, seq: int, why: int, now: int) -> int #effects(all) {
  i := net_action_find(l, seq);
  if i < 0 || l[0].a[i].state != NET_ACT_PENDING { return -1; }
  l[0].a[i].state = NET_ACT_REFUSED; l[0].a[i].done_at = now; l[0].a[i].why = why;
  l[0].refused = add_sat(l[0].refused, 1);
  return l[0].a[i].kind;
}
// Pending actions pressed `wait` ms ago or more time out at `now`: how many did.
net_action_expire :: (l: [] NetLedger, now: int, wait: int) -> int #effects(all) {
  c := 0;
  for i: 0..<NET_ACTIONS {
    if l[0].a[i].state != NET_ACT_PENDING || now - l[0].a[i].at < wait { continue; }
    l[0].a[i].state = NET_ACT_TIMED_OUT; l[0].a[i].done_at = now;
    l[0].timed_out = add_sat(l[0].timed_out, 1);
    c += 1;
  }
  return c;
}
// The newest action of a kind (the largest request number; the first line, should two hold one
// number): what a bar or a message shows. An empty action (NET_ACT_NONE) when the ledger holds
// none of that kind.
net_action_last :: (l: [] NetLedger, kind: int) -> NetAction #effects(all) {
  best := NetAction.{};
  for i: 0..<NET_ACTIONS {
    a := l[0].a[i];
    if a.state != NET_ACT_NONE && a.kind == kind && a.seq > best.seq { best = a; }
  }
  return best;
}
// The oldest action of a kind still pending (the smallest request number; the first line, should
// two hold one number): the one a word that names no request answers, when the room answers a
// kind's requests in the order they were pressed. An empty action when none of that kind waits.
net_action_oldest :: (l: [] NetLedger, kind: int) -> NetAction #effects(all) {
  best := NetAction.{};
  for i: 0..<NET_ACTIONS {
    a := l[0].a[i];
    if a.state == NET_ACT_PENDING && a.kind == kind && (best.state == NET_ACT_NONE || a.seq < best.seq) { best = a; }
  }
  return best;
}
// A new socket to the room: what was pending will never be answered. Each pending action (pressed
// at `now` or before) times out at `now`; how many did.
net_action_rejoin :: (l: [] NetLedger, now: int) -> int #effects(all) { return net_action_expire(l, now, 0); }
```

### The camera is the client's

The camera is drawn only by the client, from the client's own state. It is
never corrected by the server, and it must not follow anything a correction
changes. Before #12777 the follow camera turned toward the player's
velocity, which a correction sets without the player asking, so a collider
that only the server had swung the camera. It now turns toward
`walk_asked`, the motion the controls ask for:

<!-- source: metaverse-engine/move/walk.coax asked -->
```jai
// The motion your controls ask for, m/s on the flat: the wish at the pace you walk or run. A follow
// camera turns toward this, not toward the velocity, which a wall, a push or a room server's
// correction changes without you asking (#12777).
body_asked :: (p: Body) -> Vec3 #effects(all) {
  w := body_wish(p);
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
coax/bin/coax test metaverse-engine/move/walk.coax --only "walk: the intent"
coax/bin/coax test everglade/shared/everglade-step.coax --only "step:"
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
