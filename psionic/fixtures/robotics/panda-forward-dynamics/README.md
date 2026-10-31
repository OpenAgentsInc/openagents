# Fixed Panda applied-force forward query

`pfd_forward` answers `M(q) * acceleration = applied - bias(q, rate)` for the
qualified sixteen-body, nine-coordinate Panda. Seven position coordinates are
radians and two are metres; rates are radians/second and metres/second. Applied
hinge forces have energy/radian units, and applied slide forces have newton
units. The returned existing `PmsOutput` preserves seven formal
Radian/Second² and two Metre/Second² quantities. Finger-position targets from
the controller are metres and cannot be supplied as these newton forces.

The public entry checks all span shapes before numerical values, delegates
position admission to owned FK, admits rates, and admits applied forces before
quantity arithmetic. One fresh FK and one locally constructed source profile
produce the full owned metric and zero-acceleration inverse-force bias. Matching
typed force quantities are subtracted, and net forces are separately admitted.
The existing explicit one-radian/one-metre PMS basis, matrix admission,
Cholesky solve and atomic publication complete the query. Applied and net
coefficients each have their own inclusive magnitude-10000 numerical limit;
there is no clipping. These are not physical actuator limits.

Position boxes remain ±100rad/±1m; rate boxes are ±20rad/s/±2m/s. Refusal leaves
the complete caller output unchanged. Disjoint inputs and spare records remain
unchanged on success. This wrapper also reads all needed input before output
publication: the new native witness checks output overlapping all three packed
input views, including bytewise refusal preservation. Callers still provide
valid storage and truthful extents. This does not broaden an earlier solver's
separate disjoint-storage contract. Output acceleration has no new cap and can
exceed the separate inverse query's acceleration-admission box.

`pfd_subtract`, `pfd_words` and the reused pose/profile, basis, matrix-only solve
and publication helpers remain source-visible with trusted preconditions. Their
forgeable PODs attest no model provenance. Existing compiler lowering does not
enumerate every authored scalar-quantity operator token: the typed subtraction
is checked with independent literal differences in all nine lanes and the
independent SDK equation corpus. The actual emitted focused mutation scope is
85/85 killed, not a claim of complete quantity-token mutation coverage.

[experiment.json](experiment.json) and [cases.f64](cases.f64) froze all 36
contexts and unchanged PMS scalar budgets before the first new product run:

- Fifteen retained original SDK `M*a+bias` force contexts recover their original
  prescribed acceleration; both nonzero rates and both finger coordinates occur.
- Three zero-applied, zero-rate contexts compare gravity-driven acceleration
  with independent partial-pivot Gaussian solutions. Gravity is not disabled.
- Eighteen signed single-coordinate applied-cap contexts distinguish applied
  admission from net-force refusal, with no hidden clipping.

The oracle imports only the already qualified independent tooling Gaussian,
matvec and scalar comparison helpers. It runs new right-hand sides and no old
PMS/PID numerical gates or SDK calls. Original SDK M/bias and force/case artifacts
are hash-bound. Expected inputs, all reference coefficients and every decoded
actual word must be finite before any error maximum; six new decoder poisons
check nonfinite final fields, invalid status and wrong extents.

The first debug, release and fail-fast ASan/UBSan attempt passed with 4,028
assertions per profile. Each produces 33 admitted acceleration records and
three exact net-range refusals. All 324 output words, including preserved finite
poison on refusal, and all statuses agree byte for byte across the three modes.
The 297 admitted acceleration coefficients differ from independent Gaussian by
at most 7.276e-12; the original 135 prescribed coefficients by 7.627e-15. The
retained-SDK equation residual is at most 2.110e-11. The frozen bounds are
`2e-10 + 2e-12*abs(reference)` for each acceleration coefficient and
`2e-9 + 2e-12*abs(net)` for each normalized force residual. Native checks include
135 IEEE nonfinite input variants, count extremes, both admission priorities,
36 coordinate/rate endpoints, 18 applied endpoints, four overlap cases,
raw/typed identity, canaries, spare output and input immutability. Four new
source refusals retain unit and result-role distinctions.

[qualification.json](qualification.json) records that first native proof;
[checks-qualification.json](checks-qualification.json) binds the eight authored
Coax check groups/125 cases, focused zero-survivor scopes and runnable example.
The qualified numerical target is Darwin arm64 F64 scalar libm without fast
math or contraction. Other target determinism and compiler R15 remain unclaimed.
This query supplies no passive/damping/contact forces, actuator mapping,
integration, simulator step, full `mj_forward.qacc` parity, controller rollout,
compiler `#bounded` proof or hardware control.

```shell
COAX=coax/bin/coax psionic/tests/robotics/panda_forward_dynamics_check.sh /tmp/panda-forward-new
coax/bin/coax run psionic/examples/panda-forward-dynamics.coax
coax/bin/coax run psionic/examples/panda-forward-dynamics.coax --release
```
