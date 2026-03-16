# ARC Subtree Spec
Status: draft for coding agents
Date: 2026-03-15
Audience: coding agents and human maintainers working in `crates/arc/*`,
`crates/psionic/*`, and `docs/*`.

## 0. Why this exists

This spec defines the target ARC namespace under `crates/arc/` and the first
serious solver program that should live inside it.

It answers a narrower question than the full Psionic roadmap:

> how should OpenAgents port the relevant ARC Prize stack into Rust without
> collapsing ARC-specific logic into Psionic?

This is not current MVP product scope from `docs/MVP.md`.
It is a bounded architecture and implementation spec for:

- ARC-AGI-1 and ARC-AGI-2 static tasks
- ARC-AGI-3 interactive environments
- benchmark, recording, and scorecard infrastructure
- solver and model work that consumes Psionic as reusable substrate

The main research takeaway behind this spec is:

- ARC-AGI-2 stresses symbolic interpretation, compositional reasoning,
  contextual rule application, and efficiency.
- ARC Prize 2024 and 2025 both point toward hybrid systems rather than one
  monolithic method family.
- ARC Prize 2025 centers the refinement loop.
- ARC-AGI-3 makes interactive reasoning, action efficiency, replay, planning,
  and memory first-class.

Therefore the ARC subtree must be built around:

1. object-centric representations
2. program-like hypotheses
3. task-local refinement
4. portfolio solving across multiple lanes
5. strict budgeted verification and replay
6. trace capture for learning and audit
7. a clean owner split between ARC semantics and Psionic substrate

## 0.1 Normative Language

The keywords **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY**
are used as described in RFC 2119.

- MUST / MUST NOT: required for conformance
- SHOULD / SHOULD NOT: recommended but not strictly required
- MAY: optional behavior

Coding agents implementing ARC crates MUST treat all normative requirements as
binding constraints.

## 0.2 Doc Authority

The ARC subtree currently has four canonical documents with different roles:

- `crates/arc/docs/INDEX.md`
  - compact entrypoint linking the current canonical ARC docs and future
    acceptance-matrix anchors
- `crates/arc/docs/spec.md`
  - canonical architecture, ownership, and crate-boundary spec
- `crates/arc/docs/ROADMAP.md`
  - canonical dependency-ordered execution roadmap
- `crates/arc/docs/CLAIMS.md`
  - canonical ARC claim vocabulary and minimum artifact expectations
- `crates/arc/docs/UPSTREAM_TARGETS.md`
  - canonical first-pass upstream benchmark and protocol target freeze
- `docs/audits/2026-03-15-arcprize-rust-port-and-psionic-integration-audit.md`
  - port-source and upstream-mapping audit, not the canonical current-state
    contract

Acceptance artifacts and future ARC matrices SHOULD use the frozen claim names
from `crates/arc/docs/CLAIMS.md` rather than inventing near-duplicates.
Readers who need the compact document map SHOULD start at
`crates/arc/docs/INDEX.md`.

## 1. Architectural fit and boundaries

This spec must obey `docs/OWNERSHIP.md`.

### 1.1 `crates/arc/*` owns ARC-specific semantics

The ARC subtree owns:

- ARC-AGI-2 task, grid, and dataset semantics
- ARC-AGI-3 action, frame, level, recording, and scorecard semantics
- deterministic ARC engine behavior
- ARC local/remote client wrappers and compatibility-server behavior
- ARC canonicalization, object extraction, and relation views
- ARC solver DSL, hypothesis IR, search, refinement, verification, and
  arbitration
- ARC benchmark logic, scoring, checkpoints, and run manifests
- ARC-specific model definitions, evaluator glue, and metrics

### 1.2 `crates/psionic/*` owns reusable substrate

Psionic owns only the reusable pieces that should serve more than ARC:

- dataset manifests and packing substrate
- eval-run orchestration and aggregate reporting
- research run/experiment substrate
- local model serving, routing, and provider inventory
- train/runtime/collective execution substrate
- reusable environment/session receipts and evidence
- generic artifacts, traces, and execution provenance

Psionic must not own ARC task schema, ARC action vocabulary, ARC scorecards,
ARC DSL semantics, or ARC-specific scorers.

### 1.3 Kernel/Nexus does not own the hot loop

Kernel or Nexus may later own:

- experiment registration
- run receipts
- accepted-outcome projections
- public/internal score publication
- later marketplace or verification projections

Kernel must not own:

- per-candidate search steps
- per-branch verifier logic
- hypothesis generation
- ARC object extraction
- task-local adaptation state
- ARC scorecard or recording truth

### 1.4 Apps do not own solver semantics

`apps/*` may expose operators, dashboards, or CLI control, but solver truth
lives in `crates/arc/*`.

### 1.5 Framework rule

ARC semantics belong above generic tensor/compiler/runtime substrate, not inside
`psionic-core`, `psionic-ir`, `psionic-runtime`, or `psionic-train`.

## 2. ARC subtree thesis

The solver should be a hybrid portfolio system, not a single monolithic model.

The ARC subtree must support two benchmark modes:

- static solving for ARC-AGI-1 and ARC-AGI-2
- interactive action selection for ARC-AGI-3

Those two modes should share as much as possible:

- grid and object representations
- task and episode trace contracts
- verifier and refinement infrastructure
- replay and budget accounting
- benchmark evidence and acceptance artifacts

The solver itself must combine four families of capability:

### 2.1 Induction / program synthesis

