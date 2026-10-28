# 27. Building a voice system in pieces

The earlier chapters taught Coax with small programs, and Chapter 26 put
several features together in one boundary. In this chapter we'll look at
something bigger: the voice system OpenAgents uses to let people, NPCs and
agents talk to each other. It's a real system, not a teaching toy. It runs
on a Mac today, it has a phone client in progress, and its decisions about
who may be heard and what speech costs are held to the same rules as the
wallet.

We won't read it top to bottom. Instead we'll build it up in the order a
team would: the protocol first, then one 20 ms frame of audio, then turns,
interruptions, decisions, buffering, the two `#authority` files, the C face,
the server, the phone, and the tests that hold it all together. At each
step we'll write a small runnable version of the idea in
`coax/book/examples/ch27_*.coax`, run it, and then look at how the real code
in `metareal/` does the same thing with more care.

By the end you'll have seen most of what the book has covered so far used
in one place: `#import_proto`, fixed arrays and views, `#run`, `#test` and
`#forall`, `#authority` with `coax mutate`, `#export_c` libraries, and C and
Swift hosts that stay thin because Coax decides.

> Note: everything shown here was run with `coax0` 0.10.0 on an Apple
> silicon Mac. The Coax engine, the C face, the voice host and its tests
> exist and pass. The iOS client in `apps/openagents-ios/OpenAgents/Voice/`
> is still being finished, and nothing is deployed to a server yet. Where
> something isn't built, this chapter says so. The system was changing
> while the chapter was written (on 2026-10-28), so the transcripts and
> step counts here are a snapshot; rerun the commands to see today's.

## The problem

OpenAgents has a 3D world, the Verse, where players walk around, NPCs have
lines, and agents can be talked to. It also has a phone app and a web app.
The owner asked for voice that works the same way everywhere: tap one
toggle, then just talk, and see the right things happen. No push-to-talk,
no "wait for transcription" button.

That sounds like one feature. It's really several problems at once:

- Something has to decide, every 20 milliseconds, whether you're speaking,
  and when you've finished. That decision is most of what makes a
  conversation feel good or bad.
- If you talk over a reply, the reply has to stop within a frame, and any
  of its audio still in flight must never play.
- Speech recognition and synthesis use Google's models (Speech-to-Text v2
  with `chirp_3`, and Text-to-Speech with Chirp 3 HD voices), because they
  work on every surface. But the phone must never hold Google credentials
  or talk to Google directly, so a server of ours sits in between.
- Speech costs money per millisecond recognized and per character
  synthesized. Something must decide whether a turn may run at all.
- Many kinds of speaker share the system: a player, an NPC, an agent, a
  narrator. None of them gets a special code path.

The team had built voice about twenty times before, in other stacks. The
lesson that mattered most was that working parts died with the apps they
lived in. Code written as Coax libraries with C headers can sit under the
phone app, a Linux server and later a browser, the way the camera and
wallet already do. That's the argument for writing the core in Coax.

Here's the shape we're building toward:

```text
iOS app (Swift host)  ──WebSocket, one voice.v1alpha1.Frame per binary message──►  voice host (C host)
  mic (VoiceProcessingIO) → oa_voice_uplink (Coax) → Audio frames up                 Coax engine (same libs)
  speaker ← playback ← oa_voice_downlink (Coax) ← Audio frames down                  → Google STT / TTS (REST)
                                                                                      → agent (dev: Gemini or echo)
```

The same Coax libraries run on both ends. The hosts do only what Coax
can't: talk to audio hardware, open sockets, and make HTTPS calls.

| Piece | File | What it decides or does |
| --- | --- | --- |
| The schema | `proto/openagents/voice/v1alpha1/voice.proto` | Every message on the wire, with its bounds |
| The engine | `metareal/voice.coax` | Resampling, levels, voice activity, end of turn, barge-in, jitter buffer, gain, features, decisions |
| The wire | `metareal/voice-wire.coax` | Frame codecs, generated from the schema |
| The floor | `metareal/voice-floor.coax` | Consent, who holds the floor, media epochs (`#authority`) |
| The meter | `metareal/voice-meter.coax` | Free tier, prices, whether a turn may run (`#authority`) |
| The C face | `metareal/platform/voice.h`, `voice.c` | A stable C API over the four libraries |
| The voice host | `metareal/voice-host/` | WebSocket server, sessions, Google calls, a dev client |
| The reply path | `metareal/voice-host/voice-reply.coax` | When a partial may stand as the final transcript, and where a reply splits into sentences (`#authority`, newest and still changing) |
| The phone | `apps/openagents-ios/OpenAgents/Voice/` | Capture, playback, the socket, the UI (in progress) |

## Start with the protocol

The first piece isn't Coax at all. It's the schema. That order matters
more than it looks. At least three programs speak this wire: the
phone, the voice host, and the dev client; a web client and an external
decider come later. If each one invented its own numbering for "turn
state: trailing" or its own idea of how long a transcript may be, we'd
find out at runtime, on a phone, in someone's ear. Writing the contract
first means every number, every enum value and every bound is decided once,
in one reviewed place, before any code depends on it.

The schema is `openagents.voice.v1alpha1`. It has two halves: primitives
that every speaker shares (`VoiceProfile`, `Speaker`, `Utterance`,
`TurnEvent`, `Route`) and the wire itself, one `Frame` per WebSocket
binary message. Here's one message from the wire half, the one we'll spend
the most time with later:

Filename: proto/openagents/voice/v1alpha1/voice.proto

```protobuf
// A bounded micro-change to a stream, from apply_from_sequence onward.
message FrameDecision {
  option (openagents.options.v1alpha1.data_class) = DATA_CLASS_INTENT;
  option (openagents.options.v1alpha1.max_message_bytes) = 256;
  Stream stream = 1;
  optional uint32 apply_from_sequence = 2;
  optional uint64 media_epoch = 3;
  FrameAction action = 4;
  // Gain change in dB, clamped to -24..12.
  optional float gain_db = 5 [(openagents.options.v1alpha1.unit) = "dB"];
  // For DUCK: how far down, clamped to 0..40.
  optional float duck_db = 6 [(openagents.options.v1alpha1.unit) = "dB"];
  // Playback rate, clamped to 0.8..1.25. Omitted means unchanged.
  optional float playback_rate = 7;
  // For HOLD_TURN: extra silence to wait before ending the turn, 0..2000.
  optional uint32 hold_ms = 8 [(openagents.options.v1alpha1.unit) = "ms"];
  // 0 to 1. Below the receiver's floor, the decision is ignored.
  optional float confidence = 9;
  // Who decided: "voice.coax", "decisions:<model>".
  optional string decider = 10 [(openagents.options.v1alpha1.max_bytes) = 64];
  // Ignore this decision if it arrives after this frame sequence.
  optional uint32 deadline_sequence = 11;
}
```

*Listing 27-1: The `FrameDecision` message, with its clamps written down
before any code*

Notice what the comments promise: gain is clamped to -24..12 dB, a hold
to at most 2000 ms, and a decision past its deadline is ignored. Those are
requirements for the code we haven't written yet. Notice too that almost
every field is `optional`. In proto3 that gives the field *presence*: a
receiver can tell "gain 0 dB" from "no gain sent". For a decision that
difference matters, because "set gain to 0" and "leave the gain alone" are
different instructions.

Chapter 15 showed how a schema reaches Coax. The repository's
`proto/modules.json` gained a `voice` profile naming this file, and
`scripts/proto.sh` builds the checked-in descriptor
`proto/descriptors/voice.pb` from it. Then a single directive in the wire
file generates every codec:

Filename: metareal/voice-wire.coax

```jai
#import_proto "../proto/descriptors/voice.pb" as V;
```

You can see what that directive produces by asking the compiler for it
directly. From the repository root:

```sh
$ coax/bin/coax proto proto/descriptors/voice.pb --prefix V | wc -l
    3285
$ coax/bin/coax proto proto/descriptors/voice.pb --prefix V | grep -c "^V_[A-Za-z_]* ::"
681
```

That's 3,285 lines and 681 generated declarations: a getter and a setter per field, a
`V_Frame_decode`, a `V_Frame_case_body` that says which `oneof` body a
frame carries, and the bounds checks from the schema's options. None of it
was written by hand, and none of it can drift from the schema, because it
is regenerated from the same descriptor every build.

The rest of `voice-wire.coax` is a thin mapping between the generated
types and plain structs the C face can fill in. Each builder follows the
rule Chapter 15 described for generated setters: an edit reads one scratch
buffer and writes the other, so the two alternate.

Filename: metareal/voice-wire.coax

```jai
pick :: (k: int, a: [] u8, b: [] u8) -> [] u8 { return ifx k % 2 == 0 then a else b; }
```

```jai
enc_route :: (w: WRoute, a: [] u8, b: [] u8) -> V_Route {
  m := V_Route_new(); k := 0;
  if w.turn.count > 0 { m = V_Route_set_turn(m, w.turn, pick(k, a, b)); k += 1; }
  if w.target.count > 0 { m = V_Route_set_target(m, w.target, pick(k, a, b)); k += 1; }
  if w.reason.count > 0 { m = V_Route_set_reason(m, w.reason, pick(k, a, b)); k += 1; }
  return m;
}
```

*Listing 27-2: Building a `Route` with the generated setters, alternating
two scratch buffers*

There's one thing the wire checks can't do in the evaluator. The setters
check at runtime that the input and output buffers don't overlap, and the
compile-time evaluator can't observe native addresses (Chapter 26 hit the
same wall, as error E0404). So the file's `#test` blocks decode frames, and
encoding is tested natively by `metareal/test_voice.sh`, which we'll run at
the end of the chapter. The comment in the file says exactly that:

```jai
// The setters need a runtime address check (bytes_overlap), so encoding runs
// only natively: metareal/test_voice.sh round-trips every kind through the C
// face and has protoc decode each encoded frame. These checks decode those
// exact protoc-verified frames.
```

The byte arrays in those decode tests came from frames that `protoc`
decoded independently, so the Coax decoder is checked against a second
implementation, not against itself.

## Levels and voice activity on one 20 ms frame

Now for audio. Everything in the engine happens one frame at a time, and a
frame is 20 ms. The engine processes at 16 kHz, so a frame is 320 samples
of signed 16-bit PCM. Phones capture at 48 kHz, 44.1 kHz or 24 kHz, so the
first step for each frame is resampling to 16 kHz.

The resampler uses a windowed-sinc filter, and its table is a nice use of
`#run` from Chapter 9. The table is computed while compiling, so the
program contains only the finished numbers, and two `#assert`s check the
table before any code uses it:

Filename: metareal/voice.coax

