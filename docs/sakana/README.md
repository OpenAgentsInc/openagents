# Sakana AI — LLM Coordination Papers

Summaries of two ICLR 2026 papers from Sakana AI on **test-time model
composition via coordination**: combining the complementary strengths of
diverse, black-box LLMs without merging weights or retraining the workers.

Both papers share the same author group (Xu, Sun, Schwendeman, Nielsen, Cetin,
Tang), the same 7-model worker pool, and the same four training benchmarks
(MATH500, MMLU, RLPR, LiveCodeBench V1→V6). They are two answers to the same
question — *how do you learn a coordinator that orchestrates frontier models?* —
reached from opposite directions.

| | TRINITY | Conductor |
|---|---|---|
| arXiv | [2512.04695](https://arxiv.org/abs/2512.04695) | [2512.04388](https://arxiv.org/abs/2512.04388) |
| Coordinator | Qwen3-**0.6B** SLM + ~10K-param head | Qwen2.5-**7B** LLM |
| What it emits | **logits** (pick agent + role) | **natural language** (subtasks + topology) |
| Training | **sep-CMA-ES** (evolution, <20K learnable params) | **GRPO** (RL, full 7B finetune) |
| Coordination unit | 3 fixed roles (Thinker/Worker/Verifier) | free-form subtasks + access lists |
| LiveCodeBench V6 | **86.2%** pass@1 (SOTA) | **83.9%** pass@1 (SOTA) |
| Hardware | minimal (head training) | 2× H100 80GB, 200 GRPO iters |

**Our summaries:**
- [`trinity.md`](trinity.md) — the lightweight, logit-emitting, evolution-trained coordinator.
- [`conductor.md`](conductor.md) — the RL-trained, natural-language, agentic-workflow coordinator.

**Full papers as Markdown** (converted from arXiv HTML via
[arxiv2md](https://github.com/timf34/arxiv2md), refs/citations kept):
- [`trinity-2512.04695v3.md`](trinity-2512.04695v3.md)
- [`conductor-2512.04388v5.md`](conductor-2512.04388v5.md)

**How we adapt them into our system:**
- [`adapting-sakana-coordination.md`](adapting-sakana-coordination.md) — concrete
  mapping of TRINITY (logit router) and Conductor (NL workflow planner) onto our
  Tassadar/Pylon/Psionic seams, with a phased build plan.
- [`coordinator-as-verified-work.md`](coordinator-as-verified-work.md) — the
  structural edge we have over Sakana: a cryptographic, sat-denominated terminal
  reward (exact-trace replay), and the coordinator as a paid market participant.
- [`psionic-coordinator-roadmap.md`](psionic-coordinator-roadmap.md) —
  Psionic-specific primitives audit (verified against source): the 5 missing
  learning-side primitives, where each attaches, and the phased build order.
- [`tassadar-run-integration.md`](tassadar-run-integration.md) — how a learned
  coordinator combines with the live Tassadar run via its verification-class
  registry (`exact_trace_replay` / `seeded_replication`) as the reward oracle.

## Why these matter to us

Both validate that a **small, cheap coordinator can beat every frontier model
it orchestrates** by routing per-task and per-subtask — relevant to any
multi-model product surface that wants frontier-level quality without betting on
a single provider. TRINITY is the cheaper-to-train, lower-latency design;
Conductor is the more expressive, more general one (it prompt-engineers and
designs topologies in plain language, and can recurse on itself for test-time
scaling).
