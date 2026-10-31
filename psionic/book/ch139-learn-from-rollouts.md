# 139. Learn from rollouts

> **Status: unimplemented — design draft.** Nothing here runs yet.

nanochat's `chat_rl.py` is a simplified on-policy recipe. The model
generates answers, a checker scores them, and the policy gradient raises the
probability of answers that scored above the mean. This chapter rebuilds
that specific recipe. It does not claim general GRPO, PPO or RLHF support.

## What this chapter covers

- The pinned recipe: mean-centered rewards and token-level normalization,
  with no KL trust region and no PPO ratio or clipping.
- Bounded rollouts: a fixed number of samples per prompt and a fixed maximum
  length.
- Rollouts from chapter 242's environments, each with a checker.
- Independent checks of rewards and of the policy gradient on tiny cases.
- Held-out task results.

## Dependencies

- [Chapter 136](ch136-supervised-conversations-and-tools.md): the SFT model
  that the recipe starts from.
- [Chapter 242](ch242-environments-with-checkers.md): environments and
  checkers.
- [Chapter 53](ch053-choosing-the-next-token.md): seeded sampling.
- This chapter does not need chapter 137 or chapter 138.

## Done means

- On tiny fixtures, the policy gradient matches an independent calculation.
- Rewards match each checker's verdict for every rollout.
- Held-out task results are reported before and after, with the noise floor
  of chapter 240.
- Generation, reward evaluation and sample budgets are recorded for every
  run.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
