# 141. Distill Mammoth into Elephant

> **Status: unimplemented — design draft.** Nothing here runs yet.

Mammoth gives better decisions than Elephant and costs more to run. This
chapter trains Elephant to match Mammoth's answer distributions. The
distilled student is a new model profile. It inherits neither model's
evidence and earns its own.

## What this chapter covers

- Mammoth's slot distributions as soft targets on a recorded request corpus.
- KL divergence and cross-entropy over decision slots as the training loss.
- The LoRA and hybrid backward machinery of chapter 140.
- Held-out agreement with Mammoth.
- Chapter 16's quality metrics on held-out requests.
- Elephant's cost measured against Mammoth's on the same host.

## Dependencies

- [Chapter 14](ch014-execute-elephant.md) and
  [chapter 15](ch015-execute-mammoth.md): both models.
- [Chapter 16](ch016-are-the-probabilities-any-good.md): quality metrics.
- [Chapter 140](ch140-post-train-a-decision-model.md): post-training
  machinery.
- [Chapter 247](ch247-outcomes-become-training-rows.md): admitted training
  rows and the exposure ledger.

## Done means

- Held-out agreement with Mammoth is reported before and after distillation.
- Chapter 16's metrics are reported for the student, Elephant and Mammoth on
  the same held-out corpus.
- The student's latency and resource cost are measured against Mammoth's.
- The student has its own model identity, and no result from either parent
  model is reused for it.

## Sources

- [Chapter plan](PLAN.md)
