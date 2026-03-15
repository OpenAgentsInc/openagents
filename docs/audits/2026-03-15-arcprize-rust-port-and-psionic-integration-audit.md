# 2026-03-15 ARC Prize Rust Port And Psionic Integration Audit

## Intent

This audit answers the concrete porting question:

> if OpenAgents wants a Rust-native ARC subtree inside this repo, what ARC Prize
> code is actually worth porting, how should it be split under `crates/arc/*`,
> how should it use Psionic, and which missing reusable primitives belong in
> Psionic rather than in ARC-specific crates?

The goal is not to recreate every Python file line-for-line.

The goal is to:

- keep the reusable Psionic boundary clean
- move all durable ARC domain logic into Rust
- replace Python orchestration with typed Rust crates
- avoid pretending the HRM training code can be ported before Psionic has the
  right framework-core and environment primitives

## Relationship To Current Repo Scope

Per `docs/MVP.md`, this is not current MVP product scope.

Per `docs/OWNERSHIP.md`, `crates/psionic/*` own reusable execution substrate,
while ARC-specific game semantics, scoring, datasets, benchmark logic, and
solver behavior should live outside Psionic in their own crate family.

So the right target is not "put ARC inside Psionic."

The right target is:

- `crates/arc/*` owns ARC-specific domain logic
- `crates/psionic/*` grows only where the missing primitive is clearly reusable
  beyond ARC

## Sources Reviewed

OpenAgents sources:

- `docs/MVP.md`
- `docs/OWNERSHIP.md`
- `crates/psionic/README.md`
- `crates/psionic/docs/ARCHITECTURE.md`
- `crates/psionic/docs/TRAIN_SYSTEM.md`
- `crates/psionic/docs/ENVIRONMENT_ABI_REFERENCE.md`
- `crates/psionic/docs/DATASET_TOKENIZER_PACKING_REFERENCE.md`
- `crates/psionic/docs/EVAL_RUNTIME_REFERENCE.md`
- `crates/psionic/docs/RESEARCH_EXPERIMENT_REFERENCE.md`
- `crates/psionic/docs/RESEARCH_RUNNER_REFERENCE.md`
- `crates/psionic/docs/TRAIN_RUN_GRAPH_REFERENCE.md`
- `crates/psionic/psionic-core/src/lib.rs`
- `crates/psionic/psionic-ir/src/lib.rs`
- `crates/psionic/psionic-ir/src/autodiff.rs`
- `crates/psionic/psionic-runtime/src/lib.rs`
- `crates/psionic/psionic-environments/src/lib.rs`
- `crates/psionic/psionic-data/src/lib.rs`
- `crates/psionic/psionic-eval/src/lib.rs`
- `crates/psionic/psionic-research/src/lib.rs`
- `crates/psionic/psionic-train/src/lib.rs`
- `crates/psionic/psionic-train/src/optimizer.rs`
- `crates/psionic/psionic-models/src/lib.rs`

ARC Prize sources under `/Users/christopherdavid/code/arcprize`:

