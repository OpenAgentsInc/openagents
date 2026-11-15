# 264. Run the policy on a real robot

> **Status: unimplemented — design draft.** This chapter plans authorized
> physical commissioning and repeatability experiments. It performs no
> purchase, powered test, deployment or safety qualification.

Our simulator pauses when inference takes longer than expected. A real
robot's momentum and gravity do not pause. The local runtime must meet its
measured contract while the UI, cameras, network and training workers are
busy or unavailable. That requires a named hardware/software profile and
evidence about actual effects.

Start with one complete, documented assembly. Record mechanical revision,
mass distribution, motors, firmware, compute, power system, sensor placement,
calibration and permitted operating area. Printable repair parts carry the
manufacturing/process revision that produced them. A change affecting mass,
friction, compliance or geometry can invalidate a policy's transfer evidence.

## Allocate a timing budget without inventing a result

Consider a proposed 20 ms application period. An illustrative budget assigns
3 ms to sensor acquisition, 1 ms to preparation, 2 ms to inference, 0.2 ms to
supervision and 0.8 ms to dispatch. Total planned work is 7 ms, leaving 13 ms.
Those are allocations, not measurements. Interrupts, queued device transfers,
preemption, contention and actual application delay must be observed too.

The release record reports each phase, end-to-end sensor-to-actuator age,
missed periods, highest observed time, distributions and test duration under
declared load. No finite measurement proves a universal worst-case bound.
Compiler-bounded storage/work, when available, is a separate property from
meeting the wall-clock period.

Keep the motor's inner firmware loop distinct from the application actor
period. Configure only a qualified control mode. A position target is not a
torque command, and lowering the actor rate does not necessarily lower the
motor's internal servo rate.

## Commission one boundary at a time

First inspect recorded sensor data with outputs disconnected. Then qualify
an isolated actuator and its stop/power behavior. Next qualify the selected
support/restraint arrangement for the assembled robot before introducing
bounded motion in a controlled area. Record which stage each result covers.
The support arrangement itself can change dynamics and cannot establish an
unrestrained walking result.

Check encoder zero/sign, joint order, camera/IMU frames, timing and calibration
against independent measurements. Start from a known posture and an admitted
package. Arming is explicit. Ensure that the local stop mechanism is effective
when the application, GPU, network or operator UI fails. Removing torque can
make an upright mechanism fall; the stop outcome must fit this assembly.

Authorize each physical experiment through the repository's owner-action
process and record it in `NEEDS_OWNER.md` when it is actually required.
Preparing software, fixtures and a reviewable experiment can proceed first.
This draft is a plan, not a pending request to move a robot now.

## Transfer is a comparison, not a video

Choose predeclared trials covering speed/turn commands, rest/recovery and
each enabled skill transition. Specify the physical unit, surface, payload,
battery/thermal conditions and operator procedure. Match those conditions
to the simulation distribution and report differences rather than silently
tuning the simulator after seeing the final tests.

Retain all trials, stops, falls and interventions. Compare joint/IMU traces,
command tracking, saturation, timing and task outcomes with simulated runs
under the appropriate uncertainty. Use held-out units or conditions where
the claim concerns generality beyond one assembled robot. A single unit
cannot supply evidence for manufacturing variation across a fleet.

Hardware faults include unplugged/reordered sensors, stale calibration,
bus loss, brownout, overheating, bad feedback and a stalled policy worker.
Observe the actual commanded and physical response. A fake-device pass from
chapter 263 supplies software coverage but cannot replace these observations.

## What closes the chapter

The record names its exact build and parameters, preserves failed trials,
shows package compatibility and qualified calibration, and demonstrates the
declared timing/fault/stop behavior and task repeatability. Independent
measurement or an independently implemented reader checks the evidence.
Physical traces are not expected to replay bit for bit.

[RB-11](../docs/robotics/microduck-implementation-roadmap.md#rb-11-authorized-physical-commissioning)
is blocked on actual hardware authorization and completed software gates,
not on a new general-purpose robotics cloud. A robot that can execute locally
without the development machine is ready for the coordination experiment in
[chapter 265](ch265-a-course-a-flock-and-a-repeatable-result.md).