```jai
make_kernel :: () -> [KN] f32 {
  k: [KN] f32;
  for i: 0..<KN { k[i] = f32(kernel_at(f64(i) / f64(KRES))); }
  return k;
}
KERNEL :: #run make_kernel();
#assert KERNEL[0] > 0.79 && KERNEL[0] < 0.81;
#assert KERNEL[KN - 1] == 0.0;
```

*Listing 27-3: The resampling filter, built at compile time*

Once we have 320 samples at 16 kHz, we measure them. Let's write a small
version ourselves. We want three numbers per frame: the RMS level, the
peak level, and the zero-crossing rate. Levels are in dBFS, decibels
relative to full scale, so 0 is the loudest a sample can be and digital
silence is minus infinity, which we'll pin at -120.

Filename: examples/ch27_meter.coax

<!-- checked-source: examples/ch27_meter.coax -->
```jai
#import "Basic";

// One 20 ms frame at 16 kHz.
FRAME :: 320;

// Power (0..1 of full scale squared) to dBFS, with -120 for silence.
db_from_power :: (p: f64) -> f32 {
  if p <= 0.000000000001 { return -120.0; }
  db := f32(4.342944819032518 * log(p));
  if db < -120.0 { return -120.0; }
  if db > 0.0 { return 0.0; }
  return db;
}

Levels :: struct { rms_db: f32; peak_db: f32; zcr: f32; }

// RMS and peak in dBFS, and the fraction of neighbouring samples that
// change sign: a cheap cue for voicing.
measure :: (x: [] s16) -> Levels {
  l: Levels;
  if x.count == 0 { l.rms_db = -120; l.peak_db = -120; return l; }
  p: f64 = 0;
  peak := 0;
  crossings := 0;
  sign := 0;
  for i: 0..<x.count {
    v := cast(int) x[i];
    p += f64(v * v);
    a := ifx v < 0 then -v else v;
    if a > peak { peak = a; }
    if v != 0 {
      now := ifx v > 0 then 1 else -1;
      if sign != 0 && now != sign { crossings += 1; }
      sign = now;
    }
  }
  full: f64 = 32768.0 * 32768.0;
  l.rms_db = db_from_power(p / f64(x.count) / full);
  l.peak_db = db_from_power(f64(peak * peak) / full);
  l.zcr = ifx x.count > 1 then f32(crossings) / f32(x.count - 1) else 0.0;
  return l;
}

// Fill a frame with a sine at `hz`, amplitude `amp` (0..1 of full scale).
tone :: (out: [] s16, hz: f32, amp: f32) {
  for i: 0..<out.count {
    out[i] = cast(s16) int(amp * 32767.0 * sin(2.0 * 3.14159265 * hz * f32(i) / 16000.0));
  }
}

#test "silence, full scale and a quiet tone" {
  x: [FRAME] s16;
  l := measure(x[..]);
  #assert l.rms_db == -120.0 && l.peak_db == -120.0 && l.zcr == 0.0;
  for i: 0..<FRAME { x[i] = ifx i % 2 == 0 then 32767 else -32767; }
  l = measure(x[..]);
  #assert l.rms_db > -0.01 && l.zcr == 1.0;
  // 0.1 of full scale: -20 dBFS peak, 3 dB lower RMS for a sine.
  tone(x[..], 900.0, 0.1);
  l = measure(x[..]);
  #assert abs(l.peak_db + 20.0) < 0.1 && abs(l.rms_db + 23.0) < 0.1;
  #assert measure(x[0..<0]).rms_db == -120.0;
}

main :: () {
  x: [FRAME] s16;
  l := measure(x[..]);
  print("silence:        rms % dBFS, peak % dBFS, zcr %\n", l.rms_db, l.peak_db, l.zcr);
  tone(x[..], 300.0, 0.1);
  l = measure(x[..]);
  print("300 Hz at 0.1:  rms % dBFS, peak % dBFS, zcr %\n", l.rms_db, l.peak_db, l.zcr);
  tone(x[..], 3000.0, 0.5);
  l = measure(x[..]);
  print("3 kHz at 0.5:   rms % dBFS, peak % dBFS, zcr %\n", l.rms_db, l.peak_db, l.zcr);
}
```

*Listing 27-4: A level meter for one 20 ms frame*

A few lines deserve a closer look. `4.342944819032518 * log(p)` is
10·log10(p) written with the natural log: decibels of a power ratio.
Squaring a sample as an `int` before converting to `f64` keeps the sum
exact for 16-bit values. The crossing counter skips zero samples, so a
signal that touches zero doesn't count twice. And the `#test` checks the
two facts any audio person would reach for: a sine at 0.1 of full scale
peaks at -20 dBFS, and its RMS sits 3 dB lower.

Run it:

```sh
$ coax run examples/ch27_meter.coax
silence:        rms -120.0 dBFS, peak -120.0 dBFS, zcr 0.0
300 Hz at 0.1:  rms -23.012287 dBFS, peak -20.002121 dBFS, zcr 0.03448276
3 kHz at 0.5:   rms -9.03153 dBFS, peak -6.02113 dBFS, zcr 0.37304077
```

A 300 Hz tone crosses zero 600 times a second, which is about 0.0375 per
sample at 16 kHz. Over one 320-sample frame we measured 0.034. The 3 kHz
tone crosses ten times as often. Voices sit low on this scale; hiss and
fricatives like "s" sit high.

Our `measure` is very nearly the one in `metareal/voice.coax`. That's
deliberate: once you understand this listing you can read the real one.

### From levels to "is someone speaking?"

A level on its own can't tell you whether someone is speaking. A quiet
room in one place is a noisy room somewhere else. So the engine keeps a
running estimate of the noise floor, and judges each frame against it. The
floor follows quiet frames down quickly, and creeps up slowly only while
nobody is talking:

Filename: metareal/voice.coax

```jai
// The floor follows quiet frames down fast and rises slowly (at most
// 0.12 dB a frame) only while nobody is speaking, inside -90..-20 dBFS.
noise_next :: (noise: f32, rms: f32, speaking: bool) -> f32 {
  n := noise;
  if rms < n { n += (rms - n) * 0.3; }
  else if !speaking { n += lo32(rms - n, 6.0) * 0.02; }
  return clampf(n, -90.0, -20.0);
}
```

*Listing 27-5: The noise floor estimate*

Then a frame's speech likelihood combines three cues: how far it sits above
the floor, how loud it is at all, and how often it crosses zero. Here is a
standalone version, with the `#forall` from the real file:

Filename: examples/ch27_vad.coax

<!-- checked-source: examples/ch27_vad.coax -->
```jai
#import "Basic";

clampf :: (x: f32, low: f32, high: f32) -> f32 {
  if x != x { return low; }
  if x < low { return low; }
  if x > high { return high; }
  return x;
}

sigmoid :: (x: f32) -> f32 {
  y := clampf(x, -30.0, 30.0);
  return 1.0 / (1.0 + exp(-y));
}

// How likely a frame is speech, from three measurements: how far it sits
// above the noise floor, how loud it is at all, and how often it crosses
// zero (hiss crosses far more often than a voice).
speech_likelihood :: (rms: f32, noise: f32, zcr: f32) -> f32 {
  p_snr := sigmoid((rms - noise - 10.0) / 2.0);
  p_abs := sigmoid((rms + 50.0) / 3.0);
  p_z := ifx zcr <= 0.3 then 1.0 else clampf(1.0 - (zcr - 0.3) * 2.5, 0.2, 1.0);
  return clampf(p_snr * p_abs * p_z, 0.0, 1.0);
}

#forall "a probability that rises with level" rms: -120..<1, noise: -90..<-19 {
  p := speech_likelihood(f32(rms), f32(noise), 0.1);
  #assert p >= 0.0 && p <= 1.0;
  #assert speech_likelihood(f32(rms) + 1.0, f32(noise), 0.1) >= p;
}

#test "voice, hiss and the floor" {
  #assert speech_likelihood(-25.0, -60.0, 0.1) > 0.95;
  #assert speech_likelihood(-60.0, -60.0, 0.1) < 0.01;
  #assert speech_likelihood(-25.0, -60.0, 0.6) < speech_likelihood(-25.0, -60.0, 0.1);
}

main :: () {
  noise: f32 = -60.0;
  levels := [5] f32.[-60.0, -52.0, -48.0, -40.0, -25.0];
  for i: 0..<5 {
    print("rms % dBFS over a % dBFS floor: speech %\n", levels[i], noise, speech_likelihood(levels[i], noise, 0.1));
  }
  print("the same -25 dBFS as hiss (zcr 0.6): speech %\n", speech_likelihood(-25.0, noise, 0.6));
}
```

*Listing 27-6: Speech likelihood from three cues, with a rule checked over
every whole-dB level and floor*

Each cue goes through a sigmoid, so it's a soft 0-to-1 vote rather than a
hard threshold, and the product needs all three to agree. The sigmoid's
input is clamped first, so `exp` never sees anything extreme. The
`#forall` states the two rules any decider downstream relies on: the
result is a probability, and turning the level up never makes a frame
*less* likely to be speech. It tries every whole-dB level from -120 to 0
against every floor from -90 to -20:

```sh
$ coax test examples/ch27_vad.coax
ok   #forall a probability that rises with level (8591 cases, 2029012 steps)
ok   #test voice, hiss and the floor (1 case, 469 steps)
2 checks, 2 passed, 0 failed
$ coax run examples/ch27_vad.coax
rms -60.0 dBFS over a -60.0 dBFS floor: speech 0.00023053659
rms -52.0 dBFS over a -60.0 dBFS floor: speech 0.091236666
rms -48.0 dBFS over a -60.0 dBFS floor: speech 0.4830516
rms -40.0 dBFS over a -60.0 dBFS floor: speech 0.95909256
rms -25.0 dBFS over a -60.0 dBFS floor: speech 0.99975604
the same -25 dBFS as hiss (zcr 0.6): speech 0.24993901
```

The engine counts a frame as speech when this likelihood reaches 0.6 (the
`speech_threshold` default). The same loud frame scored as hiss drops to
0.25 and doesn't count.

That last line is also a lesson the real engine learned from recorded
speech while this chapter was being written. One of the test recordings
begins "Let's take the long way through the garden", and the "s" of
"Let's" is hiss: high zero crossings, no voicing. Scored like our example,
it stopped counting as speech partway through a word. The current
`speech_likelihood` in `metareal/voice.coax` keeps the voicing penalty for
frames near the floor, but lets a frame that stands far above the floor
through as an unvoiced consonant:

Filename: metareal/voice.coax

```jai
  // High zero crossings mean noise, unless the frame stands far above the
  // floor: then it is an unvoiced consonant (the s of "Let's"), which is speech.
  voiced := ifx zcr <= 0.3 then 1.0 else hi32(0.2, 1.0 - (zcr - 0.3) * 2.5);
  p_z := hi32(voiced, sigmoid((rms - noise - 20.0) / 3.0));
```