- `README.md`
- `docs/README.md`
- `docs/methodology.mdx`
- `docs/scoring.md`
- `docs/game-schema.mdx`
- `docs/actions.mdx`
- `docs/scorecards.mdx`
- `docs/recordings.mdx`
- `docs/rest_overview.mdx`
- `docs/full-play-test.mdx`
- `docs/local-vs-online.mdx`
- `docs/toolkit/overview.mdx`
- `docs/toolkit/environment_wrapper.mdx`
- `docs/toolkit/listen_and_serve.mdx`
- `docs/toolkit/competition_mode.mdx`
- `docs/add_game.mdx`
- `docs/edit_games.mdx`
- `docs/arc3v1.yaml`
- `ARCEngine/README.md`
- `ARCEngine/arcengine/OVERVIEW.md`
- `ARCEngine/arcengine/base_game.py`
- `ARCEngine/arcengine/camera.py`
- `ARCEngine/arcengine/enums.py`
- `ARCEngine/arcengine/level.py`
- `ARCEngine/arcengine/sprites.py`
- `ARC-AGI/README.md`
- `ARC-AGI/arc_agi/api.py`
- `ARC-AGI/arc_agi/base.py`
- `ARC-AGI/arc_agi/local_wrapper.py`
- `ARC-AGI/arc_agi/remote_wrapper.py`
- `ARC-AGI/arc_agi/models.py`
- `ARC-AGI/arc_agi/scorecard.py`
- `ARC-AGI/arc_agi/server.py`
- `ARC-AGI/test_environment_files/*`
- `ARC-AGI-2/readme.md`
- `arc-agi-benchmarking/README.md`
- `arc-agi-benchmarking/main.py`
- `arc-agi-benchmarking/cli/run_all.py`
- `arc-agi-benchmarking/src/arc_agi_benchmarking/schemas.py`
- `arc-agi-benchmarking/src/arc_agi_benchmarking/scoring/scoring.py`
- `arc-agi-benchmarking/src/arc_agi_benchmarking/adapters/provider.py`
- `arc-agi-benchmarking/src/arc_agi_benchmarking/prompts/*`
- `arc-agi-benchmarking/src/arc_agi_benchmarking/checkpoint/task_checkpoint.py`
- `ARC-AGI-3-Agents/README.md`
- `ARC-AGI-3-Agents/agents/agent.py`
- `ARC-AGI-3-Agents/agents/swarm.py`
- `ARC-AGI-3-Agents/agents/templates/llm_agents.py`
- `ARC-AGI-3-Agents/agents/templates/random_agent.py`
- `arc-agi-3-benchmarking/README.md`
- `arc-agi-3-benchmarking/docs/create_agent.md`
- `arc-agi-3-benchmarking/src/arcagi3/runner.py`
- `arc-agi-3-benchmarking/src/arcagi3/game_client.py`
- `arc-agi-3-benchmarking/src/arcagi3/arc3tester.py`
- `arc-agi-3-benchmarking/src/arcagi3/checkpoint.py`
- `arc-agi-3-benchmarking/src/arcagi3/agent.py`
- `arc-agi-3-benchmarking/src/arcagi3/utils/context.py`
- `arc-agi-3-benchmarking/src/arcagi3/schemas.py`
- `arc-agi-3-benchmarking/src/arcagi3/prompts/manager.py`
- `arc-agi-3-benchmarking/src/arcagi3/utils/image.py`
- `arc-agi-3-benchmarking/src/arcagi3/utils/parsing.py`
- `arc-agi-3-benchmarking/src/arcagi3/adcr_agent/*`
- `hierarchical-reasoning-model-analysis/README.md`
- `hierarchical-reasoning-model-analysis/config/cfg_pretrain.yaml`
- `hierarchical-reasoning-model-analysis/pretrain.py`
- `hierarchical-reasoning-model-analysis/puzzle_dataset.py`
- `hierarchical-reasoning-model-analysis/dataset/build_arc_dataset.py`
- `hierarchical-reasoning-model-analysis/evaluators/arc.py`
- `hierarchical-reasoning-model-analysis/models/layers.py`
- `hierarchical-reasoning-model-analysis/models/losses.py`
- `hierarchical-reasoning-model-analysis/models/sparse_embedding.py`
- `hierarchical-reasoning-model-analysis/models/hrm/hrm_act_v2.py`
- `ARC-AGI-Community-Leaderboard/README.md`

## Executive Summary

The ARC Prize codebase is very portable to Rust in domain terms.

The highest-value parts are:

- deterministic grid and game semantics
- ARC-AGI-2 and ARC-AGI-3 schemas and scoring rules
- local and remote environment wrappers
- scorecards, recordings, checkpoints, and benchmark runners
- typed agent orchestration

The lowest-value parts to port directly are:

