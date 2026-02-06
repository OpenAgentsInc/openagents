# Horizons and OpenAgents Autopilot / Effect / DSE: Synergies and Learnings

This doc explores **Horizons** (`~/code/Horizons`, [Synth Laboratories](https://github.com/synth-laboratories/Horizons))—a Rust-first runtime for shipping agent systems—and how it aligns with our **Autopilot**, **Effect**, and **DSE** approach. It summarizes what we can learn and where shared patterns or integration could add value.

**Horizons in one sentence:** Event-driven orchestration, project-scoped state, **DAG graph execution** (LLM / tool / Python nodes), auditable actions, with optional **Monty** for in-process Python in graphs, plus standalone crates for **memory** (Voyager), **optimization** (mipro_v2), and **evaluation** (RLM).

---

## 1. Horizons recap (relevant bits)

### 1.1 Core and server

- **horizons_core:** Domain models and backend traits: events, project/org DB, **core agents** (specs, actions, approvals, schedules), **pipelines**, **context refresh**, **evaluation** and **optimization** wiring, **memory** and **onboard** (filestore, vector store, graph store). No implementation by default; server and integrations provide backends.
- **horizons_server:** Axum HTTP API; routes for agents, actions, engine, graph, memory, evaluation, optimization, MCP, etc. Org/project scoped via headers (`x-org-id`, `x-project-id`).

### 1.2 Graph engine (horizons_graph)

- **DAG execution:** Graphs defined in YAML (or JSON): `nodes`, `control_edges`, `start_nodes`, `end_nodes`. Node types include:
  - **python_function** — Inline Python (e.g. `fn_str`); runs via local `python3` subprocess or **Monty** (feature `graph_monty`, env `HORIZONS_GRAPH_PYTHON_BACKEND=monty`).
  - **template_transform** — LLM node: prompt templates, model, temperature, max_tokens; optional tools.
  - **Tool nodes** — Call out to a `ToolExecutor` (local or remote via `GRAPH_TOOL_EXECUTOR_URL`).
- **State:** Shared `exec_state` across nodes; input/output mappings (JSON path / expressions). Map/reduce for fan-out/fan-in.
- **Budgets:** `max_supersteps`, `max_llm_calls`, `max_time_ms` in run config; execution fails fast when exceeded.
- **Built-in verifier graphs:** YAML graphs for rubric-based, few-shot, or RLM-style verification (summarize artifacts → LLM evaluate → outcome_reward / event_rewards).

### 1.3 Standalone crates (no Horizons dependency)

- **rlm:** Reward verification. `RewardSignal`s with weights; `VerificationCase` → `RewardOutcome` in [0,1]; `EvalReport` (Markdown/JSON). LLM-backed signals supported.
- **mipro_v2:** Batch prompt/policy optimization. Dataset → train/holdout split; `VariantSampler` generates candidates; `Evaluator` (LLM + metric) on holdout; best candidate, early stopping, iteration.
- **voyager:** Long-term agent memory. Scope `{ org_id, agent_id }`; append-only `MemoryItem`; optional `index_text` for embeddings; retrieval (vector + recency bias); optional batch summarization. Backend-agnostic (in-memory, pgvector, etc.).

### 1.4 SDKs and integrations

- **SDKs:** Python (`horizons`), TypeScript (`@horizons-ai/sdk`), Rust (`horizons-ai`). Same API surface across languages.
- **Integrations:** Langfuse (observability), RabbitMQ/SQS (queue), pgvector (vector store).

---

## 2. Alignment with our constraints and goals

| Our constraint / goal | Horizons angle |
|-----------------------|----------------|
| **No containers** (Autopilot spec) | Graph engine runs in-process; Python via subprocess or **Monty**. Optional sandbox backends (Docker, Daytona) exist for *core agents* but graph execution itself is no-container. |
| **DSE: signatures, modules, compile, artifacts** | Graph nodes are a DAG of “steps”; verifier graphs are explicit eval pipelines. **mipro_v2** is a direct analogue to our compile loop: policy variants, holdout eval, best candidate. **RLM** is evaluation: signals + aggregation → reward. |
| **Effect: typed, testable boundaries** | Horizons uses traits (e.g. `ToolExecutor`, `LlmClientApi`, `VectorStore`). We use Effect services. Same idea: swappable implementations, clear boundaries. |
| **Auditable, replayable** | Events, audit entries, trace in graph runs. We want receipts and REPLAY; they have structured trace and event bus. |
| **One agent per user** (Autopilot) | Horizons is multi-tenant (org/project); we could adopt project-scoped state and still enforce one logical “agent” per user within a project. |
| **Tiny tool surface** (MVP) | Tools are a trait; executor can be local or remote. We have tool contracts and AgentApiService; same abstraction. |

So Horizons is **architecturally close**: graph as pipeline, eval/optimization as separate crates, tools and LLM behind traits, event-driven. We’re TypeScript/Effect on the web and Workers; they’re Rust-first with HTTP API and SDKs. The **patterns** (graph DAG, budgets, verifier graphs, optimization loop, memory scope) transfer.

---

## 3. Synergies

### 3.1 Graph as DAG of steps (vs. DSE modules)

We have **DSE modules**: Effect programs `I -> Effect<R, E, O>`, composed in pipelines. Horizons has **explicit DAGs**: YAML-defined nodes (python, LLM, tool), control edges, start/end.

**Synergy:** We could:
- Describe a “pipeline” or “verifier” as a **graph IR** (our format or a subset of theirs) and execute it with a small engine (TS/Effect or call into a Rust/WASM graph runner).
- Reuse the **node type** idea: “signature step” (LLM), “tool step”, “python step” (Monty). Our DSE signature is one logical node; a graph is a composition of such nodes with explicit data flow.
- Adopt **budgets** (max steps, max LLM calls, max time) in our runtime so every DSE run and every Autopilot turn is bounded.

**Learning:** Explicit DAG + node types + state passing + budgets is a good execution model. We don’t have to adopt YAML; we can keep Effect as the composition language but still enforce a graph-like shape (acyclic, declared inputs/outputs per step).

### 3.2 Verifier graphs and RLM (evaluation)

Horizons ships **built-in verifier graphs**: e.g. summarize artifacts → single LLM call with rubric + trace → `outcome_reward` and optional `event_rewards`. The **rlm** crate is standalone: `RewardSignal`s, weighted aggregation, `EvalReport`.

We need **evaluation** for DSE: run a signature/module on a dataset, score outputs, feed the optimizer. Their verifier graph is “one way to produce a reward from a trace + rubric.” RLM is “generic reward signals + aggregation.”

**Synergy:**
- **DSE eval:** Could use an RLM-like abstraction: define signals (e.g. “matches expected,” “format valid,” “LLM-as-judge”), weights, and aggregate to a single reward. Our compiler then maximizes that reward over the holdout set.
- **Verifier as a graph:** If we ever expose “run verifier” as an API, a small graph (summarize → LLM with rubric) could be shared or reimplemented in Effect; same inputs/outputs as RLM’s `VerificationCase` / `RewardOutcome`.

**Learning:** Separate “evaluation engine” (RLM-style signals + aggregation) from “how we run the thing being evaluated” (graph, or our DSE runner). That keeps eval reusable and testable.

### 3.3 Optimization loop (mipro_v2 vs. DSE compile)

**mipro_v2:** Dataset → train/holdout split; `VariantSampler` generates prompt/policy variants; `Evaluator` scores on holdout; pick best, iterate with early stopping. No dependency on Horizons.

Our **DSE compile:** Signatures + compile job (dataset, metric, search space, optimizer) → produce compiled artifact (params, hashes, eval report).

**Synergy:** The **loop** is the same: (1) current policy/signature, (2) generate candidates (sampler / optimizer), (3) evaluate on holdout (evaluator / metric), (4) best wins, (5) repeat or early stop. We’re TypeScript/Effect; they’re Rust. We could:
- Implement our compile loop to mirror mipro_v2’s shape (sampler interface, evaluator interface, config for iterations/holdout).
- Reuse or port **evaluation** (RLM or similar) as the scoring backend for that loop.

**Learning:** Keep “optimizer” and “evaluator” as separate interfaces; config drives iterations and stopping. That matches both mipro_v2 and our DSE compile job spec.

### 3.4 Memory (Voyager) and long-term context

**Voyager:** Scope `(org_id, agent_id)`; append-only items; optional embedding for retrieval; recency bias; optional summarization. Backend-agnostic.

We have one Autopilot thread per user; transcript in the Durable Object. We don’t yet have “long-term memory” (episodic, semantic) beyond the transcript.

**Synergy:** If we add memory (e.g. for RAG, or for “remember this” across sessions), we could adopt Voyager’s **concepts**: scope by user/agent, append-only, retrieval by embedding + recency, optional summarization. Implementation could be our own (Convex, vector store) or we could call a Voyager-backed service. The **trait** (append, retrieve, optional summarize) is what we’d align to.

**Learning:** Scope + append-only + retrievable by embedding + optional summarization is a good minimal memory model. We can implement it in TS/Effect without depending on Voyager.

### 3.5 Tool executor and LLM as traits

Horizons graph engine takes `Arc<dyn LlmClientApi>` and `Arc<dyn ToolExecutor>`. Tools can be local or remote (`GRAPH_TOOL_EXECUTOR_URL`). LLM is abstract.

We have **AgentApiService** (Effect) for `/agents/*` and tool contracts. Our “executor” is the Worker + DO. Same idea: tools and LLM are **pluggable boundaries**; the graph (or our pipeline) doesn’t care how they’re implemented.

**Learning:** Keep tool and LLM behind interfaces; inject implementations at runtime. That gives us testability (mocks) and flexibility (remote executor, different LLM backends).

### 3.6 Python in graphs (Monty optional)

Horizons supports **python_function** nodes: inline `fn_str`. Backend is either `python3` subprocess or **Monty** (feature + env). So they already integrated Monty as an optional execution backend for graph nodes.

We’re considering Monty for “agent writes code” (see `docs/autopilot/monty-synergies.md`). Horizons shows a **concrete integration**: “node type = python, backend = subprocess or Monty.” We could offer a similar choice: when we run a “code” step, use subprocess or Monty depending on config.

**Learning:** Python-in-graph (or “code” node) as a first-class node type, with backend selectable (subprocess vs. Monty), is a clean pattern. We can mirror it in our design.

### 3.7 Events and audit

Horizons has an **event** system and **audit** (e.g. audit entries, queryable). Graph runs produce **trace** (node start/finish, timing). We want **receipts** and **REPLAY** (replayable log of tool calls and decisions).

**Synergy:** Emit a **structured event** for every significant step (tool call, LLM call, node start/end). Store for audit and replay. Their event bus and our “receipt per tool call” are the same idea: every side effect is logged with enough context to replay or debug.

**Learning:** Design our receipt/REPLAY format so it can be consumed by an event store or audit API. Same pattern as “event-driven orchestration.”

### 3.8 Project/org scoping

Horizons keys everything by `org_id` and `project_id` (headers, DB, memory scope). We currently have user/thread. For multi-tenant or “projects,” we’d need a similar scoping.

**Learning:** If we add org/project, adopt a consistent keying (e.g. org + project + optional agent/user). Their header-based injection (`_horizons: { org_id, project_id }` in graph inputs) is a simple way to pass scope without threading it through every function.

---

## 4. What we should learn from Horizons’ approach

1. **Graph as first-class execution model** — DAG with typed nodes (LLM, tool, python), explicit edges, shared state, and budgets. We can keep Effect for composition but still enforce a graph-like execution (acyclic, declared IO, limits).

2. **Budgets on every run** — `max_supersteps`, `max_llm_calls`, `max_time_ms`. We should add equivalent limits to DSE runs and to Autopilot (e.g. max tool calls per turn, max time per request).

3. **Verifier as a graph** — Evaluation “pipeline” (summarize → LLM with rubric) as a small, reusable graph. We could have a “verifier graph” or “eval pipeline” that consumes trace + rubric and outputs reward.

4. **Evaluation and optimization as separate crates** — RLM (signals + aggregation) and mipro_v2 (sampler + evaluator + loop) are standalone. We should keep “eval” and “optimize” as clear layers that consume our signatures/datasets and produce scores/artifacts.

5. **Tool and LLM behind traits** — Pluggable, testable. We already do this with Effect services; reinforce that pattern.

6. **Memory: scope + append + retrieve + optional summarize** — Voyager’s minimal model is enough for many agents. We can implement it without depending on Voyager.

7. **Optional Monty for Python nodes** — When we add “code” or “python” steps, support both subprocess and Monty and make it a config/feature choice.

8. **Events and trace for audit/replay** — Every step emits; store and query. Align our receipt/REPLAY format with that.

---

## 5. Possible next steps (no commitment)

- **Document** execution budgets for DSE and Autopilot (max steps, max LLM calls, max time) and enforce them in the runtime.
- **Design** a minimal “graph IR” or pipeline format (or adopt a subset of Horizons’ YAML) so we can describe multi-step flows (e.g. verifier, or a small DSE pipeline) and run them with one engine.
- **Implement** an RLM-style evaluator in TS/Effect: signals with weights, aggregation, output as reward in [0,1]. Use it from our DSE compile loop.
- **Align** our compile loop with mipro_v2’s shape: sampler (candidates), evaluator (holdout score), config (iterations, early stop). No dependency on mipro_v2; same interface idea.
- **Consider** Voyager-style memory (scope, append, retrieve, optional summarize) when we add long-term context; implement behind an Effect service.
- **Reuse** the “python node + Monty optional” pattern when we add code execution (see Monty synergies doc).

---

## 6. References

- **Horizons:** `~/code/Horizons`, [github.com/synth-laboratories/Horizons](https://github.com/synth-laboratories/Horizons) — README, AGENTS.md, horizons_core, horizons_graph, horizons_server, horizons_ts, rlm, mipro_v2, voyager.
- **Our spec:** `docs/autopilot/spec.md` (no containers, one Autopilot per user, tiny tool surface).
- **Our DSE:** `docs/autopilot/dse.md` (signatures, modules, tool contracts, compile, artifacts).
- **Our Effect:** `packages/effuse/docs/effect-migration-web.md`, `packages/effuse/docs/effuse-conversion-apps-web.md`.
- **Monty synergies:** `docs/autopilot/monty-synergies.md` (Horizons uses Monty optionally for graph Python nodes).
