# ARC Subtree Roadmap

> Status: written 2026-03-15 after reviewing `docs/MVP.md`,
> `docs/OWNERSHIP.md`, `crates/arc/spec.md`,
> `crates/psionic/docs/ROADMAP.md`, and
> `docs/audits/2026-03-15-arcprize-rust-port-and-psionic-integration-audit.md`.
>
> This is now the canonical roadmap for `crates/arc/*`.
> `crates/arc/spec.md` remains the canonical architecture and ownership spec.
> `crates/arc/docs/CLAIMS.md` remains the canonical claim-vocabulary doc.
> `crates/arc/docs/INDEX.md` remains the compact ARC docs entrypoint.
> `crates/arc/docs/UPSTREAM_TARGETS.md` remains the canonical first-pass
> upstream target freeze.
> The audit remains the port-source and upstream-mapping reference.

## Executive Summary

- ARC is a domain subtree, not a Psionic feature lane.
- The first work is contracts, engine parity, client parity, benchmark truth,
  and replay fixtures, not model glamour.
- `arc-benchmark` must become the canonical score, recording, checkpoint, and
  refusal surface before solver claims count.
- `arc-solvers` should be a portfolio system with explicit verifier and replay
  truth, not a prompt wrapper museum.
- `arc-ml` is ARC-specific model work over Psionic substrate, not a reason to
  move ARC semantics into Psionic.
- HRM-class model porting is gated on explicit Psionic roadmap items for
  interactive environments, collectives, train-class operator coverage, and
  stronger model-state IO.

## Why This Doc Exists

The current ARC docs already answer two different questions:

- `crates/arc/spec.md` answers the architectural and ownership question
- the 2026-03-15 audit answers the upstream porting and crate-split question

What was still missing was a dependency-ordered roadmap for the ARC subtree
itself.

This document answers the next question:

> what is the execution-ordered ARC program once the crate split and Psionic
> boundary are fixed?

## Relationship To Product Scope

This roadmap does not widen current product MVP scope in
[docs/MVP.md](/Users/christopherdavid/code/openagents/docs/MVP.md).

This is a library, benchmark, and research-program roadmap.

Per [docs/OWNERSHIP.md](/Users/christopherdavid/code/openagents/docs/OWNERSHIP.md):

- `crates/arc/*` owns ARC-specific domain logic
- `crates/psionic/*` owns reusable execution substrate
- `apps/*` own product behavior and UX
- kernel and Nexus own authority truth, not ARC cognitive inner loops

Nothing in this roadmap should be read as permission to move app logic,
wallet/payout logic, marketplace authority, or generic training substrate into
ARC crates.

## Relationship To Psionic

The Psionic roadmap in
[crates/psionic/docs/ROADMAP.md](/Users/christopherdavid/code/openagents/crates/psionic/docs/ROADMAP.md)
is the canonical roadmap for reusable substrate.

The ARC roadmap is the canonical roadmap for ARC-specific semantics,
benchmarks, solvers, and model-family work.

The governing rule is:

- if the primitive is reusable beyond ARC, it belongs in Psionic
- if the logic is ARC-specific, it belongs in `crates/arc/*`

That rule is especially important for:

- structured interactive environments
- trajectory and episode receipts
- interactive benchmark runtime bridges
- train-class collectives and checkpoint IO
- gather/scatter/loss/attention/sparse operators needed by ARC models

## ARC v1 Target

ARC v1 means the smallest subtree that satisfies:

- `contracts-real`
- `benchmark-real`
- `solver-real`

ARC v1 explicitly excludes:

- ARC-ML model parity
- HRM-class models
- distributed training
- advanced learned search lanes

## Objective

Build `crates/arc/*` into a Rust-native ARC subtree with:

- shared static and interactive benchmark contracts
- deterministic ARC engine and compatibility runtime
- truthful exact-match and interactive score computation
- replayable benchmark, checkpoint, recording, and refusal surfaces
- a portfolio solver for ARC-AGI-1 and ARC-AGI-2
- an honest ARC-AGI-3 agent runtime with local and remote parity
- ARC-specific model and training work layered onto Psionic
- explicit acceptance artifacts and regression fixtures at every phase

This roadmap is not:

- a plan to put ARC into Psionic
- a plan to copy every Python file line-for-line
- a plan to win by prompt glue before benchmark truth exists
- a plan to claim HRM parity before Psionic has the right substrate
- a plan to tune directly on public evaluation feedback

## Explicit Non-Goals

These are not goals of this roadmap:

- porting docs-site scaffolding, Flask glue, or vendor SDK clutter as-is
- treating random-agent or prompt-only baselines as the core architecture
- moving ARC action vocabulary, scorecards, or DSL semantics into Psionic
- claiming solver progress without replay and verifier evidence
- claiming model parity from one large-scale training anecdote
- reproducing H100-scale HRM training before tiny deterministic parity exists
- blending static ARC-AGI-2 and interactive ARC-AGI-3 semantics into one vague
  "agent benchmark" abstraction
- using public evaluation splits as online hillclimb feedback

## Architectural Direction

The governing ARC rule is:

- benchmark-truthful at the bottom
- solver-truthful in the middle
- model-truthful at the top
- reusable substrate below the ARC boundary, not inside it

That yields four structural layers.

### Layer 1: ARC contracts and benchmark truth

Owns:

- tasks, grids, actions, frames, scorecards, recordings, and checkpoints
- canonicalization, objects, and relation views
- exact-match and RHAE scoring
- run manifests, replay bundles, and refusal/result envelopes