Infer an explicit latent transformation program from demonstration pairs.

Use this for:

- exact computations
- multi-step composition
- controllable reasoning
- hypothesis debugging
- high-confidence generalization from few examples

### 2.2 Transduction / direct output prediction

Predict output grids directly from task context and test input.

Use this for:

- perceptual or fuzzy concepts
- pattern completion
- weakly symbolic visual regularities
- proposals that seed or rescue induction

### 2.3 Recursive / iterative refinement

Repeatedly improve a candidate answer or action plan using verifier-derived
feedback.

Use this for:

- answer repair
- branch pruning
- iterative latent updates
- masked or denoising loops
- self-correction
- test-time training or adaptation

### 2.4 Compression / simplicity pressure

Prefer explanations that are short, stable, and falsifiable.

Use this for:

- candidate ranking
- search pruning
- distinguishing brittle fits from actual rules
- MDL-style adaptation lanes

## 3. Proposed `crates/arc/` tree

The target layout is a real subtree under `crates/arc/`.
`crates/arc/spec.md` is the namespace spec, not a root Cargo package.

| Directory | Package | Owns |
| --- | --- | --- |
| `crates/arc/core` | `arc-core` | shared ARC schema and value types: grids, tasks, actions, frames, states, operation modes, recordings, scorecards, score-policy IDs, canonicalization, objects, relations, budgets, and solver result envelopes |
| `crates/arc/datasets` | `arc-datasets` | ARC-AGI-1/2 loaders, ARC dataset lineage, augmentation builders, synthetic-task manifests, and Psionic dataset-manifest export |
| `crates/arc/engine` | `arc-engine` | deterministic local game engine, sprite/camera/level logic, game package loading, and replay-safe action execution |
| `crates/arc/client` | `arc-client` | ARC REST client, cookie-affine session handling, local/remote wrappers, and compatibility server behavior |
| `crates/arc/benchmark` | `arc-benchmark` | exact-match scoring, versioned interactive RHAE scoring, recordings, checkpoints, scorecards, run manifests, resume semantics, and eval summaries |
| `crates/arc/solvers` | `arc-solvers` | ARC DSL, hypothesis IR, search/refinement control, verifier, arbiter, baseline agents, prompt policies, and Psionic-backed local solver integration |
| `crates/arc/ml` | `arc-ml` | HRM and baseline model definitions, train/eval bridges, ARC-specific metrics, and Psionic-backed training adapters |

### 3.1 Dependency shape

Keep the subtree acyclic and predictable:

- `arc-core` must not depend on other ARC crates.
- `arc-datasets` may depend on `arc-core` and Psionic data crates.
- `arc-engine` may depend on `arc-core` only.
- `arc-client` may depend on `arc-core` and `arc-engine`.
- `arc-benchmark` may depend on `arc-core`, `arc-engine`, `arc-client`, and
  Psionic eval/research crates.
- `arc-solvers` may depend on `arc-core`, `arc-engine`, `arc-client`, and
  Psionic serve/runtime/research crates.
- `arc-ml` may depend on `arc-core`, `arc-datasets`, and Psionic train/eval
  crates. Solver-facing adapters should point from `arc-solvers` to `arc-ml`,
  not the reverse.

### 3.2 Day-one implementation rule

Do not create every crate before there is code for it.

The target split above is the architecture.
The phased creation order is:

1. `arc-core`
2. `arc-datasets`
3. `arc-engine`
4. `arc-client`
5. `arc-benchmark`
6. `arc-solvers`
7. `arc-ml`

`crates/arc/core` is now the first live ARC package in the workspace. It is the
shared owner for ARC task IDs, grid/task contracts, deterministic analysis
summaries, solver-facing budget/result envelopes, and the first typed
ARC-AGI-3 action/frame/recording/scorecard contracts. Benchmark scoring,
client/session behavior, score-policy IDs, and solver search state remain out
of scope for this crate and stay in higher ARC packages.

## 4. Shared domain model

The old single-crate `psionic-arc` idea is replaced by a shared-domain core
plus specialized crates above it.

### 4.1 `arc-core` owns the common contracts

`arc-core` must define the stable types that multiple ARC crates consume.

That includes:

- static task types
- interactive action and frame types
- interactive state and operation-mode types
- recording and scorecard shapes
- score-policy identifiers and benchmark metadata envelopes
- canonical task/object/relation views
- budget envelopes
- solve-result envelopes
- trace locators and digest references

`arc-core` may begin as one crate, but its internal boundary is still
normative:

- schema layer
  - tasks, grids, actions, frames, scorecards, recordings, score-policy IDs,
    operation modes, and recording-envelope identifiers
- analysis layer
  - canonicalization, objects, relation graphs, correspondence candidates, and
    task-level feature bundles
- execution-envelope layer
  - budgets, refusal taxonomy, solve results, trace locators, and other
    replay-safe envelopes shared across benchmark and solver code

#### Fundamental task types

```rust
pub struct ArcTask {
    pub task_id: String,
    pub benchmark: ArcBenchmark,
    pub train: Vec<ArcExamplePair>,
    pub test: Vec<ArcTestInput>,
}

pub struct ArcExamplePair {
    pub input: ArcGrid,
    pub output: ArcGrid,
}

pub struct ArcTestInput {
    pub index: usize,
    pub input: ArcGrid,
}
```

```rust
pub struct ArcGrid {
    pub height: u8,
    pub width: u8,
    pub cells: Vec<u8>,
}
```

