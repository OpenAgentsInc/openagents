# Decisions in production: notes from Jev and the decision-model work

Planning note, 2026-10-29, for Part XXII of the [book plan](../book/PLAN.md)
and small additions to chapters 12 and 17. It condenses pre-reset research,
readable with `git show 0b401561a0^:<path>`:

- `docs/research/typesafe/`: three transcripts of TypeSafe's Diogo Almeida
  (AI Council, 2026-06-19; AI Engineer, 2026-07-31; a16z on Jev, 2026-09-28)
  and two calibration notes.
- `docs/research/decisionmodels/`: an overview, calibration, evidence, the
  s1-llm-auto-router, REAX, an ONNX-native Psionic roadmap and CPU/CUDA next
  steps.
- `docs/research/baselabs-decision-models/README.md`, already reflected in
  the targets for Elephant and Mammoth.

The numbers below are historical measurements and vendor claims, not Psionic
results.

## What a decision model is for

A request is a JSON state and typed questions (choice, yes/no, score); the
answer is a probability per option, from one forward pass and no generated
text. Almeida's argument: preference-tuned chat models optimize what people
like, which overstates confidence; a decision model is trained for calibrated
probabilities, and automation needs reliability that people can program
against without trying examples first (uptime, determinism, and robustness to
irrelevant changes in the input). TypeSafe's Jev 1.13 answered in about 247 ms
hosted; the stated aim is "intelligence per dollar", answering more like a
database than a chat.

## Calibration is the consumer's job, and it can be measured

- **Thresholds need your own labelled data.** TypeSafe's guidance used three
  bands, with a middle band routed to a person and more than 0.9 required for a
  destructive action. A probability is never permission.
- **Our earlier router measured it.** Binned maps fitted on a calibration split
  lowered held-out expected calibration error from 0.050 to 0.029 on `route`
  (254 rows) and from 0.131 to 0.062 on `answer` (182 rows), but the map was
  left off because about 40 thresholds had been tuned on raw numbers by hand.
- **Costs give thresholds.** With a wrong action costing ten model calls and a
  fall-through costing one, act when p ≥ 0.9. On held-out data (189 rows) it
  cut wrong answers served from 5 to 3, but cost per row barely moved and the
  confidence intervals overlap: a measured hypothesis, not yet a win.
- **The planned loop.** Record raw and calibrated readings, the threshold and
  the model identity for every decision; join each to its outcome; refit on a
  schedule; alert on drift; report reliability, ECE, Brier, log loss and
  confident errors; refuse a map fitted on another model or question set.

These become chapters 156 to 158.

## Checking that two machines answer alike

Decision Models' REAX subnet verifies miners by recomputing fresh seeded items
and comparing distributions in centred log space. The pre-reset Psionic
measured the same distance: honest pairs stayed small (CPU against Metal at
most 0.0035; CUDA f16 against CPU at most 0.063), while cheap substitutes
mostly failed (a Q8_0 copy passed 3.6% of rounds, another runtime 18%). Limits
set between the two separate honest machines from substitutes. Stock vLLM was
also reported to flip 6 of 300 answers between identical passes. These become
chapters 159 and 160; chapter 21's re-execution checks use the distance.

## Decisions in milliseconds

The s1-llm-auto-router classifies a request with an 11-layer mmBERT-small
encoder (119.4M parameters, 98.3M of them uint8 embeddings) and seven fixed
heads, exported as ONNX. The vendor reports 14 ms p50 on four CPU cores; the
pre-reset Psionic measured 3.3 ms p50 locally. Its native engine later beat
ONNX Runtime (CPU, 512 tokens: 66.1 against 81.1 ms; CUDA, full mix: 0.50
against 1.27 ms) with raw-logit error at most 5.25e-6. Both missed a 20 ms
p99 target at 512 tokens. Lessons kept: execution parity is not decision
quality; an explicitly selected engine never silently falls back; model
identity is separate from engine identity; measure cold and warm, interleaved.
These become chapters 161 to 163.

## Rolling out a different model

The router plan promoted a new model only after running it beside the current
one on live traffic and meeting disagreement and outcome gates. That is
chapter 164, distinct from chapter 122's rollout of a new build of the same
model.

## Small additions elsewhere

- Chapter 17: warm up with seeded decisions before reporting ready; the
  pre-reset engine's first answer fell from 382 to 206 ms with warm-up.
- Chapter 12: name the corpus and the model identity on every reported result.
