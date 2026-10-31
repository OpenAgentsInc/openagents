# 38. A simulated robot, end to end

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This chapter uses roadmap R12–R20 (spec stage 10).

Chapters 29–37 each add one physical-world facility. This chapter puts them
together in one program: the simulated robot that the roadmap names as a
consumer of every phase from R12 to R20. The robot is the Panda arm of
Psionic's closed control loop
([Psionic chapter 170](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch170-close-the-simulated-control-loop.md)).
Today that loop runs with plain numbers, conventions in comments and
reviewed fixed arrays. When this chapter is written, the same loop uses
checked frames, clocks, bounded steps, stamps, a supervisor, replay, fault
scenarios and parameter provenance. Every device in it is simulated.

## What this chapter will cover

- How a robot's joints, poses and torques become typed quantities in named
  frames.
- How a control step at one rate reads a physics state written at another.
- How a supervisor refuses a command, and how a fault scenario shows it.
- How you replay a whole closed loop, and find the first step where two runs
  differ.

## Outline

1. **The loop as it runs today.** Raw cameras and arm state enter Coax, the
   GR00T network runs on Metal
   ([Psionic chapter 169](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/book/ch169-execute-a-groot-policy.md)),
   and an operational-space controller turns pose commands into seven torques
   and two finger targets for MuJoCo 3.3.1. List which properties are reviewed
   by hand: fixed arrays, checked loop limits, no allocation.
2. **Import the arm.** Load the pinned arm description through the R17 URDF
   subset ([chapter 34](ch34-robot-models.md)). Compare every value with an
   independent parser. Show the refusal for an unknown element.
3. **Frames and units.** Put joint angles, link poses and the end-effector
   target on R12 types ([chapter 29](ch29-units-and-frames.md)). A world-frame
   pose given where a gripper-frame pose is expected is a compile error.
4. **Two rates.** Declare the physics step and the control step as R13 tasks
   ([chapter 30](ch30-clocks-and-ticks.md)). The controller reads the latest
   state by sample-and-hold with a maximum age.
5. **A bounded control step.** Mark the operational-space controller and the
   supervisor `#bounded` ([chapter 31](ch31-bounded-control.md)). Compare the
   computed stack bound with the measured native stack.
6. **Stamped sensors and the supervisor.** Stamp each joint and camera sample
   ([chapter 35](ch35-sensors-and-actuators.md)). The supervisor checks
   freshness, joint and rate limits, controller identity, authority epoch,
   sequence and expiry before any torque reaches the simulator.
7. **Faults.** Run the R19 corpus ([chapter 36](ch36-fault-scenarios.md)):
   sensor dropout, reordered samples, a timing overrun, actuator saturation
   and an invalid value. Each one gives the expected refusal, hold or safe
   stop.
8. **Replay.** Record the run with R16 ([chapter 33](ch33-replay-and-snapshots.md)),
   restore a mid-run snapshot and match the uninterrupted run. Replay at the
   observation level to see what the controller knew when it acted.
9. **Provenance.** Give the calibrated gains and limits R20 records
   ([chapter 37](ch37-provenance-and-rule-modules.md)), so the run's report says
   which values were measured and which were chosen.
10. **Moving to Aethyr's engine.** Swap MuJoCo for Aethyr's rigid and joint
    solvers
    ([Aethyr chapters 17–22](https://github.com/OpenAgentsInc/openagents/blob/main/aethyr/book/ch017-a-scene-owns-the-state.md))
    and compare the two backends on frozen recordings, as the
    [Genesis parity plan](https://github.com/OpenAgentsInc/openagents/blob/main/psionic/docs/genesis-parity.md)
    describes.

## The consumer and the gate

The consumer is Psionic chapters 168–170. Each of R12–R20 has its own gate
on this robot: typed joints and poses (R12), declared schedules (R13), a
`#bounded` control step (R14), a replayed closed loop with the solver's state
(R16), an imported arm checked against an independent parser (R17), refusals
for stale, mismatched, over-limit, revoked, reordered and expired commands
(R18), a fault corpus where removing a supervisor check makes a scenario fail
(R19), and calibrated parameters with provenance (R20).

## Done when

- R12–R20 have shipped with the robot gates above.
- The chapter's examples run under `check-examples.sh`, with exact output.
- The loop runs with no plain-number frames, clocks or limits left in the
  control path.
- The chapter says plainly that nothing in it drives real hardware. A
  hardware bridge needs its own authorization, safety review and
  qualification.

## Dependencies

- Chapters [29](ch29-units-and-frames.md) to [37](ch37-provenance-and-rule-modules.md).
- [Chapter 16](ch16-determinism.md) for strict profiles and
  [chapter 17](ch17-gpu.md) for the Metal path.
- Psionic chapters 167–170 and Aethyr Part IV.
