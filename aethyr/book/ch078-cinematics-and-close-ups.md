# 78. Cinematics and close-ups

> **Status: partly implemented.** Typed flight and close-up producers share
> chapter 10's `CameraShot` and quintic blend. Hecate's Cemetery and the Garden
> use them. Native debug and release run a bounded camera take with recorded
> controls; the staged page recorder writes a provenance sidecar. A saved realm
> replay is not yet connected to that recorder. See [Still to do](#still-to-do).

An arrival can cross a whole landscape. A close-up moves a metre to let you
place a flower. Both produce a view: an eye and a point to look at. Neither
moves the player. When the player skips or finishes, the view returns to the
live camera over a declared interval.

## What this chapter teaches

- Author keys, splines, easing and intentional cuts.
- Return a typed world-space shot from a flight or close-up.
- Bound a hand-back using its distance and duration.
- Point through the shot that was actually drawn.
- Record and replay a small camera take, and distinguish that state check
  from a page clip.

## One output for every camera

[CameraShot](../shot.coax) contains two `WorldPoint` values, `eye` and `at`.
The new [flight](../shot/flight.coax) and [close-up](../shot/closeup.coax)
producers return that same type. The [legacy adapter](../shot/legacy.coax)
copies all six components to or from the existing `Point`/`Shot` record at a
page boundary. It changes no units or coordinate axes.

[The blend](../shot/blend.coax) clamps a share to its endpoints, applies
`shot_ease`, then interpolates both points. Its curve is:

```text
s(u) = 6u^5 - 15u^4 + 10u^3,  0 <= u <= 1
```

At a quarter, `s(0.25) = 0.103515625`. An eye moving from x = 0 to x = 8
therefore reaches x = 0.828125. At a half, it reaches x = 4. Both the eye and
look point use the same share. At the endpoints the function returns the
original record exactly, preserving signed zero and avoiding cancellation
when a very large coordinate returns to a small one. A NaN share returns the
starting shot.

The old flight and arranging code already used this quintic curve. The new
producers share its implementation while retaining their existing durations:
0.9 seconds for a flight hand-back and 0.7 seconds for a close-up transition.
A close-up's stored `blend` advances linearly; the camera producer eases that
share once.

## Author the actual arrival

[aethyr/cinematic.coax](../cinematic.coax) retains the flight state and key
admission. A flight holds at most 16 keys. Times must be finite, nonnegative
and strictly increasing. A key with a nonfinite shot is refused. Easing is
`EASE_LINEAR`, `EASE_SMOOTH`, `EASE_IN` or `EASE_OUT`.

The eye and look point follow Catmull–Rom splines through each pair of keys.
The neighbouring keys set their tangents. A cut holds the previous take until
the cut's time, then jumps to the cut key. `verse_flight_take` tells the page
which take is visible so it can change the scene at that boundary.

Hecate's keys now live in
[verse/planetary-villa/arrival.coax](../../verse/planetary-villa/arrival.coax).
The page and this chapter's checks load that same function. The planet centre
is (-17, -40, 120); the table shows the resulting world coordinates.

| Time (s) | Eye | Look point | Arrival rule |
| ---: | --- | --- | --- |
| 0 | (-3, -22, -70) | (-17, -40, 120) | First take |
| 3.2 | (-14, -28, 56) | (-17, -18, 120) | Ease in |
| 3.4 | (-17, 70, -95) | (-17, 0, -4) | Intentional cut |
| 6 | (-24, 24, -54) | (-17, 1, -14) | Smooth |
| 8.6 | (-17, 2.7, -32.2) | (-17, 1.5, -23) | Smooth |

The authored part ends at the last key. `camera_flight` holds that key while
blending to the live camera. A skip captures the current authored shot and
starts the same hand-back immediately. With reduced motion, or no keys,
`verse_flight_start` leaves the flight inactive and the producer returns the
live shot at once. The host continues to own input, scene changes and the
player's free orbit.

## State the movement bound

A fixed frame distance cannot cover arbitrary flight lengths, a moving target
and a fixed hand-back deadline. Chapter 10's follow-camera check concerns its
own fixture; an arrival across a landscape needs its own stated profile.

For a fixed source eye A and fixed live eye B, distance D, duration T and
frame interval dt, the quintic derivative has maximum 1.875. Therefore:

```text
frame distance <= 1.875 * D * dt / T
```

For D = 4 m, T = 0.9 s and 60 FPS, the bound is 0.138889 m per frame.
For a moving live eye, add its movement during the frame and use the largest
distance from A to that live eye over the interval. A teleport needs a
separate cut policy; easing cannot promise both a universal speed cap and
completion in 0.9 seconds.

[The path fixture](../tests/camera-take.coax) uses the actual Hecate keys and
a declared stationary live eye (-17, 2, -30), looking at (-17, 1, -23).
At 30, 60 and 144 FPS it checks:

- Every ordinary arrival interval stays below **128 m/s**, or 128/FPS metres
  per frame. The one authored cut is counted separately.
- Each of the **258, 516 and 1,239** displayed source frames can be skipped.
  Every hand-back frame respects the analytic distance bound, with 0.0001 m
  rounding allowance.
- A skip begins at the exact incoming shot, reaches the hand-worked midpoint,
  and returns all six live components exactly at `skip time + HANDBACK`.
- Reduced motion returns the live camera immediately.

The evaluator runs 32 bounded slices under its ordinary per-check budget.
The native example runs all slices together. These measurements qualify the
stated path and live camera, not every possible realm camera or browser FPS.

## Point through the close-up

[aethyr/arrange.coax](../arrange.coax) owns the vase, orbit, opening and up to
24 stems. It frames the opening from 1.05 m away, pitched down 0.6 radians,
looking 0.2 m above the opening. Opening preserves the approach direction;
closing clears the current selection and returns the view over 0.7 seconds.

The Garden passes its current live camera to `camera_closeup`. For a click
or hover, it passes the resulting drawn shot to `camera_closeup_point`.
Projection scales must be positive, finite and below the shot module's
one-million-unit limit; normalized pointer coordinates must be inside [-1, 1].
All six camera components must be finite and inside that same limit. A
refused projection clears both hover kind and stem index, so an old selection
cannot turn a malformed click into a removal.

The ray uses the same horizontal and vertical scales as the page's view
matrix. The Garden suite independently projects a spot in the vase and a
flower tip through that matrix, clicks each, then checks placement, removal
and the return control. Camera input is held while the close-up owns it.

## Record the camera take

[The worked fixture](../tests/camera-take.coax) runs 256 ticks at 64 Hz. It
records accepted control envelopes through chapter 5's `ReplaySession`.
Recording and replay both take the event before stepping the camera.

| Tick | Time (s) | Command |
| ---: | ---: | --- |
| 0 | 0 | Start the actual arrival |
| 64 | 1 | Skip |
| 128 | 2 | Open the vase close-up |
| 176 | 2.75 | Point at the opening and place a flower |
| 192 | 3 | Close the view |

Every tick's mark includes the float bits of all six shot components, the
close-up share and the flower count. Replay rebuilds the initial camera and
applies the five recorded events on their ticks. Omitting the click diverges
at tick 176. This bounded fixture does not serialize the full realm, its
network traffic, persistence or rendered pixels.

Run [a-flight-and-a-flower.coax](../examples/a-flight-and-a-flower.coax):

```shell
coax/bin/coax run aethyr/examples/a-flight-and-a-flower.coax
coax/bin/coax run aethyr/examples/a-flight-and-a-flower.coax --release
```

<!-- source: aethyr/examples/a-flight-and-a-flower.coax example -->
```jai
#load "../tests/camera-take.coax";
#import "Basic";
exit :: (code:s32) #foreign #effects(foreign);
main :: () #effects(all) {
 for fps:[3]int.[30,60,144] {
  r:=take_rate(fps,0,1);if !r.ok || r.cuts!=1 || r.maximum>=128 {exit(1);return;}
  print("% FPS: % source frames skipped; one authored cut; below 128 m/s\n",fps,r.skips);
 }
 b:[1]ReplayBacking;s:=replay_backing(b[..]);live:=take_record(s);digest:=replay_digest(s);
 print("record: % events, % ticks, % flower\n",replay_events(s),replay_tick(s),live.vase.count);
 played:=take_play(s,false);
 if !replay_matched(s) || replay_digest(s)!=digest || played.vase.count!=1 {exit(1);return;}
 print("replay: all six camera components and flower count match\n");
 played=take_play(s,true);
 print("missing click: first divergent tick %\n",replay_diverged(s));
 if replay_diverged(s)!=176 || played.vase.count!=0 {exit(1);}
}
```

Both modes exit 0 and match [the exact transcript](../fixtures/a-flight-and-a-flower.out):

```text
30 FPS: 258 source frames skipped; one authored cut; below 128 m/s
60 FPS: 516 source frames skipped; one authored cut; below 128 m/s
144 FPS: 1239 source frames skipped; one authored cut; below 128 m/s
record: 5 events, 256 ticks, 1 flower
replay: all six camera components and flower count match
missing click: first divergent tick 176
```

[closeup-refuses-projection.coax](../examples/closeup-refuses-projection.coax)
is the deliberate refusal example. It supplies a zero horizontal projection
scale with a stale stem selection. In both modes it prints
[the refusal fixture](../fixtures/closeup-refuses-projection.out) and exits 2:

```shell
coax/bin/coax run aethyr/examples/closeup-refuses-projection.coax
coax/bin/coax run aethyr/examples/closeup-refuses-projection.coax --release
```

## Capture a staged page clip

[aethyr/tools/staged-clip.coax](../tools/staged-clip.coax) delegates frame capture and encoding
to the existing [shared recorder](../../coax/web/clip.coax). Headless, muted
Chrome draws one fixed step, captures it and repeats. ffmpeg encodes those
frames. Slow capture changes the time spent recording, not the game-time
interval between frames.

The wrapper adds a checked `.mp4.json` sidecar. It always labels its source
`staged`; it refuses `--replay`. Required fields are game, a 40-character
lowercase hexadecimal commit, stage label, seed, width, height and integer
FPS. Names use 1–64 lowercase letters, digits or hyphens; seed is an unsigned
32-bit value; dimensions are 1–4,096 and FPS is 1–144. A short output buffer
publishes no partial label.

These fields describe the caller's setup. `--stage` and `--seed` label the
capture; they do not inject a stage or seed into a realm. The URL and any
`--eval` setup must actually establish it. Seed 0 below denotes the Garden's
authored setup without an external random seed. The development `probe` URL
holds W, advances 3.2 seconds, presses E and advances into the close-up. It
is a reproducible test setup, not a saved player's session.

```shell
coax/web/serve.sh apps/openagents-web --static --listen 127.0.0.1:8990
# In another terminal, use the source commit you built for COMMIT_SHA40.
coax/web/tool.sh aethyr/tools/staged-clip.coax \
  'http://127.0.0.1:8990/realms/garden?golden=1&probe&do=kw;t3.2;ke;t1' \
  /tmp/garden-close-up.mp4 --game garden --commit COMMIT_SHA40 \
  --stage close-up --seed 0 --size 960x600 --fps 30 --seconds 1 \
  --frames /tmp/garden-close-up-frames
```

Golden mode freezes the Garden's visual effects for this staged capture.
Hecate's golden mode suppresses its arrival, so it must be off when checking
that flight. A frame-for-frame repeat of this Garden setup would qualify
that staged clip only; it would not establish a clip rendered from a saved
realm replay.

## Checks and evidence

```shell
coax/bin/coax test aethyr/tests/camera-take.coax
coax/bin/coax mutate aethyr/shot/blend.coax --library --root-only
coax/bin/coax mutate aethyr/shot/flight.coax --library --root-only
coax/bin/coax mutate aethyr/shot/closeup.coax --library --root-only
coax/bin/coax mutate aethyr/tools/clip-label.coax --library --root-only
coax/bin/coax test apps/openagents-web/villa.coax --library
coax/bin/coax test apps/openagents-web/garden.coax --library
python3 psionic/tests/book.py --book aethyr
```

[The qualification record](../../docs/verification/aethyr-cameras-2026-11-06/README.md)
records the failing-before-fix checks, mutation scopes, native fixtures, page
suites, headless captures, drift check and Wasm budgets. The root mutation
scores apply to the new producer and label logic; imported legacy checks
still run. They are not a claim that every legacy cinematic or arranging
mutation is killed.

## Still to do

Connect a validated saved realm replay to the browser recorder, including its
initial state, seed, accepted external events and camera direction. Then
render it twice and compare every decoded frame, with a sidecar bound to that
record's identity. General timeline controls, orbit and dolly paths, slow
motion and reframing remain in [the clip plan](../../docs/verse/clips.md).
The chapter remains partly implemented until that replay-to-pixel path is
qualified. The native camera take and staged page captures keep their
separate evidence.
