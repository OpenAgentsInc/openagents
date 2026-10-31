# 68. Sound in the world: playback, space and reaction

> **Status: unimplemented — design draft.** Today
> [`coax/web/host.js`](../../coax/web/host.js) plays a realm's sounds and its
> one music loop from decoded audio files: a realm returns lines of
> `name gain pan`, and the host plays at most 16 at once through Web Audio
> gain and stereo-panner nodes. Nothing in it uses Aethyr's synthesizer. This
> chapter plans three additions: real-time playback of Aethyr's samples in
> the browser and natively, sound placed in the world, and music that
> changes with the game's state. Nothing here runs yet.

[Chapters 65–67](ch065-sound-from-numbers.md) render a whole score to a file.
A realm needs sound while it runs. The sound must follow what happens on
screen, come from where its source is, and replay exactly with the session
([chapter 5](ch005-replay-a-session-exactly.md)).

## What this chapter teaches

- How to render audio in blocks on a separate clock from the frame loop.
- How to give a sound a position in the world, so that its gain, pan, and
  delay come from the listener's view.
- How to switch music between sections on the beat grid when the game's
  state changes, and still replay it exactly.

## The method

### Blocks on the audio clock

The audio device asks for samples at its own rate, in blocks of 128 frames in
Web Audio. Aethyr renders each block with the integer modules of chapters 65
and 67. The audio clock is a count of samples rendered. It is separate from
[chapter 1](ch001-a-frame-has-a-fixed-step.md)'s frame clock:

1. Each simulation step records sound events with the step's index.
2. The audio side converts a step index to a sample with integer arithmetic:
   `sample = step × step_ns × rate / 10⁹`, plus a fixed latency.
3. Each block plays the events whose samples fall inside it.

A late or dropped frame changes when the events arrive, not which sample
they play on. So a replay renders the same samples as the live session.

### Late events and buffer underruns

The rule above makes the *rendered stream*, the sequence of samples that the
session defines, independent of frame timing. What the device plays live can
still differ from it when the simulation or the audio thread stalls. The
design separates the two and states exactly when they can differ:

1. **The audio side renders ahead by a fixed lead.** The lead `L` is a whole
   number of blocks, for example 16 blocks of 128 frames (2,048 frames,
   42.7 ms at 48 kHz). An event recorded at step `k` is scheduled at sample
   `s(k) + L`.
2. **A late event is trimmed, not moved.** If an event's scheduled sample has
   already been sent to the device when the event arrives, the event starts
   at the first unsent sample, and the samples it missed are skipped. The
   sound stays aligned with the rendered stream from that sample on. The
   host counts each late event and records its step and the number of
   skipped samples.
3. **An underrun plays silence and keeps the clock.** If the device asks for a
   block that isn't rendered yet, the host outputs silence for that block.
   The audio clock still advances by the block's length, and the renderer
   skips that block's position in the rendered stream. Audio never falls
   behind the picture. The host counts each underrun and records its sample
   range.
4. **Replay uses the rendered stream.** Replay and offline rendering produce
   the rendered stream. Late-event and underrun records are diagnostics; they
   aren't replay input, and they don't change the rendered stream.

The promise follows from these rules. Live playback equals the rendered
stream sample for sample only when no stall is longer than the lead. A longer
stall produces reported silence or trimmed sounds in the live output, and the
rendered stream stays the same.

### Sound in space

A sound source has a world position. The listener is the camera's lens from
[chapter 2](ch002-spaces-and-units.md). For each source, the method uses the
view-space position `v`:

- Distance `d = |v|`, in metres, gives the gain `g = 1 / max(1, d / d_ref)`,
  clamped to zero past a cutoff distance.
- The angle in the view's x–z plane gives the pan, through the constant-power
  pan of chapter 65.
- `d / 343` seconds (the speed of sound in air at 20 °C) gives a delay, in
  whole samples, so a far event is heard after it is seen.
