# A fixed-grip whole pose solve

This additive query repeats the already qualified geometric pose update. It
leaves all one-step `Ppo` and position-only `Pik` sources and identities intact.
`PpsOutput` is a new nominal result; it cannot implicitly substitute for the
one-step result even though their 152-byte coefficient layouts match.

`pps_solve` accepts seven typed radians, one `PpoTarget`, a caller update budget
in 0..96, and exactly one result slot. The raw adapter accepts 7/3/4 scalar spans.
Admission order is shapes, update budget, source joint/XYZ domain, then attitude
components and unit norm. Raw adapters may stage scalar copies before delegated
numeric admission. No non-ok status publishes any caller output.

The query computes fresh position and principal-orientation errors before doing
an update. Success requires **both** position <= 1e-6 metre **and** orientation
<= 1e-6 radian. A matched goal can succeed with zero updates even with a zero
budget. Exhaustion returns the new typed `not_converged` status, preserving the
complete old result. It does not silently return an unfinished candidate.

The source-visible trusted `pps_walk` stages the last candidate, status and
cumulative update/projection counts, including on exhaustion. That POD is **not
an admission certificate**. Only the checked exported wrappers publish caller
storage, and only on `ok`. `pps_walk` requires an already admitted seed/target
and a budget in 0..96; it is not a total admission API for arbitrary raw inputs.
Low-level helpers remain unsealed.

Task weight 0.25 m/rad, damping 0.05 m, global 0.1-rad step cap, source joint limits
and raw Hamilton WORLD-from-grip conventions are carried unchanged from `Ppo`.
The angular rows are the geometric rate Jacobian, not the exact differential of
a finite logarithmic residual. No global reachability, collision handling, RRT,
nullspace policy, dynamics, motor-command permission, compiler `#bounded`, R14
or hardware behavior is claimed.

The new independent experiment uses newly frozen target/seed geometries and
whole trajectories. It does not repeat the older 54 one-step or 4,200 rotation
corpora. Cached source geometry and the carried kinematic operators are reused;
no SDK, renderer, neural model, or old arithmetic gate is run.

The independent new profile has fourteen physical geometries, twelve converged
and two finite far targets that exhaust 96 updates. It freezes 68 base queries,
eight AND-policy guards and 36 predeclared convergence-boundary queries: 112
configured query records, 54 successes and 58 atomic refusals. The independent
matrix/tangent and pivoted-Gaussian recipe computes 279 unique update states once.
The active source-limit context converges in 13 updates with 38 cumulative
projected coordinates. All six comparison groups use frozen absolute 2e-10,
relative 0 budgets; original one-step error budgets remain unchanged.

Nine new source groups, four static unit/frame/nominal-result refusals, and 111/111
actual emitted mutation sites qualify the new root. The first 112-site run's six
survivors and exact changed-observation/fail-closed closure remain recorded.
Quantity lowering does not preserve every authored token, so these counts are
not full quantity-token mutation coverage. No equivalence waiver was used.

First debug/release/fail-fast ASan+UBSan profiles each pass 23,665 assertions.
The 405 native events are 112 configured actual Pps queries, 279 EXTERNAL Ppo
one-step transitions on new paths, and 14 zero-budget actual Pps calls from the
external replay terminal seeds. Twelve terminal calls admit zero work and two
refuse; these are not instrumented internal whole-solver states. Public successes
carry exact cumulative counters; no public refusal publishes an unfinished
candidate. Complete numerical files are byte-identical across all three modes.
There are 345 success payloads and 60 unavailable-result markers.

Maximum joint/position/rotation/orientation-error discrepancies are 1.994e-12 rad,
6.59e-14 m, 8.18e-13 per matrix entry and 5.48e-13 rad. The first proof also qualifies
112 sign checks, 12 aliases, 14 joint bounds, 70 IEEE cases, 4 shell cases and 14
poisoned-reader guards. These are source-bound scalar-libm/F64 observations,
not a reproducibility qualification on other machines or hardware evidence. The independently
prewritten moderate-B example passes debug/release and its own display check;
it prints residuals in nanometres/nanoradians instead of rounding them to zero.
