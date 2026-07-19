# MemoHarness: Agent Harnesses That Learn from Experience — full paper summary

Date: 2026-07-18  
Source: [arXiv:2607.14159v1](https://arxiv.org/abs/2607.14159), source archive and figures  
Paper: _MemoHarness: Agent Harnesses That Learn from Experience_  
Authors: Yue Huang, Wenjie Wang, Han Bao, Yuchen Ma, Xiaonan Luo, Yi Nian,
Haomin Zhuang, Zheyuan Liu, Yue Zhao, and Xiangliang Zhang  
Code named by the paper: [HowieHwong/MemoHarness](https://github.com/HowieHwong/MemoHarness)  
Status: preprint, under review

## Executive summary

MemoHarness treats the **agent harness**—the control layer around a fixed LLM—as
the object to optimize. The harness includes context construction, tools and
retrieval, decoding, multi-call orchestration, memory, and output handling. The
paper argues that optimizing only prompts or workflows leaves much of this
control surface fixed, while deploying one globally tuned harness ignores the
fact that different cases need different amounts of retrieval, reasoning,
memory, and validation.

The proposed system has three parts:

1. A six-dimensional representation makes harness changes structured rather
   than treating the harness as one opaque prompt or code blob.
2. A dual-layer experience bank stores both case-level executions and distilled
   cross-case patterns about successes, failures, and dimension interactions.
3. A one-shot test-time controller retrieves relevant experience and adapts the
   globally selected harness to each unlabeled case, without test labels,
   feedback, gradient updates, or another search loop.

The reported results are promising point estimates. On the paper's
Terminal-Bench protocol, MemoHarness scores **0.806**, versus **0.722** for the
strongest compared baseline, Codex. The validation-selected final harness also
improves over the base harness on LiveCodeBench (**0.900 to 0.967**) and
FinanceAgent (**0.600 to 0.767**). A Terminal-Bench-derived harness transfers
positively to all six additional models tested, with a mean improvement of
**+0.098**, and some learned harnesses transfer to unseen suites. The cost result
is conditional: MemoHarness consumes the most raw input tokens, but its authors
report that most retrieved context is cached, producing a **$6.89** cost versus
**$10.28** for Codex on their 18-task Terminal-Bench evaluation.

The evidence does **not** yet identify which MemoHarness component causes the
gains or establish statistical robustness. The main Terminal-Bench evaluation
contains only 18 held-out tasks, the paper reports no confidence intervals or
significance tests, some baselines are system-level rather than controlled
harness swaps, and the cost advantage depends heavily on cache reuse. The right
reading is therefore: execution experience is a credible substrate for adaptive
harness optimization, but this paper is an encouraging prototype study rather
than a settled recipe or a universal performance claim.

## 1. Problem and thesis

An LLM agent's behavior is a product of both the model and its harness. The
harness decides what the model sees, what it can call, how many calls are made,
what state survives between calls, and what output is finally accepted. The
paper identifies two gaps in prior optimization work:

- Prompt optimizers, LM-program compilers, and workflow search methods improve
  narrower artifacts without jointly editing the whole control layer.
- Even harness-level search generally produces one training-time artifact that
  is reused unchanged for every deployment case.

This creates a coupled search problem. Retrieval changes can alter the best
prompt structure, generation budget, workflow topology, memory policy, and
validator. A benchmark score says whether a run worked but usually does not say
which control surface caused a failure. At deployment time, specialization must
also avoid label leakage: it can use the visible input and prior training
experience, but not the test answer or post-execution feedback.

MemoHarness separates the problem into two phases:

- **Global optimization:** search over harnesses on labeled cases and retain
  structured execution experience.
- **Instance adaptation:** specialize the selected global harness once for each
  unlabeled test case using only frozen, training-derived experience.

## 2. The six-dimensional harness space

The harness is represented as a tuple over six editable dimensions:

| Dimension              | Control stage                    | Typical operations                                      |
| ---------------------- | -------------------------------- | ------------------------------------------------------- |
| D1. Context assembly   | Input construction before a call | Structure instructions, add examples, compress context  |
| D2. Tool interaction   | External tools and retrieval     | Enable retrieval, choose top-k, rerank evidence         |
| D3. Generation control | Decoding                         | Set token budget or temperature, sample candidates      |
| D4. Orchestration      | Workflow topology                | Replace one call with plan/execute/refine stages        |
| D5. Memory management  | State across calls               | Preserve, summarize, or discard prior state             |
| D6. Output processing  | Post-call handling               | Extract an answer, validate a schema, choose a fallback |

The decomposition follows the temporal flow of inference. Its purpose is not to
claim that the dimensions are independent. The paper explicitly treats their
interactions as important. The value is diagnostic: a failed run can be mapped
to a primary and secondary dimension, and a candidate can make a legible change
to a bounded control surface.

## 3. Formal setup and optimization objective

The labeled search cases contain an instruction, visible case features, and a
reference answer. Test cases contain only the instruction and visible features.
Features may describe domain, ambiguity, complexity, or need for external
knowledge.

Executing harness `W` on a case produces:

- a prediction.
- a model/tool trace containing intermediate outputs. And
- runtime diagnostics: model-call count, total tokens, and latency.

The primary reward is the task metric comparing the prediction with the search
reference. **Total token use is the only secondary search cost.** Call count and
latency are recorded for diagnosis but do not enter selection, while dollar
cost is calculated offline from token counts and public model prices.

The final global harness is selected lexicographically: maximize mean task
reward first, then minimize mean token use only among candidates tied on the
primary reward. A separate feasibility filter may enforce a resource budget.
This “correctness first, cost as tiebreaker” rule is meant to prevent the search
from preferring a cheap but less correct harness.

## 4. Dual-layer experience bank

The experience bank is a typed pair rather than a single undifferentiated
memory.

### Per-case layer

For every case and search iteration, MemoHarness records:

- case and iteration identifiers.
- visible case features.
- the applied harness.
- the configuration delta from the harness most recently used on that case.
- the full execution trajectory.
- reward and token cost. And
- a structured diagnosis.

The diagnosis includes a success bit, one primary failure dimension (or none),
secondary contributing dimensions, and natural-language analysis. The bank also
tracks lightweight statistics such as consecutive failures, recent mean reward,
reward trend, and dimension-level failure counts.

### Global layer

A distillation operator converts recurring failure clusters into global
patterns. Each pattern describes the recurring phenomenon, its supporting
evidence, and the expected effect of a targeted harness change. The controller
queries the bank by case features, iteration, failure statistics, or dimension
and receives a bounded slice containing selected entries, global patterns, and
aggregates. This retrieval boundary keeps controller context from growing with
the entire bank.

In the reference implementation, verifier results, exceptions, timeouts,
missing artifacts, command failures, and traces are mapped heuristically to one
of the six dimensions. The diagnosis is deliberately coarse. Repeated evidence
is expected to create more useful structure at the global-pattern layer.

## 5. Training-time harness search

Search begins from a deliberately minimal harness: no demonstrations, no
structured instruction scaffolding, no external tools, deterministic one-call
generation, no cross-call memory, and raw output passthrough. The authors' goal
is for every added behavior to be justified by observed execution evidence.

At each outer iteration, the controller:

1. constructs a query from the current harness and accumulated bank.
2. retrieves a bounded evidence slice.
3. proposes a new six-dimensional harness.
4. executes it on every labeled search case.
5. records rewards, costs, trajectories, and diagnoses. And
6. periodically or opportunistically distills repeated failures into global
   patterns.

The implementation represents a candidate as a **harness bundle**: a structured
D1–D6 policy plus textual operating rules, a persistent playbook, and the
currently scoped distilled memory. A change updates both the typed dimension
state and the text that realizes it.

The reported controller-bank loop runs for **10 outer iterations**. It receives
compact summaries of **10 recent successes and 10 recent failures**. The
appendix describes a dual distillation trigger after **5 new bank entries** or
**3 consecutive failures on one case**, whichever comes first. All reported
generation is deterministic: temperature `0.0`, top-p `1.0`, one candidate, and
an 8,192-token maximum generation budget.

## 6. Test-time case adaptation

At evaluation time, the experience bank is frozen and there is no iterative
feedback loop. For a new case, MemoHarness retrieves:

- the most instruction-similar successful historical cases.
- the most instruction-similar failed historical cases.
- feature-conditioned bank entries and statistics. And
- the global patterns.

Instruction similarity is defined as cosine similarity between instruction
representations. A test-time controller uses the retrieved evidence to transform
the global harness into a case-specific harness, then executes that harness once
to produce the final answer. A simple case can therefore remain lightweight,
while a retrieval-heavy, multi-step, or format-sensitive case can receive richer
orchestration.

If labels arrive after evaluation, the trajectory may later be appended for
future use, but the current test run does not learn, reselect a global harness,
or distill new patterns. This boundary is central to the paper's claim that the
adaptation does not use test-time feedback.

## 7. Experimental design

The primary search model is **GPT-5.3-Codex**. Three source benchmarks cover
different workloads:

- **Terminal-Bench:** long-horizon shell work with tools, files, and process
  management. The 89 tasks are split 80/20, producing 18 held-out tasks.
- **LiveCodeBench:** recent competitive-programming problems emphasizing
  single-shot code generation.
- **FinanceAgent:** multi-step analytical reasoning over financial documents
  and tool calls.

The split uses seed `42` and is held fixed across repeated runs. The paper says
it reports mean task success over repeated runs and evaluates the
validation-selected harness rather than the highest held-out checkpoint. It
does not state the number of repeated runs in the paper.

Terminal-Bench baselines are Terminus, OpenCode, Claude Code, and Codex. When a
framework permits the generator to be swapped, the authors use GPT-5.3-Codex.
otherwise they use the closest released configuration. Consequently, some
comparisons are closer to controlled harness comparisons and others are
comparisons between full released systems.

Cross-dataset transfer uses MMMLU, HumanEvalFix, StrongReject,
Reasoning-Gym-Easy, LawBench, and SWE-Bench Pro. Cross-model transfer uses the
search-derived Terminal-Bench harness without retraining on Claude Sonnet 4.6,
Gemini 3.1 Pro, Qwen3.5-397B-A17B, GLM-5, GPT-4.1, and DeepSeek-V3.2.

## 8. Results

### RQ1: Absolute Terminal-Bench performance

| Harness/system  | Mean success |
| --------------- | -----------: |
| Terminus-2      |        0.361 |
| Claude Code     |        0.389 |
| OpenCode        |        0.556 |
| Codex           |        0.722 |
| **MemoHarness** |    **0.806** |

MemoHarness improves over Codex by **+0.084** and over the remaining compared
systems by **+0.250 to +0.445**. Codex is already terminal-specialized, making
it the strongest and most relevant baseline in this experiment. The paper is
appropriately cautious that not every row isolates the harness under the same
model and runtime.

### RQ2: Search progress

The validation-selected final harness improves on the starting harness on all
three source benchmarks:

| Benchmark      |  Base | Final | Absolute change |
| -------------- | ----: | ----: | --------------: |
| Terminal-Bench | 0.722 | 0.806 |          +0.084 |
| LiveCodeBench  | 0.900 | 0.967 |          +0.067 |
| FinanceAgent   | 0.600 | 0.767 |          +0.167 |

The in-search peak is not always the selected result. Terminal-Bench reaches
0.833 at checkpoint 4, and LiveCodeBench reaches 1.000 at checkpoint 3, but the
final harness is chosen without peeking at the held-out peak. The per-iteration
curves show different regimes: FinanceAgent climbs from 42.5% at iteration 1 to
65.0% at iterations 8 and 9, while LiveCodeBench stays in a narrow 91.2%–95.0%
band after beginning near the model's ceiling. The authors interpret this as
more remaining harness-repair opportunity in long-horizon analytical work than
in nearly saturated single-shot code generation.

### RQ3: Transfer to unseen datasets

Each source-specific harness is applied to six unseen suites under the shared
GPT-5.3-Codex model:

| Search source / harness    | MMMLU | HumanEvalFix | StrongReject | Reasoning-Gym-Easy | LawBench | SWE-Bench Pro |
| -------------------------- | ----: | -----------: | -----------: | -----------------: | -------: | ------------: |
| Untuned Codex baseline     | 0.818 |        1.000 |        0.879 |              0.947 |    0.675 |         0.706 |
| Terminal-Bench MemoHarness | 0.848 |        1.000 |        0.909 |              0.947 |    0.676 |         0.765 |
| FinanceAgent MemoHarness   | 0.818 |        1.000 |        0.909 |              0.947 |    0.682 |         0.706 |
| LiveCodeBench MemoHarness  | 0.879 |        1.000 |        0.909 |              0.947 |    0.669 |         0.706 |

Transfer is **selective**, not universal. The Terminal-Bench harness has the
broadest positive effect, including +0.059 on SWE-Bench Pro. The FinanceAgent
harness mainly helps StrongReject and LawBench. The LiveCodeBench harness helps
MMMLU and StrongReject but slightly lowers LawBench. HumanEvalFix and
Reasoning-Gym-Easy are already saturated and do not move. The authors suggest
that robust control decisions learned on long-horizon, tool-heavy work may
transfer, but they do not isolate which decisions produced each change.

### RQ4: Transfer across base models

The Terminal-Bench harness learned with GPT-5.3-Codex is applied without new
search to six other models:

| Model             |  Base | MemoHarness | Change |
| ----------------- | ----: | ----------: | -----: |
| GPT-5.3-Codex     | 0.722 |       0.806 | +0.084 |
| Claude Sonnet 4.6 | 0.530 |       0.583 | +0.053 |
| Gemini 3.1 Pro    | 0.611 |       0.694 | +0.083 |
| Qwen3.5-397B-A17B | 0.444 |       0.528 | +0.084 |
| GLM-5             | 0.500 |       0.733 | +0.233 |
| GPT-4.1           | 0.500 |       0.538 | +0.038 |
| DeepSeek-V3.2     | 0.333 |       0.444 | +0.111 |

Every tested model improves, with a mean gain of **+0.098**. The range is wide:
GPT-4.1 gains only +0.038 while GLM-5 gains +0.233. Because the harness is not
re-searched per model, this is evidence that at least some learned execution
policy transfers beyond source-model prompt quirks. It remains a one-benchmark,
small-split transfer result rather than proof of broad model independence.

### RQ5: Inference cost

The cost comparison covers the 18 held-out Terminal-Bench tasks. Token values
are millions, and dollar cost is calculated from the reported public
GPT-5.3-Codex prices.

| Harness/system  |      Input | Cached input | Non-cached input |    Output |      Cost |
| --------------- | ---------: | -----------: | ---------------: | --------: | --------: |
| Codex           |      8.23M |        4.33M |            3.90M |     0.19M |    $10.28 |
| Terminus        |      3.96M |        0.94M |            3.03M |     0.09M |     $6.68 |
| Claude Code     |      7.32M |        3.11M |            4.21M |     0.11M |     $9.51 |
| OpenCode        |      5.48M |        5.07M |            0.41M |     0.05M |     $2.34 |
| **MemoHarness** | **14.18M** |   **13.32M** |        **0.86M** | **0.22M** | **$6.89** |

MemoHarness has by far the largest raw context because it retrieves experience,
but approximately 94% of that input is counted as cached in these runs. Under
that accounting it costs less than Codex and Claude Code while scoring higher.
Terminus and OpenCode remain cheaper but less accurate. The result should not be
generalized to providers or workloads with lower cache reuse.

## 9. Operation-level diagnostic

The appendix looks beyond scalar curves by examining adjacent iterations on the
same task. For each shell operation newly appearing in the later harness
output, it asks how often reward also increases. The all-transition positive
baseline is about 13.2%.

The strongest reported associations are `cat` (8 positive transitions among 11
additions. +59.5 Percentage-point lift), `sed` (4/11, +23.2 pp), `which` (5/15,
+20.2 pp), and `test` (14/46, +17.3 pp). `pip`, `python3`, `strings`,
`pdftotext`, and `head` also have positive lift. `grep`, `echo`, and `curl` are
below baseline, while several rare operations have zero positive examples.

This analysis illustrates why retaining traces is useful: the optimizer can
identify concrete inspection or condition-checking behaviors associated with
improvement. It is **correlational**, however. Counts are often small, newly
adding an operation may proxy for a larger harness change, and the paper reports
no uncertainty or multiple-comparison control. The table should guide new
hypotheses, not be read as a causal ranking of shell commands.

## 10. Position relative to prior work

The paper organizes prior work into two lines:

- **Optimization for agents:** ReAct, Toolformer, Tree of Thoughts, LATS, OPRO,
  ProTeGi, Promptbreeder, Self-Refine, Reflexion, DSPy/MIPRO, TextGrad,
  AutoFlow/AFlow, Meta-Harness, and AlphaEvolve. These methods optimize prompts,
  reasoning, LM programs, workflows, or harness code, but generally produce a
  pre-deployment artifact rather than a per-case harness adapted from stored
  execution experience.
- **Harness engineering:** practitioner guidance from Anthropic, LangChain, and
  OpenAI, plus Meta-Harness, Natural-Language Agent Harnesses, Terminal-Bench,
  SWE-agent, and OpenHands. This work establishes context, tools, interfaces,
  and runtime scaffolding as first-class performance determinants.

MemoHarness's claimed novelty is the combination of structured harness search,
reusable diagnostic experience, and feedback-free case-level adaptation at
test time. Meta-Harness is the closest prior optimizer, but the paper excludes
it from direct experiments because suitable public code was not available when
the experiments were finalized. Later reference code was not incorporated.

## 11. Limitations and critical reading

The authors explicitly acknowledge five main limitations:

1. Terminal-Bench uses only 18 held-out tasks, with point estimates rather than
   confidence intervals or significance tests.
2. Some baseline comparisons cannot hold both model and runtime fixed.
3. The experience bank, global patterns, and test-time adaptation are not fully
   ablated from one another.
4. The favorable cost result depends on retrieved context remaining highly
   cacheable.
5. The controller, diagnostic mapper, and distiller use practical heuristics,
   not a learned or proven general controller.

The paper's own caveats are material. Several additional details also limit
replication or interpretation from the paper alone:

- It says results are averaged over repeated runs but does not give the run
  count or variance.
- It repeatedly calls the final harness “validation-selected,” while the split
  description names an 80/20 train/evaluation partition but does not clearly
  define a separate validation set.
- The case-feature extractor, instruction representation, test-time neighbor
  count, controller/distiller prompts or models, and exact case-adaptation edit
  constraints are not fully specified in the text.
- The appendix says the controller summaries use 10 recent successes and 10
  recent failures with semantic retrieval disabled (`D2.top_k=0`), while the
  formal test-time method depends on cosine-similarity Top-K retrieval. The
  relationship between these two retrieval settings and the actual test-time K
  needs clearer documentation.
- No experiment isolates a static globally optimized harness from the
  case-adapted harness, so the central incremental value of test-time adaptation
  remains unquantified.
- The operation-level table is useful exploratory evidence but does not provide
  causal attribution.

Accordingly, the strongest defensible conclusion is narrower than “adaptive
harnesses always win”: under this protocol, a structured, experience-informed
harness optimizer produces better point estimates than the compared fixed
harnesses and exhibits some transfer. Larger splits, uncertainty estimates,
component ablations, cold-cache cost studies, and more complete controller
specification are needed before treating the result as robust.

## 12. OpenAgents relevance

The codebase-specific Blueprint integration audit and recommended delivery
sequence are documented in
[`2026-07-18-memoharness-blueprint-integration-analysis.md`](./2026-07-18-memoharness-blueprint-integration-analysis.md).

This paper reinforces the earlier OpenAgents research note on evolving the
harness before changing model weights:
[`2026-07-04-harness-optimization-evolve-the-harness-audit.md`](./2026-07-04-harness-optimization-evolve-the-harness-audit.md).
MemoHarness adds a particularly relevant idea: preserve execution evidence in a
typed, queryable form and use it for bounded case adaptation, rather than merely
promoting one globally improved harness.

The ideas worth carrying forward are:

- Treat a harness as a versioned policy bundle with explicit context, tool,
  generation, orchestration, memory, and output dimensions.
- Store candidate deltas, trajectories, verifier outcomes, costs, and diagnoses
  together so an improved score remains attributable and auditable.
- Separate per-case evidence from distilled global patterns and keep retrieval
  bounded.
- Rank correctness before cost, while still recording exact token, cache, call,
  and latency evidence.
- Keep test-time specialization label-free and policy-constrained. Do not let
  adaptation become an untracked online mutation path.

Adoption should require stronger gates than this paper demonstrates: strict
candidate diffs, a real validation split, repeated-run uncertainty, static
global-versus-adapted ablations, cross-family transfer checks, warm- and
cold-cache cost receipts, redaction boundaries for stored traces, and explicit
promotion authority. This document is a research summary, not an adoption or
runtime-authority decision.

## Bottom line

MemoHarness presents a coherent architecture for turning agent executions into
reusable harness knowledge. Its most important conceptual move is the
separation of **global harness learning** from **one-shot case-specific
adaptation**, backed by a bank that remembers both individual trajectories and
cross-case patterns. The experiments support continued investigation: the
point-estimate gains are consistent across the three source benchmarks, some
changes transfer to unseen suites, and the Terminal-Bench harness helps every
tested base model. But the small evaluation, missing error analysis, incomplete
ablations, mixed baseline control, and cache-sensitive economics mean the paper
should be used as a design hypothesis and experimental template—not yet as a
general performance law.
