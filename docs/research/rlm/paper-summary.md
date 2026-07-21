# RLM Paper Summary

**STATUS: Point-in-time research record (2026-07-21).** Based on local
`arXiv:2512.24601v3` LaTeX source. Not product authority.

## Core claim

Large language models hit two walls on long work:

1. **Hard context limits** (window size).
2. **Context rot** — quality falls as prompts grow, even inside the window, and
   the fall is worse on denser tasks.

Popular general fixes (especially **compaction / condensation**) lose fine
detail by design. Coding agents and sub-agent scaffolds often still load the
user prompt into the root window, or they only **verbalize** a few sub-tasks
instead of launching many sub-calls from code.

**Recursive Language Models (RLMs)** treat the long prompt as part of an
**external environment**. The root model never has to hold the full prompt. It
gets a symbolic handle (a REPL variable), writes code to inspect and transform
that handle, and may **recursively call** the same system on programmatically
chosen slices.

The external interface stays simple: string in, string out — same as a normal
LM completion — but with effectively unbounded input, longer output stitching,
and unbounded semantic work in principle (`Ω(|P|)` or even `Ω(|P|^2)` sub-work).

## Method

### Algorithm (high level)

1. Initialize a persistent REPL.
2. Store prompt `P` as a variable (for example `context`).
3. Expose a sub-RLM / sub-LM function inside the REPL.
4. Give the root model only **constant-size metadata** (length, short prefix,
   access instructions).
5. Loop:
   - Model emits code.
   - REPL executes code and updates state.
   - Only **truncated metadata** of stdout returns to the model history.
6. Stop when a designated final answer is set in the environment. Return it.

This truncation rule is intentional: it forces the model to keep long strings
in variables and sub-calls instead of dumping them into the root window.

### Three design choices the paper marks as essential

| Choice | RLM does | Weak “similar” agents often do |
| --- | --- | --- |
| Symbolic handle to the prompt | `P` lives outside the root window | Copy `P` into history first |
| Symbolic final answer | Answer can be a REPL value of arbitrary length | Model must `Finish` by generating the full answer in-window |
| Symbolic recursion | Code can loop and call sub-(R)LMs over slices | Separate “code” and “sub-agent” tools that cannot programmatically fan out |

The paper includes a contrasting “bad algorithm” table to make this sharp:
having code execution **and** a sub-LLM tool is not enough if sub-calls cannot
be driven from programs over the full prompt object.

### Implementation sketch in the paper

- Python REPL.
- Tools (including sub-LM / sub-RLM) as modules / functions.
- Root model and recursive models may differ (for GPT-5 experiments: GPT-5 root,
  GPT-5-mini recursive sub-models).
- Recursion **depth** is a first-class knob:
  - depth 0: REPL only, no sub-calls
  - depth 1: sub-LM calls
  - depth >1: sub-RLMs (nested REPL loops)

### Training (small scale)

**RLM-Qwen3-8B** is the first model the authors post-train as a native RLM.

- Teacher: Qwen3-Coder-480B-A35B as RLM on LongBenchPro (unrelated to the eval
  suite).
- Collect trajectories, filter failures and one-turn stubs.
- Distill **root turns** only (insight: leaf sub-calls are normal LM work. The
  hard skill is root orchestration).
- Programmatic fixes for common template errors (`FINAL` / `FINAL_VAR` style
  mistakes in the training trajectories).
- ~1,000 filtered samples. The run used about 48 H100 hours with `prime-rl` and batch size 64 for
  300 steps.

Separate RLVR experiment: train Qwen3-4B-Instruct as RLM on shorter MRCRv2
needles. Observe generalization to 1M / 8-needle split.

## Evaluation setup

### Tasks (complexity scales with length differently)

| Task | Rough processing complexity | Notes |
| --- | --- | --- |
| S-NIAH (RULER-style) | O(1) needles | Needle size fixed as length grows |
| BrowseComp-Plus (1K docs) | Multi-hop deep research | ~6M–11M tokens. Gold docs are guaranteed in 1K offline corpus |
| OOLONG (`trec_coarse`) | Linear in lines | Semantic label + aggregate over nearly all lines |
| OOLONG-Pairs | Quadratic pairs | Aggregate over nearly all pairs. Use F1 on list answers |
| LongBench-v2 CodeQA | Fixed multi-file repo MCQ | Code understanding under large repos |
| LongCoT-mini (extra) | Long compositional reasoning | Not only long *input* |

### Baselines

- Base model direct call
- CodeAct (with BM25 retrieval, and with sub-calls)
- Compaction agent (GPT-5-nano for compaction in GPT-5 experiments)
- OpenCode (with / without context offloading)
- Claude Code / Claude Opus 4.1 (with / without context offloading)

Models in the main table: **GPT-5** (medium reasoning) and
**Qwen3-Coder-480B-A35B**.

## Main numerical results (Table 1, selected)

Costs are average API cost in USD ± std as reported in the table. Scores are
task metrics as defined in the paper.

### GPT-5 family (RLM sub-calls to GPT-5-mini)

