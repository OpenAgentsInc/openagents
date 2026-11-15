# 258. A servo that behaves like the motor

> **Implemented profile: scalar-servo-play-f64-v1 (CPU).** Requested/limited
> force, explicit delay, a semi-implicit scalar rotor and a separate output-play
> observation have independent numerical checks. All coefficients are illustrative.

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

The implemented first map reads admitted joint state, a target and parameters, then returns
requested force, limited force and diagnostics. It owns no device effect.
Stage all outputs before publication; failure cannot partly advance a queue.

This CPU profile also declares viscous/Coulomb friction, constant voltage
scaling and a delay of 0–16 accepted updates. It uses motor-side feedback.
At exactly zero speed the Coulomb term is zero. The constant voltage fraction
scales the effort cap; it is not a battery model. Define the sign convention at zero speed. A Stribeck curve,
back-EMF, firmware control law or gearbox backlash needs an explicit equation
and parameter provenance before entering a release. Do not claim fidelity by
collecting a list of familiar motor terms.

The implemented play operator keeps its previous output in the interval
`[new_motor_position - play, new_motor_position + play]`, moving to the closest
boundary when necessary. It applies no load reaction. It is an observation
surrogate with explicit state, rather than a mechanical gearbox.

For physical backlash, distinguish actuator-side coordinate from output-side coordinate.
Define which side an encoder measures. A simple play operator is a teaching
precursor; reference compatibility may require explicit passive coordinates
and contact/constraint behavior. Freeze which model the policy learned against.

Randomization samples a parameter once at a declared boundary: per unit,
episode or step. Battery sag is not independent white noise unless that is the
chosen physical hypothesis. A sampled delay and a sampled friction coefficient
need separate provenance and independent counters.

## Run the checked scalar profile

The [frozen contract](../docs/robotics/actuator-contract-rb02.md) fixes every
equation, operation order, numerical box and state field. Physical damping
contributes explicit force in this profile. The older rotor query's implicit
damping map retains its own identity and evidence.

These are the independently visible force terms:

<!-- source: psionic/lib/robotics/actuator.coax actuator-force -->
```jai
rsa_force_owned :: (p:RsaParams,position:rpu.Quantity(rpu.Radian,f64),rate:rpu.Quantity(RpdRate,f64),target:rpu.Quantity(rpu.Radian,f64))->RsaForce #effects(all) {
  f:RsaForce;zero:rpu.Quantity(RpdForce,f64);
  f.proportional=convert_unit(RpdForce,p.kp*(target-position));
  f.derivative=zero-convert_unit(RpdForce,p.kd*rate);
  f.requested=f.proportional+f.derivative;f.limit=p.effort*p.voltage;
  clipped:=scalar_clip_f64(f.requested.value,-f.limit.value,f.limit.value);
  f.delivered.value=clipped.value;f.clipped=clipped.clipped;
  direction:=ifx rate.value>0 then 1.0 else ifx rate.value<0 then -1.0 else 0.0;
  f.passive=(zero-convert_unit(RpdForce,p.viscous*rate))-p.coulomb*direction;
  f.net=f.delivered+f.passive;return f;
}
```

The snapshot carries its parameters as well as pending commands. Restoring
only position and velocity would change the next delayed action:

<!-- source: psionic/lib/robotics/actuator-types.coax actuator-state -->
```jai
RsaSnapshot :: struct {
  params:RsaParams;
  position:rpu.Quantity(rpu.Radian,f64);rate:rpu.Quantity(RpdRate,f64);
  output_position:rpu.Quantity(rpu.Radian,f64);
  pending:[RSA_DELAY]rpu.Quantity(rpu.Radian,f64);
  cursor:int;steps:int;
}
```

The candidate state is staged before the final numerical admission. A refusal
cannot consume a queue entry or increment the accepted-step counter:

<!-- source: psionic/lib/robotics/actuator.coax actuator-step -->
```jai
  a:RsaStep;a.snapshot=previous;a.applied_target=target;p:=previous.params;
  if p.delay>0 {
    a.applied_target=previous.pending[previous.cursor];
    a.snapshot.pending[previous.cursor]=target;
    a.snapshot.cursor=(previous.cursor+1)%p.delay;
  }
  a.force=rsa_force_owned(p,previous.position,previous.rate,a.applied_target);
  a.acceleration=convert_unit(RpdAcceleration,a.force.net/p.inertia);
  a.snapshot.rate=previous.rate+convert_unit(RpdRate,p.step*a.acceleration);
  a.snapshot.position=previous.position+convert_unit(rpu.Radian,p.step*a.snapshot.rate);
  play:=scalar_clip_f64(previous.output_position.value,a.snapshot.position.value-p.play.value,a.snapshot.position.value+p.play.value);
  a.snapshot.output_position.value=play.value;
  a.output_rate=convert_unit(RpdRate,(a.snapshot.output_position-previous.output_position)/p.step);
  a.snapshot.steps=previous.steps+1;
  status=rsa_snapshot(a.snapshot);if status!=RsaStatus.ok {return status;}
  output[0]=a;return RsaStatus.ok;
```

Run the [example](../examples/robot-actuator.coax), which obtains its target
through chapter 257's admitted action map:

```shell
coax/bin/coax run psionic/examples/robot-actuator.coax
```

```text
servo: requested 200 mNm, delivered 150 mNm, clipped 1
step: rate 30 mrad/s, position 60 urad
delay 2: first two targets 0, restored third target 100 mrad
negative inertia: status 3 (range), output unchanged
```

Both valid and refused queries finish with example exit status zero. The
negative-inertia query returns `RsaStatus.range` (3), preserving every output
field. Separately, giving a metre-valued target to `rsa_force` fails compilation
with `E0317`; assigning force as inertia fails with `E0327`. The dispatcher
requires those exact diagnostics and nonzero compiler exits.

An independent Fraction oracle retains 56 complete steps and 17 columns per
step, including a reversal with derivative/passive terms and half available
effort. Coax checks and native debug/release/sanitizer runs compare all numerical
outputs with the frozen tolerance, and exact flags/cursors/counts. Pending-queue
restore, delay zero/full capacity, exhausted counters, every parameter endpoint,
nonfinite active/inactive values and atomic derived-state failure have separate
checks. The [receipt](../../docs/verification/robotics-actuator-rb02-2026-11-15.md)
records observed qualification and scope.

No persistent snapshot format is implemented here. Chapter 260's complete
experiment resume must register its companion before storing these values.

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
