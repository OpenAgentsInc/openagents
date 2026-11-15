# 258. A servo that behaves like the motor

> **Status: unimplemented — design draft.** The proposed actuator slice is a
> bounded CPU reference with independently checked response and state. No
> coefficients below describe a measured Microduck or qualify hardware.

The learner can exploit any gap between our simulated motor and the real one.
If a simulated joint reaches a target immediately, it can learn a gait that
depends on impossible acceleration. If we omit gearbox play, it can rely on
tiny reversals that a worn transmission never delivers. We need a model whose
limits and uncertainty are visible to the experiment.

Start with a small declared servo model, then add terms only when a reference
or measurement supports them. A complete vendor actuator model is a separate
profile. The scalar oscillator and fixed Panda force components are useful
arithmetic references; neither is already a general small-servo implementation.

## Requested torque, delivered torque and motion

For a teaching calculation, let a joint have inertia 0.01 kg·m², no gravity or
friction in this isolated example, zero position/rate and a target of 0.10 rad.
Choose a proportional gain of 2 N·m/rad and a 0.15 N·m torque cap:

```text
requested torque = 2 * (0.10 - 0) = 0.20 N*m
delivered torque = 0.15 N*m
acceleration     = 0.15 / 0.01 = 15 rad/s^2
```

For a separately declared semi-implicit 0.002-second step, the new rate is
0.03 rad/s and the new position is 0.00006 rad. This calculation demonstrates
force limiting and integration order. It says nothing about closed-loop
stability, motor current, a gearbox or contact. Keep those statements separate.

A two-update command delay adds state. If the initial command is zero, the
first two application updates still use zero; the third uses the first
accepted target. A restored simulation needs the delay queue and its cursor,
not only the joint's position and rate.

## Model the boundary in stages

The first map reads admitted joint state, a target and parameters, then returns
requested force, limited force and diagnostics. It owns no device effect.
Stage all outputs before publication; failure cannot partly advance a queue.

The next profile introduces damping/friction, voltage-dependent effort and a
declared delay. Define the sign convention at zero speed. A Stribeck curve,
back-EMF, firmware control law or gearbox backlash needs an explicit equation
and parameter provenance before entering a release. Do not claim fidelity by
collecting a list of familiar motor terms.

For backlash, distinguish actuator-side coordinate from output-side coordinate.
Define which side an encoder measures. A simple play operator is a teaching
precursor; reference compatibility may require explicit passive coordinates
and contact/constraint behavior. Freeze which model the policy learned against.

Randomization samples a parameter once at a declared boundary: per unit,
episode or step. Battery sag is not independent white noise unless that is the
chosen physical hypothesis. A sampled delay and a sampled friction coefficient
need separate provenance and independent counters.

## Identification is an experiment

Reuse [chapter 111](ch111-recover-the-physics-from-the-motion.md)'s lesson:
different parameter combinations can explain the same observations. Fit on
one collection of response trajectories and evaluate on distinct excitation,
load, voltage and direction changes. Report residuals and ambiguity. A low
training error is insufficient evidence that the identified motor will work
through the contact loads of walking.

Physical identification is a later authorized bench activity. Before that,
independent analytic trajectories and permitted frozen reference captures
qualify the numerical map. Keep coefficients labeled as illustrative, sourced,
fitted or measured, with their experiment identity.

## Checks that make the model useful

Check exact effort caps and delay boundaries; zero/reversed motion; passive
power under the stated friction law; held targets; unit mismatch refusals;
nonfinite/negative inertia; and full output preservation on failure. Test a
checkpoint with a nonempty delay queue against uninterrupted continuation.
Use analytic torque/inertia results and a separately written discrete-state
reference. Broader energy/stability claims need their own physical-system proof.

Extend or extract the relevant pure query in
[`psionic/lib/robotics/`](../lib/robotics/) once rather than copying the Panda
controller. Engine integration must preserve the downward layer rule: the
engine takes already computed generalized force through its boundary or uses
a deliberately extracted lower-level primitive; it does not load Psionic.

[RB-02](../docs/robotics/microduck-implementation-roadmap.md#rb-02-actuator-response-and-delay)
implements this numerical precursor. The learner's data collector in
[chapter 259](ch259-rollouts-from-independent-worlds.md) will record both the
sampled action and what the actuator actually receives.