`ArcGrid` storage is row-major.

Index formula:

`index = row * width + column`

`cells` MUST be stored as a flat contiguous buffer of length
`height * width`.

Static ARC task grids MUST satisfy:

- `height in 1..=30`
- `width in 1..=30`
- `color in 0..=9`

These limits apply to ARC task grids.
Interactive ARC-AGI-3 frame rasters remain governed by their frame and engine
contracts rather than by `ArcGrid`.

#### Interactive environment types

```rust
pub enum ArcBenchmark {
    ArcAgi1,
    ArcAgi2,
    ArcAgi3,
    InternalSynthetic,
    InternalHoldout,
}

pub struct ArcEpisodeStep {
    pub step_index: u32,
    pub action: ArcAction,
    pub observation: ArcObservation,
    pub terminal: bool,
}
```

```rust
pub enum ArcOperationMode {
    Normal,
    Offline,
    Online,
    Competition,
}

pub enum ArcGameState {
    NotFinished,
    Win,
    GameOver,
}
```

`ArcAction`, `ArcObservation`, `FrameData`, `Scorecard`, and `Recording`
belong in `arc-core` because they are benchmark contracts, not solver
implementation details.

Interactive contracts must preserve the upstream ARC-AGI-3 wire semantics:

- the standardized seven-action vocabulary: `RESET`, `ACTION1` through
  `ACTION7`
- `ACTION6` as a coordinate-bearing action with `x` and `y` in `0..=63`
- `ACTION7` as the standardized undo action where supported
- `available_actions` on every returned frame as the authoritative dynamic
  action-space surface
- `ACTION6` availability without implicit enumeration of active coordinates
- `guid` plus sticky cookie-jar behavior for online sessions
- scorecard metadata envelopes including `source_url`, `tags`, and `opaque`

### 4.2 Canonicalized task state

Canonicalization and object extraction should be shared by solvers, benchmarks,
and model code, so their stable data model also belongs in `arc-core`.

```rust
pub struct CanonicalTask {
    pub raw: ArcTask,
    pub normalized_train: Vec<CanonicalPair>,
    pub object_views: Vec<ObjectView>,
    pub relation_views: Vec<RelationGraph>,
    pub global_features: TaskFeatureBundle,
}
```

Canonicalization must include:

- optional color renaming normalization
- grid padding and bounds metadata
- connected-component extraction
- hole detection
- symmetry signatures
- bounding-box inventory
- repetition and alignment signatures
- train/test dimension summaries
- correspondence candidates between train input and train output

Canonicalization MUST produce stable ordering for:

- objects
- relations
- correspondence candidates

Tie-breaking MUST use:

1. bounding-box top-left
2. area
3. color histogram
4. stable fallback to original scan order

### 4.3 Object and relation model

```rust
pub struct BitGrid {
    pub width: u8,
    pub height: u8,
    pub bits: Vec<u64>,
}
```

`BitGrid` is row-major and packed contiguously.

Bit index formula:

`bit_index = row * width + column`

Packing rules:

- `word_index = bit_index / 64`
- `bit_offset = bit_index % 64`
- the least-significant bit in each `u64` is the first stored bit for that word
- trailing bits past `width * height` MUST be zero

```rust
pub struct ArcObject {
    pub object_id: ObjectId,
    pub color_histogram: SmallVec<[u8; 10]>,
    pub bbox: Rect,
    pub mask: BitGrid,
    pub holes: u8,
    pub connectivity: ConnectivityKind,
    pub centroid: (f32, f32),
    pub shape_signature: ShapeSignature,
}
```

```rust
pub struct RelationGraph {
    pub objects: Vec<ArcObject>,
    pub edges: Vec<ObjectRelation>,
}
```

`ObjectRelation` must represent:

- adjacency
- overlap
- containment
- touch
- distance ordering
- row/column alignment
- symmetry pairing
- count relations
- color mapping
- source/target correspondence hypotheses

### 4.4 Shared budget and result contracts

These are cross-crate contracts, so the envelope types belong in `arc-core`
even though `arc-solvers` produces them.

Every solve attempt must bind a `TaskBudget` with at least:

- `max_wall_ms`
- `max_candidates`
- `max_verifier_evals`
- `max_train_pair_execs`
- `max_refinement_steps`
- `max_model_forward_calls`
- `max_ttt_updates`
- `max_memory_mb`

Every solve attempt must produce a `SolveResult`:

- `status`: `solved | unsolved | budget_exhausted | refused | invalid`
- `task_id`
- `attempt_index`
- `selected_answer`
- `selected_lane`
- `confidence`
- `verification_summary`
- `budget_summary`
- `trace_digest`
- `trace_locator`
- `seed_bundle`
- `solver_manifest`

### 4.5 Determinism requirements

ARC solver and benchmark behavior MUST be deterministic under the tuple:

`(task_id, solver_manifest_digest, seed_bundle_digest, TaskBudget)`

The following components MUST produce deterministic outputs:

- DSL program execution
- canonicalization and object extraction
- verifier evaluation
- arbiter ranking and selection
- benchmark scoring
- replay of stored trace bundles

Sources of nondeterminism such as random search ordering, stochastic model
calls, or parallel scheduling MUST be seeded through the `seed_bundle` and
recorded in the trace bundle.

Parallel execution MAY reorder internal work, but the final solver outputs MUST
remain deterministic under replay.

## 5. Solver-specific ownership

### 5.1 `arc-solvers` owns the cognitive inner loop