- Python SDK glue for many model vendors
- docs-site scaffolding
- the community leaderboard repo
- incidental Flask, matplotlib, AgentOps, and Weights & Biases wiring

The one major area that is not an immediate direct port is the HRM training
stack.

That code is conceptually portable, but it depends on framework-core and
training primitives that Psionic does not yet own in execution reality. So the
correct plan is:

1. build `crates/arc/*` for ARC domain logic first
2. bind ARC datasets, eval, and experiments onto Psionic where the substrate
   already exists
3. add a small, explicit list of reusable missing primitives to Psionic
4. only then port the HRM and transformer training path onto Psionic

## What Is Relevant To Port

### Direct port targets

- `ARCEngine`
  - this is real deterministic engine logic, not UI fluff
- `ARC-AGI`
  - this contains the core client/wrapper/server contracts for ARC-AGI-3
- `ARC-AGI-2`
  - the dataset format and success criterion are canonical
- `arc-agi-benchmarking`
  - schemas, scoring, checkpointing, prompt assembly, and async orchestration
    matter
- `arc-agi-3-benchmarking`
  - the runner, checkpoint model, agent contract, and scorecard plumbing matter
- `hierarchical-reasoning-model-analysis`
  - dataset building, evaluator logic, and model architecture definitions matter

### Spec references, not code ports

- `docs/`
  - use as product and protocol truth, not as code to mirror
- `ARC-AGI-3-Agents`
  - useful as baseline examples, but much of it is superseded by
    `arc-agi-3-benchmarking`
- `ARC-AGI-Community-Leaderboard`
  - public submission schema only; not worth turning into a Rust crate unless
    OpenAgents later wants a native verifier or importer

## Proposed `crates/arc/*` Tree

The cleanest layout is a namespace subtree under `crates/arc/`, with
directories such as `crates/arc/core` and package names such as `arc-core`:

| Crate | Owns | Main upstream source |
| --- | --- | --- |
| `arc-core` | shared ARC schema and value types, internally split into schema, analysis, and execution-envelope layers: tasks, grids, frames, actions, states, operation modes, scorecards, recordings, score-policy IDs, canonicalization, objects, relations, budgets, and solver result envelopes | `ARC-AGI-2`, `ARC-AGI`, `ARC-AGI-3` docs |
| `arc-datasets` | ARC-AGI-2 loaders, ARC augmentation builders, Psionic dataset-manifest export | `ARC-AGI-2`, `hierarchical-reasoning-model-analysis/dataset/*` |
| `arc-engine` | deterministic local game engine, sprite/camera/level logic, game package loading | `ARCEngine` |
| `arc-client` | ARC REST client, cookie-affine session handling, local/remote wrappers, compatibility server | `ARC-AGI` |
| `arc-benchmark` | static exact-match scoring, versioned interactive RHAE scoring, scorecards, recordings, checkpoints, run manifests, and benchmark-policy truth | `arc-agi-benchmarking`, `arc-agi-3-benchmarking` |
| `arc-solvers` | ARC DSL, normative solver object model, hypothesis IR, search/refinement control, verifier, arbiter, agent traits, baseline agents, prompt policies, and Psionic-backed local solver integration | `arc-agi-3-benchmarking`, `ARC-AGI-3-Agents` |
| `arc-ml` | HRM and baseline model definitions, training/eval bridges, ARC-specific metrics over Psionic train/eval | `hierarchical-reasoning-model-analysis` |

That split respects `docs/OWNERSHIP.md`:

- ARC-specific semantics stay in `crates/arc/*`
- reusable execution substrate stays in `crates/psionic/*`

Recommended dependency shape:

- `arc-core` is the shared base and should not depend on other ARC crates
- `arc-engine` should depend on `arc-core`
- `arc-client` should depend on `arc-core` and `arc-engine`
- `arc-benchmark` should consume `arc-core` contracts and optionally replay
  through `arc-engine` or `arc-client`
