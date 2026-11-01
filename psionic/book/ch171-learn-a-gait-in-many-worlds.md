# 171. Learn a gait in many worlds

The runnable CPU F64 profiles provide return targets, a clipped scalar objective and normalized Gaussian policy primitives. They do not train a policy, simulate a quadruped or satisfy the full gait acceptance criteria. This chapter builds those foundations before attempting the full learner.

The eventual workload is a quadruped practising across many randomized simulated worlds, followed by held-out terrain and teacher/student evaluation. Environments, policy/critic networks, optimizer updates, device batching and transfer remain design work below. A return array shaped like several worlds is not evidence that those physical worlds exist.

## First runnable slice: targets before a learner

The first executable part constructs generalized-advantage targets and the scalar clipped PPO objective in CPU F64. It does not yet train a policy or simulate a quadruped. The LQR learning, held-out gait, same-target learning replay and teacher/student acceptance criteria remain open.

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

Scalar distribution/log-density and ratio adapters are qualified below. Next come policy/critic update gradients and a frozen small LQR learning experiment against its known optimum. Keep score-function policy gradients distinct from differentiating through the environment. A serial CPU return batch does not supply Aethyr's quadruped contacts, sensors, reset/RNG independence or device batching. Those foundations, the full four chapter acceptance criteria and compiler R14/R20/R21/R23 remain later work.

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

The [Gaussian fixture](../fixtures/robotics/locomotion-gaussian/README.md) retains contract, input/reference, native, typing and example identities. Policy/critic update assembly and a frozen known-optimum LQR experiment still come next; walking, device batching, terrain, teacher/student and physical hardware remain later work.

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

- `psionic/lib/robotics/locomotion/`: environment, reward, PPO learner,
  terrain curriculum, teacher–student distillation.

## Remaining acceptance checks and oracles

- PPO on a small environment with a known optimum (a linear-quadratic
  regulator) converges to it within a stated gap.
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