| Method | CodeQA | BrowseComp+ | OOLONG | OOLONG-Pairs |
| --- | --- | --- | --- | --- |
| Base | 24.0* | 0.0* | 44.0 | 0.1 |
| Compaction | 58.0 | 70.5 | 46.0 | 0.1 |
| OpenCode + offload | 64.0 | **94.0** | 52.0 | 4.8 |
| RLM depth=0 | 58.0 | 88.0 | 36.0 | 43.9 |
| RLM depth=1 | 62.0 | 91.3 | 56.0 | 58.0 |
| RLM depth=2 | **66.0** | 92.0 | 56.5 | 65.5 |
| RLM depth=3 | 58.0 | 92.0 | **58.0** | **76.0** |

`*` marks runs that hit input context limits.

Median abstract claim vs GPT-5 baselines across evaluated benchmarks:

- +26% vs compaction
- +130% vs CodeAct with sub-calls
- +13% vs Claude Code

(Exact per-task values live in the table. The abstract reports medians of
relative gains.)

### Qwen3-Coder-480B-A35B

| Method | CodeQA | BrowseComp+ | OOLONG | OOLONG-Pairs |
| --- | --- | --- | --- | --- |
| Base | 20.0* | 0.0* | 36.0 | 0.1 |
| Compaction | 50.0 | 38.0 | 44.1 | 0.31 |
| OpenCode + offload | 40.0 | 58.0 | 24.0 | 2.1 |
| RLM depth=0 | **66.0** | 46.0 | 43.5 | 17.3 |
| RLM depth=1 | 56.0 | 44.7 | **48.0** | **23.1** |
| RLM depth=2 | 54.0 | 68.0 | 26.0 | 19.0 |
| RLM depth=3 | 44.0 | **68.7** | 32.0 | 21.1 |

Note: deeper recursion **helps GPT-5** more reliably. Deeper recursion often
**hurts Qwen3-Coder** on average because syntax / template errors propagate into
sub-RLMs.

### Training result

- **RLM-Qwen3-8B** outperforms base Qwen3-8B as an RLM by a **median ~28%**
  across the four tasks, and approaches vanilla GPT-5 quality on three of them
  (abstract claim).
- Post-trained model is also **faster and cheaper** in trajectory length /
  mistakes (appendix runtime figure).

### Long reasoning (Observation 5)

On LongCoT-mini, RLM(GPT-5.2, depth=1) beats base GPT-5.2. With explicit
decomposition hints, the RLM builds a reasoning graph in the REPL and solves
nodes via sub-calls (~**+69.5%** overall in the paper’s report).

## Six observations (authors’ framing)

1. **Scale past 10M tokens** with large quality gains at comparable cost.
2. **REPL is necessary** for long inputs. **Recursion helps most** on
   information-dense tasks (OOLONG / pairs). Depth 0 alone already beats many
   scaffolds on CodeQA / BrowseComp.
3. **Base models degrade faster** as length × task complexity grow. RLMs
   degrade more slowly.
4. **Median cost** of RLM can be lower than base, while **mean cost** is pulled
   up by outlier failed searches.
5. **Longer reasoning**, not only longer input, benefits from programmatic
   decomposition.
6. **Training transfers** across domains and can show **length generalization**
   under RLVR.

## Qualitative behavior

Typical trajectory pattern:

1. Probe `context` (length, samples, structure).
2. Choose a decomposition (chunking, filtering, pair loops).
3. Fan out sub-calls (batched when possible).
4. Aggregate buffers in code.
5. Emit final answer.

Ablations:

- In-context **example trajectories** in the system prompt strongly shape the
  **first** decomposition. First decomposition predicts overall success.
- Trajectories often recover from bad early plans, but recovery is costly.
- Qwen3-Coder trajectories contain more syntax errors even when correct.
  nested RLM depth amplifies that failure mode.

## Related work position

Two long-context families:

1. Change the base architecture / train longer windows.
2. Scaffold around a fixed window model.

RLMs are firmly in (2), but differ from compaction and memory hierarchies by
**delegating all window management to the model’s programs**. They also differ
from task-decomposition recursive agents that still cannot hold / manipulate an
arbitrarily long prompt object.

Close relatives named: CodeAct, ViperGPT, THREAD, ReDel, Context Folding,
AgentFold, DisCIPL, MemGPT-style memory, ReSum compaction, Claude Code
subagents.

## Limitations (paper + appendix)

- Harder natural long-context tasks and **guardrails** are under-explored.
- Risk of **exploding sub-call cost**.
- Blocking sequential sub-calls make wall-clock runtime poor.
- Same system prompt does not transfer cleanly across model families.
- Weak coding models struggle as RLMs.
- Thinking models can exhaust output tokens before finishing a turn.
- Final-answer tagging / structured termination is brittle until models are
  trained as RLMs.
- Local REPL security is not a hard sandbox without isolated environments.

## Conclusion in one sentence

RLMs reframe long context as **environment interaction plus programmatic
recursion**, show strong empirical gains on dense long tasks, and open a
training axis where models learn to be native recursive orchestrators rather
than pure next-token solvers over a stuffed window.
