# 171. Learn a gait in many worlds

The runnable CPU F64 profiles provide return targets, a clipped scalar objective, Gaussian primitives and a projected scalar-LQR actor/update pipeline. Its mathematical qualification and prescribed three-seed execution are complete; all three runs miss both frozen convergence goals. These scalar profiles do not simulate a quadruped or satisfy the full gait acceptance criteria.

The eventual workload is a quadruped practising across many randomized simulated worlds, followed by held-out terrain and teacher/student evaluation. The scalar LQR environment and actor updates are implemented below. General neural actors, learned critics, physical worlds, device batching and transfer remain design work. A return array shaped like several worlds is not evidence that those physical worlds exist.

## First runnable slice: targets before a learner

The first executable part constructs generalized-advantage targets and the scalar clipped PPO objective in CPU F64. It does not yet train a policy or simulate a quadruped. The later scalar-LQR experiment has its own prospective learning/quality criteria; held-out gait, full-curve replay and teacher/student acceptance remain separate.

A rollout row supplies a reward, its current value estimate, the value of its actual next/final observation, and separate `terminated` and `truncated` flags. These are observations from the caller; this API cannot certify where the next value came from. `RgaReward` is a nominal normalized-reward wrapper. Gamma, lambda, epsilon and probability ratio use dimensionless quantities. An advantage is not implicitly a length or a raw probability ratio.

### Bootstrap and trace are different decisions

A true terminal state has no future task value. An external interruption of a continuing task still has a final-state value, but the advantage recurrence must not continue into an automatically reset episode. A collector tail also stops the recurrence without declaring environmental termination. A genuine finite-horizon task deadline belongs in `terminated`; it must not be indiscriminately bootstrapped as an external time cutoff.

```jai
bootstrap_t = !terminated_t;
trace_t = !(terminated_t || truncated_t || local_collector_last_row);
next_t = bootstrap_t ? gamma * next_value_t : 0;
delta_t = (reward_t + next_t) - current_value_t;
advantage_t = trace_t ? delta_t + (gamma * lambda) * advantage_next : delta_t;
lambda_return_t = advantage_t + current_value_t;
```

Both flags may be one; termination wins. Truncation and an ordinary nonterminal collector tail retain bootstrap. Their `next_value` must describe the actual final state before reset, not the reset observation. Every numeric field is admitted even when masked; a terminal flag is not permission to hide a NaN.

The implementation admits 1..16 world-major fragments with horizon 1..128 and at most 2,048 rows. The recurrence starts anew in each world. Shapes/axes are checked before multiplying dimensions; both hyperparameters must be finite and in [0,1], every reward/value/next-value must be finite and within ±1e6, and flags must be exactly zero or one. All admission completes before arithmetic. A private fixed stage holds every output until successful complete publication, including when caller views overlap. The 2,048-row storage bound is not a compiler R14 stack/scratch or no-blocking proof.

The CPU profile freezes each F64 rounding boundary: gamma×lambda once, conditional bootstrap multiplication, left-associated delta, one multiply/add per continuing recurrence, then advantage+current value. Contraction and fast reassociation are disabled. Per-world horizon, not total batch size, bounds the recurrence: admitted |advantage|≤3.84e8 and |lambda return|≤3.85e8. Later learner code must detach these targets and old-policy data.