- `arc-solvers` should sit above `arc-core` and `arc-engine`, using `arc-client`
  only for remote or compatibility-backed interactive runs
- `arc-ml` should consume `arc-core` and `arc-datasets`, with solver-facing
  adapters pointing from `arc-solvers` into `arc-ml` to avoid cycles

Inside `arc-core`, keep three visible sublayers even if they begin in one crate:

- schema
  - tasks, actions, frames, recordings, score-policy IDs
- analysis
  - canonicalization, objects, relations, correspondence candidates
- execution envelopes
  - budgets, refusals, solve results, trace locators

## Repo-By-Repo Port Plan

## 1. `ARCEngine` -> `arc-engine`

This is the cleanest direct port.

What should port nearly verbatim in behavior:

- `ARCBaseGame`
- `Camera`
- `GameAction`, `ActionInput`, `FrameData`, `FrameDataRaw`
- `Level`
- `Sprite`
- collision and rendering rules
- reset, next-level, and multi-frame action semantics

Why it belongs outside Psionic:

- the 64x64 palette-grid game engine is ARC-specific
- Psionic should not own sprite semantics or ARC action vocabulary

Implementation note:

- port to Rust with deterministic value types and explicit no-panic validation
- keep behavior parity using fixture games from `ARC-AGI/test_environment_files`
- prefer a small custom grid buffer over a Python-like dynamic object model

## 2. `ARC-AGI` -> `arc-client`

This should be split into two Rust surfaces:

- a native ARC client library
- a compatibility server for local execution and non-Python clients

Port these pieces:

- environment metadata models
- operation modes
- local wrapper over `arc-engine`
- remote wrapper over the REST API
- cookie-preserving session behavior
- rate-limit-aware retry and backoff behavior
- scorecard open/get/close flows
- default scorecard reuse behavior in the convenience client
- local compatibility server matching `docs/arc3v1.yaml`

What not to copy literally:

- Flask
- dynamic Python module execution through `exec`

Rust shape:

- `reqwest` client with cookie jar and typed `429` handling for remote mode
- `axum` or `hyper` server for local compatibility mode
- filesystem-discovered game packages loaded into `arc-engine`

## 3. `ARC-AGI-2` -> `arc-core` + `arc-datasets` + `arc-benchmark`

This is mostly schema and scorer work.

Port directly:

- task JSON schema
- canonicalization and object/relation extraction
- exact-match success rule across all test pairs
- hashable task identity
- loader utilities
- shared budget/result envelopes consumed by benchmark and solver crates

Then bridge it into Psionic:

- `arc-datasets` should emit `psionic_data::DatasetManifest` and split metadata
- `arc-benchmark` should emit typed eval summaries and pass/fail metrics

This does not require new Psionic primitives.

Psionic already has enough data and eval contract surface for the static ARC
task family.

## 4. `arc-agi-benchmarking` -> `arc-benchmark`

This repo is worth porting, but not as a Python-SDK museum.

What matters:

- result schemas
- prompt assembly
- exact-match scorer
- batch orchestration
- retry and checkpoint semantics
- cost and token accounting

What should be rewritten rather than copied:

- per-provider Python SDK adapters
- Python-only environment management

Rust target:

- a small adapter trait for external model calls
- one OpenAI-compatible HTTP adapter first
- one Psionic local-runtime adapter first
- optional additional vendors later only if still useful

This will immediately remove a large amount of duplicated Python provider glue.

## 5. `arc-agi-3-benchmarking` -> `arc-benchmark` + `arc-solvers`

This is the strongest starting point for the ARC-AGI-3 Rust harness.

Port:

- `GameClient`
- scorecard lifecycle
- action execution loop
- checkpoint manager
- `SessionContext`
- `GameStep`, `GameResult`, action and model-call records
- result saving and resume semantics
- runner registry and agent trait
- competition-mode policy restrictions
- JSONL recording compatibility
- versioned interactive score-policy wiring

In the Rust split:

