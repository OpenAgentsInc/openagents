# TRINITY: An Evolved LLM Coordinator

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


**Authors:** Jinglue Xu, Qi Sun, Peter Schwendeman, Stefan Nielsen, Edoardo
Cetin, Yujin Tang (Sakana AI; Univ. Michigan; Institute of Science Tokyo)
**Venue:** ICLR 2026 · **arXiv:** [2512.04695](https://arxiv.org/abs/2512.04695)

## One-line summary

A ~0.6B small language model plus a **~10K-parameter head**, trained with an
evolutionary strategy, learns to orchestrate a pool of frontier LLMs over
multiple turns — beating every individual model and prior multi-agent method,
and setting a new LiveCodeBench record of **86.2%**.

## Problem

Combining diverse foundation models is promising, but **weight-merging** is
blocked by mismatched architectures and closed APIs — it can't incorporate the
closed-source models that define the frontier. TRINITY instead does **macro-level
composition via coordination**: treat each model as a black box and fuse their
strengths at test time, without touching any weights.

The core bet: the **hidden states** of a compact LM already contain enough
contextual signal for a tiny head to make good coordination decisions. The
penultimate-token hidden state, in particular, attends over the whole sequence
and carries rich context.

## Method

**Parametrization (under 20K learnable params total):**
- Backbone: **Qwen3-0.6B** SLM, mostly frozen.
- A **lightweight head** runs in parallel to the LM head, takes the
  penultimate-token hidden state `h ∈ R^d`, and emits `L+3` logits — `L` to pick
  one LLM from the pool, `3` to assign a role. A single linear layer (~10K
  params) is the best head.
- **Singular value fine-tuning (SVF):** for a chosen backbone layer, do an SVD
  and learn only the singular-value *scales*, keeping the orthogonal matrices
  fixed. Cheap, but measurably improves the representation.
- The coordinator's generated *text* is discarded — only the head's logits
  matter, so it can decide from an early-token hidden state without a full
  generation.

**Tri-role coordination (≤5 turns):** at each turn the coordinator picks an
agent and a role, injects a role-specific prompt, queries the agent, and appends
the lightly-processed output to the transcript.
- **Thinker** — strategizes: high-level plans, decompositions, critiques.
- **Worker** — executes: concrete derivations, code, numerical results.
- **Verifier** — evaluates: emits `ACCEPT`/`REVISE` (+ diagnosis). On `ACCEPT`,
  coordination halts and the current answer is returned. Otherwise it stops at
  the turn budget.

**Training — sep-CMA-ES:** the objective is the expected terminal (binary)
reward over full multi-turn trajectories. This regime is brutal for gradient
methods: parameters are weakly coupled (REINFORCE's per-param gradients are
low-SNR), each evaluation is expensive (it runs the whole agent pipeline), and
the budget is tiny (1.5k–40k evaluations for a ~10K-dim problem). The authors
find **separable CMA-ES** (diagonal-covariance evolution) dominates because the
problem exhibits strong **block-ε-separability** — informative signal
concentrates within parameter blocks, inter-block interference is negligible.
Appendix theory (Propositions 1–2) shows sep-CMA-ES gains grow ~linearly with
iterations while random search grows only logarithmically.

## Key results

- **In-distribution (Fig. 3):** highest score on all four of MATH500, MMLU,
  RLPR, LiveCodeBench; mean relative error reduction of **21.9%** over the
  second-best method. Approaches the "Per-Question-Best" oracle upper bound on
  three of four tasks.
- **LiveCodeBench V6 SOTA (Fig. 4):** with output-length constraints removed
  (no retraining), pass@1 = **0.862 ± 0.5%**, vs GPT-5 0.838, Gemini 2.5 Pro
  0.672, Claude-4-Sonnet 0.465. Performance rises monotonically with the turn
  budget (0.823 → 0.863 from 2 → 6 turns) — evidence it's *coordinating*, not
  just routing.
- **Zero-shot transfer (Table 1):** beats every individual model on all four
  held-out tasks (AIME, BigCodeBench, MT-Bench, GPQA-D); top average 54.21.
- **Ablations (Table 2):** removing SVF, the Thinker role, the tri-role scheme,
  or agent selection all hurt; using the last (EOS) token instead of the
  penultimate causes a >10-point LiveCodeBench collapse.
- **Representation separability (Figs. 5, 12):** a linear SVM classifies task
  type from hidden states with ~1.0 accuracy — the manifold is near-linearly
  separable, which is why a linear head suffices. Higher separability correlates
  with better coordination.
- **Optimizer comparison (Table 4):** sep-CMA-ES > SFT > RS ≫ REINFORCE. SFT is
  competitive for single-step routing but its label-generation cost explodes
  combinatorially in the multi-turn setting (~8.7×10¹⁰ queries), making it
  unusable.

## Worker pool

GPT-5, Gemini-2.5-Pro, Claude-4-Sonnet (closed); Gemma-3-27B-It,
DeepSeek-R1-Distill-Qwen-32B, Qwen3-32B (direct), Qwen3-32B (reasoning) (open).

## Limitation

The gap between abstract reasoning and grounded execution: the coordinator can
plan tool use but can't act on tools. Future work adds heterogeneous agents
(code interpreters, APIs).

## Takeaway

You don't need a capable coordinator — you need a *well-represented* one. A
near-free head on a small frozen LM, trained by evolution rather than gradients,
extracts frontier-beating coordination from black-box models.
