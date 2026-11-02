# Separate fine-tuning comparison

Declared before preparing/evaluating these candidate weights, 2026-11-02.
Start from each of the five saved shortcut models. Ten full-batch AdamW
steps, rate 0.002, beta1=0.9, beta2=0.99, epsilon=1e-8, no decay. Optimize
the existing decoder's full sequence next-token loss on the fifteen
development conflicts (answers 1–3); do not inspect held-out results until
all five artifacts have been written. No hyperparameter search or retry.

Each model costs 150 sequence forward/backward passes. Exhaustive ablation
costs 231 development forward passes including the base, without a backward
pass. These are different compute budgets, explicitly reported. All 664
parameters may move. Reuse the original held-out/retained metrics and
descriptive criterion unchanged, including any discovered weakness in it.