- `arc-benchmark` should own scorecards, recordings, checkpoints, and final
  run truth
- `arc-benchmark` should also own score-policy versioning because the upstream
  docs currently describe competition-specific weighting/squaring and earlier
  preview RHAE prose that must not be left ambiguous
- `arc-solvers` should own the agent trait and action-selection logic
- `arc-engine` should remain the deterministic state-transition substrate those
  agents act against in local mode

Keep the ADCR pattern, but move it to `arc-solvers` as a baseline, not as the
core library contract.

The current Python split between benchmark harness and agent examples is good.
The Rust version should keep that same separation.

## 6. `ARC-AGI-3-Agents` -> examples in `arc-solvers`, not a first-class port target

This repo is mostly:

- educational templates
- one older swarm runner
- direct OpenAI/chat-completions examples

Useful pieces:

- random-agent baseline
- simple LLM-agent prompts
- replay/recording format expectations

Not worth preserving as-is:

- older duplicated swarm orchestration
- direct Python SDK patterns that `arc-agi-3-benchmarking` already supersedes

Port only the useful baselines and fold them into `arc-solvers/examples`.

## 7. `hierarchical-reasoning-model-analysis` -> staged port into `arc-datasets` + `arc-ml`

This is not a drop-in port today.

The code is valuable in three layers:

### Layer A: immediately portable

- ARC dataset builders and augmenters
- ARC `pass@k` evaluator logic
- puzzle identifier and augmentation lineage handling

These should move first into:

- `arc-datasets`
- `arc-benchmark`

### Layer B: portable after modest Psionic growth

- model configs
- HRM and transformer architecture definitions
- ACT loop semantics
- ARC-specific loss and eval wrappers

These belong in:

- `arc-ml`

but only after Psionic exposes the operator and training support they need.

### Layer C: blocked on missing Psionic primitives

- flash-attention-class training path
- sparse embedding training path
- real distributed gradient exchange
- full checkpoint and multi-rank training execution parity

Those are not ARC-specific. They belong in Psionic.

## What Should Use Psionic Immediately

The ARC subtree should use existing Psionic crates where the substrate is
already generic and real:

- `arc-datasets` -> `psionic-data`
  - versioned dataset keys
  - split manifests
  - iteration and packing contracts
- `arc-benchmark` -> `psionic-eval`
  - typed eval summaries
  - benchmark packages
  - repeated-run aggregation
- `arc-solvers` -> `psionic-serve`, `psionic-router`, `psionic-provider`
  - local model execution
  - model inventory
  - structured inference surfaces
- `arc-benchmark` -> `psionic-research`
  - prompt, policy, and model hillclimb experiment manifests
- `arc-ml` -> `psionic-train`
  - typed run graphs
  - optimizer, checkpoint, and orchestrator substrate

## Missing Psionic Primitives

These are the missing reusable pieces that should land in Psionic, not in
`crates/arc/*`.

The interactive-environment primitives in this section must remain
benchmark-agnostic, action-schema-agnostic, score-policy-agnostic, and
game-state-taxonomy-agnostic.

## 1. Structured interactive environment turns

Current `psionic-environments` sessions are still text-turn oriented:

- turn input is just `content: String`
- tool calls are generic JSON

ARC-AGI-3 needs reusable structured turn payloads:

- typed observations
- typed actions
- typed resets
- typed terminal state changes

This belongs in Psionic because it is not ARC-specific.
Any deterministic interactive environment family will need it.

Recommended Psionic addition:

- structured `EnvironmentObservation`
- structured `EnvironmentAction`
- structured `EnvironmentStepResult`
- resume-safe session state snapshots for multi-turn interactive environments

## 2. Trajectory and episode receipts

ARC-AGI-3 scorecards and recordings are built around episode trajectories:

- action counts
- level transitions
- resets
- terminal states
- optional per-turn reasoning metadata

That trajectory substrate is reusable for any interactive benchmark family.

