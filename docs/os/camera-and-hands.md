# Camera and hands

A CoderOS desktop can have one daemon that owns the camera, a gesture
module in the Coder compositor that turns hand landmarks into desk input,
and a Jev seam beside the gesture rules. This page says how the three fit
together, what the compositor's log shows, and how to measure the seam.

The code moved from the private Coder repository on 2026-09-28:
`crates/coderos-camera`, `crates/coder-hands`, `crates/coder-hands-measure`,
the hands module of `crates/coder-compositor`, and the NixOS modules
`camera.nix`, `hands.nix`, and `recording-hud.nix` under
`os/modules/coderos/`. `coderos.desktop.camera.enable` turns the daemon and
the circle on, and `coderos.desktop.hands.enable` turns hand tracking on.

## The camera daemon

`coderos-camera` is one daemon that owns the camera node, started with the
session ahead of the camera circle. It opens the node once and serves every
consumer:

- A `v4l2loopback` node, `/dev/video10` by default, loaded at boot with
  `exclusive_caps=1` and a fixed `video_nr`, for `mpv`, a video client, and
  a sandboxed browser.
- A recording of the camera to a file through `ffmpeg`, with the same
  receipt `screen-record` leaves.
- Hand landmarks, one JSON line a frame on
  `$XDG_RUNTIME_DIR/coderos-camera/hands.sock`.

Its grant is `/etc/coderos/camera.json`, and its verbs go over
`$XDG_RUNTIME_DIR/coderos-camera/control.sock`: `coderos-camera status`,
`record start <path>`, `record stop`, `hands on`, `hands off`, and
`outputs`. [`crates/coderos-camera/README.md`](../../crates/coderos-camera/README.md)
describes the protocol. `nix build ./os#coderos-camera` builds the package,
with the static ONNX Runtime fetched by digest.

Before the daemon, the camera circle's `mpv` held `/dev/video0` and
nothing else could read it. `camera-overlay` reads the loopback node as
YUYV and waits for the daemon to report it up, and Super+C toggles the
circle's window while the daemon keeps the camera.

## Hands at the desk

With hands on, the Coder compositor turns the camera daemon's landmarks
into desk input. A host that turns hands on names `hands` among the
compositor's launchers in `/etc/coderos/compositor.json`, so the session
starts with hands driving the desk; Super+H turns them off and on again.
The compositor asks the daemon for `hands on` when they go on and
`hands off` when they go off, so the tracker runs only while something
reads it. `coder-desk status` prints `hands: on` or `hands: off`.

The compositor reads one JSON line a frame from the socket
`CODEROS_HANDS_SOCKET` names, which `os/bin/coder-compositor-session` sets
to `$XDG_RUNTIME_DIR/coderos-camera/hands.sock`, the path the daemon
derives for itself. It reads on a thread of its own and connects again
while the daemon is away. The session starts the compositor before it
starts the daemon, so the compositor's first `hands on` reaches nothing.
The compositor asks again before each attempt at the socket until the
daemon takes the verb, and again after a daemon goes away, because a
daemon that comes back comes back with its tracker off. To see whether the
tracker is running, read `coderos-camera --json status` rather than the
compositor's own state.

Each frame's landmarks are smoothed toward the reading with a 0.09-second
time constant, and the pointer is mirrored, so the hand moves it the way a
mouse does. The outer 15 percent of the frame at each edge maps to the
screen's edge, so a hand near the edge of the picture reaches the corner
of the screen.

| Gesture | What the desk does | The rule and its constant |
| --- | --- | --- |
| Point: the index finger out, the middle finger curled | Moves the pointer | The index tip past 1.30 palms from the wrist and the middle tip inside 1.20 (`POINT_EXTENDED`, `POINT_CURLED`) |
| Pinch: the thumb on the index finger | Presses the left button, and drags while it holds | A thumb-to-index gap under 0.35 of the palm (`PINCH_PALM`); the button lets go when that rule's margin passes -0.57 (`PINCH_RELEASE`), which puts the release at 0.55 of a palm, and the band between the two is the hysteresis |
| Palm: a flat hand carried sideways | Switches desks, one a swipe | The palm crossing 0.25 of the frame inside 0.60 seconds, then 0.80 seconds before the next (`SWIPE_DISTANCE`, `SWIPE_WINDOW`, `SWIPE_COOLDOWN`) |
| Fist: every finger curled, held | Sends Escape to the focused window, once a hold | Two seconds of fist (`FIST_HOLD`) |

`crates/coder-hands/src/gestures.rs` holds the rules and the constants,
and each constant's doc comment says what the recordings of 2026-09-18
said about it. Two things those recordings say that no constant can fix:

- **The camera's rate is not one number.** Three recorded sessions ran at
  30, 30, and 15 frames a second, and the CoderOS daemon reports 15 to 16
  in its own room. Every rule is written in seconds, and the seam's window
  is sized from the rate rather than fixed at a frame count, so both hold
  at either rate.
- **The bottom of the screen is out of reach.** While the hand steered,
  its index tip ran from 0.21 to 0.88 across the frame, which `EDGE` maps
  onto very nearly the whole screen, but only from 0.15 to 0.53 down it,
  so the lower half of the screen never came under the pointer. One
  constant at both edges of both axes can't fix that; a separate vertical
  band would, and nobody has built one.

While hands drive the desk, the compositor draws them over every window on
the focused screen: the 21 landmarks, the bones between them, and a
caption in the bottom-left corner with the gesture and its margin. Its log
carries each press, release, swipe, and Escape as it happens, and once a
second the last second of labels with their margins.

## The seam in shadow

