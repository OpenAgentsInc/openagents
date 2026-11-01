# One fixed-grip pose update

This is an additive position-plus-orientation query for the source-bound Panda
arm. It returns **one** geometric damped least-squares candidate. A successful
status means admission and arithmetic succeeded; it does not mean a target was
reached or a command is permitted. The existing position-only solver is unchanged.

`ppo_step` accepts seven typed radians and one `PpoTarget`: a world/metre `Point3`
and a raw Hamilton wxyz world-from-grip attitude. The attitude array is an explicit
boundary, not a generic compiler-proven transform. The checked raw adapter accepts
7/3/4 scalar spans. Ordinary refusals preserve the complete output.

Both attitudes must have finite components of magnitude at most 2 and squared
norm within `2e-12` of one. The operator normalizes and canonicalizes each before
forming the WORLD-left relative attitude `target * conjugate(current)`. The
principal SO(3) logarithm uses a scaled vector norm and a 24-term quarter-angle
atan series. An exact half-turn chooses the first nonzero positive axis. Across
the principal cut, physical rotations and magnitudes are compared; a continuous
signed vector is not claimed. Quaternion sign changes produce exact output bytes.

The fixed task weight is `0.25 m/rad`, damping `0.05 m`, and global largest-joint
step cap `0.1 rad`. Explicit one-radian coordinate bases make all six residual
and Jacobian rows metre coefficients before solving. The angular rows are the
WORLD geometric **rate** Jacobian, not the exact differential of a finite log
residual. Source joint limits are applied after the global cap and reported.
All fresh pose/error fields are published together.

`experiment.json` and the original `pose-cases.f64` remain preserved. The
pre-comparison addendum moves only four signed boundary seeds 0.01 rad inward;
`pose-cases-v2.f64` is the executed 54-row corpus. Targets, other rows, rotation
corpus and budgets are unchanged. The original SDK poses and cached controller
SITE facts are reused; there are no new SDK calls. The independent matrix oracle
supplies 24 genuinely new target poses, rather than using owned FK as its target
generator.

The rotation corpus has 4,200 cases: 3,192 outside the principal cut band and
1,008 inside, including 112 literal canonical-axis ties. The frozen budgets are
`2e-12 rad` for the standalone error and `2e-12` per reconstructed rotation entry;
pose coordinates/diagnostics use `2e-10` in the named coordinate units, with zero
relative tolerance. Status, projection counts and cross-profile output bytes
have exact criteria.

The new source qualification records 15 focused groups and three root-only
mutation runs, 132/65/123 actual emitted sites with zero survivors. These counts
are not full quantity-operator token coverage: current quantity lowering does
not preserve every authored token. The first rotation run's eight survivors and
its changed-check/math closure are retained in the receipt. Four actual static
unit/frame refusals and the example's independently prewritten output are also
recorded. Existing FK/Jacobian/physics gates are carried, not repeated.

Low-level source-visible helpers are trusted, unsealed numerical primitives.
The evidence does not qualify a complete pose solver, finite-log optimization
Jacobian, nullspace policy, RRT, trajectory, collision handling, motor command,
compiler `#bounded` function, R14, or hardware behavior.

Native qualification keeps two debug contexts distinct: the saved original run
wrote and qualified all 4,200/54 numerical records before its independent shell
fixture failed; the changed guard-only continuation passes 37,246 assertions.
First full release and fail-fast ASan/UBSan profiles pass 231,306 assertions each.
Numerical output files are byte-identical across debug/release/sanitized modes.
Maximum vector/angle errors are 8.88e-16/1.11e-15 rad; maximum cut-band rotation
entry error is 1.06e-15. Pose angle/position/rotation maxima are 2.23e-15 rad,
1.12e-15 m and 1.45e-15 per entry. Reader correction recognizes the actual signed
row families without pairing the 24 independent matrix rows. Initial tooling
failures remain recorded; no product, corpus, threshold or expected value changed.
