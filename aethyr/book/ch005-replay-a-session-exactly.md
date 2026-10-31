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

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md),
[chapter 3](ch003-state-starts-from-its-defaults.md) and
[chapter 4](ch004-input-becomes-typed-actions.md). From Coax: R15 strict math
and R16 deterministic replay and snapshots
([Coax roadmap](../../coax/ROADMAP.md)), determinism
([Coax chapter 16](../../coax/book/ch16-determinism.md)).
