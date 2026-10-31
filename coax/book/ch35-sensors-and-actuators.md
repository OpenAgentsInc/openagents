# 35. Stamped sensors and supervised actuators

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R18 (spec stage 10).

A sensor sample is only useful if you know when it was taken, in which frame
and with which calibration. A command to an actuator is only safe if the
state it came from is fresh and within limits. This chapter plans stamps on
every sample and a supervisor that checks every command. All devices in this
phase are simulated.

## What this chapter will cover

- The fields of a stamp.
- Freshness checks and bounded extrapolation.
- Light time between emission and reception.
- Geometric, light-time-corrected and aberration-corrected states.
- The actuator supervisor and its refusals.

## Outline

1. **Stamps.** Each sample carries its R13 clock and instant
   ([chapter 30](ch30-clocks-and-ticks.md)), R12 frame, sequence number and
   calibration identity, and its uncertainty where the source reports it.
2. **Freshness.** Checks use maximum ages. Extrapolation past a sample is
   explicit and bounded.
3. **Matching transforms.** A transform applied to a measurement must match
   its timestamp and calibration, not only its frame.
4. **Light time.** A remote observation carries its emission and reception
   instants. A bounded iteration solves
   `t_receive − t_emit = |r_target(t_emit) − r_observer(t_receive)| / c`.
5. **Corrected states.** Geometric, light-time-corrected and
   aberration-corrected states are distinct types, after JPL's
   [aberration corrections](https://naif.jpl.nasa.gov/pub/naif/toolkit_docs/C/req/abcorr.html).
   Light time inside the simulated world is kept apart from network latency
   between the simulation and its clients.
6. **The supervisor.** It checks each proposed command against:
   - the freshness of the state it was computed from;
   - joint and rate limits from the R17 model
     ([chapter 34](ch34-robot-models.md));
   - the controller identity and an authority epoch;
   - the sequence number and expiry.
7. **Refusals as results.** A command from a revoked epoch or past its
   expiry is refused. Refusals are R2 results, not traps. The supervisor and
   the control step it guards are `#bounded`
   ([chapter 31](ch31-bounded-control.md)).
8. **The hardware boundary.** Driving real actuators is a separately
   authorized integration with its own safety review, hardware qualification
   and real-time deployment profile. Nothing in this chapter grants it.

## The consumer and the gate

- The simulated robot's sensors and actuators run through stamps and the
  supervisor ([Psionic chapter 170](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch170-close-the-simulated-control-loop.md)).
- Fixtures: a stale sample, a frame or calibration mismatch, a command
  beyond a limit, a command from a revoked epoch, a reordered or duplicated
  sequence and an expired command. Each is refused with its reason.
- Spacecraft fixtures: a one-way light time of about 499.0 s at 1 AU, a
  round trip with moving geometry, and a controller refused when it reads a
  target's true state instead of a received one.

## Done when

- R18 ships with those fixtures, on simulated devices only.
- This chapter's examples run under `check-examples.sh`, and each refusal
  prints its reason.

## Dependencies

- R12 frames ([chapter 29](ch29-units-and-frames.md)), R13 freshness
  ([chapter 30](ch30-clocks-and-ticks.md)) and R14 bounded control
  ([chapter 31](ch31-bounded-control.md)).
- R17 limits ([chapter 34](ch34-robot-models.md)).
- Used by Aethyr chapters
  [33](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch033-sensors-read-the-simulation.md)
  and [45](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch045-sensors-that-see-the-past.md).
