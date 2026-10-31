# 128. A complete pretraining run

> **Status: unimplemented — design draft.** Nothing here runs yet.

This chapter trains the nanochat architecture from scratch in Coax:
tokenizer, model and evaluation. You start with a tiny run on a small
licensed corpus, then scale one variable at a time with measured tokens,
memory, throughput and quality.

## What this chapter covers

- Tokenizer training from [chapter 123](ch123-learn-a-tokenizer.md).
- Initialization as the pinned code specifies it.
- Learning-rate schedules, gradient accumulation and token budgets.
- Document packing. The upstream loader uses BOS-aligned best-fit cropping
  and describes its own resume as approximate. Psionic requires exact resume
  as a stronger contract.
- Resume from checkpoints.
- Held-out loss and bits per byte on frozen data.
- A compute comparison: the same budget spent on a smaller model trained
  longer and on a larger model trained for fewer steps, including the
  deployed inference cost of each.

## Dependencies

- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md) and
  [chapter 120](ch120-data-and-resumable-state.md): checkpoints, splits and
  exact resume.
- [Chapter 123](ch123-learn-a-tokenizer.md) and
  [chapter 124](ch124-adamw-and-explicit-precision.md): tokenizer training
  and MuonAdamW.
- [Chapter 127](ch127-differentiate-nanochat.md): checked gradients.
- [Chapter 129](ch129-train-a-whole-model-on-one-gpu.md) for runs beyond the
  CPU.
- Upstream scale, timing and cost are not Psionic promises. A larger run
  needs an owner-approved compute budget.

## Done means

- The tiny run completes end to end in Coax with an explicit compute
  envelope, and its held-out bits per byte is reported.
- An interrupted run resumes and reproduces the uninterrupted run.
- The two-way budget comparison reports held-out quality and inference cost
  for both models.
- A tiny demonstration and an upstream speedrun reproduction are reported as
  separate milestones.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