`CODEROS_HANDS_JUDGE=shadow` turns on the Jev seam beside the gesture
rules, which [Jev and hand tracking](hands-judge.md) designs. The hands
module sets it when `coderos.desktop.hands.judge` is on. In shadow, the
compositor asks about a window whose transition the rules decided on a
thin margin, or whose labels keep flipping, and records the answer.
Nothing the desk does changes, the pointer waits on no request, and the
overlay reads as it did. The seam acts only at the `act` rung, which waits
on the measurement below.

The compositor's log shows what the seam does. One line at the start says
the seam is on, which rung it runs, and which model it asks. One line for
each answer carries the record:

```text
hands judge: {"window":30,"pose":"pinch","margin":0.04,"deadline_ms":1000,
"met_deadline":true,"report":{"intent":"press","confidence":0.86,
"probabilities":[["press",0.86],["point",0.14]],"addressed":0.91,
"continuing":0.12},"rules":["point"],"action":"press"}
```

That is one line in the file, wrapped here to fit. `pose` and `margin` are
what the rules read on the newest frame, `rules` is what the desk did on
it, `report` is the answer with a probability for every option,
`met_deadline` says whether the answer arrived inside the one-second
window, and `action` is what the seam would have done. A request that
failed carries `failed` with the reason and no `report`. An answer that
missed the deadline is recorded the same way and never acts on the desk.

A window the rules couldn't settle and the seam didn't ask about gets a
line of its own, with a `skipped` field:

```text
hands judge: {"window":30,"pose":"pinch","margin":0.04,"rules":["point"],
"skipped":"paced"}
```

`paced` means the seam asked less than a window of hand ago. `in flight`
means a request was already running, and `closed` means the seam's thread
is gone. One line when tracking stops adds the four up, so a session says
what it didn't ask as well as what it did.

The log is the compositor's standard output, which
`os/bin/coder-compositor-session` also writes to a file named after the
terminal the session holds: the session on tty1 writes
`$XDG_RUNTIME_DIR/coder-compositor.tty1.log`. Each start of the compositor
empties its own file, so copy it before you restart the session:

```sh
cp "$XDG_RUNTIME_DIR/coder-compositor.tty1.log" ~/hands-shadow-$(date +%F-%H%M).log
```

A session with no key, in `TYPESAFE_API_KEY` or `~/.openagents/jev.json`,
says so once and runs the rules alone.

## The measurement that lets the seam act

The seam acts only when its answers match what you meant more often than
the rules do, on the windows the rules couldn't settle. The gesture
constants were chosen by reading rather than by measuring as well. Nobody
has run this, so no number exists, the seam doesn't act, and no constant
has met a hand. `coder-hands-measure` is the harness, and your part of it
is about a minute at a camera:

1. Log in to the Coder compositor session and open a terminal there.
   Hands start on, and Super+H turns them off and on.
2. Run `coder-hands-measure record`. It prints one prompt at a time and
   counts you in before each: point, pinch, a flat sweep to your left,
   and a held fist, twice through. Follow the prompts and type nothing.
   It takes about 55 seconds and writes the run to
   `~/.openagents/hands/runs/`.
3. Make each gesture at the edge of where it works, because that is where
   the rules can't settle: a pinch that barely closes, a palm that turns
   as it crosses, a fist that opens for a frame.
4. Run `coder-hands-measure score ~/.openagents/hands/runs/RUN`. It
   reports, for each gesture, how often the rules did what the gesture
   owes and how long they took, how often they did nothing, how often
   their label flipped, how the deciding margins are spread, and how many
   windows they couldn't settle. `--windows` lists those windows and says
   which of them the trigger asked about. It also reports how often the
   seam is asked, how many ambiguous windows the trigger held back, and
   what the same run would ask at a sweep of the trigger's three numbers.
5. Run `coder-hands-measure ask RUN` once. It sends each of those windows
   to the seam and writes the answers beside the run as
   `RUN.answers.jsonl`. Score the run again, and the report adds the
   seam's half: what it answered, at what probabilities, whether each
   answer met the deadline, whether acting on the answers would have
   matched the prompt more often than the rules did, and what each floor
   would act on.
6. Keep the run. A run copied under `bench/golden/hands/runs/` is scored
   again every time a constant in `crates/coder-hands/src/gestures.rs` or
   a floor in `crates/coder-hands/src/judge.rs` moves, so nobody has to
   repeat the recording.
7. Write the counts and the date into a dated file under `docs/os/`, and
   name that file in `crates/coder-hands/src/judge.rs` beside the floors,
   which is where a measured value belongs.

On a CoderOS host the recorder asks the camera daemon to track and leaves
it tracking, and it reads the same socket the desk reads, so the desk
keeps working while you record. A run in an empty room records frames,
reports no gesture, and asks nothing.

The floors in `judge.rs` are starting points rather than measured values:
`0.60` for an action, `0.75` for a press or an Escape, and `0.50` for each
of the two other questions. The same runs set them.

The harness already reports two numbers. First, the window is one second
of hand, sized from the rate the frames arrive at, because the camera in
the owner's room answers about 15 times a second. Every report prints the
frames a window holds, what that is in seconds, and how many windows cover
more than one gesture; read those before the seam's counts, because a
window that holds two gestures is being asked the wrong question.

Second, the ask rate. Replayed over three recordings of the owner's own
hands on 2026-09-18, the unspaced trigger asked 1.1 to 3.4 times a second
against one request in flight under a one-second deadline, so most of
what it asked was dropped before it was answered. The trigger now holds
an ambiguous window back until the window has turned over, which brings
that to 0.2 to 0.6 a second and records every window it held. Read the
`paced` count in the report and the `skipped` lines in the log together
with the asks: a seam that drops most of what it asks can't be measured
honestly.
