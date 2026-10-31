# 5. Replay a session exactly

> **Status: unimplemented — design draft.** Replay exists piecemeal: Ice
> Cream Man replays traces recorded from its Rust predecessor and checks a
> state fingerprint every step; Orbit League keeps a goal's last seconds of
> drives and a keyframe a second; Belt 2200 runs lockstep with FNV-1a state
> hashes. This chapter plans one recording and replay for every realm.
> Engine-core task: `ec-replay` (#12609); the engine-core agent fills this
> chapter in as the module lands. Nothing here runs yet.

With a fixed step (chapter 1), a constructed initial state (chapter 3) and
input reduced to one `Actions` sample per step (chapter 4), a session is a
function of a short record: the initial state's identity, the seed and the
action stream. Replaying that record must give the same state at every step,
bit for bit, natively and in the browser. That one property gives bug
reports that reproduce, regression tests made from play, saves, networking by
orders (chapter 50), and the physics recordings of
[chapter 36](ch036-record-and-replay-a-world.md).

## What this chapter teaches

- What a session record contains and what it must not.
- Seeded random streams that do not depend on call order across systems.
- State digests and finding the first divergent step.
- What breaks determinism: frame time, iteration order, host math, threads.

## The method

```jai
Record :: struct {
  realm, build:   identities (source digest, compiler, numeric profile)
  step:           h (chapter 1)
  seed:           u64
  initial:        digest of init(T) after setup (chapter 3)
  actions:        [Actions] one per step (chapter 4)
  marks:          digest every K steps
}
replay(r) : s = setup(init(T), r.seed)
            for k in 0..<len(r.actions): s = step(s, r.actions[k])
                                         if k % K == 0: check digest(s) == r.marks[k/K]
```

Randomness comes from counter-based streams: `rand(seed, stream, counter)`,
with a stream per system or entity, so adding a system does not shift
another's draws. Iteration is in a stable order (slot order, chapter 6).
Nothing in a step reads the frame's `Δ`, the wall clock or the host's state.
Floating-point operations follow one strict profile on both hosts: no fused
multiply-add contraction, the same transcendental implementations, no
fast-math.

On a mismatch, replay bisects between the last matching mark and the first
failing one, re-running from a snapshot to find the first divergent step,
then reports the fields that differ.

## What exists today

- [verse/icecream/replay.c](../../verse/icecream/replay.c) and
  [verse/icecream/trace.coax](../../verse/icecream/trace.coax): replay
  Rust-recorded traces on the Coax rules and check every step's state
  `fingerprint` (in `verse/icecream/rules.coax`) and events.
- `apps/openagents-web/orbitleague.coax`: `REPLAY_SECONDS`, `KEY_EVERY :: 120`
  (a saved state every second) and `REPLAY_STEPS :: 720` drives, replayed
  through the same physics after a goal.
- [apps/openagents-web/belt-lockstep.coax](../../apps/openagents-web/belt-lockstep.coax):
  orders tagged with their tick, `lock_tick`, and `belt_hash` (FNV-1a over
  quantized quantities) every `LOCK_CHECK` ticks to report a desync.
- Ad hoc generators: `next_seed` in `apps/openagents-web/everglade.coax` (an
  LCG), `rand_next` in `belt.coax`, `sp_rand` in `everglade-spells.coax`.

Three replay formats and several random generators, none shared.

## What it builds

- `aethyr/replay.coax`: `Record`, the recorder the loop calls each step,
  `replay`, `state_digest` (from chapter 3) at a declared interval, and
  first-divergence bisection.
- `aethyr/rand.coax`: counter-based random streams keyed by seed, stream and
  counter.
- A per-realm replay check run in CI over recorded sessions.

## Acceptance checks and oracles

- A recorded session replays bit-identically natively and in the browser:
  same digest at every mark.
- A replay check runs for each migrated realm (chapter 64).
- Planted faults are found: reading frame time in a step, a changed
  iteration order or an unseeded draw each produce a reported first
  divergent step and field.
- Oracle: Ice Cream Man's existing Rust traces continue to pass through the
  shared format, as an independent recorder.
- Random streams: draws from one stream are unchanged when another stream
  is consumed more or less.

## Contract: one envelope for every external event

> **Design draft.** This contract is not implemented. It is kept separate
> from the rest of the chapter so the `ec-replay` task can fill in the
> sections above without changing it.

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
[chapter 3](ch003-state-starts-from-its-defaults.md) and
[chapter 4](ch004-input-becomes-typed-actions.md). From Coax: R15 strict math
and R16 deterministic replay and snapshots
([Coax roadmap](../../coax/ROADMAP.md)), determinism
([Coax chapter 16](../../coax/book/ch16-determinism.md)).