Current and planned crates:

- `arc-core`
- `arc-datasets`
- `arc-benchmark`

### Layer 2: deterministic execution and compatibility

Owns:

- local deterministic game execution
- game package loading
- local and remote wrapper behavior
- operation-mode and competition-mode policy
- compatibility-server behavior
- local-vs-online parity fixtures

Current and planned crates:

- `arc-engine`
- `arc-client`

### Layer 3: solver and agent cognition

Owns:

- ARC DSL and interpreter
- hypothesis IR
- search and refinement loops
- verifier and falsifier behavior
- portfolio arbitration
- agent traits, baseline agents, and action-selection policy

Current and planned crates:

- `arc-solvers`

### Layer 4: ARC-specific model and research program

Owns:

- ARC model definitions
- ARC losses and metrics
- evaluator wrappers
- search-guide learning data
- training and eval adapters onto Psionic

Current and planned crates:

- `arc-ml`

### Psionic Underlay

Psionic remains the substrate for:

- dataset manifests
- eval-run orchestration
- research runs
- local model serving and provider/runtime execution
- environment-session evidence once generalized
- train/runtime/collective execution
- checkpoint and artifact infrastructure

## Crate Boundary Rules

The crate split in `crates/arc/spec.md` is not optional.

- `arc-core` owns shared ARC schema, canonicalization, objectization, budgets,
  and solve-result envelopes.
- `arc-datasets` owns dataset loading, augmentation, synthetic lineage, and
  `psionic-data` export.
- `arc-engine` owns deterministic ARC environment transitions and local replay.
- `arc-client` owns local/remote wrappers, operation-mode surfaces,
  cookie-affine REST behavior, rate-limit/backoff policy, and
  compatibility-server surfaces.
- `arc-benchmark` owns exact-match scoring, interactive RHAE, recordings,
  scorecards, checkpoints, run manifests, and benchmark summaries.
- `arc-solvers` owns the cognitive inner loop: DSL, verifier, search,
  refinement, agent policy, arbiter, and trace generation.
- `arc-ml` owns ARC-specific model definitions, losses, metrics, and evaluator
  glue over Psionic.

Dependency rules:

- `arc-core` must not depend on other ARC crates
- `arc-engine` depends on `arc-core`
- `arc-client` depends on `arc-core` and `arc-engine`
- `arc-benchmark` depends on `arc-core` and may replay through `arc-engine` or
  `arc-client`
- `arc-solvers` depends on `arc-core`, `arc-engine`, and optionally
  `arc-client`
- `arc-ml` depends on `arc-core`, `arc-datasets`, and Psionic train/eval
  crates, with solver-facing adapters pointing from `arc-solvers` into
  `arc-ml`

Inside `arc-core`, keep three internal layers visible even if they begin in one
crate:

- schema
  - tasks, frames, actions, recordings, scorecards, score-policy IDs
- analysis
  - canonicalization, objects, relations, correspondence candidates
- execution envelopes
  - budgets, refusals, solve results, trace locators

### Cross-Crate Concern Matrix

| Concern | Owner |
| --- | --- |
| state transition correctness | `arc-engine` |
| local replay and deterministic checkpoint application | `arc-engine` |
| transport, session, cookies, and backoff | `arc-client` |
| local compatibility serving and REST schema conformance | `arc-client` |
| scorecard lifecycle policy | `arc-benchmark` |
| competition-mode scoring restrictions | `arc-benchmark` |
| replay acceptance and benchmark result truth | `arc-benchmark` |
| verifier logic, candidate identity, deduplication, and attempt policy | `arc-solvers` |
| ARC-specific losses, metrics, and evaluator glue | `arc-ml` |

## Reference Truth Rules

- `arc-benchmark` is the canonical scorer and episode/result truth surface.
- `arc-benchmark` is the only crate allowed to compute benchmark scores.
- exact-match and RHAE claims only count if computed by typed benchmark code,
  not by ad hoc notebook logic.
- `arc-engine` local deterministic execution is the reference for interactive
  behavior parity.
- interactive score policies must be versioned explicitly so methodology,
  leaderboard, and compatibility behavior do not drift silently.
- unsupported tasks, actions, runtime states, and model capabilities must
  refuse explicitly with typed reasons.
- public evaluation splits must not be used as iterative tuning feedback in the
  same development loop.
- solver runs MUST reproduce identical result envelopes under identical task,
  solver manifest, seed bundle, and budget inputs.
- every solver and benchmark claim must be replayable from fixtures, seeds,
  budgets, manifests, and trace artifacts.

## Public-Eval Hygiene Rules

These rules are non-negotiable:

- no per-task manual solver tuning on public evaluation tasks
- no roadmap acceptance artifact may be generated from public evaluation tasks
  unless it is labeled explicitly as non-regression and non-optimization
- public evaluation runs must not feed search-guide, repair-model, or
  calibration training datasets
- internal hidden holdout must stay disjoint from synthetic tasks derived from
  public evaluation tasks
- public evaluation visibility may be used only for bounded compatibility or
  non-regression checks, not for optimization loops

Canonical operator policy and harness:

- `crates/arc/docs/PUBLIC_EVAL_HYGIENE.md`
- `scripts/lint/arc-public-eval-hygiene-check.sh`

## Success Bar

The ARC subtree should be judged against five progressively stronger claims.

Canonical claim vocabulary:
`crates/arc/docs/CLAIMS.md`

### Claim 1: `contracts-real`

