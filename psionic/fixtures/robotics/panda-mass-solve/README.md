# Fixed Panda net-force mass solve

`pms_forward` solves `M(q) * acceleration = NET generalized force` for the
already qualified sixteen-body/nine-coordinate Panda. It constructs the owned
mass metric from seven radian and two metre positions. The caller supplies
seven net forces in energy/radian and two in newtons; the output has seven
Radian/Second² and two Metre/Second² quantities. `pms_forward_raw` attaches those
explicit labels to F64 inputs. Neither entry assembles gravity, velocity bias,
passive/actuator/contact forces, nor advances a simulator.

Shapes are checked before numerical admission. Position admission keeps the
existing ±100rad/±1m mathematical box, then net forces are checked in index
order for finiteness and inclusive magnitude 10000 in their declared units.
Only a complete admitted acceleration record is published; refusal preserves
all nine caller output words. Native callers supply valid disjoint backing
storage and truthful extents. These numerical bounds do not admit motor commands.

The quantity normalization is explicit. For each coordinate choose one Radian
or one Metre and collect those bases in diagonal D. `Dᵀ M D` then has kg·m²
coefficients, and `Dᵀ F_net` has joule coefficients. New Coax F64 Cholesky and
two triangular substitutions solve for coefficients in Second⁻². Multiplying
by the original coordinate bases restores angular/linear accelerations. There
is no implicit angle erasure, quantity square root or BLAS call.

The raw numerical profile admits finite matrix entries with magnitude at most
1e8, finite forces at most 1e4, exact numerical symmetry, and every Cholesky
Schur pivot at least .25 (inclusive). This is a fixed-model profile, not generic
SPD acceptance. The normalized source armature minimum is .714286, and the body
kinetic metric is positive semidefinite; .25 is a conservative floor for this
model. Fixed dimension, coefficient/force caps and this floor bound arithmetic.

`pms_basis`, `pms_restore`, `pms_publish`, `pms_factor` and
`pms_solve_force_admitted` remain source-visible helpers with trusted
preconditions. The last helper assumes that force was already admitted in the
declared basis; it separately admits the matrix. The public fixed-source path
admits original force exactly once before quantity arithmetic. The complete
checked raw numerical wrapper is exercised through a test-only C seam. POD
fields, status fields and absence from a C header do not authenticate provenance
or seal Coax source modules.

[experiment.json](experiment.json) froze limits, scalar comparison budgets and
129 new model contexts before implementation observations. Two force patterns
use all 57 previously qualified metric configurations, including numerical
position boundaries. Fifteen additional contexts construct force from each of
five original SDK matrices times three independently prescribed accelerations.
Seven raw numerical profiles cover diagonal/dense rational SPD matrices,
inclusive pivot/coefficient/force bounds, and signed zero.

The bundled `cached-metric-*` files are PREVIOUSLY QUALIFIED COAX COMPONENT
outputs and inputs, not new SDK measurements. Their exact hashes point to the
older M9 qualification. The nine cached gravity words are never added as forces.
The five SDK matrices also retain their distinct original provenance. New
product calls evaluate M9 inside the solver; no previous M9 mutation/native/FD,
SDK, renderer, neural model or episode gate reruns.

[qualification.json](qualification.json) records 213 new-root mutants killed:
math98, basis60, public wrapper55, with zero survivors/waivers. Four source
fixtures refuse wrong position/force/result units. New native debug, release
and ASan/UBSan modes each pass 10,028 ABI/refusal assertions and publish 1,224
words. Their complete outputs agree byte for byte under the declared
Darwin-arm64 scalar-libm profile, not a cross-target/R15 promise.

An independent row-pivoted Gaussian elimination checks all 1,224 acceleration
values; largest error is 3.64e-12. Residuals against the bound reference matrices
reach 9.10e-12. Independently prescribed acceleration values reach 7.78e-16.
The frozen acceleration allowance is `2e-10 + 2e-12*abs(reference)`; residual
allowance is `2e-9 + 2e-12*abs(force)`. These apply per scalar in the explicit
one-Radian/one-Metre basis, not to a norm over incompatible coordinates. Every
actual/reference array is checked for complete extent and finite values before
any maximum. Four new tooling guard groups cover malformed/nonfinite arrays,
budget boundaries and the independent Gaussian basic cases.

The first native test harness accidentally retained a NaN between two separate
fault setup cases. [initial-harness-diagnostic.json](initial-harness-diagnostic.json)
retains that failure. Only the new C test setup changed: clear all input words
between cases. Exact independent Gaussian/case/source-refusal/debug-emission
facts were carried forward; they were not recomputed. Product source and its
213-mutant proof remained unchanged, and the repaired harness passed all modes.

```shell
COAX=coax/bin/coax psionic/tests/robotics/panda_mass_solve_check.sh /tmp/panda-mass-solve-new
python3 psionic/tests/robotics/panda_mass_solve_guard_checks.py
coax/bin/coax run psionic/examples/panda-mass-solve.coax
```

The output directory must be new. The default proof uses committed reference
bytes, Coax, a C compiler and Python stdlib test tooling; the product requires
no Python or third-party runtime. The example's prewritten golden follows a
chosen left-slide acceleration of 2m/s² and the independently known 1.11kg
effective slide mass, hence 2.22N. It applies the whole mass column so that the
other coordinates have zero prescribed acceleration; applying only the scalar
slide force would generally accelerate the coupled arm too.

A future composition with inverse dynamics must first assemble NET force
under its own contract. Its nominal `PidAngularAcceleration` and
`PidLinearAcceleration` units also need explicit `convert_unit` from these
formal quotient units. This slice does not qualify full forward dynamics,
force assembly, contacts, integration, compiler `#bounded`, hardware control
or observation/clock provenance.