The `#forall` didn't need to change: the result is still a probability
that never falls as the level rises. That's the value of writing the rule
separately from the formula. You can change the formula freely, and the
rule tells you whether you broke anything it promised.

> Note: this is an energy-and-voicing detector, not a learned model. The
> design decided to start here, test on recorded speech, and add a small
> learned turn model only when recordings show these rules failing.

## End of turn as a small state machine

Knowing that one frame is speech isn't the same as knowing that a *turn*
has started or ended. A cough isn't a turn. A pause to think in the middle
of a sentence shouldn't end one. So the engine runs a small state machine
over the per-frame yes/no: idle, speaking, trailing (silence after speech,
not yet decided), and back to idle.

State machines are where `#forall` shines, because the inputs are small
and finite. Let's build a toy one first. A turn starts after three speech
frames in a row, and ends after three silent ones.

Filename: examples/ch27_turns.coax

<!-- checked-source: examples/ch27_turns.coax -->
```jai
#import "Basic";

IDLE :: 1;
SPEAKING :: 2;
TRAILING :: 3;

EV_START :: 1;
EV_END :: 2;

MIN_SPEECH :: 3;   // frames of speech before a turn starts
END_SILENCE :: 3;  // frames of silence that end it

Turn :: struct {
  state: int = IDLE;
  run: int;      // speech frames in a row while idle
  silence: int;  // silence frames in a row while in a turn
}

Step :: struct { t: Turn; ev: int; }

// One 20 ms frame: was it speech?
step :: (t0: Turn, speech: bool) -> Step {
  o: Step;
  o.t = t0;
  if o.t.state == IDLE {
    if !speech { o.t.run = 0; return o; }
    o.t.run += 1;
    if o.t.run < MIN_SPEECH { return o; }
    o.t.state = SPEAKING;
    o.ev = EV_START;
    return o;
  }
  if speech {
    o.t.state = SPEAKING;
    o.t.silence = 0;
    return o;
  }
  o.t.state = TRAILING;
  o.t.silence += 1;
  if o.t.silence >= END_SILENCE {
    o.t = Turn.{};
    o.ev = EV_END;
  }
  return o;
}

// Every 12-frame pattern of speech and silence: starts and ends alternate,
// a start comes only after MIN_SPEECH speech frames in a row, and an end
// only after END_SILENCE silent ones.
#forall "starts and ends pair up" pattern: 0..<4096 {
  t: Turn;
  open := false;
  run := 0;
  quiet := 0;
  for i: 0..<12 {
    speech := (pattern >> i) & 1 == 1;
    if speech { run += 1; quiet = 0; } else { run = 0; quiet += 1; }
    o := step(t, speech);
    if o.ev == EV_START { #assert !open && run >= MIN_SPEECH; open = true; }
    if o.ev == EV_END { #assert open && quiet >= END_SILENCE; open = false; }
    #assert open == (o.t.state != IDLE);
    t = o.t;
  }
}

main :: () {
  frames := "..ss.sss.ss....s..";
  t: Turn;
  for i: 0..<frames.count {
    o := step(t, frames[i] == 's');
    t = o.t;
    if o.ev == EV_START { print("frame %: turn starts\n", i); }
    if o.ev == EV_END { print("frame %: turn ends\n", i); }
  }
}
```

*Listing 27-7: A toy end-of-turn machine, checked on every 12-frame
pattern*

`step` is a pure function: it takes the old state and one frame, and
returns the new state and an event. It never reads a clock or a global, so
we can drive it from a string in `main` and from an integer's bits in the
check. The `#forall` treats each of the 4096 numbers below 2^12 as twelve
frames, bit `i` meaning "frame `i` is speech", and keeps its own simple
count of speech and silence runs to compare against. That's every possible
12-frame history, not a sample of them.

```sh
$ coax test examples/ch27_turns.coax
ok   #forall starts and ends pair up (4096 cases, 3622267 steps)
1 check, 1 passed, 0 failed
$ coax run examples/ch27_turns.coax
frame 7: turn starts
frame 13: turn ends
```

Read the string along with the output. The two `s` frames at 2 and 3
aren't enough, and the silent frame at 4 resets the count. Frames 5, 6
and 7 start the turn. The single silent frame at 8 doesn't end it, because
speech resumes at 9. Frames 11, 12 and 13 are three in a row, so the turn
ends at 13. The lone `s` at 15 never becomes a turn.

Now let's make the mistake that's easiest to make in a machine like this.
Suppose we forget to reset the run when an idle frame is silent, and write
`if !speech { return o; }` instead. Save that as `ch27_turns_broken.coax`
and run the checks:

```sh
$ coax test ch27_turns_broken.coax
FAIL #forall starts and ends pair up
ch27_turns_broken.coax:59:27: error[E0406]: assertion failed
      if o.ev == EV_START { #assert !open && run >= MIN_SPEECH; open = true; }
                            ^
  note: in #forall starts and ends pair up
  counterexample: pattern = 11
1 check, 0 passed, 1 failed
```

Pattern 11 is binary `1011`: speech, speech, silence, speech. The broken
machine counted three speech frames across the gap and started a turn,
which our rule says needs three in a row. The `#forall` found the
smallest pattern that shows it.

The real machine in `metareal/voice.coax` has the same shape with more
inputs. It works in milliseconds rather than frames, has a maximum turn
length, can be told to hold the turn open a little longer or to end it
now, and tracks how much near-speech energy is in the trailing silence.

It also made a different choice from ours about exactly the case our
broken version got wrong. Real speech has short silences inside words:
the closure before a "t" or a "k" is a frame or two of near silence. A
machine that resets on any quiet frame can miss the start of a word, so
the real one deliberately bridges short gaps in an onset, and says so:

Filename: metareal/voice.coax

```jai
turn_step :: (t0: Turn, speech: bool, above: bool, p: f32, end_now: bool, end_silence: u32, min_speech: u32, max_turn: u32) -> TurnOut {
  o: TurnOut;
  o.t = t0;
  if o.t.state == TURN_ENDED { o.t = Turn.{turn_ms = t0.turn_ms}; }
  if o.t.state == TURN_IDLE {
    // An onset starts at the first frame above the floor and is confirmed
    // by min_speech of speech frames; up to ONSET_GAP quiet frames in a row
    // (stops, the closure before a consonant) don't break it, and it must
    // confirm within ONSET_SPAN frames.
```

```jai
// Every 12-frame speech pattern, with a 3-frame minimum, a 3-frame end and a
// 10-frame cap: starts and ends pair up; a start needs 3 speech frames since
// the onset began, where more than 2 quiet frames in a row (or a turn's end)
// begin a new onset; an end needs the silence or the cap; only turn frames
// forward.
#forall "turn machine" pattern: 0..<4096 {
```

*Listing 27-8: The start of the real turn machine, and the rule its check
states over every 12-frame pattern*

`ONSET_GAP` is 2 frames and `ONSET_SPAN` is 25 (500 ms). The new `above`
input says the frame is more than 6 dB over the noise floor, which can
start an onset before the speech likelihood is sure. So pattern 11, which
our toy check rejects, is a legal start for the real machine under its
check's settings. Neither is wrong. What matters is that each check states
the rule its machine is meant to follow, so the rule is a decision someone
wrote down, not an accident of the code. The real check's model counts
speech frames since the onset began, just as its comment says.

The real check runs the machine with small settings (a 60 ms minimum, a
60 ms end, a 200 ms cap) so that every interesting transition fits in
twelve frames. The production defaults are larger: 160 ms of speech to
start, 700 ms of silence to end, and a 30-second cap. The real file also
has `#test`s that drive the whole engine with generated audio, such as
"a tone burst is one turn, with pre-roll" and "a too-short blip is not a
turn", and one for the onset rule itself, "an onset: above-floor frames
lead, gaps are bridged, slow onsets expire".

That test name mentions pre-roll. Because a turn only starts after 160 ms
of speech, the start of the first word has already gone by when the
machine decides. The engine keeps the last 16 frames in a ring, and
`metareal_voice_preroll` hands the onset back so recognition hears the
first syllable. You'll see `pre-roll 160 ms` in the client's output later.

## Barge-in and media epochs

When the agent is replying and you start talking over it, two things must
happen. Playback must stop at once. And any reply audio already on its
way, in a socket buffer or the network or the jitter buffer, must never
play, even if it arrives a second later.

The first is easy. The second needs a name for "the audio before the cut".
The engine calls it the *media epoch*: a counter that only goes up, and
goes up exactly when playback is cut. Every audio packet carries the epoch
it was made in. After a cut, anything stamped with an older epoch is
dropped on arrival. Here's the cut itself:

Filename: metareal/voice.coax

```jai
// Cut playback: a new media epoch, nothing buffered, no duck or pause, and
// pending decisions of the old epoch die with it.
cut :: (s: [] VoiceState) {
  s[0].epoch = s[0].epoch + 1;
  jitter_flush(s);
  s[0].playing = false;
  s[0].recent_play = 0;
  s[0].ducked = false;
  s[0].auto_duck = false;
  s[0].paused = false;
  s[0].quiet_frames = 0;
}
```

*Listing 27-9: Cutting playback moves the epoch forward and empties the
buffer*

If you've read Chapter 23, this is the same idea as a request epoch: a
completion for an old request is ignored by comparing a counter, without
having to find and cancel every piece of work in flight.

Deciding *when* to barge in is part of the uplink path. Speech over
playback has to last as long as the minimum speech time, so a click or a
breath doesn't cut the reply, and it happens once per turn:

```jai
  // Barge-in: speech over playback for the minimum speech time, once.
  decided := false;
  if playing && speech && !s[0].barge_lock {
    s[0].barge_run = addsat(s[0].barge_run, 1);
    if s[0].barge_run * FRAME_MS >= s[0].cfg.min_speech_ms {
      if self_decide(s, STREAM_DOWNLINK, ACT_BARGE, 0, 0.0, 0, 1.0) {
        ev |= EV_BARGE_IN | EV_EPOCH | EV_DECISION;
        decided = true;
        s[0].barge_lock = true;
        s[0].barge_run = 0;
      }
    }
  } else if !speech { s[0].barge_run = 0; }
```

*Listing 27-10: Barge-in in `metareal_voice_uplink`*

Notice `self_decide`. The engine doesn't cut playback directly here. It
makes a decision, the same kind of decision an outside decider could send,
and passes it through the same gate. That's the subject of the next
section.

Across the wire, the phone sends `Control BARGE_IN` with its new epoch and
how many reply samples had played. The host cuts its own engine, adopts
the client's epoch if it's ahead, bumps the floor's epoch, and reports
`TurnEvent INTERRUPTED`. Here's that happening with the dev client, which
talks over the reply with a second recording (`--barge`):

