# Inverse physics qualification

Product code is an explicit scalar-program unroll of chapter 109's exact F64
RK4 field, shifts and left-associated weighted sums on chapter 104's existing
reverse tape. Only spring stiffness and damping occupy parameter slots. This
is not a surrogate and does not add general chapter 107 function transforms.

```shell
coax/bin/coax test psionic/lib/physics/inverse-evaluate.coax
coax/bin/coax mutate psionic/lib/physics/inverse-data.coax --root-only
coax/bin/coax mutate psionic/lib/physics/inverse-physics.coax --root-only
coax/bin/coax mutate psionic/lib/physics/inverse-fit.coax --root-only
coax/bin/coax mutate psionic/lib/physics/inverse-evaluate.coax --root-only
psionic/tests/physics/check-inverse.sh /tmp/inverse-native
coax/bin/coax run psionic/examples/recover-the-physics.coax
coax/bin/coax run psionic/examples/recover-the-physics.coax --release
```

The wrapper is for reproduction. Initial native qualification ran debug,
release and ASan+UBSan separately, reusing passing modes rather than rerunning
the wrapper over them. Both example transcripts match a golden generated from
the independent analytic-sensitivity optimizer and forecasts.

`fixtures/physics/inverse/experiment.json` freezes the six noise/sampling cases,
seeds, initial guesses, rates, iteration counts, parameter bounds, heldout
conditions, objective and numerical budgets before fitting. Each normal fit
uses 255 projected-gradient updates plus one final forward objective: 256 forward
and 255 reverse passes. The 16×16 uniform grid uses 256 forward objectives and no
reverse passes. This matches forward-loss count, not total arithmetic cost;
metadata records completed passes. Grid endpoints divide the integer coordinate
first, so rounding cannot produce a value just above the admitted upper bound.

Fifteen authored checks kill 354/354 changed-root mutants with zero survivors.
They cover the original transitions, loss averaging and sampling times, tape
and physical admission, clipping, atomic fit rollback after a late failure,
work receipts, grid ties, malformed arithmetic and equilibrium ambiguity.
Compile-time checks use tiny graphs and test the maximum iteration/grid
metadata directly. Full 255-update/16×16 workloads run in the native harness;
this avoids the compiler's aggregate-traversal ceiling without changing it.

Each native mode passes 9,326 assertions including storage/serialization checks:

- 2,520 state pairs from 35 variant/parameter trajectories match chapter 109's
original `os_step` bit for bit, including both parameter-box endpoints.

- 35 reverse-gradient cases cover seven observation variants and five parameter
points; 252 central differences sweep six step sizes at interior points.

- 666 nonfinite observation/force/initial-state cases and six nonfinite parameter
cases refuse before a usable graph/gradient is published.

- Seven matched-budget fit/grid experiments and their 384-state heldout
forecasts complete, including the deliberately unidentifiable equilibrium.

`inverse_reference.py` is an independent stdlib numerical oracle (Firm reason2).
It differentiates `I+hA+h²A²/2+h³A³/6+h⁴A⁴/24` by propagating
`d(Az)=dA*z+A*dz`, rather than copying RK stage code or calling Coax objectives.
It rebuilds all 1,554 data values, 35 analytic gradients, projected descent,
uniform-grid selection and heldout forecasts. Maximum absolute differences
are 4.44e-16 for data, 2.78e-16 for gradients, 5.56e-16 for fitted parameters and
1.83e-14 for heldout metric sums/peaks. No external AD/numerical package is used.

The FD sweep is a separate consistency check of the Coax objective and its
reverse gradient; it is not mislabeled as an independent recovery proof. Its
worst absolute errors across the interior cases are:

| Step | Worst gradient error |
| --- | ---: |
|1e-2|4.34414e-6|
|1e-3|4.34408e-8|
|1e-4|4.34833e-10|
|1e-5|3.99662e-12|
|1e-6|3.32057e-11|
|1e-7|1.97906e-10|

After those numerical passes, independent review identified missing closed
transcript-count assertions. Current `inverse_reference.py` now checks exact
35 gradient/7 fit records, all mode/point combinations, all 252 FD pairs and the
expected node/observation counts before any zip. The new
`inverse_coverage.py` qualifies those guards, rejects five truncated/duplicate
transcripts and independently checks all 5,040 state words against the matrix
polynomial (maximum 5.56e-16). It ran against each existing native transcript.
Its records prove the old/current numeric-function ASTs are identical and pin
the earlier numerical reference, original oracle and payload hashes. Those
passed numerical/model/native/mutation gates were not repeated. The combined
current reproduction wrapper runs both numerical and coverage oracles; use a
fresh output directory when an oracle source identity changes.

Debug/release/sanitized datasets, state words, gradients/FDs and fit/forecast
records are bit identical. Detailed proof and all seven outcomes are retained
in `fixtures/physics/inverse/qualification.json`.

The one fixed noise stream describes these cases; it is not a statistical
noise-robustness study. Diverse excited trajectories recover the known
synthetic coefficients, but this is not a global identifiability theorem.
A constant-force equilibrium at q=.5,u=1.2,k=2.4 is identical for damping .2
and .9. Starting there at .9 leaves it unchanged with zero training loss, yet
its heldout moving trajectories have nonzero error. No real sensor, unknown
mass/forcing/initial condition, articulated/contact dynamics, Panda replacement,
compiler boundedness, typed-clock or hardware claim follows.
