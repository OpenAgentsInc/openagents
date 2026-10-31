# 141. Distill Mammoth into Elephant

> **Status: unimplemented — design draft.** Nothing here runs yet.

Mammoth gives better decisions than Elephant and costs more to run. This
chapter trains Elephant to match Mammoth's answer distributions. The
distilled student is a new model profile. It inherits neither model's
evidence and earns its own.

## What this chapter covers

- Mammoth's slot distributions as soft targets on a recorded request corpus,
  kept as `distill-v1` targets, separate from outcome labels.
- KL divergence and cross-entropy over decision slots as the training loss.
- The LoRA and hybrid backward machinery of chapter 140.
- Held-out agreement with Mammoth.
- Chapter 16's quality metrics on held-out requests.
- Elephant's cost measured against Mammoth's on the same host.

## The distillation-target contract

The targets follow the `distill-v1` contract in
[chapter 247](ch247-outcomes-become-training-rows.md#a-distillation-target-is-a-different-record).
For this chapter:

- **Teacher identity.** Mammoth's pinned weights, tokenizer and readout, and
  the numerical profile that produced each distribution (the CPU reference of
  chapter 15 or the CUDA path of chapter 46). Distributions from the two
  profiles form two target sets, and the student trains on one.
- **Teacher distributions.** Mammoth's probability for each decision option
  at temperature 1, plus the mass outside the options.
- **Split provenance.** Splits come from the shared exposure ledger. A
  request group held out to evaluate the student against outcomes is never a
  training target.
- **Predictions apart from correctness.** Mammoth's answers are targets, not
  labels. The student's accuracy and calibration come from outcome rows with
  labels from a check or a diff. A request with a Mammoth target and no
  outcome row counts toward agreement only.

## Dependencies

- [Chapter 14](ch014-execute-elephant.md) and
  [chapter 15](ch015-execute-mammoth.md): both models.
- [Chapter 16](ch016-are-the-probabilities-any-good.md): quality metrics.
- [Chapter 140](ch140-post-train-a-decision-model.md): post-training
  machinery.
- [Chapter 247](ch247-outcomes-become-training-rows.md): admitted training
  rows and the exposure ledger.

## Done means

- Held-out agreement with Mammoth is reported before and after distillation,
  separately from accuracy against outcome labels.
- Every target names Mammoth's identity and numerical profile, and every
  split names the exposure-ledger artifact that assigned it.
- Chapter 16's metrics are reported for the student, Elephant and Mammoth on
  the same held-out corpus.
- The student's latency and resource cost are measured against Mammoth's.
- The student has its own model identity, and no result from either parent
  model is reused for it.

## Sources

- [Chapter plan](PLAN.md)
