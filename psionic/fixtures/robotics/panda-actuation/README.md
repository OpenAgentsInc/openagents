# Source Panda actuators and joint-passive forces

`pa_forward` consumes seven radian positions, two metre slide positions, their
angular/linear rates, seven arm controls in energy/radian and two signed finger
position targets in metres. Its `PaOutput` keeps actuator generalized forces
and joint-passive generalized forces separate, with strict control/force
clipping counts. The two `PidOutput` records preserve seven energy/radian and
two newton quantities each. `pa_forward_raw` labels three packed nine-word
F64 views explicitly. No normalized gripper-command transform or right-finger
sign flip occurs here: the targets are already signed joint coordinates.

The compiled source specialization has nine direct joint transmissions with
unit gear, no activation states, fixed arm gain 1 and no bias, and finger gain 1000
with affine bias `[0,-1000,0]`. The finger calculation preserves separate
`gain*target` and affine bias operations rather than factoring to `k*(target-q)`.
Controls clamp first: five arm ranges are ±80, two are ±12, and finger targets
have ranges `[0,.04]` and `[-.04,0]` metres. Finger actuation-space forces then
clamp to ±20N. Unit sparse moments map each actuator to exactly its expected
DOF; joint-level actuator force limits and actuator gravity compensation are
absent in this admitted source profile. These facts are verified on the actual
compiled SDK model before any case forward, and actual sparse rows are checked
after each forward. [Official source order](https://github.com/google-deepmind/mujoco/blob/a209134315dda126a714529bfa80f7478902644d/src/engine/engine_forward.c#L244).

All span shapes precede numerical admission. Inclusive mathematical boxes are
±100rad/±1m for position, ±20rad/s/±2m/s for rate, and ±10000 energy/radian/±1m
for arm control/finger target. Nonfinite and outside-box values refuse atomically;
the SDK's bad-control warning/all-controls-zero fallback is not reproduced.
Finite controls inside those broad boxes follow the exact source clipping map.
Endpoints do not count as clipping; counts describe strict changes. Success
preserves disjoint inputs and spare output. Refusal preserves every output byte.
Valid overlapping input/output storage is supported by snapshot-before-publication
and tested for all three packed views; callers still supply truthful extents.

The source joint spring coefficients are exactly zero. Owned joint passive
therefore specializes to viscous damping: coefficients 0.1 for the first five
hinges, 0.01 for the final two, and 100 for each slide, with physical
energy·second/radian² and newton·second/metre units. The position servo gain 1000
is actuator bias, not a passive joint spring. Source-visible helpers and PODs
are unsealed and trust their stated finite/bound/source-profile preconditions;
this is not an arbitrary actuator importer or provenance certificate.

Joint passive is **not full passive force**. The original scene enables fluid
at density 1.2 and viscosity 2e-5, and MuJoCo adds fluid to spring/damper force.
The captured corpus has 56 nonzero hinge-fluid coefficients (maximum absolute
4.701012241 energy/radian) and 16 slide-fluid coefficients (maximum absolute
0.859280139 N). Those are separate-unit diagnostic maxima, not a mixed force
norm. Fluid, gravity-compensation and total passive vectors are retained for
future work. `dof_frictionloss` is a constraint parameter; no Coulomb term is
inserted into this joint-passive output. [Passive-force assembly](https://github.com/google-deepmind/mujoco/blob/a209134315dda126a714529bfa80f7478902644d/src/engine/engine_passive.c#L399).

[experiment.json](experiment.json) froze 18 q/v/control contexts before capture
or implementation. Twelve contexts combine three states with four control
patterns, including control-clamp cases deliberately below the later force cap.
Six additional contexts bracket both finger force limits by exact and one-ULP
inside/outside values. The immutable SDK acquisition performs 18 new forwards,
zero steps/render/NN, checks 162 actual sparse rows and captures all 2916 finite
words: 11 nine-word force/transmission/control arrays plus full 21
`qacc_smooth`, `qacc` and `qfrc_constraint` future diagnostics. The metadata also
retains all 26 bodies and 152 geoms/fluid records. The SDK is not invoked by the
ordinary offline component gate. See [SDK-README.md](SDK-README.md).

The first debug, release and fail-fast ASan/UBSan native attempt passes 3,960
assertions per profile. Each of 162 actuator, 162 generalized actuator and 162
joint-passive coefficients agrees numerically exactly with retained SDK and
independent scalar equations under frozen absolute 5e-11/zero-relative budgets.
Clipping counts match the independent source-profile oracle exactly; SDK force
fields themselves do not expose clipping counters. All 324 actual force words
are finite before maxima. All force/count output bytes agree between the three
owned profiles. Native cases include 135 IEEE input poisons, count extremes,
36 position/rate and 18 control endpoints, input/spare/canary checks, raw/typed
identity and four overlap success/refusal cases. Four new compile refusals keep
control, target, rate and output roles distinct.

Eight Coax groups cover 135 cases. The math scope kills 20/20 emitted mutants;
the first wrapper scope kills 97/98. Its one survivor exposed a missing public
raw-coordinate refusal observation. Adding 54 raw admission cases changes no
product body, and a raw-procedure-only 25/25 follow-up closes it; the unchanged
`pa_forward` body carries its initial kills by exact body identity. The receipt
binds that body and the final passing `pa_forward_raw` scope. No final full-wrapper
rerun, equivalent waiver or repeated old passing gate is claimed. Scalar-quantity lowering still does not enumerate every authored typed
operator token; literal differences and the independent SDK/scalar corpus cover
the physical equations without overstating mutation-token coverage.

[qualification.json](qualification.json) contains the first native proof;
[checks-qualification.json](checks-qualification.json) binds the Coax scopes,
retained failed first run and runnable example. Target qualification is Darwin
arm64 F64 without fast math or contraction. No cross-target/R15, full passive,
friction/contact constraint, full qacc, integration, generic import, compiler
`#bounded`, rollout, command authorization or hardware claim is made.

```shell
COAX=coax/bin/coax psionic/tests/robotics/panda_actuation_check.sh /tmp/panda-actuation-new
coax/bin/coax run psionic/examples/panda-actuation.coax
coax/bin/coax run psionic/examples/panda-actuation.coax --release
```
