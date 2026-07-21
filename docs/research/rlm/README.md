# Recursive Language Models (RLM)

Research notes on **Recursive Language Models** (`arXiv:2512.24601v3`, Zhang,
Kraska, Khattab, MIT CSAIL / OASYS) and the companion open-source repository
`alexzhang13/rlm`.

- Paper arXiv: <https://arxiv.org/abs/2512.24601>
- Code: <https://github.com/alexzhang13/rlm> · local clone
  `~/work/projects/repos/rlm`
- Minimal reference: <https://github.com/alexzhang13/rlm-minimal>
- Blog: <https://alexzhang13.github.io/blog/2025/rlm/>
- Docs site: <https://alexzhang13.github.io/rlm/>
- Training model example:
  <https://huggingface.co/mit-oasys/rlm-qwen3-30b-a3b-v0.1>

## Source boundary

This public repo does not vendor the paper PDF or the upstream Python package.
The notes here are paraphrased, OpenAgents-oriented summaries. Agents that need
the full paper should read the local LaTeX source package or fetch arXiv. The
repo analysis is based on the pinned local clone listed in [`source.md`](source.md).

Related prior OpenAgents note (decision audit, not this synopsis):

- [`../2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`](../2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md)

## Contents

| File | Role |
| --- | --- |
| [`source.md`](source.md) | Exact paper package path, digests, repo pin, citation |
| [`paper-summary.md`](paper-summary.md) | Structured synopsis of the paper |
| [`paper-analysis.md`](paper-analysis.md) | Critical analysis of claims, methods, and limits |
| [`repo-analysis.md`](repo-analysis.md) | Full analysis of the `alexzhang13/rlm` implementation |
| [`openagents-implications.md`](openagents-implications.md) | What RLM means for OpenAgents surfaces |

## One-paragraph summary

An RLM is an inference scaffold that does **not** put a long user prompt into the
root model context. It loads the prompt as a variable in a persistent REPL,
gives the model only short metadata, and lets the model write code that peeks,
slices, and **programmatically** calls sub-LMs or sub-RLMs over pieces of the
prompt. On four long-context tasks the paper reports large quality gains versus
compaction, CodeAct, and coding agents at comparable cost, including work past
10M tokens. A small fine-tune (`RLM-Qwen3-8B` on 1,000 distilled trajectories)
improves an 8B base by a median of about 28%. The open repository (`rlms` 0.1.3)
implements that loop with multi-provider clients, local and cloud sandboxes,
depth-limited recursion, budgets, a trajectory visualizer, and a
`prime-rl` / `verifiers` training harness.

## Why it matters here

OpenAgents already treats long-horizon coding, fleet dispatch, and sandbox
execution as first-class product work. RLM is a concrete pattern for:

1. **Unbounded prompt handling** without trusting compaction alone.
2. **Symbolic fan-out** of sub-model work from code, not only verbal tool calls.
3. **Trainable harness behavior** (root model learns to orchestrate the REPL).

It does **not** replace OpenAgents authority, receipt, or product-promise gates.
Use it as a candidate executor pattern on sandboxed capacity, not as online
governance authority.

## Integration program

The full integration audit, architecture, and the RLM epic/issue program live
in [`../../rlm/2026-07-21-rlm-integration-audit-and-roadmap.md`](../../rlm/2026-07-21-rlm-integration-audit-and-roadmap.md)
(epic #9136, RLM-01..RLM-08 = #9137-#9144). The unified chat-runtime
sequencing is
[`../../desktop/2026-07-21-chat-runtime-unified-roadmap.md`](../../desktop/2026-07-21-chat-runtime-unified-roadmap.md).
