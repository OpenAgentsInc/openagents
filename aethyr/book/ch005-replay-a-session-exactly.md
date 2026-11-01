# 5. Replay a session exactly

> Engine-core task: `ec-replay` (#12609). The module is
> [aethyr/replay.coax](../replay.coax), with its small test realm in
> [aethyr/tests/replay-oracle.coax](../tests/replay-oracle.coax). Random
> streams are in [aethyr/rand.coax](../rand.coax) and strict sine, cosine
> and powers in [aethyr/math.coax](../math.coax).

A session is a function of a short record: the digest of its constructed
state ([chapter 3](ch003-state-starts-from-its-defaults.md)), a seed, and
the external events it accepted. The fixed step
([chapter 1](ch001-a-frame-has-a-fixed-step.md)) and the typed actions
([chapter 4](ch004-input-becomes-typed-actions.md)) make that possible.
Replaying the record gives the same state at every step, natively and in the
browser. You use that property for bug reports that reproduce, for
regression tests recorded from play, for saves, for networking by orders
([chapter 50](ch050-shared-state-over-the-network.md)), and for the physics
recordings in [chapter 36](ch036-record-and-replay-a-world.md).

A session takes in more than the player's controls. A model answers a
question several steps after the realm asked it. A peer's update arrives
twice, or after a newer one. If the simulation applied each of these when it
arrived, a replay would need the same arrival times, and the model and the
network again. Instead, one gate decides about every external event once,
stamps the step that applies it, and records it. Live play and replay then
apply the same record in the same way.

## What this chapter teaches

- The envelope that every accepted external event travels in.
- How the gate refuses duplicate, stale, and unknown events.
- Why a live session and its replay apply events through the same call.
- How state digests find the first step that differs.
- Why the host's math library breaks replay across hosts, and what to use
  instead.
- Random streams that don't depend on the order of draws across systems.

## The envelope

Every external event is a `ReplayEvent`:

- `tick`: the step that applies it. The gate sets it; the sender's own tick
  is ignored.
- `source`: the player's controls (`REPLAY_ACTIONS`), a model's answer
  (`REPLAY_MODEL`), or another player's update (`REPLAY_NETWORK`).
- `request`: the frame number, the request number, or the sender's sequence
  number, from 1.
- `epoch`: the source's epoch that the event belongs to.
- `words`: the accepted payload, ten 64-bit words. Floats travel as their
  bits.

<!-- source: aethyr/replay.coax envelope -->
```jai
// Where an event comes from. Controls and network updates are streams, latest-wins: the gate
// accepts only a number past the last one it accepted from that source. Model answers reply to
// the realm's requests: the gate accepts each open request's answer once, in any order.
REPLAY_ACTIONS :: 0;   // the player's controls: one Intent a frame, numbered by frame
REPLAY_MODEL :: 1;     // a model's answer to the realm's request (replay_ask)
REPLAY_NETWORK :: 2;   // another player's update, numbered by its sender
REPLAY_SOURCES :: 3;
REPLAY_WORDS :: 10;    // payload words: an Intent and its frame's steps fill all ten
REPLAY_OPEN :: 62;     // a model request is open until answered or 62 newer requests are asked

// One accepted external event. The gate sets `tick`; the sender sets the rest.
ReplayEvent :: struct {
  tick: int;                     // the step that applies it
  source: int;                   // REPLAY_ACTIONS, REPLAY_MODEL or REPLAY_NETWORK
  request: int;                  // the request, frame or sender sequence number, from 1
  epoch: int;                    // the source's epoch it belongs to
  words: [REPLAY_WORDS] int;     // the accepted payload
}

// What the gate says about an offered event.
REPLAY_ACCEPTED :: 0;
REPLAY_DUPLICATE :: 1;   // the request the source last had accepted, again
REPLAY_STALE :: 2;       // another epoch, a stream's earlier number, or a request no longer open
REPLAY_UNKNOWN :: 3;     // no such source, a request below 1, or an answer never asked for
REPLAY_FULL :: 4;        // the record has no room
REPLAY_REPLAYING :: 5;   // a live event during a replay: the record already decided
```

## The session record

A `ReplaySession` is views of the realm's own arrays, like an
`EntityStore` ([chapter 6](ch006-entities-are-born-and-die.md)): the
events in the order the gate accepted them, one state digest per step, a
gate per source, and the counters.

<!-- source: aethyr/replay.coax session -->
```jai
// The session's counters, at these places in `n` (REPLAY_COUNTERS of them).
REPLAY_MODE :: 0;        // REPLAY_RECORDING or REPLAY_PLAYING
REPLAY_TICK :: 1;        // steps run: the next step's number
REPLAY_EVENTS :: 2;      // events in the record
REPLAY_CURSOR :: 3;      // the next event a step takes
REPLAY_INITIAL :: 4;     // the constructed state's digest (chapter 3)
REPLAY_SEED :: 5;        // the session's seed (aethyr/rand.coax)
REPLAY_MARKS :: 6;       // marks kept: one state digest a step
REPLAY_SEEN :: 7;        // every step digest seen so far, as one number
REPLAY_DIVERGED :: 8;    // a replay's first step whose digest differs, plus one; 0 if none
REPLAY_MISSED :: 9;      // recorded events a replay passed without taking
REPLAY_REFUSED :: 10;    // refusals, one counter each from here (REPLAY_REFUSED + verdict)
REPLAY_COUNTERS :: 16;

REPLAY_RECORDING :: 0;
REPLAY_PLAYING :: 1;

// Each source's gate: its epoch, the last request accepted, the requests the realm asked and,
// for model answers, which of the last REPLAY_OPEN requests are answered (bit k: request asked - k).
ReplayChannel :: struct { epoch: int; last: int; asked: int; answered: int; }

ReplaySession :: struct {
  events: [] ReplayEvent;     // the record, in the order the gate accepted them
  marks: [] int;              // the state digest after each step
  channels: [] ReplayChannel; // REPLAY_SOURCES of them
  n: [] int;                  // the counters
}
```

A record starts with the digest of the constructed state. `replay_record`
takes that digest and the seed. `replay_play` takes the digest of the state
that the replay just constructed, and refuses to start if it differs. A
replay from a state built some other way fails before its first step, not
somewhere in the middle.

<!-- source: aethyr/replay.coax begin -->
```jai
// A new recording: the constructed state's digest first, then the seed.
replay_record :: (s: ReplaySession, initial: int, seed: int) {
  for i: 0..<s.n.count { s.n[i] = 0; }   // REPLAY_MODE is REPLAY_RECORDING
  replay_reset_channels(s);
  s.n[REPLAY_INITIAL] = initial;
  s.n[REPLAY_SEED] = seed;
  s.n[REPLAY_SEEN] = 2166136261;
}
// Replays the session's record from its first step. The realm builds its state again,
// constructs it as chapter 3 says, and passes that state's digest: false, and nothing
// changes, if it is not the digest the record starts with.
replay_play :: (s: ReplaySession, initial: int) -> bool {
  if initial != s.n[REPLAY_INITIAL] { return false; }
  replay_reset_channels(s);
  s.n[REPLAY_MODE] = REPLAY_PLAYING;
  s.n[REPLAY_TICK] = 0;
  s.n[REPLAY_CURSOR] = 0;
  s.n[REPLAY_SEEN] = 2166136261;
  s.n[REPLAY_DIVERGED] = 0;
  s.n[REPLAY_MISSED] = 0;
  return true;
}
replay_playing :: (s: ReplaySession) -> bool { return s.n[REPLAY_MODE] == REPLAY_PLAYING; }
replay_tick :: (s: ReplaySession) -> int { return s.n[REPLAY_TICK]; }
replay_seed :: (s: ReplaySession) -> int { return s.n[REPLAY_SEED]; }
replay_events :: (s: ReplaySession) -> int { return s.n[REPLAY_EVENTS]; }
replay_refused :: (s: ReplaySession, verdict: int) -> int { return s.n[REPLAY_REFUSED + verdict]; }
```

## The gate

`replay_offer` decides about one event. It checks these rules in order:

1. During a replay, it refuses every live event (`REPLAY_REPLAYING`). The
   record already holds the decisions.
2. It refuses an unknown source, a request number below 1, and a model
   answer to a request that the realm never asked (`REPLAY_UNKNOWN`).
3. It refuses an event from another epoch (`REPLAY_STALE`). A new epoch
   (`replay_new_epoch`) cancels every open request and every stream update
   sent before it.
4. For a model answer, it refuses a request that is no longer open: 62
   newer requests have been asked since (`REPLAY_STALE`). It refuses an
   answer to a request that already has one (`REPLAY_DUPLICATE`).
5. For a stream (controls, network updates), it refuses a number below the
   last one accepted from that source (`REPLAY_STALE`), and that number
   again (`REPLAY_DUPLICATE`).
6. It refuses an event when the record is full (`REPLAY_FULL`).
7. Otherwise it accepts the event, stamps it with the next step's number,
   and appends it to the record.

Each refusal is counted under its verdict, so a realm and its checks can see
how much a network or a model sends twice or late.

Model answers can arrive in any order: each open request's answer is
accepted once, and the record keeps the order they were accepted in. A
stream is latest-wins: a peer's position update that arrives after a newer
one is stale, because the newer position replaces it.

A delayed event isn't refused. It applies at the step after it arrives, and
the record says which step that is. The replay applies it at that step
without waiting for anything.

<!-- source: aethyr/replay.coax gate -->
```jai
// A request to a model: its number and the epoch it belongs to, and whether to send it. A
// replay asks too, so the numbers match, but sends nothing: the answer is in the record.
ReplayAsk :: struct { request: int; epoch: int; live: bool; }
replay_ask :: (s: ReplaySession, source: int) -> ReplayAsk {
  if source < 0 || source >= REPLAY_SOURCES { return ReplayAsk.{}; }
  s.channels[source].asked += 1;
  s.channels[source].answered = (s.channels[source].answered & 1152921504606846975) << 1;   // 2^60 - 1: REPLAY_OPEN bits
  return ReplayAsk.{request = s.channels[source].asked, epoch = s.channels[source].epoch, live = !replay_playing(s)};
}
replay_epoch :: (s: ReplaySession, source: int) -> int {
  if source < 0 || source >= REPLAY_SOURCES { return 0; }
  return s.channels[source].epoch;
}
// A new epoch for a source (the question changed, the peer rejoined): every event from an
// earlier epoch is stale from now on.
replay_new_epoch :: (s: ReplaySession, source: int) {
  if source < 0 || source >= REPLAY_SOURCES { return; }
  s.channels[source].epoch += 1;
}

// What the gate decides about an event, without changing anything.
replay_verdict :: (s: ReplaySession, e: ReplayEvent) -> int {
  if replay_playing(s) { return REPLAY_REPLAYING; }
  if e.source < 0 || e.source >= REPLAY_SOURCES || e.request < 1 { return REPLAY_UNKNOWN; }
  c := s.channels[e.source];
  if e.source == REPLAY_MODEL {
    if e.request > c.asked { return REPLAY_UNKNOWN; }
    if e.epoch != c.epoch || c.asked - e.request >= REPLAY_OPEN { return REPLAY_STALE; }
    if ((c.answered >> (c.asked - e.request)) & 1) == 1 { return REPLAY_DUPLICATE; }
  } else {
    if e.epoch != c.epoch || e.request < c.last { return REPLAY_STALE; }
    if e.request == c.last { return REPLAY_DUPLICATE; }
  }
  if s.n[REPLAY_EVENTS] >= s.events.count { return REPLAY_FULL; }
  return REPLAY_ACCEPTED;
}
// The gate: an accepted event goes into the record, stamped to apply at the next step; a
// refused one is counted under its verdict.
replay_offer :: (s: ReplaySession, e: ReplayEvent) -> int {
  v := replay_verdict(s, e);
  if v != REPLAY_ACCEPTED { s.n[REPLAY_REFUSED + v] += 1; return v; }
  stamped := e;
  stamped.tick = s.n[REPLAY_TICK];
  s.events[s.n[REPLAY_EVENTS]] = stamped;
  s.n[REPLAY_EVENTS] += 1;
  replay_accept(s, e);
  return v;
}
// What accepting an event changes in its source's gate: in a recording at the offer, in a replay
// at the take, so the gates agree at every step.
replay_accept :: (s: ReplaySession, e: ReplayEvent) {
  c := s.channels[e.source];
  if e.source == REPLAY_MODEL { c.answered = c.answered | (1 << (c.asked - e.request)); }
  else { c.last = e.request; }
  s.channels[e.source] = c;
}
```

`replay_ask` runs in both modes, so request numbers match. Only a live
session sends the request: `live` is false during a replay, and the realm
makes no remote call.

## A step

The realm never applies an event when it arrives. Before each step, it calls
`replay_take` until it returns no event, and applies each one. Live, the
step takes what the gate just appended. In a replay, it takes what was
recorded. The code that applies events is the same in both modes. After the
step, the realm passes its state digest to `replay_stepped`. A recording
keeps the digest as that step's mark. A replay compares the digest with the
mark and keeps the first step that differs.

<!-- source: aethyr/replay.coax step -->
```jai
// The next event this step applies, if any: call it until `ok` is false, before the step.
// A replay passes, and counts, any recorded event for a step already run.
ReplayTake :: struct { ok: bool; event: ReplayEvent; }
replay_take :: (s: ReplaySession) -> ReplayTake {
  while s.n[REPLAY_CURSOR] < s.n[REPLAY_EVENTS] && s.events[s.n[REPLAY_CURSOR]].tick < s.n[REPLAY_TICK] {
    s.n[REPLAY_CURSOR] += 1;
    s.n[REPLAY_MISSED] += 1;
  }
  if s.n[REPLAY_CURSOR] >= s.n[REPLAY_EVENTS] { return ReplayTake.{}; }
  e := s.events[s.n[REPLAY_CURSOR]];
  if e.tick != s.n[REPLAY_TICK] { return ReplayTake.{}; }
  s.n[REPLAY_CURSOR] += 1;
  if replay_playing(s) { replay_accept(s, e); }
  return ReplayTake.{ok = true, event = e};
}
// After the step: its state digest. A recording keeps it; a replay compares it with the
// recording's and keeps the first step that differs.
replay_stepped :: (s: ReplaySession, digest: int) {
  t := s.n[REPLAY_TICK];
  s.n[REPLAY_SEEN] = replay_mix_wide(s.n[REPLAY_SEEN], digest);
  if replay_playing(s) {
    differs := t >= s.n[REPLAY_MARKS] || s.marks[t] != digest;
    if differs && s.n[REPLAY_DIVERGED] == 0 { s.n[REPLAY_DIVERGED] = t + 1; }
  } else if t < s.marks.count {
    s.marks[t] = digest;
    s.n[REPLAY_MARKS] = t + 1;
  }
  s.n[REPLAY_TICK] = t + 1;
}
// The first step whose digest differs from the recording's, or -1.
replay_diverged :: (s: ReplaySession) -> int { return s.n[REPLAY_DIVERGED] - 1; }
// A replay is done and matched: every step recorded ran with the recorded digest, and every
// recorded event for those steps was taken at its step. Events accepted after the last step
// (for a step the recording never ran) are left.
replay_matched :: (s: ReplaySession) -> bool {
  c := s.n[REPLAY_CURSOR];
  taken := c >= s.n[REPLAY_EVENTS] || s.events[c].tick >= s.n[REPLAY_TICK];
  return replay_playing(s) && s.n[REPLAY_DIVERGED] == 0 && s.n[REPLAY_MISSED] == 0
    && s.n[REPLAY_TICK] == s.n[REPLAY_MARKS] && taken;
}
```

The session keeps a digest after every step, not every Kth step. A mark is
one `int`, so ten minutes at 64 steps a second take 38,400 of them. With a
mark per step, the first differing mark is the first divergent step, and no
bisection is needed.

The player's controls go through the same envelope. `replay_frame_event`
packs a frame's `Intent` and the number of steps the frame runs, and
`replay_intent` unpacks them bit for bit. The realm applies them with
`input_feed` and `input_frame`, as chapter 4 describes.

## One number for a session

`replay_digest` reduces a session to one number: its initial digest, its
seed, every event, and every step digest seen. A recording and a replay that
matches it give the same number. Two hosts that run the same session give the
same number only if every step's state is the same on both.

<!-- source: aethyr/replay.coax digest -->
```jai
// One event as one number.
replay_event_digest :: (h: int, e: ReplayEvent) -> int {
  d := replay_mix_wide(replay_mix_wide(replay_mix_wide(replay_mix_wide(h, e.tick), e.source), e.request), e.epoch);
  for w: 0..<REPLAY_WORDS { d = replay_mix_wide(d, e.words[w]); }
  return d;
}
// The session as one number: its initial digest, its seed, every event and every step digest
// seen. A recording and a replay that matched it give the same number, on any host.
replay_digest :: (s: ReplaySession) -> int {
  h := replay_mix_wide(replay_mix_wide(2166136261, s.n[REPLAY_INITIAL]), s.n[REPLAY_SEED]);
  for k: 0..<s.n[REPLAY_EVENTS] { h = replay_event_digest(h, s.events[k]); }
  return replay_mix_wide(replay_mix(h, s.n[REPLAY_TICK]), s.n[REPLAY_SEEN]);
}
// Copies a record (its events, marks and counters) into another session: a save, or a
// recording sent to another host. False if `to` is too small.
replay_copy :: (from: ReplaySession, to: ReplaySession) -> bool {
  if to.events.count < from.n[REPLAY_EVENTS] || to.marks.count < from.n[REPLAY_MARKS] || to.n.count < REPLAY_COUNTERS || to.channels.count < REPLAY_SOURCES { return false; }
  for k: 0..<from.n[REPLAY_EVENTS] { to.events[k] = from.events[k]; }
  for k: 0..<from.n[REPLAY_MARKS] { to.marks[k] = from.marks[k]; }
  for c: 0..<REPLAY_SOURCES { to.channels[c] = from.channels[c]; }
  for i: 0..<REPLAY_COUNTERS { to.n[i] = from.n[i]; }
  return true;
}
```

The digest is 32-bit FNV-1a. Equal digests don't prove equal states, but a
difference always means the states differ.

The Grid now declares execution/digest profile 2. Its canonical digest includes
the collision and query diagnostics from chapter 8, both halves of the refusal
counter and the residual's float bits. Replay refuses other profiles before
writing its destination. The independently calculated initial pin is
`1068547055`; `3864885271` remains the historical profile-1 pin. This versions
the Grid's execution contract while retaining the `ReplayEvent` envelope.

## The host's math library

The first host check of this module failed. A recorded walk gave different
session digests natively and in wasm. Its step digests were equal for 105
steps. At step 105, `vel.x` differed by one bit, in the step after the
camera turned.

The walking controller computed its direction with Coax's `sin` and `cos`
and its camera with `pow`. Coax compiles these to the C library's `sinf`,
`cosf` and `powf`. Natively that's macOS's libm, and in the browser it's
wasi-libc's. For some inputs, the two libraries round differently. Every
other operation in a step (`+`, `-`, `*`, `/`, `sqrt`, `floor`) is exact in
IEEE 754 and gives the same bits on both hosts, because both builds turn off
fused multiply-add contraction and fast math.

`aethyr/math.coax` gives `math_sin`, `math_cos` and `math_pow` built only
from those exact operations:

- Sine and cosine reduce the angle to a quarter turn in `f64`, with
  π/2 split into two parts so that the reduction is exact for angles below
  10^6 radians. Then they evaluate the Taylor series to r^13 and r^14 and
  round once to `f32`.
- `math_pow(a, b)` computes e^(b ln a) in `f64`. `ln` uses the float's
  exponent bits and the series for 2 atanh(s); `exp` reduces by ln 2 and
  scales by 2^k through the exponent bits.
- An angle past 10^6 radians, a NaN, or a base that isn't positive and
  finite is refused: sine gives 0, cosine 1, and a power 0 (or 1 when
  b = 0).

Each result is within 2 units in the last place of the C library's, and the
`f64` kernels match Python's `math` at the ends of their ranges.

`verse/player.coax` now calls these in its step, and the walk gives the same
digests on both hosts. A simulation step calls `aethyr/math.coax`, never
`sin`, `cos`, `tan`, `exp`, `log` or `pow` (INVARIANTS.md). Drawing can use
either, because what it computes isn't part of a step's state.

## Random streams

A draw from `aethyr/rand.coax` is a pure function of a seed, a stream and a
counter. Give each system or entity its own stream and count its own draws.
Then a new system, or one that draws more or less, doesn't shift another
stream's numbers. The record keeps the seed, and the counters are part of
the realm's state.

<!-- source: aethyr/rand.coax rand -->
```jai
RAND_LOW :: 4294967295;   // the low 32 bits

// a * b, wrapped at 32 bits.
rand_mul :: (a: u32, b: u32) -> u32 { return cast(u32) ((cast(u64) a * cast(u64) b) % 4294967296); }
// lowbias32: every input bit reaches every output bit.
rand_mix :: (x: u32) -> u32 {
  h := x;
  h = h ^ (h >> 16);
  h = rand_mul(h, 2146121005);   // 0x7feb352d
  h = h ^ (h >> 15);
  h = rand_mul(h, 2221713035);   // 0x846ca68b
  h = h ^ (h >> 16);
  return h;
}
rand_low :: (v: int) -> u32 { return cast(u32) (v & RAND_LOW); }
rand_high :: (v: int) -> u32 { return cast(u32) ((v >> 32) & RAND_LOW); }
// Draw number `counter` of stream `stream` under `seed`: 32 random bits.
rand_u32 :: (seed: int, stream: int, counter: int) -> u32 {
  h := rand_mix(rand_low(seed) ^ 2654435769);   // 0x9e3779b9
  h = rand_mix(h ^ rand_high(seed));
  h = rand_mix(h ^ rand_low(stream));
  h = rand_mix(h ^ rand_high(stream));
  h = rand_mix(h ^ rand_low(counter));
  h = rand_mix(h ^ rand_high(counter));
  return h;
}
// A number in [0, 1): the top 24 bits over 2^24, exact in f32.
rand_unit :: (seed: int, stream: int, counter: int) -> f32 {
  return cast(f32) (rand_u32(seed, stream, counter) >> 8) / 16777216.0;
}
// A whole number in [0, n): the 32 bits scaled by n, so no modulo bias past 1 in 2^32. A draw
// has 32 bits, so n runs from 1 to 2^32; any other n gives -1, no draw.
RAND_RANGE :: 4294967296;
rand_below :: (seed: int, stream: int, counter: int, n: int) -> int {
  if n < 1 || n > RAND_RANGE { return -1; }
  return cast(int) ((cast(u64) rand_u32(seed, stream, counter) * cast(u64) n) >> 32);
}
```

The mix is Chris Wellons' lowbias32 hash. `aethyr/tests/rand_oracle.py`
computes the same draws in Python, and the checks pin its values.

## An example

The example records a walk of 240 frames of uneven length with the walking
controller. The session includes:

- a peer whose position updates arrive twice, or after a newer one;
- a model whose answers set your pace and arrive 3 to 70 steps after the
  question, some of them twice, and some after a new epoch has started.

Then the example replays the walk, and replays it again with a fault planted
at step 150. The program is
[aethyr/examples/replay-a-walk.coax](../examples/replay-a-walk.coax), and
the walk's state and step are in
[aethyr/tests/replay-walk.coax](../tests/replay-walk.coax).

<!-- source: aethyr/examples/replay-a-walk.coax example -->
```jai
#load "../tests/replay-walk.coax";
#import "Basic";

// A recorded walk of 240 uneven frames, its replay, and a replay with a fault planted at
// step 150. The browser runs the same file (coax/web/replay-hosts.test.ts).
main :: () {
  refused := replay_walk(3);
  print("refused: % duplicate, % stale, % unknown\n", refused / 1000000, refused / 1000 % 1000, refused % 1000);
  print("recorded session: %\n", replay_walk(0));
  print("replayed session: %\n", replay_walk(1));
  print("fault planted at step 150, first divergent step: %\n", replay_walk(2));
}
```

```shell
coax/bin/coax run aethyr/examples/replay-a-walk.coax
```

```text
refused: 36 duplicate, 18 stale, 0 unknown
recorded session: 662099296
replayed session: 662099296
fault planted at step 150, first divergent step: 150
```

The gate refused 54 events. The replay sent no request to the model, took
every recorded event at its step, and gave the recorded session's digest.
The planted fault was found at the step where it starts.

## Checks

The test realm in `aethyr/tests/replay-oracle.coax` runs the same step code
live and in a replay. Live, a remote model answers each request 1 to 30
steps later, some answers twice. A peer sends updates, some twice and some
held back until after a newer one. The realm starts a new epoch every 50
steps. The check requires duplicate and stale refusals and at least one
answer applied more than a step after its request. The replay must then
match at every step and send nothing. A copy of the record, replayed on a
second session store, must match too.

<!-- source: aethyr/replay.coax check -->
```jai
#forall "delayed, duplicate and stale events: a live session and its replay agree at every step" seed: 0..<8 {
  b: [1] ReplayBacking; s := replay_backing(b[..]);
  live: [1] OracleRealm;
  remote: [1] OracleRemote;
  oracle_realm_init(live[..]);
  replay_record(s, oracle_realm_digest(live[..]), seed);
  oracle_session(s, live[..], remote[..], seed, ORACLE_STEPS);
  sent := remote[0].sent;
  #assert sent > 4 && replay_events(s) > ORACLE_STEPS / 2;
  #assert replay_refused(s, REPLAY_DUPLICATE) > 0 && replay_refused(s, REPLAY_STALE) > 0 && remote[0].accepted == replay_events(s);
  // some answer applies more than a step after its request (requests go out at steps 3, 19, 35...)
  late := 0;
  for k: 0..<replay_events(s) { e := b[0].events[k]; if e.source == REPLAY_MODEL && e.tick > 16 * (e.request - 1) + 4 { late += 1; } }
  #assert late > 0;
  recorded := replay_digest(s);
  // the replay: the same constructed state, no remote, the record's decisions
  again: [1] OracleRealm;
  oracle_realm_init(again[..]);
  #assert replay_play(s, oracle_realm_digest(again[..]));
  oracle_replay(s, again[..], remote[..]);
  #assert remote[0].sent == sent;   // the replay sent nothing
  #assert replay_matched(s) && replay_digest(s) == recorded && oracle_realm_digest(again[..]) == oracle_realm_digest(live[..]);
  // the same session from a copy of the record, on a second session store
  c: [1] ReplayBacking; v := replay_backing(c[..]);
  #assert replay_copy(s, v);
  third: [1] OracleRealm;
  oracle_realm_init(third[..]);
  #assert replay_play(v, oracle_realm_digest(third[..]));
  oracle_replay(v, third[..], remote[..]);
  #assert replay_matched(v) && replay_digest(v) == recorded && remote[0].sent == sent;
}
```

The other checks cover:

- the gate's stamp, and a sender's own tick being ignored;
- each refusal (duplicate, stale by order, stale by epoch, unknown source,
  unasked answer, full record), each counted;
- model answers accepted in any order, each once, while their request is
  open, and the same answers marked at a replay's takes;
- a record with one payload word changed, which parts from the recording at
  that event's step;
- a step taking exactly its own events in the order accepted, and a replay
  that skips an event or leaves one not matching;
- a replay from a differently constructed state being refused;
- a planted fault found at the step it starts, over eight seeds;
- a pinned digest that changes with every part of the session;
- a frame's `Intent` crossing the envelope bit for bit.

```shell
coax/bin/coax test aethyr/replay.coax --library
coax/bin/coax mutate aethyr/replay.coax --library --root-only
coax/bin/coax test aethyr/rand.coax --library
coax/bin/coax test aethyr/math.coax --library
python3 aethyr/tests/rand_oracle.py
bun test coax/web/replay-hosts.test.ts
```

| What | Result (2026-10-31) |
| --- | --- |
| `aethyr/replay.coax` mutants killed | 321 of 321 |
| `aethyr/rand.coax` mutants killed | 50 of 50 |
| `aethyr/math.coax` mutants killed | 154 of 154 |
| Recorded walk, native and wasm session digests | equal (662099296) |
| Same walk with the host's `sin`, `cos` and `pow` | parts at step 105 |

`coax/web/replay-hosts.test.ts` builds `aethyr/tests/replay-walk.coax` to
wasm the way pages are built (`coax/web/wasm.sh`: `-O2`, no fast math, no
contraction). It runs the module as the browser does, and compares every
number with the native run of the example.

## Still to do

- The Grid is the first realm that records and replays its sessions through
  this module. Its recorded session must give the same digests natively and
  in the browser ([chapter 56](ch056-the-grid.md)).
- Each migrated realm gets a replay check
  ([chapter 64](ch064-every-realm-runs-on-aethyr.md)). That includes moving
  its own random generators (`next_seed` in Everglade, `rand_next` in Belt,
  `sp_rand` in Everglade's spells) onto `aethyr/rand.coax`, and its step's
  `sin`, `cos` and `pow` onto `aethyr/math.coax`.
- Ice Cream Man's Rust traces (`verse/icecream/replay.c`), Orbit League's
  goal replays and Belt's lockstep orders move onto this record. The Rust
  traces then serve as an independent recorder.
- Everglade's guard decisions take a model's action through the ticket in
  `psionic/lib/game/decision.coax`, which decides whether the answer is
  still current. The action it hands the game then goes through
  `replay_offer` as a `REPLAY_MODEL` event, so those decisions replay too.
- A mismatch reports its step, but not yet the fields that differ. A realm
  can add that by comparing its state's fields at that step.
- The host check runs the same session on both hosts. Replaying a record
  made natively in the browser, and the reverse, needs a way to load a
  record into the wasm module.

## Contract: one envelope for every external event

> `aethyr/replay.coax` implements this contract, as the sections above
> describe. Where the module differs from the text below:
>
> - `order` is the event's place in the record: events for a tick are in
>   the order the gate accepted them.
> - `identity` is two fields, `request` and `epoch`.
> - The payload is ten 64-bit words, not bytes. A realm encodes its accepted
>   value into them.
> - A request is no longer outstanding when its epoch has ended or 62 newer
>   requests have been asked. The module has no timeout of its own; a realm
>   cancels with a new epoch.
> - Replay computes the same request numbers (`replay_ask`) but doesn't
>   compare them with the next envelope. A missing or different fact shows
>   up as a divergent step.
> - Streams (controls, network updates) are latest-wins, so a stream update
>   older than the last accepted one is stale even in the same epoch.
> - The checks below are in `aethyr/replay.coax` and
>   `aethyr/tests/replay-oracle.coax`, except "no calls in replay", where the
>   remote counts its calls instead of failing on them.

A session depends on more than the player's actions. A model answers a
guard's question, a peer sends a position, the host reports that a purchase
went through, a file finishes loading. Each of these facts arrives at a time
the realm doesn't control. To replay a session exactly, the record keeps
every accepted external fact in one envelope, and replay reads the envelopes
instead of calling anything.

### The envelope

```jai
ExternalEvent :: struct {
  apply_tick: u64       // the step at which the event takes effect
  order:      u32       // position among the events applied at that tick
  source:     SourceId  // input, a model service, a peer, the host, a store
  identity:   u64       // the request ID for a reply, or the epoch for a stream
  payload:    [u8]      // the accepted payload, in canonical bytes
}
```

The session record holds the envelopes in `(apply_tick, order)` order. An
action sample from chapter 4 is one kind of envelope: its source is `input`
and its identity is the step number. Model answers, network updates, host
results and loaded assets use the same envelope with their own sources.

### Rules

1. **Admission picks the tick.** When a fact arrives, the loop admits or
   rejects it at the next step boundary. An admitted fact gets that step as
   its `apply_tick`. A fact is never applied at a tick that has already run.
2. **The record holds accepted facts only.** A rejected fact doesn't change
   state, so it isn't replay input. A realm can log it separately for
   diagnosis.
3. **The payload is the accepted value.** The record stores the payload after
   validation and canonical encoding, not the raw bytes from the network.
   Replay doesn't validate again, so a later validator change can't change an
   old replay.
4. **One fact, one admission.** A `(source, identity)` pair is admitted at
   most once. A second copy is rejected.
5. **Stale facts are rejected.** A reply whose request ID is no longer
   outstanding (it was cancelled, superseded or timed out) is rejected. A
   stream update whose epoch is older than the current epoch is rejected.
6. **Requests come from state.** A live run issues a request only from a step,
   so the request's identity is a function of the state. Replay computes the
   same identity and checks it against the next envelope from that source.
7. **Replay makes no remote calls.** In replay, the model, network, host and
   store adapters are replaced by a reader over the record. A replay that
   needs a fact the record doesn't hold stops with a divergence report
   instead of calling out.

### Acceptance checks for the envelope

- **Delayed event.** A model answer arrives 30 steps after its request. The
  live run applies it at the admitted tick. Replay applies it at the same
  tick, and the digests match at every mark.
- **Duplicate event.** A peer update arrives twice with the same identity.
  The second copy is rejected and isn't recorded. The state equals a run that
  received one copy.
- **Stale event.** An answer arrives for a request the realm cancelled, and an
  update arrives with an old epoch. Both are rejected, neither is recorded,
  and the state equals a run in which they never arrived.
- **Reordered arrival.** Two answers arrive in the opposite order from their
  requests. The record keeps the admitted order, and replay reproduces it.
- **No calls in replay.** A replay runs with every adapter set to fail on any
  call. It completes and matches every mark.
- **Edited record.** Changing one byte of a recorded payload makes replay
  report the first divergent step at that envelope's `apply_tick`.

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md),
[chapter 3](ch003-state-starts-from-its-defaults.md),
[chapter 4](ch004-input-becomes-typed-actions.md) and
[chapter 6](ch006-entities-are-born-and-die.md) (`entity_digest` belongs in
a realm's state digest). From Coax: R15 strict math and R16 deterministic
replay and snapshots ([Coax roadmap](../../coax/ROADMAP.md)), and
determinism ([Coax chapter 16](../../coax/book/ch16-determinism.md)).
`aethyr/math.coax` does in Aethyr what R15's strict profile will do in the
compiler.