`arc-solvers` owns:

- ARC DSL and interpreter
- hypothesis IR
- search and refinement control
- lane traits and lane implementations
- verifier and falsifier logic
- portfolio arbitration
- solver trace generation

### 5.2 `arc-benchmark` owns score and run truth

`arc-benchmark` owns:

- exact-match scoring for static tasks
- versioned RHAE policy for interactive tasks, including the current
  weighted-and-squared methodology/competition policy and any explicit
  compatibility variants we need for upstream parity
- scorecard open/get/close semantics, timeout/finalization behavior, and
  competition-mode restrictions
- recordings, JSONL replay compatibility, and replay bundles
- checkpoints and resume behavior
- run manifests and aggregate reports

### 5.3 `arc-ml` owns model and training glue

`arc-ml` owns:

- HRM and other ARC-specific model definitions
- model config schemas
- ARC-specific losses and metrics
- evaluator wrappers
- training/eval adapters onto Psionic

### 5.4 Cross-crate concern matrix

The owner split below is normative for agent implementations:

| Concern | Owner |
| --- | --- |
| state transition correctness | `arc-engine` |
| local replay and deterministic checkpoint application | `arc-engine` |
| transport, sessions, cookies, retry, and backoff | `arc-client` |
| local compatibility serving and REST schema conformance | `arc-client` |
| scorecard lifecycle policy | `arc-benchmark` |
| competition-mode scoring restrictions | `arc-benchmark` |
| replay acceptance and benchmark result truth | `arc-benchmark` |
| hypothesis generation, verifier logic, and attempt policy | `arc-solvers` |
| ARC-specific losses, metrics, and evaluator glue | `arc-ml` |

If a behavior touches more than one crate, the first owner above remains the
policy authority and the others consume typed contracts from it.

## 6. ARC DSL / program IR

The solver must define an explicit ARC program IR.
This IR is the heart of the induction lane and belongs in `arc-solvers`.

### 6.1 DSL principles

The IR must be:

- typed
- small
- inspectable
- serializable
- executable deterministically
- expressive enough for common ARC transformations
- stable under tracing and replay

It must not use Python as the canonical IR or fallback executor.
Upstream Python remains reference material only, not a planned ARC runtime.

### 6.2 Tier A DSL scope for v1

Tier A is the only required DSL surface for the first honest solver claim.
Do not widen the language before Tier A fixtures and replay are green.

Selectors:

- select all objects
- select by color
- select by size, count, holes, or connectivity
- select by relation
- select extreme objects

Constructors:

- empty grid
- copy input grid
- crop region
- pad or resize
- paint rectangle, line, or mask
- paste or transform object
- tile, repeat, stack, or align

Object transforms:

- translate
- reflect
- rotate
- recolor
- scale by integer factor
- extract outline or interior

Structural ops:

- connected components
- flood fill
- count
- frame or border detection
- symmetry detection

Control flow:

- map over selected objects
- filter
- if/else
- reduce/fold
- ordered composition
- bounded loop over object list

Symbol binding:

- bind simple task-local symbols from demonstrations
- use bound symbols in downstream transforms

### 6.3 Tier B DSL scope after v1 closure

Tier B expands research breadth once Tier A is honest.
Tier B features MUST NOT be implemented before Tier A fixtures, replay, and
solver claims are green.

Selectors:

- select by contextual predicate

Object transforms:

- fill holes
- merge or split
- order by relation and place

Structural ops:

- grouping or partition
- correspondence inference

Control flow:

- recurrence over ordered placements
- contextual gating

Symbol binding:

- define richer task-local symbol meanings from demonstrations
- bind symbol/value lookup tables

### 6.4 Tier C DSL scope

Tier C is explicitly out of v1 and should stay experimental until Tier A and B
replay evidence is stable.

Examples:

- induced symbol tables with broad contextual rebinding
- higher-order macros or learned macro expansion
- learned primitives that are not reducible to typed Tier A or Tier B ops
- open-ended recurrence beyond explicitly bounded search limits

### 6.5 IR shape

```rust
pub enum ArcExpr {
    Grid(GridExpr),
    Objects(ObjectExpr),
    Scalar(ScalarExpr),
    Bool(BoolExpr),
    Color(ColorExpr),
    ProgramCall { op: OpId, args: Vec<ArcExpr> },
    If {
        cond: Box<ArcExpr>,
        then_branch: Box<ArcExpr>,
        else_branch: Box<ArcExpr>,
    },
    Let {
        name: Symbol,
        value: Box<ArcExpr>,
        body: Box<ArcExpr>,
    },
}

pub struct ArcProgram {
    pub inputs: ArcProgramInputs,
    pub body: ArcExpr,
    pub metadata: ArcProgramMetadata,
}
```

### 6.6 Interpreter properties

The interpreter must be:

- pure
- deterministic
- side-effect free
- total over supported inputs
- explicitly refusing on unsupported semantics
- traceable per node

No hidden fallback behavior is allowed.

Interpreter safety rules:

- DSL execution MUST enforce explicit grid bounds
- DSL programs MUST refuse if they produce grids larger than the maximum ARC
  grid size

## 7. Solver lanes

The system must support multiple lanes with a common interface.

```rust
pub trait SolverLane {
    fn id(&self) -> SolverLaneId;

    fn propose(
        &self,
        task: &CanonicalTask,
        budget: &TaskBudget,
        ctx: &LaneContext,
    ) -> LaneProposalBatch;

    fn refine(
        &self,
        task: &CanonicalTask,
        budget: &TaskBudget,
        ctx: &LaneContext,
        prior: &[Hypothesis],
    ) -> LaneProposalBatch;
}
```