- A one-pole lowpass whose coefficient falls with distance removes high
  frequencies from far sources.

### Music that reacts

A score is divided into sections of whole bars, and each section has
stems: the pad, the drums, the ostinato, and the brass. The game's state
selects the section and the stems' levels, for example from calm to danger. A
change takes effect on the next bar line of
[chapter 66](ch066-a-sequencer-on-the-beat-grid.md)'s grid, never in the
middle of a bar. The level changes through gain automation over one beat.
Because the selection is a pure function of the state at a recorded step, the
music replays with the session.

## What exists today

- `coax/web/host.js` decodes `sound` and `loop` assets with
  `decodeAudioData` and plays them with `AudioBufferSourceNode`,
  `GainNode`, and `StereoPannerNode`. The decoder and the browser's resampler
  decide the samples, so they can differ between browsers.
- A realm chooses a sound's gain and pan itself, with no shared distance
  model.
- `verse/voice.coax` resamples, levels, and gates microphone audio for voice
  chat. It is a separate path from game sound.
- Aethyr's synthesizer renders whole scores offline only
  ([chapter 67](ch067-scores-mixing-and-a-soundtrack.md)).

## The design in Aethyr

- An `aethyr/audio/stream.coax` module renders one block into a caller's
  view from a score's state, and carries the score's state from block to
  block: phases, envelopes, filters, and the reverb's delay lines.
- The browser host gets an `AudioWorkletProcessor` that calls the wasm's
  block function and copies the samples into the output. It holds no audio
  logic. The worklet's `Firm reason:` is 1 in
  [tools-in-coax.md](../../coax/tools-in-coax.md), like the rest of the
  browser host.
- The native hosts get the same block function through the C platform layer
  (Core Audio on Apple platforms).
- An `aethyr/audio/space.coax` module computes the gain, pan, delay, and
  filter for each source from the lens.
- A score gets sections and stems as data, with the selection function
  supplied by the realm.

## What it builds

- `aethyr/audio/stream.coax`, `aethyr/audio/space.coax`, and the stem and
  section data in `aethyr/audio/score.coax`.
- A thin worklet beside `coax/web/host.js`, and the matching native hook.
- A chapter example that renders a session's sound in blocks and prints the
  hash of the samples.

## Acceptance checks and oracles

- Rendering a score in blocks of 128 frames gives the same samples as
  rendering it whole, for any block size from 1 to 4,096.
- A replayed session gives the same rendered-stream hash as the live
  session, with frames delivered at 30, 60, and 144 FPS and with a 300 ms
  stall.
- With a lead of 42.7 ms and a 30 ms stall, the live device output equals the
  rendered stream sample for sample, with no late events or underruns.
- With the same lead and a 300 ms stall, the live device output differs from
  the rendered stream only inside the reported underrun ranges and the
  reported skipped samples of late events. Every difference is in a record.
- An event that arrives after its scheduled sample starts at the first unsent
  sample, and the samples after that equal the rendered stream.
- Native and browser builds give the same hash for the same session.
- A source at the reference distance has a gain of 1, and the gain halves
  when the distance doubles. A source on the view's right axis pans fully
  right.
- A source 343 m away is heard 1 second after its event, to the sample.
- A music change requested in the middle of a bar takes effect on the next
  bar line, and no stem's level changes by more than one automation step a
  sample.
- The worklet's per-block time stays under its budget; a block that misses
  its deadline is counted and reported, not hidden.

## Dependencies

- Chapters [1](ch001-a-frame-has-a-fixed-step.md),
  [2](ch002-spaces-and-units.md), [5](ch005-replay-a-session-exactly.md), and
  [65–67](ch065-sound-from-numbers.md).
- [Chapter 10](ch010-the-camera-follows.md)'s camera for the listener's lens.
- Coax's wasm build for the worklet, and a C audio hook for native hosts.
