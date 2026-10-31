# 46. Time warp that stops for every event

> **Status: unimplemented — design draft.** Aethyr already catches a fast
> ball with swept tests (`sweep_sphere_plane`, `sweep_sphere_sphere` in
> [`verse/contact.coax`](../../verse/contact.coax)) and runs Belt 2200 in
> lockstep (`lock_tick`, `belt_hash` in
> [`apps/openagents-web/belt-lockstep.coax`](../../apps/openagents-web/belt-lockstep.coax)).
> This chapter plans time acceleration as event scheduling. Nothing here
> runs yet.

A ship coasting for days should take a moment to watch, but nothing that
matters may be skipped: a burn ending, a tank running dry, a radiator
overheating, a projectile arriving. Time warp is therefore not a bigger
step. It is a scheduler that advances to the earliest event and splits the
step there, so warp 1 and warp 10⁶ produce the same events in the same
order and the same final state.

## What this chapter teaches

- Event functions and root finding over a propagation step.
- Swept time of impact with curvature refinement.
- Splitting a long step at the earliest root.
- Replay at omniscient and observation levels.
- Why per-participant warp cannot exist inside one shared encounter.

## The method

Each subsystem registers event functions `g_k(x, t)` whose sign change marks
an event: burn boundaries, propellant exhaustion (chapter 41), thermal limits
(chapter 43), proximity and possible impacts, signal arrivals (chapter 45),
and domain transitions (entering a sphere of influence or a contact bubble).
Over a candidate step `[t, t + h]`:

```jai
for each k: bracket the sign change of g_k using the integrator's dense output
t* = min over k of the root (bisection, then Illinois refinement)
if t* < t + h { step to t*, apply event k, continue from t* }
```

Ties are ordered by a fixed key (time, then event kind, then entity ID), so
order never depends on registration or thread. Impacts use the swept
separation of the two trajectories; because coast arcs curve, a linear sweep
is refined against the propagated arc before it is trusted. At 1,200 km/s a
projectile moves 20 km in a 1/60 s frame, so overlap tests at frame
boundaries cannot catch it.

Replay records commands, seeds, event order and solver settings (Coax R16).
It can show the omniscient run, or the observation level: what a controller
knew when it acted, from chapter 45's stamps.

## What exists today

[`verse/contact.coax`](../../verse/contact.coax) has `sweep_sphere_plane`
and `sweep_sphere_sphere`; its check catches a ball at 200 m/s before it
passes a wall, and games split the step at the hit. Belt 2200's lockstep
applies ordered commands per tick and compares state hashes
(`Order`, `lock_apply`, `lock_tick`, `belt_hash`, `lock_compare`). Its time
compression scales a fixed step; there is no event registry and no root
finding across a coast.

## What it builds

- `aethyr/space/events.coax`: the event registry, root finding on dense
  output, and the canonical tie order.
- `aethyr/space/warp.coax`: the warp scheduler and the shared-time policy for
  encounters.
- Swept curved-arc impact tests, built on
  [chapter 21](ch021-find-where-they-touch.md)'s time of impact.

## Acceptance checks and oracles

- A 1,200 km/s projectile cannot pass a target at any warp.
- Identical event order and final state at warp 1 and 10⁶.
- A mid-run snapshot restores the same run.
- A per-participant warp inside a shared encounter is refused.
- Every registered event kind is triggered by a fixture and stops the warp.

## Dependencies

[Chapter 40](ch040-propagate-with-a-stated-error.md) (dense output),
[41](ch041-burns-consume-mass.md), [43](ch043-heat-has-to-go-somewhere.md),
[44](ch044-inside-a-turning-accelerating-ship.md) and
[45](ch045-sensors-that-see-the-past.md) for their event functions;
[chapter 5](ch005-replay-a-session-exactly.md) and
[chapter 36](ch036-record-and-replay-a-world.md) for replay. From Coax: R13
multirate clocks, R15 deterministic schedules, R16 replay and snapshots.
