# 45. Sensors that see the past

> **Status: unimplemented — design draft.** In Aethyr's space games today
> every ship and AI reads authoritative positions directly (Belt 2200's
> tactical layer, `tac_nearest_enemy` and `tac_lead` in
> [`apps/openagents-web/belt-tactical.coax`](../../apps/openagents-web/belt-tactical.coax)).
> This chapter plans truth, signals, measurements and tracks as separate
> data. Nothing here runs yet.

Light takes 499 seconds to cross 1 AU. A captain watching a target sees
where it was, not where it is, and a reply to a command arrives a round trip
later. A simulation that hands agents the true state teaches them to cheat.
This chapter separates what is true from what was emitted, what was received
and what is believed, and enforces the separation at the interface.

## What this chapter teaches

- Truth, emitted signal, received measurement and estimated track as four
  kinds of data.
- The light-time solution, round-trip radar and command latency.
- Detection from sensitivity, geometry, background and emission.
- A track filter whose uncertainty grows with age and maneuverability.
- Stamped observations as the only input to agents and policies.

## The method

Light time from emission at `t_e` to reception at `t_r`:

```jai
t_r − t_e = |r_target(t_e) − r_observer(t_r)| / c
```

solved by fixed-point iteration (it contracts because speeds are far below
`c`). A radar round trip solves it twice, outbound and back, with the
geometry moving during both legs. At 1 AU the one-way time is about
499.0 s (8 min 19 s).

Detection compares received signal with noise: received power from emitted
power, distance and aperture, against background in the sensor's band; a
drive plume is bright, a coasting hull still emits heat. A track is a state
estimate with covariance, propagated between measurements by a
constant-acceleration model whose process noise is the target's declared
maneuver capability:

```jai
x̂⁻ = F x̂,  P⁻ = F P Fᵀ + Q(a_max)       // predict
K  = P⁻ Hᵀ (H P⁻ Hᵀ + R)⁻¹               // update with a measurement
```

Every measurement carries a Coax R18 stamp: emit time, receive time, frame,
sensor and maximum age. The observation interface is the only path from the
world to an agent; it has no accessor for truth.

## What exists today

Belt 2200's tactical combat (`Tac`, `TacUnit`, `tac_step`,
`tac_nearest_enemy`, `tac_lead` in
[`apps/openagents-web/belt-tactical.coax`](../../apps/openagents-web/belt-tactical.coax))
targets and leads from current positions. [`aethyr/orbit.coax`](../../aethyr/orbit.coax)
gives geometric states only. Psionic's game-decision path
([`docs/verse/pylon-guard-decisions.md`](../../docs/verse/pylon-guard-decisions.md))
already passes typed context rather than raw state to a model, the pattern
this chapter generalizes.

## What it builds

- `aethyr/space/signals.coax`: emission events, light-time and aberration
  solutions using chapter 39's corrections.
- `aethyr/space/sensors.coax`: detection models with declared parameters.
- `aethyr/space/tracks.coax`: the filter, uncertainty growth, and stamped
  observations.
- `aethyr/space/observe.coax`: the observation interface agents use.

## Acceptance checks and oracles

- About 499.0 s one-way at 1 AU.
- Iterated light-time and aberration corrections match chapter 39's
  references ([NAIF aberration corrections](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/abcorr.html)).
- A round trip with moving geometry matches the two-leg solution.
- Covariance grows after an unexpected maneuver and the track recovers on
  new measurements.
- A planted policy that reads the authoritative state is refused by the
  observation interface.

## Dependencies

[Chapter 39](ch039-ephemerides-time-scales-and-frames.md)'s corrections;
[chapter 41](ch041-burns-consume-mass.md)'s ships and plumes;
[chapter 33](ch033-sensors-read-the-simulation.md)'s sensor stages. From
Coax: R13 clocks, R18 stamped sensors.