### 7.1 Normative solver object model

The solver object model below is normative.
Local crates must not invent alternate proposal or verification shapes.

```rust
pub struct HypothesisId(pub String);

pub enum HypothesisKind {
    Program,
    DirectGrid,
    Repair,
    InteractivePlan,
    CompressionModel,
    Refusal,
}

pub struct CandidateIdentity {
    pub canonical_signature: String,
    pub answer_digest: Option<String>,
    pub program_digest: Option<String>,
    pub action_plan_digest: Option<String>,
}

pub struct PlannedActionStep {
    pub action: ArcAction,
    pub expected_state: Option<ArcGameState>,
    pub expected_level_index: Option<u16>,
    pub reset_marker: bool,
}

pub struct Hypothesis {
    pub id: HypothesisId,
    pub kind: HypothesisKind,
    pub lane_id: SolverLaneId,
    pub attempt_index: u8,
    pub candidate_identity: CandidateIdentity,
    pub program: Option<ArcProgram>,
    pub static_answer: Option<ArcGrid>,
    pub interactive_plan: Option<Vec<PlannedActionStep>>,
    pub local_score: f32,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

pub struct LaneProposal {
    pub hypothesis: Hypothesis,
    pub local_rank: u32,
    pub rationale_digest: String,
}

pub enum LaneBatchStatus {
    Proposed,
    Empty,
    Refused,
    BudgetExhausted,
}

pub struct LaneProposalBatch {
    pub lane_id: SolverLaneId,
    pub phase: ProposalPhase,
    pub status: LaneBatchStatus,
    pub proposals: Vec<LaneProposal>,
    pub refusal: Option<RefusalEnvelope>,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

pub struct VerificationReport {
    pub hypothesis_id: HypothesisId,
    pub verifier_config_digest: String,
    pub exact_fit: bool,
    pub pair_results: Vec<PairVerificationResult>,
    pub falsification_checks: Vec<FalsificationCheckResult>,
    pub simplicity_score: f32,
    pub stability_score: f32,
    pub spuriousness_risk: f32,
    pub verifier_pass: bool,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

pub struct ArbiterDecision {
    pub task_id: String,
    pub attempt_index: u8,
    pub selected_hypothesis: Option<HypothesisId>,
    pub ranked_hypotheses: Vec<HypothesisId>,
    pub second_attempt_allowed: bool,
    pub refusal: Option<RefusalEnvelope>,
    pub decision_reason: String,
    pub trace_locator: TraceLocator,
    pub budget_delta: BudgetCounterDelta,
}

pub struct TraceBundleManifest {
    pub schema_version: u32,
    pub bundle_id: String,
    pub task_id: String,
    pub seed_bundle_digest: String,
    pub solver_manifest_digest: String,
    pub lane_batches: Vec<TraceLocator>,
    pub verification_reports: Vec<TraceLocator>,
    pub arbiter_decision: TraceLocator,
    pub budget_ledger_digest: String,
    pub final_result_digest: String,
}

pub struct BudgetCounterDelta {
    pub wall_ms: u64,
    pub candidates_generated: u32,
    pub verifier_evals: u32,
    pub train_pair_execs: u32,
    pub refinement_steps: u32,
    pub model_forward_calls: u32,
    pub ttt_updates: u32,
    pub peak_memory_mb: u32,
}
```

Normative rules:

- `HypothesisId` must be stable within one run from lane identity, attempt
  index, and canonical candidate identity.
- `CandidateIdentity.canonical_signature` MUST be a deterministic digest
  derived from:
  - hypothesis kind
  - normalized program AST if present
  - normalized output grid if present
  - normalized action plan if present
- candidate deduplication is keyed by
  `CandidateIdentity.canonical_signature`
  - static duplicates share the same answer digest and, where present, the same
    program digest
  - interactive duplicates share the same normalized action-plan digest
- candidate identity MUST treat semantically equivalent programs as identical
  even if syntactically different, when they produce identical canonical
  execution traces on all train pairs
  - implementations MAY approximate this equivalence using canonical program
    normalization or execution hashing
- `LaneProposalBatch.status = Refused` means the lane could not legally or
  honestly run because of capability, policy, or minimum-budget constraints
- `LaneProposalBatch.status = Empty` means the lane ran within budget and found
  no admissible distinct proposal
- `LaneProposalBatch.status = BudgetExhausted` means the lane began valid work
  but exhausted budget before producing an admissible distinct proposal batch
- `VerificationReport` must be deterministic for the same canonical task,
  hypothesis, verifier config digest, and seed bundle
- verifier pass/fail belongs to `VerificationReport`; final acceptance belongs
  to the arbiter
- every propose, refine, verify, and arbitrate step must emit exactly one
  `BudgetCounterDelta`
- the sum of all `BudgetCounterDelta` entries in a trace bundle MUST equal the
  `budget_summary` recorded in `SolveResult`
- each step MUST check budget availability before performing work
- lane implementations MUST check `TaskBudget` before expanding search
  branches, model calls, or program execution
- any step that would exceed `TaskBudget` MUST refuse before executing
- budget counters are monotonic; any step that would overdraw a `TaskBudget`
  must refuse or stop before doing out-of-budget work
