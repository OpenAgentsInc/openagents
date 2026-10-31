# Fixed Panda fluid and passive force

This query owns inertia-box fluid resistance and complete passive force for
the pinned model's nine robot DOFs. Its three fields expose fluid, joint
viscous damping and their sum. Source metadata proves absent springs, gravity
compensation, tendons, flex, plugins and callbacks. Frictionloss belongs to the
constraint calculation.

`pfl_forward` accepts seven radian arm coordinates, two metre finger
coordinates, matching angular/linear rates and exactly one `PflOutput`.
Shapes precede coordinate admission (±100 rad / ±1 m), then rate admission
(±20 rad/s / ±2 m/s), then atomic publication. The raw API explicitly attaches
those units. Hinge forces are joule/radian; slide forces are newton. Truthful
storage is required. Input/output overlap is supported because publication
follows all reads. No force clipping or later solver-cap guarantee is provided.

The 16 owned bodies are SDK IDs 2–17. Their 66 geoms select inertia-box
fallback, including the positive-mass hand with no geometry. Computation uses
the COM and principal-inertial frame, preserving original inertias including
artificial fingertip boxes. Fixed base/link0 project zero; mass-zero EEF is
skipped. The original experiment and failed oracle preflight are retained:
its initial omission guard assumed every other body had another root. The
versioned support addendum instead proves each omitted body's complete
ancestor chain has no robot DOF 0..8, including fixed mounts 18–22 at root 2.

Density is 1.2 kg/m³, viscosity is 2e−5 kg/(m·s), and wind is zero. Explicit
division by one radian converts angular rate to angle-free frequency; physical
COM moment is divided by one radian before generalized-force projection.
Public queries construct source poses/profile. Low-level coefficient helpers
remain trusted, source-visible and unsealed. `PflBox.floored` reports strict
triangle-floor clipping in that helper; it adds no public force field or
general inertial-model admission.

The frozen corpus has 42 contexts. The 18 retained SDK rows contain seven
unique q/v pairs, only two moving; 24 new owned matrix cases add coverage.
This proof makes no SDK calls, steps, rendering or neural inference. The
independent oracle uses homogeneous-matrix product-rule tangents,
principal-frame scalar drag and virtual work. Its table was frozen before
Coax comparison.

- 17 new Coax groups / 74 cases; emitted root mutations 130/130 for math,
  12/12 for body and 53/53 for the public boundary. All have zero survivors
  and no equivalent waivers. Current scalar Quantity source-token mutation
  coverage is incomplete; these scores describe emitted sites.
- 42 × 27 force coefficients and 3,752 assertions per debug, release and
  fail-fast ASan+UBSan profile pass. Complete owned streams are byte-identical.
  SDK comparisons are numerical; raw signed-zero byte identity is unclaimed.
- Original force budgets are 5e−10 absolute / 0 relative for each unit. SDK
  fluid hinge error is 1.69e−14 J/rad and slide error is 5.00e−15 N; independent
  matrix errors are 1.16e−12 J/rad and 4.00e−14 N. Full passive passes too.
- Original power budget is 5e−9 W; maximum virtual-work error is 4.95e−12 W.
  Fluid and complete passive power are nonpositive at zero wind.
- 90 IEEE poisons, 36 inclusive boundaries, four overlaps, refusal atomicity,
  canaries/spares/input snapshots, four source unit/result refusals, exact
  typed/raw ABI and 16 body metadata rows pass.
- A reader-only followup rejects late actual/expected NaN/±Inf and wrong
  extents. Initial tooling failures remain in qualification history; passed
  native modes were carried rather than rerun.

The numerical profile is Darwin arm64 F64/libm, FPCR0, with fast math and
contraction disabled. Leak detection is unsupported on this Darwin sanitizer
and disabled. Cross-target/R15, arbitrary models/wind/ellipsoid import,
compiler bounded proof, integration, contact/constraints and hardware remain
outside this component.

The opt-in offline gate requires a current compiler supporting the
quantity/frame source and that declared host profile. It consumes frozen
references and never regenerates the passed oracle:

```shell
COAX=/path/to/current/coax psionic/tests/robotics/panda_fluid_check.sh /tmp/panda-fluid-proof-new
```

Use a fresh output directory; receipts preserve failures. `qualification.json`,
`checks-qualification.json`, `oracle-qualification.json`, the support addendum
and guard/example receipts record exact identities. The runnable example is
`psionic/examples/panda-fluid.coax`, marker `pfl-query`.

The historical proof binds its actual a429 compiler SHA to FP580. The driver
reports that fingerprint only for that exact known binary; unknown compiler
fingerprints remain null. A metadata-only followup records this correction.
