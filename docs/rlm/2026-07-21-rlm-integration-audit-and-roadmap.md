# Recursive Language Models — Integration Audit And Roadmap For OpenAgents

**Date:** 2026-07-21
**Lane:** research and analysis. This document has no dispatch authority. It
flips no promise state, admits no issue, and changes no runtime behavior.
**Authorities:** `AGENTS.md`, `INVARIANTS.md`, `docs/sol/MASTER_ROADMAP.md`
(revision 126), and live issue state stay the factual and sequencing
authorities. The RLM issue list in section 8 is a candidate program. It needs
Sol admission and owner acceptance before any work starts.
**Owner framing:** integrate RLM where it fits. The flagship use case is smart
traversal of long context and conversation history WITHOUT the lossy
compaction that most harnesses use.


> **OWNER DECISION AMENDMENT (2026-07-21, after initial publication).** The
> owner rejected the Python leaf executor. The RLM engine is implemented
> **Effect-native**: a bounded typed-operation agent loop over the corpus with
> an injected `effect/unstable/ai` `LanguageModel` root (see #9140). The
> paper's Python REPL becomes a fail-closed typed operation vocabulary — the
> Tier D operations plus `Subcall` and `Answer` — decoded per iteration with
> Effect Schema. §5.1's Python placement and the §8 line that excluded an
> Effect port are SUPERSEDED by this decision. Caps, honesty, privacy, and
> authority boundaries are unchanged.

## Sources

Read in this order. Paths are exact.

1. Paper LaTeX source — `~/Downloads/arXiv-2512.24601v3/` (`main.tex`,
   `sections/*.tex`, `appendix/*.tex`, `tables/*.tex`). arXiv 2512.24601v3,
   "Recursive Language Models", MIT CSAIL.
2. Existing repo notes — `docs/research/rlm/` (`README.md`,
   `paper-summary.md`, `paper-analysis.md`, `repo-analysis.md`,
   `openagents-implications.md`, `source.md`) and the decision audit
   `docs/research/2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md`.
3. Reference implementation — local clone `~/work/projects/repos/rlm`
   (`rlms` 0.1.3, pinned commit `72d6940142dd`). The `rlm-minimal` companion
   repo is NOT cloned locally at this date.
4. Runtime docs this must integrate with —
   `docs/desktop/2026-07-21-openagents-desktop-chat-runtime-reference.md`
   (the chat runtime map) and
   `docs/fable/2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md`
   (STREAM-01..07). Also `packages/agent-harness-contract/README.md`
   (HARN-01..08, epic #9115) and
   `docs/fable/2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md`.

Repo state at write time is `main` at `202a0bb963`, clean before this file.

---

## 1. Paper distillation

This section is a precise mechanical account from the LaTeX source. Rendered
section numbers follow the order in `main.tex`. LaTeX labels appear in
parentheses so a future reader can grep the source.

### 1.1 The inference paradigm (§2, label `sec3:rlm`)

An RLM is an inference-time scaffold around a base model `M` with window `K`.
The external interface is unchanged — a string prompt in, a string response
out. The internals differ in one decisive way. The prompt `P` never enters the
root context window. The RLM initializes a persistent REPL environment with
`P` bound to a variable and a sub-call function available as a module. The
root model receives only constant-size metadata about `P` — its length, a
short prefix, and access instructions (§2, Algorithm 1).

The loop per iteration:

1. The root model emits code.
2. The REPL executes the code and updates persistent state.
3. Only constant-size metadata about stdout — a short prefix and the length —
   returns to the root history. The reference implementation truncates stdout
   at about 20K characters.
4. When the code sets the final-answer state, the loop stops and that value is
   the response.

The stdout starvation is deliberate. It forces the model to keep long strings
in variables and sub-calls instead of re-importing the corpus into its own
window. A footnote in §2 gives the bound — with each turn trimmed to `c`
tokens, the root gets at most `K/c` iterations, and each iteration can launch
arbitrarily many sub-calls.

The paper names three design choices that separate an RLM from a
superficially similar code-plus-subagent scaffold (§2, the "bad algorithm"
contrast):

1. **Symbolic handle.** `P` lives in the environment, never in root history.
2. **Symbolic output.** The final answer can be a REPL variable of arbitrary
   length. The model does not have to generate the answer inside its window.
3. **Symbolic recursion.** Code inside the environment can invoke the model
   on programmatically constructed slices of `P`, inside loops. This permits
   `Ω(|P|)` or `Ω(|P|²)` semantic work. A scaffold that has a code tool AND a
   separate verbal sub-agent tool cannot do this. It can only delegate the few
   sub-tasks the root verbalizes.

### 1.2 Exact mechanics — REPL surface, depth, sync behavior (§2, §3.2, Appendix C)

What the REPL exposes (system prompts in Appendix C, label `appx3:methods`):

- `context` — the prompt variable. String or chunk list. The system prompt
  states the type, the total character count, and the chunk lengths.
- `llm_query(prompt)` — a one-shot sub-LM call. In the GPT-5 experiments the
  sub-model handles about 500K characters per call.
- `rlm_query(context, query)` — present only when max depth exceeds 1. It
  spawns a full child RLM loop with its own REPL. At the depth limit it falls
  back to `llm_query` automatically.
- `print(...)` — the only channel back to the root, truncated.
- A final-answer protocol. The paper prompts use `FINAL(...)` and
  `FINAL_VAR(name)`. The open-source package moved to setting
  `answer["content"]` plus `answer["ready"] = True` inside the REPL.

Depth is a first-class knob (§3.2, label `sec4.2-methods`). Depth 0 is REPL
only, no sub-calls. Depth 1 allows sub-LM calls. Depth above 1 allows
sub-RLMs. The paper default is depth 1. The GPT-5 experiments use GPT-5 as
the root and GPT-5-mini as the sub-model — a deliberate heterogeneous split
for cost.

Sub-calls in the paper implementation are **blocking and sequential** (§4
Observation 4 caveat, Appendix B). The authors state that asynchronous
sub-calls and sandboxed REPLs are the main future levers for runtime and cost
(§7). The open-source package adds threaded batching
(`max_concurrent_subcalls`, default 4, with `llm_query_batched` and
`rlm_query_batched`) but the root loop stays turn-serial.

What returns to the caller — a single response string, plus (in the package)
a trajectory record with per-iteration code, truncated stdout, sub-call
metadata, and usage. On a budget, timeout, token, or error-threshold stop the
package prefers returning a best partial answer over raising.

### 1.3 Benchmarks and results (§3, §4, Table 1)

Task ladder, chosen so processing complexity scales differently with length
(§3.1, label `sec4.1-experimental-setup`):

| Task | Length in Table 1 | Complexity versus length |
| --- | --- | --- |
| S-NIAH | scaled 2^13..2^20 | constant (one needle) |
| LongBench-v2 CodeQA | 23K–4.2M tokens | fixed file set per instance |
| BrowseComp-Plus (1K docs) | 6M–11M tokens | multi-hop over a constant document count |
| OOLONG `trec_coarse` | 131K tokens | linear — nearly every line matters |
| OOLONG-Pairs | 32K tokens | quadratic — nearly every pair matters |

Headline numbers (Table 1, GPT-5 with RLM sub-calls to GPT-5-mini):

- CodeQA — base 24.0, compaction 58.0, RLM depth 1 62.0, depth 2 66.0.
- BrowseComp-Plus — base 0.0 (cannot fit), compaction 70.5, RLM depth 1 91.3.
  OpenCode with context offloading scores 94.0 and beats the RLM here.
- OOLONG — base 44.0, compaction 46.0, RLM depth 1 56.0, depth 3 58.0.
- OOLONG-Pairs — base 0.1, compaction 0.1, Claude Code 0.1, RLM depth 1 58.0,
  depth 3 76.0 F1.

Abstract medians against GPT-5 baselines — +26% versus compaction, +130%
versus CodeAct with sub-calls, +13% versus Claude Code.

The six observations (§4, label `sec4.3-results`):

1. RLMs scale into the 10M+ token regime at comparable or lower cost. On
   BrowseComp-Plus the RLM averages $0.99 per task where a linear
   extrapolation of GPT-5-mini ingesting 6M–11M tokens is $1.50–$2.75.
2. The REPL offload alone (depth 0) already beats most scaffolds on long
   inputs. Recursion adds its largest gains on information-dense tasks. For
   Qwen3-Coder on CodeQA, depth 0 beats every sub-calling variant.
3. Base-model quality degrades as a function of length TIMES task complexity.
   The RLM curve degrades far more slowly (Figure 1). GPT-5 stays strong on
   S-NIAH at 2^20 but falls early on OOLONG and collapses on OOLONG-Pairs.
4. Median RLM cost is often below the median base-model cost. Mean cost is
   higher because of long-tail outlier trajectories (Appendix F quartile
   plots). Runtime numbers are implementation-bound because all calls block.
5. RLMs also help long reasoning, not only long input. On LongCoT-mini,
   RLM(GPT-5.2) with decomposition hints beats base GPT-5.2 by 69.5% overall.
   The hint prompt (Appendix C, `env_tips`) is an orchestration recipe —
   extract a DAG of sub-problems, solve layer by layer with batched
   sub-calls, memoize verified answers in a dict, verify before propagating,
   assemble by lookup only.
6. Training transfers. RLM-Qwen3-8B and the MRCRv2 RLVR run show domain and
   length generalization (details below).

Trajectory analysis (§5, label `sec4.4-qualitative`) — the first
decomposition attempt strongly predicts success. In-context example
trajectories in the system prompt shape it, even when the example task is
unrelated. Qwen3-Coder trajectories carry many more syntax errors than GPT-5
trajectories, even when correct, and depth above 1 propagates those errors
into children — this is why deeper recursion HURTS Qwen3-Coder on average.
Sub-call counts vary widely (Appendix F) — Qwen3-Coder averages about 500
sub-calls on correct OOLONG rollouts.

### 1.4 Cost characteristics — when RLM is cheaper and when it is not

From §4 Observation 4, Appendix F, and Table 1 cost columns:

- **Cheaper or comparable** — median case on long inputs, because the root
  reads metadata and slices instead of the whole corpus, and because leaves
  can be a cheaper model. Past the window limit the comparison is not even
  possible for a single call.
- **Pricier** — the tail. Failed search paths, bad first decompositions, and
  per-line sub-calling produce high-variance, long-tailed cost distributions.
  Depth raises variance (GPT-5 OOLONG depth 2 shows ±$3.25 on a $1.10 mean).
  Cost scales with task complexity by design — a quadratic task buys
  quadratic sub-call work.
- **Not competitive** — short prompts with low density, where one direct call
  inside the effective window is both cheaper and faster. The RLM also pays a
  wall-clock penalty while sub-calls are sequential.

Any product use therefore needs hard caps (budget, tokens, timeout, depth,
iterations), honest partial-answer semantics, and cost display in the UX.

### 1.5 Failure modes the paper admits (§7, Appendix B)

- Guardrails and harder natural long-context evaluations are under-explored.
- Sub-call costs can explode structurally.
- Blocking sequential sub-calls make wall-clock time poor.
- One system prompt does not transfer across model families. Qwen3-Coder
  needed an added warning against thousands of tiny sub-calls.
- Models with weak coding ability fail as RLM roots.
- Thinking models can exhaust per-call output tokens mid-trajectory.
- The final-answer tag protocol is brittle until models are trained for it.
- The local REPL is not a security boundary. Isolated environments are.

### 1.6 The trained variant (§3.2, §4 Observation 6, Appendix A)

RLM-Qwen3-8B is Qwen3-8B fine-tuned on about 1,000 filtered root-turn samples
distilled from RLM(Qwen3-Coder-480B-A35B) trajectories on LongBenchPro — a
domain unrelated to the evaluation suite. The key insight — leaf sub-calls
are ordinary LM requests, so training can focus on the ROOT skill of REPL
manipulation and sub-call judgment. Filtering removed zero-score and one-turn
trajectories, cut turns past the 8B context limit, and programmatically
repaired common template mistakes (16% of turns misused `FINAL`, 13% misused
`FINAL_VAR`). Training used `prime-rl`, batch 64, 300 steps, about 48 H100
hours. Result — a median +28.3% over the base 8B as an RLM across the four
tasks, with 3x faster trajectories from fewer mistakes. A separate RLVR run
trained Qwen3-4B on the short MRCRv2 split and generalized to the 1M-token,
8-needle split. This matters to OpenAgents as a future option — a small local
orchestrator model is trainable at modest cost once we have our own
successful trajectories, but nothing in the current program depends on it.

---

## 2. Corrections and extensions to `docs/research/rlm`

The existing notes are accurate on the mechanism, the results tables, the
repo architecture, and the hybrid stance. Full-source reading confirms them.
The following are the material deltas.

1. **Density at SHORT lengths is under-emphasized.** OOLONG-Pairs is only 32K
   tokens and OOLONG is 131K — both fit easily in frontier windows, and base
   models still fail (0.1 F1 on Pairs). The notes frame RLM mostly as a
   beyond-10M capability. The stronger product lesson is the opposite end —
   RLM-style traversal wins on DENSE tasks at sizes our thread histories
   already reach today. This is the direct justification for the
   history-recall thesis in section 4.
2. **The LongCoT `env_tips` recipe is missing from the notes.** Appendix C
   contains a complete orchestration prompt — DAG extraction, layer-by-layer
   batched sub-calls, memoized verified answers, verify-before-propagate,
   assemble-by-lookup. It is the most directly reusable artifact in the paper
   for Full Auto style decomposition and is not summarized anywhere in
   `docs/research/rlm`.
3. **Depth is task-dependent even for strong roots.** `paper-summary.md` says
   deeper recursion helps GPT-5 "more reliably". Table 1 is more nuanced —
   GPT-5 CodeQA falls back to 58.0 at depth 3 after peaking at 66.0 at depth
   2, while Pairs climbs monotonically to 76.0. Depth is a per-task-class
   knob, not a general dial. Our default policy should be depth 0 to 1 with
   depth as measured escalation, which the notes also recommend but for the
   weaker reason (Qwen error propagation only).
4. **Per-iteration re-prompting detail.** The reference loop appends a fresh
   bounded user prompt every iteration ("fully prefixed trajectory" in
   `rlm/core/rlm.py`) carrying iteration count, context count, and history
   count. The notes describe history assembly loosely. This matters for any
   Effect port of the loop shape — the root history is system + metadata +
   alternating (user, assistant, truncated-repl) turns, optionally compacted
   at 85% of the root window by the package's own opt-in compaction.
5. **LongCoT used a different harness.** The Observation 5 experiment ran on
   Prime Intellect's `rlm-harness` fork with a file-based final answer
   (`/task/answer.txt`), not the Table 1 implementation (Appendix C). Do not
   treat the LongCoT numbers as produced by the pinned `rlms` package.
6. **`rlm-minimal` is not in the local reference set.** `source.md` links it.
   No local clone exists under `~/work/projects/repos/`. Integration work
   should pin the main `rlm` clone only, or add the lane to
   `projects/manifest.txt` first.

The 2026-06-28 hybrid decision stands and this audit builds on it — Python
RLM as a leaf executor on sandbox-capable tiers, Effect as the online
authority, and no RLM REPL inside the multi-tenant Cloud Run monolith. One
update to that audit's framing is required — its online substrate references
(Cloudflare Workers, D1, R2, Durable Objects) are retired. The current
authority is Google Cloud, and the sandbox tier below is now concretely the
harness sandbox-provider contract, which did not exist in June.

---

## 3. What the current runtime already gives us

The desktop chat runtime reference and the harness contract define the exact
substrate RLM integration must respect. The relevant facts:

- **Two turn engines.** Stack A (`makeProviderLaneDispatcher`,
  `apps/openagents-desktop/src/provider-lane.ts`) runs production coding
  turns on the lanes (codex-local, claude-local, Grok ACP, Cursor ACP).
  Stack B (`TurnService` in `packages/agent-turn-runtime`) runs Apple FM
  routed turns and delegation. They meet at `runDelegateLaneTurn`
  (`main.ts:1431`).
- **Bounded prompt history today.** The thread store caps every thread at
  `maxNotes = 80` (`apps/openagents-desktop/src/thread-store.ts:19`). The
  claude-local lane prepends a bounded 12-message `historyPrompt` window when
  no provider session resumes (`claude-local-runtime.ts:488-503`). Provider
  runtimes compact natively, and the harness contract carries an explicit
  `compact` verb (`packages/agent-harness-contract/src/session.ts`). Every
  one of these is lossy by construction.
- **A durable, cursor-exact, never-compacted event log already exists.**
  `harness-event-recorder.ts` observes every dispatched turn event, projects
  it to the neutral `KhalaRuntimeEvent` union
  (`packages/agent-runtime-schema/src/index.ts:740-927`), and appends it to a
  per-turn `HarnessEventLogStore` that rejects non-increasing sequences
  (`packages/agent-harness-contract/src/event-log-store.ts:84-91`). Replay
  and live attach at an exact cursor are shipped (HARN-02). The recorder is a
  pure observer and never disturbs dispatch.
- **Suspend and continue at an exact cursor.** The slice runner
  (`packages/agent-harness-contract/src/slice-runner.ts`, HARN-06) time-boxes
  a turn by event budget and resumes from `continueFrom` with no gap and no
  duplicate.
- **A sandbox-provider contract with a real local rung.**
  `HarnessSandboxProvider` (HARN-07) is the fail-closed workspace and
  command-execution port. `local-process-sandbox-provider.ts` is a REAL
  provider over the host filesystem and `child_process`. A managed sandbox
  implements the same port later.
- **Known neutral-log gap.** The projection covers only the seven core kinds
  (`harness-projection.ts:201-207`). Plan, meter, question, child, and notice
  facts do not reach the neutral log at HEAD. Any recall corpus built on the
  neutral log must state this bound honestly, and the corpus builder should
  join the thread store notes to fill the display-side gaps.

The important synthesis — OpenAgents already persists exactly the artifact
RLM traversal needs. The paper offloads the prompt into a REPL variable
because the prompt does not fit the window. We already offload the entire
conversation history into a durable, ordered, replayable log for other
reasons (crash recovery, redaction, exact usage). The RLM insight applied
here is that this log should be READ the way an RLM reads its context
variable — probed, sliced, grepped, and sub-called over — instead of being
truncated into a bounded prompt window or summarized by compaction.

---

## 4. Fit analysis

### 4.1 The flagship — history traversal without lossy compaction

Today a lane turn sees at most a bounded window (12 messages on claude-local
history assembly, 80 notes in the store) or whatever the provider's own
compaction has retained. A question like "what did we decide about X 400
turns ago" fails structurally — the deciding turn left every window long ago.
Compaction cannot fix this. Compaction presumes some early detail may be
forgotten, and the paper shows compaction scoring 0.1 F1 on pair-dense tasks
(Table 1) precisely because the needed detail is gone.

The RLM alternative maps cleanly onto shipped modules:

- **The context variable** is a deterministic export of the full thread
  history — the neutral `KhalaRuntimeEvent` logs for every turn of the
  thread, joined with the thread store notes and, where present, provider
  session transcript projections. Call this the **history corpus**. It is
  never compacted. Cursors (turn ref plus `sequence`) are its citation
  scheme.
- **The root loop** is a recall engine that probes the corpus with cheap
  deterministic operations first (length, structure, grep, time slicing) and
  escalates to sub-model calls over selected slices only when semantics
  demand it. This is exactly the paper's depth-0 versus depth-1 split, and
  the paper's Observation 2 says depth 0 alone already carries much of the
  value.
- **Recursive sub-calls** map to bounded slice questions — "label each turn
  in this range", "find the decision about X in these 30 turns and quote it".
  Buffers aggregate in code. The final answer is a bounded, CITED result —
  answer text plus the exact cursors it derived from.
- **Suspend and continue compose naturally.** A long recall over a large
  corpus is itself a long turn. The slice runner already lets a long
  traversal run as time-boxed slices that suspend at an exact cursor and
  resume later, so recall work survives short process lifetimes the same way
  coding turns do. Recall sub-calls are leaf work inside a slice.

Concrete surfaces this unlocks, in value order:

1. **History recall as a host tool.** Every lane (Codex, Claude, Grok,
   Cursor, Apple FM) gets a `history_recall` host tool in the harness
   host-tool vocabulary (`packages/agent-harness-contract/src/host-tool.ts`).
   The model asks a question about the past instead of receiving a truncated
   window. The tool answers with cited spans. This benefits every turn
   without changing any lane's prompt budget.
2. **Long Full Auto runs.** `full-auto-run-registry.ts` already consumes
   cursor-exact liveness from the recorder. A run that has executed for many
   turns can ask the recall engine "what has been tried, what failed, what
   was the owner's last constraint" over the run's own event history instead
   of re-deriving state from a bounded prompt. This directly attacks the
   long-horizon drift the paper's LongCoT `env_tips` recipe addresses —
   memoize verified facts, never trust the window.
3. **Owner memory across threads.** "What did we decide about X" spans
   threads. The corpus builder can scope to one thread, one run, or an
   owner-wide slice of threads. Owner-wide scope stays owner-local and
   respects per-event `visibility` and `redactionClass`.
4. **Evidence packs and workroom analysis.** ProductSpec workroom evidence
   and receipt chains are long, dense, and never compactable. An offline
   recall pass ("list every packet whose verification cites test T") is
   OOLONG-shaped work. The output is an untrusted analyst summary with
   citations, never a verification verdict.
5. **Repo-scale context questions.** CodeQA-style questions over a checkout
   are the weakest marginal fit — the paper itself shows coding agents with
   file offloading are already competitive there (OpenCode 94.0 versus RLM
   92.0 on BrowseComp-Plus, and depth 0 winning CodeQA for Qwen). Our lanes
   ARE coding agents with file access. Do not build a parallel RLM path for
   what Codex already does over files. The corpus tool should serve history,
   logs, and evidence — artifacts that are NOT files in the workspace.

### 4.2 Where RLM does not belong

- **The latency-sensitive chat hot path.** An RLM trajectory takes seconds to
  minutes with a heavy tail. First-token latency for an ordinary chat turn
  must not wait on a traversal. Recall runs as an explicit tool call with
  visible progress, or as a background pass — never as a hidden pre-step on
  every turn.
- **Authority, verification, and release surfaces.** A recall answer is an
  untrusted candidate with citations. It never becomes a `RouteDecision`, a
  workroom verification, a promise transition, a payment action, or a public
  claim. This is the standing evidence-only boundary from the 2026-06-28
  audit and `INVARIANTS.md`.
- **In-process in the Cloud Run monolith.** The reference implementation is
  Python `exec`. Its local environment is explicitly not a tenant boundary
  (Appendix B posture, repo docs). Server-side RLM execution requires an
  isolated sandbox and a new admission. Owner-local execution comes first.
- **The Apple FM router path.** Routing is a fail-closed guided-generation
  decision in milliseconds. The router may some day CONSULT a cached recall
  result as ambient context, but the router turn itself must never block on
  traversal.
- **Short, sparse contexts.** Inside the effective window on low-density
  questions, one direct call is cheaper and faster (Table 1 base rows on
  CodeQA within limits). The recall engine should answer trivially small
  corpora by direct inclusion, not ceremony.

### 4.3 Why not just adopt compaction harder

The harness `compact` verb and provider-native compaction stay — they manage
the PROVIDER's window. The point of this program is that OpenAgents' memory
does not have to equal the provider's window. The durable log is the memory.
Compaction becomes a view optimization, not a destruction of history. That is
the owner's no-lossy-compaction framing made precise — we do not remove
compaction from providers, we stop depending on it for recall.

---

## 5. Integration architecture

### 5.1 Placement — the leaf executor stays Python, sandboxed, owner-local first

Per the standing hybrid decision, the semantic traversal engine is the
upstream `rlms` package running as a leaf executor, never a port of its REPL
into the online Effect path. Placement uses the shipped harness sandbox
contract:

- **Owner-local first.** The executor runs under
  `local-process-sandbox-provider.ts` (HARN-07) on the owner's machine — a
  real process rung with a per-session workspace, materialized input files,
  and command execution. The desktop main process owns the invocation.
- **Managed sandbox later.** The same `HarnessSandboxProvider` port admits a
  managed sandbox implementation when cloud capacity is admitted. Nothing in
  the contract changes — the provider swaps.
- **Depth policy.** Depth 0 and depth 1 only at first. `max_depth = 1`,
  `max_iterations` around 15, `max_timeout`, `max_tokens`, and
  `max_concurrent_subcalls` all set explicitly. Depth 2+ needs measured
  evidence on our corpora (section 2 item 3).
- **Leaf model policy.** Sub-calls use the owner's already-authenticated
  accounts through existing lanes or API keys the owner already uses for
  chat. Sending a history slice to a sub-model is the same trust boundary as
  today's prompt assembly, which already sends history to that provider. An
  on-device Apple FM leaf is attractive for privacy-sensitive slices and
  short labeling calls once measured.

### 5.2 The Effect-side contract — a context source and a recall service, not a chat harness

RLM should NOT be an `AgentHarness` adapter. A harness adapter models a
conversational coding runtime with turns and approvals. Recall is a typed
query capability. The right Effect shapes:

1. **`HistoryCorpus` (context source).** A pure builder that exports a
   deterministic corpus for a scope — one thread, one Full Auto run, or an
   owner-wide thread set. Inputs are the harness event log stores, the thread
   store, and optional provider history projections. Output is an ordered,
   cursor-addressed corpus artifact (JSONL of neutral events plus a manifest
   with counts, ranges, and redaction summary). The builder honors
   `visibility` and `redactionClass` per event and records what it excluded.
2. **`HistoryRecall` (service).** A typed Effect service with one verb —
   `recall(request)` where the request carries scope, question, budget caps,
   and an execution tier. The response is a `RecallAnswer` — answer text,
   cited cursor spans, confidence class, cost and token totals, and an
   honesty field (`complete`, `partial_budget`, `partial_timeout`,
   `refused`). Two backends implement it:
   - **Tier D (deterministic, depth-0 analog).** Pure Effect traversal —
     grep, time and cursor slicing, key-turn extraction, structural
     summaries. No model call. Fast, free, always available. This tier alone
     answers a large share of "what happened" questions and ships first.
   - **Tier S (semantic).** The sandboxed `rlms` leaf executor. The corpus
     mounts into the sandbox workspace as files, and a small runner script
     loads it as the `context` variable and calls
     `RLM.completion(context, root_prompt=question)`. The typed
     request/response crosses the process boundary as JSON validated by
     Effect Schema on the Effect side. Fail-closed decode, no free-form
     trust.
3. **`history_recall` (host tool).** The service exposed to every lane
   through the harness host-tool vocabulary, and to the Stack B kernel as a
   turn policy capability. The tool schema constrains scope and caps so a
   model cannot request an unbounded traversal.

### 5.3 Re-entry into the neutral event stream

Recall activity must be observable in the same vocabulary as everything else:

- The tool invocation appears as `tool.call` and its completion as
  `tool.result` on the `KhalaRuntimeEvent` stream, so the recorder persists
  it, replay reproduces it, and the renderer shows it as a normal tool row.
- The `tool.result` payload is the bounded `RecallAnswer` — cited spans, cost
  totals, honesty field. Raw slices and sub-call transcripts never enter the
  neutral stream.
- Sub-call progress surfaces as bounded `tool_progress` display events on the
  renderer envelope (sub-call count, elapsed, tokens so far), mapped from the
  package's `on_subcall_*` and `on_iteration_*` callbacks. Display-only, not
  persisted — the same transient-versus-persisted split STREAM-05 models.
- Token accounting is exact-only. Every sub-call's usage lands in the usage
  ledger (`usage-ledger.ts`) per provider and account, and in
  `token_usage_events` if a server-side path ever runs. No synthesized
  counts.

### 5.4 Cost and latency honesty in UX

- Recall is visibly a tool run with live progress, elapsed time, and a
  running token/cost figure — never a silent stall (the paper's tail
  distributions make silent execution unacceptable).
- Every request carries hard caps. On cap hit the answer returns as
  `partial_*` with whatever the buffers hold — mirroring the package's
  best-partial behavior — and says so in the UI.
- Tier D answers are free and immediate. The UI should prefer them and offer
  Tier S escalation explicitly ("search semantically — uses model calls")
  until measurement justifies automatic escalation.

### 5.5 Privacy and redaction boundaries

- Raw history never leaves owner-local execution. The corpus builder, the
  sandbox workspace, and the leaf executor all run on the owner's machine in
  the first phases. The managed-sandbox phase requires its own admission and
  the same owner-scoped storage rules as Pylon raw event chunks.
- Sub-calls send history slices only to providers the owner already uses for
  that history, under the owner's own accounts.
- Public projections receive nothing. No recall content in counters, Forum
  posts, issues, or promise output. The `RecallAnswer` that reaches the
  renderer honors the same redaction classes as the events it cites.
- The corpus honors per-event `visibility` and `redactionClass` at build
  time. An `owner_only` event is available to the owner's local recall and to
  nothing else.

---

## 6. Relation to the AI SDK harvest (STREAM-01..07)

RLM work composes with the streaming harvest and duplicates none of it:

- **STREAM-01 (Effect AI model substrate).** Tier S leaf sub-calls can
  eventually route through `LanguageModel.streamText` as their transport,
  giving typed `AiError` mapping to the conformance failure classes. Not a
  dependency for the pilot — the Python package speaks to providers itself —
  but the natural convergence point for any Effect-native leaf.
- **STREAM-02 (UiMessageChunk projection and reducer).** Recall tool rows
  render through the same progressive reducer as every other tool call. RLM
  adds no renderer vocabulary.
- **STREAM-03 (ChatTransport over the event log).** Unrelated mechanically,
  but both consume the same durable log. Transport replays it to renderers.
  Recall traverses it as a corpus. One artifact, two consumers.
- **STREAM-04 (smoothStream).** Applies to recall answer text like any text.
- **STREAM-05 (ExecutionPlan fallback).** Candidate mechanism for leaf-call
  retry and provider fallback inside a Tier S run.
- **STREAM-06 (partial-object streaming).** A streaming `RecallAnswer` could
  use it later. Not needed for the pilot.
- **STREAM-07 (host tools as Effect Toolkit).** `history_recall` should be
  defined in whatever tool substrate STREAM-07 lands, so it is one host tool
  among many, not a bespoke path.

The one-line division — STREAM is how events reach screens. RLM is how
models reach old events. Both stand on the harness event log.

---

## 7. Honest preconditions

- The neutral log at HEAD carries only the seven core kinds. RLM-01 must
  join thread notes and state the coverage bound in the corpus manifest, and
  widening the projection is independent HARN-lane work, not an RLM
  precondition.
- Delegate turns at HEAD start with empty history (`main.ts:1454`, #9118
  pending). Recall makes this gap survivable but must not be used to excuse
  it — the delegate history fix is separate.
- Running Python `rlms` requires a Python 3.11 runtime on the owner machine.
  The runner must probe and fail typed (`runtime_unavailable`), never
  half-install.
- No public quality claim about recall without a registered promise and a
  verification gate. Lab numbers on our fixtures are internal evidence only.
- Owner and Sol admission gates everything below. This document mints
  nothing.

---

## 8. Roadmap — proposed epic and issue list

Pattern follows the HARN sprint (epic plus numbered children). Titles are
final proposals. Numbers come from GitHub at filing time. Ordering is the
dependency order. RLM-01 through RLM-03 involve no model calls and no new
runtime, so they are low-risk admissions. RLM-04 onward touch model spend and
a Python leaf process and need explicit owner acceptance of that shape.

### Epic — RLM: history traversal without lossy compaction

One epic issue holding the program frame from this document — the corpus,
the two-tier recall service, the host tool, the sandboxed leaf executor, the
Full Auto consumer, and the evaluation gate. Cites this audit and the
standing hybrid decision.

### RLM-01 — `HistoryCorpus`: deterministic history export from the durable stores

- **Goal.** One pure builder that exports a cursor-addressed, redaction-aware
  corpus for a scope (thread, run, owner thread set).
- **Deliverables.** New package `packages/history-corpus` — corpus and
  manifest Effect Schemas, the builder over `HarnessEventLogStore` plus the
  thread store, visibility/redaction filtering with an exclusions record, a
  coverage statement for the seven-kind projection bound.
- **Files/packages.** `packages/history-corpus` (new),
  read-only consumption of `packages/agent-harness-contract`
  (`event-log-store.ts`), `packages/agent-runtime-schema`, and the desktop
  thread store shape.
- **Verification.** Package tests — determinism (same stores, same corpus),
  cursor addressing round-trip, redaction exclusion proof, empty and large
  scope bounds. `pnpm --dir packages/history-corpus test`.
- **Dependencies.** None. Pure addition.

### RLM-02 — Tier D recall: the deterministic traversal engine

- **Goal.** Answer structural and lexical history questions with zero model
  calls — grep, cursor and time slicing, key-turn extraction, per-turn
  structural summaries, cited spans.
- **Deliverables.** `HistoryRecall` service tag, request/response schemas
  including the honesty field, the Tier D implementation, budget-cap
  enforcement even for deterministic work (corpus size, span count).
- **Files/packages.** `packages/history-corpus` (service lives beside the
  corpus), tests.
- **Verification.** Fixture corpora with known decisions planted hundreds of
  turns deep — recall must return the exact cited cursors. Property tests on
  cap behavior. `pnpm --dir packages/history-corpus test`.
- **Dependencies.** RLM-01.

### RLM-03 — `history_recall` host tool on the lanes and the kernel

- **Goal.** Every lane turn can ask the past a question instead of relying on
  the bounded window.
- **Deliverables.** Host-tool definition in the harness host-tool vocabulary,
  desktop main-process wiring so the tool resolves against the owner's local
  stores, `tool.call`/`tool.result` re-entry on the neutral stream, renderer
  row with cited spans, a behavior-contract entry for the recall UX
  statement, Stack B turn-policy exposure.
- **Files/packages.** `packages/agent-harness-contract` (`host-tool.ts`
  registration only), `apps/openagents-desktop/src/` (main-process service
  wiring, lane host-tool plumbing, renderer row),
  `packages/behavior-contracts`.
- **Verification.** Desktop tests proving the tool round-trips through the
  dispatcher, appears in the neutral log, and renders. Behavior-contract
  oracle. `pnpm run check` green.
- **Dependencies.** RLM-02. Coordinates with STREAM-07 if that lands first.

### RLM-04 — Sandboxed `rlms` leaf executor pilot (owner-local, Tier S core)

- **Goal.** Run the upstream Python RLM under the local-process sandbox
  provider with a typed JSON contract, hard caps, and typed failures.
- **Deliverables.** A runner layout under the sandbox session workspace
  (corpus files in, one JSON result out), Effect Schema decode of the result,
  cap plumbing (`max_depth = 1`, iteration, timeout, token, budget), typed
  `runtime_unavailable` and `partial_*` outcomes, sub-call telemetry mapped
  from the package callbacks, exact usage capture into the usage ledger.
- **Files/packages.** `apps/openagents-desktop/src/` (executor service),
  `packages/agent-harness-contract` consumption
  (`local-process-sandbox-provider.ts`), a pinned note on the upstream
  `rlm` commit in the runner doc.
- **Verification.** Hermetic test with a stub LM endpoint (no spend) proving
  contract round-trip, cap enforcement, and partial-answer honesty. One
  owner-run live smoke with a small corpus and a real provider, receipts
  recorded. No CI spend.
- **Dependencies.** RLM-01. Parallel to RLM-02/03.

### RLM-05 — Tier S semantic recall behind `HistoryRecall`

- **Goal.** Route semantic questions to the leaf executor and return cited,
  bounded answers through the same service and host tool.
- **Deliverables.** Tier selection policy (D first, S on explicit escalation),
  slice-citation preservation through the leaf (the runner instructs the
  root to answer with cursor citations), cost/latency display fields, usage
  ledger rows per sub-call, redaction-honoring corpus mount.
- **Files/packages.** `packages/history-corpus`,
  `apps/openagents-desktop/src/` (service wiring, renderer progress).
- **Verification.** Fixture eval — planted-decision recall at 100, 400, and
  1000 turns, Tier D versus Tier S versus a bounded-12-message baseline,
  answers scored on citation exactness. Results recorded as internal
  evidence, no public claim.
- **Dependencies.** RLM-03 and RLM-04.

### RLM-06 — Full Auto long-run recall consumer

- **Goal.** Let a long Full Auto run consult its own full event history for
  continuation decisions instead of a bounded prompt.
- **Deliverables.** Run-scoped corpus builder wiring in the run registry
  path, a bounded "run memory" recall call at continuation framing time with
  caps per run, refusal-safe behavior when recall is unavailable, run-graph
  observability of recall cost.
- **Files/packages.** `apps/openagents-desktop/src/full-auto-run-registry.ts`
  and `full-auto-lane.ts` (consumer only), `packages/history-corpus`.
- **Verification.** Registry tests with a synthetic long run proving the
  framed prompt cites recall output and that recall failure never stalls the
  run. Full Auto guardrails (caps, leases) untouched — assert unchanged.
- **Dependencies.** RLM-05 (or RLM-02 for a Tier-D-only first cut).

### RLM-07 — Dense-recall evaluation harness and honesty gate

- **Goal.** Measure before believing. An OOLONG-style dense fixture suite
  over OUR transcript shapes, run against Tier D, Tier S, bounded-window, and
  provider-compaction baselines.
- **Deliverables.** Fixture generator (linear-density and pair-density
  questions over synthetic and consented-real corpora), scoring, a written
  evidence report under `docs/rlm/`, explicit criteria for admitting depth
  escalation or automatic Tier S.
- **Files/packages.** `packages/history-corpus` (eval module), `docs/rlm/`.
- **Verification.** The suite runs hermetically with the stub LM for CI. Live
  provider runs are owner-triggered with recorded cost.
- **Dependencies.** RLM-05.

### RLM-08 — Managed-sandbox and evidence-pack extension (deferred)

- **Goal.** The same leaf executor on a managed `HarnessSandboxProvider`, and
  the evidence-pack analyst surface for workroom/receipt corpora.
- **Deliverables.** Managed provider wiring, server-side owner-scoped storage
  rules mirroring the Pylon raw-chunk posture, workroom corpus adapter,
  analyst output marked untrusted-candidate.
- **Files/packages.** To be scoped at admission — touches cloud placement and
  spend.
- **Verification.** To be scoped. Requires its own owner and Sol admission.
  Listed here only so the boundary is explicit.
- **Dependencies.** RLM-05, RLM-07, and a separate cloud admission.

Explicitly NOT in this program — porting the RLM REPL loop into Effect,
training an orchestrator model (revisit only after RLM-07 evidence and under
training authority), server-side recall in the Cloud Run monolith, and any
public long-context product claim.

---

## 9. Bottom line

The paper's durable insight is that a long prompt should be an environment
object the model programs over, not a window the model drowns in. OpenAgents
already holds the environment object — the durable, cursor-exact, never
compacted `KhalaRuntimeEvent` log plus the thread stores. The gap is purely a
reader. The proposed program builds that reader in three cheap deterministic
steps (corpus, Tier D recall, host tool), then adds the sandboxed Python RLM
leaf as the semantic tier under hard caps on owner-local capacity, then lets
Full Auto and the evidence surfaces consume it. Compaction remains a provider
window optimization and stops being the memory. Authority surfaces never see
an uncited claim. Everything above needs Sol admission before a single issue
is worked.
