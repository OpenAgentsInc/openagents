# 138. Preferences and reward models

> **Status: unimplemented — design draft.** Nothing here runs yet.

A reward model scores an answer so that training can prefer better ones.
This chapter trains one from pairwise human preferences and measures where
it fails. An optional DPO experiment trains a policy directly from the same
pairs.

## What this chapter covers

- HelpSteer3 pairwise preference data, with provenance and fixed splits.
- Ranking and reward losses, such as a pairwise logistic loss.
- A reward model built on the SFT model of chapter 136.
- Held-out comparisons: how often the reward model prefers the answer people
  preferred.
- Reward-model limitations: cases where the score rises while quality does
  not, such as longer answers.
- An optional, separately specified DPO experiment.

## Dependencies

- [Chapter 136](ch136-supervised-conversations-and-tools.md): the SFT model.
- [Chapter 120](ch120-data-and-resumable-state.md): splits.
- [Chapter 240](ch240-how-far-a-metric-moves-by-itself.md): noise floors for
  held-out comparisons.

## Done means

- Held-out pairwise accuracy is reported with an uncertainty estimate.
- The chapter reports at least one measured reward-model limitation.
- If the DPO experiment runs, its objective, data and evaluation are written
  before it starts, and it is reported apart from the reward model.
- No broad RLHF parity is claimed.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