```sh
$ build/voice-client --url ws://127.0.0.1:8899/ --barge fixtures/wait-stop.wav fixtures/hello-foreman.wav
welcome session=s2-5d393b95 provider=local in=16000 out=24000 max_turn_ms=30000 free_ms_left=595160
turn-start t1 at 560 ms of input (pre-roll 160 ms)
turn t1 STARTED epoch=1
route t1 -> foreman (default)
turn t1 PARTIAL "I heard 880 milliseconds of speech. Thank you." epoch=1
turn t1 PARTIAL "I heard 1880 milliseconds of speech. Thank you." epoch=1
turn t1 PARTIAL "I heard 2080 milliseconds of speech. Thank you." epoch=1
turn-end t1 (2600 ms)
turn t1 FINAL "I heard 2080 milliseconds of speech. Thank you." epoch=1
meter recognized_ms=9680 synthesized_chars=56 free_ms_left=590320 free_chars_left=4944 owed_msat=0
meter recognized_ms=9680 synthesized_chars=112 free_ms_left=590320 free_chars_left=4888 owed_msat=0
speech u1 "You said: I heard 2080 milliseconds of speech. Thank you." rate=24000 epoch=1
latency final_ms=0 first_audio_ms=1 (from TURN_END)
barge-in epoch=2 played_samples=27816
turn-start t2 at 4160 ms of input (pre-roll 160 ms)
turn t1 INTERRUPTED epoch=2
turn t2 STARTED epoch=2
route t2 -> foreman (default)
turn t2 PARTIAL "I heard 880 milliseconds of speech. Thank you." epoch=2
turn-end t2 (1400 ms)
turn t2 FINAL "I heard 880 milliseconds of speech. Thank you." epoch=2
meter recognized_ms=10560 synthesized_chars=112 free_ms_left=589440 free_chars_left=4888 owed_msat=0
meter recognized_ms=10560 synthesized_chars=167 free_ms_left=589440 free_chars_left=4833 owed_msat=0
speech u2 "You said: I heard 880 milliseconds of speech. Thank you." rate=24000 epoch=2
latency final_ms=0 first_audio_ms=2 (from TURN_END)
speech_end u2 played_samples=75600 epoch=2
turn t2 ENDED epoch=2
summary finals=2 partials=4 speeches=2 speech_ends=1 interrupted=1 audio_in=24 downlink_samples=114000 stale=0 features=0 decisions=0 meters=4 errors=0 peak=9000
```

This run, from `metareal/voice-host/`, used the host's offline provider,
which we'll meet later; it "recognizes" by reporting how long the turn
was. The first reply was cut
after 27,816 of its samples at 24 kHz, a little over a second. The epoch
went from 1 to 2, the first turn was reported as interrupted, and the
second turn was heard and answered under epoch 2.

## Frame features and frame decisions

The owner asked for frame-by-frame control: every 20 ms frame, in both
directions, should be observable, and something should be able to
micro-steer the stream from a given frame on. Today that something is the
engine's own built-in policy. Later it could be a decision model reached
over the network.

So the engine describes every frame as a `FrameFeatures` (the levels,
noise floor, speech likelihood, turn state, playback state and so on, all
plain numbers and no words), and accepts `FrameDecision`s from any
decider. The key rule is in the schema's comment: a decision is never
awaited. Audio keeps flowing under the last decision until a new one
arrives.

Why be so strict? An audio frame comes due every 20 ms whether or not
anyone has decided anything. If the audio path ever waited for a decider,
a slow model or a dropped packet would become a stall you can hear. With
the rule, a slow or absent decider can only fail to steer, never stall.

That makes the gate the interesting part. A decision might arrive late,
or for an epoch that's already been cut, or with nonsense numbers in it.
Let's write a small gate that handles those cases:

Filename: examples/ch27_gate.coax

<!-- checked-source: examples/ch27_gate.coax -->
```jai
#import "Basic";

HAS_GAIN :: 1;
HAS_DEADLINE :: 2;
HAS_EPOCH :: 4;

Decision :: struct {
  has: u32;       // which optional fields were sent
  gain_db: f32;   // clamped to -24..12
  epoch: u64;     // the media epoch it was made for
  deadline: u32;  // ignore it after this frame sequence
  confidence: f32;
}

Stream :: struct {
  epoch: u64 = 1;
  next: u32;      // the next frame's sequence
  gain_db: f32;   // the gain audio flows under now
}

clampf :: (x: f32, low: f32, high: f32) -> f32 {
  if x != x { return low; }
  if x < low { return low; }
  if x > high { return high; }
  return x;
}

// Clamp what was sent; a NaN gain counts as not sent.
clamp :: (d0: Decision) -> Decision {
  d := d0;
  if (d.has & HAS_GAIN) != 0 {
    if d.gain_db != d.gain_db { d.has &= 7 ^ HAS_GAIN; d.gain_db = 0; }
    else { d.gain_db = clampf(d.gain_db, -24.0, 12.0); }
  } else { d.gain_db = 0; }
  d.confidence = clampf(d.confidence, 0.0, 1.0);
  return d;
}

// The one gate. 0: kept. Otherwise why it was dropped.
DROP_CONFIDENCE :: 1;
DROP_EPOCH :: 2;
DROP_LATE :: 3;
gate :: (s: [] Stream, d0: Decision) -> int {
  d := clamp(d0);
  if d.confidence < 0.5 { return DROP_CONFIDENCE; }
  if (d.has & HAS_EPOCH) != 0 && d.epoch != s[0].epoch { return DROP_EPOCH; }
  if (d.has & HAS_DEADLINE) != 0 && s[0].next > d.deadline { return DROP_LATE; }
  if (d.has & HAS_GAIN) != 0 { s[0].gain_db = d.gain_db; }
  return 0;
}

#forall "whatever arrives, the gain stays in range" g: -40..<40, has: 0..<8, late: 0..<3 {
  s: [1] Stream;
  s[0].next = 10;
  d: Decision;
  d.has = cast(u32) has;
  d.gain_db = f32(g);
  d.epoch = 1;
  d.deadline = cast(u32) (8 + late);
  d.confidence = 1.0;
  why := gate(s[..], d);
  #assert s[0].gain_db >= -24.0 && s[0].gain_db <= 12.0;
  #assert (why == DROP_LATE) == ((has & HAS_DEADLINE) != 0 && late < 2);
  if why != 0 || (has & HAS_GAIN) == 0 { #assert s[0].gain_db == 0.0; }
}

#test "NaN is not a gain" {
  s: [1] Stream;
  d: Decision;
  d.has = HAS_GAIN;
  d.gain_db = 0.0f32 / 0.0f32;
  d.confidence = 1.0;
  #assert gate(s[..], d) == 0 && s[0].gain_db == 0.0;
}

show :: (s: [] Stream, label: string, d: Decision) {
  why := gate(s, d);
  if why == 0 { print("%: kept, gain now % dB\n", label, s[0].gain_db); }
  if why == DROP_CONFIDENCE { print("%: dropped, confidence too low\n", label); }
  if why == DROP_EPOCH { print("%: dropped, stale epoch\n", label); }
  if why == DROP_LATE { print("%: dropped, past its deadline\n", label); }
}

main :: () {
  s: [1] Stream;
  s[0].next = 40;
  d: Decision;
  d.confidence = 0.9;
  d.has = HAS_GAIN; d.gain_db = -6.0;
  show(s[..], "-6 dB", d);
  d.gain_db = 30.0;
  show(s[..], "+30 dB", d);
  d.has = HAS_GAIN | HAS_DEADLINE; d.gain_db = -3.0; d.deadline = 39;
  show(s[..], "-3 dB due by frame 39", d);
  d.has = HAS_GAIN | HAS_EPOCH; d.epoch = 0;
  show(s[..], "-3 dB for epoch 0", d);
  d.has = HAS_GAIN; d.confidence = 0.2;
  show(s[..], "-3 dB at confidence 0.2", d);
}
```

*Listing 27-11: A decision gate: clamps, a confidence floor, epochs and
deadlines*

The `has` bits are the proto's presence carried into Coax, so a missing
field and a zero field stay different. `clamp` does what the schema
comments promised, and treats a NaN gain as "not sent": `x != x` is true
only for NaN, so `clampf` turns NaN into the low bound and `clamp` drops
the field entirely. Then `gate` refuses in a fixed order and reports why.

```sh
$ coax run examples/ch27_gate.coax
-6 dB: kept, gain now -6.0 dB
+30 dB: kept, gain now 12.0 dB
-3 dB due by frame 39: dropped, past its deadline
-3 dB for epoch 0: dropped, stale epoch
-3 dB at confidence 0.2: dropped, confidence too low
$ coax test examples/ch27_gate.coax
ok   #forall whatever arrives, the gain stays in range (1920 cases, 335434 steps)
ok   #test NaN is not a gain (1 case, 117 steps)
2 checks, 2 passed, 0 failed
```

A +30 dB request wasn't refused; it was kept and clamped to +12. That's a
choice the schema made: a decider asking for "much louder" gets "as loud
as allowed". A decision that arrives after its deadline is a different
matter. Its author meant it for an earlier moment, so applying it now
could be wrong, and it's dropped.

The real gate does the same and a little more. It checks that the action
belongs on the stream (you can duck the downlink, but you can't duck the
microphone), and instead of applying at once it parks the decision until
its `apply_from_sequence` comes due:

Filename: metareal/voice.coax

```jai
// The one gate every decision passes. True if kept.
decide :: (s: [] VoiceState, d0: VoiceDecision) -> bool {
  if !action_fits(d0.stream, d0.action) { return false; }
  d := clamp_decision(d0);
  if d.confidence < s[0].cfg.confidence_floor { return false; }
  if (d.has & HAS_EPOCH) != 0 && d.epoch != s[0].epoch { return false; }
  at := stream_next(s, d.stream);
  if (d.has & HAS_DEADLINE) != 0 && at > d.deadline { return false; }
  if (d.has & HAS_APPLY_FROM) == 0 || d.apply_from < at { d.apply_from = at; }
  if d.action == ACT_BARGE && d.apply_from == at {
    cut(s);
    return true;
  }
  i := cast(int) d.stream;
  s[0].pend[i] = d;
  s[0].pend_on[i] = true;
  s[0].pend_epoch[i] = s[0].epoch;
  s[0].pend_self[i] = false;
  return true;
}
```

*Listing 27-12: The engine's one decision gate*

Each frame, `apply_due` looks at the parked decision for its stream. If
the epoch changed since the decision was parked, it's thrown away. If its
sequence has come, it's applied. Either way the frame is processed with
whatever state is current, so nothing waits.

The real file checks `clamp_decision` with a `#forall` over 40,000
combinations of gain, hold time and presence bits, and checks
`action_fits` over every stream and action number, including ones the
schema doesn't define.

