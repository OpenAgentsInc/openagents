# 255. Edit an original language model

> **Status: unimplemented — design draft.**

[Chapter 253](ch253-read-and-repair-a-models-weights.md) establishes an edit
boundary and exploratory tiny-model results. This chapter will test whether
targeted parameter changes improve a reproducible failure in an original
language model. Its result remains open. The weight-space programming blog's
Qwen3.5-0.8B experiment is a research lead, not a capability already present
in Psionic.

## Admit a model and a reproducible problem

Select the smallest fully supported original decoder with a failure that
can be reproduced under pinned tokenizer, chat recipe, sampling settings,
seed and token budget. Establish reference parity before intervention.
Qwen3.5-0.8B requires its own complete generation/admission path and evidence;
Elephant and Mammoth's Qwen-derived decision paths do not qualify it.

Begin with repeated-generation loops if they actually occur. Record answer
accuracy, completion rate, repeated-token spans, token-limit hits and tokens
per completed answer. Compare sampling changes before editing weights.
Keep a nonlooping control with ordinary reasoning errors. This separates
removing an obstruction from adding an absent ability.

## Grow the editing interface deliberately

Chapter 253's profile has at most 4,096 binary64 values. An original model
needs a new versioned profile: authenticated original shards, bounded
selected tensor windows, explicit BF16/F32 rounding, source and result
digests, sparse overlays or exported candidate tensors, and complete
publication with rollback. Reuse chapters 7–8's inert artifacts. Never
silently widen `weight-edit-f64-v1` or edit the admitted source files in place.

Add reusable bounded inspection: named activation capture, QK/OV products,
direction projection and localized sparse or low-rank edits. Explain the
matrix orientation and work a rank-one removal by hand. Compare a permanent
edit with an independently implemented activation intervention, and test
both no-op and exact restoration under their declared numerical profiles.

## Budgets, controls and qualification

Use distinct weights-only, numerical-analysis and activation/evaluation
conditions. Count individual sequences, generated tokens and environment
steps, as well as tool calls and total proposer cost. Keep a base model,
sampling-only correction, random count/norm-matched edits, simple ablations,
low-rank fine-tuning and an informed repair as explicit baselines.

Choose edits using development failures. Save each candidate before opening
held-out tasks. Freeze target improvement, retained-task floors, log loss,
confident errors and resource limits. Chapter 253's untrained-model false
positives prohibit using relative NLL improvement alone. Use chapters
239–241's independent qualification and multiple task/seed blocks; five
paired copies of one prompt are not five independent capabilities.

Candidate identity must differ from the base and remain bound to its edit
record. [Chapter 224](ch224-shadow-a-new-model-then-promote-it.md)'s shadow
and atomic activation design is the eventual integration point after
qualification. This draft authorizes no automatic promotion or live model
replacement. A small changed fraction does not prove unrelated behavior
was preserved; measure it.

Graduation requires original-model parity, saved proposals and edits,
independent numerical checks, frozen held-out/retained evaluation, a full
cost ledger, exact supported scope and a runnable failure diagnostic. A
failure to improve is a publishable result of the chapter.
