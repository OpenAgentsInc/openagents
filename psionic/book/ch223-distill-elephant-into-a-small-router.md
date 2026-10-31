# 223. Distill Elephant into a small router

> **Status: unimplemented — design draft.** Nothing here runs yet.

Elephant answers decisions well but costs a full decoder pass. This chapter
trains a small encoder to imitate Elephant's answer distributions, so cheap
requests can skip Elephant. The student is promoted only when it meets stated
agreement, calibration and precision limits.

## What this chapter covers

- Training data: Elephant's answer distributions on owned synthetic requests,
  admitted as rows by [Chapter 247](ch247-outcomes-become-training-rows.md).
- The student: [Chapter 222](ch222-a-decision-in-milliseconds.md)'s encoder with fixed heads.
- The loss: KL divergence or cross-entropy against Elephant's distributions.
- Temperatures fitted on a development split.
- Promotion gates, frozen before the test split is read:
  - agreement with Elephant;
  - calibration error;
  - cheap-answer precision, the share of answers the student keeps that are
    correct.
- Serving: the student routes a request to a cheap answer or to Elephant.

## Dependencies

- [Chapter 14](ch014-execute-elephant.md) and [Chapter 16](ch016-are-the-probabilities-any-good.md): Elephant and its decision metrics.
- [Chapter 222](ch222-a-decision-in-milliseconds.md): the encoder.
- [Chapter 118](ch118-one-trainer-for-every-model.md) and [Chapter 124](ch124-adamw-and-explicit-precision.md): the trainer and optimizers.
- [Chapter 215](ch215-fit-our-own-calibration.md): calibration fitting.
- [Chapter 224](ch224-shadow-a-new-model-then-promote-it.md) and [Chapter 241](ch241-gates-that-can-say-unverifiable.md): shadow promotion and verdicts.
- [Chapter 247](ch247-outcomes-become-training-rows.md): training rows.

## Done means

- The student passes all three promotion gates on held-out requests, or the
  chapter reports which gate failed.
- The student is a new model profile with its own evidence. It inherits none
  of Elephant's results.
- The chapter reports the cost and latency saved per request at the chosen
  operating point.

## Sources

- [Chapter plan](PLAN.md), Part XXII.
- [Decisions in production](../docs/decisions.md).
