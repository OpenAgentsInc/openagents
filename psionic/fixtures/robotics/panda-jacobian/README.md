# Owned Panda grip-site Jacobian

`panda-jacobian.coax` computes the fixed seven-hinge model's world-space
geometric grip-site Jacobian. It calls the frozen owned FK, computes seven
world axes/anchors, and publishes one complete `PjOutput`. It needs no SDK,
inverses, dynamics, renderer, neural model or original-weight execution.

```jai
q: [7] pfku.Quantity(pfku.Radian, f64);
j: [1] PjOutput;
status := pj_forward(q[..], j[..]);
```

Check `status == PfkStatus.ok` before consuming the result. The typed entry
accepts exactly seven radian quantities and exactly one output slot; the raw
adapter is named `pj_forward_raw` and explicitly labels its F64 inputs radians.
Shape/number/range refusals preserve caller output. The geometric domain is
FK's finite inclusive ±100 radians, including singular configurations. It is
not an actuator-limit or physical-command permit. `pj_project_owned` is absent from the C exports and is coordinate arithmetic
only, not an admission or origin certificate for an arbitrary caller-supplied
FK record. It remains source-visible through `#load`; this is not a claim of
general source access control.

`linear[21]` is row-major world XYZ by joint1..7 in Metre/Radian;
`angular[21]` is the angular-radian/joint-radian ratio `UnitOne`. These are a
named per-radian derivative boundary, with no implicit erasure of physical
angle units. Multiplication by a joint rate in Radian/Second gives linear
Metre/Second and angular Radian/Second. The angular block is geometric world
angular velocity, not the derivative of the hand-BODY axis-angle observation.

The frozen profile has zero local hinge anchors/references and local Z axes,
so each world axis is rotation column `(2,5,8)` from FK body `links[2+i]` and
each anchor is that body's world position. For each column:

```jai
linear = world_axis cross (grip_position - world_anchor);
angular = world_axis;
```

The implementation spells out raw coordinate products. This does not add a
generic typed cross product, angular role, transform chain or R13 epoch proof.
Only this source-bound twelve-body/seven-hinge profile is supported.

`experiment.json` froze the exact corpus, source identities and budgets before
comparison: 62 configurations, 47 interior finite-difference cases, retained
SDK Jacobian absolute tolerance `2e-12`, and primary finite-difference step
`h=1e-5` radians with absolute tolerance `3e-8`. The independent oracle uses
homogeneous matrices, not the product quaternion calculation or another
axis-cross-product implementation. Its angular reference uses the WORLD-left
increment `Rplus * Rminus^T` and an atan2 SO(3) log. It does not differentiate
quaternion signs or treat an axis-angle coordinate derivative as angular J.
The ±100 cases test admission and finite results; the product API is not
perturbed outside its admitted domain. No derivative-of-refusal or derivative
of the bit-rounded program is claimed.

The seven original PcSamples supply all 294 original SDK Jacobian values by
hash. They are fresh reset+forward grip-SITE captures. There are no new SDK
calls. Their old standalone controller/FK gates are reused rather than rerun.
New native debug/release/fail-fast ASan/UBSan each passed 770 ABI/admission
assertions and 62×42 derivative words, with identical complete output bytes.
The first interpreter negative fixture could not construct a nonfinite float
and was corrected; native ABI checks supply all-axis NaN/Inf cases. The first
oracle attempt used the argument diagnostic E0317 for return-unit refusals;
those correctly use E0330. Its failed receipt and prepared matrix tables were
retained, then the unchanged prepared recipe/input was reused without
repeating successful phases.

The arithmetic root killed 33/33 mutants. Two initial survivors exposed a
cardinal-fixture alias between adjacent matrix entries; a dense proper-rotation
literal check closed that gap. The wrapper killed 14/14. No equivalent waiver
was used.

Measured maxima under the frozen numerical profile:

| Comparison | Maximum absolute error | Frozen budget |
| --- | ---: | ---: |
| All 294 original SDK J entries | 1.1102230246251565e-15 | 2e-12 |
| Primary h=1e-5 position FD | 3.264755132903474e-11 | 3e-8 |
| Primary h=1e-5 angular FD | 1.5693224497681513e-11 | 3e-8 |
| Retained world site J×qd | 1.6653345369377348e-16 | 2e-10 |

The whole h sweep is recorded. At h=0.001, linear FD error was
`1.5446327883417155e-7`, outside the primary budget; h=1e-5 was selected before
comparison and remains the acceptance step. Smaller h eventually increases
roundoff. No best step was chosen after observing results.

```shell
COAX=/path/to/qualified/coax \
psionic/tests/robotics/panda_jacobian_check.sh /tmp/panda-jacobian-new
```

The shell entry uses the self-contained retained-controller bundle once
installed by the FK fixture work; `PANDA_RETAINED_CONTROLLER` can explicitly
select the original acquired directory. The Python driver also accepts
`--retained`. Its optional `--prepared` reuses a previous pre-native prepared
FD/input artifact only after hash, compiler/platform, source-recipe AST,
experiment, shape and refusal checks. Existing/partial output directories are
never overwritten. Python is development tooling, not a product dependency.

The profile is Darwin-arm64 F64 scalar libm with `-fno-builtin-sin`,
`-fno-builtin-cos`, no fast math and no contraction. It does not promise
cross-target deterministic math, a compiler `#bounded` proof, a well-conditioned
controller inverse, synchronized mixed-age telemetry, or hardware control.
The actual merged 0.45 publication compiler emitted byte-identical debug/release
C and headers; its default C also matches the previously executed ASan source.
These numerical results were reused, not rerun. A guard-only tooling followup
now rejects nonfinite expected entries before max; all 10,206 cached expected
values were checked finite without recalculating derivatives or native output.
