# 33. Replay and snapshots

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R16 (spec stage 10).

A replay runs a recorded session again and must reach the same states. A
snapshot saves a run so it can resume. [Chapter 16](ch16-determinism.md)
plans `#test replay`; this chapter plans what a recording holds, what a
snapshot holds, and how a replay reports the first place it diverges.

## What this chapter will cover

- The observation boundaries a recording keeps.
- Snapshots with versioned schemas and an inventory.
- Exact comparison and first-divergence reports.
- Trajectory contracts for tolerant simulations.
- Qualified replay profiles.
- Omniscient and observation-level replay.

## Outline

1. **Recording.** Record every observation boundary: input acceptance
   order, asset publication, R13 clock readings
   ([chapter 30](ch30-clocks-and-ticks.md)), random-stream state, executable
   identity and the R15 numeric and runtime profile.
2. **Snapshots.** A snapshot saves logical state, entity identities and
   generations, pending structural commands, accepted future inputs, and
   solver integration or warm-start state, through versioned schemas with R5
   stable identities.
3. **The inventory.** Each snapshot lists what it includes and which external
   resources (graphics devices, network connections, device interfaces) you
   must reconstruct rather than restore.
4. **Divergence.** Replay compares state exactly where the profile promises
   exact results, and reports the first divergence by tick, field, system and
   source operation.
5. **Tolerant simulation.** A numerically tolerant simulation uses a
   separate trajectory contract. A per-kernel tolerance is not an end-to-end
   trajectory guarantee.
6. **Qualified profiles.** A replay profile names its scope (build, worker
   count, target and architecture) and refuses operations outside it.
7. **Physical inventory.** Recordings also name ephemeris and rotation-model
   versions, integrator and tolerance settings, and, once R20 defines them,
   the fidelity profile and active rule modules
   ([chapter 37](ch37-provenance-and-rule-modules.md)).
8. **Two levels.** Omniscient replay shows the authoritative state.
   Observation replay shows what each agent had received when it acted
   ([chapter 35](ch35-sensors-and-actuators.md)), so you can ask what a
   controller knew when it issued a command.

## The consumer and the gate

- Record and replay the simulated robot's closed loop, including the
  Psionic deterministic solver's state
  ([Psionic chapter 170](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch170-close-the-simulated-control-loop.md)).
- Record and replay one Verse session.
- Restore from a mid-run snapshot and match the uninterrupted run.
- Replay the spacecraft scenario at two warp factors and get the same events
  and final state.
- Fixtures omit each inventory item in turn and must show the divergence it
  causes. A replay under a different profile is refused, not silently
  compared.

## Done when

- R16 ships with those fixtures and consumers.
- `#test replay` in [chapter 16](ch16-determinism.md) runs, and its
  planned note is removed.
- This chapter's examples, including a refused replay with a `.status`
  file, run under `check-examples.sh`.

## Dependencies

- R5 stable identities ([chapter 13](ch13-data-layout.md)), R13 recorded
  clocks ([chapter 30](ch30-clocks-and-ticks.md)) and R15 strict profiles
  ([chapter 32](ch32-strict-math-and-integrators.md)).
- Used by Aethyr chapters
  [5](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch005-replay-a-session-exactly.md)
  and [36](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch036-record-and-replay-a-world.md).