This follows the residual-sum estimator in [Schulman et al., GAE, equation 16](https://arxiv.org/abs/1506.02438), with an explicit owned cutoff/transport profile. The distinction between a true finite horizon and an external continuing-task time limit follows [Pardo et al., 2018](https://proceedings.mlr.press/v80/pardo18a.html). Array limits, flag admission, staging and the precise F64 recipe are our contracts, not claims made by those papers.

### The clipped objective is sign-aware

Use `reward_t` for reward and `rho` for the probability ratio; they are different values. The scalar query computes:

```jai
lower = RN(1 - epsilon);
upper = RN(1 + epsilon);
clipped_rho = clamp(rho, lower, upper);
surrogate = min(rho * advantage, clipped_rho * advantage);
loss = -surrogate;
```

For a positive advantage, the clipped branch becomes active only above the upper ratio bound. For a negative advantage, it becomes active only below the lower bound. A ratio outside the interval is therefore not synonymous with an active clip. The profile admits rho in [2^-20,16], epsilon in [0,1] and |advantage|≤1e9, so every admitted GAE advantage fits without an extra normalization or clipping step.

The reported formal derivative with respect to the new log-probability is `-rho*advantage` on the unclipped branch and zero on the active clipped branch, assuming the old policy and advantage are detached. At an **exact rounded ratio knot**, the profile selects the unclipped derivative. Active clipping is decided from sign and ratio thresholds, never from rounded-product equality. Two products may round to identical words while the branch still differs. Zero advantage alone canonicalizes surrogate, loss and derivative to +0; other zero signs follow the frozen arithmetic.

This is a real-formal piecewise derivative, not a derivative of a quantized F64 output staircase. The query takes a direct ratio: no exp/log-probability adapter, network, gradient tape or optimizer has been added. It implements the per-sample objective from [Schulman et al., PPO, equation 7](https://arxiv.org/abs/1707.06347), without silently adding a batch mean, KL penalty, entropy, value loss or advantage whitening.

### A small synthetic example

The first fragment ends in a true terminal row. The second is truncated before a dramatically different reset row; it bootstraps the supplied final value without tracing through reset. Its collector tail also bootstraps. All expected transcript values were calculated before the first example execution.

<!-- source: psionic/examples/locomotion-targets.coax locomotion-targets -->
```jai
 steps:[4]RgaStep;
 steps[0].reward.value=1;steps[0].current_value.value=0.5;steps[0].next_value.value=1;
 steps[1].reward.value=2;steps[1].current_value.value=1;steps[1].next_value.value=100;steps[1].terminated=1;
 steps[2].reward.value=3;steps[2].next_value.value=4;steps[2].truncated=1;
 steps[3].reward.value= -100;steps[3].next_value.value=2;
 targets:[4]RgaOutput;
 #assert rga_returns_raw(2,2,0.5,0.5,steps[..],targets[..])==RgaStatus.ok;
 for i:0..<4 {
  print("row %: bootstrap %, trace %; delta %, advantage %, target % micro-reward\n",i,targets[i].bootstrap,targets[i].trace,locomotion_micro(targets[i].delta.value),locomotion_micro(targets[i].advantage.value),locomotion_micro(targets[i].lambda_return.value));
 }
 rho:rlu.Quantity(UnitOne,f64);epsilon:rlu.Quantity(UnitOne,f64);rho.value=1.5;epsilon.value=0.25;
 objective:[1]RcpOutput;
 #assert rcp_surrogate(rho,targets[0].advantage,epsilon,objective[..])==RcpStatus.ok;
 print("PPO ratio %; clipped ratio %; active %\n",locomotion_micro(rho.value),locomotion_micro(objective[0].clipped_ratio.value),objective[0].clipped_active);
 print("surrogate %, loss %, formal logp gradient % micro-reward\n",locomotion_micro(objective[0].surrogate.value),locomotion_micro(objective[0].loss.value),locomotion_micro(objective[0].dloss_dlogp.value));
```

```shell
coax/bin/coax run psionic/examples/locomotion-targets.coax
```

The first row's advantage is 1.25 reward units and its target is 1.75. With ratio 1.5 and epsilon 0.25, the positive-advantage clipped surrogate is 1.5625, loss is −1.5625 and the chosen formal derivative is zero. These are synthetic arithmetic fixtures, not rewards from a simulated policy.

### Evidence and limits

The two new authority roots qualify 16 groups and 156 actual emitted mutation sites (100 GAE, 56 PPO), with zero survivors or equivalent waivers. The public aggregate only loads those roots and has zero new body sites; there is no invented third mutation proof. Three new compiler witnesses reject a physical-time discount, a physical-length ratio and a dimensionless Quantity substituted for nominal Reward. The first native target is Darwin arm64 with gradual-underflow binary64; unsupported targets refuse this qualification. API quantities do not qualify general Quantity-operator provenance, which remains separate compiler work. Initial GAE observational survivors remain in the source history; matching dimension spans and raw-bit numeric predicates close them without changing the recurrence.

The first debug, release and fail-fast ASan+UBSan profiles each pass 22,545 assertions, including 374 refusal queries, 22 overlaps and 85 IEEE patterns. They consume 96 frozen GAE cases/5,023 rows and 1,108 scalar PPO cases; the complete result streams are byte-identical across all three modes. Twenty-eight new reader poisons are refused.

Every one of the 5,023 delta words and all 10,046 bootstrap/trace integers matches exactly. The 565 one-row segments also match advantage and target exactly, making 6,153 exact GAE F64 word comparisons in total. General longer-segment advantages/targets are compared with the independent future-sum oracle under their frozen forward-error bounds, not called bit-exact: maximum absolute errors are 1.283e−8 and 1.281e−8 reward units, using at most 3.761% and 4.279% of the corresponding per-row budgets. All 4,432 PPO F64 words and 1,108 active flags match the independent rounded oracle bit for bit, including 218 zero-advantage and 152 exact-knot cases.

The independent GAE oracle uses explicit future-residual sums/products, exact Fraction rounding or Decimal80, not the product's reverse recurrence. Every delta and mask is exact; the frozen per-row advancement budget scales with the effective local segment length and weighted absolute residual sum, including cancellation. All three fields are exact for a one-row segment. The PPO oracle uses exact rational products, the rounded clipping endpoints and the declared formal branch; all output words and active flags are exact. Every actual and expected word must be finite before error maxima are formed.

The [foundation fixtures](../fixtures/robotics/locomotion/README.md) retain input, source, reference, native, typing and transcript identities. No old optimizer, tape, model, SDK or robot gate is rerun. This return-target/objective profile itself supplies no environment, distribution sampler, optimizer update, training result, batched physics, gait, domain randomization, device throughput, teacher/student transfer or hardware evidence. The separate scalar distribution query below covers only Gaussian arithmetic. Low-level helpers and PODs retain numeric/provenance preconditions.

### Next steps remain explicit

Scalar distribution/log-density and ratio adapters are qualified below. The scalar actor/update and frozen LQR experiment appear below; its three-seed outcome remains pending. General neural actor/learned-critic training remains later work. Keep score-function policy gradients distinct from differentiating through the environment. A serial CPU return batch does not supply Aethyr's quadruped contacts, sensors, reset/RNG independence or device batching. Those foundations, the full four chapter acceptance criteria and compiler R14/R20/R21/R23 remain later work.

## A normalized scalar Gaussian policy primitive

The next runnable slice provides Gaussian log-density and analytic scores, an explicit seeded scalar transform, and an admitted log-ratio adapter to the existing clipped objective. It adds no policy/critic graph, optimizer, environment or training result. The earlier return-target/PPO profile and counter RNG remain unchanged.

`RgpAction` is a nominal **normalized scalar** action/mean. Log standard deviation and diagnostic fields use dimensionless quantities. This is not a distribution over physical metres, radians or actuator force. A physical action map needs its own coordinate and density Jacobian; changing a unit label alone does not supply it.

### Log-density and detached-action scores

With `sigma = exp(log_std)`, the implementation freezes this binary64 recipe:

```jai
inv_sigma = exp(-log_std);
d = action == mean ? +0 : RN(action - mean);
z = RN(d * inv_sigma);
z2 = RN(z * z);
logp = RN(RN(RN(-0.5 * z2) - log_std) - C);
score_mean = RN(z * inv_sigma);
score_logstd = RN(z2 - 1);
entropy = RN(log_std + RN(0.5 + C));
// C = 0.9189385332046728, the rounded real ln(2*pi)/2.
```

The four output fields are log-density, its formal mean/log-standard-deviation scores and differential entropy. At equal action/mean, the mean score is exactly +0 and the log-standard-deviation score is −1. These are smooth-formula derivatives with the sampled action detached, not derivatives of the rounded program or a path through the plant. Log-density may be positive and differential entropy may be negative; they are not probability mass at a point.

All inputs must be finite before bounds: mean lies in [−16,16], density action in [−128,128], and log standard deviation in [−4,2]. Views have exactly one output slot and refused calls preserve it completely. These are normalized numerical domains, not limits for a physical robot.

### Two counter draws, one finite-grid scalar

The pure uniform transform admits binary64 uniforms in [0,1). It computes the rounded angle and radial transform, then a scalar Box–Muller value:

```jai
w = RN(1 - U_radial);
theta = RN(6.283185307179586 * U_angle);
radius = sqrt(RN(-2 * log(w)));
z = RN(radius * cos(theta));
action = RN(mean + RN(exp(log_std) * z));
```

Only an input radial uniform numerically equal to ±0 selects the explicit zero branch: action copies the mean's bytes, including negative zero, and z is +0. All inputs are still admitted first. Tiny **nonzero** uniforms for which `RN(1-U)==1` follow the nonzero sqrt(−0)/cos recipe; they are not silently canonicalized. Cosine is evaluated at the actual rounded angle. A quarter-turn input does not imply that the backend's `cos(rounded_pi/2)` is exactly zero.

The seeded API accepts `CrSeed` by value, checks mean/log-standard-deviation admission and then reserves room for exactly two existing counter draws. It refuses counters above UINT64_MAX−2. First draw supplies the radial uniform, second supplies the angle; value, standardized value and second.next publish together. The final counter may equal UINT64_MAX. No spare variate, retry, global state or partial RNG advance is hidden in the API.

The **seeded** transform uses the existing top53 grid; the direct uniform API accepts the broader binary64 interval. The grid gives `w >= 2^-53` and a finite tail. Parameter bounds keep every generated action inside the density action box, so no sample clamp is necessary. This is a seeded numerical approximation, not an exact continuous Gaussian law or an independence-quality proof. It is also separate from physical saturation.

### Admit a log-ratio without hiding its rounding

`rlr_surrogate` accepts finite new/old log-density values, a detached nominal Reward advantage and dimensionless epsilon. It does not cap either individual log-density; equal very large finite values may still give a useful zero difference. After advantage/epsilon admission it subtracts once in F64. Overflow or a difference outside the fixed inward log interval refuses before exp.

The interval is the smallest binary64 at or above exact ln(2^-20), through the largest binary64 at or below exact ln(16):

```jai
lower_log_ratio_bits = 0xc02bb9d3beb8c86b;
upper_log_ratio_bits = 0x40062e42fefa39ef;
```

This is an inward **log-domain** profile, not every rounded preimage accepted by the direct-ratio API. The adapter computes backend exp, admits the actual finite rho in [2^-20,16], and never clamps it. Only then does it call the unchanged clipped objective with that rho. Exact ratio knots and its formal derivative convention refer to the realized binary64 ratio.

The very small real inward margin at the lower endpoint does not promise a correctly rounded backend exp. An independently bounded libm result just below the direct-ratio floor must produce range refusal. The numerical budget is not permission to enlarge the public domain.

### A pure example

This example queries a density at its mean, a deterministic uniform fixture, an actual two-draw zero-radius seed and a scalar log-ratio. Its transcript was prewritten before either example execution; it does not collect rewards or train a network.

<!-- source: psionic/examples/locomotion-gaussian.coax locomotion-gaussian -->
```jai
 density:[1]RgpDensity;
 #assert rgp_density_raw(2,0,2,density[..])==RgpStatus.ok;
 print("density at mean: logp %, mean score %, logstd score %, entropy % micro-units\n",locomotion_gaussian_micro(density[0].logp.value),locomotion_gaussian_micro(density[0].score_mean.value),locomotion_gaussian_micro(density[0].score_logstd.value),locomotion_gaussian_micro(density[0].entropy.value));
 value:[1]RgpValue;#assert rgp_from_uniforms_raw(2,0,0.5,0,value[..])==RgpStatus.ok;
 print("uniform fixture: action %, standardized % micro-units\n",locomotion_gaussian_micro(value[0].action.value),locomotion_gaussian_micro(value[0].z.value));
 draw:[1]RgpDraw;negative_zero:=f64_from_bits(0x8000000000000000);
 // This seed's first qualified counter draw has radial uniform exactly zero.
 #assert rgp_sample_raw(CrSeed.{seed=0x61c8864680b583eb},negative_zero,2,draw[..])==RgpStatus.ok;
 print("seeded zero radius: mean sign preserved %, z bits %, next counter %\n",(ifx f64_bits(draw[0].action.value)==f64_bits(negative_zero) then 1 else 0),f64_bits(draw[0].z.value),draw[0].next.counter);
 objective:[1]RlrOutput;#assert rlr_surrogate_raw(1,0,2,0.25,objective[..])==RlrStatus.ok;
 print("log-ratio fixture: logratio %, ratio %, clipped ratio %, active %\n",locomotion_gaussian_micro(objective[0].log_ratio.value),locomotion_gaussian_micro(objective[0].ratio.value),locomotion_gaussian_micro(objective[0].objective.clipped_ratio.value),objective[0].objective.clipped_active);
 print("loss %, formal logp gradient % micro-reward\n",locomotion_gaussian_micro(objective[0].objective.loss.value),locomotion_gaussian_micro(objective[0].objective.dloss_dlogp.value));
```

```shell
coax/bin/coax run psionic/examples/locomotion-gaussian.coax
```

### Evidence and remaining work

The three new public math roots close 130 actual emitted sites: density 42, sampling 48 and ratio 40, with 15 source groups and zero survivors. A separate checked test-only exp transport closes seven admission sites in one group. It is not a fourth Gaussian policy proof or an exp oracle. The new load-only aggregate has zero body sites and no invented mutation run. Four compiler witnesses reject incorrect physical units or normalized action/reward roles. General Quantity-operator provenance and compiler R14 remain separate.

The first debug, release and fail-fast ASan+UBSan host profiles each pass 32,579 assertions, including 250 dedicated refusals, 56 scalar-by-value overlap observations and 65 IEEE patterns. The 960 density, 1,344 pure-uniform, 160 seeded and 640 ratio records form complete byte-identical streams across all three modes. Fifty-nine new reader poisons are rejected. Seeded rows comprise 112 admitted samples and 48 exhausted counters; ratio rows comprise 536 admitted and 104 range refusals.

The maximum density error is 7.451e−9 on large-magnitude fields and consumes at most 5.322% of the applicable frozen mixed budget; entropy is exact. Sample action/z errors are at most 1.777e−15 and 2.221e−16, using at most 1.839% of their budgets. The declared exact comparisons cover 1,344 density fields, 1,280 pure-uniform fields and six seeded fields; all seed/counter transitions and 192 density zero-sign checks are exact. Every actual/expected comparison field is checked finite before a maximum is formed.

All 536 eligible observed exp values match the independently Decimal-rounded reference in this corpus. Their exact conditional ratio/objective composition then passes. This corpus result is not a general correctly-rounded-libm proof and does not remove the public actual-rho admission at the inward endpoints.

The independent reference uses high-precision Decimal exp/log/sqrt and independently reduced cosine, with exact Fraction/RN local operations. The native test transport observes exp only for frozen eligible log differences. Its rho is first compared to the independent high-precision exp bound. Exact ratio-domain predicates and all clipped-objective words are then derived conditionally from that **observed, independently bounded** rho. Successful public rho must match probe bytes. This separates exp validation from exact objective composition; it does not validate exp by comparing it with itself.

The first native target is Darwin arm64 scalar libm, round-to-nearest with gradual underflow, no contraction/fast math and explicit scalar exp/log/sqrt/cos calls. This is neither an owned-libm accuracy proof nor cross-target determinism. Density and sample smooth fields use frozen numerical budgets; entropy, density-at-mean, explicit radial-zero copies, counters and declared branch/zero cases are exact. No old RNG, GAE/PPO, tape, model or robot gate is repeated.

The [Gaussian fixture](../fixtures/robotics/locomotion-gaussian/README.md) retains contract, input/reference, native, typing and example identities. The additive scalar-LQR actor/update component follows below, with its three-seed outcome pending. General neural actors, learned critics, walking, device batching, terrain, teacher/student and physical hardware remain later work.

## A projected scalar-LQR PPO actor

This component connects the already qualified scalar Gaussian policy, GAE, reverse tape and AdamW to a normalized scalar environment. Only one actor gain K is trainable; variance is fixed at sigma=.25. There is no learned critic, neural policy, quadruped, SDK or hardware in this experiment.

The plant is x_next=x+u and the reward is minus(x²+u²), with policy mean −K*x. All observed states/actions and the sampled standardized value have explicit bounds; observations are refused rather than silently clamped. Each run has 16 independent world records, with explicit disjoint counter ranges. Fragments are 32 steps long and their actual final states bootstrap values. Worlds and counters persist between fragments. Initial states are four consecutive copies each of −1,−.25,.25,1. Seeds 0,1,7 and all hyperparameters are fixed before learning.

At rollout-start K_old, a declared continuous-Gaussian moment model supplies a nominal quadratic critic. It is not an exact value function for the finite-top53 sampler and is not fitted from the optimum. Observations/actions, old logp, nominal critic, advantages, returns and K_old are detached for all four full-batch epochs. No entropy bonus, whitening, value loss or plant gradient is added.

### Local VJP, full-batch update and projection

Each epoch uses eight contiguous 64-row tape blocks for 512 samples. A block graph has exactly 7+13*N nodes,839 at 64 rows, and ONE parameter. Each sample's coefficient is the existing sign-aware clipped objective's dloss/dlogp, detached at the current actor. The graph's scalar is sum(coefficient*logp), a LOCAL VJP proxy. Its value is not the PPO loss; the actual clipped losses are reported separately. Every graph logp word must match the existing Gaussian density before backward. An unexpected tape or word-coherence failure is a fatal internal invariant before publication, not a policy fallback.

Raw gradient and actual-loss sums are combined in fixed block order and divided once by sample count. The named experiment always uses 512; small mathematical queries permit 1..512 and partial final blocks. Exactly one binary64 AdamW step follows each full epoch, with learning rate .001, betas(.9,.999),eps1e−8 and zero decay. Proposed moments and step are retained. Only K is projected into the global [.25,1] box intersected with rounded K_old±1/256, anchored to the SAME rollout-start gain throughout all four epochs. The optimizer has one persistent K/master value; the raw candidate and projection count remain visible.

The source-owned batch function performs the rollout and all FOUR epoch transitions. C handles chunk/checkpoint/file effects. This explicit runtime/work profile is not compiler #bounded, R14 or a general training framework. Lower helpers and PODs have numeric/shape/lineage preconditions and are not provenance certificates.

### An independent quality lane

A separate scoring authority root computes the nominal optimum, stable gap and deterministic held-out checks. It is not imported or called by learner/types/fixtures, so Kstar/gap never select or stop an update. The six held-out initial starts differ from training starts. Nominal noisy-policy J, deterministic mean-policy finite trajectories plus analytic tail, and sampled training observations are distinct quantities.

All three prescribed seeds must finish 128 batches/512 updates with no skips or numeric refusals. Prospective gain error is at most .05 and nominal held-out cost gap at most1% on all six starts. All final seeds are reported; no best seed/checkpoint is promoted. A checkpoint after 64 batches replays only its next complete batch, with separate workload counts; this is not a repeated whole learning curve or full R16 qualification.

<!-- source: psionic/examples/locomotion-lqr.coax locomotion-lqr -->
```jai
 density:[1]RgpDensity;#assert rgp_density_raw(-0.5,-1.3862943611198906,0,density[..])==RgpStatus.ok;
 rows:=[1]RlpRow.[RlpRow.{x=1,action=0,old_logp=density[0].logp.value,advantage=2}];block:[1]RlpBlock;#assert rlp_block(0.5,rows[..],block[..])==RlpStatus.ok;
 print("actual PPO loss %, K gradient %, tape nodes %\n",lqr_micro(block[0].actual_loss_sum),lqr_micro(block[0].gradient_sum),block[0].nodes);
 epoch:[1]RltEpoch;#assert rlt_epoch(0.5,RltOptimizer.{K=0.5},rows[..],epoch[..])==RlpStatus.ok;
 print("one-row mathematical update: K %, Adam step %, projected %\n",lqr_micro(epoch[0].next.K),epoch[0].next.adam.step,epoch[0].projection);
 evaluation:[1]RlsEvaluation;#assert rls_evaluate(0.5,2,1,evaluation[..])==RlpStatus.ok;
 print("one deterministic step: finite cost %, nonzero tail %, total %\n",lqr_micro(evaluation[0].finite_cost),lqr_micro(evaluation[0].tail),lqr_micro(evaluation[0].result));
 state:[1]RltState;#assert rlt_begin(0,state[..])==RlpStatus.ok;
 print("untrained initial K %, batch %, last-world counter %\n",lqr_micro(state[0].optimizer.K),state[0].batch,state[0].worlds[15].rng.counter);
```

The prewritten tiny transcript has loss −2 and K-gradient 16 for the declared detached one-row fixture; its first mathematical Adam step displays K=.499000. A one-step mean-policy check displays finite cost .703125 and a nonzero tail .219006. The initializer remains untrained. These examples do not call the full learner or establish convergence.

The four new policy roots close 554 actual emitted sites in 21 groups: actor 158, rollout 111, update 188 and score 97, all zero survivors. A separate checked coefficient transport closes 23 sites in two groups; it is a test transport, not a fifth learner proof. Four compiler-only record/normalized-raw-boundary witnesses refuse. General physical dimensions, compiler R14 and a generic learner remain separate.

The first debug, release and fail-fast ASan+UBSan mathematical profiles each pass 229,063 assertions, 177 refusals, 12 overlaps and 83 reader poisons; complete output streams are byte-identical. The corpus contains 112 actor blocks and 24 epochs. Gradient absolute error is at most 2.911e−10 under the FROZEN per-block conditioning bounds; loss/proxy errors are at most 2.138e−11/5.821e−11. Epoch loss/gradient and raw-candidate errors are numerically zero in this corpus. The coefficient layer is independently validated before analytic local-VJP comparison; observed score/gradient fields are not used as their own oracle.

Each mathematical host run executes 597 corpus rollout steps plus 13 guard rollout steps, 798 short deterministic evaluation steps and other declared scalar/epoch guard queries. It executes ZERO successful full training batches. Those fixtures are separate from canonical learning, checkpoint replay and the later 3,072-step named evaluation. Ordinary critic/transition/projection comparison error is zero; short evaluation error is at most 3.553e−15 and rollout error 1.777e−15. This is the qualified Darwin arm64 binary64 scalar-libm/no-FMA profile, not cross-target determinism or an owned-libm proof.

The fixed experiment completed all 128 batches and 512 AdamW updates per seed. All 31 execution/audit actions passed, including byte-exact next-batch checkpoint replay. **All three seeds miss both prospective convergence goals.** No seed is promoted and no setting is retuned.

| Seed | Final K | Absolute gain error | Worst nominal held-out cost gap | Result |
| --- | ---: | ---: | ---: | --- |
| 0 | 0.5184256094 | 0.085306525 | 1.404964% | Both goals missed |
| 1 | 0.5227558679 | 0.080976266 | 1.259577% | Both goals missed |
| 7 | 0.5214162090 | 0.082315925 | 1.303622% | Both goals missed |

The nominal optimum is K*=0.6037321344. The frozen limits are gain error at most .05 and cost gap at most 1%. The initial K=.25 has gain error .3537321344 and worst nominal gap 40.451147%; the learned gains improve those measures substantially, but improvement does not satisfy the acceptance rule. The independent real-arithmetic margins also reject all three seeds. Rounded table values are explanatory; the original receipts retain full precision.

Canonical learning executes 196,608 plant steps, 1,536 AdamW updates, 12,288 tape forward/backward pairs and 786,432 sample-gradient evaluations. Declared checkpoint replay adds 1,536 steps/12 updates/96 tape pairs; four mean-policy evaluations add 3,072 deterministic steps. The summed action time is about 208 seconds on this named host, not a GPU throughput or end-to-end engineering-time measurement. The [LQR fixture](../fixtures/robotics/locomotion-lqr/README.md) retains the full outcomes and compact curves.

This closes the mathematical and experiment-integrity slice. The LQR convergence criterion remains **open**. A future separately frozen experiment may study optimizer/update budget; extending this profile would also require revisiting its state/counter bounds. The current result is retained without extra training or in-place tuning.

This measured scalar experiment supplies no learned critic, terrain, sensors, contact-rich locomotion, teacher/student transfer, device throughput or hardware evidence. Full gait acceptance remains separate.

## What the full chapter will teach

- Locomotion as a reinforcement learning problem: observations, actions as
  joint targets, rewards, terminations.
- Proximal policy optimization (PPO) with generalized advantage estimation.
- Domain randomization and terrain curricula.
- Teacher–student training: a policy with privileged state teaches one that
  sees only sensors.

## Math of the planned learner

PPO maximizes the clipped surrogate

```jai
rho_t(theta) = pi_theta(action_t | state_t) / pi_old(action_t | state_t);
L_clip(theta) = E_t[min(rho_t(theta) * A_t,
    clip(rho_t(theta), 1 - epsilon, 1 + epsilon) * A_t)];
// reward_t is a reward; rho_t is a probability ratio.
// A_t uses the separate bootstrap/trace rules implemented above.
```

([Schulman et al., 2017](https://arxiv.org/abs/1707.06347);
[Schulman et al., GAE, 2016](https://arxiv.org/abs/1506.02438)). Massively
parallel PPO for legged robots trains a walking policy in minutes on one GPU
([Rudin et al., CoRL 2021](https://arxiv.org/abs/2109.11978)).

## The planned learner in Psionic

The planned environment is Aethyr chapter 31's batch of quadrupeds on randomized terrain,
with Aethyr chapter 33's sensors and chapter 167's PD targets as the action
interface. The planned learner uses Psionic's own training stack (autodiff, the
optimizer chapters) and reads observations without host copies (Coax R23).
The full design requires every randomized parameter to be declared with its range and provenance (R20).
Planned training runs are recordings in Aethyr chapter 36's sense: seed, scene, profile
and learner configuration, so a policy names exactly how it was made.
Real hardware remains a separate, authorized integration.

## What the full learner still needs

- `psionic/lib/robotics/locomotion/`: a physical quadruped environment/reward,
  general neural actor and learned-critic training, terrain curriculum and
  teacher–student distillation. The scalar LQR pipeline does not supply those.

## Remaining acceptance checks and oracles

- The frozen scalar-LQR PPO actor completes all three prescribed seeds and
  meets its declared optimum gaps: mathematical fixtures pass; learning
  outcomes remain pending.
- The quadruped walks at a commanded speed with stated tracking error on
  held-out terrain and randomized parameters.
- The same seed and configuration reproduce the same learning curve on the
  same target.
- The student matches the teacher's return within a stated fraction.

## How Genesis does it

Genesis's locomotion examples train quadrupeds and humanoids with PPO in
thousands of parallel environments using rsl_rl
([locomotion](https://genesis-world.readthedocs.io/en/latest/user_guide/getting_started/locomotion.html)).
We plan the same workload on our own learner, with reproducible runs and
declared randomization.

## Dependencies

Chapters Aethyr 31, 33, 35, Psionic 167 and Psionic's training chapters. From Coax:
R14, R20, R21, R23.
