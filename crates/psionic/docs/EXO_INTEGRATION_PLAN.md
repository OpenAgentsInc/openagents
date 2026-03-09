# exo Audit And Integration Plan For Psionic

> Status: drafted 2026-03-08 after reviewing the current
> `crates/psionic/docs/ROADMAP.md` plus the local `~/code/exo` source tree,
> including `README.md`, `docs/architecture.md`, `docs/api.md`,
> `src/exo/main.py`, `src/exo/master/*`, `src/exo/worker/*`,
> `src/exo/worker/engines/mlx/*`, `rust/networking/*`,
> `rust/exo_pyo3_bindings/*`, and selected tests.

## Why This Doc Exists

The current Psionic roadmap already names four primary reference repos:

- `~/code/candle`
- `~/code/tinygrad`
- `~/code/llama.cpp`
- `~/code/ollama`

Those should remain the primary sources of truth for Rust loader/runtime
structure, GGUF semantics, GPT-OSS/Harmony behavior, NVIDIA behavior, and
Ollama-visible API semantics.

`~/code/exo` is worth adding to the working set as a secondary reference repo,
but for a narrower purpose:

- cluster discovery and topology handling
- placement and sharding orchestration
- MLX distributed execution patterns
- multi-API adapter shape
- GPT-OSS / Harmony output parsing fixtures

The short conclusion is:

- `exo` is useful as a reference and possible optional orchestration peer
- `exo` is not a good direct execution dependency for the current Psionic roadmap
- Psionic should port the right ideas from `exo`, not bind Psionic's execution truth to
  `exo`

## Executive Summary

`exo` is a real distributed local-inference system, not just a thin API shim. It
already does device discovery, leader election, event ordering, placement,
download orchestration, per-node worker scheduling, isolated runner processes,
MLX distributed model loading, pipeline and tensor parallel execution, KV
prefix-cache reuse, OpenAI/Claude/Responses/Ollama API adaptation, and
GPT-OSS/Harmony stream parsing.

That makes it relevant to Psionic in two places:

1. as a reference for multi-node orchestration semantics
2. as a source of concrete GPT-OSS/Harmony parser behavior and test cases

It is not a clean direct fit for the active Epic G work in Psionic because the
current Psionic blockers are Rust GGUF loader/runtime/backend gaps on NVIDIA, while
`exo` is a Python + MLX system with Apple-first platform assumptions and no
shipped Linux CUDA execution path today.

The recommended strategy is:

- use `exo` immediately as a reference for parser, topology, placement, and
  orchestration semantics
- do not make `exo` a required runtime dependency for Psionic
- finish `PSI-179` through `PSI-183` as Psionic-only work first
- only after Epic G, decide whether to build an optional `exo` bridge or to
  port the useful control-plane ideas into Rust-native Psionic crates

## What exo Currently Does

### 1. Cluster control plane

`exo` has a real distributed control plane:

- `src/exo/main.py` wires together router, event router, election,
  download coordinator, worker, master, and API
- `docs/architecture.md` describes the system as event sourced with message
  passing
- `src/exo/routing/router.py` routes typed topic traffic over Rust-backed
  networking bindings
- `rust/networking/*` implements libp2p-based discovery and gossip
- `src/exo/master/event_log.py` persists ordered events to disk

Relevant to Psionic:

- explicit cluster events and ordered state transitions
- separation between local events, global events, commands, and connection
  updates
- durable event log and replay model

### 2. Leader election and topology awareness

`exo` has built-in election and topology flow:

- nodes participate in election, not just designated coordinators
- connection discovery is fed back into state
- worker logic observes network reachability and emits topology edge updates
- placement is topology aware rather than just "pick any machine with memory"

Relevant to Psionic:

- cluster-wide master/coordinator semantics
- stable topology graph as first-class runtime input
- distinction between control-plane discovery and execution placement

### 3. Placement and sharding

`exo` already performs model placement:

- `src/exo/master/placement.py` chooses candidate topology cycles with enough
  memory
- `src/exo/master/placement_utils.py` allocates layers proportionally to memory
- it supports pipeline and tensor sharding
- it distinguishes `MlxRing` and `MlxJaccl` instance metadata
- RDMA-capable cycles are selected explicitly for JACCL

