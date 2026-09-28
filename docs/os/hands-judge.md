# Jev and hand tracking

**Status (2026-09-28):** wired and shadow-only. The code moved from the
private Coder repository on 2026-09-28, where it had run in shadow on the
owner's CoderOS host since 2026-09-18. `crates/coder-hands/src/judge.rs`
holds the window, the three questions, the floors, the deadline, and the
answers-to-action table behind the crate's `judge` feature;
`crates/coder-hands/src/seam.rs` runs one request at a time on a thread of
its own; and `crates/coder-hands/src/watch.rs` is the caller, pushing
every frame into the window, asking on each ambiguous transition, and
writing a record to the compositor's log. What has not happened is the
measurement: nobody has sat at the camera with the seam on, so no number
says whether the seam beats the rules, and until one does the seam does
not act.

The harness for it is `crates/coder-hands-measure`: one scripted minute at
a camera becomes a labeled run, the run is replayed through the rules,
the windows they cannot settle are asked about once, and the answers are
kept beside the run. A floor that moves is scored against those answers
without another request, and
[Camera and hands](camera-and-hands.md#the-measurement-that-lets-the-seam-act)
says what you run.

## The question

The gesture classifier in `crates/coder-hands/src/pose.rs` is a set of
geometric rules over 21 landmarks: a pinch is a thumb-to-index gap under
`0.35` of the palm, a fist is four curled fingers, and an open hand is
five extended fingers with a tip spread over `1.6` times the knuckle
spread. Every input those rules don't match is `HandPose::None`, and the
state machine above them treats `None` as "no gesture". A hand turned
sideways, a pinch that opens to `0.40` of a palm for three frames, a swipe
that starts as a pointer move, a palm that never quite spreads: each of
these is a gesture the person made and the desk dropped.

The owner asked whether Jev could classify the unclear cases instead of
every case being programmatic. The answer is yes for a narrow slice and no
for the frame loop, and the useful judgment is the person's intent over a
window of frames, not the geometry of one frame.

## What rules Jev out of the frame loop

Landmarks arrive at the camera's rate, 15 to 30 frames a second on the
machines measured on 2026-09-18, so the compositor has 33 to 67 ms per
frame. Jev answers in about 100 ms plus the round trip to the API, and
costs about four hundredths of a cent per request. A call per frame is
three frames of lag and about $40 an hour. Jev also evaluates state as
text: a request that carries 63 floats and asks which pose they form is a
judgment Jev is not built to make, and deterministic evidence belongs in
code.

So the per-frame path stays in code: the rules in `pose.rs`, smoothing,
hysteresis on each threshold, and a pointer that never waits on a network
call. That path decides every clear gesture, which is most of them.

## Where a judgment fits

Three things are semantic rather than geometric, and each is a judgment a
person makes in a second given a short history.

**Which gesture a borderline window is.** When a rule fires on a margin,
the answer depends on what the hand was doing before and after, not on the
one frame. A thumb-to-index distance of `0.12` after two seconds of
pointing is a pinch starting; the same distance after a held pinch is a
release starting; the same distance on a hand that is leaving the frame is
nothing. The rules see one frame. A Choice over a one-second window sees
the sequence.

**Whether the person is addressing the desk at all.** A hand that scratches
a chin, lifts a mug, or gestures while the person talks makes shapes the
rules match. The compositor has no rule for "this is not for me", and
adding one by hand means more constants. A Noul over the window, "is this
hand deliberately operating the screen", is the judgment the program most
lacks, and it's the one to measure first, because landmarks alone might
not carry the signal.

**Whether a held gesture is still held.** A drag or a two-second fist
flickers across its threshold. Without the seam, a flicker ends the drag.
A Noul over the last half second, "is this the same gesture continuing",
carries a hold across a few bad frames.

Everything else stays in code: which way a swipe went, where the pointer
is, how long a fist was held once the hold is established, and every side
effect.

## The design

One seam, one module, one request per ambiguous transition.

### The trigger

The gesture module computes a margin for every rule it applies: the
distance between the measured ratio and the constant, in units of the
constant. A transition whose every margin is wide is clear and acts at
once. A transition with a thin margin, or a window whose labels flip more
than once, is ambiguous. A pointing hand at rest produces no transitions
and no requests.

An ambiguous transition is not by itself a question. The seam asks about
one window of hand at a time, and it holds an ambiguous transition back
until the window has turned over: `judge::ASK_SPACING` windows of new
frames since the last ask, which is one. Two asks less than a window
apart read mostly the same frames and carry nearly the same state, so the
second buys almost nothing and costs a whole request.

That cap is what makes the seam measurable. Without it the trigger fired
1.1 to 3.4 times a second over three recordings of the owner's own hands
on 2026-09-18, against one request in flight under a one-second deadline,
so most of what it asked was dropped before it was answered, and the
answers that arrived were the ones that happened to arrive rather than the
interesting ones. With it the same recordings ask 0.2 to 0.6 times a
second, and if every round trip took the whole second the deadline
allows, one request in flight would carry 93% to 100% of them.

Every ambiguous window the seam doesn't ask about is written to the
transcript with the reason: `paced` for the trigger holding it back,
`in flight` for a request already running, and `closed` for a seam whose
thread is gone. A silent drop is what made the earlier numbers
untrustworthy.

`THIN_MARGIN` and the flicker count have not moved. Over the same
recordings, with the spacing in place, moving `THIN_MARGIN` anywhere
between 0.05 and 0.25 changes the ask count by nothing, and moving the
flicker count from 1 to 8 changes it by at most one ask a run: ambiguity
is dense enough that nearly every window holds one, so what the two
numbers choose is which window inside each second is asked about, not how
many. `coder-hands-measure score` prints the sweep on every run.

### The window

The window is one second of hand, sized from the rate the camera reports
rather than fixed at a count of frames. It used to be 30 frames with a
comment that called that one second, which held at 30 frames a second and
not at 15: the CoderOS daemon reports 15 to 16 in its room, where 30
frames is 2.04 seconds.

Two seconds is the wrong length to ask a question over. Every labeled
run of one rule label in the recordings of 2026-09-18 was over inside
0.87 seconds at the ninety-fifth percentile, and a swipe is over inside
`SWIPE_WINDOW`, 0.60 seconds, so a two-second window holds a gesture, the
end of it, and the start of the next. Over those recordings a 2.04-second
window covered more than one gesture in 67% to 97% of its positions,
against 55% to 78% at one second.

One second is what `judge::WINDOW_SECONDS` holds, and
`judge::window_frames` turns a rate into the count, between 8 and 45
frames. One second holds a whole gesture 95 times in 100; it's the span
the rules' own log writes; and it's `DEADLINE`, so the window a request
reads and the time it has to answer are the same length.

### The state

The state is a JSON object the module builds from the last second, and it
carries features rather than raw landmarks:

- Per frame, the rule label, the pinch distance and each finger's
  extension as ratios of palm size, the palm's facing, and the index tip's
  velocity.
- The previous acted-on gesture.
- Whether a second hand is present.

The palm every ratio divides by is the one the pinch rule divides by, the
wrist to the index knuckle. Palm-relative ratios make the state the same
at any distance from the camera. No image and no raw landmarks leave the
machine. The window is small, a few hundred tokens, so every question the
seam has goes in the same request.

`coder_hands::judge::sendable` scans the state before it leaves: a string
passes only when it's a short word of ASCII letters, digits, `_`, `-`, and
spaces, which is every pose label and gesture name, and anything else is
replaced with `[withheld]` and counted. In the private Coder repository
this was a general redaction scanner shared by every Jev seam; this
repository's `crates/jev` is the client alone, so the scan here is the
narrow one a hand window needs.

### The questions

Names here are the module's; question IDs are not sent to the model.

| Question | Type | Asks |
| --- | --- | --- |
| `intent` | Choice over `point`, `press`, `release`, `swipe_left`, `swipe_right`, `escape`, `rest`, and `none` | Which action, if any, this window carries, with contrastive `what` and `not_for` descriptions per option and `none` for a hand doing something else |
| `addressed` | Noul | Whether this hand is deliberately operating the screen |
| `continuing` | Noul | Whether this window is the same gesture as the previous acted-on one, still held |

`rest` and `none` both exist because the Choice's probabilities sum to
one, and a ranking alone can't say nothing fits.

### The thresholds

The module holds them beside the questions, and each starts at a default
for a verification seam: act only when the top probability is at least
`0.60`. Then:

- `addressed` below its floor drops the window, whatever `intent` says.
- `intent` acts only when its probability clears the floor for that
  action, and `press` and `escape` carry higher floors than `point`,
  because a wrong press costs more than a wrong pointer move.
- `continuing` above its floor holds the current gesture across the
  window and emits nothing.

Every floor is measured on labeled windows from `bench/golden/hands/`
before it acts. A starting figure is never the value.

### The deadline and the fallback

The request runs on its own thread with a deadline of one window. An
answer that arrives late is recorded and never acted on: the desk has
moved on by then, while the measurement counts an answer against what the
hand meant rather than against when it arrived. Until an answer arrives,
the deterministic result stands, which for an ambiguous window is `None`:
Jev can turn a dropped gesture into an acted one, and never the reverse
except through `addressed`, which only ever drops.

When Jev is unreachable, rate limited, over budget, or off, the module
runs the rules alone. The transcript the compositor keeps records every
request, every answer with its probabilities, the deadline it met or
missed, what the rules did instead, and the action taken.

One request runs at a time, which holds the cost to one request a round
trip and keeps a slow answer from queueing windows the desk has already
moved past.

### What leaves the machine

A window of hand features is not video, and it's not a file or a command,
but it is a record of a person's movement. The seam is off unless
`CODEROS_HANDS_JUDGE` names a rung, and the key comes from
`TYPESAFE_API_KEY` or `~/.openagents/jev.json`.

The owner settled the retention question on 2026-09-17: retention is not a
concern on their machine, so this seam may send its window. That is the
owner's decision about their own machine, not a claim about what TypeSafe
keeps. A host that turns the seam on makes the same decision for itself.

## Rollout

1. **Shadow.** The seam asks about one window of hand at a time and
   nothing changes. The transcript records the answers beside what the
   rules did and what the person did next, which is the label. A recorded
   run carries the label itself, because the prompt says what the person
   was asked for, and `coder-hands-measure` counts both against it. This
   stage is also where `addressed` proves or fails to carry signal.
2. **Suggestion.** The overlay shows Jev's top `intent` as a hint beside
   the rule label, so the person sees what the desk would have done.
3. **Act at high confidence.** Above the measured floor, an ambiguous
   window acts on `intent`. Below it, the rules stand.
4. **Tune the rules from the corpus.** The labeled windows say where the
   constants in `pose.rs` are wrong, and a constant that moves so a rule
   catches a case is a case Jev no longer sees. The seam shrinks as the
   corpus grows.

Stage 4 is the honest alternative to the whole seam: some of what looks
ambiguous is a rule normalized to image space instead of palm size, or a
rule that assumes the palm faces the camera. Better geometry fixes those
with no request at all. Jev earns its place at the intent level, where no
ratio decides.

## Where in the code

- **The gesture module** is `crates/coder-hands/src/gestures.rs`, which
  holds the state machine and the rules, and
  `crates/coder-hands/src/watch.rs`, which sits beside it, pushes each
  frame into the window, asks on each ambiguous transition, and writes
  the record.
- **The seam module** is `crates/coder-hands/src/judge.rs`, behind the
  crate's `judge` feature so the camera daemon builds without the client
  stack. It holds the state builder, the three questions with their
  criteria text, the floors, the deadline, and the answers-to-action
  table. `crates/coder-hands/src/seam.rs` beside it owns the thread, the
  one-request bound, the deadline the record reports, and where the key
  and the model come from.
- **The `jev` crate** is the client and holds no question text.
- **The fixture** is `crates/coder-hands/fixtures/jev-hands-seam.json`:
  one recorded request with fixed answers, which the crate's test serves
  from a loopback server so the test runs with no key. The landmark
  windows the rules are measured on are `bench/golden/hands/`.

## What this waits on

A shadow measurement that shows Jev agreeing with the person's next action
on ambiguous windows more often than the rules do. Without that number the
seam does not act. The procedure is in
[Camera and hands](camera-and-hands.md#the-measurement-that-lets-the-seam-act),
and it needs the owner's hands in front of the camera.
