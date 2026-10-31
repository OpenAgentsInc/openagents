# 192. Learn a gait in many worlds

> **Status: unimplemented — design draft.** Nothing here runs yet.

A quadruped can learn to walk over rough ground in minutes of wall time if
it practises in thousands of simulated worlds at once, each with slightly
different friction, mass and terrain, so that what it learns transfers to a
real robot it has never touched. This chapter plans that loop end to end:
batched environments, a reward, a policy-gradient learner, and the
randomization that makes transfer plausible.

## What this chapter teaches

- Locomotion as a reinforcement learning problem: observations, actions as
  joint targets, rewards, terminations.
- Proximal policy optimization (PPO) with generalized advantage estimation.
- Domain randomization and terrain curricula.
- Teacher–student training: a policy with privileged state teaches one that
  sees only sensors.

## The math

PPO maximizes the clipped surrogate

```jai
L(θ) = E_t[ min(r_t(θ) Â_t, clip(r_t(θ), 1 − ε, 1 + ε) Â_t) ],   r_t = π_θ(a_t|s_t) / π_old(a_t|s_t)
Â_t  = Σ_l (γλ)^l δ_{t+l},   δ_t = r_t + γ V(s_{t+1}) − V(s_t)
```

([Schulman et al., 2017](https://arxiv.org/abs/1707.06347);
[Schulman et al., GAE, 2016](https://arxiv.org/abs/1506.02438)). Massively
parallel PPO for legged robots trains a walking policy in minutes on one GPU
([Rudin et al., CoRL 2021](https://arxiv.org/abs/2109.11978)).

## The design in Psionic

The environment is chapter 179's batch of quadrupeds on randomized terrain,
with chapter 181's sensors and chapter 188's PD targets as the action
interface. The learner uses Psionic's own training stack (autodiff, the
optimizer chapters) and reads observations without host copies (Coax R23).
Every randomized parameter is declared with its range and provenance (R20).
Training runs are recordings in chapter 184's sense: seed, scene, profile
and learner configuration, so a policy names exactly how it was made.
Real hardware remains a separate, authorized integration.

## What it builds

- `psionic/lib/robotics/locomotion/`: environment, reward, PPO learner,
  terrain curriculum, teacher–student distillation.

## Acceptance checks and oracles

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

Chapters 179, 181, 183, 188 and Psionic's training chapters. From Coax:
R14, R20, R21, R23.