The built-in policy goes through this same gate. When near-speech energy
shows up over playback, it ducks the reply by 12 dB; after 200 ms of quiet
it resumes, but only a duck it made itself, never one an outside decider
asked for. When the trailing silence of a turn still carries breathy,
hesitant energy near the end, it asks for a 400 ms hold. Each of those has
a `#test` in the file ("the policy ducks near speech and resumes after
quiet", "the policy holds a hesitant ending").

Outside decisions work over the wire today in one direction. The dev
client can send a downlink `FrameDecision` with `--decide-gain -12`, and
the host's engine applies it to the reply audio it sends. The host's test
compares the reply's peak with and without it, as you'll see in the
testing section: 2480 against 9000.

## The jitter buffer and replaying from a trace

Reply audio crosses a network. Packets arrive late, early, twice, or not
at all. The jitter buffer sits between the socket and the speaker: it
holds a couple of packets so that small delays don't cause gaps, plays in
sequence order, refuses duplicates and packets from old epochs, and covers
a missing packet with something better than a click.

What makes Coax a good home for it is determinism. Chapter 13 listed what
makes two runs differ (clocks, ambient randomness, the order threads
finish) and showed Coax code keeping them out.
For a jitter buffer that means a recorded trace of what arrived and when
we played can be replayed exactly, as many times as we like, and a bug
seen once in the field can become a test. Let's build a small one and
replay a trace through it.

Filename: examples/ch27_jitter.coax

<!-- checked-source: examples/ch27_jitter.coax -->
```jai
#import "Basic";

SLOTS :: 4;

// A packet is one value standing in for 20 ms of audio.
Slot :: struct { used: bool; seq: u32; value: s16; }

Jitter :: struct {
  slots: [SLOTS] Slot;
  epoch: u64 = 1;
  next: u32;      // the sequence to play next
  based: bool;    // `next` was set by the first packet
  depth: u32;
}

flush :: (j: [] Jitter) {
  for i: 0..<SLOTS { j[0].slots[i].used = false; }
  j[0].depth = 0;
  j[0].based = false;
}

// Admit a packet, or refuse it: stale epoch, too old, too far ahead, or a
// duplicate. A newer epoch throws away everything buffered.
push :: (j: [] Jitter, seq: u32, epoch: u64, value: s16) -> bool {
  if epoch < j[0].epoch { return false; }
  if epoch > j[0].epoch { j[0].epoch = epoch; flush(j); }
  if !j[0].based { j[0].next = seq; j[0].based = true; }
  if seq < j[0].next || cast(u64) seq >= cast(u64) j[0].next + SLOTS { return false; }
  k := cast(int) (seq % SLOTS);
  if j[0].slots[k].used { return false; }
  j[0].slots[k] = Slot.{used = true, seq = seq, value = value};
  j[0].depth += 1;
  return true;
}

// Play one packet's worth. A missing packet plays silence and is skipped.
pop :: (j: [] Jitter) -> s16 {
  k := cast(int) (j[0].next % SLOTS);
  out: s16 = 0;
  if j[0].slots[k].used && j[0].slots[k].seq == j[0].next {
    out = j[0].slots[k].value;
    j[0].slots[k].used = false;
    j[0].depth -= 1;
  }
  j[0].next += 1;
  return out;
}

// A trace: what arrived, in arrival order, and when we played.
// kind 0 = a packet (seq, epoch, value); kind 1 = play one.
Event :: struct { kind: int; seq: u32; epoch: u64; value: s16; }

TRACE :: [12] Event.[
  Event.{kind = 0, seq = 0, epoch = 1, value = 100},
  Event.{kind = 0, seq = 2, epoch = 1, value = 300},   // arrives before 1
  Event.{kind = 0, seq = 1, epoch = 1, value = 200},
  Event.{kind = 0, seq = 1, epoch = 1, value = 999},   // a duplicate
  Event.{kind = 1},
  Event.{kind = 1},
  Event.{kind = 0, seq = 4, epoch = 1, value = 500},   // 3 never comes
  Event.{kind = 1},
  Event.{kind = 1},
  Event.{kind = 0, seq = 9, epoch = 2, value = 900},   // barge-in: a new epoch
  Event.{kind = 0, seq = 5, epoch = 1, value = 600},   // late audio of the old one
  Event.{kind = 1},
];

// Replay the trace; return what played, folded into one number.
replay :: (verbose: bool) -> u32 {
  j: [1] Jitter;
  h: u32 = 2166136261;
  for i: 0..<TRACE.count {
    e := TRACE[i];
    if e.kind == 0 {
      ok := push(j[..], e.seq, e.epoch, e.value);
      if verbose { print("push seq % epoch %: %\n", e.seq, e.epoch, ifx ok then "kept" else "refused"); }
    } else {
      v := pop(j[..]);
      if verbose { print("play: %\n", v); }
      h = cast(u32) ((cast(u64) (h ^ cast(u32) (cast(int) v + 32768)) * 16777619) % 4294967296);
    }
  }
  return h;
}

#test "the same trace plays the same audio" {
  #assert replay(false) == replay(false);
}

main :: () {
  first := replay(true);
  second := replay(false);
  print("replays match: %\n", first == second);
}
```

*Listing 27-13: A four-slot jitter buffer and a trace replayed through it*

The trace is a constant array, so it's part of the program, and the check
replays it twice inside the compiler. The fingerprint is FNV-1a, done in
`u64` and reduced, because Coax traps on integer overflow rather than
wrapping. (A first version of this listing multiplied in `u64` without
reducing, and both `coax run` and `coax test` stopped with "integer
overflow in '*'". That's the trap doing its job.)

```sh
$ coax run examples/ch27_jitter.coax
push seq 0 epoch 1: kept
push seq 2 epoch 1: kept
push seq 1 epoch 1: kept
push seq 1 epoch 1: refused
play: 100
play: 200
push seq 4 epoch 1: kept
play: 300
play: 0
push seq 9 epoch 2: kept
push seq 5 epoch 1: refused
play: 900
replays match: true
```

Packet 2 arrived before packet 1, and they still played in order. The
duplicate of 1 was refused. Packet 3 never came, so its turn played
silence. Then a packet from epoch 2 arrived, which flushed the buffer, and
late audio from epoch 1 was refused. That last line is barge-in seen from
the receiving end.

The real buffer in `metareal/voice.coax` is the same idea at full size: 16
slots of up to 200 ms each, playout that starts once two packets are
buffered, a rebase when an earlier packet arrives before playout starts,
and concealment that repeats the last 10 ms of audio while fading it to
silence over 40 ms instead of playing a hard zero. Its tests include
"jitter: the same pushes give the same output", which drives two buffers
with 40 rounds of identical pushes and pops and compares every sample.

## The authorities: who may be heard, and what it costs

Two parts of the system decide things that matter beyond sound. One is
who may be heard: whether a participant has consented, who holds the
floor, and where their words go. The other is money: how much speech an
account has used and whether another turn may run. Both files start with
`#authority;`, the directive Chapter 10 introduced for code that guards
money. Under `#authority`, every procedure in the file must be run by its
checks, and `coax mutate` must kill every mutant the compiler makes of it.

Let's see what that buys with a small floor of our own. Four slots, a
consent flag each, and at most one slot holding the floor:

Filename: examples/ch27_floor.coax

<!-- checked-source: examples/ch27_floor.coax -->
```jai
// Chapter 27: who may be heard, decided from consent and one floor holder.
#import "Basic";
#authority;

SLOTS :: 4;

Floor :: struct {
  consent: [SLOTS] bool;
  holder: s32 = -1;   // the slot holding the floor; -1 = nobody
}

// Withdrawing consent also takes the floor back.
floor_consent :: (f: Floor, slot: s32, yes: bool) -> Floor {
  g := f;
  if slot < 0 || slot >= SLOTS { return g; }
  g.consent[cast(int) slot] = yes;
  if !yes && g.holder == slot { g.holder = -1; }
  return g;
}

// Only a consenting slot can be granted the floor; -1 releases it.
floor_grant :: (f: Floor, slot: s32) -> Floor {
  g := f;
  if slot == -1 { g.holder = -1; return g; }
  if slot < 0 || slot >= SLOTS { return g; }
  if !g.consent[cast(int) slot] { return g; }
  g.holder = slot;
  return g;
}

may_hear :: (f: Floor, slot: s32) -> bool {
  if slot < 0 || slot >= SLOTS { return false; }
  return f.consent[cast(int) slot] && f.holder == slot;
}
```

*Listing 27-14: A small floor: consent, one holder, and who may be heard*

For the check, we'll do what the real file does: write a reference model
the obvious way, inline in the check, and compare the floor to it after
every operation. Our first try runs every two-step sequence of operations
(consent, withdraw, grant) on every slot from -1 to 4, including the bad
ones:

```jai
// Every two-step sequence of operations on slots -1..4, checked against
// a model written the obvious way. Ops: 0 consent, 1 withdraw, 2 grant.
#forall "the floor matches its model" o1: 0..<3, s1: -1..<5, o2: 0..<3, s2: -1..<5 {
  f: Floor;
  consent: [SLOTS] bool;
  holder := -1;
  for step: 0..<2 {
    op := ifx step == 0 then o1 else o2;
    s := ifx step == 0 then s1 else s2;
    inside := s >= 0 && s < SLOTS;
    if op == 0 {
      f = floor_consent(f, cast(s32) s, true);
      if inside { consent[s] = true; }
    } else if op == 1 {
      f = floor_consent(f, cast(s32) s, false);
      if inside { consent[s] = false; if holder == s { holder = -1; } }
    } else {
      f = floor_grant(f, cast(s32) s);
      if s == -1 { holder = -1; } else if inside && consent[s] { holder = s; }
    }
    #assert f.holder == cast(s32) holder;
    for k: -1..<5 {
      want := k >= 0 && k < SLOTS && consent[k] && holder == k;
      #assert may_hear(f, cast(s32) k) == want;
    }
  }
}
```

*Listing 27-15: A first model check, over every two-step sequence*

It passes all 324 cases. It looks thorough. Let's ask `coax mutate`
whether it is:

```sh
$ coax mutate examples/ch27_floor.coax
examples/ch27_floor.coax:17:11: mutant survived: the if condition was negated
    if !yes && g.holder == slot { g.holder = -1; }
            ^
examples/ch27_floor.coax:17:11: mutant survived: '&&' became '||'
    if !yes && g.holder == slot { g.holder = -1; }
            ^
examples/ch27_floor.coax:17:23: mutant survived: '==' became '!='
    if !yes && g.holder == slot { g.holder = -1; }
                        ^
examples/ch27_floor.coax:17:42: mutant survived: the assignment was removed
    if !yes && g.holder == slot { g.holder = -1; }
                                           ^
examples/ch27_floor.coax:17:44: mutant survived: -1 became 0
    if !yes && g.holder == slot { g.holder = -1; }
                                             ^
examples/ch27_floor.coax:24:28: mutant survived: the assignment was removed
    if slot == -1 { g.holder = -1; return g; }
                             ^
34 mutants, 28 killed, 6 survived
```

Six survivors, and they cluster on two lines: the one that takes the
floor back when a holder withdraws consent, and the one that releases the
floor. We could delete either line and our check would still pass. That's
alarming for a file whose whole job is "nothing of a participant who said
no is heard".

Why did 324 cases miss it? To withdraw while holding the floor, a slot
must first consent, then be granted the floor, then withdraw. That's three
steps. In two steps the holder is always -1 when those lines run, so
nothing they do is visible. The fix is to check every three-step sequence:

```jai
// Every three-step sequence of operations on slots -1..4, checked against
// a model written the obvious way. Ops: 0 consent, 1 withdraw, 2 grant.
#forall "the floor matches its model" o1: 0..<3, s1: -1..<5, o2: 0..<3, s2: -1..<5, o3: 0..<3, s3: -1..<5 {
  f: Floor;
  consent: [SLOTS] bool;
  holder := -1;
  for step: 0..<3 {
    op := ifx step == 0 then o1 else ifx step == 1 then o2 else o3;
    s := ifx step == 0 then s1 else ifx step == 1 then s2 else s3;
    // ...
```

*Listing 27-16: The model check extended to three steps*

```sh
$ coax test examples/ch27_floor.coax
ok   #forall the floor matches its model (5832 cases, 5057816 steps)
1 check, 1 passed, 0 failed
$ coax mutate examples/ch27_floor.coax
34 mutants, 34 killed, 0 survived
$ coax run examples/ch27_floor.coax
grant 1 before consent: may hear 1 = false
consent, then grant 1:  may hear 1 = true
1 withdraws consent:    may hear 1 = false, holder -1
```

Every mutant dies now. This is what zero survivors buys: each comparison,
assignment and constant in the file is pinned by some check, so a later
edit that changes what the floor decides has to change a check too. It
doesn't prove the model is the *right* rule; Chapter 25 is careful about
that difference. But it rules out a whole class of "the tests were
looking somewhere else".

The real `metareal/voice-floor.coax` has eight slots, binds each slot to a
route target, and keeps the floor's media epoch. Its model check runs
every three-step sequence of 19 operations, including bad slot numbers,
on three slots: 6,859 cases. From the repository root:

```sh
$ coax/bin/coax test metareal/voice-floor.coax
ok   #forall the floor matches its model (6859 cases, 7838207 steps)
ok   #forall bad slots (20 cases, 4078 steps)
ok   #test the top slot and negative grants (1 case, 603 steps)
ok   #test epochs move forward and stop at the top (1 case, 286 steps)
4 checks, 4 passed, 0 failed
$ coax/bin/coax mutate metareal/voice-floor.coax
77 mutants, 77 killed, 0 survived
```

`metareal/voice-meter.coax` decides money. Each account gets a free tier
per day (10 minutes recognized and 5,000 characters synthesized), and
beyond it every millisecond and character is priced in millisatoshis. A
turn may start only if its worst case still fits the free tier plus the
budget the host sets. The arithmetic saturates instead of trapping, and
it's written so that every boundary is one a check can see. The comment
explains why the comparisons look a little unusual:

Filename: metareal/voice-meter.coax

```jai
// Each comparison is placed where its boundary changes the answer, so the
// boundary is something a check can see (a saturated sum at exactly TOP - 1
// is not saturated; usage of exactly the free amount owes nothing).
add_sat :: (a: u64, b: u64) -> u64 {
  if b == TOP { return TOP; }
  if a > TOP - 1 - b { return TOP; }
  return a + b;
}
// Units of `used` beyond `free` (free < TOP).
over :: (used: u64, free: u64) -> u64 {
  if used < free + 1 { return 0; }
  return used - free;
}
```

*Listing 27-17: Saturating arithmetic written so mutation can see its
boundaries*

`used < free + 1` means the same as `used <= free`, so why write it the
long way? Let's try the short way. Copy the file as
`voice-meter-le.coax`, change that one line to `if used <= free { return
0; }`, and mutate it:

```sh
$ coax mutate voice-meter-le.coax
...
voice-meter-le.coax:44:11: mutant survived: '<=' became '<'
    if used <= free { return 0; }
            ^
70 mutants, 69 killed, 1 survived
```

The surviving mutant is `used < free`. At exactly `used == free` the
original returns 0 early, and the mutant falls through to `used - free`,
which is also 0. No input can tell them apart, so no check can kill the
mutant. It's an *equivalent* mutant, and the rule for these files is that
you never mark one equivalent to make the gate pass. Instead the author
rewrote the comparison so that every mutant of it changes some answer:
with `used < free + 1`, the mutant `used <= free + 1` returns 0 at
`free + 1`, where the real code returns 1, and a check is waiting there.
The real file passes its gate:

```sh
$ coax/bin/coax mutate metareal/voice-meter.coax
72 mutants, 72 killed, 0 survived
```

## The C face

So far everything has been Coax. The phone is Swift and the server is C,
so they need a way in. `coax c FILE --library --header Name.h` turns each
of the four voice files into a C library with a generated header, as
Chapter 17 described. But the hosts don't include those headers directly.
They include one hand-written file, `metareal/platform/voice.h`, backed by
`metareal/platform/voice.c`. Why the extra layer?

Start with where the engine's state lives. Coax has no heap and no global
variables:

```sh
$ coax check globals.coax
globals.coax:1:1: error[E0212]: global variables are not supported by coax0 yet
  engine_state: [4] s16;
  ^
1 error
```

So all of the engine's state is one struct, `VoiceState`, and every export
takes it as a one-element view, `s: [] VoiceState`. Someone outside Coax
has to own that memory. That's the C face's first job: it defines an
opaque `oa_voice`, tells the caller its size (195,768 bytes, as the test
run reports later), and wraps each call.

The second job is the boundary Coax can't cross yet in the other
direction. A view can come *into* an exported procedure, but `#foreign`
imports pass only scalars. Here's what happens if we try to declare an
Opus encoder that takes sample buffers:

```sh
$ coax c opus.coax --library
opus.coax:1:17: error[E0376]: parameter 'pcm' is [] s16; #foreign procedures pass integers, floats and bool
  opus_encode :: (pcm: [] s16, out: [] u8) -> int #foreign;
                  ^
opus.coax:1:30: error[E0376]: parameter 'out' is [] u8; #foreign procedures pass integers, floats and bool
  opus_encode :: (pcm: [] s16, out: [] u8) -> int #foreign;
                               ^
2 errors
```

That's why the design puts codecs in host C, and why `#foreign` imports
that take views are one of the two language additions the voice design
asked for. Until then, Coax code can't call out with audio buffers; the
host calls in with them.

The third job is a contract that doesn't move. The generated headers name
things after Coax (`coax_t_VoiceDecision`, `coax_v_t_VoiceState`) and
change when the Coax structs change. The hosts need something steadier,
and the header says what it promises:

Filename: metareal/platform/voice.h

```c
 * This is the plain-C face of the voice engine. The voice host
 * (metareal/voice-host/) and the iOS app (apps/openagents-ios/OpenAgents/Voice/)
 * both code against exactly this file. Changes from here on are additive only:
 * new functions, new constants, new fields appended at the END of a struct
 * (callers zero-initialize structs, so a zero new field must mean "absent" or
 * "the old behavior"). Never renumber, reorder, or repurpose anything below.
```

*Listing 27-18: The C face's compatibility promise*

Its enums are numbered exactly as the proto, and `voice.c` asserts that at
compile time with `_Static_assert`. Everything else in `voice.c` is
conversion. Here's the uplink, which builds the views Coax expects from
plain pointers:

Filename: metareal/platform/voice.c

```c
uint32_t oa_voice_uplink(oa_voice *v, const int16_t *pcm, int16_t *out16k, oa_voice_features *f) {
  if (!pcm) return 0;
  coax_t_VoiceFeatures k;
  memset(&k, 0, sizeof k);
  coax_v_s16 in = {(int16_t *)pcm, (int64_t)metareal_voice_input_frame_samples(sv(v))};
  coax_v_s16 out = {out16k, out16k ? OA_VOICE_FRAME_SAMPLES : 0};
  coax_v_t_VoiceFeatures fv = {&k, 1};
  uint32_t ev = metareal_voice_uplink(sv(v), in, out, fv);
  if (f) features_out(&k, f);
  return ev;
}
```

*Listing 27-19: One call through the C face*

Notice that the input view's count comes from Coax
(`metareal_voice_input_frame_samples`), not from the caller. Chapter 17
warned that Coax trusts the count it's given; here the face makes sure
the count is the one the engine was configured for.

One small trap is worth knowing about. On macOS's case-insensitive disk,
a generated `Voice.h` for `voice.coax` would be the same file as
`platform/voice.h`. So the build names the engine's library `VoiceEngine`,
and the others `VoiceWire`, `VoiceFloor` and `VoiceMeter`.

## The voice host: a thin C host

The voice host is one C process in `metareal/voice-host/`. It does the
things Coax can't: it listens on a socket (`127.0.0.1:8787` by default),
speaks WebSocket (a small RFC 6455 implementation in `ws.c`), and makes
HTTPS calls to Google with libcurl. It links the same four engine
libraries through the same C face as the phone, plus `voice-reply.coax`,
its own reply-path decisions. The top of `host.c` states the one
threading rule that keeps it simple:

Filename: metareal/voice-host/host.c

```c
// The voice engine (voice.coax via voice.h) is confined to the main thread:
// every oa_voice*, oa_voice_floor* and oa_voice_meter* call happens here.
// Workers only do HTTP (speech.c) and hand results back through a pipe.
```

The host's own decisions are thin too. When a client starts a turn, the
host asks the meter and the floor, and does what they say:

```c
  // voice-meter.coax decides whether this turn may run at all (worst case:
  // the longest turn the host accepts plus the longest reply).
  oa_voice_config cfg;
  oa_voice_get_config(s->v, &cfg);
  if (!oa_voice_meter_may_run(s->acct->m, cfg.max_turn_ms, MAX_TEXT)) {
    s->turn_refused = true;
    send_error(s, OA_ERROR_LIMIT, "the free tier and budget for today are used up", s->turn);
    send_meter(s);
    return;
  }
  // voice-floor.coax: the speaker must hold consent and the floor to be heard.
  if (!oa_voice_floor_may_hear(s->fl, 0)) {
    s->turn_refused = true;
    send_error(s, OA_ERROR_UNAUTHORIZED, "this speaker may not be heard now", s->turn);
    return;
  }
```

*Listing 27-20: The host asks the two authorities before a turn runs*

Where the transcript goes is the floor's answer too
(`oa_voice_floor_route`), never something read from the words. That's a
rule from the design: speech grants no authority, and routing is by
explicit binding.

Speech providers sit behind one small interface in `speech.h`. There are
two. `google` calls Speech-to-Text v2 `recognize` with `chirp_3` and
Text-to-Speech with a Chirp 3 HD voice. `local` is offline and
deterministic, made for tests: recognition reports how many milliseconds
of speech it was given, and synthesis returns a rising chirp whose length
depends on the text. There are two agents as well: `echo`, which replies
"You said: ..." and `gemini`, which asks Vertex AI for a short spoken
reply.

`build.sh` turns each `metareal/voice*.coax`, and `voice-reply.coax`, into
a C library with `coax c --library --header`, compiles the C face over them, and links the host
against libcurl from the macOS SDK. Because `#authority` files run their
checks inside `coax c`, a failing floor or meter check stops the build
before a host exists.

Here's a full offline session with the dev client, which plays a WAV
recording in real time through `oa_voice_uplink`, exactly as the phone
does with its microphone. These commands run in `metareal/voice-host/`
after `./build.sh`:

```sh
$ build/voice-host --listen 127.0.0.1:8899 --provider local --agent echo &
$ build/voice-client --url ws://127.0.0.1:8899/ --out reply.wav fixtures/hello-foreman.wav
welcome session=s1-a00724ec provider=local in=16000 out=24000 max_turn_ms=30000 free_ms_left=600000
turn-start t1 at 560 ms of input (pre-roll 160 ms)
turn t1 STARTED epoch=1
route t1 -> foreman (default)
turn t1 PARTIAL "I heard 880 milliseconds of speech. Thank you." epoch=1
turn t1 PARTIAL "I heard 1880 milliseconds of speech. Thank you." epoch=1
turn t1 PARTIAL "I heard 2080 milliseconds of speech. Thank you." epoch=1
turn-end t1 (2600 ms)
turn t1 FINAL "I heard 2080 milliseconds of speech. Thank you." epoch=1
meter recognized_ms=4840 synthesized_chars=0 free_ms_left=595160 free_chars_left=5000 owed_msat=0
meter recognized_ms=4840 synthesized_chars=56 free_ms_left=595160 free_chars_left=4944 owed_msat=0
speech u1 "You said: I heard 2080 milliseconds of speech. Thank you." rate=24000 epoch=1
latency final_ms=0 first_audio_ms=1 (from TURN_END)
speech_end u1 played_samples=76800 epoch=1
turn t1 ENDED epoch=1
summary finals=1 partials=3 speeches=1 speech_ends=1 interrupted=0 audio_in=16 downlink_samples=76800 stale=0 features=0 decisions=0 meters=2 errors=0 peak=9000
wrote reply.wav (76800 samples at 24000 Hz)
```

Look at `recognized_ms=4840` for one short turn. That isn't a bug in the
meter. Google's streaming recognition is gRPC-only, so for now the host
makes partial transcripts by re-recognizing the audio so far over REST,
and each of those calls is real usage. The three partials (880, 1,880 and
2,080 ms) add up to 4,840. The meter charges what was actually sent.

Notice also that the final transcript is the same as the last partial,
and no separate final was charged. That's a newer decision in the host:
when a finished partial already covers everything up to the last voiced
frame, it may stand as the final, so the agent can start at once. The
host's log line for the turn says so, `t1 final (partial) in 0 ms`. That
decision, along with when to recognize early during trailing silence and
where a streaming reply may be cut into sentences, lives in a fifth Coax
file, `metareal/voice-host/voice-reply.coax`. It's an `#authority` file
too, because whether a partial may stand decides whether the agent hears
every word. It appeared while this chapter was being written and was
still changing, so we only point at it here; when we ran it, `coax test`
passed its 7 checks and `coax mutate` killed all 74 mutants.

With `--observe`, the host also sends the client a `FrameFeatures` for
every frame its engine processed. The first few lines of that stream:

```text
features up seq=0 rms=-120.0 peak=-120.0 speech=0.00 noise=-71.0 turn=1 play=1 speech_ms=0 silence_ms=0 zc=0.000
features up seq=1 rms=-24.2 peak=-10.2 speech=1.00 noise=-71.0 turn=1 play=1 speech_ms=0 silence_ms=0 zc=0.125
features up seq=2 rms=-16.6 peak=-6.7 speech=1.00 noise=-71.0 turn=1 play=1 speech_ms=0 silence_ms=0 zc=0.097
```

These are the same numbers our level meter computed, for real speech: a
-16 dBFS frame over a -71 dBFS floor is speech with near certainty.
`turn=1` is `TURN_STATE_IDLE`. Turn boundaries are the client's decision
(its engine heard the onset first and sent `TURN_START` with the
pre-roll); the host's engine only measures the turn's audio, so it is
still counting its own minimum run in these first frames.

## The phone: a thin Swift host

The iOS side lives in `apps/openagents-ios/OpenAgents/Voice/`, and it is
the piece still being finished as this chapter is written. Here's what
exists. `VoiceSession.swift` sets up the audio session and captures with
`AVAudioEngine`, turning on Apple's voice processing for echo
cancellation. The microphone tap does one thing: it copies samples into
a preallocated single-producer, single-consumer ring (`VoiceRing.swift`)
that never locks or allocates. A serial worker queue reads 20 ms frames
from the ring and owns the `oa_voice` engine, the wire and the socket.
`VoiceFrames.swift` builds and decodes frames through `oa_voice_encode`
and `oa_voice_decode`, so there's no protobuf written by hand in Swift.
`VoiceBar.swift` is the UI: a toggle at the top left of the Grid, a
five-bar level indicator, and a caption.

The heart of it is how little the Swift code decides. Every branch on what
to do is a test of an event bit the engine returned:

Filename: apps/openagents-ios/OpenAgents/Voice/VoiceSession.swift

```swift
    private func uplink() {
        var f = oa_voice_features()
        let ev = oa_voice_uplink(v, frameIn, frame16k, &f)
        lastUplinkTurn = f.turn_state
        onFeatures?(f)
        let level = Self.level(f.rms_db)
        publish { s in
            s.uplink = f; s.frames &+= 1
            if level > s.peakLevel { s.peakLevel = level }
        }
        if ev & UInt32(OA_EV_BARGE_IN) != 0 { bargeIn() }
        if ev & UInt32(OA_EV_TURN_START) != 0 { beginTurn() }
        if ev & UInt32(OA_EV_FORWARD | OA_EV_TURN_START) != 0, ev & UInt32(OA_EV_DROPPED) == 0, let turn = currentTurn, welcomed {
            sendAudio(turn: turn, samples: frame16k, count: Int(OA_VOICE_FRAME_SAMPLES))
        }
        if ev & UInt32(OA_EV_DECISION) != 0 { appliedPolicyDecision() }
        if ev & UInt32(OA_EV_TURN_END) != 0 { endTurn(tooLong: ev & UInt32(OA_EV_TOO_LONG) != 0) }
        if options.observe, welcomed, f.sequence % 5 == 0 {
            // The dev console's trace: every fifth uplink frame's measurements.
            var m = oa_voice_msg(); m.kind = UInt32(OA_MSG_FEATURES.rawValue); m.u.features = f
            send(&m)
        }
    }
```

*Listing 27-21: The phone's uplink: Coax decides, Swift moves bytes*

Swift never decides that a turn started or ended, or that you barged in.
It's told, and it does the platform part: send a control frame, stop the
player, update the screen. If you compare this with the dev client in
`metareal/voice-host/client.c`, you'll find the same sequence, which is
why the dev client is a fair stand-in for the phone in the host's tests.

A launch argument, `-OAVoiceFixture`, streams a bundled recording into
the same ring the microphone writes, so the simulator test exercises the
real uplink path without a microphone.

> Note: what's still in progress on the phone, as of this writing. The
> Swift files above exist and the app builds them, but they belong to work
> another engineer is finishing, and we did not run the simulator test for
> this chapter. Nothing has been run on a physical iPhone. A phone has no
> default voice host yet (the simulator uses the Mac's `127.0.0.1:8787`).
> And `INVARIANTS.md` still says the home's only network use is the Spark
> wallet and that it acquires no device permissions; turning on the
> microphone and a voice socket is an invariant change that has to land
> with the app code. One more honest detail: the engine runs on the serial
> worker queue, fed by a timer, not inside the real-time audio callback.

## Testing at every layer

We've run checks along the way. Let's put them together, from the
smallest scope to the largest, because each layer catches things the
others can't.

| Layer | Command | What it establishes |
| --- | --- | --- |
| Coax checks | `coax test metareal/voice*.coax --steps 20000000` | Every rule and example in the evaluator, with each check's step count |
| Mutation | `coax mutate metareal/voice-floor.coax`, `voice-meter.coax` | The authority checks pin every decision |
| Native engine | `metareal/test_voice.sh` | The libraries and C face under AddressSanitizer and UBSan, every frame kind round-tripped, `protoc` cross-check |
| Host, offline | `metareal/voice-host/test.sh --offline` | Whole sessions over a real socket with the deterministic local provider |
| Host, live | `metareal/voice-host/test.sh` | One real Google recognition on a committed fixture |
| Simulator | `apps/openagents-ios/tools/voice-sim-test.sh` | The app in the Grid, end to end, with a bundled recording |

### Worst-case frames under a step budget

An audio callback runs every 20 ms and must never take too long. The
design's rule is that any procedure called per frame gets a `#test` that
runs it once on its most expensive input, so `coax test --steps` reports
and bounds its cost. The last five checks in `voice.coax` are exactly
that:

```sh
$ coax/bin/coax test metareal/voice.coax --steps 20000000
...
ok   #test worst case: uplink frame at 48 kHz (1 case, 2074649 steps)
ok   #test worst case: uplink frame at 44.1 kHz (1 case, 1888894 steps)
ok   #test worst case: downlink 200 ms at 24 kHz (1 case, 714780 steps)
ok   #test worst case: jitter push and pop of a full packet (1 case, 1547745 steps)
ok   #test worst case: decide (1 case, 51660 steps)
41 checks, 41 passed, 0 failed
```

A 48 kHz uplink frame, the most expensive because of resampling, costs
about two million evaluator steps. If someone makes it more expensive, the
count goes up, and a tight enough budget turns that into a failure:

```sh
$ coax/bin/coax test metareal/voice.coax --steps 2000000
...
FAIL #test worst case: uplink frame at 48 kHz
metareal/voice.coax:265:3: error[E0400]: compile-time evaluation exceeded 2000000 steps
    return KERNEL[i] + (KERNEL[i + 1] - KERNEL[i]) * f;
    ^
```

Be clear about what this does and doesn't measure. The budget applies to
every check in the file, and many ordinary tests drive the engine for
dozens of frames, so at 2,000,000 steps 17 of the 41 checks fail, not just
the worst-case ones. That's why the scripts use a generous 20,000,000 and
the worst-case step counts are read rather than enforced one by one.
Steps are also evaluator work, not microseconds on a phone. They're a
stable, machine-independent cost signal; the native smoke test below
prints real per-frame times. A compiler-checked marker for procedures
that may run on the audio thread is planned, not built.

### The engine natively, under sanitizers

`metareal/test_voice.sh` runs the Coax checks and both mutation gates,
then builds the four libraries and the C face with
`-fsanitize=address,undefined` and runs a C smoke test over them. If
`protoc` is installed, it also decodes every frame the smoke test encoded,
as a second implementation of the wire. Here's a full run:

```sh
$ metareal/test_voice.sh
voice: 41 checks, 41 passed, 0 failed
voice-wire: 7 checks, 7 passed, 0 failed
voice-floor: 4 checks, 4 passed, 0 failed
voice-meter: 6 checks, 6 passed, 0 failed
voice-floor mutate: 77 mutants, 77 killed, 0 survived
voice-meter mutate: 72 mutants, 72 killed, 0 survived
voice engine (oa_voice 195768 bytes)
  16000 Hz tone burst: turn frames 57..114, 58 forwarded, pre-roll 2560 samples, 1.7 us/frame
  16000 Hz speech-like noise: turn frames 137..204
  24000 Hz tone burst: turn frames 57..115, 59 forwarded, pre-roll 2560 samples, 32.3 us/frame
  24000 Hz speech-like noise: turn frames 137..205
  44100 Hz tone burst: turn frames 57..115, 59 forwarded, pre-roll 2560 samples, 57.3 us/frame
  44100 Hz speech-like noise: turn frames 137..205
  48000 Hz tone burst: turn frames 57..115, 59 forwarded, pre-roll 2560 samples, 63.9 us/frame
  48000 Hz speech-like noise: turn frames 137..204
  barge-in: epoch 1 -> 2, 2 decisions
  decisions: clamps, epoch, deadline, floor, stream rules hold
  jitter buffer: reorder, duplicate, underrun, epoch fencing
voice wire
  frame kind  1:    93 bytes, round trip ok
  frame kind  2:    41 bytes, round trip ok
  frame kind  3:  9631 bytes, round trip ok
  frame kind  4:    29 bytes, round trip ok
  frame kind  5:    49 bytes, round trip ok
  frame kind  6:   105 bytes, round trip ok
  frame kind  7:    32 bytes, round trip ok
  frame kind  8:    11 bytes, round trip ok
  frame kind  9:    31 bytes, round trip ok
  frame kind 10:    43 bytes, round trip ok
  frame kind 11:    46 bytes, round trip ok
  wire: refusals hold; 254 of 840 one-bit corruptions refused, the rest decoded safely
voice floor and meter
  floor and meter: consent, binding, epochs, free tier, prices
all voice checks passed
garden-pause.wav: first audio sent at 280 ms (speech begins at 300)
protoc decodes all 11 frame kinds
```

The per-frame times are from a sanitizer build at `-O1`, so they're far
slower than an optimized build, and they move a little from run to run.
They still fit in a 20 ms frame with plenty of room: 64 microseconds is
about 0.3% of the frame. The `garden-pause.wav` line is a regression check
for the onset rule we saw earlier: speech in that recording begins at
300 ms, and the engine must start sending audio no later than 320 ms. Notice also
the corruption line: every one-bit flip of the encoded frames was either
refused or decoded into a valid message, and the sanitizers saw no
out-of-bounds access in any of them.

### The host, offline and live

The host's test script builds everything, starts the host on a free port
with the local provider and the echo agent, and drives it with the dev
client and three recorded fixtures. Because the local provider is
deterministic, these tests don't need a network and give the same answers
every run. After a line reporting the build:

```sh
$ metareal/voice-host/test.sh --offline
...
offline: --provider local --agent echo
ok   health
ok   hello
ok   hello reply WAV written
ok   pause-is-one-turn
ok   observe
ok   barge-in
ok   decision-gain
ok   decision-gain peak 2480 vs 9000
ok   two-sessions
all voice host tests passed
```

`pause-is-one-turn` uses a recording with a 350 ms pause inside a
sentence and checks that it's one turn, because the engine waits 700 ms.
`decision-gain` is the frame decision from earlier, applied over the wire:
-12 dB brought the reply's peak from 9000 to 2480, about a quarter. (A
-12 dB gain is a factor of 0.25.)

Without `--offline`, the script adds one live case when this Mac can get a
Google access token: the real recognizer must hear "what are we building
today" in `hello-foreman.wav`. Here's a live session run by hand, against
a host started with `--provider google --agent echo`:

```sh
$ build/voice-client --url ws://127.0.0.1:8898/ --expect "what are we building today" --out live.wav fixtures/hello-foreman.wav
welcome session=s1-915c8fa3 provider=google in=16000 out=24000 max_turn_ms=30000 free_ms_left=600000
turn-start t1 at 560 ms of input (pre-roll 160 ms)
turn t1 STARTED epoch=1
route t1 -> foreman (default)
turn t1 PARTIAL "Hello, foreman." epoch=1
turn t1 PARTIAL "Hello foreman, what are we building today?" epoch=1
turn-end t1 (2600 ms)
turn t1 FINAL "Hello foreman, what are we building today?" epoch=1
meter recognized_ms=4840 synthesized_chars=0 free_ms_left=595160 free_chars_left=5000 owed_msat=0
meter recognized_ms=4840 synthesized_chars=51 free_ms_left=595160 free_chars_left=4949 owed_msat=0
speech u1 "You said: Hello foreman, what are we building today?" rate=24000 epoch=1
latency final_ms=248 first_audio_ms=249 (from TURN_END)
speech_end u1 played_samples=104640 epoch=1
turn t1 ENDED epoch=1
summary finals=1 partials=2 speeches=1 speech_ends=1 interrupted=0 audio_in=22 downlink_samples=104640 stale=0 features=0 decisions=0 meters=2 errors=0 peak=26958
wrote live.wav (104640 samples at 24000 Hz)
```

The client's `latency` line measures from its own `TURN_END`: 248 ms to
the final transcript, and 249 ms to the first reply audio. Be careful
with that second number. The echo agent answers instantly, and both
sentences of the reply came from the host's render cache (its log says
`sentence 0: 58560 samples in 1 ms (cached)`), because this exact line had
been synthesized on an earlier run. A real agent and a cold render add
their own time. The development notes in `docs/voice/dev.md` record an
earlier cold run with the Gemini agent, before the host synthesized
sentence by sentence, at about 3.4 seconds from end of turn to the first
reply audio. A run on one Mac is a data point, not a latency claim; the
design asks for a harness that records each step on real phones.

### The app in the simulator

The last layer is `apps/openagents-ios/tools/voice-sim-test.sh`. It builds
the app, starts the voice host offline (and then live, with credentials),
boots a dedicated simulator and runs `Tests/VoiceUITests.swift`. The test
taps Start, checks the voice toggle sits at the top left of the Grid,
turns voice on, accepts the notice that says Google processes speech, and
then waits for the phases "hearing", a final transcript, and the echo
agent's reply, finally checking that the level indicator moved. It drives
the real uplink path through `-OAVoiceFixture`. As the note above says, we
didn't run it for this chapter, because the phone work is still in
progress.

## What production needs next

The system works end to end on one Mac. Several things stand between that
and people using it every day, and it's worth being specific about each.

Streaming recognition. Today the host recognizes over REST, and makes
partial transcripts by re-sending the audio so far. As the meter showed,
that nearly doubles the recognized milliseconds for a few-second turn,
and every partial is a full round trip. Google's streaming recognition is
gRPC-only, and it isn't built. The recognition jobs in `host.c`
(`JOB_RECOGNIZE`, `JOB_FINAL`, `JOB_TAIL`) and `speech_recognize` are where
it would go.

Measured latency. The host now streams the agent's reply and synthesizes
it sentence by sentence, but that was landing as this chapter was written,
and nothing has measured a real conversation since. The design's budget
(350 ms to decide the turn, 300 ms to the final transcript, 300 ms to the
first reply audio) needs the latency harness and real phones before
anyone can say it's met.

Opus. Talking to agents uses raw 16 kHz PCM, by design. Player-to-player
voice in the Verse needs a codec, because raw audio would cost about ten
times as much network. The schema reserves `CODEC_OPUS`. As we saw, Coax
can't call an encoder with sample buffers yet, so Opus will live in host
C unless `#foreign` gains views first.

The forwarder. Players talking to each other need no speech models at
all, only the host forwarding each speaker's stream to the listeners who
should hear it. "Who hears whom" is meant to be a Coax decision in a
`voice-space.coax` grown from Metareal's haunt audio; neither the
forwarder nor that file exists yet.

Deployment. The plan is one small CPU VM in us-central1, behind the
existing load balancer, since no speech model runs on our machines. It
isn't deployed. Several host features are development-only as well:
`Hello.token` isn't checked yet, and the meter lives in memory, so
restarting the host resets everyone's usage.

Cost. The design's capacity estimate, from Google's list prices, is about
$61 an hour for 100 people talking to agents at once (about $29 of
recognition and $32 of synthesis), or roughly $0.60 per person-hour. The
machines are small by comparison, around $200 a month for an active and a
standby VM. Those numbers are why `voice-meter.coax` is a launch
requirement rather than a nicety, and why turn-scoped recognition matters:
the estimate assumes people speak about 30% of the time and that
recognition runs only during turns. They're estimates; the design says to
replace them with measured numbers once the host runs for real.

## Summary

We built a voice system the way a team would, one piece at a time, and
each piece used something from earlier in the book. The schema came
first, so the phone, the host and the dev client agree on every number,
and `#import_proto` generated the codecs from it. A 20 ms frame became
levels, a noise floor and a speech likelihood, with a `#forall` over every
level and floor. End of turn became a pure state machine checked on every
12-frame history, and a forgotten reset showed up as a counterexample.
Media epochs made barge-in safe against late audio. Frame decisions pass
one gate that clamps, checks epochs and deadlines, and never makes audio
wait. A deterministic jitter buffer let us replay a trace exactly. The
floor and the meter are `#authority` files, and `coax mutate` showed us a
model check that looked thorough but missed the line that takes the floor
back from someone who withdrew consent.

Around that core, the hosts stay thin. The C face owns storage, because
Coax has no globals or heap, and offers a contract that only grows. The
voice host does sockets and HTTPS and asks Coax before every turn. The
phone moves audio and bytes and acts on event bits. And each layer has its
own test, from step-counted worst-case frames to a live Google run.

This is the last chapter. If you want to keep going, the real files are
the best next step: `metareal/voice.coax` is about 1,450 lines, and after
this chapter you should be able to read all of it. The appendices collect
the keywords, error codes and terms we've used along the way.
