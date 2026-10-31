# 37. Provenance, fidelity profiles and rule modules

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R20 (spec stage 10).

A simulation mixes measured values, values inferred from them and values
chosen for a scenario. A game adds rules that depart from physics on
purpose. This chapter plans how each parameter will say where it came from,
how a fidelity profile will name what a run uses, and how a rule module will
declare exactly what it changes.

## What this chapter will cover

- The provenance record and its three kinds.
- Fidelity profiles: engineering, game and presentation.
- Presentation code that can't write authoritative state.
- Typed rule modules with declared scope and trigger.

## Outline

1. **Provenance records.** An ordinary checked type over R12 quantities:
   value and unit, kind (`explicit`, `inferred` or `invented`), source (a
   citation, dataset identity or scenario name), uncertainty where known, and
   the configuration it belongs to.
2. **Cost.** The hot path reads the plain value at R12's layout cost. R5
   reflection exposes the record to tools, interfaces and reports, so a
   tooltip can say whether a number is measured, estimated or chosen.
3. **Required provenance.** A profile that requires provenance refuses a
   value without it.
4. **Fidelity profiles.** Checked configuration naming the models, rule
   modules, integrator settings and parameter sets a run uses:
   - engineering: ordinary physics, explicit or inferred parameters only;
   - game: the same motion plus declared invented parameters and rule
     modules;
   - presentation: time compression, enlarged markers and control
     assistance, which change only what is drawn.
5. **Recorded profiles.** The profile is part of R16's recorded inventory
   ([chapter 33](ch33-replay-and-snapshots.md)).
6. **Presentation effects.** Presentation code runs under an R11 effect that
   can't write authoritative state, so a readability setting can't change a
   trajectory.
7. **Rule modules.** A module that departs from ordinary physics (a
   hypothetical propulsion performance, a scenario-specific field, a region
   where a technology stops working) declares its scope (objects, state
   fields and subsystems) and its trigger. Each change it makes is an event
   in the run log.
8. **Checked writes.** The checker derives what a module writes from R11
   summaries and refuses a write outside its declaration. A module is active
   only in profiles that list it. An engineering profile lists none.

## The consumer and the gate

- The spacecraft scenario's drive, radiator and crew-tolerance parameters
  carry provenance into its report.
- Its engineering and game profiles give identical trajectories wherever no
  rule module or invented parameter differs.
- A presentation-only change leaves the recorded state bit for bit
  identical.
- Fixtures: a parameter without provenance, and an invented one, refused
  under the engineering profile; a presentation procedure that writes a
  velocity refused with its path; a rule module writing a field outside its
  declaration refused; a module enabled in a profile that doesn't list it
  refused; replay under a different profile refused by R16.
- The simulated robot's calibrated parameters
  ([chapter 35](ch35-sensors-and-actuators.md)) use the same records.

## Done when

- R20 ships with those fixtures and the spacecraft report.
- This chapter's examples run under `check-examples.sh`, including each
  refused fixture with its `.status` file.

## Dependencies

- R5 annotations ([chapter 13](ch13-data-layout.md)), R12 quantities
  ([chapter 29](ch29-units-and-frames.md)) and R16 recorded inventories
  ([chapter 33](ch33-replay-and-snapshots.md)).
- R11 effect summaries, for rule modules.
- Used by Aethyr
  [chapter 48](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch048-one-world-at-three-fidelities.md).