Recommended Psionic addition:

- typed episode receipt family in `psionic-environments` or `psionic-eval`
- per-turn observation/action/result hashing
- final episode summary separate from text-session summaries

ARC-specific RHAE stays in `arc-benchmark`.
Reusable trajectory evidence belongs in Psionic.

## 3. Interactive benchmark runtime bridge

Psionic eval already owns typed eval runs, but the current reference path is
still oriented around environment session summaries and rubric outcomes, not
turn-by-turn interactive benchmark loops.

Recommended Psionic addition:

- a generic bridge from interactive environment sessions to eval samples
- time, token, and trajectory evidence capture per episode
- benchmark execution helpers for repeated interactive runs

Again: the ARC scoring formula stays in ARC.
The runtime bridge belongs in Psionic.

## 4. Training-class operator coverage for HRM

The HRM code depends on reusable framework primitives that are not ARC-specific:

- embedding lookup and learned positional embedding
- gather and scatter-add
- argmax and sigmoid
- BCE-with-logits and softmax cross-entropy
- ACT-style loop support
- efficient scaled dot-product attention backward path
- sparse embedding update path

Parts of this vocabulary already exist directionally in Psionic IR and runtime,
but not yet as a credible training-class execution lane.

Recommended Psionic additions:

- framework-core ops for gather, scatter-add, argmax, pad, and loss kernels
- real training-class SDPA or flash-attention-class kernels
- sparse-embedding parameter/update support
- loop/control-flow support strong enough for ACT-style training graphs

Minimum readiness bar before the ARC subtree claims HRM parity:

- CPU reference fixtures for `gather`, `scatter-add`, `pad`, `argmax`,
  `BCE-with-logits`, and `softmax cross-entropy`
- deterministic checkpoint save/load for small models
- one tiny single-host train/eval parity fixture
- ACT-style loop semantics in train/eval graphs
- at least one attention path credible for tiny reference workloads

## 5. Real collective execution, not only collective planning

The HRM repo uses real distributed operations:

- `all_reduce`
- `all_gather_into_tensor`
- multi-rank checkpoint/eval behavior

Psionic already owns collective planning and train run-graph contracts, but the
HRM port needs real execution.

Recommended Psionic additions:

- actual collective execution backends wired to `psionic-collectives`
- multi-rank optimizer and gradient exchange runtime
- distributed checkpoint IO that training jobs can consume directly

## 6. Model-state IO for ARC training programs

The HRM code expects model-state save/load behavior that is broader than the
current narrow train examples.

Recommended Psionic additions:

- stronger train-state save/load for arbitrary ARC models
- stable model-state manifests for promoted ARC checkpoints
- small-model local training harnesses that do not require app-owned glue

## What Should Stay Out Of Psionic

These are ARC-specific and should stay in `crates/arc/*`:

- ARC-AGI-2 task schema
- ARC-AGI-3 action vocabulary
- ARC-AGI-3 scorecards and recording semantics
- RHAE scoring
- ARC-specific augmentation logic
- ARC prompt templates and baseline agent policies
- community leaderboard import/export

## Recommended Port Order

## V1 cut line

The first honest ARC subtree release should stop at:

- `arc-core`
- `arc-datasets`
- `arc-engine`
- `arc-client`
- `arc-benchmark`
- `arc-solvers` with one symbolic lane and one non-symbolic lane

It should explicitly exclude:

- `arc-ml` parity claims
- HRM ports
- broad distributed ARC training
- advanced learned search-guide lanes
- competition submission packaging
- leaderboard import/export

## Phase 1: contracts and deterministic domain core

Build:

- `arc-core`
- `arc-datasets`

Port:

- ARC-AGI-2 task schema
- ARC-AGI-3 frame/action/recording/scorecard schema
- ARC-AGI-3 action-space/state/operation-mode contracts
- canonicalization, object extraction, and relation views
- exact-match scorer
- RHAE scorer
- augmentation builders and dataset lineage

