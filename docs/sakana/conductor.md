# Learning to Orchestrate Agents in Natural Language with the Conductor

**Authors:** Stefan Nielsen, Edoardo Cetin, Peter Schwendeman, Qi Sun, Jinglue
Xu, Yujin Tang (Sakana AI; Univ. Michigan; Institute of Science Tokyo)
**Venue:** ICLR 2026 · **arXiv:** [2512.04388](https://arxiv.org/abs/2512.04388)

## One-line summary

A 7B model trained with reinforcement learning becomes a **Conductor** that, in
plain natural language, breaks a problem into subtasks, assigns each to a worker
LLM, and wires up who-sees-what — discovering coordination strategies that beat
every frontier worker and set new SOTA on LiveCodeBench and GPQA Diamond.

## Problem

Frontier LLMs are individually powerful but **none is universally best**, and
extracting their full value still relies on hand-designed agentic workflows and
prompt engineering. Can a model *learn* to orchestrate other models — designing
the communication topology *and* the prompts — purely from end-to-end reward?

## Method

**The Conductor task.** Given a question, the Conductor (itself an LLM) outputs
an entire **agentic workflow** as three parallel Python lists, after a
chain-of-thought:
- `model_id` — which worker (by ordinal id) runs each step;
- `subtasks` — the natural-language instruction for each step;
- `access_list` — which previous steps' outputs are visible to each step
  (`"all"`, `[]`, or specific indices), defining the topology.

Workers are presented as anonymous "Model 0, Model 1, …" to prevent
name-based bias and force exploration. The final step's output is returned to
the user. This single output medium — free natural language — subsumes
best-of-N, sequential chains, and arbitrary tree topologies.

**Training — GRPO.** Start from a **Qwen2.5-7B** checkpoint. Two progressive
rewards: a **format** condition (parseable lists → else 0) and a **correctness**
condition (executed workflow matches solution → 1, else 0.5). Trained with GRPO
(group-relative advantages, no value network). Notably efficient: **200 GRPO
iterations**, batch 256, **no KL regularization**, on **2× H100 80GB**. Relying
on a powerful worker pool sidesteps the exploration problem that usually plagues
small-model RL. Few-shot examples in the prompt cold-start the output format.

**Two finetuning extensions:**
- **Adaptive worker pools** — finetune on randomly sampled k-of-n model subsets
  so the Conductor generalizes to *any* available set (e.g. open-only or
  closed-only), catering to cost/availability constraints.
- **Recursive topologies / test-time scaling** — let the Conductor select
  *itself* as a worker. After seeing its prior strategy's output, it can revise
  on the fly. Capping recursion depth gives a new tunable inference-compute axis.

## Key results

- **Unconstrained SOTA (Table 1):** 7B Conductor tops every frontier baseline
  across MATH500 (99.4), MMLU (94.1), RLPR (44.75), LiveCodeBench (83.93),
  AIME25 (93.3), BigCodeBench (37.86), GPQA-D (87.5) — avg **77.27** vs GPT-5's
  74.78. The ~3% gains on AIME/GPQA-D are comparable to a full GPT generation
  jump on those saturated benchmarks.
- **Beats expensive baselines cheaply (Figs. 4–5):** outperforms MasRouter, MoA,
  RouterDC, Smoothie and 5× self-reflection while using an **average of ~3
  steps** (well under the 5-step limit) — lower inference cost than all but
  RouterDC.
- **Recursion helps (Table 2):** on BigCodeBench, recursion lifts the Conductor
  37.8 → 40.0 by redistributing away from GPT-5 (which underperforms there)
  toward Claude/Gemini after observing its own output.
- **Dynamic pools (Fig. 6):** finetuned on open-only models, it beats Claude
  Sonnet 4 by ~10% in that constrained setting; on closed-only it fully retains
  pretrained performance.
- **Scale (Fig. 7):** 3B and 7B converge to the same *agent selection*, but 7B
  wins via better *prompt engineering* — natural-language capability is the
  scaling axis.
- **Difficulty adaptivity (Fig. 8):** learns to spend more steps on hard tasks
  (LiveCodeBench: multi-planner → implement → verify) and fewer on easy ones
  (MMLU: 1–2 retrieval steps).

## Notable findings

- **OOD few-shot beats in-distribution few-shot** (Table 4): showing the
  Conductor *out-of-domain* coordination examples improves performance more than
  in-domain ones — OOD examples convey "which agents combine well" without
  giving a reward-hackable strategy to copy.
- **Emergent roles:** the SOTA LiveCodeBench strategy uses Gemini + Claude as
  high-level planners and GPT-5 only to write final code; weak open models can
  outperform frontier ones at narrow subtasks (e.g. Qwen-32B as a format-checker
  where GPT-5 violates formatting).
- **Conductor "role abdication":** sometimes it hands its own planning job to a
  strong worker (e.g. Gemini) to devise subtasks for the others.
- **MoA can be hurt by weak workers** on high-variance tasks (LiveCodeBench):
  averaging in incorrect candidate solutions drags frontier performance down.

## Worker pool

GPT-5, Gemini-2.5-Pro, Claude-Sonnet-4 (closed); DeepSeek-R1-Distill-Qwen-32B,
Gemma3-27B-instruct, Qwen3-32B direct/thinking (open). Training data: 960
problems from MATH, MMLU, RLPR, LiveCodeBench V1.

## Limitation / ethics note

Performance still rests on access to expensive frontier APIs, which the authors
note may widen AI cost barriers. Fine-grained topology control didn't beat the
simple binary access scheme at 7B — left to larger future Conductors.

## Takeaway

Coordination is a *language* skill. Given a strong worker pool, pure end-to-end
RL teaches a small model to prompt-engineer and design workflows that exceed any
single frontier model — and letting it recurse on itself opens a new test-time
scaling dimension.

## Relationship to TRINITY

Same lab, same benchmarks, same worker pool, opposite design point.
[TRINITY](trinity.md) emits **logits** from a 0.6B + 10K-param head trained by
**evolution** (cheap, fast, fixed roles). Conductor emits **natural language**
from a 7B model trained by **RL** (expressive, general, designs its own
topologies and prompts). TRINITY edges LiveCodeBench (86.2 vs 83.9); Conductor
generalizes more flexibly and scales at test time via recursion.