This means the shared ARC contracts are frozen enough to build against:

- task, grid, action, frame, scorecard, and recording schemas
- canonicalization and object/relation views
- budget, result, and refusal envelopes
- dataset manifests and split metadata

### Claim 2: `benchmark-real`

This means the benchmark runtime is truthful:

- deterministic local engine parity exists
- compatibility-server and REST behavior are typed and tested across
  `Offline`, `Online`, and `Competition`
- exact-match and versioned interactive scores are computed by `arc-benchmark`
- recordings, checkpoints, replay bundles, scorecard lifecycle, and
  session-affine online runs survive restart and resume

### Claim 3: `solver-real`

This means the static ARC solver is more than one-off prompting:

- DSL and interpreter exist
- verifier and falsifier exist
- at least one symbolic lane and one non-symbolic lane exist
- traces, budgets, and attempts are explicit
- portfolio results beat the best single lane on internal holdout

### Claim 4: `interactive-real`

This means ARC-AGI-3 behavior is honest:

- agent trait and runner exist
- local and remote game-client parity exists
- action budgets and refusal semantics are explicit
- recordings and scorecards are replayable
- baseline agents are reproducible

### Claim 5: `research-credible`

This means ARC model work is honest rather than aspirational:

- evaluator parity exists
- small model ports run on Psionic substrate
- search-guide and trace-derived learning loops are real
- HRM-class claims are gated on explicit Psionic readiness
- metrics, checkpoints, and failure semantics are auditable

## Claim Acceptance Scoreboard

| Claim | Required green surfaces | Disqualifiers |
| --- | --- | --- |
| `contracts-real` | task/action/frame/recording schemas, canonicalization, objects/relations, dataset manifests, result envelopes | crate-local duplicate schemas, missing refusal taxonomy, unstable IDs |
| `benchmark-real` | engine parity, REST compatibility, exact-match scorer, versioned RHAE scorer, checkpoint/resume, scorecard lifecycle, competition-mode policy, replay fixtures | score computed outside `arc-benchmark`, hidden environment assumptions, unversioned score-policy drift |
| `solver-real` | DSL/interpreter, verifier, trace bundles, budget accounting, symbolic and non-symbolic lanes, hidden holdout reporting | prompt-only demos, no replay, no attempt policy, no verifier |
| `interactive-real` | agent trait, game runner, local/remote parity, scorecard truth, action-budget policy, replayable trajectories | ad hoc game loops, no deterministic parity, opaque action/refusal semantics |
| `research-credible` | evaluator parity, small-model train/eval loops, checkpoint truth, bounded HRM gates, cross-reference to Psionic readiness | large-scale training anecdotes without fixture parity, ARC-specific substrate hidden inside Psionic |

## Claim Artifact Expectations

| Claim | Minimum artifact family |
| --- | --- |
| `contracts-real` | schema fixtures and deterministic serialization fixtures |
| `benchmark-real` | exact-match and RHAE score parity harnesses plus replay fixtures |
| `solver-real` | trace-bundle corpus, determinism fixtures, and hidden-holdout reports |
| `interactive-real` | replayable trajectory pack plus local-vs-remote parity fixtures |
| `research-credible` | checkpoint artifacts, train/eval logs, and bounded parity summaries |

## Acceptance Artifact Rule

No roadmap item is complete unless it lands with at least one acceptance
artifact:

- a capability matrix
- a replay or refusal fixture set
- a benchmark or score parity harness
- a checkpoint/recording compatibility matrix
- a trace corpus summary

Code without one of those artifacts is not roadmap completion.

## Capability Tiering

ARC capability matrices should classify coverage explicitly:

- Tier A: required for benchmark truth and first serious solver claims
- Tier B: important breadth for research usefulness, but not a gate on the
  first honest subtree claim
- Tier C: advanced, experimental, or long-tail capability

Tiering is required to stop every interesting ARC idea from pretending to be a
Phase 1 blocker.

## V1 Subtree Closure

ARC v1 means the smallest honest Rust-native subtree that unlocks
`contracts-real`, `benchmark-real`, and `solver-real`, with bounded baseline
interactive parity.

Included in v1:

- `arc-core`, `arc-datasets`, `arc-engine`, `arc-client`, `arc-benchmark`, and
  `arc-solvers`
- exact-match static scoring plus versioned interactive benchmark truth
- scorecard lifecycle, recording, checkpoint, replay, and local-vs-remote
  parity fixtures
- one symbolic lane and one non-symbolic lane with common verifier, typed
  candidate identity, deduplication, budget accounting, attempt policy, and
  trace bundles
- baseline interactive agent surfaces needed for local/remote parity and typed
  action-budget truth
- non-Python smoke suites for the above

Explicitly excluded from v1:

- `arc-ml` parity claims
- HRM ports
- broad distributed ARC model training
- advanced learned search-guide lanes
- competition submission packaging
- leaderboard import/export

The v1 cut line ends before `ARC-403`, `ARC-405`, `ARC-406`, and all of Epic 5.
Those may begin only after the v1 closure artifacts are green.

## Current Baseline

The current baseline is intentionally thin.

### Foundations already present

- `crates/arc/spec.md` now fixes the crate split and owner boundaries
- `crates/arc/core`, `crates/arc/datasets`, `crates/arc/engine`,
  `crates/arc/client`, and `crates/arc/benchmark` now exist in the workspace
  with the first live contract, dataset, engine, client, compatibility, parity,
  and exact-match scoring surfaces