Deliverable:

- Rust fixtures proving static and interactive score parity on small examples

## Phase 2: local engine and client

Build:

- `arc-engine`
- `arc-client`

Port:

- ARCEngine behavior
- metadata discovery
- local wrapper
- remote wrapper
- REST client
- local compatibility server

Deliverable:

- one local sample game loaded and executed in Rust
- one REST compatibility test against `docs/arc3v1.yaml`

## Phase 3: benchmark runtime

Build:

- `arc-benchmark`

Port:

- static benchmark runner
- interactive benchmark runner
- score-policy versioning for interactive runs
- checkpoints
- recordings
- scorecards
- cost and token accounting

Integrate:

- `psionic-eval`
- `psionic-research`

Deliverable:

- resumable Rust benchmark runs for ARC-AGI-2 and ARC-AGI-3
- competition-mode and score-policy fixtures with explicit parity evidence

## Phase 4: solver layer

Build:

- `arc-solvers`

Port:

- ARC DSL and interpreter
- Tier A DSL scope first, not the whole research language surface
- normative solver object model with candidate identity and deduplication
- common verifier and arbiter
- random baseline
- ADCR baseline
- minimal prompt-based LLM solver

Integrate:

- first OpenAI-compatible adapter
- first Psionic local-model adapter

Deliverable:

- Rust-native solver harness with no Python dependency

## Phase 5: Psionic primitive expansion

Land the reusable missing primitives listed above in:

- `psionic-environments`
- `psionic-eval`
- `psionic-core`
- `psionic-ir`
- `psionic-runtime`
- `psionic-collectives`
- `psionic-train`

Deliverable:

- interactive-environment substrate strong enough for ARC-AGI-3 episodes
- framework-core strong enough for the first HRM-class model port

## Phase 6: `arc-ml`

Build:

- `arc-ml`

Port:

- evaluator logic first
- small transformer baseline second
- HRM variant third

Do not start with the full H100-scale reproduction.
Start with small deterministic parity fixtures and grow from there.

## Verification Strategy

Every port phase should carry fixture-driven parity checks.

Minimum required parity harnesses:

- `ARCEngine` parity on test games under `ARC-AGI/test_environment_files`
- static task JSON round-trip and exact-match scoring parity on ARC-AGI-2
- interactive scorecard and versioned RHAE parity on documented examples
- REST client/server conformance to `docs/arc3v1.yaml`
- session-affinity cookie behavior and typed `429` backoff behavior
- competition-mode restriction fixtures
- recording and checkpoint replay parity on captured fixtures
- HRM evaluator parity for `pass@k` aggregation before model porting
- model-level parity only on tiny deterministic fixtures before any large-scale
  training claims

Evaluation hygiene requirements:

- no per-task manual tuning on public evaluation tasks
- no acceptance artifact from public evaluation tasks unless labeled as
  non-regression and non-optimization
- public evaluation runs do not feed search-guide, repair-model, or calibration
  datasets
- internal hidden holdout stays disjoint from synthetic tasks derived from
  public evaluation tasks

## Final Recommendation

OpenAgents should absolutely port the relevant ARC Prize stack to Rust, but it
should do it as an ARC namespace that consumes Psionic, not as ARC-specific code
inside Psionic.

The right near-term build is:

- `crates/arc/*` for ARC semantics, engine, benchmarking, solvers, and model
  family code
- targeted Psionic growth only where the primitive is clearly reusable

The practical sequence is:

1. port engine, schemas, datasets, client, scorecards, checkpoints, and
   benchmark runners now
2. wire those crates onto existing Psionic data, eval, serve, and research
   surfaces
3. expand Psionic for structured interactive environments and HRM-class
   training primitives
4. port the HRM stack only after those reusable primitives are real

That is the shortest honest path to "all relevant ARC Prize code in Rust"
without breaking OpenAgents ownership boundaries or faking Psionic capability.