- budget counters MUST never decrease or reset within a run
- `TraceBundleManifest` must be sufficient to replay proposal, verification,
  arbiter, and final-result lineage without notebook-only metadata
- trace bundles MUST contain sufficient information to reproduce candidate
  generation order, verification results, arbiter ranking decisions, final
  answer selection, and budget accounting
- trace bundles MUST NOT depend on external notebook state
- attempt 2 is materially distinct only when it changes at least one of
  answer digest, program digest, action-plan digest, or hypothesis kind, and
  is not just a prompt, temperature, or ordering variant over the same
  canonical signature

### 7.2 Hypothesis lifecycle

A hypothesis progresses through these states:

1. proposed
2. deduplicated
3. verified
4. ranked
5. accepted or rejected

Transitions:

- proposal to deduplication occurs within `arc-solvers`
- deduplicated hypotheses are evaluated by the verifier
- verified hypotheses enter arbiter ranking
- arbiter selects or rejects candidates

Hypotheses MUST remain immutable after verification except for trace annotation
metadata.

### 7.3 Lane A: symbolic induction / program search

Required behavior:

- generate candidate programs
- execute them on train pairs
- rank by fit, simplicity, and stability
- repair near-miss programs
- carry full execution traces

Allowed search strategies:

- beam search
- best-first search
- MCTS
- evolutionary search
- abstraction library growth
- sketch completion
- branch-and-bound
- typed enumerative search

### 7.4 Lane B: transductive neural lane

Required behavior:

- consume multi-example task context
- propose one or more output grids
- support test-time adaptation
- support iterative self-refinement
- expose calibrated score or uncertainty
- support exact output shape prediction

### 7.5 Lane C: recursive tiny-model lane

Required behavior:

- initialize from tiny checkpoint or from scratch
- maintain latent state and answer state separately
- apply bounded recursive improvement steps
- support halt/continue scoring
- allow optional test-time updates
- expose intermediate answer snapshots

### 7.6 Lane D: MDL / compression lane

Required behavior:

- optimize a compact task-local model or code representation
- score candidates by compression of demonstrations plus solution
- allow no-pretraining mode
- provide an independent ranking signal

### 7.7 Lane E: learned search-guide lane

This lane is optional in v1 and required in later stages.

It should guide symbolic search with:

- branch priors
- value estimates for partial programs
- repair suggestion distributions
- output-shape priors
- object-correspondence priors

## 8. Common verifier and portfolio arbiter

The solver must have one common verifier.

```rust
pub trait CandidateVerifier {
    fn evaluate(
        &self,
        task: &CanonicalTask,
        hypothesis: &Hypothesis,
    ) -> VerificationReport;
}
```

The verifier must:

- run candidate program or answer on all train pairs
- measure exact fit
- compute per-pair residuals
- detect invalid grids or dimensions
- compute invariance and stability checks
- compute simplicity/compression features
- identify likely spurious fits

Required falsification checks, where budget allows:

1. augmentation stability
2. counterexample pressure
3. holdout-on-train for tasks with enough demonstrations
4. minimality pressure
5. cross-lane agreement

The portfolio arbiter must consider:

- exact verifier fit
- simplicity
- stability
- lane reliability priors
- compute spent so far
- diversity of candidate explanations
- historical calibration on internal holdout slices

Attempt 2 is allowed only when it is materially distinct from attempt 1.

Two attempts are materially distinct only if they differ in at least one of:

- answer digest
- program digest
- action-plan digest
- hypothesis kind
- DSL program structure

Changing only the following does NOT qualify as a materially distinct attempt:

- search order
- prompt wording
- temperature
- beam ordering
- evaluation ordering

### 8.1 Public-eval hygiene rules

The solver program must enforce these rules:

- no per-task manual solver tuning on public evaluation tasks
- no acceptance artifact generated from public evaluation tasks unless it is
  labeled explicitly as non-regression and non-optimization evidence
- public evaluation runs must not feed search-guide, repair-model, or
  calibration training datasets
- internal hidden holdout must stay disjoint from synthetic tasks derived from
  public evaluation tasks

Canonical operator policy and validator:

- `crates/arc/docs/PUBLIC_EVAL_HYGIENE.md`
- `scripts/lint/arc-public-eval-hygiene-check.sh`

## 9. ARC-AGI-3 seam

The v1 solver targets static ARC first, but the subtree must already prepare for
ARC-AGI-3 interactive episodes.

### 9.1 What to preserve now

The system must already expose:

- state snapshots
- action-like traces
- replay logs
- budget and accounting
- memory-state hooks
- branch and hypothesis history
- verifier feedback loops

### 9.2 Static vs interactive owner split

- `arc-engine` owns deterministic state transition and local replay.
- `arc-client` owns remote/local transport wrappers and compatibility-server
  behavior.
- `arc-solvers` owns action-selection policy and planning.
- `arc-benchmark` owns RHAE scoring, recordings, scorecards, and episode
  summaries.

### 9.3 What not to do yet

Do not prematurely turn the static solver into a generic game-agent framework.

The seam is:

- reuse perception and objectization
- reuse hypothesis, verification, and refinement infrastructure
- replace final-output selection with action selection when moving into
  interactive benchmarks

### 9.4 Interactive protocol and score requirements

The interactive runner must preserve the upstream ARC-AGI-3 operating split:

- `Normal`, `Offline`, `Online`, and `Competition` as explicit mode surfaces
- local mode as the fast, no-rate-limit development path
- online mode as the official scorecard/replay path with cookie-affine
  session handling and rate-limit/backoff-aware clients
