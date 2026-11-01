# 171. Learn a gait in many worlds

The first runnable profile is CPU F64 return-target and clipped-objective math. It does not train a policy, simulate a quadruped or satisfy the full gait acceptance criteria. This chapter builds those foundations before attempting the full learner.

The eventual workload is a quadruped practising across many randomized simulated worlds, followed by held-out terrain and teacher/student evaluation. Environments, Gaussian policies, optimizer updates, device batching and transfer remain design work below. A return array shaped like several worlds is not evidence that those physical worlds exist.

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

The [foundation fixtures](../fixtures/robotics/locomotion/README.md) retain input, source, reference, native, typing and transcript identities. No old optimizer, tape, model, SDK or robot gate is rerun. This CPU foundation supplies no environment, Gaussian sampler, optimizer update, training result, batched physics, gait, domain randomization, device throughput, teacher/student transfer or hardware evidence. Low-level helpers and PODs retain numeric/provenance preconditions.

### Next steps remain explicit

First qualify distribution/log-density and ratio adapters, then policy/critic update gradients and a frozen small LQR learning experiment against its known optimum. Keep score-function policy gradients distinct from differentiating through the environment. A serial CPU return batch does not supply Aethyr's quadruped contacts, sensors, reset/RNG independence or device batching. Those foundations, the full four chapter acceptance criteria and compiler R14/R20/R21/R23 remain later work.

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
