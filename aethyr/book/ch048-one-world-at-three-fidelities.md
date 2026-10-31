# 48. One world at three fidelities

> **Status: unimplemented — design draft.** Aethyr's space games today mix
> engineering and game rules in one state: a tank holds real propellant or a
> boost meter (`thruster_budget` in [`verse/thrust.coax`](../../verse/thrust.coax)),
> and the floating-origin camera (`spacecam_screen` in
> [`verse/spacecam.coax`](../../verse/spacecam.coax)) keeps presentation
> apart from f64 state. This chapter plans fidelity profiles, typed rule
> modules, batched rollouts and admitted environments over one state.
> Nothing here runs yet.

The same solar system should serve a game, a training environment and an
engineering study. That works only if the fidelity is a declared profile
over one state, if every change to the physics is a module that says what it
changes, and if presentation never touches trajectories. This chapter ties
the part together and runs its regression suite.

## What this chapter teaches

- Fidelity profiles: engineering baseline, game and teaching presentation.
- Typed rule modules that declare the objects and laws they change.
- Provenance on every tunable: explicit, inferred or invented, with unit
  and source.
- Batched rollouts, learned surrogates, and admitting a scenario as a
  training environment.

## The method

A profile is data: which propagators, tolerances, rule modules and
presentation aids apply. A rule module declares its footprint:

```jai
RuleModule :: struct {
    id: ModuleId;
    changes: [] LawId;          // e.g. drive.power_limit, thermal.coupling
    objects: [] Selector;       // which ships, bodies or regions
    parameters: [] Tunable;     // each with value, unit, source, provenance
}
```

The engineering profile refuses any module or parameter whose provenance is
`invented`. Presentation (time compression, enlarged markers, control
assistance) reads state and never writes it.

Rollouts of chapters 40–46 batch through Psionic
[chapter 107](../../psionic/book/ch107-transform-functions-not-just-tensors.md)'s
`vmap` on CPU and GPU, over [chapter 31](ch031-many-worlds-in-one-step.md)'s
batched worlds. Learned surrogates for propagation and thermal steps follow
Psionic [chapter 109](../../psionic/book/ch109-learn-a-physical-system.md)'s
method; each reports its error against the solver it replaces and refuses
inputs outside its training envelope. A scenario becomes a training
environment through Psionic chapter 242's checker and is replayed with
Psionic chapter 234's exact replay.

## What exists today

[`verse/thrust.coax`](../../verse/thrust.coax) has `thruster` (real
propellant) and `thruster_budget` (a game meter) side by side, chosen per
game. [`verse/spacecam.coax`](../../verse/spacecam.coax) keeps positions in
f64 metres and converts to f32 only after subtracting the camera's centre;
its check draws a hull 5 AU out to a millionth of a pixel. Orbit League
([`apps/openagents-web/orbitleague.coax`](../../apps/openagents-web/orbitleague.coax))
and Belt 2200 ([`apps/openagents-web/belt.coax`](../../apps/openagents-web/belt.coax))
are the two consumers. There are no profiles, rule modules or provenance
records, and no batched or learned paths.

## What it builds

- `aethyr/space/profile.coax`: profiles, rule modules, provenance and the
  engineering profile's refusals.
- `aethyr/space/batch.coax`: batched rollouts over chapter 31.
- `aethyr/space/surrogate.coax`: learned surrogates with envelopes and
  reported error.
- `aethyr/space/suite/`: the regression scenarios below.

## Acceptance checks and oracles

- Presentation changes leave trajectories bit for bit identical.
- A rule module changes only what it declares.
- The engineering profile refuses invented parameters.
- The regression suite passes at engineering fidelity: two-body orbit,
  constant-acceleration segment, variable-mass rocket, off-centre thruster,
  rotating-frame release, docking frame change, one- and two-way signals,
  fast projectile, radiator equilibrium, compartment leak.
- A surrogate outside its envelope refuses; inside it, its error against the
  solver is within its stated bound.
- Engineering fidelity means agreement with the named public references
  within stated error; it is a study profile, not flight qualification.

## Dependencies

Chapters [39](ch039-ephemerides-time-scales-and-frames.md) to
[47](ch047-hits-through-layers.md); [chapter 31](ch031-many-worlds-in-one-step.md)
and [chapter 36](ch036-record-and-replay-a-world.md); Psionic chapters 107,
109, 234 and 242. From Coax: R12–R20, with R20 for rule modules and
provenance.