- competition mode as a stricter benchmark policy surface, not just a flag on
  ordinary online runs

Competition-mode semantics must be modeled explicitly:

- API interaction only
- one scorecard only
- one `make`/open per environment
- level resets allowed, but game resets collapse to level resets
- no inflight `get_scorecard` behavior
- scoring against all available environments, not only the subset the agent
  happened to interact with

Interactive score policy must also be versioned explicitly.
At minimum, `arc-benchmark` must model the current methodology policy:

- `level_score = min(1.0, (human_baseline_actions / ai_actions)^2)`
- per-game score as a level-index-weighted average of per-level scores
- total score as the mean of per-game scores

If earlier upstream preview docs imply a different aggregation or weighting
story, that must be represented as a named compatibility policy rather than as
an ambiguous default.

## 10. Integration with existing Psionic subsystems

ARC must use Psionic where the substrate is already reusable and real.

### 10.1 `arc-datasets` -> `psionic-data`

Use for:

- task storage
- dataset manifests
- split manifests
- synthetic dataset lineage packaging
- tokenizer metadata for neural lanes, if needed

### 10.2 `arc-benchmark` -> `psionic-eval` and `psionic-research`

Use for:

- eval-run orchestration
- aggregate reports
- regression packs
- repeated-run aggregation
- experiment manifests for prompt, policy, and model sweeps

### 10.3 `arc-solvers` -> serving/runtime substrate

Use:

- `psionic-serve`
- `psionic-router`
- `psionic-provider`
- `psionic-runtime`

for:

- local model execution
- model inventory
- structured inference surfaces
- execution receipts and evidence

### 10.4 `arc-solvers` and `arc-benchmark` -> environment substrate

Use `psionic-environments` only for reusable interactive-environment session
and evidence primitives once those are strong enough for structured turn data.

ARC must not force ARC-specific action vocabulary into Psionic.
Those Psionic contracts must remain benchmark-agnostic, action-schema-agnostic,
score-policy-agnostic, and game-state-taxonomy-agnostic.

### 10.5 `arc-ml` -> train/runtime/collectives

Use:

- `psionic-train`
- `psionic-runtime`
- `psionic-collectives`

for:

- training graphs
- optimizer and checkpoint substrate
- distributed execution
- model-state IO

Do not force full train-system breadth before ARC-specific lanes exist.

## 11. Missing Psionic primitives

These gaps belong in Psionic, not in `crates/arc/*`.

Every interactive-environment primitive below must stay benchmark-agnostic,
action-schema-agnostic, score-policy-agnostic, and game-state-taxonomy-agnostic.

### 11.1 Structured interactive environment turns

ARC-AGI-3 needs reusable structured turn payloads:

- typed observations
- typed actions
- typed resets
- typed terminal state changes

Recommended Psionic additions:

- `EnvironmentObservation`
- `EnvironmentAction`
- `EnvironmentStepResult`
- resume-safe session snapshots for interactive environments

### 11.2 Trajectory and episode receipts

ARC-AGI-3 scorecards and recordings are episode-first.

Recommended Psionic additions:

- typed episode receipt families
- per-turn observation/action/result hashing
- final episode summaries distinct from text-session summaries

### 11.3 Interactive benchmark runtime bridge

Recommended Psionic additions:

- generic bridge from interactive environment sessions to eval samples
- time, token, and trajectory evidence capture per episode
- helpers for repeated interactive benchmark runs

### 11.4 Training-class operator coverage for HRM-class work

Recommended Psionic additions:

- gather
- scatter-add
- argmax
- pad
- BCE-with-logits
- softmax cross-entropy
- ACT-style loop support
- training-class SDPA or flash-attention-class kernels
- sparse-embedding parameter/update support

### 11.5 Real collective execution

Recommended Psionic additions:

- actual collective execution backends wired to `psionic-collectives`
- multi-rank optimizer and gradient exchange runtime
- distributed checkpoint IO that training jobs can consume directly

### 11.6 Stronger model-state IO

Recommended Psionic additions:

- stronger train-state save/load for arbitrary ARC models
- stable model-state manifests for promoted ARC checkpoints
- small-model local training harnesses without app-owned glue

### 11.7 Minimum HRM readiness bar

`arc-ml` must not begin HRM-class parity work until the following are green:

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
  - only when the claimed HRM workload actually exercises multi-rank
    collectives

## 12. Port order and milestones

### Phase 1: contracts and deterministic domain core

Build:

- `arc-core`
- `arc-datasets`

Deliver:

- ARC-AGI-2 task schema
- ARC-AGI-3 action/frame/recording/scorecard schema
- ARC-AGI-3 action-space, game-state, operation-mode, and score-policy IDs
- canonicalization and object extraction
- augmentation builders and dataset lineage
- exact-match scorer inputs and result envelopes

Acceptance:

- static task JSON round-trip fixtures
- deterministic object extraction fixtures
- dataset manifests emitted through `psionic-data`

### Phase 2: local engine and client

Build:

- `arc-engine`
- `arc-client`

Deliver:

- ARCEngine behavior
- metadata discovery
- local wrapper with offline-mode parity
- remote wrapper with online/competition-mode semantics
- REST client
- local compatibility server

Acceptance:

- one local sample game loaded and executed in Rust
- one REST compatibility test against the ARC docs schema
- one cookie-affine remote-session fixture with typed 429/backoff handling

