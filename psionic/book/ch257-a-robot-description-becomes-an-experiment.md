# 257. A robot description becomes an experiment

> **Status: partly implemented.** The `decoded-joint-experiment-f64-v1` CPU
> profile now admits decoded contracts and maps normalized joint actions.
> Its wire codec, general robot import and physical integration remain open.

A policy cannot discover what its action coordinates mean by looking at their
shape. Fourteen numbers could be joint angles, angular increments, velocities
or normalized efforts. A robot description adds another set of conventions:
which link owns an inertia, where each joint axis points and which transform
puts the camera in the world. Before learning, we make those facts explicit.

[Chapter 256](ch256-build-and-teach-a-small-robot.md) introduced the complete
route. Here we build its experiment contract. A contract binds a robot model,
an observation/action mapping, a task, numerical limits, a dataset partition,
an execution target and an evaluation rule. Admission returns one complete
usable description or a refusal. No admission opens a motor port.

## Work one joint through the boundary

Consider an illustrative joint, unrelated to any vendor's calibration. Its
home is 0.20 radians, its positive direction is counterclockwise in its declared
frame and a normalized action is multiplied by 0.40 radians. For action −0.5,

```text
target = home + scale * action
       = 0.20 rad + 0.40 rad * (-0.5)
       = 0.00 rad.
```

If the admitted range is [−0.10, 0.70] radians, that target fits. Action −1
requests −0.20 radians. The profile must say whether the action map clips it
to −0.10 or refuses it. For the first teaching contract, choose an explicit
bounded map that returns both requested and limited targets and a saturation
flag. Neither value is a physical command until the local supervisor admits it.

Now reverse the joint sign while keeping the same network weights. The target
for −0.5 becomes 0.40 radians. The shapes still match, but the behavior changes.
That sign belongs in the embodiment identity. A package made for the first
mapping must refuse the second, rather than silently accepting fourteen outputs.

## A small contract before a general importer

The implemented first slice accepts already decoded bounded records and checks
their consistency. It does not start with an XML parser, online downloader or
general robot compiler. Use a two-joint analytic fixture before a pinned biped.

Record, conceptually, the following groups; wire field names and IDs are not
published by this draft:

- Model and assembly revision, joint names/order, frame/unit conventions,
  source bytes and their rights.
- Observation slots: source, timestamp/frame/calibration, units, scaling,
  history, padding and actor/critic visibility.
- Action slots: target meaning, home/sign/scale, limits, motor mapping and
  the control mode that receives them.
- Physics/control periods, numerical/storage limits and explicit reset state.
- Task/reward/termination definitions, seed sets, partition identities,
  budget and predeclared evaluator.

Admission checks counts before multiplying sizes, finite values before
comparisons, unique joint slots and complete coverage before indexing, and
identity before publishing any output. Unknown observations or units refuse.
Keep compatibility records for upstream policies distinct from our owned
training profile; the 61-input/14-output reference is one contract, not a
universal small-robot interface.

Every persistent or exchanged record starts with a specification and row in
[the protocol registry](../../proto/README.md). Reuse its owning codec.
The memory layout of a Coax struct is not the persistent schema.

## Independent checks and refusals

Write checks first for the two-joint hand calculation, zero and negative-zero
actions, exact limits, sign reversal and requested/applied target reporting.
Poison each numeric group with a nonfinite value. Reject duplicate slots,
missing calibration, inverted limits, unknown conventions, short buffers and
an otherwise compatible artifact with the wrong assembly revision.

Refusal must preserve the entire caller output. An independently authored
table interpreter supplies the expected mapping, including the identity
mismatches. The runnable example shows a valid contract and then a changed joint sign
under unchanged advertised identities. It returns `RexStatus.identity` (6)
and preserves every output target. This is a query refusal, not a motor stop.

## Run the decoded query

The [frozen profile and independent table](../docs/robotics/experiment-contract-rb01.md)
state its bounds and meaning. Model, assembly and calibration identities are
matched against a caller-provided expected embodiment, together with every
active joint mapping field. The check does not authenticate an asset file or
establish where that expected embodiment came from.

The joint keeps the physical angle unit through the map:

<!-- source: psionic/lib/robotics/experiment-types.coax experiment-joint -->
```jai
RexJoint :: struct {
  joint:int; motor:int; sign:int; unit:int=1; mode:int=1;
  home:rxu.Quantity(rxu.Radian,f64); scale:rxu.Quantity(rxu.Radian,f64);
  low:rxu.Quantity(rxu.Radian,f64); high:rxu.Quantity(rxu.Radian,f64);
}
```

Admission is a complete publication after all checks succeed:

<!-- source: psionic/lib/robotics/experiment-policy.coax experiment-admit -->
```jai
rex_admit :: (c:RexExperiment,expected:RexEmbodiment,output:[]RexExperiment)->RexStatus #export_c #effects(all) {
  if output.count!=1 {return RexStatus.shape;}
  status:=rex_check(c,expected);if status!=RexStatus.ok {return status;}
  output[0]=c;return RexStatus.ok;
}
```

The action map stages every target, retaining the original request as well
as the limited value. Equality at a limit preserves requested signed-zero bits:

<!-- source: psionic/lib/robotics/experiment-policy.coax experiment-map -->
```jai
  stage:[REX_JOINTS]RexTarget;
  for i:0..<n {
    j:=c.embodiment.joints[i];requested:=j.home+j.scale*(cast(f64)j.sign*actions[i]);
    limited:=ifx requested<j.low then j.low else ifx requested>j.high then j.high else requested;
    stage[i]=RexTarget.{joint=j.joint,motor=j.motor,requested=requested,limited=limited,saturated=requested!=limited};
  }
  for i:0..<n {output[i]=stage[i];}return RexStatus.ok;
```

Run the [example](../examples/robot-experiment.coax):

```shell
coax/bin/coax run psionic/examples/robot-experiment.coax
```

```text
experiment: 2 joints, 4 observations, 4 worlds x 8 steps
joint 0 motor 1: requested -200 mrad, limited -100 mrad, saturated 1
joint 1 motor 0: requested -375 mrad, limited -250 mrad, saturated 1
changed map: status 6 (identity), targets unchanged
```

The robotics dispatcher compares this complete transcript in debug and release.
Its native qualifier separately checks the hand-derived action table and
nonfinite values with ASan/UBSan. [The receipt](../../docs/verification/robotics-experiment-rb01-2026-11-15.md)
states what passed and what remains open.

## Existing owners and the next slice

Reuse [quantities](../../coax/QUANTITIES.md),
[spatial types](../../coax/SPATIAL.md), the shared protobuf/JSON readers and
[chapter 167's existing control math](ch167-command-a-robot.md). General URDF/MJCF
admission remains engine/Coax work; fixed Panda coefficients do not become a
generic importer by renaming them.

[RB-01 in the implementation roadmap](../docs/robotics/microduck-implementation-roadmap.md#rb-01-experiment-and-embodiment-admission)
records the implemented first slice and the next codec work. [Chapter 258](ch258-a-servo-that-behaves-like-the-motor.md)
then gives the admitted joint a response model instead of assuming that it
instantly reaches its target.
