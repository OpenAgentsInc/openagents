# Owned Panda query source: first shadow integration

`panda-owned-query-shadow-v1` selects the fixed Coax-owned nine-coordinate
controller sample. Seven coordinates are radians and two finger slides are
metres; rates use seconds. It reuses the qualified owned FK, grip-site
Jacobian, mass and inverse-dynamics producers. SDK mode selects the SDK's
144 controller fields from the unchanged 173-word telemetry layout.

This profile requires both the SDK telemetry effect and the new
`PandaRobotStateV1` effect. The latter copies nine named qpos and nine named
DOF velocities without forwarding or advancing the model. Shape, mode,
effect success, every required finite word, and the selected sample's
mathematical admission are checked in that order. An owned refusal never
falls back to SDK data. All failed selections preserve the output.

Owned selection can ignore a finite invalid SDK mass or rotation because
those fields are unselected. Nonfinite SDK shadow data still refuses this
first profile. SDK `pc_sample` validates numbers, pose and actuator limits;
matrix inversion occurs later in controller reset/step. Its admission alone
does not prove a positive-definite mass matrix.

`pqs_sdk` and the imported low-level Coax helpers remain source-visible and
have trusted input preconditions. They are not sealed source-provenance APIs.
The exported `pqs_select` is the admitted source-selection boundary.

## New evidence

The checks precede the selector implementation. Its changed-root mutation
run killed **97/97** sites without a waiver. An initial surviving rate-copy
site led to a valid nonzero-rate fixture. The executed proof and a later
test-title-only correction are both preserved in `qualification.json`.

The new native selector harness compared 15 existing SDK/owned sample pairs
in debug, release and fail-fast ASan+UBSan builds. Each checked 4,320 field
words bit for bit and made 5,595 assertions, including actual NaN/Inf input
words, metadata precedence and atomic refusals. Reconstructed 173-word
native inputs have explicitly zero unused telemetry lanes; they are not
claimed as captured full telemetry records. Prior sample/controller gates
were reused rather than rerun.

The new SDK experiment used the authenticated `demo_000` XML/reset pair
and delayed-first-request action fixture. One 250-step run used SDK samples
for control; another used owned samples. Both recomputed controls from the
same recorded **action** words, maintained an independently evolving
opposite-source controller at each actual physical state, and advanced
MuJoCo 3.3.1 at 500 Hz. They never copied the original actuator controls.
There were exactly 500 new integration steps and no rendering or neural
forward passes. Each run had request origins `[0,25,225]`, one fallback row
and one refused late reply.

The frozen tolerances and source identities are in `prefix-experiment.json`.
All 1,000 normal selected/shadow controller calls returned OK. Local shadow
torque error stayed below `1.38e-12`; the largest controller-state difference
was `4.33e-15`. Across the two independently integrated runs:

| Quantity | Maximum absolute difference | Frozen budget |
| --- | ---: | ---: |
| Applied arm torque | `1.8634e-12` | `1e-6` |
| Model qpos words, mixed coordinates | `1.3323e-15` | `1e-8` |
| Model qvel words, mixed rates | `7.2406e-14` | `1e-7` |
| Applied gripper command | `0` | `2e-12` |
| Request/fresh snapshot geometry | `5.5512e-16` | `1e-8` |
| Request F32 observation | `0`, also bit-identical | `1e-6` |

SDK time bits, logical record/action/flag fields, cached/fresh stamps and
controller status/ready/clip counts agreed exactly. Null-effect probes and
before/after state copies qualified the new read-only effect. The first-nine
flat-state address assertion is specific to this pinned compiled model;
the effect resolves each joint's qpos and DOF address independently.

Request observations deliberately retain SDK cached geometry **before** the
next forward. Control samples are selected **after** an explicit forward.
The SDK's current integration/contact machinery and all its shadow geometry
queries remain. This is a short controlled prefix, not a new neural episode,
task-success result, hardware command certificate, SDK geometry elimination,
or complete compiler clock/boundedness proof. The full neural host retains
its prior query path until explicitly integrated with this selector.

## Reproduction boundaries

The portable selector harness uses only committed reference data and a
chosen compiler, and performs no SDK calls:

```shell
python3 psionic/tests/robotics/panda_query_source_native_reference.py \
  --compiler coax/bin/coax --output /tmp/NEW-query-source-native
coax/bin/coax run psionic/examples/panda-query-source.coax
```

The opt-in prefix driver has separate preparation and execution phases. It
authenticates the exact private XML, reset, trace and original case ledger,
freezes the generated C/header/binary and recursive source dependency
inventory, then allows the two new executions once. It currently pins the
actually qualified compiler and SDK identities in its proof. A new output
directory is required; an earlier completed prefix is never rerun as a
routine check. See `prefix-qualification.json` for exact executed commands,
artifact hashes, empty stderr receipts and the preparation-only provenance
follow-up. Private scene paths/data and generated trajectory binaries are
not committed.

`example-reference.json` records the independent prewritten transcript:
at the geometric zero arm pose, a `0.088 m` lever arm and `0.5 rad/s` first
hinge rate imply `0.044 m/s` world-Y grip-site velocity. Joint4=0 is outside
the source's physical soft range, so this teaching query issues no command.
