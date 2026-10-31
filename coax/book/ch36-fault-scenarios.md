# 36. Fault scenarios you can replay

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R19 (spec stage 10).

A controller is tested by what it does when something goes wrong. This
chapter plans fault scenarios as data: you inject a fault into a simulated
run, the supervisor responds, and the run replays exactly from its
recording.

## What this chapter will cover

- The fault kinds for the robot and the spacecraft.
- A scenario as recorded data.
- Expected outcomes: refusal, hold or safe stop.
- How a scenario corpus proves that a check matters.

## Outline

1. **Robot faults.** Sensor dropout, delayed or reordered samples, timing
   overruns, actuator saturation, disconnects and invalid values.
2. **Spacecraft faults.** A failed thruster, a leaking tank, a damaged
   radiator and a drive cutoff in the middle of a burn. A cutoff must leave a
   propagated trajectory, not a frozen ship.
3. **Scenarios as data.** A scenario uses the R16 run format
   ([chapter 33](ch33-replay-and-snapshots.md)). The same scenario, seed and
   build reproduce the same run and the same supervisor decisions.
4. **Expected outcomes.** Each fault names the response it expects from the
   R18 supervisor ([chapter 35](ch35-sensors-and-actuators.md)): a refusal,
   a hold or a safe stop.
5. **Sharpness.** Removing a supervisor check must make at least one
   scenario fail. This is the same idea as a mutation gate
   ([chapter 11](ch11-writing-checks.md)).

## The consumer and the gate

- A scenario corpus for the simulated robot in which each fault produces the
  expected refusal, hold or safe stop, and the outcome replays exactly.
- Removing a supervisor check makes at least one scenario fail.
- The spacecraft freighter scenario (load cargo, transfer, ride out a fault,
  dock) uses the same format; see Aethyr
  [chapter 48](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch048-one-world-at-three-fidelities.md).

## Done when

- R19 ships with the robot corpus and its sharpness check.
- This chapter's examples run one scenario per fault kind under
  `check-examples.sh` and print the supervisor's decision.

## Dependencies

- R16 replay ([chapter 33](ch33-replay-and-snapshots.md)) and R18
  supervision ([chapter 35](ch35-sensors-and-actuators.md)).