- the 2026-03-15 audit maps the relevant ARC Prize repos into the crate plan
- the upstream Python sources and protocol docs have already been audited
- Psionic already offers meaningful substrate for dataset manifests, eval runs,
  local model execution, research runs, and train/runtime scaffolding

### Biggest current gaps

- `arc-solvers` and `arc-ml` still do not exist
- `arc-benchmark` now covers exact-match static scoring plus bounded
  versioned interactive RHAE policy surfaces plus bounded checkpoint/resume
  and repeated-run summary surfaces, but checkpoint bundles still persist
  canonical JSON artifacts rather than consuming JSONL traces directly
- there is not yet a benchmark-truthful REST/runtime parity layer for
  local/remote ARC-AGI-3 flows; the bounded local compatibility server is now
  in-tree, the wrapper-level local-vs-remote parity harness is now in-tree,
  and `arc-client` now owns bounded retry/backoff plus JSONL trace
  compatibility, but full benchmark-truth runtime parity and score-policy
  authority still remain open
- there is no explicit versioned score-policy implementation despite the
  methodology, scoring, and competition docs carrying policy-specific behavior
- the first bounded operation-mode and competition-mode lifecycle surface is
  now in Rust, and the client path now has typed retry/backoff plus JSONL
  compatibility, but authoritative online benchmark/runtime parity still
  remains open
- there is no ARC DSL, verifier, or portfolio solver in Rust yet
- there is no replayable ARC-AGI-3 agent harness in Rust yet
- ARC model work is blocked on both missing ARC crates and missing Psionic
  primitives
- the public/private split discipline for solver evaluation still needs to be
  encoded into actual harnesses, not just docs

## Psionic Dependency Crosswalk

The ARC subtree depends on specific reusable Psionic work.

| ARC outcome | Psionic dependency | Psionic roadmap IDs |
| --- | --- | --- |
| structured ARC-AGI-3 turns and resumable sessions | typed environment observations/actions/results and session snapshots | `PLIB-512` |
| replayable trajectories and scorecard evidence | episode and trajectory receipts with per-step hashing | `PLIB-513` |
| repeated interactive benchmark runs | bridge from environment sessions into eval samples and repeated-run aggregation | `PLIB-514` |
| train-class multi-rank model work | real collective execution and evidence for `all_reduce` / `all_gather` class behavior | `PLIB-509`, `PLIB-515` |
| HRM-class and similar non-adapter models | gather/scatter/loss/sparse/loop operator coverage | `PLIB-612`, `PLIB-614` |
| honest checkpoint and small-model training closure | stronger train-state and model-state IO plus promoted checkpoint manifests | `PLIB-613` |

ARC should reference these IDs directly instead of re-inventing equivalent
substrate locally.

### HRM Minimum Readiness Bar

`ARC-506` remains blocked until the following are green:

- `PLIB-612`
  - CPU reference fixtures for `gather`, `scatter-add`, `pad`, `argmax`,
    `BCE-with-logits`, and `softmax cross-entropy`
- `PLIB-613`
  - deterministic checkpoint save/load for small models
  - one tiny single-host train/eval parity fixture over `psionic-train` and
    `psionic-eval`
- `PLIB-614`
  - ACT-style loop semantics in train/eval graphs
  - at least one attention path credible for tiny reference workloads
- `PLIB-509` and `PLIB-515`
  - only when the claimed workload exercises multi-rank collective behavior

## Roadmap Shape

This roadmap is organized into seven epics.

| Epic | Theme | Outcome |
| --- | --- | --- |
| Epic 0 | Governance and acceptance | one canonical ARC roadmap, claim vocabulary, and acceptance discipline |
| Epic 1 | Contracts and dataset core | `arc-core` and `arc-datasets` become real shared foundations |
| Epic 2 | Engine, client, and benchmark truth | deterministic runtime, REST compatibility, scorecards, and replay truth |
| Epic 3 | Static solver program | first serious ARC-AGI-1/2 portfolio solver in Rust |
| Epic 4 | Interactive ARC-AGI-3 agents | reproducible agent runtime over truthful benchmark contracts |
| Epic 5 | ARC-ML over Psionic | ARC-specific model work with explicit substrate gates |
| Epic 6 | Evidence, regression, and operator use | capability matrices, fixture corpora, docs, and non-Python operator workflows |

Epics 0 through 3 now have live GitHub issue blocks:

