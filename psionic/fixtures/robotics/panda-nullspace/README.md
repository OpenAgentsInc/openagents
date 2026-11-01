# Source-bound linearized posture nullspace

The public query accepts seven q angles and seven rest angles, all typed radians
within the pinned source joint limits. It owns fresh FK and the fixed grip-SITE
Jacobian. It returns a projected delta only, not a new pose or command. Existing
Ppo/Pps/kinematic/physics source files remain unchanged.

Linear Jacobian derivatives times one radian become metres. Angular derivatives
times one radian times 0.25 m/rad become metres. Divide every task row by one
explicit metre, and normalize rest-q by one explicit radian. The resulting A6x7,
n and d are dimensionless; p=n*(n^T*d) is restored to typed projected radians.
Physical task_leak[0:3] is linearized displacement in metres; task_leak[3:6] is
weighted angular displacement in metres, not raw angular error.

Signed 6x6 column minors use partial-pivot LU, with exact-zero pivots producing
zero minors. Row-max scaling is arithmetic conditioning only. The normalized
volume is restored as product(row_scales)*norm(scaled_cofactors), equivalent to
the product of six singular values in exact arithmetic. It is NOT sigma_min or
a general conditioning certificate. The numerical profile requires gamma>2^-24.
The direction's largest-absolute component is positive, with earliest tie; exact
zero direction and projected components are canonical +0. Independent comparison
uses the line projector n*n^T and projected delta, avoiding a spurious sign issue.

Residual admission is |dot(n,n)-1|<=2e-12, max|A*n|<=2e-10, and
max|A*p|<=2e-10*max(1,||d||2). The four-word residual helper checks finite and
nonnegative values PER FIELD, then restNorm<=16, then policy thresholds. An
earlier negative field wins over a later NaN. It does not perform an all-finite
pass before every range check. Norm here is squared-norm error; restNorm is
Euclidean sqrt(sum d²).

Shapes precede q then rest admission in index order. Every public refusal leaves
the complete old output unchanged. Raw bridges may stage scalar copies before
delegated numeric admission. Owned projection helpers trust finite |A|<=4 and
|d|<=6. The diagnostic pns_measure additionally trusts finite |n|/|p|<=16 and
0<=volume<=1e16; it intentionally measures non-null vectors so real norm/leak
refusals can be observed. These helper PODs are unsealed numerical staging,
not forged-input admission certificates. Test-only math/residual probes have
explicit shape/domain checks and do not replace the actual source FK/J path.

q/rest admission does not imply q+p stays in the source box. Frozen case 20 has
joint 2 q=-1.7578 rad, projectedDelta=-.06625616678890314 rad; their hypothetical sum
is -1.8240561667889033 rad, about .061256 rad below its -1.7628 rad limit. This is cached
reference arithmetic, not a newly applied pose. Coordinate clipping also destroys
nullness; finite-pose, limit-step, collision, dynamics, trajectory and actuation
policy remain separate.

The frozen new corpus has 48 physical queries (16 configurations x 3 rest vectors)
and 24 synthetic cases. All 48 legal physical cases pass this volume/residual
profile; none of the axis-aligned candidates is incorrectly called singular.
Fourteen synthetic cases pass and ten are rank-refused. Nine restored-volume
threshold cases cover below/equal/above in first-row scale families (1, 2, .5);
the products of row maxima are (1, 2, .5), respectively, while the global maximum coefficients are (1, 2, 1). Exact-Fraction Gram
arithmetic and 80-digit Decimal Gaussian projection provide the independent
reference, with no cofactor oracle or damping. The all-zero arm is not a legal
public source-limit fixture because joint 4=0 is outside its allowed interval.

Thirteen new source groups, four meaningful unit refusals, and 209/65/42 actual
emitted sites across math/public/test-bridge roots are qualified with zero
survivors. Initial math and bridge survivors are preserved, followed by signed
cofactor/dense-LU/independent diagnostic and transport-boundary observations.
No equivalence waiver, input or budget tuning was used. Counts do not imply
complete authored quantity-token provenance coverage.

The first debug, release and fail-fast ASan/UBSan profiles each pass 9,760 assertions. All 72 complete records match byte for byte across modes: 62 successful payloads and 10 unavailable rank markers. Maximum null-line projector discrepancy is 2.082e-15; projected-angle discrepancy is 9.881e-15 rad. Maximum measured null residual is 3.919e-16, scaled posture residual 3.369e-16, and volume discrepancy 2.220e-16. Direct projector idempotence error is 2.22e-16, trace error 4.44e-16, symmetry error 0, and delta-versus-P*d error 1.02e-16. Exact restored-threshold bits, canonical ties/+0, thirteen overlap cases, fifty-three residual probes and twenty-six poisoned-reader guards pass.

Frozen comparison budgets: projected radians and line projector 2e-8 absolute;
squared norm 2e-12; projector symmetry/idempotence/trace 2e-10; normalized volume
2e-12+2e-10*abs(reference). Rank labels, statuses, literal canonical rules and mode
bytes have exact criteria. The independently prewritten example passes its own
display check plus debug/release output. No SDK, neural, renderer, old numeric
corpus or old mutation gate is repeated. The numerical profile is scalar-libm/F64,
not compiler #bounded/R14, cross-machine or hardware qualification.
