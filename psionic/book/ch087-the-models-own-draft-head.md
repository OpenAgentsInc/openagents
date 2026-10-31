# 87. The model's own draft head

> **Status: unimplemented — design draft.** Nothing here runs yet.

Mistral Large 4 ships an embedded multi-token-prediction draft head (`mtp`):
three layers with an "eagle" normalization strategy and a recurrent head. This
chapter uses it as a speculator under chapter 75's acceptance rule.

## What this chapter covers

- The draft head's own layers and normalization.
- Drafts verified under chapter 75's standard acceptance rule.
- Identical greedy output with and without speculation.
- Exact acceptance probabilities on tiny fixtures.
- Measured acceptance rate and draft length.

## Dependencies

- [Chapter 75](ch075-let-the-small-model-guess.md): the acceptance rule and
  snapshots.
- [Chapter 76](ch076-verify-a-draft-block-in-parallel.md): batched device
  verification.
- [Chapter 86](ch086-sparse-attention-and-shared-layers.md): the Large 4 layers
  the head attaches to.

## Done means

- Greedy output with the draft head equals greedy output without it.
- Acceptance probabilities on tiny fixtures equal their exact values.
- Acceptance and draft length are reported on a named workload. No speedup is
  claimed without a measured latency comparison.

## Sources

- [Chapter plan](PLAN.md), Part VIII.
- [Mistral target survey](../docs/targets/mistral.md).
