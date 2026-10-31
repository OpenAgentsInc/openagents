# 115. Train the whole tiny decoder

> **Status: unimplemented — design draft.** Nothing here runs yet.

Chapter 113 gives you a small causal decoder with checked gradients. This
chapter trains it end to end: data, training, held-out likelihood and
generation. The trained model then stays in the repository as a permanent
regression for every later training change.

## What this chapter covers

- The complete small transformer from chapter 113: attention, feed-forward,
  normalization and the language-model head.
- Training on fixed licensed splits with chapter 112's byte vocabulary or
  chapter 123's learned tokenizer.
- An optimizer from chapter 124 and a recorded learning-rate schedule.
- Held-out likelihood and bits per byte.
- Seeded generation through the sampling of chapter 53.
- Checkpoints and exact resume from chapter 122.
- A pinned, small training run that later chapters use as a regression.

## Dependencies

- [Chapter 112](ch112-bytes-and-a-small-language-model.md): bytes, splits and
  a named RNG.
- [Chapter 113](ch113-differentiate-a-decoder.md): the decoder and its
  gradients.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md) and
  [chapter 124](ch124-adamw-and-explicit-precision.md).
- [Chapter 119](ch119-change-an-activation-change-the-answer.md) and
  [chapter 235](ch235-can-gradient-descent-learn-exact-execution.md) use this
  trained model.

## Done means

- The run beats a bigram baseline on held-out likelihood.
- The same seed and data order reproduce the same loss curve and final
  weights under the declared profile.
- An interrupted run resumes and reproduces the uninterrupted run.
- Seeded generation is reproducible.
- The run is short enough to stay in the regular checks.

## Sources

- [Chapter plan](PLAN.md)
