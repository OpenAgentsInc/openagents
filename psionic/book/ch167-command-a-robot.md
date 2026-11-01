# 167. Command a robot

This chapter builds position-only inverse kinematics, principal orientation
error and one pose update, seven-joint PD with gravity compensation, and a
short coupled run on our owned Panda dynamics.
The queries return source-limited coordinates or explicit force terms. The
250-step experiment then recomputes the PD command on every owned state and
compares that trajectory with one MuJoCo reference. General stability and
the broader planning tools remain work.

The separate [chapter 170](ch170-close-the-simulated-control-loop.md) already
runs operational-space control against MuJoCo. Holding/tracking experiments,
complete pose/nullspace IK, collision-free planning, trajectory timing and
compiler-enforced bounded control retain their separate acceptance criteria.

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
establish general cross-target reproducibility. A complete pose solve,
nullspace behavior, collision admission, RRT and compiler `#bounded` control
remain separate work.

The [component record](../fixtures/robotics/panda-pose-ik/README.md) links
the unchanged targets, pre-comparison correction, independent named-grid check,
source-restore note and separate numerical/native contexts.


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
steady-state or stability threshold. Contact response, large disturbances,
gain analysis, timed trajectories and whole-policy success are still open.

The [complete evidence](../fixtures/robotics/panda-joint-pd-prefix/README.md)
retains the first generated-C printer-warning build failure, before any SDK
load, and its narrow build-only followup. Passed emissions and decoder guards
were reused; the SDK prefix and three numerical profiles ran once. The example's
[transcript](../examples/panda-joint-pd-rollout.out) was written from the SDK
endpoint before its first debug/release execution. Historical PD and Euler
mutation scopes are carried separately on unchanged product files; this
composition adds no new admission rule or compiler boundedness claim.

## The remaining control and planning layers

The following sections are design work; the position and force queries above
do not qualify these complete control and planning tools.


- Measured joint-space PD holding/tracking, gain analysis and saturation behavior.
- Inverse kinematics by damped least squares, with joint limits and
  null-space objectives.
- Sampling-based motion planning with RRT-Connect, and path shortcutting.
- Trajectory timing within velocity and acceleration limits.

## Planned control and planning math

The typed PD query above supplies this force equation. Its remaining response
and gain analysis needs the dynamics and a declared timestep:

```jai
τ = K_p (q_ref − q) + K_d (q̇_ref − q̇) + g(q)
```

Damped least squares IK for an end-effector error `e` with Jacobian `J`:

```jai
Δq = Jᵀ (J Jᵀ + λ² I)⁻¹ e + (I − J⁺ J) Δq_null
```

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
plan is reproducible; shortcutting and time parameterization follow.

## Planned additional modules

- `psionic/lib/robotics/control.coax` (broader control profiles and IK), `plan.coax` (RRT-Connect,
  shortcutting, timing).

## Remaining acceptance checks and oracles

- PD holds a pose under gravity with steady-state error below a stated bound;
  step response matches the second-order model of its gains on a single
  joint.
- Broader IK reaches held-out position/orientation targets within declared
  tolerances; exhausted iterations return typed nonconvergence, preserving
  the distinction between local failure and global unreachability. Joint
  ranges remain enforced.
- Planned paths are collision-free when re-checked at 10× resolution;
  the same seed gives the same plan.
- Timed trajectories never exceed velocity and acceleration limits.

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
