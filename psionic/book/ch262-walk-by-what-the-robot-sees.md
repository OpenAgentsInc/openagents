# 262. Walk by what the robot sees

> **Status: unimplemented — design draft.** Camera/depth task observation,
> perception-driven navigation and teacher/student training are planned.
> Existing image operators and world-model fixtures do not qualify this route.

The simulator knows every obstacle's location. The physical robot has only
its sensors and calibration. Giving the deployed actor simulator truth during
evaluation hides the hardest part of the task. We first separate what each
component can observe, then give the walking policy commands from a local
planner using those permitted observations.

Keep balance, perception and mission progress as separate results. A robot
can remain upright while walking into an obstacle or failing to reach its
destination. A correct detector does not establish that the locomotion
controller can execute the path.

## A geometric observation we can work out

For an illustrative pinhole camera, use focal length 100 pixels, principal
point (50, 40), image coordinate (60, 40) and axial depth 2 metres. In the
declared optical convention:

```text
x = (u - cx) * z / fx = (60 - 50) * 2 / 100 = 0.2 m
y = (v - cy) * z / fy = 0 m
z = 2 m.
```

This is depth along the optical axis, not Euclidean range along the ray.
Apply the camera-to-body transform and its timestamp/calibration before
comparing the point with a body-frame obstacle envelope. A zero or absent
depth sample is missing data under this draft profile, not a free-space ray.

The first hand-check uses a small depth grid and fixed transforms. It does
not need a learned world model, a photorealistic course or a large visual
encoder. Later observations must preserve the same physical meaning.

## Three interfaces, three tests

The sensor adapter delivers admitted image/depth data with shape, intrinsics,
extrinsics, clock/instant, sequence and calibration. The perception component
returns an explicitly framed obstacle representation with freshness and
uncertainty. The planner returns a bounded body-velocity/head command or a
blocked/stale result. The walking actor remains the motion controller.

Reuse [image operators](ch142-pixels-convolutions-and-patches.md), existing
engine cameras/spatial queries and the declared observation record work in
[chapter 165](ch165-sequences-of-observations.md). Extend their owners; do not
write a new image normalizer or collision library inside the demonstration.
Physical sensor simulation belongs to engine chapters 33–34, not a hidden
renderer embedded in Psionic's collector.

## Start geometric, then learn only the needed part

Begin with calibrated depth and a geometric local planner against simple
held-out obstacles. Establish range, collision and course-completion metrics.
Then add an encoder or task-specific head and compare against that baseline
under identical courses and budgets. Freeze input resizing, masks, depth
units and invalid-pixel handling before training.

A privileged teacher may use full simulator state during training. A student
receives only the sensor/history channels available on the robot. Store
teacher and student observation contracts separately. Training the student
on teacher actions does not prove transfer; evaluate independent closed-loop
student rollouts, including the student's own errors and recovery states.

Randomize appearance separately from physical parameters. Keep related scene
layouts, renders and synthetic descendants in the same split group. Agent
course generation cannot inspect the final held-out distribution and then
claim that distribution as unseen. Record every exposure.

## The first course suite

Specify a straight corridor, a turn, a narrow passage, a blocked route and a
case with delayed or absent depth. Select dimensions after the embodiment's
measured envelope is known. Make collision/clearance, goal crossing and stable
arrival explicit predicates. If no route exists, a correct blocked result
must not be counted as a navigation failure or as task completion.

Check the hand back-projection, transform composition, axial-depth/range
distinction, invalid pixels, stale calibration and clock mismatch. Independent
analytic geometry and brute-force occupancy provide oracles. In the actual
scene, report perception error, contact/collision, walking stability, path
completion and intervention separately. A frame that looks right is insufficient.

[RB-09](../docs/robotics/microduck-implementation-roadmap.md#rb-09-sensors-perception-and-courses)
implements the sensor/geometry baseline before learning a replacement.
[Chapter 263](ch263-export-a-policy-and-supervise-its-steps.md) packages the
complete admitted policy and preprocessing contract.