Important details:

- placement is cycle based, not arbitrary mesh placement
- single-node instances are forced to pipeline/ring
- tensor placement checks model support and simple divisibility constraints
- coordinator IP selection contains explicit heuristics for RDMA versus ring

Relevant to Psionic:

- a real topology-aware placement heuristic exists to study
- Psionic already has `ExecutionTopologyPlan` and selected-device truth, but exo has
  a fuller cluster orchestration story around those ideas

### 4. Worker / runner split and fault isolation

`exo` separates orchestration from execution:

- `src/exo/worker/main.py` runs a planning loop and supervises runners
- `src/exo/worker/plan.py` turns global state into concrete tasks
- `src/exo/worker/runner/runner_supervisor.py` manages isolated child processes
- `src/exo/worker/runner/bootstrap.py` starts image or LLM runners in a
  subprocess and reports failures back into the event stream

Relevant to Psionic:

- explicit process boundary around model execution
- state-machine-driven runner lifecycle
- a concrete example of "worker supervises execution substrate" rather than
  collapsing everything into one process

### 5. Model loading and execution

The current `exo` execution path is centered on MLX / MLX-LM:

- `src/exo/worker/engines/mlx/utils_mlx.py` loads models and tokenizers
- distributed init is handled through MLX ring or JACCL
- `src/exo/worker/engines/mlx/auto_parallel.py` patches models for pipeline and
  tensor parallel execution
- `src/exo/worker/engines/mlx/generator/generate.py` handles prefill, decode,
  stop handling, logprobs, usage, and generation stats
- `src/exo/worker/engines/mlx/cache.py` implements KV prefix caching with LRU
  eviction pressure

Relevant to Psionic:

- prefix-cache semantics and update rules
- distributed prefill/decode coordination patterns
- topology-aware model sharding behavior
- prompt/render/decode statistics collection

Not directly reusable for Psionic:

- MLX-specific model patching
- Python runtime assumptions
- Apple/MLX backend behavior as execution truth

### 6. API compatibility layer

`exo` exposes a broad adapter surface:

- OpenAI Chat Completions
- Claude Messages
- OpenAI Responses
- Ollama Chat / Generate / Tags / Show / PS
- image generation and image edits endpoints

This is implemented in:

- `src/exo/master/api.py`
- `src/exo/master/adapters/chat_completions.py`
- `src/exo/master/adapters/claude.py`
- `src/exo/master/adapters/responses.py`
- `src/exo/master/adapters/ollama.py`

Relevant to Psionic:

- `exo` proves that one internal task model can back multiple API contracts
- its adapters are useful comparison material when Psionic expands beyond the
  current Ollama-compat and provider-facing surfaces

### 7. GPT-OSS and Harmony-specific logic

This is the highest-value near-term overlap with Psionic Epic G:

- `src/exo/worker/runner/llm_inference/model_output_parsers.py` uses
  `openai-harmony`
- `parse_gpt_oss` turns Harmony recipient/channel output into text vs tool-call
  events
- `src/exo/worker/tests/unittests/test_runner/test_parse_gpt_oss.py` contains
  concrete parser fixtures for GPT-OSS token streams
- `src/exo/worker/engines/mlx/auto_parallel.py` includes a
  `GptOssShardingStrategy`
- `src/exo/worker/engines/mlx/utils_mlx.py` includes GPT-OSS-specific EOS token
  handling and chat-template patching

This does not close Psionic Epic G by itself, but it is directly relevant to:

- `PSI-180` Harmony prompt rendering and channel parsing
- `PSI-181` real GPT-OSS decoder execution behavior
- `PSI-183` validation against external reference behavior

### 8. Test coverage and maturity signals

`exo` is not an empty prototype. The local tree currently has roughly:

- 53 test files under `src/exo/**/tests`
- 6 additional test or harness files under top-level `tests/`

Coverage includes:

- placement utilities
- event log
- API behavior
- cancel behavior
- Claude / Responses adapters
- GPT-OSS parser behavior
- DSML tool parsing
- MLX auto-parallel and prefix-cache behavior

That said, the repo also carries explicit TODO and missed-items documents, which
means its current behavior should be treated as "real but still moving."

## What exo Does Not Solve For Psionic

