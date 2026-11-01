# 167. Command a robot

This chapter builds position-only inverse kinematics, principal orientation
error, a capped whole-pose solve, linearized posture projection, rest-to-rest
reference timing, isolated PD gain/response analysis, seven-joint PD with gravity
compensation, and short experiments on our owned nonlinear Panda dynamics.
IK returns source-limited coordinates; posture returns a delta, timing returns
reference samples, and PD returns explicit force terms. A 250-step prefix and
four new hold/track/pulse/brake cases compare owned feedback with MuJoCo.
Numerical agreement passes; the original baseline misses its nominal final-error
and brake-recovery goals. A separately frozen nine-profile owned-plant gain
study finds no feasible family and selects no winner. A new common damping law
then qualifies all four original owned cases using exact command carry and one
new brake trajectory. A separate prospective validation at two frozen C/D
poses now meets all eight fixed goal sets with no-contact SDK/own parity.
General stability and the broader planning tools remain work.

The separate [chapter 170](ch170-close-the-simulated-control-loop.md) already
runs operational-space control against MuJoCo. Meeting broader tracking and
recovery criteria, finite-step nullspace/IK integration, collision-free planning
and compiler-enforced bounded control remain separate work. A timing reference
does not establish those physical behaviors.

## A source-bound position IK query

We can now ask the owned Panda kinematics for seven radian coordinates that
place its grip SITE near a requested world position. This is the first runnable
slice of this chapter. It uses the exact twelve-body/seven-hinge source profile
and the already qualified grip Jacobian. The fingers do not change this fixed
SITE position; this query has no finger coordinate or orientation objective.

`pik_solve` takes seven typed radian seed coordinates, one
`Point3(PfkWorld, Metre, f64)` target, an integer update budget from zero through
96, and one caller-owned output slot. Seeds must already lie in the seven
inclusive joint ranges captured from the pinned compiled source model. The
public target box is ten metres per world coordinate. These are source and
numerical admissions, not collision or actuator permission.

The [implementation](../lib/robotics/panda-ik.coax) solves a three-dimensional
damped least-squares problem.
We normalize each Jacobian column by exactly one radian. Its coefficient then
has metres, so the damping coefficient is **0.05 metres** in this coordinate
basis. With `e = target - position`, the coefficient equation is:

```jai
A  = J * transpose(J) + (0.05 * 0.05) * identity3;
dq = transpose(J) * inverse(A) * e;
```

The small positive diagonal regularizes singular geometric configurations. The
implementation reuses the existing checked three-by-three Cholesky inverse
inside the fixed stride-seven controller matrix storage. Each raw `dq`
coefficient restores the same one-radian basis. We cap the largest absolute
joint increment at 0.1 radians by scaling the whole vector, then project each
new coordinate into its pinned source range. Projection can prevent progress;
there is no global reachability guarantee or adaptive line search.

The update budget counts actual proposed updates. An initially solved seed can
succeed with zero updates. A successful solution has Euclidean position error
at most **one micrometre**. Exhaustion returns `not_converged` and leaves the
entire caller output unchanged. It is a statement about this seed and fixed
iteration profile, not a proof that the target is unreachable.

`pik_step` exposes a separately admitted one-update candidate, its freshly
computed grip position, residual and number of projected coordinates. It can
publish useful diagnostic progress before convergence. The solver accumulates
those counts and publishes a result only after the residual criterion passes.
All non-OK paths preserve output and spare storage. The raw adapters name their
radian and world-metre coefficient boundaries explicitly.

The following example starts joint1 at its source upper range. Its target is
one centimetre farther along world -X than that configuration's independently
computed grip position. The independent reference predicts eleven updates,
eleven projected coordinates and a final residual of approximately
`4.6571171e-7` metres. Display values round to micrometres or microradians; the
library retains F64 values.

<!-- source: psionic/examples/panda-ik.coax pik-query -->
```jai
 // Typed radians and one explicit world/metre target. Joint1 starts exactly
 // at its source upper limit; the target is displaced 1cm along world -X.
 seed:=pik_home();seed[0].value=2.8973;
 target:[1]Point3(PfkWorld,pfku.Metre,f64);
 target[0].x= -1.2036558811324911;target[0].y=0.11059065150635836;target[0].z=1.0110438715851884;
 solution:[1]PikOutput;
 #assert pik_solve(seed[..],target[..],11,solution[..])==PikStatus.ok;
 #assert solution[0].error_metres<=1e-6;
 print("updates: %; projected coordinates: %\n",solution[0].updates,solution[0].projected_coordinates);
 p:=solution[0].position;
 print("grip xyz (um): % % %\n",pik_example_micro(p.x),pik_example_micro(p.y),pik_example_micro(p.z));
 print("residual below 1 um\n");
 q:=solution[0].angles;
 print("candidate joints (urad): % % % % % % %\n",pik_example_micro(q[0].value),pik_example_micro(q[1].value),pik_example_micro(q[2].value),pik_example_micro(q[3].value),pik_example_micro(q[4].value),pik_example_micro(q[5].value),pik_example_micro(q[6].value));
 // Exhaustion refuses publication; a previous candidate stays intact.
 before:=solution[0];
 #assert pik_solve(seed[..],target[..],0,solution[..])==PikStatus.not_converged;
 pik_same(solution[0],before);
 print("zero-update unsolved: not_converged; output unchanged\n");
```

```shell
coax/bin/coax run psionic/examples/panda-ik.coax
```

The frozen experiment contains 49 contexts: seven near-start targets from
retained freshly forwarded SDK grip samples; 24 reachable targets generated by
an independent authored homogeneous-matrix calculation; the home target;
fourteen exact source-range endpoints; and three explicit budget/nonconvergence
contexts. A separately frozen oracle profile adds the existing eleven-update,
eleven-projection contract case, preserving the original 49-case bytes and
budgets. The SDK samples bind both the target and its matching joint tuple;
they are not cached geometry paired with a later coordinate state. No SDK,
rendering, integration or neural inference runs are needed for these checks.

The independent tooling computes analytic matrix-product tangents and solves
its own pivoted Gaussian three-by-three system. The product's quaternion/cross
Jacobian path and Cholesky inverse are not used to generate reference
trajectories. Coordinate, freshly computed position and residual comparisons
use the frozen **2e-10 absolute, zero-relative** coefficient budgets; update and
projection counts must agree exactly. The native record retains every
trajectory, all refusals and all outcomes, including exhaustion.

The new authority roots have **102/102** and **152/152** actually emitted
mutation sites killed, with zero survivors. Twelve focused source groups cover
38 cases. Four meaningful compile refusals distinguish metre coordinates from
radians, HalfMetre targets from Metre targets, nominal world frames, and radian
outputs from lengths. The example matches its independently prewritten golden
in debug and release. The independent reference and native proof cover 50 contexts: **47 converge
and three exhaust their declared budgets**. Debug, release and fail-fast
ASan/UBSan each pass **15,507 assertions**. All **363 events**, including **228
trajectory updates** and **35 standalone steps**, agree on exact statuses and
update/projection counters. Maximum errors are `1.22e-14` radians, `4.22e-15`
metres in position and `9.00e-16` metres in residual, inside the unchanged
`2e-10` budgets. The three complete owned output streams are byte-identical.
Fourteen source-limit and six target-box boundaries, 50 IEEE poisons, eight
overlap cases, complete input/output snapshots and ten late actual/expected
reader poisons pass. The record is
[`native-qualification.json`](../fixtures/robotics/panda-ik/native-qualification.json).

The numerical qualification uses the named Darwin arm64 scalar-libm F64
profile, with contraction and fast math disabled. It carries the existing FK
and Jacobian evidence without repeating their passing suites. Cross-target
bit identity remains a separate determinism requirement. This chapter still
needs orientation and nullspace IK, PD and gravity-control rollout, collision
queries, RRT-Connect, trajectory timing and compiler-enforced bounded control.
This position candidate is useful before any of those claims are made.

## One update that includes orientation

The position-only query cannot choose a grip attitude. Add an orientation error
without changing that solver: `ppo_rotation_raw` computes a principal SO(3)
logarithm, and `ppo_step` uses it to propose exactly one geometric DLS update.
An `ok` result says that this candidate was admitted and computed. It makes no
convergence or command-permission claim.

The target position is a `Point3(PfkWorld, Metre, f64)`. The second target field
is a raw Hamilton **wxyz world-from-grip** quaternion, so attitude conventions
remain a caller boundary rather than a compiler-proven transform chain. The
relative rotation is WORLD-left: target times conjugate(current), corresponding
to `R_target * transpose(R_current)`.

The checked boundary admits four finite quaternion coefficients of magnitude
at most two, with squared norm within **2e-12** of one, before normalization.
Target position components must be within **10 metres** and the seven seed
angles within the pinned source joint ranges. Shape errors precede numerical
admission; ordinary refusals preserve complete caller output.

The principal logarithm handles a case the older cross-axis error misses: that
error is proportional to
sine(theta), so it vanishes at a half-turn. This helper retains the principal
angle through pi. It normalizes and canonicalizes both inputs, uses a scaled
vector norm for very small angles, and evaluates a fixed quarter-angle atan
series. Exact pi chooses the first nonzero positive axis; either sign of the
same quaternion produces identical complete output bytes. The principal vector
has a cut at pi, so the corpus tests physical rotation and magnitude there,
rather than claiming continuity across that cut.
The comparison band was frozen as **|relative.w| <= 2e-14** before any
product comparison. Outside it, all logarithm components are compared.