- Epic 0 master:
  [#3737](https://github.com/OpenAgentsInc/openagents/issues/3737)
- Epic 1 master:
  [#3738](https://github.com/OpenAgentsInc/openagents/issues/3738)
- Epic 2 master:
  [#3739](https://github.com/OpenAgentsInc/openagents/issues/3739)
- Epic 3 master:
  [#3740](https://github.com/OpenAgentsInc/openagents/issues/3740)

Later epics still use roadmap-local IDs until activated.

## Epic 0: Governance And Acceptance

### Goal

Make the ARC subtree claim vocabulary explicit and keep future work anchored on
benchmark truth rather than on isolated solver demos.

### Exit Criteria

- one canonical ARC roadmap exists
- ARC claim families are explicit and non-overlapping
- `spec.md`, roadmap, and audit stay aligned
- execution order is anchored on benchmark and replay truth, not model hype

Master issue:
[#3737](https://github.com/OpenAgentsInc/openagents/issues/3737)

### Issues

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-001` | landed | Write the canonical ARC subtree roadmap. This document closes that issue. |
| `ARC-002` / [#3665](https://github.com/OpenAgentsInc/openagents/issues/3665) | landed | Freeze the ARC claim vocabulary: `contracts-real`, `benchmark-real`, `solver-real`, `interactive-real`, and `research-credible`. |
| `ARC-003` / [#3666](https://github.com/OpenAgentsInc/openagents/issues/3666) | landed | Add one compact index linking `crates/arc/spec.md`, this roadmap, the audit, and future acceptance matrices. |
| `ARC-004` / [#3667](https://github.com/OpenAgentsInc/openagents/issues/3667) | landed | Freeze the supported upstream benchmark and protocol versions this subtree targets first. |
| `ARC-005` / [#3668](https://github.com/OpenAgentsInc/openagents/issues/3668) | landed | Publish the public-eval hygiene rule as explicit harness, artifact-labeling, and operator policy, not only as prose. |

## Epic 1: Contracts And Dataset Core

### Goal

Make `arc-core` and `arc-datasets` real enough that every later crate can
depend on one stable source of truth.

### Exit Criteria

- shared ARC schemas exist in one place
- canonicalization and object/relation views are deterministic
- dataset loaders and split manifests are typed and reproducible
- budgets, results, and refusal envelopes are shared across solver and
  benchmark crates

Master issue:
[#3738](https://github.com/OpenAgentsInc/openagents/issues/3738)

### Issues

#### Shared Contracts

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-101` / [#3669](https://github.com/OpenAgentsInc/openagents/issues/3669) | landed | Create `crates/arc/core` as package `arc-core`. |
| `ARC-102` / [#3670](https://github.com/OpenAgentsInc/openagents/issues/3670) | landed | Add `ArcGrid`, `ArcTask`, and ARC-AGI-3 action/frame/recording/scorecard contracts. |
| `ARC-103` / [#3671](https://github.com/OpenAgentsInc/openagents/issues/3671) | landed | Add budget, solve-result, refusal, and trace-locator envelopes used across benchmark and solver layers. |
| `ARC-104` / [#3672](https://github.com/OpenAgentsInc/openagents/issues/3672) | landed | Freeze deterministic task IDs, hashes, and serialization contracts so fixtures survive refactors. |
| `ARC-111` / [#3673](https://github.com/OpenAgentsInc/openagents/issues/3673) | landed | Add interactive benchmark policy contracts: dynamic `available_actions`, game-state enums, operation modes, score-policy IDs, and recording-envelope identifiers. |

#### Canonicalization And Objectization

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-105` / [#3674](https://github.com/OpenAgentsInc/openagents/issues/3674) | landed | Implement canonicalization, color normalization, and train/test dimension summaries. |
| `ARC-106` / [#3675](https://github.com/OpenAgentsInc/openagents/issues/3675) | landed | Implement connected components, holes, bounding boxes, symmetry signatures, and relation graphs. |
| `ARC-107` / [#3676](https://github.com/OpenAgentsInc/openagents/issues/3676) | landed | Add correspondence-candidate extraction between train inputs and outputs. |
| `ARC-108` / [#3677](https://github.com/OpenAgentsInc/openagents/issues/3677) | landed | Add deterministic parser/objectization fixture corpora over sampled ARC-AGI-1 and ARC-AGI-2 tasks. |

#### Datasets And Synthetic Lineage

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-109` / [#3678](https://github.com/OpenAgentsInc/openagents/issues/3678) | landed | Create `crates/arc/datasets` with ARC-AGI-1/2 loaders and split metadata export through `psionic-data`. |
| `ARC-110` / [#3679](https://github.com/OpenAgentsInc/openagents/issues/3679) | landed | Port augmentation builders, synthetic-task lineage metadata, and trace-derived dataset packaging. |

## Epic 2: Engine, Client, And Benchmark Truth

### Goal

Build the deterministic execution and benchmark-truth layer before claiming
solver or model progress.

### Exit Criteria

- local ARCEngine behavior is reproducible in Rust
- local and remote wrapper behavior are typed and tested across operation modes
- exact-match and versioned RHAE scoring are canonicalized in Rust
- recordings, scorecards, checkpoints, replay bundles, and online-session
  semantics survive restart and resume

Master issue:
[#3739](https://github.com/OpenAgentsInc/openagents/issues/3739)

### Issues

#### Engine And Compatibility Runtime

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-201` / [#3680](https://github.com/OpenAgentsInc/openagents/issues/3680) | landed | Create `crates/arc/engine` and port deterministic game execution, including sprites, level logic, and action transitions. |
| `ARC-202` / [#3681](https://github.com/OpenAgentsInc/openagents/issues/3681) | landed | Add fixture-driven parity against the upstream `ARC-AGI/test_environment_files` samples. |
| `ARC-203` / [#3682](https://github.com/OpenAgentsInc/openagents/issues/3682) | landed | Create `crates/arc/client` with local and remote wrappers, REST models, and cookie-affine session behavior. |
| `ARC-204` / [#3683](https://github.com/OpenAgentsInc/openagents/issues/3683) | landed | Port the local compatibility server against the ARC docs schema and local-vs-online behavior docs. |
| `ARC-205` / [#3684](https://github.com/OpenAgentsInc/openagents/issues/3684) | landed | Add local-vs-remote parity harnesses so game-client behavior is explicitly comparable across modes. |
| `ARC-211` / [#3690](https://github.com/OpenAgentsInc/openagents/issues/3690) | landed | Implement versioned interactive score policies, including the current weighted-and-squared methodology/competition policy and any required compatibility fixtures for upstream preview behavior. |
| `ARC-212` / [#3691](https://github.com/OpenAgentsInc/openagents/issues/3691) | landed | Implement scorecard lifecycle and operation-mode policy surfaces: default-scorecard reuse, auto-close behavior, closed-card refusal, and competition-mode restrictions. |
| `ARC-213` / [#3692](https://github.com/OpenAgentsInc/openagents/issues/3692) | landed | Implement cookie-affine remote sessions, 429/backoff handling, and local-vs-online recording policy with JSONL recording compatibility. |

#### Benchmark Truth

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-206` / [#3685](https://github.com/OpenAgentsInc/openagents/issues/3685) | landed | Create `crates/arc/benchmark` and implement exact-match static scoring for ARC-AGI-1 and ARC-AGI-2. |
| `ARC-207` / [#3686](https://github.com/OpenAgentsInc/openagents/issues/3686) | landed | Implement interactive RHAE scoring, scorecards, recordings, and per-step summaries for ARC-AGI-3. |
| `ARC-208` / [#3687](https://github.com/OpenAgentsInc/openagents/issues/3687) | landed | Port checkpoint, resume, and run-manifest behavior from the benchmarking repos. |
| `ARC-209` / [#3688](https://github.com/OpenAgentsInc/openagents/issues/3688) | landed | Add typed benchmark summaries and repeated-run aggregation over `psionic-eval` and `psionic-research`. |
| `ARC-210` / [#3689](https://github.com/OpenAgentsInc/openagents/issues/3689) | landed | Publish benchmark parity fixtures covering exact-match, RHAE, checkpoints, recordings, and replay. |

## Epic 3: Static Solver Program

### Goal

Build the first serious ARC-AGI-1/2 solver in Rust as a portfolio system with
verifier-first truth.

Solver architecture rule:

- prompt-only agents MUST NOT be treated as architectural lanes
- prompt policies MAY exist only as baseline agents
- final solver claims MUST rely on DSL, verifier, and arbiter infrastructure
- DSL Tier A is the only required DSL scope for v1 solver claims

### Exit Criteria

- the DSL and interpreter are real
- verifier and falsifier logic gate final answers
- at least one symbolic lane and one non-symbolic lane are real
- traces, budgets, and attempt policy are explicit
- solved sets are reported against internal hidden holdout with full trace
  bundles
- solver development accumulates a trace corpus for successful solves, verifier
  rejections, spurious fits, and repair attempts
- search implementations enforce bounded candidate growth under `TaskBudget`
- portfolio performance on internal holdout beats the best single lane

Master issue:
[#3740](https://github.com/OpenAgentsInc/openagents/issues/3740)

### Issues

#### Solver Core

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-301` / [#3693](https://github.com/OpenAgentsInc/openagents/issues/3693) | done | Created `crates/arc/solvers` with a bounded Tier A seed DSL and pure interpreter covering typed symbols, empty/input/variable grids, crop/paint/rotate/reflect/recolor transforms, extreme/color selectors, sequencing, let-binding, and conditional object checks. |
| `ARC-302` / [#3694](https://github.com/OpenAgentsInc/openagents/issues/3694) | done | Added `arc-solvers::model` with typed digests/ids, solver refusal envelopes, candidate identity + deduplication, budget ledgers, hypothesis envelopes, solve-attempt envelopes, and material-distinctness checks for attempt 2. |
| `ARC-303` / [#3695](https://github.com/OpenAgentsInc/openagents/issues/3695) | done | Added the bounded common verifier in `arc-solvers` with deterministic train-pair execution, per-pair residual reports, augmentation-stability falsification, holdout-on-train checks, budget-aware skip behavior, and explicit refusal for unsupported interactive hypotheses. |
| `ARC-304` / [#3696](https://github.com/OpenAgentsInc/openagents/issues/3696) | done | Added typed proposal/refinement batches, arbiter decisions, solver trace bundles, JSON read/write helpers, and replay validation for proposal, verification, refinement, arbiter, and final-result budget lineage. |

#### Lanes

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-305` / [#3697](https://github.com/OpenAgentsInc/openagents/issues/3697) | done | Added the bounded symbolic lane with typed enumerative seed search, explicit budget accounting, deduplicated proposal/refinement batches, and typed recolor/output-transform repair operators. |
| `ARC-306` / [#3698](https://github.com/OpenAgentsInc/openagents/issues/3698) | done | Added a bounded transductive lane in `arc-solvers` with stable prompt rendering, typed adapter request/response contracts, a generic local-model adapter trait, a Psionic-backed text-generation adapter over `psionic-serve`, explicit parse/task-shape refusals, and fixture-backed tests for both real Psionic reference-runtime failure handling and successful JSON-grid proposals. |
| `ARC-307` / [#3699](https://github.com/OpenAgentsInc/openagents/issues/3699) | done | Added `arc-solvers::recursive` with a typed tiny-model runner trait, explicit checkpoint-vs-scratch bootstrap mode, separate latent-state and answer-state tracking, bounded recursive step traces with halt/continue scoring, optional test-time updates, deduplicated refinement hypotheses, and deterministic tests that verify the lane can refine to a verifier-passing answer without overstating model scope. |
| `ARC-308` / [#3700](https://github.com/OpenAgentsInc/openagents/issues/3700) | done | Added `arc-solvers::mdl` with explicit task-local compression representations, description-length accounting over model/residual/solution bits, `no_pretraining` and warm-start initialization modes, deduplicated static-answer proposals, and deterministic tests showing simplicity-aware ranking plus fixed-shape fill solves under the common verifier. |
| `ARC-309` / [#3701](https://github.com/OpenAgentsInc/openagents/issues/3701) | planned | Implement the portfolio arbiter, second-attempt policy, and cross-lane agreement features. |

#### Evaluation Hygiene

| ID / GitHub | Status | Work |
| --- | --- | --- |
| `ARC-310` / [#3702](https://github.com/OpenAgentsInc/openagents/issues/3702) | planned | Build internal hidden holdout, synthetic regression, concept-slice reporting, and explicit public-eval hygiene enforcement so solver progress cannot depend on public-eval leakage. |

## Epic 4: Interactive ARC-AGI-3 Agents

### Goal

Build a truthful agent runtime over ARC-AGI-3 contracts without collapsing
score, trajectory, and environment truth into ad hoc prompting.

### Exit Criteria

- the agent trait and runner exist
- local and remote `GameClient` flows are typed and tested
- action budgets, refusal behavior, and scorecard truth are explicit
- baseline agents are reproducible and replayable
- interactive run artifacts are compatible with `arc-benchmark`

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `ARC-401` | planned | Port the `GameClient`, runner registry, and baseline agent trait into `arc-solvers` and `arc-benchmark`. |
| `ARC-402` | planned | Port the random baseline and a minimal deterministic scripted baseline. |
| `ARC-403` | planned | Port the ADCR-style baseline as a baseline agent, not as the library contract. |
| `ARC-404` | planned | Add explicit action-budget, reset, and terminal-state refusal semantics for interactive runs. |
| `ARC-405` | planned | Export typed trajectory bundles through the generalized Psionic environment/receipt path once `PLIB-512` and `PLIB-513` land. |
| `ARC-406` | planned | Add repeated interactive benchmark runs over the Psionic eval bridge once `PLIB-514` lands. |
| `ARC-407` | planned | Add local-vs-remote game-client parity fixtures for action execution, checkpoints, and scorecards. |
| `ARC-408` | planned | Add memory/context and prompt-policy surfaces that remain subordinate to typed action and score contracts. |

## Epic 5: ARC-ML Over Psionic

### Goal

Build `arc-ml` as the ARC-specific model and training layer over Psionic
without pretending Psionic is already broad enough for full HRM parity.

### Exit Criteria

- evaluator parity exists before model claims
- at least one small ARC model path runs over Psionic train/eval substrate
- ARC losses, metrics, and search-guide data are typed and replayable
- HRM-class work is explicitly gated on named Psionic roadmap items

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `ARC-501` | planned | Create `crates/arc/ml` and port ARC evaluator logic plus `pass@k` aggregation first. |
| `ARC-502` | planned | Port a small transductive baseline onto `psionic-train` and `psionic-eval` before any HRM-class claim. |
| `ARC-503` | planned | Add ARC-specific losses, metrics, and calibration summaries above Psionic train/eval substrate. |
| `ARC-504` | planned | Add search-guide learning and trace-derived dataset generation for solver guidance models. |
| `ARC-505` | planned | Port recursive tiny-model training/eval flows with explicit intermediate-step traces. |
| `ARC-506` | planned | Port HRM configs and model definitions only after the explicit HRM readiness bar tied to `PLIB-612`, `PLIB-613`, and `PLIB-614` is green. |
| `ARC-507` | planned | Add bounded distributed model execution only after `PLIB-509` and `PLIB-515` make collectives and evidence honest. |
| `ARC-508` | planned | Keep all HRM or transformer parity claims behind tiny deterministic fixtures, checkpoint truth, and replayable eval artifacts before any large-scale reproduction attempt. |

## Epic 6: Evidence, Regression, And Operator Use

### Goal

Make the subtree auditable, usable, and regression-resistant without depending
on Python-only workflows.

### Exit Criteria

- fixture corpora and capability matrices exist
- operator-facing docs exist for local and remote runs
- non-Python smoke suites exist for core static and interactive flows
- score and replay claims remain attributable to typed artifacts

### Issues

| ID | Status | Work |
| --- | --- | --- |
| `ARC-601` | planned | Publish capability matrices for contracts, engine/runtime parity, solver lanes, and ARC-ML readiness. |
| `ARC-602` | planned | Curate fixture corpora for parser/objectization, engine parity, score parity, recording replay, and checkpoint resume. |
| `ARC-603` | planned | Write operator docs for local benchmark runs, remote runs, compatibility-server checks, and fixture replay. |
| `ARC-604` | planned | Add non-Python smoke suites proving core static and interactive paths work without Python sidecars. |
| `ARC-605` | planned | Add bounded import/export tools for recordings, scorecards, or leaderboard-oriented formats only after core benchmark truth is stable. |
| `ARC-606` | planned | Publish one compact acceptance index linking fixtures, capability matrices, benchmark reports, and Psionic dependency gates. |

## Current Execution Order

This is the recommended dependency order for the ARC subtree.

Epic 3 MUST NOT start until exact-match and RHAE benchmark parity fixtures are
green.

That gate is now satisfied in bounded form by `ARC-210`.

### Phase 1: lock governance and shared contracts

1. `ARC-002` through `ARC-005`
2. `ARC-101` through `ARC-104` plus `ARC-111`
3. `ARC-105` through `ARC-110`

### Phase 2: make benchmark truth real before solver claims

4. `ARC-201` through `ARC-205`
5. `ARC-206` through `ARC-213`

### Phase 3: build the static solver program

6. `ARC-301` through `ARC-304`
7. `ARC-305` through `ARC-310`

### Phase 4: add interactive ARC-AGI-3 agents

8. `ARC-401` through `ARC-404`
9. `ARC-407` and `ARC-408`
10. `ARC-405` and `ARC-406` after `PLIB-512` through `PLIB-514`

### Phase 5: add ARC-ML over honest Psionic substrate

11. `ARC-501` through `ARC-505`
12. `ARC-506` through `ARC-508` after `PLIB-509`, `PLIB-515`,
    `PLIB-612`, `PLIB-613`, and `PLIB-614`

### Phase 6: harden evidence and operator workflows

13. `ARC-601` through `ARC-606`

## Program Risks And Dependency Hazards

### Risk 1: contract drift before crates exist

If `arc-core` is not frozen early enough, later crates will grow duplicate
schema and result types.

### Risk 2: solver excitement outrunning benchmark truth

If Epic 3 begins before exact-match and RHAE scoring parity are fixture-backed,
progress claims will rely on ad hoc scoring and unverifiable traces.

### Risk 3: prompt baselines masquerading as architecture

If `ARC-403` arrives before the verifier, arbiter, and DSL core, the subtree
will regress toward benchmark glue instead of a solver system.

### Risk 4: interactive substrate re-implemented locally

If `PLIB-512` through `PLIB-514` slip and ARC works around them with local
one-offs, the Psionic boundary will rot.

### Risk 5: HRM work starting before substrate closure

If `ARC-506` starts before `PLIB-612`, `PLIB-613`, and `PLIB-614`, model work
will either stall or smuggle reusable substrate into ARC crates.

### Risk 6: public-eval leakage

If `ARC-310` is weak, solver iteration will quietly overfit to public benchmark
behavior.

### Risk 7: replay and refusal evidence treated as optional

If artifacts are not mandatory, solver and benchmark regressions will be hard
to attribute.

### Risk 8: score truth split across crates

If `arc-solvers` or `arc-client` start computing benchmark truth, `arc-benchmark`
will stop being canonical.

### Risk 9: local and remote behavior drifting apart

If `ARC-205` and `ARC-407` are weak, local fixture wins will not transfer to
real benchmark runtime behavior.

### Risk 10: `arc-ml` creating dependency cycles

If solver logic backflows into `arc-ml`, or model-family quirks backflow into
Psionic core crates, the architecture will become harder to evolve.

### Risk 11: score-policy drift across upstream docs

If score policy is not versioned early, the subtree will silently mix preview
RHAE prose, methodology updates, and competition-mode semantics.

### Risk 12: local and official online semantics diverging

If operation modes, scorecard lifecycle, session affinity, and rate-limit
behavior are not modeled explicitly, local parity will not transfer to official
online runs.

## Roadmap Rules

### 1. Do not move ARC-specific semantics into Psionic

ARC tasks, actions, scorecards, recordings, DSL semantics, and ARC scorers stay
in `crates/arc/*`.

### 2. Do not count solver progress without benchmark truth

If exact-match, RHAE, replay, or refusal evidence is not typed and fixture
backed, it does not count as subtree progress.

### 3. Do not let prompt policy replace solver architecture

Prompt baselines are allowed.

They are not the center of the roadmap.

Prompt-only agents MUST NOT count as architectural lanes.

### 4. Do not start HRM-class model work before Psionic is ready

The substrate gaps are explicit.

Respect them.

### 5. Do not tune on public evaluation feedback

Use synthetic tasks, train-derived dev slices, internal holdout, and replay
regression sets.

No per-task manual tuning, dataset creation, or acceptance optimization may use
public evaluation runs.

### 6. New reusable primitives enter Psionic first

If a missing primitive would help multiple interactive benchmarks or training
programs, add it to Psionic instead of burying it in ARC.

### 7. Keep score, replay, and checkpoint truth centralized

`arc-benchmark` remains canonical for benchmark truth.

## Benchmark Honesty Rules

- every benchmark claim must declare benchmark family, task or game pack,
  attempt count, wall-clock budget, candidate budget, model backend, and
  local/remote mode
- static exact-match and interactive RHAE must be computed by `arc-benchmark`
- local engine parity must cite fixture packs, not just qualitative behavior
- interactive claims must declare recording format, reset policy, and terminal
  semantics
- interactive claims must declare score-policy version, operation mode, and
  whether competition-mode restrictions were active
- public-eval-based evidence must declare itself as non-regression and
  non-optimization or it does not count as roadmap progress
- unsupported tasks, actions, models, or runtime capabilities must refuse
  explicitly, not degrade silently into incomparable alternate behavior
- benchmark evidence must remain attributable to the library path being claimed,
  not to a notebook-side workaround

## Bottom Line

The ARC subtree should not begin with model ports.

It should begin by making benchmark truth, deterministic runtime behavior, and
shared contracts real in Rust.

The dependency order is:

- `arc-core` and `arc-datasets`
- `arc-engine`, `arc-client`, and `arc-benchmark`
- `arc-solvers`
- `arc-ml` only after named Psionic substrate gates are real

That is the shortest honest path to:

- Rust-native ARC-AGI-1/2 contracts
- Rust-native ARC-AGI-3 runtime and score truth
- a replayable portfolio solver
- bounded ARC-specific model work over Psionic
- an auditable separation between ARC semantics and reusable substrate

That is now the canonical ARC roadmap.
