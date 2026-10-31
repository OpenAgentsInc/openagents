# Fixed robot9 mass metric and zero-rate gravity

`panda-inertia*.coax` owns the generalized mass matrix and gravitational
potential gradient of the pinned sixteen-body robot: seven radian hinges and
two metre slides. It calls the already qualified finger FK, computes each
world COM and inertial orientation, and sums body kinetic metrics. It adds
the nine exact source armatures once. This is a transparent geometric COM
Jacobian sum, not a CRBA/RNEA implementation or a forward/contact solver.

`experiment.json` froze the source identities, all acceptance budgets, seed,
case groups, primary difference step and diagnostic sweep before comparison.
The source model is the original demo0 XML bound by the earlier acquisition,
with inherited compiled facts independently recorded in
`../panda-fingers/sdk-model.json`. The original XML SHA256 is
`13e900d1086ecee8e16df2876104e6b6c95e06c46a14b5860ccebb16ee1af7e5`;
the retained rewritten scene hash is
`95a4049c1454ffd77b2185f0a21966832c5be55af67f64c2b110cc32e617d02f`.
These simplified simulation parameters are not manufacturer measurements.

The gripper's anisotropic principal inertia has its own positive-X quarter
turn, distinct from its BODY orientation. Each finger and fixed tip contributes
mass at its own moving COM. The arm block therefore depends on slide position.
The tip's inertial quaternion differs from its BODY quaternion but its source
inertia is isotropic; a separate authored anisotropic fixture tests orientation
arithmetic that this source value cannot observe. Zero-mass bodies have no
kinetic/gravity contribution; their compiled COM placeholders, including eef,
are still retained in the exactly bound model table.

`pm_forward` takes seven `Quantity(Radian,f64)`, two `Quantity(Metre,f64)` and
one writable `PmOutput`. `pm_forward_raw` explicitly attaches the same coordinate
labels. Both return `PfkStatus` (zero is success), preserve all output bytes
on refusal, and publish a complete local result once. Output shape is checked
before delegating q/slide shapes and numeric admission to `pff_forward`:
arm first then slides, each index checking finite number before range.
Numerical bounds are inclusive ±100rad and ±1m; these are not source soft-limit
or command permits. Native callers supply valid backing storage and truthful
extents. No caller-substituted model or snapshot is admitted by the public C
ABI. Source-visible `pm_sum_owned` requires trusted source-bound valid poses
and profile constants; it is an arithmetic helper, not a provenance certificate
or generic model-admission API. Its input/output POD values are not sealed.

The four row-major mass blocks are HH49, HS14, SH14 and SS4. Their dimensions
are respectively kg·m²/rad², kg·m/rad, kg·m/rad and kg. Separate hinge gravity7
and slide gravity2 have energy/radian and force dimensions. The source's
conventional inertia coefficients cross an explicit one-radian normalization
boundary; no implicit angle-erasure conversion is introduced. The independent
unit check demonstrates that rates turn every mass block into energy and
coordinate increments turn gravity into work. Quantity wrappers erase to
ninety F64 ABI words; they do not make the mixed-coordinate matrix homogeneous.

The pinned SDK scene has21 generalized velocities. The reference selects only
the named robot DOFs0..8 into M9 and bias9. Book/mug/environment are different
subtrees. All five SDK captures have every qvel zero, so the source generalized
bias equals this gravity target for the bound model. This result does not
supply velocity-dependent bias, damping/friction/fluid force, actuator force,
contact, constraints, integration, control, a generic importer, hardware
fidelity, differentiability, R15 or compiler `#bounded`.

`qualification.json` records:

- 9 new Coax check groups /523 cases and338 focused root mutants killed:
  profile38, math244, wrapper56; zero survivors and zero waivers.
- 57 configurations and11,374 ABI/refusal assertions in EACH debug, release
  and fail-fast ASan/UBSan build. All5,130 output words match byte for byte
  across the named Darwin-arm64 scalar-libm profile.
- All405 retained SDK mass coefficients: maximum error4.4408921e-15 under
  the frozen absolute2e-11 bound; all45 zero-rate bias coefficients:
  maximum7.1054274e-15 under2e-10. Relative tolerance is zero.
- Independent full-angle homogeneous-matrix COM/left-SO3 differences for38
  configurations, including all five SDK cases. The preselected h=1e-5 gives
  M error2.30875e-10 under3e-8 and potential-gradient error2.92950e-9 under2e-7.
  The full1e-3/1e-4/1e-5/1e-6/1e-7 sweep is retained. The largest step misses
  both primary budgets; the smallest misses the primary gravity budget.
  Neither replaces the preselected primary step. These are smooth geometric
  derivatives, not derivatives of float rounding or refusal.
- All1,632 inertial/profile bytes exactly match an independently serialized
  source table; normalized inertial frames and independently reconstructed
  ancestor masks match the compiled model. Four source unit refusals and a
  separate physical-energy/work unit check pass.

“Interior” for the38 difference configurations means interior to numerical
admission, not the physical source joint ranges. The zero pose has joint4=0
outside its source soft range, and the five slide cases include source-range
endpoints. The eighteen numerical admission endpoints are finite-output cases;
the product API is not perturbed outside its admitted range.

`initial-harness-diagnostic.json` preserves the first failed sanitized attempt:
the NEW native refusal harness formed the intermediate pointer `s+j-7`, which
UBSan correctly rejected even though its final intended offset was valid.
The corrected `s+(j-7)` harness was qualified in all three modes. The exact
earlier independent FD table, source refusals and emitted Coax C/header facts
were reused; numerical references were not recomputed. The guard followup
explicitly binds the independently loaded arm-profile JSON to the already
bound finger-profile SHA and inventories it; the earlier executed driver
identity remains historical. Development failures and unchanged budgets are
recorded separately, not erased or reclassified as equivalent mutants.

Run only the new source roots and native/reference query:

```shell
COAX=/path/to/qualified-coax
"$COAX" test psionic/lib/robotics/panda-inertia.coax --only "Panda inertia"
"$COAX" test psionic/tests/robotics/panda_inertia_unit_checks.coax --only "Panda inertia typed blocks"
"$COAX" mutate psionic/lib/robotics/panda-inertia-profile.coax --library --root-only
"$COAX" mutate psionic/lib/robotics/panda-inertia-math.coax --library --root-only
"$COAX" mutate psionic/lib/robotics/panda-inertia.coax --library --root-only
COAX="$COAX" bash psionic/tests/robotics/panda_inertia_check.sh /tmp/new-panda-inertia-proof
"$COAX" run psionic/examples/panda-inertia.coax
```

The product needs no SDK, Python or third-party runtime. Python is independent
development tooling. The five earlier SDK mass/bias captures are reused by
exact public hashes; no SDK, renderer, neural inference, old FK/finger native
gate or original episode was run for this slice.