For a unit relative quaternion, `s = norm(xyz)` and `x = s/(1+w)` give the
angle `4 atan(x)`. A reduction keeps the series argument within sqrt(2)-1;
24 fixed Horner terms implement the [alternating arctangent series](https://dlmf.nist.gov/4.24.E3).
The truncation bound precedes floating rounding; the measured 2e-12-radian
budget remains the qualification. The canonical sign convention follows the
[positive-w, then first-nonzero-axis rule](https://docs.scipy.org/doc/scipy/reference/generated/scipy.spatial.transform.Rotation.as_quat.html);
SciPy is a documentation reference, with no product or tooling dependency.

Give the six task rows compatible dimensions before solving. The angular task
weight is **0.25 metre/radian** and the damping **0.05 metre**. An explicit
one-radian basis turns position Jacobian rows and weighted angular Jacobian rows
into metre coefficients. Solve `(J J^T + lambda^2 I) y = e`, then `dq = J^T y`,
restore typed radians, cap the largest coordinate change at **0.1 radian**, and
project onto the pinned source joint limits. The angular matrix is the WORLD
geometric **rate** Jacobian, not the exact derivative of the finite logarithmic
residual. Recompute the candidate's pose and all three residual diagnostics
before atomic publication.

The [example](../examples/panda-pose-ik.coax) uses independent matrix/Gaussian
corpus row 28; its expected output
was written before execution. It also shows quaternion-sign equality, a true
half-turn, and refusal of a zero quaternion without overwriting the candidate.

<!-- source: psionic/examples/panda-pose-ik.coax ppo-step -->
```jai
 // The target is an independently frozen matrix pose. Angles are radians;
 // position is a world/metre Point3. Hamilton wxyz stays an explicit raw boundary.
 words:=[14]f64.[-0.26070335378293363,0.4197829540218629,-0.17224976514367665,-2.7864034195561564,-0.17538875316188446,2.740753613038838,0.7788488675752366,-0.5250789228078325,-0.11265799954996873,0.8947516665468195,-0.16060863355553037,0.7705457766950156,0.5952945116922437,-0.16151940191194944];
 seed:[7]pfku.Quantity(pfku.Radian,f64);for i:0..<7 {seed[i].value=words[i];}
 target:[1]PpoTarget;target[0].position.x=words[7];target[0].position.y=words[8];target[0].position.z=words[9];for i:0..<4 {target[0].world_from_grip_wxyz[i]=words[10+i];}
 candidate:[1]PpoOutput;
 #assert ppo_step(seed[..],target[..],candidate[..])==PpoStatus.ok;
 q:=candidate[0].angles;p:=candidate[0].position;
 print("candidate joints (urad): % % % % % % %\n",ppo_example_micro(q[0].value),ppo_example_micro(q[1].value),ppo_example_micro(q[2].value),ppo_example_micro(q[3].value),ppo_example_micro(q[4].value),ppo_example_micro(q[5].value),ppo_example_micro(q[6].value));
 print("grip xyz (um): % % %\n",ppo_example_micro(p.x),ppo_example_micro(p.y),ppo_example_micro(p.z));
 print("fresh errors: % um position; % urad orientation; % um weighted\n",ppo_example_micro(candidate[0].position_error_metres),ppo_example_micro(candidate[0].orientation_error.value),ppo_example_micro(candidate[0].weighted_error_metres));
 print("updates: %; projected coordinates: %\n",candidate[0].updates,candidate[0].projected_coordinates);
 // The same physical attitude has two quaternion representations.
 opposite:[1]PpoOutput;for i:0..<4 {target[0].world_from_grip_wxyz[i]= -target[0].world_from_grip_wxyz[i];}
 #assert ppo_step(seed[..],target[..],opposite[..])==PpoStatus.ok;ppo_same(candidate[0],opposite[0]);
 print("q/-q target: complete output identical\n");
 identity:=[4]f64.[1,0,0,0];half_turn:=[4]f64.[0,0,0,-1];rotation:[1]PpoRotation;
 #assert ppo_rotation_raw(identity[..],half_turn[..],rotation[..])==PpoStatus.ok;
 print("principal +Z half-turn (urad): %\n",ppo_example_micro(rotation[0].vector[2].value));
 before:=candidate[0];for i:0..<4 {target[0].world_from_grip_wxyz[i]=0;}
 #assert ppo_step(seed[..],target[..],candidate[..])==PpoStatus.rotation;ppo_same(candidate[0],before);
 print("zero quaternion: rotation refusal; complete candidate unchanged\n");
```

Run it with:

```shell
coax/bin/coax run psionic/examples/panda-pose-ik.coax
COAX=coax/bin/coax psionic/tests/robotics/check-panda-pose-ik-native.sh /tmp/new-panda-pose-proof
```

The new source evidence is 15 focused groups, four meaningful static unit/frame
refusals, and 132/65/123 emitted mutation sites across the rotation, solve helper
and public query roots, all killed. The count describes actual emitted sites;
quantity lowering still loses some authored operator-token provenance. The
first rotation attempt's eight survivors are retained, followed by changed
checks and an observable Horner trace that closes them without equivalence
waivers. Existing kinematic and physics gates are reused.

Before any numerical comparison, the corpus received a versioned correction:
only four signed seeds at exact source limits moved 0.01 radian inward. This
avoids making an exact projection count depend on ulp-sized matching-pose
residuals. The original files, unchanged SDK targets, other 50 rows, all 4,200
rotation cases and all budgets remain preserved. The independent oracle uses
4-by-4 matrix product-rule tangents, a separate matrix-to-quaternion conversion,
`math.atan2` and pivoted Gaussian elimination. Twenty-four new target poses
come from that independent recipe; cached fresh SITE references supply the
other target families. No new SDK or neural execution is involved.

The standalone corpus includes 3,192 ordinary and 1,008 cut-band cases, of which
112 exercise exact canonical half-turn ties. The frozen absolute budgets are
2e-12 radians for standalone principal errors, 2e-12 per reconstructed rotation
entry, and 2e-10 in the named pose coordinate/diagnostic units. Counts, statuses
and cross-profile byte criteria are exact. The original debug run completed all 4,200/54 numerical records and sign/ABI probes before an independently constructed norm-shell guard failed. Its numerical outputs were retained. A corrected guard-only debug continuation passes 37,246 assertions; first full release and fail-fast ASan/UBSan runs pass 231,306 assertions each. The complete numerical streams match byte for byte across these three profiles. Maximum standalone vector/angle errors are 8.88e-16/1.11e-15 radian, with cut-band rotation-entry error 1.06e-15; pose joint/position/rotation errors are 2.23e-15 radian, 1.12e-15 metre and 1.45e-15 per entry. Reader guards pair only actual sign-paired rows; the 24 independent matrix rows are not invented sign pairs. These tooling corrections preserve the product, expected data and numerical budgets.

These are source-bound Darwin scalar-libm/F64 query observations. They do not
establish general cross-target reproducibility. The next section adds a capped whole-pose solve. Nullspace behavior, collision
admission, RRT and compiler `#bounded` control remain separate work.

The [component record](../fixtures/robotics/panda-pose-ik/README.md) links
the unchanged targets, pre-comparison correction, independent named-grid check,
source-restore note and separate numerical/native contexts.


## Solve until both pose coordinates are close

A single update is useful for inspection, but a whole query must decide when to
stop and what an unfinished result means. Add `pps_solve` with its own status and
nominal `PpsOutput`; leave every one-step and position-only source unchanged.
The caller gives seven typed radians, one world/metre position plus raw
world-from-grip wxyz attitude, and a budget from zero through 96 updates.

Compute fresh metrics **before** moving. Succeed only when position error is at
most **1e-6 metre** AND principal orientation error is at most **1e-6 radian**.
A seed already within both tolerances can succeed with no updates, even with a
zero budget. Otherwise repeat the carried geometric DLS update with its fixed
0.25 m/rad task weight, 0.05 m damping, global 0.1-rad cap and source joint limits.
The angular matrix remains a geometric rate Jacobian; this is not an exact
finite-log Hessian method or a global convergence guarantee.

After each candidate, test the two fresh errors separately. Count actual
updates and every projected joint coordinate cumulatively. If the budget runs
out, return typed `not_converged` and leave the complete caller result intact.
`PpsOutput` has its own identity: it cannot implicitly pass as a one-step result
whose update count means something different. The source-visible `pps_walk`
POD can retain staged diagnostics on exhaustion, but it is a trusted, unsealed
helper and **not an admission certificate**. It requires an already admitted
seed/target and a budget in 0..96; it is not a total raw-input admission API.
Exported wrappers publish only `ok`.

The [runnable example](../examples/panda-pose-solve.coax) uses a new independent
moderate target that takes nine updates. Reducing the budget
to eight refuses publication. Reusing the converged seed with a zero budget
succeeds without work; reversing the target quaternion gives identical bytes.

<!-- source: psionic/examples/panda-pose-solve.coax pps-query -->
```jai
 // New independent moderate-B target. This is a kinematic query, not a command.
 x:=pps_fixture([14]f64.[-0.43,-0.17,0.52,-1.87,-0.61,1.43,-0.82,-0.3312488088297923,-0.02236227277159756,1.1949403501120703,0.3087793425683993,0.26307588076182686,-0.9008416321384713,0.15469567663807282]);solution:[1]PpsOutput;
 #assert pps_solve(x.seed[..],x.target[..],96,solution[..])==PpsStatus.ok;
 #assert solution[0].position_error_metres<=1e-6 && solution[0].orientation_error.value<=1e-6;
 print("updates: %; projected coordinates: %\n",solution[0].updates,solution[0].projected_coordinates);
 q:=solution[0].angles;p:=solution[0].position;
 print("candidate joints (urad): % % % % % % %\n",pps_example_scale(q[0].value,1e6),pps_example_scale(q[1].value,1e6),pps_example_scale(q[2].value,1e6),pps_example_scale(q[3].value,1e6),pps_example_scale(q[4].value,1e6),pps_example_scale(q[5].value,1e6),pps_example_scale(q[6].value,1e6));
 print("grip xyz (um): % % %\n",pps_example_scale(p.x,1e6),pps_example_scale(p.y,1e6),pps_example_scale(p.z,1e6));
 print("fresh errors: % nm position; % nrad orientation; % nm weighted\n",pps_example_scale(solution[0].position_error_metres,1e9),pps_example_scale(solution[0].orientation_error.value,1e9),pps_example_scale(solution[0].weighted_error_metres,1e9));
 // Exhaustion preserves an earlier complete solution; no partial candidate leaks.
 before:=solution[0];#assert pps_solve(x.seed[..],x.target[..],8,solution[..])==PpsStatus.not_converged;pps_same(solution[0],before);
 print("8 updates: not_converged; complete old result unchanged\n");
 // A seed already within BOTH tolerances needs no update, even at budget zero.
 settled:=before.angles;zero:[1]PpsOutput;
 #assert pps_solve(settled[..],x.target[..],0,zero[..])==PpsStatus.ok && zero[0].updates==0 && zero[0].projected_coordinates==0;
 print("settled seed with zero budget: ok; updates %\n",zero[0].updates);
 // Both attitude signs yield the same complete, converged result.
 opposite:[1]PpsOutput;for i:0..<4 {x.target[0].world_from_grip_wxyz[i]= -x.target[0].world_from_grip_wxyz[i];}
 #assert pps_solve(x.seed[..],x.target[..],96,opposite[..])==PpsStatus.ok;pps_same(before,opposite[0]);
 print("q/-q target: complete result identical\n");
```

```shell
coax/bin/coax run psionic/examples/panda-pose-solve.coax
COAX=coax/bin/coax psionic/tests/robotics/check-panda-pose-solve-native.sh /tmp/new-whole-pose-proof
```

The new experiment freezes fourteen physical target/seed contexts before
comparison: twelve converge and two finite far targets exhaust their budgets.
Sixty-eight base queries, eight separate AND-policy guards and 36 predeclared
success-boundary queries give **112 public queries** (54 successes, 58 refusals).
The independent matrix/tangent and pivoted-Gaussian recipe computes each new
trajectory once: **279 unique updates**. No old 54-case pose-update or 4,200-case
rotation corpus, SDK acquisition or neural execution is repeated. The active
source-limit case converges in thirteen updates with **38 cumulative coordinate
projections**. The fixed errors and counts are reported without retuning.

Nine new source groups and four meaningful typing refusals qualify the policy,
including exact inclusive threshold literals, all XYZ admission roles, the
independent nine-update endpoint and the active-limit staged prefixes. The
changed-root proof kills **111/111 actual emitted sites**. Its first attempt's
six survivors are preserved: additional observations and fail-closed staging
remove redundant/hidden flows, with no equivalence waivers. Quantity lowering
still omits some authored token provenance, so this is not full quantity-token
mutation coverage. The example has an independently prewritten output and
passes its own display check plus debug/release execution.

The native proof distinguishes actual whole-query results from an external
replay of the unchanged one-step API on these new trajectories. Public successes
expose exact cumulative counts; public failures expose no unfinished candidate.
Replay-only diagnostics of a nonconverged path are not reported as published
whole-solver output. The first debug, release and fail-fast ASan/UBSan profiles each pass **23,665 assertions**, with complete numerical files identical across modes. They observe 112 configured whole-solver queries, 279 external one-step transitions, and fourteen zero-budget whole-solver calls from the external replay terminal seeds: **405 events**. Twelve terminal calls succeed with zero updates/projections; two refuse without publishing. These are 345 success payloads and 60 unavailable-result markers, not instrumented internal whole-solver states. Maximum joint, position, rotation-entry and orientation-error discrepancies are **1.994e-12 rad**, **6.59e-14 m**, **8.18e-13** and **5.48e-13 rad**. Exact status/count/sign checks and twelve overlap cases, fourteen joint-bound checks, seventy IEEE cases, four shell cases and fourteen poisoned-reader guards also pass.

All six new whole-path comparison groups retain absolute 2e-10 budgets with zero
relative tolerance. This does not change the standalone principal-error budget.
The source-bound scalar-libm profile does not establish arbitrary cross-target
determinism. Finite-step nullspace integration, collision checks, RRT, trajectory and actuator
admission, compiler `#bounded` control and hardware behavior remain separate.

The [complete component record](../fixtures/robotics/panda-pose-solve/README.md)
links frozen inputs, independent trajectory/terminal bindings and numerical,
source, typing and example receipts.


## A genuine linearized posture nullspace

A secondary rest posture should not be confused with a new grip target. Query
a fresh seven-hinge Jacobian instead: the six task rows leave one joint-space
null direction when the normalized matrix has full row rank. This new `Pns`
query leaves every pose-solver and physics source unchanged.

Normalize dimensions before arithmetic. Multiply linear derivatives by one
radian to get metres. Multiply angular derivatives by one radian and the fixed
0.25 metre/radian weight to get metres. Divide every task row by one explicit
metre. This gives dimensionless A6x7 and dimensionless rest preference
`d=(rest-q)/one_radian`. A unit right-null direction n gives `p=n*(n^T*d)`;
restore p with the same radian basis to return typed projected radians.

The product uses signed six-by-six column cofactors, partial-pivot LU and a
scaled vector norm. Row-max scaling is only arithmetic conditioning: restore
**all six row scales** when reporting gamma, the normalized cofactor volume.
By Cauchy-Binet it is the product of the six singular values in exact
arithmetic, not the smallest singular value or a general condition
certificate. The frozen numerical profile requires gamma **strictly greater
than 2^-24**. Earliest largest-magnitude component chooses the direction sign;
exact zero direction and projected components become positive zero. The
independent comparison uses `n*n^T` and the projected vector, so arbitrary
mathematical line orientation is not mistaken for a physical difference.
Literal ties are checked separately.

Check squared-norm error `abs(dot(n,n)-1) <= 2e-12`, `max(abs(A*n)) <= 2e-10`,
and `max(abs(A*p)) <= 2e-10*max(1,norm(d))`. Physical output leakage is in
metres: the first three rows are linear displacement and the final three are
weighted angular displacement. The separate checked residual operator admits
deliberately bad finite diagnostics so those refusal branches are observable.
It validates finite/nonnegative values **per field**, then the rest-norm
domain, then the policy thresholds. Trusted owned helpers remain unsealed;
their staged PODs are not forged-input admission certificates. Public wrappers
publish only `ok`.

Both q and rest must meet source joint limits, but **q+p need not**. In
already frozen case 20, joint 2 starts at -1.7578 rad and the independently
projected delta is -0.0662561668 rad. Their sum would be -1.8240561668 rad,
below its -1.7628-rad source lower limit by about 0.06126 rad. This is
arithmetic over cached facts, not a newly applied pose. Componentwise clipping
would also destroy the null property. Return a delta only: finite-pose
preservation, joint-step policy, collision and actuation remain separate.

The example uses independent Gaussian-projector physical case 22. It shows the
line direction, projected preference, volume, this query's small weighted
leakage, exact self-rest zero and atomic refusal of an invalid rest posture.

<!-- source: psionic/examples/panda-nullspace.coax pns-query -->
```jai
 // Both source-limited vectors are radians. The result is a projected delta;
 // it is never applied here as a finite pose, trajectory or motor command.
 words:=[14]f64.[0.5916784815654972,-1.1314303950772526,1.3723101939314204,-0.7863450971120733,0.9193678441656266,2.7243916210477472,-1.0067594805156277,0.0,0.19634954084936207,0.0,-2.617993877991494,0.0,2.941592653589793,0.7853981633974483];q:[7]pfku.Quantity(pfku.Radian,f64);rest:[7]pfku.Quantity(pfku.Radian,f64);for i:0..<7 {q[i].value=words[i];rest[i].value=words[7+i];}
 out:[1]PnsOutput;#assert pns_query(q[..],rest[..],out[..])==PnsStatus.ok;
 n:=out[0].direction;d:=out[0].projected_delta;
 print("unit null direction (ppm): % % % % % % %\n",pns_example_micro(n[0].value),pns_example_micro(n[1].value),pns_example_micro(n[2].value),pns_example_micro(n[3].value),pns_example_micro(n[4].value),pns_example_micro(n[5].value),pns_example_micro(n[6].value));
 print("projected preference (urad): % % % % % % %\n",pns_example_micro(d[0].value),pns_example_micro(d[1].value),pns_example_micro(d[2].value),pns_example_micro(d[3].value),pns_example_micro(d[4].value),pns_example_micro(d[5].value),pns_example_micro(d[6].value));
 print("normalized volume (ppm): %; singular-value product\n",pns_example_micro(out[0].normalized_volume.value));
 for i:0..<6 {#assert abs(out[0].task_leak[i].value)<1e-9;}
 print("this query's weighted task leakage is below 1 nm\n");
 // Self-rest has exactly zero projected preference, with canonical positive zero.
 zero:[1]PnsOutput;#assert pns_query(q[..],q[..],zero[..])==PnsStatus.ok;for i:0..<7 {#assert f64_bits(zero[0].projected_delta[i].value)==0;}
 print("self-rest: complete projected delta is +0\n");
 before:=out[0];rest[0].value=3;#assert pns_query(q[..],rest[..],out[..])==PnsStatus.limits;pns_same(out[0],before);
 print("outside source rest limit: refusal; complete old query unchanged\n");
```

```shell
coax/bin/coax run psionic/examples/panda-nullspace.coax
```

The independent reference freezes **48 genuine queries** (16 configurations
and three rest postures each) plus **24 synthetic numerical cases**. All 48
genuine queries pass this numerical profile; none is mislabeled singular
merely because an axis is aligned. Synthetic controls supply fourteen accepted
and ten rank-refused cases, including exact restored-volume floors and one-ULP
neighbors across three first-row scaling families. Their first-row scale
factors and products of row maxima are 1, 2 and 0.5; the global maximum
coefficients are 1, 2 and 1, respectively. The all-zero arm is not a legal
public source-limit case. Exact-Fraction Gram arithmetic and an 80-digit
Decimal Gaussian projector `I-A^T*(A*A^T)^-1*A` provide an independent recipe
with **no damping** and no cofactor oracle. A damped inverse complement is not
called an exact nullspace.

Thirteen new source groups, four meaningful unit refusals and three focused
root runs kill **209/65/42 actual emitted sites**, with no equivalence
waivers. The math history retains hidden zero-residual and global-gauge
observations, followed by independently oriented cofactor and deliberately
non-null diagnostic checks. The bridge history retains seven missed
transport/endpoint observations and their closure. These counts do not
establish complete authored quantity-token provenance. The independently
prewritten example passes its own display check and debug/release output. The
first debug, release and fail-fast ASan/UBSan profiles each pass **9,760
assertions**. All 72 complete records match byte for byte across modes: 62
successful payloads and 10 unavailable rank markers. Maximum null-line
projector discrepancy is 2.082e-15; projected-angle discrepancy is 9.881e-15
rad. Maximum measured null residual is 3.919e-16, scaled posture residual
3.369e-16, and volume discrepancy 2.220e-16. Direct projector idempotence
error is 2.22e-16, trace error 4.44e-16, symmetry error 0, and
delta-versus-P*d error 1.02e-16. Exact restored-threshold bits, canonical
ties/+0, thirteen overlap cases, fifty-three residual probes and twenty-six
poisoned-reader guards pass.

Frozen comparisons use 2e-8 absolute for projected radians and the null-line
projector; squared norm has 2e-12, projector symmetry/idempotence/trace 2e-10,
and volume 2e-12+2e-10*abs(reference). Numeric rank labels, canonical literal
rules, statuses and mode bytes have exact criteria. No SDK, neural model,
renderer, old kinematic corpus or old mutation gate is run. This is a
source-bound F64 scalar-libm profile, not finite motion preservation,
dynamically consistent torque nullspace, collision/trajectory admission,
compiler `#bounded` control, R14 or hardware qualification.

The [component record](../fixtures/robotics/panda-nullspace/README.md) links
the frozen corpus, independent projector, source and native qualifications.

## Put a rest-to-rest edge on a fixed grid

A joint-space path needs a reference timing before it can be tracked. This
query assigns time to one straight source-range edge; it does not move the
plant or validate collision. Both endpoint reference velocities and
accelerations are zero. Concatenating these edges therefore **stops at each
waypoint**. Retargeting a moving state, or interpolating successive 20 Hz VLA
action chunks without stopping, requires a different boundary-value problem.

Use the quintic progress `s(u)=10u³-15u⁴+6u⁵`, with `u=t/T`. Its maximum first
derivative is **15/8=1.875**; its maximum absolute second derivative is
**10/sqrt(3)**. For each of seven joints, obtain the two duration candidates
`(1.875*abs(dq))/vlim` and `sqrt((5.773502691896258*abs(dq))/alim)` with that
fixed binary64 grouping. Take the largest of fourteen candidates. Angles,
velocity/acceleration limits and durations retain radian, radian/second,
radian/second² and second types throughout.

For a nonzero edge, let R be that estimate times 500 and choose
**N=ceil(R)+1**. Do not subtract an epsilon near integer boundaries. The
reserve tick is deliberate: this is not a fastest-grid or exact-real ceil
policy. R>999999 refuses before integer conversion, so N is at most one
million. Integer total nanoseconds are `N*2000000`; floating duration is
`N/500`, with no accumulated 0.002 additions. A numeric zero edge has N=0 only
after every input has been admitted. Its only sample is tick 0, and the target
representation wins, including signed zero. A nonzero subnormal edge still
reserves a tick even if its required estimate underflows to zero.

Sample the nearer endpoint using `v=min(k,N-k)/N` and `b=v³*(10+v*(-15+6v))`;
anchor position at q0 on the first half and q1 on the second. Analytic
derivatives use `30v²(1-v)²` and `60v(1-v)(1-2v)`, reversing the acceleration
sign on the second half. Endpoint branches copy exact angles and positive-zero
derivatives. No hidden joint clamp changes the curve. These are derivatives of
the analytic reference, not derivatives of rounded output. Public calls
re-admit all four input spans and recompute timing; a mutable timing POD is
not an admission certificate. Trusted quantization requires the admitted-edge
bound of finite nonnegative required seconds below 11250; trusted sampling
requires an admitted edge, its actual timing and 0<=tick<=N. Low-level
coefficient fixtures are not public velocity-limit admissions.

The [timing and numerical
assumptions](../docs/robotics/panda-quintic-timing.md) spell out the reserve
argument and integration limits. Under the declared
round-to-nearest/gradual-underflow/correct-sqrt/no-fastmath profile, a loose
estimate-error allowance below 2^-20 tick fits inside the extra tick. The
frozen independent oracle separately checks the real duration requirements and
sample extrema. This is a numerical timing result, not a compiler R14
certificate.

The example moves one reference angle by 0.3 rad under a 0.05 rad/s limit. Its
independently predicted estimate is 11.25 s and duration 11.252 s, or 5626
ticks. At the midpoint, the reference angle is 0.15 rad and rate
0.049991112691 rad/s. It also shows an exact endpoint, atomic tick refusal and
zero-edge representation.

<!-- source: psionic/examples/panda-quintic.coax pqt-timing -->
```jai
 // Endpoints are radians, limits rad/s and rad/s^2. This API returns a
 // numerical reference; it does not admit collision or tracking behavior.
 x:=pqt_fixture();x.q1=x.q0;x.q1[0].value=0.3;x.vlim[0].value=0.05;
 timing:[1]PqtTiming;#assert pqt_time(x.q0[..],x.q1[..],x.vlim[..],x.alim[..],timing[..])==PqtStatus.ok;
 print("ticks: %; total nanoseconds: %\n",timing[0].steps,timing[0].total_nanoseconds);
 print("duration (us): %; required estimate (us): %\n",pqt_example_micro(timing[0].duration.value),pqt_example_micro(timing[0].required_estimate.value));
 sample:[1]PqtSample;mid:=timing[0].steps/2;
 #assert pqt_sample(x.q0[..],x.q1[..],x.vlim[..],x.alim[..],mid,sample[..])==PqtStatus.ok;
 print("midpoint joint1: % urad; % urad/s; % urad/s^2\n",pqt_example_micro(sample[0].angle[0].value),pqt_example_micro(sample[0].rate[0].value),pqt_example_micro(sample[0].acceleration[0].value));
 #assert pqt_sample(x.q0[..],x.q1[..],x.vlim[..],x.alim[..],timing[0].steps,sample[..])==PqtStatus.ok;
 for i:0..<7 {#assert f64_bits(sample[0].angle[i].value)==f64_bits(x.q1[i].value) && f64_bits(sample[0].rate[i].value)==0 && f64_bits(sample[0].acceleration[i].value)==0;}
 print("endpoint joint1: % urad; exact target and +0 derivatives\n",pqt_example_micro(sample[0].angle[0].value));
 before:=sample[0];#assert pqt_sample(x.q0[..],x.q1[..],x.vlim[..],x.alim[..],timing[0].steps+1,sample[..])==PqtStatus.range;pqt_sample_same(sample[0],before);
 print("tick beyond duration: range; complete sample unchanged\n");
 // Numeric equality does not erase the target's signed-zero representation.
 x.q1=x.q0;x.q1[0].value=f64_from_bits(0x8000000000000000);
 #assert pqt_time(x.q0[..],x.q1[..],x.vlim[..],x.alim[..],timing[..])==PqtStatus.ok && timing[0].steps==0;
 #assert pqt_sample(x.q0[..],x.q1[..],x.vlim[..],x.alim[..],0,sample[..])==PqtStatus.ok && f64_bits(sample[0].angle[0].value)==0x8000000000000000;
 print("zero edge: % ticks; target signed zero retained\n",timing[0].steps);
```

```shell
coax/bin/coax run psionic/examples/panda-quintic.coax
```

Thirteen new source groups and four meaningful unit refusals qualify the new
policy and sampler. Focused root runs kill **107/89 actual emitted sites**.
The initial five/three survivors are retained, followed by exact per-role/tick
endpoint observations and one shared, independently observable stable-half
policy. No equivalence waiver or input/budget tuning was used. Counts do not
imply complete authored quantity-token mutation provenance. The example's
expected output was written independently before its first passing display
check and debug/release executions.

The new frozen corpus has **27 edges** and **238 sample queries**: 25 timing
results and 236 samples publish; two timing and two sample refusals preserve
caller output before unavailable transport markers. Exact-input Fraction
rounded-operation modeling predicts the policy N independently; 80-digit
Decimal real quintics predict the samples and velocity/acceleration extrema.
The largest estimate discrepancy is 1.384e-10 tick, with real reserve at least
0.9999999999999937 tick over this corpus. These are observations of fixed
cases, not a tuned epsilon or a new collision experiment.

The first debug, release and fail-fast ASan/UBSan profiles each pass **26,756
assertions**. Complete files match byte for byte; N/ns/status and the entire
timing file also match the independent oracle exactly. Maximum angle/rate
errors are 4.441e-16 rad and 4.441e-16 rad/s; acceleration error is 1.421e-14
rad/s². Forty-seven endpoint samples match exact bits and positive-zero
derivatives. Twenty-four overlap cases, 56 numeric endpoints, 140 IEEE cases,
10 tick refusals and 20 poisoned-reader guards pass. Frozen comparison budgets
remain 2e-12 rad absolute for angles; rates/accelerations use
2e-10+2e-12*abs(reference).

A static certified straight geometric edge would retain its point set under
this monotone timing change. This API neither constructs nor binds that
collision certificate. Tracking error, dynamic obstacles, jerk limits,
moving-state retargeting, global time optimality, R13 clocks, compiler bounds
and hardware remain separate. No SDK, neural, old physics/IK/nullspace/rotor
numerical corpus or mutation gate is repeated.

The [component record](../fixtures/robotics/panda-quintic/README.md) links the
frozen edge/sampling corpus, independent timing and analytic references, and
source/native/example qualifications.

## Joint errors become forces

The next runnable profile is a fixed-source seven-arm-joint PD query with
gravity compensation. It returns a force calculation; it does not advance a
plant or qualify a controller's closed-loop stability. For each hinge, the
requested force is:

```jai
P = Kp * (target_q - current_q);
D = Kd * (target_rate - current_rate);
requested = (P + D) + gravity_g0(current_q, finger_positions);
command = source_arm_clip(requested);
```

Seven positions and targets are typed radians, both inside the pinned source
joint ranges. Current and target rates are radians/second, bounded to ±20.
The two supplied finger positions are typed metres within the existing ±1 m
**numerical geometry** box. Their masses affect gravity, even though this
query returns arm forces only. That finger box does not admit finger actuator
commands or certify physical travel.

Radian remains a physical dimension. `PjpStiffness` is joules/radian², and
`PjpDamping` is joule·second/radian². Their products with angle and angular-rate
errors each become `PmHingeGravity`, joules/radian, through explicit unit
conversion. The default Kp coefficients are `[60,60,60,60,40,20,10]`; default
Kd is `[8,8,8,8,5,3,2]`. The numerical gain boxes are inclusive
`[1e-6,500]` and `[1e-6,100]`, respectively. All gains must be finite and
positive. These values define the query profile, not a stable-gain recipe for
arbitrary plants or timesteps.

`gravity` is the same owned sixteen-body potential-gradient contribution used
by the metric query, evaluated at the **current** arm and finger geometry.
It is independent of current velocity. It does not substitute the inverse
query's velocity-dependent bias, cancel fluid/joint passive force, or supply
a desired-acceleration feedforward term.

The 288-byte output publishes five seven-element force vectors:
`proportional`, `derivative`, `gravity`, `requested`, and `command`, followed
by the clipped-coordinate count. The command cap is the source arm actuator
profile: five ±80 and two ±12 joules/radian. Requested force remains visible;
there is no hidden saturation. Feed `command` into the existing Pa arm-control
channel once. Its equal cap is idempotent, and gravity must not be added a
second time. Keep controller and actuator clipping counters as separate stage
observations rather than summing them as unique events.

All span shapes precede numerical admission. The exact coefficient order is
current q, finger positions, current rates, target q, target rates, Kp, Kd;
each coefficient checks finite representation before its own range. Every
non-OK result preserves complete output and spare storage, including overlap
with input storage. Consumers must check status; preserved output is not a
new command. Joint coordinates have the fixed source index order rather than
a generic spatial-frame parameter. Wider frame/clock/supervision migration
remains separate.

The example prints the individual terms from a predeclared dense query. It
then exercises an existing numerical stress fixture with seven saturated
coordinates, and demonstrates atomic refusal of zero damping. Its rounded
transcript was written from literal scalar PD and a retained SDK gravity row
before either native example mode was executed.

<!-- source: psionic/examples/panda-joint-pd.coax pjp-query -->
```jai
 // All q/target entries are radians, rates rad/s, Kp J/rad² and Kd J*s/rad².
 // Fingers affect owned gravity; they are positions here, not control outputs.
 x:=pjp_fixture();delta:=[7]f64.[0.1,-0.12,0.13,-0.14,0.15,-0.16,0.17];
 v:=[7]f64.[0.7,-0.4,0.3,-0.6,0.5,-0.2,0.1];tv:=[7]f64.[-0.2,0.3,-0.4,0.5,-0.6,0.7,-0.8];
 for i:0..<7 {x.target[i].value=x.q[i].value+delta[i];x.rate[i].value=v[i];x.target_rate[i].value=tv[i];}
 out:[1]PjpOutput;
 #assert pjp_forward(x.q[..],x.slides[..],x.rate[..],x.target[..],x.target_rate[..],x.kp[..],x.kd[..],out[..])==PjpStatus.ok;
 for i:0..<7 {print("hinge % P/D/g0/requested/command (micro-J/rad): % % % % %\n",i+1,pjp_example_micro(out[0].proportional[i].value),pjp_example_micro(out[0].derivative[i].value),pjp_example_micro(out[0].gravity[i].value),pjp_example_micro(out[0].requested[i].value),pjp_example_micro(out[0].command[i].value));}
 print("controller clipped coordinates: %\n",out[0].clipped);
 // A predeclared numerical stress fixture, not a stable-gain recommendation.
 lo:=pik_lower();hi:=pik_upper();
 for i:0..<7 {positive:=i%2==0;x.target[i].value=ifx positive then hi[i] else lo[i];x.rate[i].value=ifx positive then -20.0 else 20.0;x.target_rate[i].value= -x.rate[i].value;x.kp[i].value=500;x.kd[i].value=100;}
 #assert pjp_forward(x.q[..],x.slides[..],x.rate[..],x.target[..],x.target_rate[..],x.kp[..],x.kd[..],out[..])==PjpStatus.ok && out[0].clipped==7;
 print("stress query clipped coordinates: %; source limits reported\n",out[0].clipped);
 before:=out[0];x.kd[0].value=0;
 #assert pjp_forward(x.q[..],x.slides[..],x.rate[..],x.target[..],x.target_rate[..],x.kp[..],x.kd[..],out[..])==PjpStatus.range;
 pjp_same(out[0],before);
 print("zero Kd: range refusal; complete output unchanged\n");
 print("feed command to Pa once; keep clipping stages separate\n");
```

```shell
coax/bin/coax run psionic/examples/panda-joint-pd.coax
COAX=coax/bin/coax psionic/tests/robotics/panda_joint_pd_check.sh /tmp/new-panda-joint-pd-proof
```

The frozen corpus has **44 admitted contexts**. Twenty reuse five explicitly
named zero-rate SDK gravity tuples with four different control regimes each;
all five selected arm tuples are the valid home configuration from the
`panda-fingers` capture. They are not the older zero-angle FK/M9 states whose
joint4 zero lies outside this controller's source range. The record verifies
binary/profile coordinate identity and the SDK's recorded zero-rate contract
before using those gravity facts. Twenty-four additional configurations,
targets, rates and gains use independently authored matrix potential-gradient
gravity. No source SDK or neural calls are repeated.

Eight new Coax groups cover **101 meaningful cases**. Two authority roots kill
**54/54** and **91/91** actually emitted sites, with zero survivors and no
equivalence waivers. The initial missing public-copy observations and fixture
setup error remain recorded. Quantity operator-token mutation provenance is
still incomplete; literal typed arithmetic checks and the independent native
field comparisons supplement the stated emitted-site scopes. Four E0317
refusals distinguish angle/length, angular/linear rate, stiffness/force and
damping/stiffness units. The debug/release example matches its independently
prewritten golden exactly.

The native receipt is
[`native-qualification.json`](../fixtures/robotics/panda-joint-pd/native-qualification.json).
Its absolute force budget is **5e-10 joules/radian**, with zero relative
tolerance and exact clipping/status fields. Debug, release and fail-fast ASan/UBSan each pass **6,420 assertions** over
all 44×35 force fields and clipping counters. Proportional and derivative
fields agree exactly; maximum errors are `2.14e-14` in gravity, `1.14e-13` in
requested force and `1.43e-14` in clipped command. The independent matrix
potential derivative differs from the 35 retained SDK gravity values by at
most `7.11e-15`. All three owned status/force/count streams are byte-identical.
The native proof covers 220 IEEE payloads, 88 inclusive scalar boundaries,
86 adjacent-field precedence cases, four overlapping-input/output cases and
complete input/output/spare snapshots. The initial metadata-only case-label
preflight failure is retained; it occurred before any numerical reference or
native execution. No budget, gain or corpus was tuned after comparison.

The pure query supplies force terms. A response experiment additionally needs
a fixed plant, timestep, initial state, target, gains and comparison contract.

## A gain test with an isolated rotor

A pose query and a PD torque formula do not tell us which gains behave well at a
particular step size. Study that question first in a deliberately smaller model:
a constant rotor with inertia M, physical damping B, and a fixed target. Its error
e is unwrapped angle from that target, and v is angular rate. The controller uses
`-Kp*e - Kd*v`; physical B acts implicitly at the **new** rate.

Keep dimensions visible. M is J*s²/rad², B and Kd are J*s/rad², Kp is J/rad², and
h is seconds. Define `D = M + h*B`, `alpha = h²*Kp/D`, and
`beta = (M-h*Kd)/D`. Normalize `[e, h*v]` by one explicit radian before reading
its numeric coordinates. The dimensionless transition is:

```jai
A = [[1 - alpha, beta],
     [   -alpha, beta]]
```

The physically typed step is `a = (-Kp*e-(B+Kd)*v)/D`, then `v += h*a`, then
`e += h*v` using that new rate. The returned acceleration is the effective
velocity change over this step. Shape and finite-domain admission precede
arithmetic, and acceleration/rate/error endcaps precede atomic publication.
An out-of-domain update refuses the entire result; it does not clamp a state.

For this constant real-arithmetic matrix and admitted positive M/Kp/h, strict
Jury stability is exactly `B+Kd > 0` and `4M+2hB-2hKd-h²Kp > 0`. The other Jury
term is alpha, already positive. The implementation reports a **computed F64
Jury diagnostic**, including a boundary class. That boundary is not a generic
boundedness or marginal-stability certificate: defective unit poles can grow
polynomially. Stable/unstable diagnostics do not authorize or prohibit a
numerically admitted step.

The continuous natural coefficient is `sqrt(Kp/M)` in **1/second**, because the
state's angle dimension cancels. It is not a joint rate in radian/second.
Continuous damping ratio is `(B+Kd)/(2*sqrt(M*Kp))`. Typed squared quantities are
converted before the explicit scalar square roots; no implicit angle erasure or
quantity-square-root language extension is used.

Continuous critical damping and a repeated discrete pole are different tests.
The continuous-critical fixture has M=1, B=0, Kp=4 and Kd=4: zeta is one, but its
0.002-second transition has distinct poles. The separate dyadic fixture uses
h=1/512, M=1, B=0, Kp=64 and Kd=15.875. Its transition has the exact repeated
pole 63/64, while continuous zeta is 127/128. The independent repeated-root
branch uses the appropriate n*r^n term rather than dividing by a zero pole gap.

More explicit derivative gain can also destabilize sampling. With M=0.01,
B=0.02, Kp=100 and h=0.002, raising Kd from 0.2 to 12 raises continuous zeta
from 0.11 to 6.01 but makes the computed flip margin negative. The `-2*h*Kd`
term explains this loss of strict discrete stability. It does not contradict
the continuous damping ratio; it describes a different update map.

All three rows below use the same M=0.01, B=0.02, Kp=100 and h=0.002; the
values are read from the already frozen independent analysis table.

| Kd (J*s/rad²) | Continuous zeta | Computed flip margin (J*s²/rad²) | Strict class |
| --- | --- | --- | --- |
| 0.2 | 0.11 | 0.03888 | Stable |
| 4 | 2.01 | 0.02368 | Stable |
| 12 | 6.01 | -0.00832 | Unstable |

The [example](../examples/rotor-pd.coax) runs a 0.002-second isolated rotor and prints selected response
states against an independently prewritten closed-form output. Its initial
acceleration is unavailable because no step has yet occurred. It also shows the
exact Kd=3 one-ULP neighbors at a coarse 0.5-second **mathematical** fixture:
computed strict stable, boundary, and unstable. These are not Panda gains or a
nonlinear plant certificate.

<!-- source: psionic/examples/rotor-pd.coax rpd-response -->
```jai
 // M is J*s^2/rad^2; B and Kd are J*s/rad^2; Kp is J/rad^2.
 // Physical B acts implicitly at the new velocity; controller Kd uses old rate.
 p:[1]RpdParams;p[0].inertia.value=0.01;p[0].physical_damping.value=0.02;p[0].kp.value=100;p[0].kd.value=0.2;p[0].step.value=0.002;
 analysis:[1]RpdAnalysis;#assert rpd_analyze(p[..],analysis[..])==RpdStatus.ok;
 print("natural coefficient (1/s): %; zeta (ppm): %\n",cast(int)analysis[0].natural_inverse_seconds.value,rpd_example_micro(analysis[0].zeta.value));
 print("alpha/beta (ppm): % %; strict Jury: %\n",rpd_example_micro(analysis[0].alpha.value),rpd_example_micro(analysis[0].beta.value),analysis[0].strict_stable);
 state:[1]RpdState;state[0].error.value=0.375;state[0].rate.value= -0.5;out:[1]RpdOutput;
 print("initial: % urad; % urad/s; acceleration unavailable\n",rpd_example_micro(state[0].error.value),rpd_example_micro(state[0].rate.value));
 for n:1..<129 {
  #assert rpd_step(p[..],state[..],out[..])==RpdStatus.ok;state[0]=out[0].state;
  if n==1 || n==32 || n==64 || n==128 {print("step %: % urad; % urad/s; % urad/s^2\n",n,rpd_example_micro(state[0].error.value),rpd_example_micro(state[0].rate.value),rpd_example_micro(out[0].effective_acceleration.value));}
 }
 // Exact one-ULP neighbors distinguish the strict computed Jury boundary.
 p[0].inertia.value=1;p[0].physical_damping.value=0;p[0].kp.value=4;p[0].step.value=0.5;
 for side:0..<3 {word:=f64_bits(3.0);if side==0 {word-=1;}if side==2 {word+=1;}p[0].kd.value=f64_from_bits(word);#assert rpd_analyze(p[..],analysis[..])==RpdStatus.ok;print("Kd3 neighbor %: class %; strict %\n",side,analysis[0].classification,analysis[0].strict_stable);}
 before:=out[0];state[0].error.value=1e6+1;#assert rpd_step(p[..],state[..],out[..])==RpdStatus.range;rpd_same(out[0],before);
 print("outside error domain: range; complete result unchanged\n");
```

```shell
coax/bin/coax run psionic/examples/rotor-pd.coax
COAX=coax/bin/coax psionic/tests/robotics/check-rotor-pd-native.sh /tmp/new-rotor-proof
```

The independent reference freezes eleven profiles and two initial states each.
It uses exact binary-input Fractions and 80-digit Decimal root closed forms,
including distinct-real, complex-conjugate and repeated-root branches. It does
not use the product recurrence as its expected response. There are **2,032 new
applied updates** and **22 input-transport initial records**; the latter carry a
zero acceleration placeholder; the response index explicitly marks it
unavailable. No initial acceleration is computed. All thirteen analysis coefficients and both classification
fields are checked.

Eight new source groups, four meaningful unit refusals, and **136/136 actual
emitted mutation sites** qualify the new root. The first mutation run's single
survivor hid `2*sqrt` versus `2+sqrt` at the coincident literal square root two;
an independent dense profile closes that observation without changing the
product or budgets. Current quantity lowering omits some authored token
provenance, so this count is not full quantity-token mutation coverage. The
example passes its own display check and its prewritten debug/release output.
The first debug, release and fail-fast ASan/UBSan profiles each pass **22,225 assertions**. Complete analysis and response files are byte-identical across all three. Eleven analyses, 2,032 applied steps and 22 explicitly unapplied initial records match the independent reference. Maximum analysis discrepancy is 2.23e-16. Exact one-ULP Jury margins are 0 and +/-2^-51. Fourteen overlap cases, parameter/state endpoints, isolated late result endcaps, IEEE poisons and sixteen poisoned-reader guards pass. The worst response budget fraction is **1.125%**; the largest absolute acceleration discrepancy is **5.22e-8 rad/s²** on large values, within the frozen combined absolute/relative budget.

The per-scalar response bound is **2e-10 + 2e-12*abs(reference)** for error,
rate and effective acceleration; analysis uses **2e-12 + 2e-12*abs(reference)**.
Classification, statuses and cross-profile bytes have exact criteria. No old
Panda, pose, kinematic, physics, SDK, neural or renderer gate is repeated. This
isolated constant-rotor evidence does not establish nonlinear Panda stability,
contact behavior, motor-command admission, compiler `#bounded` control or
hardware behavior.

The [component record](../fixtures/robotics/rotor-pd/README.md) links the
exact-input Jury facts, independent closed forms and source/native/example
receipts.


## Couple PD to the owned plant

The next [runnable example](../examples/panda-joint-pd-rollout.coax) couples
that controller to [chapter 170's own Euler map](ch170-close-the-simulated-control-loop.md).
At each step it computes a fresh PD force from its current state, then advances
all seven hinges and both fingers by 2 ms. Its target is a small fixed offset
from the source home pose, target rates are zero, gains remain at the stated
defaults, and signed finger targets stay fixed. External force is zero.

<!-- source: psionic/examples/panda-joint-pd-rollout.coax pjp-rollout -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);sv:[2]pfku.Quantity(PidLinearRate,f64);
 target:[7]pfku.Quantity(pfku.Radian,f64);target_v:[7]pfku.Quantity(PidAngularRate,f64);
 home:=[7]f64.[0,0.19634954084936207,0,-2.617993877991494,0,2.941592653589793,0.7853981633974483];offset:=[7]f64.[0.02,-0.015,0.01,-0.02,0.015,0.01,-0.005];
 for i:0..<7 {q[i].value=home[i];target[i].value=home[i]+offset[i];}
 s[0].value=0.013;s[1].value=-0.027;finger_target:=s;
 kp:=pjp_default_kp();kd:=pjp_default_kd();external_h:[7]pfku.Quantity(PmHingeGravity,f64);external_s:[2]pfku.Quantity(PmSlideGravity,f64);clipped:int=0;
 for step:0..<250 {
  force:[1]PjpOutput;#assert pjp_forward(q[..],s[..],v[..],target[..],target_v[..],kp[..],kd[..],force[..])==PjpStatus.ok;clipped+=force[0].clipped;
  next:[1]PeuOutput;#assert peu_step(q[..],s[..],v[..],sv[..],force[0].command[..],finger_target[..],external_h[..],external_s[..],next[..])==PmsStatus.ok;
  q=next[0].angles;s=next[0].slides;v=next[0].hinge_rates;sv=next[0].slide_rates;
 }
 print("own no-contact joint-PD steps: 250\n");
 for i:0..<7 {print("hinge % q/v (micro-rad, micro-rad/s): % %\n",i+1,pjr_micro(q[i].value),pjr_micro(v[i].value));}
 for i:0..<2 {print("slide % q/v (micro-m, micro-m/s): % %\n",i+1,pjr_micro(s[i].value),pjr_micro(sv[i].value));}
 print("PD clips: %\n",clipped);
```

```shell
coax/bin/coax run psionic/examples/panda-joint-pd-rollout.coax
COAX=coax/bin/coax psionic/tests/robotics/panda_joint_pd_prefix_check.sh /tmp/new-panda-pd-prefix
```

The [frozen experiment](../fixtures/robotics/panda-joint-pd-prefix/experiment.json)
was set before the first comparison. One MuJoCo 3.3.1 acquisition uses the
same initial state, controller and held targets: one explicit forward and
250 steps, with source constraints/contact disabled (flags 17) and implicit
Euler joint damping enabled. There are no post-step forwards, neural calls
or renders. All 502 before/after effect events are retained. This is a
half-second simulated prefix; it supplies no wall-clock deadline evidence.

The SDK records **13,500 finite floating observations**. A separately authored
matrix/potential oracle checks all **8,750 controller fields** against the
initial state and then each preceding post-step state. P/D match exactly;
gravity, requested force and command differ by at most **3.20e-14 J/rad**
under the frozen **5e-10** absolute budget. All 250 PJP clipping counts match
exactly and are zero. Statuses are checked before publication, rather than
serialized as additional trace fields.

Next, three SDK-free native profiles each run two owned branches: fresh PD on
their own state, and a shadow plant driven by the recorded SDK commands.
Debug, release and fail-fast ASan/UBSan each pass **56,058 assertions**;
complete owned traces are byte-identical across those modes. Maximum fresh
arm-position error is **8.33e-17 rad** (budget 1e-8), finger-position error
**1.74e-18 m** (1e-9), arm-rate error **4.83e-16 rad/s** (1e-7), finger-rate
error **1.02e-17 m/s** (1e-8), and force-field/command error **2.49e-14 J/rad**
(1e-6). Relative tolerance is zero. The recorded-command branch separates
plant agreement from feedback agreement; its maximum arm-rate error is
8.63e-16 rad/s. Owned actuator-force clip counts use the independent scalar
source law, not an SDK-observed counter.

The target-error norm falls from **0.0384057 to 0.00993709 rad**. This is a
reported result for these gains and this short prefix, without a convergence,
steady-state or stability threshold. This prefix does not qualify contact response, large disturbances, nonlinear
stability, general timed-reference tracking or whole-policy success; the
following cases add specific measurements.

The [complete evidence](../fixtures/robotics/panda-joint-pd-prefix/README.md)
retains the first generated-C printer-warning build failure, before any SDK
load, and its narrow build-only followup. Passed emissions and decoder guards
were reused; the SDK prefix and three numerical profiles ran once. The example's
[transcript](../examples/panda-joint-pd-rollout.out) was written from the SDK
endpoint before its first debug/release execution. Historical PD and Euler
mutation scopes are carried separately on unchanged product files; this
composition adds no new admission rule or compiler boundedness claim.

## Measure a nonlinear hold, track, disturbance and brake

A scalar rotor's gain diagnostic does not establish what the coupled Panda does. This experiment composes the existing source-bound nonlinear plant with the joint PD controller and the rest-to-rest reference. Four fresh cases each run 750 updates at 500 Hz, or 1.5 seconds. The scene uses the pinned Panda's inertias, armature, physical joint damping, fluid and source actuators, with contacts and constraints disabled. It measures this isolated numerical profile. Each case starts from compiled XML defaults with named robot-coordinate overrides. The run measures these controlled initializations; it supplies no LIBERO task-reset or neural-task success score.

| Case | Reference and initial condition | External force |
| --- | --- | --- |
| Hold | New dense arm configuration, zero initial rates; keep the initial target. | Zero. |
| Track | Move seven joints by the frozen small offsets; reference limits are 0.12 rad/s and 0.6 rad/s². | Zero. |
| Pulse | Same tracking reference. | Joint 2 receives +3 J/rad for control ticks 200 through 249. |
| Brake | Keep the initial target; joint 7 starts at +2 rad/s and uses Kd=20 J·s/rad². | Zero. |

The tracking reference lasts 270 ticks, or 0.54 seconds. Control tick `k` uses reference `min(k,270)` and holds the endpoint afterward. The post-integration state is compared with reference `k+1`. This alignment matters around the pulse and the end of motion. Both fingers retain their initial position targets; their actual drift is measured. The quintic reference has zero endpoint velocity and acceleration; it is not a retargeter for a moving VLA action chunk.

The [fixed test consumer](../tests/robotics/panda-joint-pd-tracking.coax) owns the case constants, gains, reference selection, pulse interval and composition in Coax. `pnt_control` returns the reference, all five controller vectors, the clipped controls, external forces, actuator prediction and separate clipping counts. It reads the current raw joint positions and rates and computes fresh owned gravity; it does not substitute cached SDK body geometry. `pnt_step` stages the same command with one owned Euler update. Observing the actuator forces for diagnostics does not add them a second time to the plant.

<!-- source: psionic/examples/panda-joint-pd-tracking.coax pnt-tracking -->
```jai
 // Public inputs choose one of the four frozen experiments, never new gains.
 // q9/v9 are explicit mixed-coordinate raw boundaries; outputs retain units.
 initial:[1]PntInitial;command:[1]PntCommand;
 #assert pnt_ok(pnt_initial(1,initial[..]));q:=initial[0].q;v:=initial[0].v;
 #assert pnt_ok(pnt_control(1,135,q[..],v[..],command[..]));
 print("track midpoint: reference tick %/%; joint1 % urad; % urad/s\n",command[0].reference_tick,command[0].reference_steps,pnt_example_micro(command[0].reference.angle[0].value),pnt_example_micro(command[0].reference.rate[0].value));
 #assert pnt_ok(pnt_control(2,200,q[..],v[..],command[..]));
 print("pulse at tick 200: active %; joint2 external % micro-J/rad\n",command[0].pulse,pnt_example_micro(command[0].external[1]));
 #assert pnt_ok(pnt_initial(3,initial[..]));q=initial[0].q;v=initial[0].v;
 #assert pnt_ok(pnt_control(3,0,q[..],v[..],command[..]));c:=command[0].controller;
 print("brake joint7 P/D/g0/requested/command (micro-J/rad): % % % % %\n",pnt_example_micro(c.proportional[6].value),pnt_example_micro(c.derivative[6].value),pnt_example_micro(c.gravity[6].value),pnt_example_micro(c.requested[6].value),pnt_example_micro(c.command[6].value));
 print("controller clips %; actuator control/force clips %/%\n",c.clipped,command[0].actuator_control_clipped,command[0].actuator_force_clipped);
 old:=command[0];status:=pnt_control(3,750,q[..],v[..],command[..]);#assert status.stage==2 && status.code==2;pnt_command_same(command[0],old);
 print("tick 750: stage % code %; complete command unchanged\n",status.stage,status.code);
```

```shell
coax/bin/coax run psionic/examples/panda-joint-pd-tracking.coax
COAX=coax/bin/coax psionic/tests/robotics/check-panda-joint-pd-tracking.sh /tmp/new-panda-tracking
```

The runnable example executes pure queries. At the initial brake state, the independent gravity value for joint 7 is 0.0015419353651608013 J/rad. The derivative term is −40 J/rad, so the requested command is −39.99845806463484 J/rad and the controller clips it to −12 J/rad. Its one clipped coordinate is separate from the actuator's control and force clipping counts. A failed case/tick/admission leaves the complete command or step output unchanged and returns both the failing stage and its underlying code.

The new composition has twelve source check groups and 194 actual emitted root mutation sites, all killed. Initial runs exposed seven missing observations; new dense-rate, slider-role, real nonzero actuator-clipping and complete plant-output checks closed them without changing production bodies. The deliberately altered unsealed diagnostic configurations are source tests, not extra experiment cases. The counts describe emitted mutation sites; they do not close the compiler's wider quantity-token provenance gap. Existing controller, timing, actuation and dynamics proofs carry unchanged.

The SDK branch uses the same Coax controller with genuine SDK joint states. It has one initial forward per case and no explicit post-step forwards; SDK actuator observations are labelled as the pre-integration cache left by the step. Independent scalar PD and matrix potential-gradient calculations audit every recorded controller field, while the original analytic reference supplies all 751 frames per case. Separate owned branches use their own evolving feedback state and replay the SDK's recorded commands on an owned plant to distinguish feedback accumulation from plant response.

The first acquisition completed exactly 3,000 SDK steps and four initial forwards. All three new owned/native profiles passed, each with 1,329,240 assertions; their complete output files are byte-identical. Against the SDK, the largest fresh-feedback joint position/rate errors are 4.44e−16 rad and 1.60e−15 rad/s; recorded-command shadow errors are 3.33e−15 rad and 5.75e−15 rad/s. Independent matrix/scalar auditing of all 105,000 controller and 27,000 actuator fields passes, with controller error at most 6.04e−14 and actuator values numerically exact.

| Behavior in the SDK trace | Measured result | Frozen goal |
| --- | --- | --- |
| Hold peak arm drift | 0.0000162924 rad | Pass: ≤0.005 rad. |
| Nominal tracking RMS | 0.00725912 rad versus analytic stationary 0.0199807 rad | Pass: smaller RMS. |
| Nominal final maximum error | 0.0157710 rad | **Miss: goal ≤0.01 rad.** |
| Pulse final maximum error | 0.0159404 rad | Pass: ≤0.02 rad. |
| Pulse's 25-state error/rate band | Entry state 477, confirmed at 501 | Observed temporary band; not permanent settling. |
| Brake | 31 controller-clipped ticks; final error 0.0455202 rad | **Not recovered within 1.5 seconds.** |

Pulse-band entry is 0.454 seconds after the force interval ends; confirmation is at 1.002 seconds from reset. The trajectory later leaves the 0.01 rad band, as its final error shows. No actuator-stage control or force clips were observed. Every recorded post-state was within the source joint coordinate ranges. The reference's 0.12 rad/s and 0.6 rad/s² limits constrain its commanded curve, not actual motion: the brake starts at 2 rad/s and the measured peak effective arm acceleration reaches about 41.587 rad/s². These misses are retained without changing gains, cases, horizons or tolerances.

Numerical parity and behavior goals are separate results. The frozen goals include hold drift ≤0.005 rad, nominal final error ≤0.01 rad, pulse final error ≤0.02 rad, and recovery for 25 consecutive states within both 0.01 rad and 0.05 rad/s. Recovery reports entry and confirmation ticks. The stationary-position comparator is analytic, not another executed controller. A missed goal stays in the record. No gains, cases, horizons or budgets are adjusted after observing a run.

The public entry points accept only the four fixed cases and ticks 0 through 749. Their raw `q9`/`v9` inputs are explicit mixed-coordinate boundaries; physical reference, controller and actuator fields retain their quantity types. Low-level case, reference, control and apply helpers are source-visible and unsealed: their valid-profile, current-state and coherent-command preconditions must be preserved. Their PODs are not admission certificates.

This is short unconstrained behavior evidence. It does not establish global or nonlinear stability, collisions, contacts, hardware safety, general trajectory following or compiler-enforced real-time bounds. Finite nullspace integration and collision/RRT work remain separate.

The [component evidence](../fixtures/robotics/panda-joint-pd-tracking/README.md)
links the frozen inputs, source checks, first SDK acquisition, independent
controller audit and native profiles. The additive offline gate uses committed
trace/input/receipts and builds SDK-free current-source binaries. Its new
adapter was syntax/plan/stub-routing checked here; the measured numerical proof
above is carried unchanged, rather than rerun through that new adapter. New
SDK acquisition is a separate opt-in tool and is not part of this gate.

## Search a frozen gain family on the owned plant

The four-case result above exposes two missed goals. We now make gain selection a reproducible owned-model experiment. Before running a new trajectory, freeze nine scale pairs in row-major order: Kp scales `[1,2,4]`, with Kd scales `[0.5,1,2]` inside each row. Profile 1 is the original `(1,1)` pair. Its four historical fresh-feedback traces are carried by exact source, reference, numerical-profile and byte identities; they are never simulated again. Only the other eight pairs execute: 32 new rollouts, bounded by 24,000 new owned steps. There is no SDK, NN or render during this selection.

These are scales for a scenario-conditioned gain family. The original brake uses Kd7=20 while the ordinary cases use Kd7=2. Applying one scale pair preserves that distinction; it does not discover one universal gain vector. The existing controller, source actuators, reference and nonlinear Euler plant remain unchanged. The [new test consumer](../tests/robotics/panda-pd-gain-search.coax) owns only the profile factory, carry refusal, scoring and deterministic selection.

The new goal is stronger than observing one early pulse recovery window. Every joint in **all four cases** must stay within 0.01 rad of its reference and within 0.05 rad/s of reference rate at **every post-state from 626 through 750**. Hold must additionally keep full-run drift within 0.005 rad. Nominal tracking must beat the analytic stationary-position baseline's global squared position error; equal sample counts make this the same strict ordering as RMS. The baseline is an analytic comparator, not another executed controller. Reference tick `k+1` is compared with post-state `k+1`.

A refusal, nonfinite observation, post-state source-range excursion, broken previous-rate lineage or incomplete trace makes a candidate invalid. All nine coordinate ranges include both slide joints. Intentional controller or actuator clipping is recorded rather than treated as automatic invalidity. Peak actual speed, effective step acceleration and minimum source-range margin are diagnostic fields; reference limits do not constrain these actual quantities.

Among valid candidates meeting every goal, compare these keys exactly, without an epsilon:

1. Worst terminal normalized error, `max(abs(q_error)/0.01, abs(rate_error)/0.05)`.
2. Total position IAE, in rad·s, over the four cases.
3. Total number of controller-clipped coordinates.
4. Profile ID in the fixed nine-pair order.

SSE reduces in tick-major, joint-major order. For IAE, sum seven absolute errors for each post-state, multiply that row by 0.002 seconds, then accumulate rows; combined cards add cases 0 through 3. Each candidate retains every case score and goal flag. The selector publishes both winner and next feasible runner-up, the first separating key and signed runner-minus-winner gaps. If no candidate meets all goals, it returns no winner. No larger grid or best-available promotion follows a miss.

<!-- source: psionic/examples/panda-pd-gain-search.coax pgs-study -->
```jai
 gains:[1]PgsGains;
 #assert pnt_ok(pgs_gains(8,0,gains[..]));
 print("profile 8 ordinary joint7 Kp/Kd: %/%\n",cast(int)gains[0].kp[6].value,cast(int)gains[0].kd[6].value);
 #assert pnt_ok(pgs_gains(8,3,gains[..]));
 print("profile 8 brake joint7 Kp/Kd: %/%\n",cast(int)gains[0].kp[6].value,cast(int)gains[0].kd[6].value);
 // The profile scales a scenario-conditioned family, not a universal vector.
 initial:[1]PntInitial;#assert pnt_ok(pnt_initial(0,initial[..]));q:=initial[0].q;v:=initial[0].v;
 step:=[1]PntStep.[pnt_step_poison()];old:=step[0];status:=pgs_step(1,0,0,q[..],v[..],step[..]);
 #assert status.stage==7 && status.code==2;pnt_step_same(step[0],old);
 print("baseline step refused: stage % code %; carry saved trajectories\n",status.stage,status.code);
 // Missing evidence produces a finite invalid card, not an admitted candidate.
 observations:[1]PgsObservation;score:[1]PgsCaseScore;
 #assert pgs_score(0,0,observations[0..<0],score[..])==PgsStatus.ok;
 print("empty trace: valid % complete % feasible % reason %\n",score[0].valid,score[0].complete,score[0].feasible,score[0].reason);
 // These are literal absent-evidence policy fixtures, not model results.
 candidates:[9]PgsCandidate;for i:0..<9 {candidates[i].profile=i;}
 selected:[1]PgsSelection;#assert pgs_select(candidates[..],selected[..])==PgsStatus.ok;
 print("no feasible evidence: winner % runner % feasible %\n",selected[0].winner,selected[0].runner_up,selected[0].feasible_count);
```

```shell
coax/bin/coax run psionic/examples/panda-pd-gain-search.coax
COAX=coax/bin/coax psionic/tests/robotics/check-panda-pd-gain-search.sh /tmp/new-panda-gain-cached
```

This pure example runs no rollout. It demonstrates gain metadata, baseline refusal, a finite invalid empty trace and a no-feasible result using explicitly labelled absent-evidence policy fixtures. The measured scorecards are separate artifacts.

The scorer consumes cached post-state rows. For a zero/partial/faulted trace it returns a finite invalid card and preserves metrics only through the admitted prefix. API shape/index refusals preserve the complete caller output. The trace cache binds profile/case/tick identities, while Coax derives completeness, previous-rate lineage, reference alignment and goal decisions. Case and combined cards are source-visible and unsealed observations, not trajectory or goal-proof certificates. Trusted reducer helpers require admitted finite bounded inputs, coherent prior metrics and the unchanged fixed-case reference profile.

The new source qualifies 25 groups and 578 actual emitted root mutation sites: 73 control, 219 scorer and 286 selector, all killed. These counts describe actual sites, not full Quantity-token provenance or compiler-enforced bounds. The canonical physics grid executes once in release mode. Three native scorer/selector profiles then consume the same cached observations; they do not rerun the plant. Independent tooling checks objective fields under the predeclared 2e-12 absolute plus 2e-12 relative budget, and verifies flags, exact policy ordering and any threshold/tie ambiguity separately.

The one release grid completed all 32 new rollouts and 24,000 new steps; the carried baseline contributed four old trajectories and zero new plant steps. Every one of the 36 casecards is valid and complete. **None of the nine profiles satisfies every goal.** The selector returns winner −1, runner-up −1 and zero feasible candidates; all key-gap fields are finite zero. There is no qualified family to send to held-out SDK validation.

| Profile | Kp × | Kd × | Worst terminal ratio | Total IAE (rad·s) | Controller-clipped coordinates | Feasible |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 1 | 0.5 | 3.879737 | 0.266187 | 14 | No |
| 1 (carried) | 1 | 1 | 5.157346 | 0.229669 | 31 | No |
| 2 | 1 | 2 | 6.202255 | 0.198056 | 41 | No |
| 3 | 2 | 0.5 | 1.843086 | 0.206789 | 16 | No |
| 4 | 2 | 1 | 2.854085 | 0.190134 | 32 | No |
| 5 | 2 | 2 | 4.643709 | 0.173539 | 42 | No |
| 6 | 4 | 0.5 | 1.730989 | 0.152021 | 18 | No |
| 7 | 4 | 1 | 1.355279 | 0.143858 | 35 | No |
| 8 | 4 | 2 | 2.587362 | 0.139504 | 43 | No |

A terminal ratio at most one is required. Profile 7 has the smallest observed worst ratio, 1.35528, so even that diagnostic minimum misses. It is not selected or promoted as a best-available controller. Profiles 5 and 8 satisfy the moving-case terminal bands but fail braking; profiles 6 and 7 satisfy braking but fail the moving cases. Every profile meets the hold-drift and nominal-SSE improvement requirements. These observed tradeoffs apply to the fixed grid and horizon, not to all possible gains.

The three cached scorer/selector profiles each pass 515,393 assertions and produce exact complete score bytes. They simulate no plant steps. Independent direct-array reductions check all 1,152 case metric coefficients with maximum error 5.77e−14, using at most 0.9324% of the frozen mixed budget; all goal flags, counters and selector outcomes agree exactly. Fourteen cache poisons, fourteen score-reader poisons and six exact lexicographic witnesses pass. No second grid, SDK call, expanded candidate set or changed goal follows this result.

Two new geometry starts and their four scenarios were frozen before selection. No held-out validation was run after this no-winner outcome. A future separately frozen study with a qualifying family would require one-shot held-out validation: one chosen family only, both pulse signs and brake directions, and no trying a runner-up on the SDK. Owned-plant selection is not held-out validation, global optimization, general nonlinear stability or hardware qualification. Collision/contact handling, finite nullspace integration and compiler bounds remain separate.

The [complete study evidence](../fixtures/robotics/panda-gain-search/README.md)
links all nine scorecards, exact baseline carry, progress and independent
objective receipts. The default gate consumes committed cached observations
and runs only scorer/selector contexts. Its additive adapter was syntax/plan/
stub-routing checked here; original measured numerical evidence is carried
unchanged, with no canonical-grid or baseline rerun.

## One common rate-error damping law

The frozen nine-pair study remains a no-winner result. This is a separate hypothesis, sealed before new braking data: use one common gain law for every original scenario. Kp is `[240,240,240,240,160,80,40]`; the first six Kd coefficients are `[16,16,16,16,10,6]`. There is no legacy brake override. Joint 7 always uses the same law based on **reference-rate error**, not speed alone.

For `e = reference_rate7 - actual_rate7`, let `s = abs(e)`, in rad/s. Kd7 is 4 J·s/rad² at `s <= 0.1`, 20 at `s >= 0.5`, and otherwise exactly `4 + 16 * ((s - 0.1) / (0.5 - 0.1))`. The F64 denominator/order is fixed and native contraction is disabled. The absolute diagnostic clears the sign bit, including signed zero; the signed error retains the subtraction result. Regime comes from the thresholds, not the rounded gain.

The gain cap of 20 does not cap the incremental derivative-force slope at 20. For the underlying real-valued interpolation, the derivative force is `D(e)=Kd(abs(e))*e`, with slope `Kd(s)+s*Kd'(s)` between knots. In the declared coefficient basis the ramp slope is approximately `80*s`, approaching 40 J·s/rad² from below; the mathematical knot has distinct one-sided slopes. This is an interpretation of the underlying curve, not a derivative of the quantized output map. Stored F64 knots, operation order and rounding remain normative for the implementation. A nonnegative scalar damping coefficient is not a proof of whole-plant passivity or nonlinear stability: moving references, coupled dynamics, torque saturation and discrete integration still matter.

<!-- source: psionic/examples/panda-scheduled-pd.coax psd-schedule -->
```jai
 // Angular rates are rad/s; damping is J*s/rad^2. These are scalar fixtures.
 schedule:[1]PsdSchedule;
 for error:[3]f64.[0.05,0.3,2] {
  #assert psd_schedule_raw(error,0,schedule[..])==PjpStatus.ok;
  print("error (micro-rad/s): %; Kd7 (micro-J*s/rad^2): %; regime %\n",psd_example_micro(schedule[0].signed_error.value),psd_example_micro(schedule[0].damping.value),schedule[0].regime);
 }
 negative_zero:=f64_from_bits(0x8000000000000000);
 #assert psd_schedule_raw(negative_zero,0,schedule[..])==PjpStatus.ok;
 print("zero diagnostics: negative signed zero %; absolute bits %\n",(ifx f64_bits(schedule[0].signed_error.value)==0x8000000000000000 then 1 else 0),f64_bits(schedule[0].absolute_error.value));
 gains:[1]PsdGains;#assert psd_gains_raw(2,0,gains[..])==PjpStatus.ok;
 print("same law for every case: joint7 Kp/Kd %/%\n",cast(int)gains[0].gains.kp[6].value,cast(int)gains[0].gains.kd[6].value);
 old:=schedule[0];status:=psd_schedule_raw(21,0,schedule[..]);#assert status==PjpStatus.range;psd_schedule_same(schedule[0],old);
 print("rate outside numerical box: range; complete schedule unchanged\n");
```

```shell
coax/bin/coax run psionic/examples/panda-scheduled-pd.coax
COAX=coax/bin/coax psionic/tests/robotics/check-panda-scheduled-pd.sh /tmp/new-panda-scheduled-cached
```

The example executes scalar schedule and gain queries only: no force, plant, SDK or NN. Its fixtures and prewritten transcript are separate from the measurements below.

`psd_schedule` and `psd_gains` have typed angular-rate inputs and explicit raw-coefficient adapters. `psd_force` returns all five PD/gravity vectors with the schedule; its inputs carry radian/metre/rate quantities. Every view shape is checked before indexing, then both scheduling rates are checked finite before either ±20 rad/s numerical bound. Scheduled-force admission precedes the unchanged Pjp input admission. Complete outputs publish only on success. The [new command/step consumer](../tests/robotics/panda-scheduled-pd.coax) snapshots raw `q9/v9`, carries stage plus underlying status, and keeps controller versus actuator clipping distinct. It never reads the old case factory's Kp/Kd fields.

The scalar law is a source-visible helper requiring finite rate error within ±40 rad/s. The fixed-reference and staged-apply helpers are also unsealed and require their declared coherent inputs; returned PODs are not control authority or goal-proof certificates. Scoring uses the immutable earlier reductions under **scheduled-v1/local ID0**, not old grid profile 0 or profile 8. There is no nine-candidate selector in this experiment.

The source qualifies eleven new groups, 233 actual emitted root sites (117 control/116 public), and four meaningful unit refusals. Initial source-depth and observational failures are retained; only new load placement was flattened, with no compiler budget change or old source edit. Counts do not claim full Quantity-token mutation coverage or compiler-enforced bounded control.

The carry condition is stronger than matching gain constants. At every cached original state in cases 0..2, compute a **new complete command** and compare all 712 base-command bytes with authenticated old profile-8 data: 2,250 comparisons must pass before any brake step. Only then may those three unchanged trajectories be reused. A carry mismatch stops the phase; there is no fallback simulation. Extra signed/absolute error, damping and regime diagnostics are separately observed. Only the brake contributes 750 genuinely new plant updates; subsequent native scalar/command/metric checks use cached observations and do not repeat it.

All 2,250 new ordinary-case commands match the authenticated 712-byte base commands exactly, and all their 1,040-byte old records carry without change. No ordinary-case physics is simulated. Only after this passes does one release-mode brake run complete 750 new steps, with 202,535 assertions. Three cached native metric/assessment profiles then each pass 381,267 assertions and produce exact bytes, without repeating the brake. The three unchanged old metric cards carry their original proofs with only the explicit new local-ID remap.

**All four original-case goal sets pass under this new law.** The worst normalized terminal error is 0.8379269323, below one; total IAE is 0.1049563997 rad·s. The new brake's terminal ratio is 0.2424512135, with 34 controller-clipped coordinates and no actuator-stage clips.

| Original scenario | Worst terminal normalized error | Goal set |
| --- | --- | --- |
| Hold, exact carry | 0.0001732912 | Pass |
| Nominal tracking, exact carry | 0.8280275411 | Pass |
| Pulse tracking, exact carry | 0.8379269323 | Pass |
| New brake | 0.2424512135 | Pass |

The brake's final maximum position/rate errors are 0.00126465 rad and 0.0106499 rad/s. Across the complete final 125-state window, peak errors are 0.00242451 rad and 0.0106920 rad/s. A first 25-state band is observed at states 363 through 387. Treat this as an observed window, not permanent settling or an independently checked never-leaves suffix. The later full terminal window626..750 is the actual acceptance criterion and independently passes. Neither observation establishes asymptotic or permanent settling outside this finite horizon.

Independent auditing checks all 3,000 diagnostic tuples exactly and retains the exact command/record carry. The 32 fresh brake metrics differ by at most 4.22e−15 and all 26,250 fresh PD fields by at most 4.62e−14. Twenty-three reader poisons are refused. The observed regimes total 2,714 low, 253 ramp and 33 high samples; the carried ordinary cases contribute 2,250 of the low samples. The scalar schedule, gain and ABI probes already pass in all three native host modes. No source law, gain, criterion or prospective descriptor changes follow these observations.

The original four scenarios retain the strict whole terminal window: every joint/state626..750 must satisfy both 0.01 rad and 0.05 rad/s error, with full-run hold drift ≤0.005 rad and nominal SSE better than its analytic stationary comparator. Reference limits do not bound actual motion. C/D assessment descriptors were sealed prospectively but are **not executed in this slice**; passing these original cases would remain implementation evidence, not prospective validation. The old Part-A NONE outcome is immutable. No SDK, contact, hardware, global stability, collision/RRT, finite nullspace integration or compiler real-time qualification follows from this component.

The [component evidence](../fixtures/robotics/panda-scheduled-pd/README.md)
links the sealed descriptor, independent scalar table, exact command carry,
single-brake and independent metric/force audit. The additive default gate uses
committed cached observations only; it performs no brake collection or SDK
execution. Its wrapper/adapter is syntax/plan/stub-routing checked here while
the first measured numerical evidence is carried unchanged.

## Test the frozen law on prospective C and D

The common scheduled law qualified the four original cases. Two new reset poses were sealed before that experiment's brake result: C and D. This separate validation keeps the gains, interpolation knots, F64 operation order, step, reference displacement and goals fixed. It does not expand the nine-pair search or select a different controller. Part A still has no feasible candidate.

The eight contexts are `context = pose*4 + scenario`: pose 0 is C, pose 1 is D; scenarios 0..3 are hold, nominal tracking, pulse and braking. C starts its brake with joint 7 at +2 rad/s; D uses −2 rad/s. The pulse is +3 J/rad on C and −3 on D, on joint 2 during control ticks 200..249. Every reset coordinate, reference endpoint and finger target comes from the sealed prospective descriptor. The common law always replaces joint 7 damping; no legacy brake-specific gain survives.

<!-- source: psionic/examples/panda-scheduled-pd-validation.coax psv-prospective -->
```jai
 for pose:0..<2 {
  // Context = pose*4 + scenario; brake is3, pulse is2, nominal is1.
  initial:[1]PntInitial;fixture:[64]f64;reference:[1]PsvReference;
  #assert pnt_ok(psv_initial(pose*4+3,initial[..]));
  #assert pnt_ok(psv_fixture(pose*4+2,fixture[..]));
  #assert pnt_ok(psv_reference(pose*4+1,135,reference[..]));
  print("pose %: reset q1 % micro-rad; brake v7 % micro-rad/s; pulse2 % micro-J/rad\n",pose,psv_example_micro(initial[0].q[0]),psv_example_micro(initial[0].v[6]),psv_example_micro(fixture[56]));
  print("midpoint: q1 % micro-rad; v1 % micro-rad/s; tick %/%\n",psv_example_micro(reference[0].reference.angle[0].value),psv_example_micro(reference[0].reference.rate[0].value),reference[0].reference_tick,reference[0].reference_steps);
 }
 reference:=[1]PsvReference.[psv_reference_poison()];old:=reference[0];status:=psv_reference(8,135,reference[..]);#assert status.stage==2 && status.code==1;psv_reference_same(reference[0],old);
 print("unknown context: refused; complete typed reference unchanged\n");
```

This pure example queries reset/reference metadata and an atomic refusal. It executes no controller, plant, SDK, rendering or neural model. Its transcript was written from the sealed factory and independent midpoint reference table before the first debug/release executions.

```shell
coax/bin/coax run psionic/examples/panda-scheduled-pd-validation.coax
COAX=coax/bin/coax psionic/tests/robotics/check-panda-scheduled-pd-validation.sh /tmp/new-panda-cd-cached
```

The new test-only Coax consumer owns the C/D factory, signed pulse replacement, command/step composition and score adaptation. It first checks every view shape, then context, then tick. The command snapshots all nine positions and rates, calls the unchanged common controller, and replaces D's external pulse **before** either publication or plant application. The owned plant applies each actuator force once; Pa's predicted force is a diagnostic, not another added force. Complete command and plant outputs publish together only after success; underlying stage and status remain distinct.

Scoring uses the current control reference at tick `k` and the post-state reference at `k+1`. Initial peak speeds and previous-velocity lineage come from each actual new reset, including D's negative brake velocity. Source margins include the initial state and every admitted post-state across all nine coordinates. The shared reduction helpers remain unchanged; the wrappers that embed the original pose are not used. Each four-case card carries its pose ID 0 or 1 in this new validation namespace. Those labels are not gain candidates. The ordered two-pose verdict requires both valid and feasible.

The goals remain fixed: every scenario must meet both position error ≤0.01 rad and rate error ≤0.05 rad/s throughout post-states 626..750. Hold also requires full-run peak position drift ≤0.005 rad. Nominal tracking must beat the stationary-reference comparator in SSE. Feasibility is separate from numerical parity, with no tolerance or goal relaxation after results.

The first canonical SDK capture and own fresh-feedback run each completed exactly 6,000 steps (eight resets × 750), once each. There is no shadow plant. Every fixed case is valid and feasible in both datasets; the ordered two-pose verdict is `[1,1]`. The terminal ratio is the maximum of `abs(q_error)/0.01` and `abs(rate_error)/0.05` over the frozen final 125 post-states; a ratio ≤1 passes.

| Pose | Scenario | SDK terminal ratio | Own terminal ratio | Controller-clipped coordinates | Goals |
| --- | --- | ---: | ---: | ---: | --- |
| C | hold | 0.0000868377 | 0.0000868377 | 0 | pass in both |
| C | nominal | 0.7936766163 | 0.7936766163 | 0 | pass in both |
| C | pulse | 0.7798300623 | 0.7798300623 | 0 | pass in both |
| C | brake | 0.2356208390 | 0.2356208390 | 34 | pass in both |
| D | hold | 0.0000424217 | 0.0000424217 | 0 | pass in both |
| D | nominal | 0.9710715211 | 0.9710715211 | 0 | pass in both |
| D | pulse | 0.9483775273 | 0.9483775273 | 0 | pass in both |
| D | brake | 0.2478364627 | 0.2478364627 | 35 | pass in both |

The closest case is D nominal: SDK ratio 0.9710715211, leaving 0.0289284789 normalized margin. This is a measured pass at two fixed poses and a fixed horizon, not a global stability certificate. All actuator-stage control/force clipping counters are zero; the brake has 34 controller-clipped coordinates on C and 35 on D. The nominal stationary-SSE comparator and hold-drift goals pass separately. No acceptance goal was changed after measurement.

Three cached host profiles agree byte for byte without repeating the plant runs. Status, clipping, reference and pulse metadata match exactly; all 12,000 schedule records match the independent law and there are zero cross-stream regime differences. SDK-versus-own differences are at most 4.441e−16 rad in arm position, 4.111e−15 rad/s in arm velocity and 6.040e−14 J/rad in control. The independent audit observes 210,000 SDK PD fields (maximum error 4.619e−14) and 54,000 actuator fields (numerically exact). Case metrics differ by at most 6.040e−14 within the frozen mixed budget; 27 trace and 11 score reader poisons are refused.

The C/D brake peak arm effective accelerations are 41.6233 and 41.0303 rad/s², measured as state `Δv/0.002`, not SDK `qacc`. Reference rate/acceleration caps do not bound those actual plant quantities. A terminal-window pass does not assert permanent settling, collision protection, or hardware behavior.


Sixteen new check groups close 263 actual emitted authority sites: 131 control, 45 score and 87 public composition/verdict, all killed. Two new compiler witnesses refuse returning a reference angle as a rate or a rate as acceleration. The first observational and compile-only failures are retained. One isolated successful D-negative-pulse VM step qualifies new wiring; it is separate from the canonical native experiment counters. The unchanged scalar law and older arithmetic gates are carried, not repeated. These counts do not claim full Quantity-token mutation coverage or compiler-enforced bounded control.

The raw state, fixed factory/reference and parametric metric helpers remain source-visible and unsealed. Their preconditions require admitted context/tick, finite source-profile state, coherent initial/reference data and the documented metric domains. Case/candidate PODs are observations, not trajectory or control-authority certificates. This component concerns the fixed unconstrained control/physics profile. It adds no contacts, collision protection, generic simulator, NN policy, hardware result or nonlinear stability proof.

Detailed inputs and evidence: [validation fixture](../fixtures/robotics/panda-scheduled-pd-validation/README.md).

## The remaining control and planning layers

The following sections are design work; the position and force queries above
do not qualify these complete control and planning tools.


- Broader nonlinear gain, disturbance and saturation acceptance beyond the
  original cases and the two frozen C/D poses. Those prospective cases now
  pass the fixed no-contact SDK/own profile; the baseline misses and nine-pair
  no-winner result remain historical evidence. This does not qualify arbitrary
  geometry, contacts, stability or hardware.
- Finite, range-preserving nullspace steps and their integration with pose IK,
  beyond the delivered linearized posture projection.
- Sampling-based motion planning with RRT-Connect, and path shortcutting.
- Tracking timed edges under the declared plant, delay and fault profiles;
  moving-state retargeting, non-stopping blends and jerk constraints. The
  rest-to-rest numerical reference above does not supply those behaviors.

## Planned control and planning math

The typed PD query supplies this force equation. The isolated rotor above
qualifies its sampled constant-inertia response. The four nonlinear cases use
the owned coupled dynamics at a declared 0.002-second step. The bounded gain
study above reports no feasible family, while the separate common schedule
passes the original-case goal set and a separate two-pose C/D validation with
no-contact SDK/own parity. Broader gain/tracking acceptance remains work:

```jai
τ = K_p (q_ref − q) + K_d (q̇_ref − q̇) + g(q)
```

The primary damped IK update and linearized posture projection are delivered
separately. A finite, range-preserving combination with controlled task drift
remains planned:

```jai
Δq_primary = Jᵀ (J Jᵀ + λ² I)⁻¹ e
Δq_posture = n (nᵀ Δq_rest)  // J n = 0 in the normalized linear task
```

`I − J_damped# J` is not an exact nullspace projector.

([Buss, *Introduction to IK with Jacobian Transpose, Pseudoinverse and
Damped Least Squares*, 2004](https://www.math.ucsd.edu/~sbuss/ResearchWeb/ikmethods/iksurvey.pdf)).
RRT-Connect grows trees from start and goal and tries to connect them,
checking each edge for collision at a stated resolution
([Kuffner and LaValle, ICRA 2000](https://doi.org/10.1109/ROBOT.2000.844730)).

## Planned integration in Psionic

The remaining control design calls for compiler-enforced Coax R14 bounded
steps: fixed rate, bounded computation, saturating outputs with reported
saturation. The position query above has an explicit iteration cap and typed
nonconvergence, while compiler enforcement remains pending. The planner uses Aethyr chapter 20 and 21's queries for collision checks
in a single-environment scene copy, seeded from the counter-based RNG so a
plan is reproducible. A rest-to-rest timing query is available above;
connecting certified edges, shortcutting and meeting declared tracking/recovery
goals remain planned.

## Planned additional modules

- `psionic/lib/robotics/control.coax` (broader control profiles and IK), `plan.coax` (RRT-Connect,
  shortcutting and timed-reference integration).

## Remaining acceptance checks and oracles

- Broader nonlinear PD holding/tracking acceptance under declared disturbance
  and saturation profiles. The measured four-case hold passes its drift goal,
  while its baseline nominal final error and brake recovery miss their goals.
  The fixed nine-profile study still has no feasible family. The separate common
  schedule passes all four original cases and all eight fixed C/D cases, with
  no-contact SDK/own parity. Broader geometry, contact and fault acceptance
  remain work. The isolated rotor/Jury result remains separate evidence.
- Finite nullspace-step policy and pose-IK integration, plus broader behavior
  near singularities and the principal cut. The delivered posture query has
  explicit numerical rank/residual refusal; it does not qualify a finite motion.
  Keep typed local nonconvergence distinct from global unreachability. The
  fixed-source position and pose profiles already enforce source ranges and
  their separate convergence criteria.
- Planned edges retain fixed-scene conservative clearance evidence; a 10×
  denser recheck is an additional oracle. The same seed gives the same plan.
  Follow the [collision-to-planning sequence](../docs/robotics/panda-motion-planning.md)
  before qualifying RRT.
- The rest-to-rest reference meets its declared velocity/acceleration profile
  under the stated numerical assumptions. The four cases begin physical tracking
  measurement; broader limit, timing-delay and fault-recovery acceptance,
  moving-state blends and jerk-bounded profiles remain future work.

## How Genesis does it

Genesis provides PD control, inverse kinematics and RRT-Connect planning
through OMPL-style interfaces on its rigid entities
([control your robot](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/control_your_robot.html),
[IK and motion planning](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/inverse_kinematics_motion_planning.html)).
We plan the same three tools, written in Coax with bounded control steps
and reproducible planning.

## Dependencies

The runnable profiles depend on the owned fixed Panda FK/grip Jacobian and
sixteen-body gravity, checked small matrix algebra, native F64 and the initial
typed quantity/point profile. The remaining control/planning design depends on
Aethyr chapters 19 to 22 and 35, full frame transforms and compiler R14
bounded control. Those requirements are not waived by fixed arrays.