### 1. It is not the current backend truth Psionic needs

The active Psionic blockers are:

- GGUF GPT-OSS / OpenAI-MoE loading
- truthful `MXFP4` handling
- Psionic-native decoder execution
- NVIDIA text-generation kernels
- end-to-end Psionic-only GPT-OSS validation

`exo` does not close those gaps because its execution core is MLX based and its
Linux story is not the same as Psionic's current NVIDIA-host roadmap.

Evidence from the current tree:

- `pyproject.toml` pins `mlx` and `mlx-lm`
- `README.md` says Linux currently runs on CPU and GPU support is under
  development
- `PLATFORMS.md` lists Apple Silicon macOS as tier-1, while Linux CUDA is still
  planned

### 2. It is Python-first, not Rust-first

Psionic is intentionally a Rust runtime and library-first serving substrate.
`exo`'s control plane, state model, workers, and runners are primarily Python,
with Rust used for networking bindings.

That mismatch matters because Psionic's roadmap is explicitly about reusable Rust
runtime truth, not "a working local system regardless of language."

### 3. It is MLX-shaped, not backend-neutral in the Psionic sense

`exo` has real distributed execution logic, but it is strongly shaped around:

- MLX models
- MLX distributed backends
- MLX tokenizer/model loader behavior
- Apple-first deployment and RDMA-over-Thunderbolt assumptions

Psionic needs explicit backend truth across CPU, Metal, CUDA, and later bounded AMD
work, with no silent backend overclaiming.

### 4. Its artifact model is different

Psionic's current roadmap is GGUF/Ollama/OCI heavy.

`exo` is centered on:

- Hugging Face model cards
- MLX / MLX-LM model loading
- local model directories rather than the same migration boundary Psionic now
  documents

This makes `exo` useful for orchestration semantics, but much less useful as the
artifact-format source of truth.

### 5. Some operational pieces are still rough

The current local tree shows several integration risks:

- `src/exo/routing/router.py` currently returns `Keypair.generate()` early in
  `get_node_id_keypair`, which means persistent node identity is effectively
  disabled in the current code path
- `TODO.md` still calls out task cancellation, network profiling, continuous
  batching behavior, offline staging rough edges, retry logic, and RDMA
  validation
- `MISSED_THINGS.md` confirms active churn in GPT-OSS support, prefill/decode
  behavior, and placement details

That does not make `exo` bad. It means we should treat it as a live reference
system, not as a stable substrate to depend on blindly.

## Recommended Integration Boundary

The right boundary is:

- Psionic owns model format truth, runtime truth, backend truth, capability truth,
  and compute-market evidence
- `exo` may inform cluster orchestration, placement, and API adapter design
- any future `exo` bridge must remain optional and must not become the required
  execution path for Psionic

In plain terms:

- port, do not bind

What that means concretely:

- do not shell out from Psionic to `exo`
- do not make Python or MLX a required dependency for `crates/psionic/*`
- do not use `exo` as the source of truth for GGUF layouts, NVIDIA kernels, or
  backend capability claims
- do use `exo` as a reference for cluster semantics, Harmony parser fixtures,
  and multi-node orchestration ideas

## Recommended Plan

### Phase 0: Use exo as a secondary reference repo now

This can start immediately and does not need to wait for new Psionic crates.

Tasks:

- add this document to the active Psionic docs set
- treat `~/code/exo` as a secondary reference repo for cluster orchestration and
  GPT-OSS parser behavior
- when working `PSI-180`, inspect `exo`'s Harmony parser and test fixtures in
  addition to `llama.cpp`
- when expanding topology/multi-device work later, inspect `exo` placement and
  ring/JACCL coordinator logic

Exit criteria:

- `exo` is used intentionally as a reference, not informally from memory

### Phase 1: Harvest the high-value pieces into Psionic tests and docs

This should happen in parallel with or immediately after Epic G work where
relevant.

Port or adapt into Psionic:

- GPT-OSS/Harmony parser fixture cases from
  `src/exo/worker/tests/unittests/test_runner/test_parse_gpt_oss.py`
- documented GPT-OSS-specific EOS/default handling differences found in
  `utils_mlx.py`
- placement topology examples from `placement_utils.py`
- prefix-cache hit/update semantics from `cache.py` and `generate.py`