### Phase 3: benchmark runtime

Build:

- `arc-benchmark`

Deliver:

- static benchmark runner
- interactive benchmark runner
- versioned interactive score policy implementation
- checkpoints
- recordings with JSONL compatibility
- scorecards and scorecard lifecycle policy
- cost and token accounting

Acceptance:

- resumable Rust benchmark runs for ARC-AGI-2 and ARC-AGI-3
- interactive replay parity on captured fixtures
- score-policy fixtures for current methodology/competition semantics
- competition-mode policy fixtures

### Phase 4: solver layer

Build:

- `arc-solvers`

Deliver:

- ARC DSL and interpreter
- common verifier
- symbolic induction lane
- random baseline
- minimal prompt-based LLM baseline
- ADCR-style interactive baseline

Acceptance:

- Rust-native solver harness with no Python dependency
- non-trivial solved set on internal dev fixtures

### Phase 5: Psionic primitive expansion

Land the reusable missing primitives above in Psionic.

Acceptance:

- interactive-environment substrate strong enough for ARC-AGI-3 episodes
- framework-core strong enough for the first HRM-class model port

### Phase 6: `arc-ml`

Build:

- `arc-ml`

Deliver:

- evaluator logic first
- small transformer baseline second
- HRM variant third

Acceptance:

- `pass@k` parity on evaluator fixtures before claiming model parity
- model-level parity only on tiny deterministic fixtures before any large-scale
  training claims

## 13. Initial coding-agent backlog

### Phase 1: shared core

1. Create `crates/arc/core` as package `arc-core`.
2. Add `ArcGrid`, `ArcTask`, and ARC-AGI-3 action/frame types.
3. Add canonicalization and deterministic hash.
4. Add connected components, holes, bounding boxes, and symmetry signatures.
5. Add relation graph and correspondence candidates.
6. Add task-budget and solve-result envelope types.
7. Add fixture pack for parser and object extraction.

### Phase 2: datasets and engine

8. Create `crates/arc/datasets` and wire `psionic-data` manifests.
9. Port augmentation builders and dataset lineage metadata.
10. Create `crates/arc/engine` and port deterministic game execution.
11. Add replay-safe fixtures from ARC environment samples.
12. Create `crates/arc/client` and port local/remote wrappers plus REST
    compatibility behavior.

### Phase 3: benchmark

13. Create `crates/arc/benchmark`.
14. Add exact-match static scorer.
15. Add interactive RHAE scorer.
16. Add recording, checkpoint, and resume contracts.
17. Add `psionic-eval` and `psionic-research` integration hooks.

### Phase 4: solver

18. Create `crates/arc/solvers`.
19. Define typed DSL enums and serializer.
20. Implement pure interpreter.
21. Add traceable execution nodes and refusal taxonomy.
22. Implement baseline symbolic search.
23. Add repair operators.
24. Add common verifier and arbiter.
25. Add budget controller and trace bundle writer.
26. Add baseline interactive agents and prompt policies.

### Phase 5: model work

27. Create `crates/arc/ml`.
28. Add evaluator wrappers and ARC-specific metrics.
29. Add small transductive baseline through `psionic-train`.
30. Add recursive tiny-model runner.
31. Add search-guide learning dataset generator.
32. Add HRM-class model work only after Psionic primitive gaps are closed.

## 14. Acceptance matrix

Before claiming "real ARC subtree" the following must be green:

- parser/object extraction fixtures
- deterministic engine replay fixtures
- static exact-match scorer
- interactive scorecard and recording parity
- compatibility-server conformance
- benchmark resume and replay fixtures
- common verifier
- full trace and replay bundle generation

Before claiming "real ARC solver" the following must be green:

- deterministic DSL execution
- symbolic lane solves a non-trivial set
- at least one neural or recursive lane
- portfolio beats any single lane on hidden internal holdout
- exact budget accounting exists
- benchmark hygiene is documented and enforced

## 15. Bottom line

OpenAgents should build ARC as a dedicated subtree under `crates/arc/` that
consumes Psionic rather than hiding ARC-specific logic inside Psionic.

The stable shape is:

- `arc-core` for shared ARC schema and canonicalization
- `arc-datasets` for data and synthetic lineage
- `arc-engine` for deterministic interactive execution
- `arc-client` for local/remote wrappers and compatibility behavior
- `arc-benchmark` for scoring, recordings, checkpoints, and eval runs
- `arc-solvers` for DSL, search, verification, arbitration, and agent logic
- `arc-ml` for HRM-class and baseline model work over Psionic

That is the shortest honest path to:

- Rust-native ARC-AGI-1/2 contracts
- Rust-native ARC-AGI-3 benchmark runtime
- a replayable portfolio solver
- a clean Psionic boundary
- staged HRM-class model work without faking missing substrate capability

## 16. Research basis used for this spec

This spec is grounded in:

- Francois Chollet, *On the Measure of Intelligence* (2019)
- ARC Prize 2024 Technical Report
- ARC Prize 2025 Technical Report
- ARC-AGI-2 Technical Report
- ARC-AGI-2 official benchmark material
- ARC-AGI-3 docs and preview writeups
- Li et al., *Combining Induction and Transduction for Abstract Reasoning*
- Pourcel et al., *Self-Improving Language Models for Evolutionary Program
  Synthesis*
- Jolicoeur-Martineau, *Less is More: Recursive Reasoning with Tiny Networks*
- Liao and Gu, *ARC-AGI Without Pretraining*
