# 184. Record and replay a world

> **Status: unimplemented — design draft.** Psionic already records and
> replays an oscillator's trajectory with a checked codec
> ([oscillator replay](../docs/physics/oscillator-replay.md), used in
> chapter 191); this chapter plans the same for a whole scene. Nothing here
> runs yet.

When a policy fails in simulation, you want to watch that failure again,
step through it, and know that what you are watching is what happened. There
are two ways to get it. Record every state and play the recording back; or
record only the inputs and recompute, which is exact only if the simulator
is deterministic. We do both, and use each to check the other.

## What this chapter teaches

- What a recording must contain to be reproducible: scene asset digests,
  the schedule, profiles, seeds, controls and the compiler identity.
- State snapshots and their format.
- Recomputation replay, and comparing it against recorded states.
- Finding the first divergent step and stage.

## The model

A recording is `(scene digest, profile, seed, u[0..K], snapshots at k ∈ S,
digest[0..K])`. Replay recomputes `x[k+1] = Φ(x[k], u[k])` from the first
snapshot and checks

```jai
digest(x_replay[k]) = digest[k]     for every k
```

On mismatch, bisection over recorded snapshots finds the first divergent
step, and per-stage digests (chapter 165's schedule) find the stage.

## The design in Psionic

The format extends the oscillator replay codec: content-addressed, with
the scene asset, every profile, the compiler and target identities and the
seed in a header, then controls per step and snapshots at a stated
interval. Coax R16's replay inventories say which state counts. Playback of
recorded states (for viewing in Verse) and recomputation replay are separate
commands; a recording whose recomputation fails is reported, never silently
played. Cross-target replay (CUDA recording, CPU recompute) is a stated
profile with a tolerance, not bitwise.

## What it builds

- `psionic/lib/sim/record.coax`: writer, reader, recompute, bisect.
- A recording of each part fixture checked in as a small regression corpus.

## Acceptance checks and oracles

- Every corpus recording recomputes bitwise on its recording target.
- A deliberately perturbed controller step is located by bisection at the
  exact step and stage.
- Snapshot round trip: write, read, step equals stepping the original.
- Truncated or tampered recordings are refused.

## How Genesis does it

Genesis 1.4.0 added scene archives and `gs replay`, which plays back
recorded states
([release](https://github.com/Genesis-Embodied-AI/genesis-world/releases/tag/v1.4.0)).
That is recorded replay. Our plan adds recomputation replay checked against
recorded digests, which only a deterministic engine can offer; it is the
largest deliberate difference in this part.

## Dependencies

The oscillator replay codec, chapter 165's schedule. From Coax: R1 content
identities, R15 strict profiles, R16 replay inventories.