Exit criteria:

- Psionic has its own parser and tests for those semantics
- Psionic does not need `exo` at runtime to preserve the behavior

### Phase 2: Define a Psionic-native cluster seam

Do this only after Epic G is complete enough that Psionic can truthfully execute
single-node GPT-OSS on the NVIDIA host.

Add explicit Rust-native surfaces for:

- cluster topology graph
- node identity and capability inventory
- placement plan and shard assignments
- cluster command/event vocabulary
- remote runner lifecycle and isolation policy
- cluster receipts and provenance

This should extend the existing Psionic execution-topology and provider-evidence
story instead of creating a separate hidden control plane.

Exit criteria:

- cluster semantics are explicit in Rust types
- topology and placement are machine-checkable in receipts/provenance

### Phase 3: Build an optional exo-to-Psionic spike

Only once Phase 2 exists should we test a bridge.

The bridge should be scoped narrowly:

- `exo` may provide discovery, election, and placement
- local execution on each node should still be performed by Psionic
- evidence reported back to OpenAgents must remain Psionic-native

Two possible spike shapes:

1. exo as orchestrator over a local Psionic node runtime
2. exo-inspired behavior reimplemented behind a Rust Psionic cluster service

Preference:

- prefer the second long term
- the first is acceptable only as an experiment or migration aid

Exit criteria:

- the spike demonstrates topology-aware placement without surrendering runtime
  truth to `exo`

### Phase 4: Decide whether to keep a bridge or port the ideas fully

After the spike, pick one of two directions.

Direction A: keep `exo` as an optional external orchestrator

- only if it remains clearly optional
- only if Psionic receipts/capabilities still describe the real execution truth

Direction B: port the required orchestration concepts into Rust-native Psionic

- preferred if distributed inference becomes a core OpenAgents product path
- better fit for long-term compute-market evidence and maintainability

Decision rule:

- if OpenAgents needs a credible long-lived distributed execution substrate,
  Psionic should own the control-plane types in Rust
- if OpenAgents only needs short-term clustering experiments, an optional bridge
  can be acceptable

### Phase 5: Extend provider and evidence contracts for clustered execution

No matter which direction wins, clustered execution must become explicit in the
same way the current local runtime did.

Add or extend:

- selected nodes and shard map
- transport class and topology class
- per-node warm/load state
- per-node model/artifact identity
- per-node and aggregate memory budgets
- cluster-level degraded/fallback/refusal reasons
- distributed prefill/decode/cache evidence

Exit criteria:

- clustered inference is as machine-checkable as current local Psionic execution

## What Not To Do

Do not:

- block `PSI-179` through `PSI-183` on any deep `exo` integration work
- proxy Psionic execution through `exo` runners to claim Epic G is done
- make MLX, Python 3.13, or Apple-first assumptions leak into `crates/psionic/*`
- replace `llama.cpp` as the primary GPT-OSS/Harmony/NVIDIA behavior oracle
  with `exo`
- treat `exo`'s current cluster event model as automatically sufficient for
  compute-market receipts

## Proposed Ordering Relative To The Current Roadmap

The pragmatic order is:

1. keep the existing primary references from `ROADMAP.md` as primary
2. use `exo` immediately for secondary reference work around Harmony parsing and
   cluster orchestration semantics
3. finish `PSI-179` -> `PSI-180` -> `PSI-181` -> `PSI-182` -> `PSI-183`
4. only after Psionic can truthfully run GPT-OSS on the NVIDIA host by itself,
   start the Psionic-native cluster seam and any optional `exo` bridge spike

This preserves the current roadmap's core rule:

- external repos may inform semantics
- shipped Psionic execution must still be Psionic's own

## Bottom Line

`exo` is worth integrating into the Psionic planning process, but mostly as a
reference repo and possibly later as an optional orchestration peer.

It is a strong source for:

- cluster control-plane ideas
- placement and topology heuristics
- API adapter patterns
- GPT-OSS / Harmony parser fixtures

It is not the right direct dependency for closing the active Psionic roadmap
because the current open work is Rust-native GGUF/backend/runtime truth on
NVIDIA, and `exo` is currently a Python/MLX system whose strongest path is
Apple-first distributed inference.
